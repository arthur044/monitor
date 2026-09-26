//! Uso e VRAM da GPU pelos contadores de desempenho do Windows (PDH), os
//! mesmos que o Gerenciador de Tarefas usa. Funciona com qualquer fabricante.
//! Os nomes e a VRAM total das placas vêm do DXGI.

use anyhow::{Context, Result, bail};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_ERROR_NOT_FOUND, IDXGIFactory1,
};
use windows::Win32::System::Performance::{
    PDH_CSTATUS_NEW_DATA, PDH_CSTATUS_VALID_DATA, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE,
    PDH_HCOUNTER, PDH_HQUERY, PDH_MORE_DATA, PdhAddEnglishCounterW, PdhCloseQuery,
    PdhCollectQueryData, PdhGetFormattedCounterArrayW, PdhOpenQueryW,
};
use windows::core::{PCWSTR, w};

use crate::modelo::EstadoGpu;
use crate::plataforma::Gpu;
use crate::plataforma::contadores_gpu::{chave_de_luid, soma_por_placa, uso_por_placa};

pub struct GpuPdh {
    estado: Option<(Consulta, Vec<Placa>)>,
}

impl GpuPdh {
    pub fn novo() -> Self {
        Self { estado: None }
    }
}

impl Gpu for GpuPdh {
    fn fonte(&self) -> &'static str {
        "Contadores de desempenho do Windows (PDH)"
    }

    fn ler(&mut self) -> Result<Vec<EstadoGpu>> {
        let (consulta, placas) = match &self.estado {
            Some(estado) => estado,
            None => self.estado.insert((Consulta::abrir()?, listar_placas()?)),
        };

        let resultado = consulta.ler().map(|(uso, memoria)| {
            let uso = uso_por_placa(uso.iter().map(|(n, v)| (n.as_str(), *v)));
            let memoria = soma_por_placa(memoria.iter().map(|(n, v)| (n.as_str(), *v)));
            placas
                .iter()
                .map(|placa| EstadoGpu {
                    nome: placa.nome.clone(),
                    // Sem instâncias significa que nenhum processo está usando a placa.
                    uso_percent: Some(uso.get(&placa.chave).copied().unwrap_or(0.0) as f32),
                    vram_usada_bytes: memoria.get(&placa.chave).map(|&v| v as u64),
                    vram_total_bytes: Some(placa.vram_total),
                })
                .collect()
        });
        if resultado.is_err() {
            self.estado = None;
        }
        resultado
    }
}

struct Placa {
    chave: String,
    nome: String,
    vram_total: u64,
}

fn listar_placas() -> Result<Vec<Placa>> {
    let fabrica: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }.context("CreateDXGIFactory1")?;
    let mut placas = Vec::new();
    for i in 0.. {
        let adaptador = match unsafe { fabrica.EnumAdapters1(i) } {
            Ok(adaptador) => adaptador,
            Err(e) if e.code() == DXGI_ERROR_NOT_FOUND => break,
            Err(e) => return Err(e).context("EnumAdapters1"),
        };
        let desc = unsafe { adaptador.GetDesc1() }.context("GetDesc1")?;
        // "Microsoft Basic Render Driver": nem sempre vem marcado como software,
        // então também comparamos com o identificador conhecido dele.
        let software = desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0;
        if software || (desc.VendorId, desc.DeviceId) == (0x1414, 0x8C) {
            continue;
        }
        let fim = desc
            .Description
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(desc.Description.len());
        placas.push(Placa {
            chave: chave_de_luid(desc.AdapterLuid.HighPart, desc.AdapterLuid.LowPart),
            nome: String::from_utf16_lossy(&desc.Description[..fim]),
            vram_total: desc.DedicatedVideoMemory as u64,
        });
    }
    Ok(placas)
}

struct Consulta {
    consulta: PDH_HQUERY,
    uso: PDH_HCOUNTER,
    memoria: PDH_HCOUNTER,
}

type Itens = Vec<(String, f64)>;

impl Consulta {
    fn abrir() -> Result<Self> {
        let mut consulta = PDH_HQUERY::default();
        verificar(
            unsafe { PdhOpenQueryW(PCWSTR::null(), 0, &mut consulta) },
            "PdhOpenQueryW",
        )?;
        // Criado já aqui para o Drop fechar a consulta se algo abaixo falhar.
        let mut c = Consulta {
            consulta,
            uso: PDH_HCOUNTER::default(),
            memoria: PDH_HCOUNTER::default(),
        };

        let caminho = w!(r"\GPU Engine(*)\Utilization Percentage");
        verificar(
            unsafe { PdhAddEnglishCounterW(c.consulta, caminho, 0, &mut c.uso) },
            "contador de uso da GPU",
        )?;
        let caminho = w!(r"\GPU Adapter Memory(*)\Dedicated Usage");
        verificar(
            unsafe { PdhAddEnglishCounterW(c.consulta, caminho, 0, &mut c.memoria) },
            "contador de VRAM",
        )?;

        // Contadores de taxa precisam de duas coletas; esta é a primeira.
        verificar(
            unsafe { PdhCollectQueryData(c.consulta) },
            "PdhCollectQueryData",
        )?;
        Ok(c)
    }

    fn ler(&self) -> Result<(Itens, Itens)> {
        verificar(
            unsafe { PdhCollectQueryData(self.consulta) },
            "PdhCollectQueryData",
        )?;
        Ok((ler_itens(self.uso)?, ler_itens(self.memoria)?))
    }
}

impl Drop for Consulta {
    fn drop(&mut self) {
        unsafe { PdhCloseQuery(self.consulta) };
    }
}

/// Lê todas as instâncias de um contador com curinga (`(*)`).
fn ler_itens(contador: PDH_HCOUNTER) -> Result<Itens> {
    // Novas instâncias podem surgir entre a consulta do tamanho e a leitura; tenta de novo.
    for _ in 0..3 {
        let mut tamanho = 0u32;
        let mut quantidade = 0u32;
        let status = unsafe {
            PdhGetFormattedCounterArrayW(
                contador,
                PDH_FMT_DOUBLE,
                &mut tamanho,
                &mut quantidade,
                None,
            )
        };
        if status == 0 {
            return Ok(Vec::new());
        }
        if status != PDH_MORE_DATA {
            bail!("PdhGetFormattedCounterArrayW falhou (0x{status:08X})");
        }

        // O buffer guarda os itens seguidos dos nomes; u64 garante o alinhamento.
        let mut buffer = vec![0u64; (tamanho as usize).div_ceil(8)];
        let itens_ptr = buffer.as_mut_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>();
        let status = unsafe {
            PdhGetFormattedCounterArrayW(
                contador,
                PDH_FMT_DOUBLE,
                &mut tamanho,
                &mut quantidade,
                Some(itens_ptr),
            )
        };
        if status == PDH_MORE_DATA {
            continue;
        }
        verificar(status, "PdhGetFormattedCounterArrayW")?;

        let itens = unsafe { std::slice::from_raw_parts(itens_ptr, quantidade as usize) };
        return Ok(itens
            .iter()
            .filter(|i| {
                matches!(
                    i.FmtValue.CStatus,
                    PDH_CSTATUS_VALID_DATA | PDH_CSTATUS_NEW_DATA
                )
            })
            .filter_map(|i| {
                let nome = unsafe { i.szName.to_string() }.ok()?;
                Some((nome, unsafe { i.FmtValue.Anonymous.doubleValue }))
            })
            .collect());
    }
    bail!("instâncias do contador mudaram durante a leitura")
}

fn verificar(status: u32, o_que: &str) -> Result<()> {
    if status == 0 {
        Ok(())
    } else {
        bail!("{o_que} falhou (0x{status:08X})")
    }
}

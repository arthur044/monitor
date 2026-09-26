//! Uso e VRAM da GPU pelos arquivos do driver em `/sys/class/drm`.
//!
//! O driver `amdgpu` expõe `gpu_busy_percent`, `mem_info_vram_used` e
//! `mem_info_vram_total`. Outros drivers expõem só parte disso ou nada; nesse
//! caso os campos ficam `None`.

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};

use super::{ler_numero, ler_texto};
use crate::modelo::EstadoGpu;
use crate::plataforma::Gpu;

pub struct GpuDrm {
    raiz: PathBuf,
}

impl GpuDrm {
    pub fn novo() -> Self {
        Self::com_raiz("/sys/class/drm")
    }

    pub fn com_raiz(raiz: impl Into<PathBuf>) -> Self {
        Self { raiz: raiz.into() }
    }
}

impl Gpu for GpuDrm {
    fn fonte(&self) -> &'static str {
        "Driver de vídeo (sysfs)"
    }

    fn ler(&mut self) -> Result<Vec<EstadoGpu>> {
        let entradas = fs::read_dir(&self.raiz)
            .with_context(|| format!("não foi possível ler {}", self.raiz.display()))?;

        let mut placas: Vec<(u32, EstadoGpu)> = Vec::new();
        for entrada in entradas {
            let entrada = entrada?;
            // Só "card0", "card1"...; "card0-DP-1" etc. são saídas de vídeo.
            let Some(indice) = entrada
                .file_name()
                .to_str()
                .and_then(|n| n.strip_prefix("card"))
                .and_then(|n| n.parse::<u32>().ok())
            else {
                continue;
            };
            let dispositivo = entrada.path().join("device");
            let Some(fabricante) = ler_texto(&dispositivo.join("vendor")) else {
                continue;
            };

            let nome = ler_texto(&dispositivo.join("product_name"))
                .unwrap_or_else(|| format!("GPU {} (card{indice})", nome_fabricante(&fabricante)));

            placas.push((
                indice,
                EstadoGpu {
                    nome,
                    uso_percent: ler_numero(&dispositivo.join("gpu_busy_percent"))
                        .map(|v| v as f32),
                    vram_usada_bytes: ler_numero(&dispositivo.join("mem_info_vram_used")),
                    vram_total_bytes: ler_numero(&dispositivo.join("mem_info_vram_total")),
                },
            ));
        }

        placas.sort_by_key(|(indice, _)| *indice);
        Ok(placas.into_iter().map(|(_, placa)| placa).collect())
    }
}

fn nome_fabricante(id_pci: &str) -> &str {
    match id_pci {
        "0x1002" => "AMD",
        "0x10de" => "NVIDIA",
        "0x8086" => "Intel",
        outro => outro,
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::path::Path;

    fn escrever(dir: &Path, arquivo: &str, conteudo: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join(arquivo), conteudo).unwrap();
    }

    #[test]
    fn le_placas_amd_e_ignora_saidas_de_video() {
        let raiz = tempfile::tempdir().unwrap();

        let amd = raiz.path().join("card1/device");
        escrever(&amd, "vendor", "0x1002\n");
        escrever(&amd, "gpu_busy_percent", "37\n");
        escrever(&amd, "mem_info_vram_used", "1073741824\n");
        escrever(&amd, "mem_info_vram_total", "8589934592\n");

        // Placa integrada sem os arquivos do amdgpu.
        escrever(&raiz.path().join("card0/device"), "vendor", "0x8086\n");

        // Saída de vídeo, deve ser ignorada.
        escrever(&raiz.path().join("card1-DP-1/device"), "vendor", "0x1002\n");

        let placas = GpuDrm::com_raiz(raiz.path()).ler().unwrap();

        assert_eq!(
            placas,
            vec![
                EstadoGpu {
                    nome: "GPU Intel (card0)".into(),
                    ..Default::default()
                },
                EstadoGpu {
                    nome: "GPU AMD (card1)".into(),
                    uso_percent: Some(37.0),
                    vram_usada_bytes: Some(1 << 30),
                    vram_total_bytes: Some(8 << 30),
                },
            ]
        );
    }
}

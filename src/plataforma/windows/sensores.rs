//! Temperaturas pelo LibreHardwareMonitor, que publica os sensores no WMI
//! (namespace `root\LibreHardwareMonitor`) enquanto está aberto.

use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use wmi::WMIConnection;

use crate::modelo::{Componente, LeituraTemperatura};
use crate::plataforma::Sensores;

const NAMESPACE: &str = r"ROOT\LibreHardwareMonitor";

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct SensorLhm {
    name: String,
    parent: String,
    value: Option<f32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct HardwareLhm {
    identifier: String,
    name: String,
}

pub struct SensoresLhm {
    conexao: Option<WMIConnection>,
}

impl SensoresLhm {
    pub fn novo() -> Self {
        Self { conexao: None }
    }
}

impl Sensores for SensoresLhm {
    fn fonte(&self) -> &'static str {
        "LibreHardwareMonitor (WMI)"
    }

    fn ler(&mut self) -> Result<Vec<LeituraTemperatura>> {
        let conexao = match &self.conexao {
            Some(conexao) => conexao,
            None => self
                .conexao
                .insert(WMIConnection::with_namespace_path(NAMESPACE).context(
                "LibreHardwareMonitor não encontrado. Abra-o como administrador e tente de novo",
            )?),
        };

        let resultado = consultar(conexao);
        if resultado.is_err() {
            // O LHM pode ter sido fechado; reconecta na próxima leitura.
            self.conexao = None;
        }
        resultado
    }
}

fn consultar(conexao: &WMIConnection) -> Result<Vec<LeituraTemperatura>> {
    let hardware: Vec<HardwareLhm> = conexao
        .raw_query("SELECT Identifier, Name FROM Hardware")
        .context("falha ao consultar o hardware no LibreHardwareMonitor")?;
    if hardware.is_empty() {
        bail!("LibreHardwareMonitor não está em execução");
    }
    let nomes: HashMap<_, _> = hardware
        .into_iter()
        .map(|h| (h.identifier, h.name))
        .collect();

    let sensores: Vec<SensorLhm> = conexao
        .raw_query("SELECT Name, Parent, Value FROM Sensor WHERE SensorType = 'Temperature'")
        .context("falha ao consultar os sensores no LibreHardwareMonitor")?;

    let mut leituras: Vec<_> = sensores
        .into_iter()
        .filter_map(|s| {
            Some(LeituraTemperatura {
                componente: classificar(&s.parent),
                dispositivo: nomes
                    .get(&s.parent)
                    .cloned()
                    .unwrap_or_else(|| s.parent.clone()),
                sensor: s.name,
                celsius: s.value?,
            })
        })
        .collect();
    leituras.sort_by(|a, b| {
        (a.componente, &a.dispositivo, &a.sensor).cmp(&(b.componente, &b.dispositivo, &b.sensor))
    });
    Ok(leituras)
}

/// Classifica pelo identificador do hardware no LHM, ex.: `/amdcpu/0`, `/gpu-amd/0`, `/nvme/1`.
fn classificar(identificador: &str) -> Componente {
    let tipo = identificador
        .trim_start_matches('/')
        .split('/')
        .next()
        .unwrap_or("");
    match tipo {
        t if t.ends_with("cpu") => Componente::Cpu,
        t if t.starts_with("gpu") || t == "atigpu" || t == "nvidiagpu" => Componente::Gpu,
        "nvme" | "hdd" | "ssd" | "storage" => Componente::Disco,
        "lpc" | "mainboard" | "motherboard" => Componente::PlacaMae,
        _ => Componente::Outro,
    }
}

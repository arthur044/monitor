//! Temperaturas pelos sensores do kernel (`/sys/class/hwmon`).

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};

use super::ler_texto;
use crate::modelo::{Componente, LeituraTemperatura};
use crate::plataforma::Sensores;

pub struct SensoresHwmon {
    raiz: PathBuf,
}

impl SensoresHwmon {
    pub fn novo() -> Self {
        Self::com_raiz("/sys/class/hwmon")
    }

    pub fn com_raiz(raiz: impl Into<PathBuf>) -> Self {
        Self { raiz: raiz.into() }
    }
}

impl Sensores for SensoresHwmon {
    fn fonte(&self) -> &'static str {
        "Sensores do kernel (hwmon)"
    }

    fn ler(&mut self) -> Result<Vec<LeituraTemperatura>> {
        let entradas = fs::read_dir(&self.raiz)
            .with_context(|| format!("não foi possível ler {}", self.raiz.display()))?;

        let mut leituras = Vec::new();
        for entrada in entradas {
            let dir = entrada?.path();
            let Some(chip) = ler_texto(&dir.join("name")) else {
                continue;
            };
            let componente = classificar_chip(&chip);

            let mut indices: Vec<u32> = fs::read_dir(&dir)?
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    let nome = e.file_name();
                    let nome = nome.to_str()?;
                    nome.strip_prefix("temp")?
                        .strip_suffix("_input")?
                        .parse()
                        .ok()
                })
                .collect();
            indices.sort_unstable();

            for n in indices {
                // Alguns sensores falham na leitura (EIO) quando o dispositivo está dormindo.
                let Some(mili) = ler_texto(&dir.join(format!("temp{n}_input")))
                    .and_then(|t| t.parse::<i64>().ok())
                else {
                    continue;
                };
                let sensor = ler_texto(&dir.join(format!("temp{n}_label")))
                    .unwrap_or_else(|| format!("temp{n}"));
                leituras.push(LeituraTemperatura {
                    componente,
                    dispositivo: chip.clone(),
                    sensor,
                    celsius: mili as f32 / 1000.0,
                });
            }
        }

        leituras.sort_by(|a, b| {
            (a.componente, &a.dispositivo, &a.sensor).cmp(&(
                b.componente,
                &b.dispositivo,
                &b.sensor,
            ))
        });
        Ok(leituras)
    }
}

fn classificar_chip(chip: &str) -> Componente {
    match chip {
        "k10temp" | "coretemp" | "zenpower" | "cpu_thermal" => Componente::Cpu,
        "amdgpu" | "radeon" | "nouveau" | "i915" | "xe" => Componente::Gpu,
        "nvme" | "drivetemp" => Componente::Disco,
        "acpitz" => Componente::PlacaMae,
        // Chips Super I/O das placas-mãe (Nuvoton, ITE, ASUS EC etc.).
        c if c.starts_with("nct") || c.starts_with("it87") || c.starts_with("asus") => {
            Componente::PlacaMae
        }
        _ => Componente::Outro,
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
    fn le_e_classifica_sensores() {
        let raiz = tempfile::tempdir().unwrap();
        let cpu = raiz.path().join("hwmon0");
        escrever(&cpu, "name", "k10temp\n");
        escrever(&cpu, "temp1_input", "45250\n");
        escrever(&cpu, "temp1_label", "Tctl\n");
        escrever(&cpu, "temp3_input", "40000\n"); // sem label

        let gpu = raiz.path().join("hwmon1");
        escrever(&gpu, "name", "amdgpu\n");
        escrever(&gpu, "temp1_input", "61000\n");
        escrever(&gpu, "temp1_label", "edge\n");

        // Chip sem temperaturas e sensor com leitura inválida devem ser ignorados.
        escrever(&raiz.path().join("hwmon2"), "name", "BAT0\n");
        let nvme = raiz.path().join("hwmon3");
        escrever(&nvme, "name", "nvme\n");
        escrever(&nvme, "temp1_input", "");

        let leituras = SensoresHwmon::com_raiz(raiz.path()).ler().unwrap();

        let resumo: Vec<_> = leituras
            .iter()
            .map(|l| {
                (
                    l.componente,
                    l.dispositivo.as_str(),
                    l.sensor.as_str(),
                    l.celsius,
                )
            })
            .collect();
        assert_eq!(
            resumo,
            vec![
                (Componente::Cpu, "k10temp", "Tctl", 45.25),
                (Componente::Cpu, "k10temp", "temp3", 40.0),
                (Componente::Gpu, "amdgpu", "edge", 61.0),
            ]
        );
    }

    #[test]
    fn raiz_inexistente_da_erro() {
        assert!(
            SensoresHwmon::com_raiz("/caminho/que/nao/existe")
                .ler()
                .is_err()
        );
    }
}

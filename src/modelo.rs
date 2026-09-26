//! Tipos comuns a todas as plataformas.

/// Parte do computador a que um sensor pertence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Componente {
    Cpu,
    Gpu,
    Disco,
    PlacaMae,
    Outro,
}

impl Componente {
    pub fn nome(self) -> &'static str {
        match self {
            Componente::Cpu => "CPU",
            Componente::Gpu => "GPU",
            Componente::Disco => "Disco",
            Componente::PlacaMae => "Placa-mãe",
            Componente::Outro => "Outro",
        }
    }
}

/// Uma leitura de temperatura.
#[derive(Debug, Clone, PartialEq)]
pub struct LeituraTemperatura {
    pub componente: Componente,
    /// Dispositivo físico, ex.: "AMD Ryzen 7 5800X" ou "k10temp".
    pub dispositivo: String,
    /// Sensor dentro do dispositivo, ex.: "Tctl" ou "GPU Core".
    pub sensor: String,
    pub celsius: f32,
}

/// Estado de uma placa de vídeo. Campos ficam `None` quando a fonte não informa.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EstadoGpu {
    pub nome: String,
    pub uso_percent: Option<f32>,
    pub vram_usada_bytes: Option<u64>,
    pub vram_total_bytes: Option<u64>,
}

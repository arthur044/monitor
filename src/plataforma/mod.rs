//! Tudo que depende do sistema operacional fica atrás destas traits.
//!
//! Cada SO tem sua implementação, escolhida em tempo de compilação. As
//! implementações iniciam suas fontes de forma preguiçosa: se uma fonte não
//! está disponível (ex.: LibreHardwareMonitor fechado), `ler` devolve erro e a
//! próxima chamada tenta de novo.

use anyhow::Result;

use crate::modelo::{EstadoGpu, LeituraTemperatura};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

#[cfg(any(windows, test))]
mod contadores_gpu;

#[cfg(not(any(windows, target_os = "linux")))]
compile_error!("O monitor suporta apenas Windows e Linux.");

pub trait Sensores {
    /// Nome da fonte de dados, para exibir ao usuário.
    fn fonte(&self) -> &'static str;
    fn ler(&mut self) -> Result<Vec<LeituraTemperatura>>;
}

pub trait Gpu {
    /// Nome da fonte de dados, para exibir ao usuário.
    fn fonte(&self) -> &'static str;
    fn ler(&mut self) -> Result<Vec<EstadoGpu>>;
}

pub fn sensores() -> Box<dyn Sensores> {
    #[cfg(target_os = "linux")]
    return Box::new(linux::SensoresHwmon::novo());
    #[cfg(windows)]
    return Box::new(windows::SensoresLhm::novo());
}

pub fn gpu() -> Box<dyn Gpu> {
    #[cfg(target_os = "linux")]
    return Box::new(linux::GpuDrm::novo());
    #[cfg(windows)]
    return Box::new(windows::GpuPdh::novo());
}

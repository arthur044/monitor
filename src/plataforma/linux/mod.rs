mod gpu;
mod sensores;

use std::fs;
use std::path::Path;

pub use gpu::GpuDrm;
pub use sensores::SensoresHwmon;

/// Lê um arquivo do sysfs sem a quebra de linha final. `None` se não existir ou falhar.
fn ler_texto(caminho: &Path) -> Option<String> {
    let texto = fs::read_to_string(caminho).ok()?;
    let texto = texto.trim();
    (!texto.is_empty()).then(|| texto.to_owned())
}

fn ler_numero(caminho: &Path) -> Option<u64> {
    ler_texto(caminho)?.parse().ok()
}

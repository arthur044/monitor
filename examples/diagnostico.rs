//! Diagnóstico da Fase 0: imprime temperaturas e GPU cinco vezes, a cada 2 s.
//!
//! Uso: `cargo run --example diagnostico`

use std::thread;
use std::time::Duration;

use monitor::plataforma;

fn main() {
    let mut sensores = plataforma::sensores();
    let mut gpu = plataforma::gpu();
    println!("Fonte de temperaturas: {}", sensores.fonte());
    println!("Fonte de GPU:          {}", gpu.fonte());

    const VEZES: u32 = 5;
    for i in 1..=VEZES {
        println!("\n=== Leitura {i} de {VEZES} ===");

        match sensores.ler() {
            Ok(v) if v.is_empty() => println!("Temperaturas: nenhum sensor encontrado"),
            Ok(v) => {
                println!("Temperaturas:");
                for l in v {
                    println!(
                        "  [{}] {} / {}: {:.1} °C",
                        l.componente.nome(),
                        l.dispositivo,
                        l.sensor,
                        l.celsius
                    );
                }
            }
            Err(e) => println!("Temperaturas: ERRO: {e:#}"),
        }

        match gpu.ler() {
            Ok(v) if v.is_empty() => println!("GPU: nenhuma placa encontrada"),
            Ok(v) => {
                println!("GPU:");
                for g in v {
                    println!(
                        "  {}: uso {}, VRAM {} / {} MiB",
                        g.nome,
                        g.uso_percent.map_or("n/d".into(), |u| format!("{u:.0}%")),
                        g.vram_usada_bytes
                            .map_or("?".into(), |b| (b >> 20).to_string()),
                        g.vram_total_bytes
                            .map_or("?".into(), |b| (b >> 20).to_string()),
                    );
                }
            }
            Err(e) => println!("GPU: ERRO: {e:#}"),
        }

        if i < VEZES {
            thread::sleep(Duration::from_secs(2));
        }
    }

    // Ao abrir com duplo clique no Windows, o console fecharia antes de dar para ler.
    #[cfg(windows)]
    {
        println!("\nPressione Enter para sair...");
        let _ = std::io::stdin().read_line(&mut String::new());
    }
}

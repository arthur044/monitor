//! Protótipo da Fase 0: janela com temperaturas e GPU ao vivo, coleta em
//! thread própria e ícone na bandeja. Fechar a janela só a esconde; a coleta
//! continua e o contador de leituras comprova isso ao reabrir.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui;
use monitor::bandeja::{AcaoBandeja, Bandeja};
use monitor::modelo::{EstadoGpu, LeituraTemperatura};
use monitor::{icone, plataforma};

const INTERVALO_COLETA: Duration = Duration::from_secs(2);

#[derive(Default)]
struct Leituras {
    fonte_temperaturas: &'static str,
    fonte_gpu: &'static str,
    temperaturas: Option<Result<Vec<LeituraTemperatura>, String>>,
    gpus: Option<Result<Vec<EstadoGpu>, String>>,
    total: u64,
    ultima: Option<Instant>,
}

fn main() -> eframe::Result {
    tracing_subscriber::fmt::init();

    let ctx = egui::Context::default();
    let leituras = Arc::new(Mutex::new(Leituras::default()));
    iniciar_coleta(leituras.clone(), ctx.clone());

    let sair = Arc::new(AtomicBool::new(false));
    let bandeja = {
        let (ctx, sair) = (ctx.clone(), sair.clone());
        Bandeja::iniciar(move |acao| {
            match acao {
                AcaoBandeja::Abrir => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                AcaoBandeja::Sair => {
                    sair.store(true, Ordering::Relaxed);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            ctx.request_repaint();
        })
    };
    let bandeja = bandeja
        .inspect_err(|e| tracing::warn!("bandeja indisponível: {e:#}"))
        .ok();

    let opcoes = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Monitor")
            .with_inner_size([560.0, 480.0])
            .with_icon(egui::IconData {
                rgba: icone::rgba(64),
                width: 64,
                height: 64,
            }),
        ..Default::default()
    };
    let app = App {
        leituras,
        sair,
        com_bandeja: bandeja.is_some(),
    };
    let resultado = eframe::run_native_ext(
        "Monitor",
        opcoes,
        Some(ctx),
        Box::new(|_| Ok(Box::new(app))),
    );

    drop(bandeja);
    resultado
}

fn iniciar_coleta(leituras: Arc<Mutex<Leituras>>, ctx: egui::Context) {
    thread::Builder::new()
        .name("coleta".into())
        .spawn(move || {
            let mut sensores = plataforma::sensores();
            let mut gpu = plataforma::gpu();
            loop {
                let temperaturas = sensores.ler().map_err(|e| format!("{e:#}"));
                let gpus = gpu.ler().map_err(|e| format!("{e:#}"));
                {
                    let mut l = leituras.lock().unwrap();
                    l.fonte_temperaturas = sensores.fonte();
                    l.fonte_gpu = gpu.fonte();
                    l.temperaturas = Some(temperaturas);
                    l.gpus = Some(gpus);
                    l.total += 1;
                    l.ultima = Some(Instant::now());
                }
                ctx.request_repaint();
                thread::sleep(INTERVALO_COLETA);
            }
        })
        .expect("falha ao criar a thread de coleta");
}

struct App {
    leituras: Arc<Mutex<Leituras>>,
    sair: Arc<AtomicBool>,
    com_bandeja: bool,
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let fechar = ctx.input(|i| i.viewport().close_requested());
        if fechar && self.com_bandeja && !self.sair.load(Ordering::Relaxed) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let l = self.leituras.lock().unwrap();
                ui.heading("Monitor: protótipo da Fase 0");
                cabecalho(ui, &l, self.com_bandeja);

                ui.separator();
                ui.heading("Temperaturas");
                ui.weak(format!("Fonte: {}", l.fonte_temperaturas));
                match &l.temperaturas {
                    Some(Ok(v)) if v.is_empty() => {
                        ui.label("Nenhum sensor de temperatura encontrado.");
                    }
                    Some(Ok(v)) => tabela_temperaturas(ui, v),
                    Some(Err(e)) => erro(ui, e),
                    None => aguardando(ui),
                }

                ui.separator();
                ui.heading("GPU");
                ui.weak(format!("Fonte: {}", l.fonte_gpu));
                match &l.gpus {
                    Some(Ok(v)) if v.is_empty() => {
                        ui.label("Nenhuma placa de vídeo encontrada.");
                    }
                    Some(Ok(v)) => tabela_gpus(ui, v),
                    Some(Err(e)) => erro(ui, e),
                    None => aguardando(ui),
                }
            });
        });
    }
}

fn cabecalho(ui: &mut egui::Ui, l: &Leituras, com_bandeja: bool) {
    let idade = l
        .ultima
        .map(|t| format!("última há {} s", t.elapsed().as_secs()))
        .unwrap_or_else(|| "nenhuma ainda".into());
    ui.label(format!(
        "Leituras feitas: {} (a cada {} s, {idade})",
        l.total,
        INTERVALO_COLETA.as_secs()
    ));
    if com_bandeja {
        ui.label(
            "Fechar a janela esconde o monitor na bandeja e a coleta continua. \
             Para encerrar, use \"Sair\" no menu do ícone.",
        );
    } else {
        ui.colored_label(
            ui.visuals().warn_fg_color,
            "Bandeja indisponível: fechar a janela encerra o programa.",
        );
    }
}

fn tabela_temperaturas(ui: &mut egui::Ui, leituras: &[LeituraTemperatura]) {
    egui::Grid::new("temperaturas")
        .striped(true)
        .num_columns(4)
        .show(ui, |ui| {
            for titulo in ["Componente", "Dispositivo", "Sensor", "°C"] {
                ui.strong(titulo);
            }
            ui.end_row();
            for l in leituras {
                ui.label(l.componente.nome());
                ui.label(&l.dispositivo);
                ui.label(&l.sensor);
                let texto = format!("{:.1}", l.celsius);
                if l.celsius >= 85.0 {
                    ui.colored_label(ui.visuals().error_fg_color, texto);
                } else if l.celsius >= 70.0 {
                    ui.colored_label(ui.visuals().warn_fg_color, texto);
                } else {
                    ui.label(texto);
                }
                ui.end_row();
            }
        });
}

fn tabela_gpus(ui: &mut egui::Ui, gpus: &[EstadoGpu]) {
    egui::Grid::new("gpus")
        .striped(true)
        .num_columns(3)
        .show(ui, |ui| {
            for titulo in ["Placa", "Uso", "VRAM"] {
                ui.strong(titulo);
            }
            ui.end_row();
            for g in gpus {
                ui.label(&g.nome);
                ui.label(g.uso_percent.map_or("n/d".into(), |u| format!("{u:.0}%")));
                ui.label(match (g.vram_usada_bytes, g.vram_total_bytes) {
                    (Some(usada), Some(total)) => format!("{} / {}", gib(usada), gib(total)),
                    (Some(usada), None) => gib(usada),
                    (None, Some(total)) => format!("? / {}", gib(total)),
                    (None, None) => "n/d".into(),
                });
                ui.end_row();
            }
        });
}

fn gib(bytes: u64) -> String {
    format!("{:.1} GiB", bytes as f64 / (1u64 << 30) as f64)
}

fn erro(ui: &mut egui::Ui, mensagem: &str) {
    ui.colored_label(ui.visuals().error_fg_color, mensagem);
}

fn aguardando(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.spinner();
        ui.label("Aguardando a primeira leitura…");
    });
}

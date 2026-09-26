//! Ícone na bandeja do sistema, com o menu "Abrir monitor" / "Sair".
//!
//! - Windows: o ícone precisa ser criado na thread principal, antes de o
//!   eframe iniciar; o laço de eventos da janela processa as mensagens dele.
//! - Linux: o ícone usa GTK + AppIndicator, que rodam numa thread própria
//!   com o laço de eventos do GTK.

use std::sync::Arc;

use anyhow::Result;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use crate::icone;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcaoBandeja {
    Abrir,
    Sair,
}

type AoAcionar = Arc<dyn Fn(AcaoBandeja) + Send + Sync>;

pub struct Bandeja {
    #[cfg(windows)]
    _icone: TrayIcon,
    #[cfg(target_os = "linux")]
    thread_gtk: Option<std::thread::JoinHandle<()>>,
}

impl Bandeja {
    /// Cria o ícone. `ao_acionar` é chamado a partir da thread da bandeja.
    #[cfg(windows)]
    pub fn iniciar(ao_acionar: impl Fn(AcaoBandeja) + Send + Sync + 'static) -> Result<Self> {
        Ok(Self {
            _icone: criar_icone(Arc::new(ao_acionar))?,
        })
    }

    /// Cria o ícone. `ao_acionar` é chamado a partir da thread da bandeja.
    #[cfg(target_os = "linux")]
    pub fn iniciar(ao_acionar: impl Fn(AcaoBandeja) + Send + Sync + 'static) -> Result<Self> {
        use anyhow::{Context, anyhow};

        let ao_acionar: AoAcionar = Arc::new(ao_acionar);
        let (pronto_tx, pronto_rx) = std::sync::mpsc::channel();
        let thread_gtk = std::thread::Builder::new()
            .name("bandeja-gtk".into())
            .spawn(move || {
                if let Err(e) = gtk::init() {
                    let _ = pronto_tx.send(Err(anyhow!("GTK indisponível: {e}")));
                    return;
                }
                match criar_icone(ao_acionar) {
                    Ok(icone) => {
                        let _ = pronto_tx.send(Ok(()));
                        gtk::main();
                        drop(icone);
                    }
                    Err(e) => {
                        let _ = pronto_tx.send(Err(e));
                    }
                }
            })?;

        // Se a thread entrar em pânico (ex.: libayatana-appindicator3 ausente), o canal fecha.
        pronto_rx
            .recv()
            .context("a bandeja falhou ao iniciar (libayatana-appindicator3 instalada?)")??;
        Ok(Self {
            thread_gtk: Some(thread_gtk),
        })
    }
}

#[cfg(target_os = "linux")]
impl Drop for Bandeja {
    fn drop(&mut self) {
        gtk::glib::MainContext::default().invoke(gtk::main_quit);
        if let Some(thread) = self.thread_gtk.take() {
            let _ = thread.join();
        }
    }
}

fn criar_icone(ao_acionar: AoAcionar) -> Result<TrayIcon> {
    let abrir = MenuItem::new("Abrir monitor", true, None);
    let sair = MenuItem::new("Sair", true, None);
    let menu = Menu::new();
    menu.append_items(&[&abrir, &PredefinedMenuItem::separator(), &sair])?;

    let (id_abrir, id_sair) = (abrir.id().clone(), sair.id().clone());
    let acionar = ao_acionar.clone();
    MenuEvent::set_event_handler(Some(move |evento: MenuEvent| {
        if evento.id == id_abrir {
            acionar(AcaoBandeja::Abrir);
        } else if evento.id == id_sair {
            acionar(AcaoBandeja::Sair);
        }
    }));

    // No Windows, clique esquerdo abre a janela e o direito mostra o menu.
    // No Linux o AppIndicator só oferece o menu.
    #[cfg(windows)]
    tray_icon::TrayIconEvent::set_event_handler(Some(move |evento| {
        use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = evento
        {
            ao_acionar(AcaoBandeja::Abrir);
        }
    }));

    let lado = 32;
    Ok(TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .with_tooltip("Monitor")
        .with_icon(Icon::from_rgba(icone::rgba(lado), lado, lado)?)
        .build()?)
}

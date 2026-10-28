//! L'«isola»: Oltremare in una pillola nera in alto al centro dello schermo, attorno alla notch
//! sui Mac che ce l'hanno (come la Dynamic Island). Piccola a riposo, si allarga quando ascolta,
//! lavora, parla, chiede o avvisa. La finestra segue le misure della pillola: niente aree
//! trasparenti che coprono la barra dei menu.

use serde::Serialize;
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager};

#[derive(Serialize, Clone, Copy, Default)]
#[serde(rename_all = "camelCase")]
pub struct Notch {
    /// Il Mac ha la notch (o la «tacca» della fotocamera)
    pub present: bool,
    /// Larghezza della notch in punti
    pub width: f64,
    /// Altezza della barra dei menu (con notch coincide con la sua altezza)
    pub height: f64,
}

#[cfg(target_os = "macos")]
fn misura(app: &AppHandle) -> Notch {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSScreen;
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = app.run_on_main_thread(move || {
        let Some(mtm) = MainThreadMarker::new() else { return };
        let Some(s) = NSScreen::mainScreen(mtm) else { return };
        let frame = s.frame();
        let visibile = s.visibleFrame();
        let alto = s.safeAreaInsets().top;
        let barra = (frame.size.height - (visibile.origin.y + visibile.size.height - frame.origin.y)).max(24.0);
        let n = if alto > 0.0 {
            let sx = s.auxiliaryTopLeftArea().size.width;
            let dx = s.auxiliaryTopRightArea().size.width;
            Notch { present: true, width: (frame.size.width - sx - dx).max(150.0), height: alto }
        } else {
            Notch { present: false, width: 0.0, height: barra }
        };
        let _ = tx.send(n);
    });
    rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap_or(Notch { present: false, width: 0.0, height: 25.0 })
}

#[cfg(not(target_os = "macos"))]
fn misura(_app: &AppHandle) -> Notch {
    Notch { present: false, width: 0.0, height: 0.0 }
}

pub fn notch(app: &AppHandle) -> Notch {
    misura(app)
}

/// Sopra la barra dei menu, su tutte le scrivanie e sopra le app a tutto schermo.
pub fn prepara(app: &AppHandle) {
    let Some(w) = app.get_webview_window("isola") else { return };
    #[cfg(target_os = "macos")]
    {
        let w2 = w.clone();
        let _ = app.run_on_main_thread(move || {
            let Ok(ptr) = w2.ns_window() else { return };
            // SAFETY: puntatore alla NSWindow della finestra, usato sul thread principale
            let win: &objc2_app_kit::NSWindow = unsafe { &*(ptr as *const objc2_app_kit::NSWindow) };
            win.setLevel(25); // NSStatusWindowLevel: sopra la barra dei menu
            use objc2_app_kit::NSWindowCollectionBehavior as B;
            win.setCollectionBehavior(B::CanJoinAllSpaces | B::Stationary | B::FullScreenAuxiliary | B::IgnoresCycle);
            win.setHasShadow(false);
        });
    }
    let _ = w.set_always_on_top(true);
}

/// La finestra prende la misura della pillola, centrata in alto.
pub fn dimensiona(app: &AppHandle, larghezza: f64, altezza: f64) {
    let Some(w) = app.get_webview_window("isola") else { return };
    let Ok(Some(m)) = w.primary_monitor() else { return };
    let scala = m.scale_factor();
    let schermo = m.size().to_logical::<f64>(scala);
    let origine = m.position().to_logical::<f64>(scala);
    let n = notch(app);
    // Con la notch la pillola parte dal bordo alto; senza, sta appena sotto la barra dei menu
    let y = if n.present { 0.0 } else if cfg!(target_os = "macos") { n.height + 6.0 } else { 8.0 };
    let x = origine.x + (schermo.width - larghezza) / 2.0;
    let _ = w.set_size(LogicalSize::new(larghezza, altezza));
    let _ = w.set_position(LogicalPosition::new(x, origine.y + y));
}

pub fn mostra(app: &AppHandle, visibile: bool) {
    let Some(w) = app.get_webview_window("isola") else { return };
    if visibile {
        prepara(app);
        let n = notch(app);
        dimensiona(app, if n.present { n.width + 80.0 } else { 190.0 }, if n.present { n.height } else { 34.0 });
        let _ = w.show();
    } else {
        let _ = w.hide();
    }
}

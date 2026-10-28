//! Controllo dello schermo: catture (xcap), mouse e tastiera (enigo).
//!
//! Le coordinate del modello sono nello spazio dello screenshot che gli abbiamo mandato
//! (lato lungo 1280 px): si riportano ai punti dello schermo principale prima di agire.
//! Protezioni: un'azione alla volta, stop se l'utente muove il mouse, niente azioni sulle app
//! che custodiscono password, interruttore di emergenza.

use super::Esito;
use base64::Engine;
use enigo::{Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

const LATO: u32 = 1280;

#[derive(Debug, Clone, PartialEq)]
pub enum Azione {
    Screenshot,
    Zoom([f64; 4]),
    Posizione,
    Clic { pulsante: Button, volte: u32, dove: Option<(f64, f64)>, tasti: Vec<String> },
    Premi(bool),
    Sposta((f64, f64)),
    Trascina { da: (f64, f64), a: (f64, f64), tasti: Vec<String> },
    Scorri { direzione: String, quanto: i32, dove: Option<(f64, f64)>, tasti: Vec<String> },
    Scrivi(String),
    Tasti { combo: String, ripeti: u32 },
    Tieni { combo: String, secondi: f64 },
    Attendi(f64),
}

fn punto(v: &Value) -> Option<(f64, f64)> {
    let a = v.as_array()?;
    Some((a.first()?.as_f64()?, a.get(1)?.as_f64()?))
}

fn tasti_mod(v: &Value) -> Vec<String> {
    v.as_str().unwrap_or_default().split('+').map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect()
}

/// Azioni del toolset `computer_toolset_20260801` di Claude.
pub fn da_toolset(nome: &str, i: &Value) -> Result<Azione, String> {
    let dove = punto(&i["coordinate"]);
    let tasti = tasti_mod(&i["text"]);
    Ok(match nome {
        "screenshot" => Azione::Screenshot,
        "zoom" => {
            let r = i["region"].as_array().ok_or("Serve region [x0, y0, x1, y1]")?;
            let n: Vec<f64> = r.iter().filter_map(Value::as_f64).collect();
            if n.len() != 4 {
                return Err("region deve avere 4 numeri".into());
            }
            Azione::Zoom([n[0], n[1], n[2], n[3]])
        }
        "cursor_position" => Azione::Posizione,
        "left_click" => Azione::Clic { pulsante: Button::Left, volte: 1, dove, tasti },
        "right_click" => Azione::Clic { pulsante: Button::Right, volte: 1, dove, tasti },
        "middle_click" => Azione::Clic { pulsante: Button::Middle, volte: 1, dove, tasti },
        "double_click" => Azione::Clic { pulsante: Button::Left, volte: 2, dove, tasti },
        "triple_click" => Azione::Clic { pulsante: Button::Left, volte: 3, dove, tasti },
        "left_mouse_down" => Azione::Premi(true),
        "left_mouse_up" => Azione::Premi(false),
        "mouse_move" => Azione::Sposta(dove.ok_or("Serve coordinate")?),
        "left_click_drag" => Azione::Trascina { da: punto(&i["start_coordinate"]).ok_or("Serve start_coordinate")?, a: dove.ok_or("Serve coordinate")?, tasti },
        "scroll" => Azione::Scorri {
            direzione: i["scroll_direction"].as_str().unwrap_or("down").to_string(),
            quanto: i["scroll_amount"].as_i64().unwrap_or(3) as i32,
            dove,
            tasti,
        },
        "type" => Azione::Scrivi(i["text"].as_str().unwrap_or_default().to_string()),
        "key" => Azione::Tasti { combo: i["text"].as_str().unwrap_or_default().to_string(), ripeti: i["repeat"].as_u64().unwrap_or(1).clamp(1, 100) as u32 },
        "hold_key" => Azione::Tieni { combo: i["text"].as_str().unwrap_or_default().to_string(), secondi: i["duration"].as_f64().unwrap_or(1.0).min(300.0) },
        "wait" => Azione::Attendi(i["duration"].as_f64().unwrap_or(1.0).min(300.0)),
        _ => return Err(format!("Azione sconosciuta: {nome}")),
    })
}

/// Funzioni `schermo_*` (GPT, AI locale, Claude Code).
pub fn da_funzione(nome: &str, i: &Value) -> Result<Azione, String> {
    let xy = |kx: &str, ky: &str| -> Option<(f64, f64)> { Some((i[kx].as_f64()?, i[ky].as_f64()?)) };
    let tasti = tasti_mod(&i["tasti"]);
    Ok(match nome {
        "schermo_screenshot" => Azione::Screenshot,
        "schermo_zoom" => Azione::Zoom([
            i["x1"].as_f64().ok_or("Serve x1")?,
            i["y1"].as_f64().ok_or("Serve y1")?,
            i["x2"].as_f64().ok_or("Serve x2")?,
            i["y2"].as_f64().ok_or("Serve y2")?,
        ]),
        "schermo_posizione" => Azione::Posizione,
        "schermo_clic" => Azione::Clic {
            pulsante: match i["pulsante"].as_str().unwrap_or("sinistro") {
                "destro" => Button::Right,
                "centrale" => Button::Middle,
                _ => Button::Left,
            },
            volte: i["clic"].as_u64().unwrap_or(1).clamp(1, 3) as u32,
            dove: xy("x", "y"),
            tasti,
        },
        "schermo_sposta" => Azione::Sposta(xy("x", "y").ok_or("Servono x e y")?),
        "schermo_trascina" => Azione::Trascina { da: xy("da_x", "da_y").ok_or("Servono da_x e da_y")?, a: xy("a_x", "a_y").ok_or("Servono a_x e a_y")?, tasti },
        "schermo_scorri" => Azione::Scorri {
            direzione: match i["direzione"].as_str().unwrap_or("giu") {
                "su" => "up",
                "sinistra" => "left",
                "destra" => "right",
                _ => "down",
            }
            .into(),
            quanto: i["quantita"].as_i64().unwrap_or(3) as i32,
            dove: xy("x", "y"),
            tasti,
        },
        "schermo_scrivi" => Azione::Scrivi(i["testo"].as_str().unwrap_or_default().to_string()),
        "schermo_tasti" => Azione::Tasti { combo: i["combinazione"].as_str().unwrap_or_default().to_string(), ripeti: i["ripeti"].as_u64().unwrap_or(1).clamp(1, 100) as u32 },
        "attendi" => Azione::Attendi(i["secondi"].as_f64().unwrap_or(1.0).min(300.0)),
        _ => return Err(format!("Azione sconosciuta: {nome}")),
    })
}

/// Lo strumento del catalogo che regola l'azione (policy, titoli).
pub fn spec_di(a: &Azione) -> &'static str {
    match a {
        Azione::Screenshot => "schermo_screenshot",
        Azione::Zoom(_) => "schermo_zoom",
        Azione::Posizione => "schermo_posizione",
        Azione::Clic { .. } | Azione::Premi(_) => "schermo_clic",
        Azione::Sposta(_) => "schermo_sposta",
        Azione::Trascina { .. } => "schermo_trascina",
        Azione::Scorri { .. } => "schermo_scorri",
        Azione::Scrivi(_) => "schermo_scrivi",
        Azione::Tasti { .. } | Azione::Tieni { .. } => "schermo_tasti",
        Azione::Attendi(_) => "attendi",
    }
}

pub fn titolo(a: &Azione) -> String {
    let p = |d: &Option<(f64, f64)>| d.map(|(x, y)| format!(" in ({x:.0}, {y:.0})")).unwrap_or_default();
    match a {
        Azione::Screenshot => "Guarda lo schermo".into(),
        Azione::Zoom(_) => "Ingrandisce una parte dello schermo".into(),
        Azione::Posizione => "Controlla dov'è il mouse".into(),
        Azione::Clic { pulsante, volte, dove, .. } => {
            let tipo = match (pulsante, volte) {
                (Button::Right, _) => "Clic destro",
                (_, 2) => "Doppio clic",
                (_, 3) => "Triplo clic",
                _ => "Clic",
            };
            format!("{tipo}{}", p(dove))
        }
        Azione::Premi(true) => "Tiene premuto il pulsante del mouse".into(),
        Azione::Premi(false) => "Rilascia il pulsante del mouse".into(),
        Azione::Sposta(d) => format!("Sposta il mouse{}", p(&Some(*d))),
        Azione::Trascina { da, a, .. } => format!("Trascina da ({:.0}, {:.0}) a ({:.0}, {:.0})", da.0, da.1, a.0, a.1),
        Azione::Scorri { direzione, .. } => format!(
            "Scorre {}",
            match direzione.as_str() {
                "up" => "in su",
                "left" => "a sinistra",
                "right" => "a destra",
                _ => "in giù",
            }
        ),
        Azione::Scrivi(t) => format!("Scrive «{}»", t.chars().take(50).collect::<String>()),
        Azione::Tasti { combo, ripeti } => format!("Preme {combo}{}", if *ripeti > 1 { format!(" ×{ripeti}") } else { String::new() }),
        Azione::Tieni { combo, secondi } => format!("Tiene premuto {combo} per {secondi:.0} s"),
        Azione::Attendi(s) => format!("Aspetta {s:.0} s"),
    }
}

/// Ultimo screenshot: serve a riportare le coordinate allo schermo.
#[derive(Clone, Copy)]
struct Mappa {
    img_w: f64,
    img_h: f64,
    fis_w: f64,
    fis_h: f64,
    log_w: f64,
    log_h: f64,
}

#[derive(Default)]
pub struct Schermo {
    mappa: Mutex<Option<Mappa>>,
    /// Dove abbiamo lasciato il mouse: se è altrove, l'ha mosso l'utente
    atteso: Mutex<Option<(i32, i32)>>,
    turno: tokio::sync::Mutex<()>,
    contatore: std::sync::atomic::AtomicU32,
}

/// Lo schermo è uno solo: un unico gestore per tutta l'app.
pub fn schermo() -> &'static Schermo {
    static S: std::sync::OnceLock<Schermo> = std::sync::OnceLock::new();
    S.get_or_init(Schermo::default)
}

/// App su cui non si agisce mai.
const VIETATE: &[&str] = &["1password", "bitwarden", "keepass", "dashlane", "lastpass", "portachiavi", "keychain", "password", "passwords", "authy"];

#[cfg(target_os = "macos")]
fn app_in_primo_piano() -> Option<String> {
    let front = std::process::Command::new("lsappinfo").arg("front").output().ok()?;
    let asn = String::from_utf8_lossy(&front.stdout).trim().to_string();
    let info = std::process::Command::new("lsappinfo").args(["info", "-only", "name", &asn]).output().ok()?;
    let s = String::from_utf8_lossy(&info.stdout);
    s.split('=').nth(1).map(|n| n.trim().trim_matches('"').to_string())
}

#[cfg(not(target_os = "macos"))]
fn app_in_primo_piano() -> Option<String> {
    None
}

fn cattura() -> Result<(image::RgbaImage, Mappa), String> {
    let monitors = xcap::Monitor::all().map_err(|e| format!("Non vedo lo schermo: {e}"))?;
    let m = monitors.iter().find(|m| m.is_primary().unwrap_or(false)).or(monitors.first()).ok_or("Nessuno schermo trovato")?;
    let img = m.capture_image().map_err(|e| {
        format!("Non riesco a catturare lo schermo ({e}). Su macOS concedi a Oltremare la Registrazione schermo in Impostazioni di Sistema → Privacy e sicurezza")
    })?;
    let (fw, fh) = (img.width() as f64, img.height() as f64);
    let k = (LATO as f64 / fw.max(fh)).min(1.0);
    let (iw, ih) = ((fw * k).round(), (fh * k).round());
    let enigo = Enigo::new(&Settings::default()).map_err(|e| format!("Controllo del mouse non disponibile: {e}"))?;
    let (lw, lh) = enigo.main_display().map_err(|e| e.to_string())?;
    Ok((img, Mappa { img_w: iw, img_h: ih, fis_w: fw, fis_h: fh, log_w: lw as f64, log_h: lh as f64 }))
}

fn png_base64(img: &image::RgbaImage) -> Result<(String, Vec<u8>), String> {
    let mut buf = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(img.clone()).to_rgb8().write_to(&mut buf, image::ImageFormat::Png).map_err(|e| e.to_string())?;
    let bytes = buf.into_inner();
    Ok((base64::engine::general_purpose::STANDARD.encode(&bytes), bytes))
}

fn ridimensiona(img: &image::RgbaImage, w: u32, h: u32) -> image::RgbaImage {
    if img.width() == w && img.height() == h {
        return img.clone();
    }
    image::imageops::resize(img, w.max(1), h.max(1), image::imageops::FilterType::Triangle)
}

fn chiave(nome: &str) -> Result<Key, String> {
    let n = nome.trim();
    Ok(match n.to_lowercase().as_str() {
        "return" | "enter" | "invio" => Key::Return,
        "tab" => Key::Tab,
        "space" | "spazio" => Key::Space,
        "backspace" => Key::Backspace,
        "delete" | "canc" => Key::Delete,
        "escape" | "esc" => Key::Escape,
        "up" | "uparrow" => Key::UpArrow,
        "down" | "downarrow" => Key::DownArrow,
        "left" | "leftarrow" => Key::LeftArrow,
        "right" | "rightarrow" => Key::RightArrow,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" | "page_up" | "prior" => Key::PageUp,
        "pagedown" | "page_down" | "next" => Key::PageDown,
        "shift" => Key::Shift,
        "ctrl" | "control" => Key::Control,
        #[cfg(target_os = "macos")]
        "alt" | "option" | "opt" => Key::Option,
        #[cfg(not(target_os = "macos"))]
        "alt" | "option" | "opt" => Key::Alt,
        "super" | "cmd" | "command" | "meta" | "win" => Key::Meta,
        "f1" => Key::F1,
        "f2" => Key::F2,
        "f3" => Key::F3,
        "f4" => Key::F4,
        "f5" => Key::F5,
        "f6" => Key::F6,
        "f7" => Key::F7,
        "f8" => Key::F8,
        "f9" => Key::F9,
        "f10" => Key::F10,
        "f11" => Key::F11,
        "f12" => Key::F12,
        s if s.chars().count() == 1 => Key::Unicode(s.chars().next().unwrap_or(' ')),
        _ => return Err(format!("Tasto sconosciuto: {n}")),
    })
}

fn combo(c: &str) -> Result<Vec<Key>, String> {
    c.split('+').filter(|s| !s.trim().is_empty()).map(chiave).collect()
}

impl Schermo {
    fn a_schermo(&self, (x, y): (f64, f64)) -> Result<(i32, i32), String> {
        let m = self.mappa.lock().unwrap_or_else(|e| e.into_inner()).ok_or("Fai prima uno screenshot, così so dove sono le cose")?;
        if x < 0.0 || y < 0.0 || x > m.img_w || y > m.img_h {
            return Err(format!("Coordinate ({x:.0}, {y:.0}) fuori dallo schermo ({:.0}×{:.0})", m.img_w, m.img_h));
        }
        Ok(((x * m.log_w / m.img_w).round() as i32, (y * m.log_h / m.img_h).round() as i32))
    }

    fn salva(&self, cartella: &Path, bytes: &[u8]) -> Option<PathBuf> {
        let n = self.contatore.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        std::fs::create_dir_all(cartella).ok()?;
        let p = cartella.join(format!("{n:04}.png"));
        std::fs::write(&p, bytes).ok()?;
        Some(p)
    }

    /// Esegue un'azione. `cartella`: dove salvare gli screenshot (None = non salvarli).
    pub async fn esegui(&'static self, a: Azione, cartella: Option<PathBuf>) -> Esito {
        let _turno = self.turno.lock().await;
        let a2 = a.clone();
        let r = tokio::task::spawn_blocking(move || self.esegui_sync(&a2, cartella.as_deref())).await;
        // Un attimo tra un'azione e l'altra: l'interfaccia ha il tempo di reagire
        tokio::time::sleep(Duration::from_millis(120)).await;
        r.unwrap_or_else(|e| Err(e.to_string())).unwrap_or_else(Esito::errore)
    }

    fn esegui_sync(&self, a: &Azione, cartella: Option<&Path>) -> Result<Esito, String> {
        let osserva = matches!(a, Azione::Screenshot | Azione::Zoom(_) | Azione::Posizione | Azione::Attendi(_));
        let mut enigo = Enigo::new(&Settings::default()).map_err(|e| {
            format!("Controllo del mouse non disponibile ({e}). Su macOS concedi a Oltremare l'Accessibilità in Impostazioni di Sistema → Privacy e sicurezza")
        })?;
        if !osserva {
            if let Some(app) = app_in_primo_piano() {
                let l = app.to_lowercase();
                if VIETATE.iter().any(|v| l.contains(v)) {
                    return Err(format!("Per sicurezza non agisco su «{app}», che custodisce password. Chiedi all'utente di farlo lui."));
                }
            }
            // Se il mouse non è dove l'avevamo lasciato, l'utente sta usando il computer
            let atteso = *self.atteso.lock().unwrap_or_else(|e| e.into_inner());
            if let (Some((ax, ay)), Ok((x, y))) = (atteso, enigo.location()) {
                if (ax - x).abs() + (ay - y).abs() > 40 {
                    *self.atteso.lock().unwrap_or_else(|e| e.into_inner()) = None;
                    return Err("L'utente ha mosso il mouse: mi fermo per non intralciarlo. Chiedigli se posso riprendere.".into());
                }
            }
        }
        let premi_mod = |enigo: &mut Enigo, tasti: &[String], dir: Direction| -> Result<(), String> {
            for t in tasti {
                enigo.key(chiave(t)?, dir).map_err(|e| e.to_string())?;
            }
            Ok(())
        };
        let esito = match a {
            Azione::Screenshot => {
                let (img, m) = cattura()?;
                let piccola = ridimensiona(&img, m.img_w as u32, m.img_h as u32);
                *self.mappa.lock().unwrap_or_else(|e| e.into_inner()) = Some(m);
                let (b64, bytes) = png_base64(&piccola)?;
                let path = cartella.and_then(|c| self.salva(c, &bytes));
                return Ok(Esito {
                    testo: format!("Screenshot {}×{} dello schermo principale.", m.img_w, m.img_h),
                    is_error: false,
                    risultato: json!({ "kind": "computer", "action": "screenshot", "screenshotPath": path.map(|p| p.to_string_lossy().into_owned()) }),
                    immagine: Some(b64),
                });
            }
            Azione::Zoom([x0, y0, x1, y1]) => {
                let (img, m) = cattura()?;
                *self.mappa.lock().unwrap_or_else(|e| e.into_inner()) = Some(m);
                let k = m.fis_w / m.img_w;
                let (ax, ay) = ((x0.min(*x1) * k).max(0.0) as u32, (y0.min(*y1) * k).max(0.0) as u32);
                let (bx, by) = ((x0.max(*x1) * k).min(m.fis_w) as u32, (y0.max(*y1) * k).min(m.fis_h) as u32);
                if bx <= ax + 4 || by <= ay + 4 {
                    return Err("Regione troppo piccola".into());
                }
                let parte = image::imageops::crop_imm(&img, ax, ay, bx - ax, by - ay).to_image();
                let s = (LATO as f64 / parte.width().max(parte.height()) as f64).min(3.0);
                let z = ridimensiona(&parte, (parte.width() as f64 * s) as u32, (parte.height() as f64 * s) as u32);
                let (b64, bytes) = png_base64(&z)?;
                let path = cartella.and_then(|c| self.salva(c, &bytes));
                return Ok(Esito {
                    testo: "Ingrandimento della regione richiesta (le coordinate restano quelle dello screenshot intero).".into(),
                    is_error: false,
                    risultato: json!({ "kind": "computer", "action": "zoom", "screenshotPath": path.map(|p| p.to_string_lossy().into_owned()) }),
                    immagine: Some(b64),
                });
            }
            Azione::Posizione => {
                let (x, y) = enigo.location().map_err(|e| e.to_string())?;
                let m = self.mappa.lock().unwrap_or_else(|e| e.into_inner()).ok_or("Fai prima uno screenshot")?;
                let (ix, iy) = ((x as f64 * m.img_w / m.log_w).round(), (y as f64 * m.img_h / m.log_h).round());
                format!("X={ix}, Y={iy}")
            }
            Azione::Clic { pulsante, volte, dove, tasti } => {
                if let Some(d) = dove {
                    let (x, y) = self.a_schermo(*d)?;
                    enigo.move_mouse(x, y, Coordinate::Abs).map_err(|e| e.to_string())?;
                    std::thread::sleep(Duration::from_millis(40));
                }
                premi_mod(&mut enigo, tasti, Direction::Press)?;
                for _ in 0..*volte {
                    enigo.button(*pulsante, Direction::Click).map_err(|e| e.to_string())?;
                    std::thread::sleep(Duration::from_millis(45));
                }
                premi_mod(&mut enigo, tasti, Direction::Release)?;
                "OK".into()
            }
            Azione::Premi(giu) => {
                enigo.button(Button::Left, if *giu { Direction::Press } else { Direction::Release }).map_err(|e| e.to_string())?;
                "OK".into()
            }
            Azione::Sposta(d) => {
                let (x, y) = self.a_schermo(*d)?;
                enigo.move_mouse(x, y, Coordinate::Abs).map_err(|e| e.to_string())?;
                "OK".into()
            }
            Azione::Trascina { da, a, tasti } => {
                let (x0, y0) = self.a_schermo(*da)?;
                let (x1, y1) = self.a_schermo(*a)?;
                premi_mod(&mut enigo, tasti, Direction::Press)?;
                enigo.move_mouse(x0, y0, Coordinate::Abs).map_err(|e| e.to_string())?;
                enigo.button(Button::Left, Direction::Press).map_err(|e| e.to_string())?;
                for k in 1..=12 {
                    let t = k as f64 / 12.0;
                    enigo.move_mouse((x0 as f64 + (x1 - x0) as f64 * t) as i32, (y0 as f64 + (y1 - y0) as f64 * t) as i32, Coordinate::Abs).map_err(|e| e.to_string())?;
                    std::thread::sleep(Duration::from_millis(15));
                }
                enigo.button(Button::Left, Direction::Release).map_err(|e| e.to_string())?;
                premi_mod(&mut enigo, tasti, Direction::Release)?;
                "OK".into()
            }
            Azione::Scorri { direzione, quanto, dove, tasti } => {
                if let Some(d) = dove {
                    let (x, y) = self.a_schermo(*d)?;
                    enigo.move_mouse(x, y, Coordinate::Abs).map_err(|e| e.to_string())?;
                }
                premi_mod(&mut enigo, tasti, Direction::Press)?;
                let q = (*quanto).clamp(1, 30);
                let (len, asse) = match direzione.as_str() {
                    "up" => (-q, Axis::Vertical),
                    "left" => (-q, Axis::Horizontal),
                    "right" => (q, Axis::Horizontal),
                    _ => (q, Axis::Vertical),
                };
                enigo.scroll(len, asse).map_err(|e| e.to_string())?;
                premi_mod(&mut enigo, tasti, Direction::Release)?;
                "OK".into()
            }
            Azione::Scrivi(t) => {
                enigo.text(t).map_err(|e| e.to_string())?;
                "OK".into()
            }
            Azione::Tasti { combo: c, ripeti } => {
                let k = combo(c)?;
                for _ in 0..*ripeti {
                    for t in &k {
                        enigo.key(*t, Direction::Press).map_err(|e| e.to_string())?;
                    }
                    for t in k.iter().rev() {
                        enigo.key(*t, Direction::Release).map_err(|e| e.to_string())?;
                    }
                    std::thread::sleep(Duration::from_millis(30));
                }
                "OK".into()
            }
            Azione::Tieni { combo: c, secondi } => {
                let k = combo(c)?;
                for t in &k {
                    enigo.key(*t, Direction::Press).map_err(|e| e.to_string())?;
                }
                std::thread::sleep(Duration::from_secs_f64(*secondi));
                for t in k.iter().rev() {
                    enigo.key(*t, Direction::Release).map_err(|e| e.to_string())?;
                }
                "OK".into()
            }
            Azione::Attendi(s) => {
                std::thread::sleep(Duration::from_secs_f64(*s));
                "OK".into()
            }
        };
        if !osserva {
            *self.atteso.lock().unwrap_or_else(|e| e.into_inner()) = enigo.location().ok();
        }
        Ok(Esito::ok(esito.clone(), json!({ "kind": "computer", "action": format!("{a:?}").split([' ', '(', '{']).next().unwrap_or("").to_lowercase(), "text": esito })))
    }
}

/// Rilascia tutto (interruttore di emergenza).
pub fn rilascia_tutto() {
    if let Ok(mut e) = Enigo::new(&Settings::default()) {
        let _ = e.button(Button::Left, Direction::Release);
        for k in [Key::Shift, Key::Control, Key::Meta] {
            let _ = e.key(k, Direction::Release);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cattura vera dello schermo (serve il permesso Registrazione schermo): `cargo test cattura_vera -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn cattura_vera() {
        let (img, m) = cattura().expect("cattura");
        println!("fisico {}×{} → screenshot {}×{} · punti {}×{}", img.width(), img.height(), m.img_w, m.img_h, m.log_w, m.log_h);
        assert!(m.img_w.max(m.img_h) <= LATO as f64);
        let piccola = ridimensiona(&img, m.img_w as u32, m.img_h as u32);
        let (b64, bytes) = png_base64(&piccola).unwrap();
        println!("PNG {} KB", bytes.len() / 1024);
        assert!(b64.len() > 1000);
        // Lo schermo non deve essere tutto nero (succede se manca il permesso)
        let chiari = piccola.pixels().filter(|p| p.0[0] as u32 + p.0[1] as u32 + p.0[2] as u32 > 60).count();
        println!("pixel non neri: {chiari}");
    }

    #[test]
    fn toolset_actions_are_parsed() {
        assert_eq!(da_toolset("left_click", &json!({"coordinate": [500, 300], "text": "shift"})).unwrap(), Azione::Clic { pulsante: Button::Left, volte: 1, dove: Some((500.0, 300.0)), tasti: vec!["shift".into()] });
        assert!(matches!(da_toolset("scroll", &json!({"coordinate": [5, 4], "scroll_direction": "up", "scroll_amount": 2})).unwrap(), Azione::Scorri { quanto: 2, .. }));
        assert_eq!(da_toolset("key", &json!({"text": "Tab", "repeat": 4})).unwrap(), Azione::Tasti { combo: "Tab".into(), ripeti: 4 });
        assert_eq!(da_toolset("zoom", &json!({"region": [100, 200, 400, 350]})).unwrap(), Azione::Zoom([100.0, 200.0, 400.0, 350.0]));
        assert!(da_toolset("mouse_move", &json!({})).is_err());
        assert_eq!(spec_di(&da_toolset("triple_click", &json!({})).unwrap()), "schermo_clic");
        assert_eq!(spec_di(&da_funzione("schermo_scrivi", &json!({"testo": "ciao"})).unwrap()), "schermo_scrivi");
    }

    #[test]
    fn coordinates_map_to_logical_points() {
        let s = Schermo::default();
        // Retina: 2880×1800 fisici, 1440×900 punti, screenshot 1280×800
        *s.mappa.lock().unwrap() = Some(Mappa { img_w: 1280.0, img_h: 800.0, fis_w: 2880.0, fis_h: 1800.0, log_w: 1440.0, log_h: 900.0 });
        assert_eq!(s.a_schermo((640.0, 400.0)).unwrap(), (720, 450));
        assert_eq!(s.a_schermo((1280.0, 800.0)).unwrap(), (1440, 900));
        assert!(s.a_schermo((1300.0, 10.0)).is_err());
        assert_eq!(combo("ctrl+shift+s").unwrap().len(), 3);
        assert!(combo("ctrl+nonesiste").is_err());
    }
}

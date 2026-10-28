//! Centro Notifiche di macOS: si legge il database di `usernoted` (serve l'Accesso completo
//! al disco). Si prendono solo le notifiche nuove rispetto all'ultima lettura.

use super::Notifica;
use crate::App;
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

/// Secondi tra il 1970 e il 2001 (l'epoca di Apple)
const EPOCA_APPLE: f64 = 978_307_200.0;

#[cfg(target_os = "macos")]
fn percorso() -> Option<std::path::PathBuf> {
    crate::shell_env::home().map(|h| h.join("Library/Group Containers/group.com.apple.usernoted/db2/db"))
}

/// Nome leggibile dell'app dal bundle id (con una cache).
#[cfg(target_os = "macos")]
fn nome_app(bundle: &str) -> String {
    static CACHE: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);
    let noti = [
        ("net.whatsapp.WhatsApp", "WhatsApp"),
        ("com.tinyspeck.slackmacgap", "Slack"),
        ("com.microsoft.teams2", "Teams"),
        ("com.microsoft.teams", "Teams"),
        ("com.apple.MobileSMS", "Messaggi"),
        ("com.apple.mail", "Mail"),
        ("com.apple.iCal", "Calendario"),
        ("com.apple.reminders", "Promemoria"),
        ("com.microsoft.Outlook", "Outlook"),
        ("ru.keepcoder.Telegram", "Telegram"),
        ("org.telegram.desktop", "Telegram"),
        ("com.hnc.Discord", "Discord"),
        ("com.apple.FaceTime", "FaceTime"),
    ];
    if let Some((_, n)) = noti.iter().find(|(b, _)| b.eq_ignore_ascii_case(bundle)) {
        return n.to_string();
    }
    let mut g = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let c = g.get_or_insert_with(HashMap::new);
    if let Some(n) = c.get(bundle) {
        return n.clone();
    }
    let n = std::process::Command::new("mdfind")
        .arg(format!("kMDItemCFBundleIdentifier == '{bundle}'"))
        .output()
        .ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).lines().next().map(str::to_string))
        .and_then(|p| std::path::Path::new(&p).file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_else(|| bundle.rsplit('.').next().unwrap_or(bundle).to_string());
    c.insert(bundle.to_string(), n.clone());
    n
}

#[cfg(target_os = "macos")]
fn campo<'a>(d: &'a plist::Dictionary, k: &str) -> Option<&'a str> {
    d.get(k).and_then(|v| v.as_string())
}

#[cfg(target_os = "macos")]
pub fn nuove(app: &AppHandle) -> Vec<Notifica> {
    let st = app.state::<App>();
    let Some(p) = percorso() else { return vec![] };
    let Ok(conn) = rusqlite::Connection::open_with_flags(&p, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX) else {
        return vec![];
    };
    let ultimo: i64 = st.db.stato_fonte("macos:rec_id").and_then(|v| v.parse().ok()).unwrap_or(-1);
    // Primo avvio: si parte dalle ultime 24 ore, non da tutta la storia
    let soglia = if ultimo < 0 { (chrono::Utc::now().timestamp() as f64 - EPOCA_APPLE) - 86_400.0 } else { 0.0 };
    let Ok(mut q) = conn.prepare(
        "SELECT r.rec_id, a.identifier, r.data, COALESCE(r.delivered_date, 0)
         FROM record r JOIN app a ON r.app_id = a.app_id
         WHERE r.rec_id > ?1 AND COALESCE(r.delivered_date, 0) >= ?2 ORDER BY r.rec_id LIMIT 300",
    ) else {
        return vec![];
    };
    let righe: Vec<(i64, String, Vec<u8>, f64)> = q
        .query_map(rusqlite::params![ultimo, soglia], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .map(|it| it.flatten().collect())
        .unwrap_or_default();
    let mut massimo = ultimo;
    let mut out = vec![];
    for (rec, bundle, data, consegna) in righe {
        massimo = massimo.max(rec);
        let Ok(v) = plist::Value::from_reader(std::io::Cursor::new(&data)) else { continue };
        let Some(d) = v.as_dictionary() else { continue };
        let req = d.get("req").and_then(|r| r.as_dictionary()).unwrap_or(d);
        let titolo = campo(req, "titl").unwrap_or_default();
        let sotto = campo(req, "subt").unwrap_or_default();
        let corpo = campo(req, "body").unwrap_or_default();
        if titolo.is_empty() && corpo.is_empty() {
            continue;
        }
        let at = ((consegna + EPOCA_APPLE) * 1000.0) as i64;
        let titolo = if sotto.is_empty() { titolo.to_string() } else { format!("{titolo} · {sotto}") };
        out.push(Notifica::nuova("system", &nome_app(&bundle), &bundle, &titolo, corpo, at, &rec.to_string()));
    }
    if massimo > ultimo {
        st.db.salva_stato_fonte("macos:rec_id", &massimo.to_string());
    }
    out
}

#[cfg(not(target_os = "macos"))]
pub fn nuove(_app: &AppHandle) -> Vec<Notifica> {
    // Su Windows le notifiche di sistema richiedono un'app impacchettata (MSIX): non ancora disponibili
    vec![]
}

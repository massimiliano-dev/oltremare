//! Nuove email da leggere negli account collegati, come notifiche.

use super::Notifica;
use crate::App;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

pub async fn nuove(app: &AppHandle) -> Vec<Notifica> {
    let st = app.state::<App>();
    let s = st.store.settings();
    let mut out = vec![];
    for acc in st.store.accounts() {
        let chiave = format!("posta:{}", acc.id);
        let ultimo = st.db.stato_fonte(&chiave);
        let Ok(token) = crate::oauth::token(&acc, "", &s).await else { continue };
        let ctx = crate::tools::posta::Ctx { acc: &acc, token };
        let e = crate::tools::posta::esegui("mail_cerca", &json!({ "non_lette": true, "max": 10 }), &ctx).await;
        if e.is_error {
            continue;
        }
        // Gli id già visti stanno nello stato della fonte (gli ultimi 200)
        let visti: Vec<String> = ultimo.as_deref().and_then(|u| serde_json::from_str(u).ok()).unwrap_or_default();
        let mut nuovi_visti = visti.clone();
        for blocco in e.testo.split("\n• ").skip(1) {
            let id = blocco.rsplit("id: ").next().and_then(|r| r.lines().next()).unwrap_or_default().trim().to_string();
            if id.is_empty() || visti.contains(&id) {
                continue;
            }
            nuovi_visti.push(id.clone());
            if ultimo.is_none() {
                continue; // primo giro: si memorizza soltanto
            }
            let oggetto = blocco.lines().next().unwrap_or_default().trim_start_matches("[da leggere] ").to_string();
            let da = blocco.lines().nth(1).and_then(|l| l.trim().strip_prefix("da ")).and_then(|l| l.split(" · ").next()).unwrap_or("").to_string();
            let anteprima = blocco.lines().nth(2).unwrap_or_default().trim().to_string();
            out.push(Notifica::nuova("mail", &format!("Posta · {}", acc.email), "mail", &format!("{da}: {oggetto}"), &anteprima, crate::store::now_ms(), &id));
        }
        let tieni = nuovi_visti.len().saturating_sub(200);
        st.db.salva_stato_fonte(&chiave, &Value::from(nuovi_visti[tieni..].to_vec()).to_string());
    }
    out
}

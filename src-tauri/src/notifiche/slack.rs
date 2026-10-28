//! Slack: messaggi diretti nuovi, con un token utente incollato dall'utente (portachiavi).

use super::Notifica;
use crate::App;
use serde_json::Value;
use tauri::{AppHandle, Manager};

async fn get(token: &str, metodo: &str, query: &str) -> Option<Value> {
    let v: Value = crate::providers::client().get(format!("https://slack.com/api/{metodo}?{query}")).bearer_auth(token).send().await.ok()?.json().await.ok()?;
    v["ok"].as_bool().unwrap_or(false).then_some(v)
}

/// Verifica il token: restituisce nome utente e team.
pub async fn verifica(token: &str) -> Result<String, String> {
    let v = get(token, "auth.test", "").await.ok_or("Token Slack non valido")?;
    Ok(format!("{} · {}", v["user"].as_str().unwrap_or(""), v["team"].as_str().unwrap_or("")))
}

pub async fn nuove(app: &AppHandle) -> Vec<Notifica> {
    let st = app.state::<App>();
    let Ok(token) = crate::store::get_secret("slack:token") else { return vec![] };
    let Some(me) = get(&token, "auth.test", "").await else { return vec![] };
    let mio_id = me["user_id"].as_str().unwrap_or_default().to_string();
    let Some(conv) = get(&token, "conversations.list", "types=im,mpim&limit=50").await else { return vec![] };
    let ultimo: f64 = st.db.stato_fonte("slack:ts").and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let inizio = if ultimo == 0.0 { chrono::Utc::now().timestamp() as f64 } else { ultimo };
    let mut massimo = inizio;
    let mut out = vec![];
    let mut nomi: std::collections::HashMap<String, String> = Default::default();
    for c in conv["channels"].as_array().into_iter().flatten().take(30) {
        let id = c["id"].as_str().unwrap_or_default();
        let Some(h) = get(&token, "conversations.history", &format!("channel={id}&oldest={inizio}&limit=20")).await else { continue };
        for m in h["messages"].as_array().into_iter().flatten() {
            let ts: f64 = m["ts"].as_str().and_then(|t| t.parse().ok()).unwrap_or(0.0);
            massimo = massimo.max(ts);
            let utente = m["user"].as_str().unwrap_or_default().to_string();
            if utente == mio_id || ultimo == 0.0 {
                continue;
            }
            if !nomi.contains_key(&utente) {
                let n = get(&token, "users.info", &format!("user={utente}")).await.and_then(|u| u["user"]["real_name"].as_str().map(str::to_string)).unwrap_or_else(|| "Qualcuno".into());
                nomi.insert(utente.clone(), n);
            }
            out.push(Notifica::nuova("slack", "Slack", "slack", &nomi[&utente], m["text"].as_str().unwrap_or_default(), (ts * 1000.0) as i64, &format!("{id}:{ts}")));
        }
    }
    st.db.salva_stato_fonte("slack:ts", &massimo.to_string());
    out
}

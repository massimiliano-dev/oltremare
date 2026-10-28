//! Teams: messaggi nuovi nelle chat (Microsoft Graph, permesso Chat.Read).

use super::Notifica;
use crate::App;
use serde_json::Value;
use tauri::{AppHandle, Manager};

pub async fn nuove(app: &AppHandle) -> Vec<Notifica> {
    let st = app.state::<App>();
    let s = st.store.settings();
    let mut out = vec![];
    for acc in st.store.accounts().into_iter().filter(|a| a.provider == "microsoft" && a.teams) {
        let Ok(token) = crate::oauth::token(&acc, "teams", &s).await else { continue };
        let url = "https://graph.microsoft.com/v1.0/me/chats?$expand=lastMessagePreview&$top=30&$orderby=lastMessagePreview/createdDateTime%20desc";
        let Ok(r) = crate::providers::client().get(url).bearer_auth(&token).send().await else { continue };
        let Ok(v) = r.json::<Value>().await else { continue };
        let chiave = format!("teams:{}", acc.id);
        let ultimo = st.db.stato_fonte(&chiave).unwrap_or_default();
        let mut piu_recente = ultimo.clone();
        for chat in v["value"].as_array().into_iter().flatten() {
            let m = &chat["lastMessagePreview"];
            let quando = m["createdDateTime"].as_str().unwrap_or_default().to_string();
            if quando.is_empty() || quando <= ultimo {
                continue;
            }
            piu_recente = piu_recente.max(quando.clone());
            if ultimo.is_empty() {
                continue; // primo giro: si memorizza solo il punto di partenza
            }
            let da = m["from"]["user"]["displayName"].as_str().unwrap_or("Qualcuno");
            if m["from"]["user"]["userIdentityType"] == "aadUser" && da.eq_ignore_ascii_case(&acc.name) {
                continue; // messaggi miei
            }
            let testo = m["body"]["content"].as_str().unwrap_or_default();
            let testo = regex::Regex::new(r"<[^>]+>").map(|re| re.replace_all(testo, "").to_string()).unwrap_or_default();
            let titolo = match chat["topic"].as_str() {
                Some(t) if !t.is_empty() => format!("{da} in «{t}»"),
                _ => da.to_string(),
            };
            let at = chrono::DateTime::parse_from_rfc3339(&quando).map(|d| d.timestamp_millis()).unwrap_or_else(|_| crate::store::now_ms());
            out.push(Notifica::nuova("teams", "Teams", "teams", &titolo, &testo, at, m["id"].as_str().unwrap_or(&quando)));
        }
        if piu_recente != ultimo {
            st.db.salva_stato_fonte(&chiave, &piu_recente);
        }
    }
    out
}

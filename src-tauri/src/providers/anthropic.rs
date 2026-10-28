//! Claude: Messages API in streaming.
//!
//! Regole dell'API seguite qui: versione `2026-10-21` (niente temperature/top_p/top_k),
//! ragionamento adattivo con riassunto visibile, `effort` esplicito, cache sul prompt di sistema
//! e sulla coda della conversazione, fallback lato server sui rifiuti, contenuto
//! dell'assistente rimandato identico nei turni successivi.

use super::sse::{dati, Righe};
use super::{api_error, client, prossimo, Delta, Richiesta, Risposta, Stop, Usage};
use crate::agent::messaggi::{Blocco, Messaggio, Ruolo};
use crate::allegati::{self, Contenuto};
use crate::error::{AppError, Result};
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

const URL: &str = "https://api.anthropic.com/v1";
const VERSION: &str = "2026-10-21";
const BETA_FALLBACK: &str = "server-side-fallback-2026-07-01";
const BETA_CONTEXT: &str = "context-management-2025-06-27";

/// Modelli che accettano ragionamento adattivo ed `effort` (Claude 4.6 e successivi).
fn adattivo(model: &str) -> bool {
    !["claude-3", "-4-5", "-4-1", "-4-0", "-4-2025", "claude-opus-4-2", "claude-sonnet-4-2"]
        .iter()
        .any(|x| model.contains(x))
}

/// Modelli su cui è disponibile il fallback lato server con `fallbacks: "default"`.
fn fallback(model: &str) -> bool {
    ["claude-fable-5-1", "claude-opus-5-5", "claude-opus-5", "claude-sonnet-5-5"]
        .iter()
        .any(|m| model == *m || model.starts_with(&format!("{m}-")))
}

fn messaggio(m: &Messaggio) -> Value {
    match m.ruolo {
        Ruolo::Assistente => {
            if let (Some(raw), Some("anthropic")) = (&m.raw, m.raw_provider.as_deref()) {
                return json!({ "role": "assistant", "content": raw });
            }
            // Turno prodotto da un altro provider: si ricostruisce dal modello canonico
            let content: Vec<Value> = m
                .blocchi
                .iter()
                .filter_map(|b| match b {
                    Blocco::Testo { testo } if !testo.is_empty() => Some(json!({ "type": "text", "text": testo })),
                    Blocco::ToolUse { id, nome, input, .. } => Some(json!({ "type": "tool_use", "id": id, "name": nome, "input": input })),
                    _ => None,
                })
                .collect();
            json!({ "role": "assistant", "content": content })
        }
        Ruolo::Utente => {
            // I documenti vanno prima del testo che li riguarda
            let mut blocchi: Vec<&Blocco> = m.blocchi.iter().filter(|b| matches!(b, Blocco::Allegato { .. })).collect();
            blocchi.extend(m.blocchi.iter().filter(|b| !matches!(b, Blocco::Allegato { .. })));
            let content: Vec<Value> = blocchi
                .into_iter()
                .filter_map(|b| match b {
                    Blocco::Testo { testo } => Some(json!({ "type": "text", "text": testo })),
                    Blocco::Allegato { nome, mime, formato: tipo, percorso, .. } => Some(match allegati::carica(tipo, mime, percorso) {
                        Contenuto::Pdf(data) => json!({
                            "type": "document", "title": nome,
                            "source": { "type": "base64", "media_type": "application/pdf", "data": data }
                        }),
                        Contenuto::Immagine { mime, base64 } => json!({
                            "type": "image", "source": { "type": "base64", "media_type": mime, "data": base64 }
                        }),
                        Contenuto::Testo(t) => json!({
                            "type": "document", "title": nome,
                            "source": { "type": "text", "media_type": "text/plain", "data": t }
                        }),
                        Contenuto::Mancante => json!({ "type": "text", "text": format!("[allegato non più disponibile: {nome}]") }),
                    }),
                    Blocco::ToolResult { tool_use_id, contenuto, is_error, toolset, immagine } => {
                        let img = immagine.as_deref().and_then(crate::agent::messaggi::immagine_base64);
                        let content = match img {
                            Some(b64) => json!([
                                { "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": b64 } },
                                { "type": "text", "text": contenuto }
                            ]),
                            None => json!(contenuto),
                        };
                        let mut r = json!({ "type": "tool_result", "tool_use_id": tool_use_id, "content": content, "is_error": is_error });
                        if let Some(t) = toolset {
                            r["toolset_name"] = json!(t);
                        }
                        Some(r)
                    }
                    _ => None,
                })
                .collect();
            json!({ "role": "user", "content": content })
        }
    }
}

pub fn corpo(req: &Richiesta<'_>) -> Value {
    let mut body = json!({
        "model": req.modello,
        "max_tokens": req.max_tokens,
        "stream": true,
        "system": [{ "type": "text", "text": req.system, "cache_control": { "type": "ephemeral" } }],
        // Cache automatica sulla coda della conversazione
        "cache_control": { "type": "ephemeral" },
        "messages": req.messaggi.iter().map(messaggio).collect::<Vec<_>>(),
    });
    if adattivo(req.modello) {
        body["thinking"] = json!({ "type": "adaptive", "display": "summarized" });
        if !req.effort.is_empty() {
            body["output_config"] = json!({ "effort": req.effort });
        }
    }
    if fallback(req.modello) {
        body["fallbacks"] = json!("default");
    }
    if !req.tools.is_empty() {
        body["tools"] = json!(req.tools);
        // Gli screenshot vecchi si tolgono lato server: la conversazione resta leggera
        if usa_schermo(req) {
            body["context_management"] = json!({ "edits": [{ "type": "clear_tool_uses_20250919" }] });
        }
    }
    body
}

fn usa_schermo(req: &Richiesta<'_>) -> bool {
    req.tools.iter().any(|t| t["type"] == "computer_toolset_20260801")
}

pub async fn stream(req: Richiesta<'_>, on: &mut (dyn FnMut(Delta) + Send), cancel: &CancellationToken) -> Result<Risposta> {
    let body = corpo(&req);
    let mut r = client()
        .post(format!("{URL}/messages"))
        .header("x-api-key", req.chiave)
        .header("anthropic-version", VERSION)
        .json(&body);
    let mut beta: Vec<&str> = vec![];
    if fallback(req.modello) {
        beta.push(BETA_FALLBACK);
    }
    if usa_schermo(&req) {
        beta.push(BETA_CONTEXT);
    }
    if !beta.is_empty() {
        r = r.header("anthropic-beta", beta.join(","));
    }
    let resp = tokio::select! {
        _ = cancel.cancelled() => return Err(AppError::Annullato),
        r = r.send() => r?,
    };
    if !resp.status().is_success() {
        return Err(api_error(resp).await);
    }

    let mut stato = Stato::nuovo(req.modello);
    let mut righe = Righe::default();
    let mut s = resp.bytes_stream();
    while let Some(bytes) = prossimo(&mut s, cancel).await? {
        for line in righe.push(&bytes) {
            let Some(data) = dati(&line) else { continue };
            let Ok(ev) = serde_json::from_str::<Value>(data) else { continue };
            if stato.evento(&ev, on)? {
                return Ok(stato.fine());
            }
        }
    }
    Ok(stato.fine())
}

/// Ricostruisce un messaggio dagli eventi dello stream (usato anche per Claude Code).
#[derive(Default)]
pub(super) struct Stato {
    blocchi: Vec<Value>,
    json_parziale: Vec<String>,
    stop: Option<String>,
    stop_details: Option<Value>,
    usage: Usage,
    modello: String,
}

impl Stato {
    pub(super) fn nuovo(modello: &str) -> Self {
        Self { modello: modello.to_string(), ..Self::default() }
    }

    /// Applica un evento dello stream. `true` quando il messaggio è finito.
    pub(super) fn evento(&mut self, ev: &Value, on: &mut (dyn FnMut(Delta) + Send)) -> Result<bool> {
        match ev["type"].as_str().unwrap_or_default() {
            "message_start" => {
                let m = &ev["message"];
                if let Some(model) = m["model"].as_str() {
                    self.modello = model.to_string();
                }
                let u = &m["usage"];
                self.usage.input_tokens = u["input_tokens"].as_u64().unwrap_or(0);
                self.usage.cache_read = u["cache_read_input_tokens"].as_u64().unwrap_or(0);
                self.usage.cache_write = u["cache_creation_input_tokens"].as_u64().unwrap_or(0);
                self.usage.output_tokens = u["output_tokens"].as_u64().unwrap_or(0);
            }
            "content_block_start" => {
                let i = ev["index"].as_u64().unwrap_or(self.blocchi.len() as u64) as usize;
                while self.blocchi.len() <= i {
                    self.blocchi.push(Value::Null);
                    self.json_parziale.push(String::new());
                }
                // Qualsiasi blocco (anche sconosciuto, es. "fallback") si conserva così com'è
                self.blocchi[i] = ev["content_block"].clone();
            }
            "content_block_delta" => {
                let i = ev["index"].as_u64().unwrap_or(0) as usize;
                let Some(b) = self.blocchi.get_mut(i) else { return Ok(false) };
                let d = &ev["delta"];
                match d["type"].as_str().unwrap_or_default() {
                    "text_delta" => {
                        let t = d["text"].as_str().unwrap_or_default();
                        append(b, "text", t);
                        on(Delta::Testo(t.to_string()));
                    }
                    "thinking_delta" => {
                        let t = d["thinking"].as_str().unwrap_or_default();
                        append(b, "thinking", t);
                        on(Delta::Pensiero(t.to_string()));
                    }
                    "signature_delta" => append(b, "signature", d["signature"].as_str().unwrap_or_default()),
                    "input_json_delta" => self.json_parziale[i].push_str(d["partial_json"].as_str().unwrap_or_default()),
                    _ => {}
                }
            }
            "content_block_stop" => {
                let i = ev["index"].as_u64().unwrap_or(0) as usize;
                if let Some(b) = self.blocchi.get_mut(i) {
                    if b["type"] == "tool_use" || b["type"] == "server_tool_use" {
                        let raw = self.json_parziale[i].trim();
                        if !raw.is_empty() {
                            // Input non valido: resta `{}` e il ciclo lo segnalerà come errore dello strumento
                            b["input"] = serde_json::from_str(raw).unwrap_or_else(|_| json!({ "__json_non_valido": raw }));
                        }
                    }
                }
            }
            "message_delta" => {
                if let Some(r) = ev["delta"]["stop_reason"].as_str() {
                    self.stop = Some(r.to_string());
                }
                if !ev["delta"]["stop_details"].is_null() {
                    self.stop_details = Some(ev["delta"]["stop_details"].clone());
                }
                if let Some(o) = ev["usage"]["output_tokens"].as_u64() {
                    self.usage.output_tokens = o;
                }
            }
            "message_stop" => return Ok(true),
            "error" => {
                let msg = ev["error"]["message"].as_str().unwrap_or("errore del modello");
                return Err(AppError::msg(msg.to_string()));
            }
            _ => {}
        }
        Ok(false)
    }

    pub(super) fn fine(self) -> Risposta {
        let raw: Vec<Value> = self.blocchi.into_iter().filter(|b| !b.is_null()).collect();
        let blocchi = raw
            .iter()
            .filter_map(|b| match b["type"].as_str()? {
                "text" => Some(Blocco::Testo { testo: b["text"].as_str().unwrap_or_default().to_string() }),
                "thinking" => Some(Blocco::Pensiero { testo: b["thinking"].as_str().unwrap_or_default().to_string() }),
                "tool_use" => Some(Blocco::ToolUse {
                    id: b["id"].as_str().unwrap_or_default().to_string(),
                    nome: b["name"].as_str().unwrap_or_default().to_string(),
                    input: b["input"].clone(),
                    toolset: b["toolset_name"].as_str().map(str::to_string),
                }),
                _ => None,
            })
            .collect();
        let stop = match self.stop.as_deref() {
            Some("end_turn") | Some("stop_sequence") | None => Stop::Fine,
            Some("tool_use") => Stop::ToolUse,
            Some("max_tokens") => Stop::MaxTokens,
            Some("refusal") => Stop::Rifiuto,
            Some("pause_turn") => Stop::Pausa,
            Some(o) => Stop::Altro(o.to_string()),
        };
        Risposta { blocchi, raw: Value::Array(raw), stop, stop_details: self.stop_details, usage: self.usage, modello: self.modello }
    }
}

fn append(b: &mut Value, field: &str, t: &str) {
    let cur = b[field].as_str().unwrap_or_default().to_string();
    b[field] = Value::String(cur + t);
}

pub async fn models(key: &str) -> Result<Vec<String>> {
    let resp = client()
        .get(format!("{URL}/models?limit=100"))
        .header("x-api-key", key)
        .header("anthropic-version", VERSION)
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(api_error(resp).await);
    }
    let body: Value = resp.json().await?;
    Ok(body["data"]
        .as_array()
        .map(|a| a.iter().filter_map(|m| m["id"].as_str().map(str::to_string)).collect())
        .unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req<'a>(model: &'a str, msgs: &'a [Messaggio]) -> Richiesta<'a> {
        Richiesta { system: "sys", messaggi: msgs, modello: model, effort: "medium", max_tokens: 1000, chiave: "", base_url: "", tools: &[] }
    }

    #[test]
    fn request_follows_current_api_rules() {
        let msgs = vec![Messaggio::utente("ciao")];
        let b = corpo(&req("claude-opus-5-5", &msgs));
        assert!(b.get("temperature").is_none() && b.get("top_p").is_none() && b.get("tool_choice").is_none());
        assert_eq!(b["thinking"]["type"], "adaptive");
        assert_eq!(b["output_config"]["effort"], "medium");
        assert_eq!(b["fallbacks"], "default");
        assert_eq!(b["system"][0]["cache_control"]["type"], "ephemeral");
        // Modelli vecchi: niente parametri che non conoscono
        let old = corpo(&req("claude-haiku-4-5", &msgs));
        assert!(old.get("thinking").is_none() && old.get("fallbacks").is_none());
    }

    #[test]
    fn assistant_content_is_replayed_verbatim() {
        let raw = json!([{"type":"thinking","thinking":"riassunto","signature":"sig"},{"type":"fallback","from":{"model":"x"}},{"type":"text","text":"Ciao"}]);
        let msgs = vec![
            Messaggio::utente("ciao"),
            Messaggio { ruolo: Ruolo::Assistente, blocchi: vec![], raw: Some(raw.clone()), raw_provider: Some("anthropic".into()) },
        ];
        let b = corpo(&req("claude-opus-5-5", &msgs));
        assert_eq!(b["messages"][1]["content"], raw);
    }

    #[test]
    fn stream_events_rebuild_the_message() {
        let mut st = Stato::default();
        let mut text = String::new();
        let mut on = |d: Delta| {
            if let Delta::Testo(t) = d {
                text.push_str(&t)
            }
        };
        let evs = [
            json!({"type":"message_start","message":{"model":"claude-opus-5-5","usage":{"input_tokens":12,"cache_read_input_tokens":100}}}),
            json!({"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"penso"}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"abc"}}),
            json!({"type":"content_block_stop","index":0}),
            json!({"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}),
            json!({"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"Ci"}}),
            json!({"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"ao"}}),
            json!({"type":"content_block_stop","index":1}),
            json!({"type":"content_block_start","index":2,"content_block":{"type":"tool_use","id":"t1","name":"leggi_file","input":{}}}),
            json!({"type":"content_block_delta","index":2,"delta":{"type":"input_json_delta","partial_json":"{\"percorso\":"}}),
            json!({"type":"content_block_delta","index":2,"delta":{"type":"input_json_delta","partial_json":"\"/tmp\"}"}}),
            json!({"type":"content_block_stop","index":2}),
            json!({"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":40}}),
        ];
        for e in &evs {
            assert!(!st.evento(e, &mut on).unwrap());
        }
        assert!(st.evento(&json!({"type":"message_stop"}), &mut on).unwrap());
        let r = st.fine();
        assert_eq!(text, "Ciao");
        assert_eq!(r.stop, Stop::ToolUse);
        assert_eq!(r.usage.cache_read, 100);
        assert_eq!(r.raw[0]["signature"], "abc");
        assert_eq!(r.raw[2]["input"]["percorso"], "/tmp");
        assert!(matches!(&r.blocchi[2], Blocco::ToolUse { nome, .. } if nome == "leggi_file"));
    }
}

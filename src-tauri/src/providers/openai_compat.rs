//! Endpoint compatibili OpenAI (Chat Completions in streaming): OpenAI e, più avanti, llama-server.

use super::sse::{dati, Righe, ThinkFilter};
use super::{api_error, client, prossimo, Delta, Richiesta, Risposta, Stop, Usage};
use crate::agent::messaggi::{Blocco, Messaggio, Ruolo};
use crate::allegati::{self, Contenuto};
use crate::error::{AppError, Result};
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Sapore {
    OpenAi,
    Llama,
}

impl Sapore {
    fn nome(self) -> &'static str {
        match self {
            Sapore::OpenAi => "openai",
            Sapore::Llama => "local",
        }
    }
}

fn base(url: &str) -> String {
    let b = url.trim().trim_end_matches('/');
    if b.is_empty() { "https://api.openai.com/v1".into() } else { b.into() }
}

fn messaggi(system: &str, msgs: &[Messaggio], sapore: Sapore) -> Vec<Value> {
    let mut out = vec![json!({ "role": "system", "content": system })];
    for m in msgs {
        match m.ruolo {
            Ruolo::Assistente => {
                if let (Some(raw), Some(p)) = (&m.raw, m.raw_provider.as_deref()) {
                    if p == sapore.nome() {
                        out.push(raw.clone());
                        continue;
                    }
                }
                let text: String = m
                    .blocchi
                    .iter()
                    .filter_map(|b| if let Blocco::Testo { testo } = b { Some(testo.as_str()) } else { None })
                    .collect::<Vec<_>>()
                    .join("");
                let calls: Vec<Value> = m
                    .blocchi
                    .iter()
                    .filter_map(|b| match b {
                        Blocco::ToolUse { id, nome, input, .. } => Some(json!({
                            "id": id, "type": "function",
                            "function": { "name": nome, "arguments": input.to_string() }
                        })),
                        _ => None,
                    })
                    .collect();
                let mut msg = json!({ "role": "assistant", "content": text });
                if !calls.is_empty() {
                    msg["tool_calls"] = json!(calls);
                }
                out.push(msg);
            }
            Ruolo::Utente => {
                let mut testo = Vec::new();
                let mut parti: Vec<Value> = Vec::new();
                // Le immagini dei risultati non stanno nei messaggi "tool": arrivano in un messaggio dopo
                let mut immagini: Vec<Value> = Vec::new();
                for b in &m.blocchi {
                    match b {
                        Blocco::Testo { testo: t } => testo.push(t.clone()),
                        Blocco::Allegato { nome, mime, formato: tipo, percorso, .. } => match allegati::carica(tipo, mime, percorso) {
                            Contenuto::Pdf(data) => parti.push(json!({
                                "type": "file",
                                "file": { "filename": nome, "file_data": format!("data:application/pdf;base64,{data}") }
                            })),
                            Contenuto::Immagine { mime, base64 } => parti.push(json!({
                                "type": "image_url", "image_url": { "url": format!("data:{mime};base64,{base64}") }
                            })),
                            Contenuto::Testo(t) => parti.push(json!({ "type": "text", "text": format!("<file nome=\"{nome}\">\n{t}\n</file>") })),
                            Contenuto::Mancante => parti.push(json!({ "type": "text", "text": format!("[allegato non più disponibile: {nome}]") })),
                        },
                        Blocco::ToolResult { tool_use_id, contenuto, is_error, immagine, .. } => {
                            out.push(json!({
                                "role": "tool",
                                "tool_call_id": tool_use_id,
                                "content": if *is_error { format!("ERRORE: {contenuto}") } else { contenuto.clone() },
                            }));
                            if let Some(b64) = immagine.as_deref().and_then(crate::agent::messaggi::immagine_base64) {
                                immagini.push(json!({ "type": "image_url", "image_url": { "url": format!("data:image/png;base64,{b64}") } }));
                            }
                        }
                        _ => {}
                    }
                }
                if !immagini.is_empty() {
                    // Solo le ultime immagini: le più vecchie non servono e pesano
                    let tieni = immagini.len().saturating_sub(2);
                    let mut c: Vec<Value> = vec![json!({ "type": "text", "text": "Immagine restituita dallo strumento:" })];
                    c.extend(immagini.into_iter().skip(tieni));
                    out.push(json!({ "role": "user", "content": c }));
                }
                if !parti.is_empty() {
                    if !testo.is_empty() {
                        parti.push(json!({ "type": "text", "text": testo.join("\n\n") }));
                    }
                    out.push(json!({ "role": "user", "content": parti }));
                } else if !testo.is_empty() {
                    out.push(json!({ "role": "user", "content": testo.join("\n\n") }));
                }
            }
        }
    }
    out
}

pub fn corpo(req: &Richiesta<'_>, sapore: Sapore) -> Value {
    let mut body = json!({
        "model": req.modello,
        "stream": true,
        "stream_options": { "include_usage": true },
        "messages": messaggi(req.system, req.messaggi, sapore),
    });
    if !req.tools.is_empty() {
        body["tools"] = json!(req.tools);
        body["tool_choice"] = json!("auto");
    }
    match sapore {
        Sapore::Llama => {
            // Tetto alla lunghezza: un modello locale che ragiona a vuoto non blocca la macchina
            body["max_tokens"] = json!(req.max_tokens.min(8192));
            // Impegno basso = ragionamento spento (risposte rapide)
            body["chat_template_kwargs"] = json!({ "enable_thinking": !matches!(req.effort, "low" | "") });
        }
        Sapore::OpenAi => {
            if !req.effort.is_empty() {
                let e = match req.effort {
                    "xhigh" | "max" => "high",
                    e => e,
                };
                body["reasoning_effort"] = json!(e);
            }
        }
    }
    body
}

#[derive(Default)]
struct Chiamata {
    id: String,
    nome: String,
    argomenti: String,
}

pub async fn stream(req: Richiesta<'_>, sapore: Sapore, on: &mut (dyn FnMut(Delta) + Send), cancel: &CancellationToken) -> Result<Risposta> {
    let body = corpo(&req, sapore);
    let mut r = client().post(format!("{}/chat/completions", base(req.base_url))).json(&body);
    if !req.chiave.is_empty() {
        r = r.bearer_auth(req.chiave);
    }
    let resp = tokio::select! {
        _ = cancel.cancelled() => return Err(AppError::Annullato),
        r = r.send() => r?,
    };
    if !resp.status().is_success() {
        return Err(api_error(resp).await);
    }

    let mut testo = String::new();
    let mut pensiero = String::new();
    let mut chiamate: Vec<Chiamata> = Vec::new();
    let mut finish: Option<String> = None;
    let mut usage = Usage::default();
    let mut modello = req.modello.to_string();
    let mut think = ThinkFilter::default();
    let mut righe = Righe::default();
    let mut s = resp.bytes_stream();
    'leggi: while let Some(bytes) = prossimo(&mut s, cancel).await? {
        for line in righe.push(&bytes) {
            let Some(data) = dati(&line) else { continue };
            if data == "[DONE]" {
                break 'leggi;
            }
            let Ok(v) = serde_json::from_str::<Value>(data) else { continue };
            if let Some(err) = v["error"]["message"].as_str() {
                return Err(AppError::msg(err.to_string()));
            }
            if let Some(m) = v["model"].as_str() {
                modello = m.to_string();
            }
            if v["usage"].is_object() {
                let u = &v["usage"];
                usage.input_tokens = u["prompt_tokens"].as_u64().unwrap_or(0);
                usage.output_tokens = u["completion_tokens"].as_u64().unwrap_or(0);
                usage.cache_read = u["prompt_tokens_details"]["cached_tokens"].as_u64().unwrap_or(0);
            }
            let ch = &v["choices"][0];
            if let Some(f) = ch["finish_reason"].as_str() {
                finish = Some(f.to_string());
            }
            let d = &ch["delta"];
            let r = d["reasoning_content"].as_str().or(d["reasoning"].as_str()).unwrap_or_default();
            if !r.is_empty() {
                pensiero.push_str(r);
                on(Delta::Pensiero(r.to_string()));
            }
            if let Some(c) = d["content"].as_str() {
                let (vis, rag) = think.push(c);
                if !rag.is_empty() {
                    pensiero.push_str(&rag);
                    on(Delta::Pensiero(rag));
                }
                if !vis.is_empty() {
                    testo.push_str(&vis);
                    on(Delta::Testo(vis));
                }
            }
            if let Some(calls) = d["tool_calls"].as_array() {
                for c in calls {
                    let i = c["index"].as_u64().unwrap_or(0) as usize;
                    while chiamate.len() <= i {
                        chiamate.push(Chiamata::default());
                    }
                    let k = &mut chiamate[i];
                    if let Some(id) = c["id"].as_str() {
                        k.id = id.to_string();
                    }
                    if let Some(n) = c["function"]["name"].as_str() {
                        k.nome.push_str(n);
                    }
                    if let Some(a) = c["function"]["arguments"].as_str() {
                        k.argomenti.push_str(a);
                    }
                }
            }
        }
    }

    let mut blocchi = Vec::new();
    if !pensiero.is_empty() {
        blocchi.push(Blocco::Pensiero { testo: pensiero });
    }
    if !testo.is_empty() {
        blocchi.push(Blocco::Testo { testo: testo.clone() });
    }
    let mut raw_calls = Vec::new();
    for (i, c) in chiamate.iter().enumerate() {
        let id = if c.id.is_empty() { format!("call_{i}") } else { c.id.clone() };
        let input = if c.argomenti.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str(&c.argomenti).unwrap_or_else(|_| json!({ "__json_non_valido": c.argomenti }))
        };
        raw_calls.push(json!({ "id": id, "type": "function", "function": { "name": c.nome, "arguments": c.argomenti } }));
        blocchi.push(Blocco::ToolUse { id, nome: c.nome.clone(), input, toolset: None });
    }
    let mut raw = json!({ "role": "assistant", "content": testo });
    if !raw_calls.is_empty() {
        raw["tool_calls"] = json!(raw_calls);
    }
    let stop = match finish.as_deref() {
        Some("tool_calls") => Stop::ToolUse,
        _ if !chiamate.is_empty() => Stop::ToolUse,
        Some("length") => Stop::MaxTokens,
        Some("content_filter") => Stop::Rifiuto,
        _ => Stop::Fine,
    };
    Ok(Risposta { blocchi, raw, stop, stop_details: None, usage, modello })
}

pub async fn models(key: &str, base_url: &str) -> Result<Vec<String>> {
    let resp = client().get(format!("{}/models", base(base_url))).bearer_auth(key).send().await?;
    if !resp.status().is_success() {
        return Err(api_error(resp).await);
    }
    let body: Value = resp.json().await?;
    let official = base_url.contains("api.openai.com") || base_url.trim().is_empty();
    let mut ids: Vec<String> = body["data"]
        .as_array()
        .map(|a| a.iter().filter_map(|m| m["id"].as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    // Solo modelli di chat e ragionamento, niente embedding/immagini/audio
    ids.retain(|id| {
        (!official || id.starts_with("gpt") || id.starts_with('o'))
            && !["embedding", "tts", "whisper", "dall-e", "image", "audio", "realtime", "transcribe", "moderation", "search"]
                .iter()
                .any(|x| id.contains(x))
    });
    ids.sort_by(|a, b| b.cmp(a));
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_results_become_tool_messages() {
        let msgs = vec![
            Messaggio::utente("leggi"),
            Messaggio {
                ruolo: Ruolo::Assistente,
                blocchi: vec![Blocco::ToolUse { id: "c1".into(), nome: "leggi_file".into(), input: json!({"p":1}), toolset: None }],
                raw: Some(json!({"role":"assistant","content":"","tool_calls":[]})),
                raw_provider: Some("anthropic".into()),
            },
            Messaggio {
                ruolo: Ruolo::Utente,
                blocchi: vec![Blocco::ToolResult { tool_use_id: "c1".into(), contenuto: "ok".into(), is_error: false, toolset: None, immagine: None }],
                raw: None,
                raw_provider: None,
            },
        ];
        let m = messaggi("sys", &msgs, Sapore::OpenAi);
        assert_eq!(m[0]["role"], "system");
        // Il turno di un altro provider si ricostruisce, non si copia
        assert_eq!(m[2]["tool_calls"][0]["function"]["name"], "leggi_file");
        assert_eq!(m[3]["role"], "tool");
        assert_eq!(m[3]["tool_call_id"], "c1");
    }

    /// Prova reale contro llama-server: `OLTREMARE_TEST_LLAMA=http://127.0.0.1:PORTA/v1 OLTREMARE_TEST_LLAMA_KEY=... cargo test llama_tool_call -- --ignored`
    #[tokio::test]
    #[ignore]
    async fn llama_tool_call() {
        let Ok(base) = std::env::var("OLTREMARE_TEST_LLAMA") else { return };
        let key = std::env::var("OLTREMARE_TEST_LLAMA_KEY").unwrap_or_default();
        let tools = crate::tools::per_openai(&crate::tools::abilitati(&["leggi_appunti".into()]));
        let msgs = vec![Messaggio::utente("Cosa ho copiato negli appunti? Controlla con lo strumento.")];
        let req = Richiesta { system: "Sei un assistente. Usa gli strumenti quando servono.", messaggi: &msgs, modello: "tiny", effort: "medium", max_tokens: 4096, chiave: &key, base_url: &base, tools: &tools };
        let mut testo = String::new();
        let mut on = |d: Delta| {
            if let Delta::Testo(t) = d {
                testo.push_str(&t)
            }
        };
        let r = stream(req, Sapore::Llama, &mut on, &CancellationToken::new()).await.unwrap();
        let chiamata = r.blocchi.iter().any(|b| matches!(b, Blocco::ToolUse { nome, .. } if nome == "leggi_appunti"));
        assert!(chiamata, "nessuna chiamata allo strumento; stop={:?} testo={testo}", r.stop);
        assert_eq!(r.stop, Stop::ToolUse);
        assert!(r.usage.input_tokens > 0);
    }
}

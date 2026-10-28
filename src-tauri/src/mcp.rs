//! Server MCP locale: espone gli strumenti di Oltremare a Claude Code (abbonamento).
//! Ogni esecuzione ha un indirizzo con un codice casuale; le chiamate passano dallo stesso
//! esecutore del ciclo, quindi valgono le stesse regole e approvazioni.

use crate::agent::strumenti::{self, ContestoRun};
use crate::tools;
use crate::App;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};

pub struct Mcp {
    pub porta: u16,
    contesti: Mutex<HashMap<String, Arc<ContestoRun>>>,
}

impl Mcp {
    pub fn avvia(app: AppHandle) -> std::io::Result<Self> {
        let std_listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        std_listener.set_nonblocking(true)?;
        let porta = std_listener.local_addr()?.port();
        tauri::async_runtime::spawn(async move {
            let Ok(listener) = tokio::net::TcpListener::from_std(std_listener) else { return };
            let router = Router::new().route("/mcp/{token}", post(gestisci).get(non_supportato).delete(chiudi)).with_state(app);
            let _ = axum::serve(listener, router).await;
        });
        Ok(Self { porta, contesti: Mutex::new(HashMap::new()) })
    }

    pub fn registra(&self, ctx: Arc<ContestoRun>) -> String {
        let token = format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple());
        self.contesti.lock().unwrap_or_else(|e| e.into_inner()).insert(token.clone(), ctx);
        token
    }

    pub fn rimuovi(&self, token: &str) {
        self.contesti.lock().unwrap_or_else(|e| e.into_inner()).remove(token);
    }

    fn contesto(&self, token: &str) -> Option<Arc<ContestoRun>> {
        self.contesti.lock().unwrap_or_else(|e| e.into_inner()).get(token).cloned()
    }

    /// Configurazione da passare a `claude --mcp-config`.
    pub fn config(&self, token: &str) -> String {
        json!({ "mcpServers": { "oltremare": { "type": "http", "url": format!("http://127.0.0.1:{}/mcp/{token}", self.porta) } } }).to_string()
    }
}

async fn non_supportato() -> StatusCode {
    StatusCode::METHOD_NOT_ALLOWED
}

async fn chiudi() -> StatusCode {
    StatusCode::OK
}

async fn gestisci(State(app): State<AppHandle>, Path(token): Path<String>, Json(body): Json<Value>) -> Response {
    let Some(ctx) = app.state::<App>().mcp.contesto(&token) else {
        return (StatusCode::NOT_FOUND, "sessione scaduta").into_response();
    };
    if let Value::Array(lista) = body {
        let mut out = Vec::new();
        for m in lista {
            if let Some(r) = messaggio(&ctx, m).await {
                out.push(r);
            }
        }
        return if out.is_empty() { StatusCode::ACCEPTED.into_response() } else { Json(Value::Array(out)).into_response() };
    }
    match messaggio(&ctx, body).await {
        Some(r) => Json(r).into_response(),
        None => StatusCode::ACCEPTED.into_response(),
    }
}

/// Un messaggio JSON-RPC. Le notifiche (senza id) non hanno risposta.
async fn messaggio(ctx: &ContestoRun, m: Value) -> Option<Value> {
    let id = m.get("id").cloned()?;
    let p = &m["params"];
    let risultato = match m["method"].as_str().unwrap_or_default() {
        "initialize" => Ok(json!({
            "protocolVersion": p["protocolVersion"].as_str().unwrap_or("2025-06-18"),
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": "oltremare", "version": env!("CARGO_PKG_VERSION") },
            "instructions": "Strumenti per agire sul computer dell'utente. Le azioni che modificano qualcosa possono richiedere la sua approvazione.",
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools::per_mcp(&ctx.strumenti()) })),
        "tools/call" => {
            let nome = p["name"].as_str().unwrap_or_default().to_string();
            let args = if p["arguments"].is_object() { p["arguments"].clone() } else { json!({}) };
            let e = strumenti::chiama(ctx, &nome, args, None).await;
            let mut content = vec![];
            if let Some(b64) = &e.immagine {
                content.push(json!({ "type": "image", "data": b64, "mimeType": "image/png" }));
            }
            content.push(json!({ "type": "text", "text": e.testo }));
            Ok(json!({ "content": content, "isError": e.is_error }))
        }
        altro => Err(json!({ "code": -32601, "message": format!("Metodo non supportato: {altro}") })),
    };
    Some(match risultato {
        Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
        Err(e) => json!({ "jsonrpc": "2.0", "id": id, "error": e }),
    })
}

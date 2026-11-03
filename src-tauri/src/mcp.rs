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
    /// Lavori dello Studio che chiedono i permessi all'utente: codice → cartella del progetto
    permessi: Mutex<HashMap<String, String>>,
}

impl Mcp {
    pub fn avvia(app: AppHandle) -> std::io::Result<Self> {
        let std_listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        std_listener.set_nonblocking(true)?;
        let porta = std_listener.local_addr()?.port();
        tauri::async_runtime::spawn(async move {
            let Ok(listener) = tokio::net::TcpListener::from_std(std_listener) else { return };
            let router = Router::new()
                .route("/mcp/{token}", post(gestisci).get(non_supportato).delete(chiudi))
                .route("/permessi/{token}", post(gestisci_permessi).get(non_supportato).delete(chiudi))
                .with_state(app);
            let _ = axum::serve(listener, router).await;
        });
        Ok(Self { porta, contesti: Mutex::new(HashMap::new()), permessi: Mutex::new(HashMap::new()) })
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

    /// Un lavoro dello Studio che chiederà i permessi all'utente: restituisce il codice per l'indirizzo.
    pub fn registra_permessi(&self, cartella: &str) -> String {
        let token = format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple());
        self.permessi.lock().unwrap_or_else(|e| e.into_inner()).insert(token.clone(), cartella.to_string());
        token
    }

    pub fn rimuovi_permessi(&self, token: &str) {
        self.permessi.lock().unwrap_or_else(|e| e.into_inner()).remove(token);
    }

    /// Server MCP con il solo strumento «permesso», per `--permission-prompt-tool mcp__oltremare-permessi__permesso`.
    pub fn config_permessi(&self, token: &str) -> String {
        json!({ "mcpServers": { "oltremare-permessi": { "type": "http", "url": format!("http://127.0.0.1:{}/permessi/{token}", self.porta) } } }).to_string()
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
        // Con Claude Code la ricerca web è la sua (WebSearch, WebFetch): gli strumenti web di Oltremare non servono
        "tools/list" => Ok(json!({ "tools": tools::per_mcp(&ctx.strumenti().into_iter().filter(|t| t.categoria != "web").collect::<Vec<_>>()) })),
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

// ---------- Permessi dei lavori dello Studio ----------

async fn gestisci_permessi(State(app): State<AppHandle>, Path(token): Path<String>, Json(body): Json<Value>) -> Response {
    let cartella = app.state::<App>().mcp.permessi.lock().unwrap_or_else(|e| e.into_inner()).get(&token).cloned();
    let Some(cartella) = cartella else {
        return (StatusCode::NOT_FOUND, "sessione scaduta").into_response();
    };
    let lista = match body {
        Value::Array(l) => l,
        v => vec![v],
    };
    let mut out = Vec::new();
    for m in lista {
        let Some(id) = m.get("id").cloned() else { continue };
        let p = &m["params"];
        let r = match m["method"].as_str().unwrap_or_default() {
            "initialize" => json!({ "protocolVersion": p["protocolVersion"].as_str().unwrap_or("2025-06-18"), "capabilities": { "tools": { "listChanged": false } }, "serverInfo": { "name": "oltremare-permessi", "version": env!("CARGO_PKG_VERSION") } }),
            "tools/list" => json!({ "tools": [{ "name": "permesso", "description": "Chiede all'utente il permesso per un'azione delicata.", "inputSchema": { "type": "object", "properties": { "tool_name": { "type": "string" }, "input": { "type": "object" }, "tool_use_id": { "type": "string" } }, "required": ["tool_name", "input"] } }] }),
            "tools/call" => {
                let a = &p["arguments"];
                let esito = chiedi_permesso(&app, &cartella, a["tool_name"].as_str().unwrap_or(""), a["input"].clone()).await;
                json!({ "content": [{ "type": "text", "text": esito.to_string() }] })
            }
            _ => json!({}),
        };
        out.push(json!({ "jsonrpc": "2.0", "id": id, "result": r }));
    }
    match out.len() {
        0 => StatusCode::ACCEPTED.into_response(),
        1 => Json(out.remove(0)).into_response(),
        _ => Json(Value::Array(out)).into_response(),
    }
}

/// Mostra la richiesta nello Studio (e come notifica) e aspetta la risposta dell'utente, fino a 30 minuti.
async fn chiedi_permesso(app: &AppHandle, cartella: &str, strumento: &str, input: Value) -> Value {
    use crate::agent::approvazioni::Risposta;
    use tauri::Emitter;
    let st = app.state::<App>();
    let id = uuid::Uuid::new_v4().to_string();
    let cosa = input["command"].as_str().or_else(|| input["file_path"].as_str()).map(str::to_string).unwrap_or_else(|| input.to_string());
    let richiesta = json!({ "id": id, "folder": cartella, "tool": strumento, "input": input, "title": cosa, "description": input["description"], "at": crate::store::now_ms() });
    let rx = st.approvazioni.registra(&id, &format!("studio:{cartella}"), richiesta.clone());
    let _ = app.emit("studio-permission", &richiesta);
    {
        use tauri_plugin_notification::NotificationExt;
        let nome = std::path::Path::new(cartella).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let _ = app.notification().builder().title(format!("Studio · {nome}: serve il tuo permesso")).body(cosa.chars().take(180).collect::<String>()).show();
    }
    let r = tokio::time::timeout(std::time::Duration::from_secs(1800), rx).await;
    st.approvazioni.rimuovi(&id);
    let _ = app.emit("studio-permission-done", json!({ "id": id }));
    match r {
        Ok(Ok(Risposta::Consenti | Risposta::ConsentiSessione | Risposta::ConsentiSempre)) => json!({ "behavior": "allow", "updatedInput": input }),
        Ok(Ok(Risposta::Nega(m))) => json!({ "behavior": "deny", "message": m.unwrap_or_else(|| "L'utente ha negato il permesso: non riprovare lo stesso comando, proponi un'alternativa nel riepilogo.".into()) }),
        _ => json!({ "behavior": "deny", "message": "Nessuna risposta dall'utente entro 30 minuti: azione non eseguita." }),
    }
}

/// Risposta dell'utente a una richiesta di permesso dello Studio.
#[tauri::command]
pub fn studio_permission_answer(app: tauri::State<'_, App>, id: String, allow: bool) -> bool {
    use crate::agent::approvazioni::Risposta;
    app.approvazioni.rispondi(&id, if allow { Risposta::Consenti } else { Risposta::Nega(None) })
}

/// Le richieste ancora in attesa per un progetto (riaprendo lo Studio).
#[tauri::command]
pub fn studio_permissions(app: tauri::State<'_, App>, folder: String) -> Vec<Value> {
    app.approvazioni.pendenti().into_iter().filter(|r| r["folder"] == folder.as_str()).collect()
}

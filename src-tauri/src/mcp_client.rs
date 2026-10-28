//! Client MCP: gli agenti usano gli strumenti di server MCP esterni (gestione issue, database,
//! servizi interni…). Trasporti: HTTP «streamable» e processo locale (stdio).
//! Gli strumenti dei server finiscono nel registro di Oltremare: stesse regole, approvazioni e registro.

use crate::store::{get_secret, ServerMcp};
use crate::tools::{self, Rischio, ToolSpec};
use crate::App;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, LazyLock, RwLock};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{oneshot, Mutex};

const PROTOCOLLO: &str = "2025-06-18";

/// Uno strumento di un server esterno, registrato come strumento di Oltremare.
pub struct Esterno {
    pub spec: &'static ToolSpec,
    pub server_id: String,
    pub originale: String,
    pub schema: Value,
}

static ESTERNI: LazyLock<RwLock<Vec<Esterno>>> = LazyLock::new(|| RwLock::new(vec![]));
/// I nomi già creati restano validi: le stringhe dei nomi sono «statiche» e si riusano tra i ricaricamenti
static NOMI: LazyLock<std::sync::Mutex<HashMap<String, &'static str>>> = LazyLock::new(Default::default);

fn statico(s: String) -> &'static str {
    let mut m = NOMI.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(x) = m.get(&s) {
        return x;
    }
    let x: &'static str = Box::leak(s.clone().into_boxed_str());
    m.insert(s, x);
    x
}

pub fn spec(nome: &str) -> Option<&'static ToolSpec> {
    ESTERNI.read().ok()?.iter().find(|e| e.spec.nome == nome).map(|e| e.spec)
}

pub fn tutti() -> Vec<&'static ToolSpec> {
    ESTERNI.read().map(|l| l.iter().map(|e| e.spec).collect()).unwrap_or_default()
}

pub fn schema(nome: &str) -> Option<Value> {
    ESTERNI.read().ok()?.iter().find(|e| e.spec.nome == nome).map(|e| e.schema.clone())
}

fn destinazione(nome: &str) -> Option<(String, String)> {
    ESTERNI.read().ok()?.iter().find(|e| e.spec.nome == nome).map(|e| (e.server_id.clone(), e.originale.clone()))
}

fn slug(s: &str) -> String {
    let x: String = s.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect();
    x.trim_matches('_').to_string()
}

enum Trasporto {
    Http {
        client: reqwest::Client,
        url: String,
        server: ServerMcp,
        sessione: std::sync::Mutex<Option<String>>,
        intestazioni: std::sync::Mutex<Vec<(String, String)>>,
    },
    Stdio {
        scrittore: Mutex<tokio::process::ChildStdin>,
        attese: Arc<std::sync::Mutex<HashMap<i64, oneshot::Sender<Value>>>>,
        _figlio: tokio::process::Child,
    },
}

pub struct Cliente {
    trasporto: Trasporto,
    prossimo: AtomicI64,
}

/// Intestazioni del server: quelle salvate nel portachiavi più quelle prodotte dal comando «headersHelper».
async fn intestazioni(s: &ServerMcp) -> Result<Vec<(String, String)>, String> {
    let mut out: Vec<(String, String)> = vec![];
    if let Ok(j) = get_secret(&format!("mcp-headers:{}", s.id)) {
        if let Ok(Value::Object(m)) = serde_json::from_str::<Value>(&j) {
            out.extend(m.into_iter().filter_map(|(k, v)| v.as_str().map(|v| (k, v.to_string()))));
        }
    }
    if !s.headers_helper.trim().is_empty() {
        let mut cmd = if cfg!(windows) {
            let mut c = tokio::process::Command::new("cmd");
            c.arg("/C").arg(&s.headers_helper);
            c
        } else {
            let mut c = tokio::process::Command::new("/bin/sh");
            c.arg("-c").arg(&s.headers_helper);
            c
        };
        cmd.env("PATH", crate::shell_env::path_utente()).kill_on_drop(true);
        let o = tokio::time::timeout(Duration::from_secs(15), cmd.output()).await.map_err(|_| "Il comando delle intestazioni non risponde".to_string())?.map_err(|e| e.to_string())?;
        if !o.status.success() {
            return Err(format!("Il comando delle intestazioni è fallito: {}", String::from_utf8_lossy(&o.stderr).trim()));
        }
        match serde_json::from_slice::<Value>(&o.stdout) {
            Ok(Value::Object(m)) => out.extend(m.into_iter().filter_map(|(k, v)| v.as_str().map(|v| (k, v.to_string())))),
            _ => return Err("Il comando delle intestazioni deve stampare un oggetto JSON".into()),
        }
    }
    Ok(out)
}

/// Estrae dal corpo (JSON o flusso SSE) la risposta con l'id cercato.
fn trova_risposta(corpo: &str, id: i64) -> Option<Value> {
    let prova = |v: Value| -> Option<Value> {
        match v {
            Value::Array(l) => l.into_iter().find(|m| m["id"].as_i64() == Some(id)),
            m if m["id"].as_i64() == Some(id) => Some(m),
            _ => None,
        }
    };
    if let Ok(v) = serde_json::from_str::<Value>(corpo.trim()) {
        return prova(v);
    }
    let mut dati = String::new();
    for riga in corpo.lines().chain(std::iter::once("")) {
        if let Some(d) = riga.strip_prefix("data:") {
            dati.push_str(d.trim_start());
        } else if riga.trim().is_empty() && !dati.is_empty() {
            if let Some(r) = serde_json::from_str::<Value>(&dati).ok().and_then(prova) {
                return Some(r);
            }
            dati.clear();
        }
    }
    None
}

impl Cliente {
    async fn connetti(s: &ServerMcp) -> Result<(Self, Value), String> {
        let trasporto = if s.kind == "stdio" {
            if s.command.trim().is_empty() {
                return Err("Manca il comando da avviare".into());
            }
            let mut cmd = tokio::process::Command::new(s.command.trim());
            cmd.args(&s.args).env("PATH", crate::shell_env::path_utente()).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).kill_on_drop(true);
            if let Some(h) = crate::shell_env::home() {
                cmd.current_dir(h);
            }
            if let Ok(j) = get_secret(&format!("mcp-env:{}", s.id)) {
                if let Ok(Value::Object(m)) = serde_json::from_str::<Value>(&j) {
                    for (k, v) in m {
                        if let Some(v) = v.as_str() {
                            cmd.env(k, v);
                        }
                    }
                }
            }
            #[cfg(windows)]
            {
                cmd.creation_flags(0x0800_0000);
            }
            let mut figlio = cmd.spawn().map_err(|e| format!("Non riesco ad avviare «{}»: {e}", s.command))?;
            let stdin = figlio.stdin.take().ok_or("stdin non disponibile")?;
            let stdout = figlio.stdout.take().ok_or("stdout non disponibile")?;
            let attese: Arc<std::sync::Mutex<HashMap<i64, oneshot::Sender<Value>>>> = Default::default();
            let a2 = attese.clone();
            tauri::async_runtime::spawn(async move {
                let mut righe = BufReader::new(stdout).lines();
                while let Ok(Some(r)) = righe.next_line().await {
                    let Ok(v) = serde_json::from_str::<Value>(&r) else { continue };
                    if let Some(id) = v["id"].as_i64() {
                        if v.get("method").is_none() {
                            if let Some(tx) = a2.lock().unwrap_or_else(|e| e.into_inner()).remove(&id) {
                                let _ = tx.send(v);
                            }
                        }
                    }
                }
            });
            Trasporto::Stdio { scrittore: Mutex::new(stdin), attese, _figlio: figlio }
        } else {
            if !s.url.starts_with("http://") && !s.url.starts_with("https://") {
                return Err("L'indirizzo deve iniziare con https://".into());
            }
            let client = reqwest::Client::builder().connect_timeout(Duration::from_secs(15)).build().map_err(|e| e.to_string())?;
            Trasporto::Http { client, url: s.url.trim().to_string(), server: s.clone(), sessione: Default::default(), intestazioni: std::sync::Mutex::new(intestazioni(s).await?) }
        };
        let c = Self { trasporto, prossimo: AtomicI64::new(1) };
        let init = c
            .richiesta(
                "initialize",
                json!({ "protocolVersion": PROTOCOLLO, "capabilities": {}, "clientInfo": { "name": "oltremare", "version": env!("CARGO_PKG_VERSION") } }),
                Duration::from_secs(30),
            )
            .await?;
        c.notifica("notifications/initialized").await;
        Ok((c, init))
    }

    async fn notifica(&self, metodo: &str) {
        let m = json!({ "jsonrpc": "2.0", "method": metodo });
        match &self.trasporto {
            Trasporto::Stdio { scrittore, .. } => {
                let mut w = scrittore.lock().await;
                let _ = w.write_all(format!("{m}\n").as_bytes()).await;
                let _ = w.flush().await;
            }
            Trasporto::Http { .. } => {
                let _ = self.http(&m, None, Duration::from_secs(10)).await;
            }
        }
    }

    async fn http(&self, m: &Value, id: Option<i64>, timeout: Duration) -> Result<Option<Value>, String> {
        let Trasporto::Http { client, url, server, sessione, intestazioni: int } = &self.trasporto else { return Ok(None) };
        for tentativo in 0..2 {
            let mut req = client.post(url).timeout(timeout).header("Content-Type", "application/json").header("Accept", "application/json, text/event-stream").header("MCP-Protocol-Version", PROTOCOLLO);
            for (k, v) in int.lock().unwrap_or_else(|e| e.into_inner()).iter() {
                req = req.header(k, v);
            }
            if let Some(sid) = sessione.lock().unwrap_or_else(|e| e.into_inner()).clone() {
                req = req.header("Mcp-Session-Id", sid);
            }
            let r = req.json(m).send().await.map_err(|e| format!("Server non raggiungibile: {e}"))?;
            if let Some(sid) = r.headers().get("mcp-session-id").and_then(|v| v.to_str().ok()) {
                *sessione.lock().unwrap_or_else(|e| e.into_inner()) = Some(sid.to_string());
            }
            let stato = r.status();
            // Accesso scaduto: si rigenerano le intestazioni una volta
            if stato.as_u16() == 401 && tentativo == 0 && !server.headers_helper.is_empty() {
                *int.lock().unwrap_or_else(|e| e.into_inner()) = intestazioni(server).await?;
                continue;
            }
            if !stato.is_success() {
                let t = r.text().await.unwrap_or_default();
                return Err(match stato.as_u16() {
                    401 | 403 => "Accesso negato dal server: controlla le intestazioni di autenticazione".into(),
                    404 if sessione.lock().map(|s| s.is_some()).unwrap_or(false) => "Sessione scaduta: ricarica i server".into(),
                    _ => format!("Il server ha risposto {stato}: {}", t.chars().take(200).collect::<String>()),
                });
            }
            let Some(id) = id else { return Ok(None) };
            // Il corpo può essere JSON o un flusso SSE: si legge finché arriva la risposta
            let mut r = r;
            let mut corpo = String::new();
            loop {
                match r.chunk().await {
                    Ok(Some(b)) => {
                        corpo.push_str(&String::from_utf8_lossy(&b));
                        if corpo.contains("\n\n") || corpo.contains("\r\n\r\n") {
                            if let Some(v) = trova_risposta(&corpo.replace("\r\n", "\n"), id) {
                                return Ok(Some(v));
                            }
                        }
                    }
                    Ok(None) => break,
                    Err(e) => return Err(format!("Risposta interrotta: {e}")),
                }
            }
            return trova_risposta(&corpo.replace("\r\n", "\n"), id).map(Some).ok_or_else(|| "Risposta del server non valida".to_string());
        }
        Err("Accesso negato dal server".into())
    }

    pub async fn richiesta(&self, metodo: &str, params: Value, timeout: Duration) -> Result<Value, String> {
        let id = self.prossimo.fetch_add(1, Ordering::Relaxed);
        let m = json!({ "jsonrpc": "2.0", "id": id, "method": metodo, "params": params });
        let risposta = match &self.trasporto {
            Trasporto::Stdio { scrittore, attese, .. } => {
                let (tx, rx) = oneshot::channel();
                attese.lock().unwrap_or_else(|e| e.into_inner()).insert(id, tx);
                {
                    let mut w = scrittore.lock().await;
                    w.write_all(format!("{m}\n").as_bytes()).await.map_err(|_| "Il server si è chiuso".to_string())?;
                    w.flush().await.map_err(|e| e.to_string())?;
                }
                match tokio::time::timeout(timeout, rx).await {
                    Ok(Ok(v)) => v,
                    Ok(Err(_)) => return Err("Il server si è chiuso".into()),
                    Err(_) => {
                        attese.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
                        return Err("Il server non ha risposto in tempo".into());
                    }
                }
            }
            Trasporto::Http { .. } => self.http(&m, Some(id), timeout).await?.ok_or("Nessuna risposta")?,
        };
        if let Some(e) = risposta.get("error") {
            return Err(e["message"].as_str().unwrap_or("errore del server").to_string());
        }
        Ok(risposta["result"].clone())
    }
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct StatoServer {
    pub id: String,
    pub name: String,
    pub connected: bool,
    pub connecting: bool,
    pub tools: Vec<String>,
    pub error: Option<String>,
    #[serde(skip)]
    pub instructions: Option<String>,
}

#[derive(Default)]
pub struct Clienti {
    attivi: std::sync::Mutex<HashMap<String, Arc<Cliente>>>,
    stato: std::sync::Mutex<Vec<StatoServer>>,
    giro: AtomicI64,
}

impl Clienti {
    pub fn stato(&self) -> Vec<StatoServer> {
        self.stato.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Istruzioni dei server (dal loro «initialize») per gli strumenti che l'agente usa.
    pub fn istruzioni(&self, strumenti: &[&ToolSpec]) -> Vec<(String, String)> {
        self.stato()
            .into_iter()
            .filter(|s| s.instructions.is_some() && strumenti.iter().any(|t| t.categoria == format!("mcp:{}", s.id)))
            .map(|s| (s.name, s.instructions.unwrap_or_default()))
            .collect()
    }

    fn cliente(&self, id: &str) -> Option<Arc<Cliente>> {
        self.attivi.lock().unwrap_or_else(|e| e.into_inner()).get(id).cloned()
    }
}

/// Si ricollega a tutti i server attivi e aggiorna il registro degli strumenti.
pub async fn ricarica(app: &AppHandle) {
    let st = app.state::<App>();
    let giro = st.mcp_clienti.giro.fetch_add(1, Ordering::SeqCst) + 1;
    let server: Vec<ServerMcp> = st.store.settings().mcp_servers.into_iter().filter(|s| s.enabled).collect();
    *st.mcp_clienti.stato.lock().unwrap_or_else(|e| e.into_inner()) = server.iter().map(|s| StatoServer { id: s.id.clone(), name: s.name.clone(), connecting: true, ..Default::default() }).collect();
    let _ = app.emit("mcp-status", st.mcp_clienti.stato());

    let risultati = futures::future::join_all(server.iter().map(|s| async move {
        let r = async {
            let (c, init) = Cliente::connetti(s).await?;
            let mut strumenti = vec![];
            let mut cursore: Option<String> = None;
            loop {
                let p = match &cursore {
                    Some(c) => json!({ "cursor": c }),
                    None => json!({}),
                };
                let r = c.richiesta("tools/list", p, Duration::from_secs(30)).await?;
                strumenti.extend(r["tools"].as_array().cloned().unwrap_or_default());
                cursore = r["nextCursor"].as_str().map(str::to_string);
                if cursore.is_none() || strumenti.len() > 500 {
                    break;
                }
            }
            Ok::<_, String>((c, init, strumenti))
        }
        .await;
        (s, r)
    }))
    .await;

    // Un ricaricamento più recente vince
    if st.mcp_clienti.giro.load(Ordering::SeqCst) != giro {
        return;
    }
    let mut attivi = HashMap::new();
    let mut stato = vec![];
    let mut esterni = vec![];
    for (s, r) in risultati {
        match r {
            Ok((c, init, strumenti)) => {
                let prefisso = slug(&s.name);
                let mut nomi = vec![];
                for t in strumenti {
                    let Some(orig) = t["name"].as_str() else { continue };
                    let mut nome: String = format!("{}_{}", prefisso, slug(orig)).chars().take(64).collect();
                    if esterni.iter().any(|e: &Esterno| e.spec.nome == nome) || tools::CATALOGO.iter().any(|x| x.nome == nome) {
                        nome = format!("{}_{}", nome.chars().take(58).collect::<String>(), esterni.len());
                    }
                    let ann = &t["annotations"];
                    let rischio = if ann["readOnlyHint"].as_bool() == Some(true) {
                        Rischio::Read
                    } else if ann["destructiveHint"].as_bool() == Some(true) {
                        Rischio::Danger
                    } else {
                        Rischio::Write
                    };
                    let titolo = t["title"].as_str().or(ann["title"].as_str()).unwrap_or(orig).to_string();
                    let descrizione = t["description"].as_str().unwrap_or("").chars().take(1000).collect::<String>();
                    let schema = if t["inputSchema"].is_object() { t["inputSchema"].clone() } else { json!({ "type": "object", "properties": {} }) };
                    let spec: &'static ToolSpec = Box::leak(Box::new(ToolSpec {
                        nome: statico(nome.clone()),
                        etichetta: statico(titolo),
                        descrizione: statico(if descrizione.is_empty() { format!("Strumento «{orig}» del server {}", s.name) } else { descrizione }),
                        categoria: statico(format!("mcp:{}", s.id)),
                        rischio,
                        parallelo: rischio == Rischio::Read,
                        input_grande: false,
                        schema: || Value::Null,
                    }));
                    nomi.push(nome);
                    esterni.push(Esterno { spec, server_id: s.id.clone(), originale: orig.to_string(), schema });
                }
                let istruzioni = init["instructions"].as_str().filter(|x| !x.trim().is_empty()).map(|x| x.chars().take(4000).collect());
                stato.push(StatoServer { id: s.id.clone(), name: s.name.clone(), connected: true, connecting: false, tools: nomi, error: None, instructions: istruzioni });
                attivi.insert(s.id.clone(), Arc::new(c));
            }
            Err(e) => stato.push(StatoServer { id: s.id.clone(), name: s.name.clone(), error: Some(e), ..Default::default() }),
        }
    }
    if let Ok(mut l) = ESTERNI.write() {
        *l = esterni;
    }
    *st.mcp_clienti.attivi.lock().unwrap_or_else(|e| e.into_inner()) = attivi;
    *st.mcp_clienti.stato.lock().unwrap_or_else(|e| e.into_inner()) = stato;
    let _ = app.emit("mcp-status", st.mcp_clienti.stato());
}

/// Esegue uno strumento esterno.
pub async fn chiama(app: &AppHandle, nome: &str, input: &Value, cancel: &tokio_util::sync::CancellationToken) -> tools::Esito {
    let Some((server_id, originale)) = destinazione(nome) else { return tools::Esito::errore("Server non più disponibile") };
    let st = app.state::<App>();
    let Some(c) = st.mcp_clienti.cliente(&server_id) else { return tools::Esito::errore("Il server non è collegato: controlla Impostazioni → Strumenti esterni") };
    let nome_server = st.mcp_clienti.stato().into_iter().find(|s| s.id == server_id).map(|s| s.name).unwrap_or_default();
    let r = tokio::select! {
        r = c.richiesta("tools/call", json!({ "name": originale, "arguments": input }), Duration::from_secs(300)) => r,
        _ = cancel.cancelled() => Err("Interrotto".into()),
    };
    let r = match r {
        Ok(r) => r,
        Err(e) => return tools::Esito::errore(format!("{nome_server}: {e}")),
    };
    let mut testo = vec![];
    let mut immagine = None;
    for b in r["content"].as_array().into_iter().flatten() {
        match b["type"].as_str() {
            Some("text") => testo.push(b["text"].as_str().unwrap_or_default().to_string()),
            Some("image") if immagine.is_none() => immagine = b["data"].as_str().map(str::to_string),
            Some("resource") => testo.push(b["resource"]["text"].as_str().map(str::to_string).unwrap_or_else(|| b["resource"]["uri"].to_string())),
            Some("resource_link") => testo.push(format!("{} ({})", b["name"].as_str().unwrap_or_default(), b["uri"].as_str().unwrap_or_default())),
            _ => {}
        }
    }
    if testo.is_empty() && !r["structuredContent"].is_null() {
        testo.push(r["structuredContent"].to_string());
    }
    let mut t = testo.join("\n");
    if t.chars().count() > 60_000 {
        t = format!("{}\n… [risultato troncato]", t.chars().take(60_000).collect::<String>());
    }
    if t.is_empty() {
        t = "Fatto.".into();
    }
    let mut e = if r["isError"].as_bool() == Some(true) { tools::Esito::errore(t.clone()) } else { tools::Esito::ok(t.clone(), json!({ "kind": "text", "text": t.chars().take(4000).collect::<String>() })) };
    e.immagine = immagine;
    e
}

/// Server MCP configurati in Claude Code (ambito utente): si importano le definizioni.
/// Le intestazioni fisse e le variabili d'ambiente finiscono nel portachiavi.
pub fn da_claude_code() -> Vec<(ServerMcp, Option<Value>, Option<Value>)> {
    let Some(h) = crate::shell_env::home() else { return vec![] };
    let Ok(t) = std::fs::read_to_string(h.join(".claude.json")) else { return vec![] };
    let Ok(v) = serde_json::from_str::<Value>(&t) else { return vec![] };
    let mut out = vec![];
    for (nome, c) in v["mcpServers"].as_object().into_iter().flatten() {
        let tipo = c["type"].as_str().unwrap_or(if c["command"].is_string() { "stdio" } else { "http" });
        let server = ServerMcp {
            id: uuid::Uuid::new_v4().to_string(),
            name: nome.clone(),
            kind: if tipo == "stdio" { "stdio".into() } else { "http".into() },
            url: c["url"].as_str().unwrap_or_default().into(),
            headers_helper: c["headersHelper"].as_str().unwrap_or_default().into(),
            command: c["command"].as_str().unwrap_or_default().into(),
            args: c["args"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default(),
            enabled: true,
        };
        out.push((server, c.get("headers").filter(|h| h.is_object()).cloned(), c.get("env").filter(|e| e.is_object()).cloned()));
    }
    out
}

/// Nomi dei server MCP noti a Claude Code (per consentirne gli strumenti quando sviluppa).
pub fn nomi_claude_code() -> Vec<String> {
    da_claude_code().into_iter().map(|(s, _, _)| s.name).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn risposta_sse_e_json() {
        let sse = "event: message\ndata: {\"jsonrpc\":\"2.0\",\"method\":\"notifications/progress\"}\n\nevent: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":3,\"result\":{\"ok\":true}}\n\n";
        assert_eq!(super::trova_risposta(sse, 3).unwrap()["result"]["ok"], true);
        assert_eq!(super::trova_risposta("{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}", 1).unwrap()["id"], 1);
        assert!(super::trova_risposta("{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{}}", 1).is_none());
        assert_eq!(super::slug("Key Ops!"), "key_ops");
    }
}

#[cfg(test)]
mod prova_reale {
    /// Collegamento vero ai server MCP configurati in Claude Code: solo initialize e tools/list.
    #[tokio::test]
    #[ignore]
    async fn elenca_strumenti_dei_server_di_claude_code() {
        for (s, _, _) in super::da_claude_code() {
            let (c, init) = super::Cliente::connetti(&s).await.unwrap_or_else(|e| panic!("{}: {e}", s.name));
            let r = c.richiesta("tools/list", serde_json::json!({}), std::time::Duration::from_secs(30)).await.unwrap();
            let nomi: Vec<&str> = r["tools"].as_array().unwrap().iter().filter_map(|t| t["name"].as_str()).collect();
            println!("{}: {} strumenti {:?}; istruzioni: {}", s.name, nomi.len(), nomi, init["instructions"].as_str().unwrap_or("-").chars().take(80).collect::<String>());
            assert!(!nomi.is_empty());
        }
    }
}

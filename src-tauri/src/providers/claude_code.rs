//! Claude con il tuo abbonamento: usa Claude Code installato sul computer, con il tuo accesso
//! claude.ai (Pro/Max). Oltremare non legge né copia le credenziali: avvia il programma ufficiale
//! in modalità non interattiva (`claude -p`) e ne legge lo streaming, che ha gli stessi eventi
//! della Messages API. Gli strumenti di Claude Code sono spenti: le azioni passano da Oltremare.

use super::anthropic::Stato;
use super::{Delta, Risposta};
use crate::error::{AppError, Result};
use serde::Serialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use crate::shell_env::{home, path_utente};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio_util::sync::CancellationToken;

pub struct Chiamata<'a> {
    pub bin: &'a Path,
    pub cwd: &'a Path,
    pub system: &'a str,
    pub prompt: &'a str,
    /// Alias ("opus", "sonnet", "haiku"), id completo, oppure vuoto/"default" per il modello del piano
    pub modello: &'a str,
    pub effort: &'a str,
    pub sessione: Option<&'a str>,
    /// Cartelle che Claude Code può leggere con il suo strumento Read (gli allegati). Il resto del disco gli resta negato.
    pub cartelle_lettura: &'a [PathBuf],
    /// Configurazione del server MCP di Oltremare (strumenti sul computer), se l'agente ne ha
    pub mcp_config: Option<&'a str>,
    /// Quanto può durare una chiamata a uno strumento (attesa dell'approvazione compresa)
    pub timeout_strumenti_s: u64,
}

/// Trova l'eseguibile di Claude Code: percorso scelto dall'utente, posti noti, poi la shell.
pub fn trova(personalizzato: &str) -> Option<PathBuf> {
    let p = personalizzato.trim();
    if !p.is_empty() {
        let p = PathBuf::from(p);
        return p.is_file().then_some(p);
    }
    let mut candidati: Vec<PathBuf> = Vec::new();
    if let Some(h) = home() {
        if cfg!(windows) {
            candidati.push(h.join(".local/bin/claude.exe"));
            candidati.push(h.join("AppData/Roaming/npm/claude.cmd"));
        } else {
            candidati.push(h.join(".local/bin/claude"));
            candidati.push(h.join(".claude/local/claude"));
        }
    }
    if !cfg!(windows) {
        candidati.push("/opt/homebrew/bin/claude".into());
        candidati.push("/usr/local/bin/claude".into());
    }
    if let Some(c) = candidati.into_iter().find(|c| c.is_file()) {
        return Some(c);
    }
    let sep = if cfg!(windows) { ';' } else { ':' };
    let nome = if cfg!(windows) { "claude.exe" } else { "claude" };
    path_utente().split(sep).map(|d| Path::new(d).join(nome)).find(|c| c.is_file())
}

pub(crate) fn comando(bin: &Path) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(bin);
    cmd.env("PATH", path_utente())
        // Così Claude Code usa l'accesso dell'abbonamento e non una chiave API eventualmente presente
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("ANTHROPIC_AUTH_TOKEN")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    {
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StatoAccount {
    pub installed: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    pub logged_in: bool,
    pub auth_method: Option<String>,
    pub email: Option<String>,
    pub subscription: Option<String>,
    pub error: Option<String>,
}

pub async fn stato(personalizzato: &str) -> StatoAccount {
    let Some(bin) = trova(personalizzato) else {
        return StatoAccount { error: Some("Claude Code non trovato su questo computer".into()), ..Default::default() };
    };
    let mut s = StatoAccount { installed: true, path: Some(bin.to_string_lossy().into_owned()), ..Default::default() };
    if let Ok(o) = comando(&bin).arg("--version").output().await {
        s.version = Some(String::from_utf8_lossy(&o.stdout).trim().replace(" (Claude Code)", ""));
    }
    match comando(&bin).args(["auth", "status"]).output().await {
        Ok(o) => match serde_json::from_slice::<Value>(&o.stdout) {
            Ok(v) => {
                s.logged_in = v["loggedIn"].as_bool().unwrap_or(false);
                s.auth_method = v["authMethod"].as_str().map(str::to_string);
                s.email = v["email"].as_str().map(str::to_string);
                s.subscription = v["subscriptionType"].as_str().map(str::to_string);
            }
            Err(_) => s.error = Some("Non riesco a leggere lo stato dell'accesso di Claude Code".into()),
        },
        Err(e) => s.error = Some(format!("Claude Code non si avvia: {e}")),
    }
    s
}

fn errore_leggibile(testo: &str, sblocco: Option<i64>) -> AppError {
    let t = testo.to_lowercase();
    if t.contains("login") || t.contains("api key") || t.contains("authentication") || t.contains("oauth") {
        return AppError::msg("Claude Code non è collegato al tuo account: apri il Terminale, esegui «claude» e usa /login");
    }
    if t.contains("limit") || t.contains("usage") {
        if let Some(ts) = sblocco.and_then(|s| chrono::DateTime::from_timestamp(s, 0)) {
            let ora = ts.with_timezone(&chrono::Local).format("%H:%M");
            return AppError::msg(format!("Hai raggiunto il limite del tuo piano Claude: si sblocca alle {ora}"));
        }
        return AppError::msg("Hai raggiunto il limite del tuo piano Claude: riprova più tardi");
    }
    AppError::msg(format!("Claude Code: {}", testo.trim()))
}

/// Restituisce la risposta e l'id della sessione di Claude Code, da riprendere al turno dopo.
pub async fn stream(c: Chiamata<'_>, on: &mut (dyn FnMut(Delta) + Send), cancel: &CancellationToken) -> Result<(Risposta, Option<String>)> {
    std::fs::create_dir_all(c.cwd)?;
    let mut cmd = comando(c.bin);
    cmd.current_dir(c.cwd)
        .stdin(Stdio::piped())
        .args(["-p", "--output-format", "stream-json", "--verbose", "--include-partial-messages"])
        // Niente impostazioni, hook o server MCP dell'utente
        .args(["--setting-sources", "", "--strict-mcp-config"])
        .args(["--system-prompt", c.system]);
    let modello = c.modello.trim();
    if !modello.is_empty() && modello != "default" {
        cmd.args(["--model", modello]);
    }
    if !c.effort.is_empty() {
        cmd.args(["--effort", c.effort]);
    }
    if let Some(id) = c.sessione {
        cmd.args(["--resume", id]);
    }
    if let Some(cfg) = c.mcp_config {
        // Strumenti di Oltremare: autorizzati qui, perché regole e approvazioni le applica Oltremare
        cmd.args(["--mcp-config", cfg, "--allowedTools", "mcp__oltremare"])
            .env("MCP_TOOL_TIMEOUT", (c.timeout_strumenti_s * 1000).to_string())
            .env("MCP_TIMEOUT", "20000");
    }
    // Degli strumenti di Claude Code resta al massimo Read, solo sulle cartelle degli allegati
    if c.cartelle_lettura.is_empty() {
        cmd.args(["--tools", ""]);
    } else {
        cmd.args(["--tools", "Read"]);
        for d in c.cartelle_lettura {
            cmd.arg("--add-dir").arg(d);
        }
    }
    let mut child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            AppError::msg("Claude Code non trovato: installalo da claude.com/download o indica il percorso in Impostazioni → Provider")
        } else {
            AppError::msg(format!("Claude Code non si avvia: {e}"))
        }
    })?;
    // Il messaggio passa dall'input standard: nessun limite di lunghezza degli argomenti
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(c.prompt.as_bytes()).await?;
        drop(stdin);
    }
    let stdout = child.stdout.take().ok_or_else(|| AppError::msg("Claude Code: nessun output"))?;
    let mut stderr = child.stderr.take();
    let mut righe = BufReader::new(stdout).lines();

    let mut st = Stato::nuovo(if modello.is_empty() { "claude" } else { modello });
    let mut sessione: Option<String> = None;
    let mut sblocco: Option<i64> = None;
    let mut esito: Option<std::result::Result<(), String>> = None;
    loop {
        let riga = tokio::select! {
            _ = cancel.cancelled() => {
                let _ = child.start_kill();
                return Err(AppError::Annullato);
            }
            r = righe.next_line() => r?,
        };
        let Some(riga) = riga else { break };
        let Ok(v) = serde_json::from_str::<Value>(&riga) else { continue };
        if let Some(id) = v["session_id"].as_str() {
            sessione = Some(id.to_string());
        }
        match v["type"].as_str().unwrap_or_default() {
            "stream_event" => {
                st.evento(&v["event"], on)?;
            }
            "rate_limit_event" => {
                sblocco = v["rate_limit_info"]["resetsAt"].as_i64();
            }
            "result" => {
                esito = Some(if v["is_error"].as_bool().unwrap_or(false) {
                    Err(v["result"].as_str().or(v["subtype"].as_str()).unwrap_or("errore sconosciuto").to_string())
                } else {
                    Ok(())
                });
                break;
            }
            _ => {}
        }
    }
    let _ = child.wait().await;
    match esito {
        Some(Ok(())) => {
            let mut r = st.fine();
            r.raw = json!({ "session_id": sessione, "content": r.raw });
            Ok((r, sessione))
        }
        Some(Err(msg)) => Err(errore_leggibile(&msg, sblocco)),
        None => {
            let mut err = String::new();
            if let Some(e) = stderr.as_mut() {
                let _ = e.read_to_string(&mut err).await;
            }
            Err(errore_leggibile(if err.trim().is_empty() { "si è chiuso senza rispondere" } else { &err }, sblocco))
        }
    }
}

#[cfg(test)]
mod tests {
    /// Parla davvero con Claude Code (consuma pochi token del piano): `cargo test -- --ignored`
    #[tokio::test]
    #[ignore]
    async fn real_claude_code_streams_and_resumes() {
        let bin = super::trova("").expect("Claude Code installato");
        let cwd = std::env::temp_dir().join("oltremare-cc-test");
        let cancel = tokio_util::sync::CancellationToken::new();
        let mut testo = String::new();
        let mut on = |d: super::Delta| {
            if let super::Delta::Testo(t) = d {
                testo.push_str(&t)
            }
        };
        let c = super::Chiamata { bin: &bin, cwd: &cwd, system: "Rispondi in una riga.", prompt: "Il mio colore preferito è il blu oltremare. Rispondi solo: ok", modello: "haiku", effort: "", sessione: None, cartelle_lettura: &[], mcp_config: None, timeout_strumenti_s: 300 };
        let (r, sessione) = super::stream(c, &mut on, &cancel).await.unwrap();
        assert!(!testo.is_empty(), "nessun testo in streaming");
        assert!(r.raw["session_id"].is_string());
        let mut secondo = String::new();
        let mut on2 = |d: super::Delta| {
            if let super::Delta::Testo(t) = d {
                secondo.push_str(&t)
            }
        };
        let c = super::Chiamata { bin: &bin, cwd: &cwd, system: "Rispondi in una riga.", prompt: "Qual è il mio colore preferito?", modello: "haiku", effort: "", sessione: sessione.as_deref(), cartelle_lettura: &[], mcp_config: None, timeout_strumenti_s: 300 };
        super::stream(c, &mut on2, &cancel).await.unwrap();
        assert!(secondo.to_lowercase().contains("blu"), "la sessione non è stata ripresa: {secondo}");
    }

    #[test]
    fn readable_errors() {
        assert!(super::errore_leggibile("Invalid API key · Please run /login", None).to_string().contains("/login"));
        assert!(super::errore_leggibile("Claude AI usage limit reached", Some(1_793_146_800)).to_string().contains("si sblocca alle"));
    }
}

//! Sviluppo: lavori sul codice affidati a Claude Code dentro la cartella del progetto,
//! e apertura di file e cartelle nell'editor dell'utente (un'app della famiglia VS Code).

use super::{percorsi, str_arg, Esito};
use crate::App;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_util::sync::CancellationToken;

/// Comandi che Claude Code può eseguire da solo mentre sviluppa (le modifiche ai file sono già consentite).
const CONSENTITI: &[&str] = &[
    "Read", "Glob", "Grep", "Edit", "Write", "MultiEdit", "NotebookEdit", "TodoWrite", "WebFetch", "WebSearch",
    "Bash(npm run:*)", "Bash(npm test:*)", "Bash(npm install:*)", "Bash(npm ci:*)", "Bash(npx:*)", "Bash(pnpm:*)", "Bash(yarn:*)", "Bash(bun:*)",
    "Bash(node:*)", "Bash(tsc:*)", "Bash(cargo:*)", "Bash(go:*)", "Bash(python3:*)", "Bash(pytest:*)", "Bash(dotnet:*)", "Bash(swift:*)", "Bash(xcodebuild:*)",
    "Bash(git status:*)", "Bash(git diff:*)", "Bash(git log:*)", "Bash(git show:*)", "Bash(git add:*)", "Bash(git commit:*)", "Bash(git checkout:*)",
    "Bash(git switch:*)", "Bash(git branch:*)", "Bash(git stash:*)", "Bash(git restore:*)", "Bash(git fetch:*)", "Bash(git pull:*)",
    "Bash(ls:*)", "Bash(mkdir:*)", "Bash(cat:*)", "Bash(head:*)", "Bash(tail:*)", "Bash(wc:*)", "Bash(find:*)", "Bash(grep:*)", "Bash(rg:*)",
];
/// Mai senza una richiesta esplicita dell'utente: si fanno da Oltremare, con approvazione.
const VIETATI: &[&str] = &["Bash(git push:*)", "Bash(sudo:*)", "Bash(rm -rf:*)", "Bash(npm publish:*)", "Bash(cargo publish:*)", "Bash(gh pr merge:*)"];

fn breve(input: &Value) -> String {
    let o = input.as_object();
    let v = o.and_then(|o| ["file_path", "path", "pattern", "command", "url", "description"].iter().find_map(|k| o.get(*k).and_then(Value::as_str)));
    v.map(|x| {
        let x = x.replace(&crate::shell_env::home().map(|h| h.to_string_lossy().into_owned()).unwrap_or_default(), "~");
        if x.chars().count() > 70 { format!("{}…", x.chars().take(70).collect::<String>()) } else { x }
    })
    .unwrap_or_default()
}

async fn git_modifiche(cartella: &Path) -> String {
    let o = tokio::process::Command::new("git").args(["status", "--short"]).current_dir(cartella).env("PATH", crate::shell_env::path_utente()).output().await;
    match o {
        Ok(o) if o.status.success() => {
            let t = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if t.is_empty() { "nessun file modificato rispetto all'ultimo commit".into() } else { t.lines().take(80).collect::<Vec<_>>().join("\n") }
        }
        _ => "non è un repository git".into(),
    }
}

/// Nome e schema degli indirizzi dell'editor (da product.json: "vscode", oppure quello dell'app derivata).
fn info_editor(cli: &Path) -> (String, String) {
    // macOS: <App>.app/Contents/Resources/app/bin/code → product.json in Contents/Resources/app
    let prodotto = cli.parent().and_then(Path::parent).map(|d| d.join("product.json"));
    let v: Value = prodotto.and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(Value::Null);
    (v["nameShort"].as_str().unwrap_or("l'editor").to_string(), v["urlProtocol"].as_str().unwrap_or("vscode").to_string())
}

/// Porta in primo piano la finestra dell'editor con la cartella (macOS: tramite il sistema, che attiva l'app).
async fn mostra_cartella(cli: &Path, cartella: &Path) {
    #[cfg(target_os = "macos")]
    if let Some(app) = cli.ancestors().find(|a| a.extension().is_some_and(|x| x == "app")) {
        let _ = tokio::process::Command::new("open").arg("-a").arg(app).arg(cartella).status().await;
        tokio::time::sleep(Duration::from_millis(1500)).await;
    }
    // La riga di comando mette a fuoco la finestra che ha già quella cartella
    let _ = editor_cmd(cli, &[&cartella.to_string_lossy()]).await;
}

async fn editor_cmd(cli: &Path, args: &[&str]) -> bool {
    let mut c = tokio::process::Command::new(cli);
    c.args(args).env("PATH", crate::shell_env::path_utente()).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    matches!(tokio::time::timeout(Duration::from_secs(20), c.status()).await, Ok(Ok(st)) if st.success())
}

async fn sviluppa(input: &Value, radici: &[PathBuf], cancel: &CancellationToken, app: &AppHandle, run_id: &str) -> Esito {
    let cartella = match percorsi::consentito(str_arg(input, "cartella"), radici) {
        Ok(p) if p.is_dir() => p,
        Ok(p) => return Esito::errore(format!("{} non è una cartella", p.display())),
        Err(e) => return Esito::errore(e),
    };
    let st = app.state::<App>();
    let s = st.store.settings();
    let Some(bin) = crate::providers::claude_code::trova(&s.claude_code_path) else {
        return Esito::errore("Claude Code non è installato su questo computer: serve per sviluppare (claude.com/download).");
    };
    let mut consentiti: Vec<String> = CONSENTITI.iter().map(|x| x.to_string()).collect();
    // Gli strumenti dei server MCP noti (in Claude Code o collegati qui) sono consentiti
    let mut server: Vec<String> = crate::mcp_client::nomi_claude_code();
    server.extend(s.mcp_servers.iter().filter(|x| x.enabled).map(|x| x.name.clone()));
    server.sort();
    server.dedup();
    consentiti.extend(server.iter().map(|n| format!("mcp__{n}")));

    let mut cmd = crate::providers::claude_code::comando(&bin);
    cmd.current_dir(&cartella)
        .arg("-p")
        .arg(str_arg(input, "richiesta"))
        .args(["--output-format", "stream-json", "--verbose", "--permission-mode", "acceptEdits"])
        .arg("--allowedTools")
        .arg(consentiti.join(","))
        .arg("--disallowedTools")
        .arg(VIETATI.join(","))
        .arg("--append-system-prompt")
        .arg("Lavori su richiesta dell'utente tramite il suo assistente Oltremare, senza poter fare domande: decidi in modo ragionevole e dichiara le scelte. Non fare push. Alla fine scrivi un riepilogo breve: cosa hai cambiato, come verificarlo, cosa resta da fare.");
    if !s.claude_code_model.is_empty() && s.claude_code_model != "default" {
        cmd.args(["--model", &s.claude_code_model]);
    }
    let sessione = str_arg(input, "sessione").trim();
    if !sessione.is_empty() {
        cmd.args(["--resume", sessione]);
    }
    let mut figlio = match cmd.spawn() {
        Ok(f) => f,
        Err(e) => return Esito::errore(format!("Non riesco ad avviare Claude Code: {e}")),
    };
    let Some(stdout) = figlio.stdout.take() else { return Esito::errore("Uscita di Claude Code non disponibile") };
    let stderr = figlio.stderr.take();
    let progresso = |t: String| {
        let _ = app.emit("run-progress", json!({ "runId": run_id, "text": t }));
    };
    // Il lavoro si vede nell'editor: si apre il progetto prima di iniziare
    let editor = riga_di_comando(&s.editor_app);
    let mut nome_editor = String::new();
    if let Some(cli) = &editor {
        let (nome, _) = info_editor(cli);
        nome_editor = nome;
        progresso(format!("Apro {} in {nome_editor}", percorsi::mostra(&cartella)));
        mostra_cartella(cli, &cartella).await;
    }
    progresso(format!("Claude Code al lavoro in {}", percorsi::mostra(&cartella)));

    let mut righe = BufReader::new(stdout).lines();
    let mut finale: Option<Value> = None;
    let mut sessione_nuova = String::new();
    let mut passi = 0usize;
    let lettura = async {
        while let Ok(Some(r)) = righe.next_line().await {
            let Ok(v) = serde_json::from_str::<Value>(&r) else { continue };
            match v["type"].as_str() {
                Some("system") if v["subtype"] == "init" => {
                    sessione_nuova = v["session_id"].as_str().unwrap_or_default().to_string();
                }
                Some("assistant") => {
                    for b in v["message"]["content"].as_array().into_iter().flatten() {
                        if b["type"] == "tool_use" {
                            passi += 1;
                            let nome = b["name"].as_str().unwrap_or("strumento").replace("mcp__", "").replace("__", " · ");
                            progresso(format!("Claude Code · {nome} {}", breve(&b["input"])));
                        }
                    }
                }
                Some("result") => {
                    finale = Some(v);
                    break;
                }
                _ => {}
            }
        }
    };
    let esito_tempo = tokio::select! {
        r = tokio::time::timeout(Duration::from_secs(60 * 60), lettura) => r.is_ok(),
        _ = cancel.cancelled() => {
            let _ = figlio.kill().await;
            return Esito::errore("Lavoro interrotto. Le modifiche già fatte restano nei file.");
        }
    };
    if !esito_tempo {
        let _ = figlio.kill().await;
        return Esito::errore("Claude Code ha superato l'ora di lavoro ed è stato fermato. Le modifiche già fatte restano nei file.");
    }
    let _ = tokio::time::timeout(Duration::from_secs(5), figlio.wait()).await;
    let Some(f) = finale else {
        let mut err = String::new();
        if let Some(e) = stderr {
            let mut l = BufReader::new(e).lines();
            while let Ok(Ok(Some(x))) = tokio::time::timeout(Duration::from_millis(300), l.next_line()).await {
                err.push_str(&x);
                err.push('\n');
            }
        }
        return Esito::errore(format!("Claude Code si è fermato senza risultato. {}", err.trim().chars().take(600).collect::<String>()));
    };
    let sid = f["session_id"].as_str().filter(|x| !x.is_empty()).unwrap_or(&sessione_nuova).to_string();
    let risposta = f["result"].as_str().unwrap_or_default().trim().to_string();
    let modifiche = git_modifiche(&cartella).await;
    // Alla fine la conversazione di Claude Code si apre nel pannello dell'editor, per leggerla e continuarla lì
    let mut nell_editor = String::new();
    if let (Some(cli), false) = (&editor, sid.is_empty()) {
        let (_, schema) = info_editor(cli);
        // Il collegamento va alla finestra attiva dell'editor: prima si porta avanti quella del progetto
        mostra_cartella(cli, &cartella).await;
        tokio::time::sleep(Duration::from_millis(800)).await;
        let url = format!("{schema}://anthropic.claude-code/open?session={sid}");
        // macOS: l'indirizzo passa dal sistema all'app che ha registrato lo schema; altrove dalla riga di comando
        let aperto = if cfg!(target_os = "macos") {
            matches!(tokio::process::Command::new("open").arg(&url).status().await, Ok(st) if st.success())
        } else {
            editor_cmd(cli, &["--open-url", &url]).await
        };
        if aperto {
            nell_editor = format!("\nLa conversazione di Claude Code è aperta in {nome_editor}, nel pannello di Claude: l'utente può leggerla e continuarla lì.");
        }
    }
    let costo = f["total_cost_usd"].as_f64().map(|c| format!(", costo equivalente {c:.2} $")).unwrap_or_default();
    let testo = format!(
        "Claude Code ha {} in {} ({} azioni{costo}).\nSessione: {sid} (passala come «sessione» per continuare questo lavoro){nell_editor}\n\nRiepilogo di Claude Code:\n{}\n\nStato dei file (git):\n{modifiche}",
        if f["is_error"].as_bool() == Some(true) { "terminato con un errore" } else { "finito" },
        percorsi::mostra(&cartella),
        f["num_turns"].as_u64().unwrap_or(passi as u64),
        if risposta.is_empty() { "(nessun riepilogo)" } else { &risposta },
    );
    let vista = json!({ "kind": "text", "text": format!("{}\n\n{}", if risposta.is_empty() { "Fatto." } else { &risposta }, modifiche).chars().take(6000).collect::<String>() });
    if f["is_error"].as_bool() == Some(true) {
        let mut e = Esito::errore(testo);
        e.risultato = vista;
        e
    } else {
        Esito::ok(testo, vista)
    }
}

/// Editor installati della famiglia VS Code (hanno la riga di comando in bin/code), i più usati prima.
pub fn editor_installati() -> Vec<(String, String)> {
    #[cfg(target_os = "macos")]
    {
        let mut candidati: Vec<PathBuf> = vec![];
        let mut cartelle = vec![PathBuf::from("/Applications")];
        if let Some(h) = crate::shell_env::home() {
            cartelle.push(h.join("Applications"));
        }
        for c in cartelle {
            if let Ok(rd) = std::fs::read_dir(c) {
                candidati.extend(rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "app")));
            }
        }
        // Spotlight trova anche le app tenute fuori da Applicazioni
        if let Ok(o) = std::process::Command::new("mdfind").arg("kMDItemContentType == 'com.apple.application-bundle' && kMDItemFSName == '*Code*'").output() {
            candidati.extend(String::from_utf8_lossy(&o.stdout).lines().map(PathBuf::from));
        }
        candidati.sort();
        candidati.dedup();
        let mut trovati: Vec<(String, String, String)> = candidati
            .into_iter()
            .filter(|p| p.join("Contents/Resources/app/bin/code").is_file() && !p.to_string_lossy().contains("/.Trash/"))
            .map(|p| {
                let usato = std::process::Command::new("mdls").args(["-raw", "-name", "kMDItemLastUsedDate"]).arg(&p).output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
                let nome = p.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                (nome, p.to_string_lossy().into_owned(), if usato.starts_with('(') { String::new() } else { usato })
            })
            .collect();
        trovati.sort_by(|a, b| b.2.cmp(&a.2));
        trovati.into_iter().map(|(n, p, _)| (n, p)).collect()
    }
    #[cfg(not(target_os = "macos"))]
    {
        let sep = if cfg!(windows) { ';' } else { ':' };
        let nome = if cfg!(windows) { "code.cmd" } else { "code" };
        crate::shell_env::path_utente().split(sep).map(|d| Path::new(d).join(nome)).filter(|p| p.is_file()).map(|p| ("Code".to_string(), p.to_string_lossy().into_owned())).take(1).collect()
    }
}

/// Riga di comando dell'editor scelto (o del più usato).
fn riga_di_comando(scelto: &str) -> Option<PathBuf> {
    let p = if scelto.trim().is_empty() { editor_installati().into_iter().next().map(|(_, p)| p)? } else { scelto.trim().to_string() };
    let p = PathBuf::from(p);
    if p.extension().is_some_and(|x| x == "app") {
        let cli = p.join("Contents/Resources/app/bin/code");
        return cli.is_file().then_some(cli);
    }
    p.is_file().then_some(p)
}

async fn apri_in_editor(input: &Value, radici: &[PathBuf], app: &AppHandle) -> Esito {
    let percorso = match percorsi::consentito(str_arg(input, "percorso"), radici) {
        Ok(p) => p,
        Err(e) => return Esito::errore(e),
    };
    let s = app.state::<App>().store.settings();
    let Some(cli) = riga_di_comando(&s.editor_app) else {
        return Esito::errore("Nessun editor di codice trovato (VS Code o simili). Sceglilo in Impostazioni → Strumenti esterni.");
    };
    let mut cmd = tokio::process::Command::new(&cli);
    cmd.env("PATH", crate::shell_env::path_utente()).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::piped());
    match input["riga"].as_u64().filter(|_| percorso.is_file()) {
        Some(r) => cmd.arg("-g").arg(format!("{}:{r}", percorso.display())),
        None => cmd.arg(&percorso),
    };
    match tokio::time::timeout(Duration::from_secs(20), cmd.output()).await {
        Ok(Ok(o)) if o.status.success() => {
            let t = format!("Aperto {} nell'editor", percorsi::mostra(&percorso));
            Esito::ok(t.clone(), json!({ "kind": "text", "text": t }))
        }
        Ok(Ok(o)) => Esito::errore(format!("L'editor non si è aperto: {}", String::from_utf8_lossy(&o.stderr).trim())),
        Ok(Err(e)) => Esito::errore(e.to_string()),
        Err(_) => Esito::ok("Editor in apertura", json!({ "kind": "text", "text": "Editor in apertura" })),
    }
}

pub async fn esegui(nome: &str, input: &Value, radici: &[PathBuf], cancel: &CancellationToken, app: &AppHandle, run_id: &str) -> Esito {
    match nome {
        "sviluppa" => sviluppa(input, radici, cancel, app, run_id).await,
        "apri_in_editor" => apri_in_editor(input, radici, app).await,
        _ => Esito::errore(format!("Strumento sconosciuto: {nome}")),
    }
}

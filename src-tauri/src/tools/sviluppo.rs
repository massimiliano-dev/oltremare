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
    // Sotto-agenti: lavorano in parallelo nella stessa sessione, l'agente principale coordina e verifica
    "Agent", "Task", "Skill",
    "Bash(npm run:*)", "Bash(npm test:*)", "Bash(npm install:*)", "Bash(npm ci:*)", "Bash(npx:*)", "Bash(pnpm:*)", "Bash(yarn:*)", "Bash(bun:*)",
    "Bash(node:*)", "Bash(tsc:*)", "Bash(cargo:*)", "Bash(go:*)", "Bash(python3:*)", "Bash(pytest:*)", "Bash(dotnet:*)", "Bash(swift:*)", "Bash(xcodebuild:*)",
    "Bash(git status:*)", "Bash(git diff:*)", "Bash(git log:*)", "Bash(git show:*)", "Bash(git add:*)", "Bash(git commit:*)", "Bash(git checkout:*)",
    "Bash(git switch:*)", "Bash(git branch:*)", "Bash(git stash:*)", "Bash(git restore:*)", "Bash(git fetch:*)", "Bash(git pull:*)",
    "Bash(ls:*)", "Bash(mkdir:*)", "Bash(cat:*)", "Bash(head:*)", "Bash(tail:*)", "Bash(wc:*)", "Bash(find:*)", "Bash(grep:*)", "Bash(rg:*)",
];
/// Mai senza una richiesta esplicita dell'utente: si fanno da Oltremare, con approvazione.
const VIETATI: &[&str] = &["Bash(git push:*)", "Bash(sudo:*)", "Bash(rm -rf:*)", "Bash(npm publish:*)", "Bash(cargo publish:*)", "Bash(gh pr merge:*)"];

pub(crate) fn titolo_breve(input: &Value) -> String {
    breve(input)
}

fn breve(input: &Value) -> String {
    let o = input.as_object();
    let v = o.and_then(|o| ["file_path", "path", "pattern", "command", "url", "description"].iter().find_map(|k| o.get(*k).and_then(Value::as_str)));
    v.map(|x| {
        let x = x.lines().next().unwrap_or_default();
        let x = x.replace(&crate::shell_env::home().map(|h| h.to_string_lossy().into_owned()).unwrap_or_default(), "~");
        if x.chars().count() > 70 { format!("{}…", x.chars().take(70).collect::<String>()) } else { x }
    })
    .unwrap_or_default()
}

/// Le sessioni di revisione (CTO, altre AI) iniziano con questa marca: non sono conversazioni da riprendere.
pub const MARCA_REVISIONE: &str = "[Revisione]";

/// Il primo messaggio dell'utente in una sessione di Claude Code (è il titolo della conversazione).
pub fn primo_messaggio(file: &Path) -> String {
    std::fs::read_to_string(file)
        .ok()
        .and_then(|t| t.lines().find_map(|l| serde_json::from_str::<Value>(l).ok().filter(|v| v["type"] == "user" && v["isMeta"].as_bool() != Some(true)).map(|v| testo_contenuto(&v["message"]["content"])).filter(|t| !t.trim().is_empty() && !t.starts_with('<'))))
        .unwrap_or_default()
}

pub fn e_revisione(file: &Path) -> bool {
    primo_messaggio(file).starts_with(MARCA_REVISIONE)
}

/// L'ultima conversazione di lavoro del progetto (le revisioni non contano).
pub fn ultima_conversazione(cartella: &Path) -> Option<String> {
    sessioni(cartella).into_iter().map(|(f, _)| f).find(|f| !e_revisione(f)).and_then(|f| f.file_stem().map(|s| s.to_string_lossy().into_owned()))
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

/// Claude Code senza interfaccia, nella cartella del progetto, con le impostazioni e i server MCP dell'utente.
/// Come lavora Claude Code in background: modello, modalità (piano o modifiche) e, se scelta, un'altra AI via Claudex.
#[derive(Clone, Default)]
pub struct Opzioni {
    /// Alias o nome del modello per --model (vuoto = quello delle impostazioni)
    pub modello: String,
    /// "plan" (analizza e propone, non modifica) oppure "acceptEdits"
    pub modo: String,
    /// Ambiente per usare un altro provider tramite il proxy di Claudex
    pub ambiente: Vec<(String, String)>,
    /// Istruzioni in più per questa esecuzione
    pub istruzioni: String,
    /// Cartelle fuori dal progetto in cui Claude può scrivere (es. le note dei progetti nella memoria)
    pub cartelle: Vec<PathBuf>,
    /// Solo lettura (revisioni, pareri): niente modifiche ai file né comandi che cambiano lo stato
    pub solo_lettura: bool,
}

/// Strumenti per chi deve solo leggere e verificare (il CTO che rivede, un secondo parere)
const SOLO_LETTURA: &[&str] = &[
    "Read", "Glob", "Grep", "WebFetch", "WebSearch", "TodoWrite", "Agent", "Task", "Skill",
    "Bash(git status:*)", "Bash(git diff:*)", "Bash(git log:*)", "Bash(git show:*)",
    "Bash(npm test:*)", "Bash(npm run test:*)", "Bash(npm run lint:*)", "Bash(npm run check:*)", "Bash(npm run build:*)", "Bash(npx vitest:*)", "Bash(npx jest:*)", "Bash(npx tsc:*)", "Bash(npx eslint:*)", "Bash(npx svelte-check:*)",
    "Bash(cargo test:*)", "Bash(cargo check:*)", "Bash(cargo clippy:*)", "Bash(go test:*)", "Bash(go vet:*)", "Bash(pytest:*)", "Bash(python3 -m pytest:*)", "Bash(dotnet test:*)", "Bash(swift test:*)",
    "Bash(ls:*)", "Bash(cat:*)", "Bash(head:*)", "Bash(tail:*)", "Bash(wc:*)", "Bash(find:*)", "Bash(grep:*)", "Bash(rg:*)",
];

fn comando_headless(bin: &Path, cartella: &Path, messaggio: &str, sessione: Option<&str>, s: &crate::store::Settings) -> tokio::process::Command {
    comando_headless_con(bin, cartella, messaggio, sessione, s, &Opzioni::default())
}

pub(crate) fn comando_headless_con(bin: &Path, cartella: &Path, messaggio: &str, sessione: Option<&str>, s: &crate::store::Settings, o: &Opzioni) -> tokio::process::Command {
    let mut consentiti: Vec<String> = if o.solo_lettura { SOLO_LETTURA } else { CONSENTITI }.iter().map(|x| x.to_string()).collect();
    let mut vietati: Vec<String> = VIETATI.iter().map(|x| x.to_string()).collect();
    if o.solo_lettura {
        vietati.extend(["Edit", "Write", "MultiEdit", "NotebookEdit"].map(String::from));
    }
    // Gli strumenti dei server MCP noti (in Claude Code o collegati qui) sono consentiti
    let mut server: Vec<String> = crate::mcp_client::nomi_claude_code();
    server.extend(s.mcp_servers.iter().filter(|x| x.enabled).map(|x| x.name.clone()));
    server.sort();
    server.dedup();
    consentiti.extend(server.iter().map(|n| format!("mcp__{n}")));
    // Claude in Chrome: il browser vero dell'utente, visibile, per provare l'app e navigare
    consentiti.push("mcp__claude-in-chrome".into());
    let mut cmd = crate::providers::claude_code::comando(bin);
    cmd.current_dir(cartella)
        .arg("-p")
        .arg(messaggio)
        .args(["--output-format", "stream-json", "--verbose", "--chrome", "--permission-mode", if o.solo_lettura { "default" } else if o.modo.is_empty() { "acceptEdits" } else { &o.modo }])
        .arg("--allowedTools")
        .arg(consentiti.join(","))
        .arg("--disallowedTools")
        .arg(vietati.join(","))
        .arg("--append-system-prompt")
        .arg(format!(
            "Lavori su richiesta dell'utente tramite il suo assistente Oltremare, senza poter fare domande: decidi in modo ragionevole e dichiara le scelte. Non fare push. Alla fine scrivi un riepilogo breve: cosa hai cambiato, come verificarlo, cosa resta da fare.\nPer vedere e provare l'app (o navigare un sito) usa Claude in Chrome (strumenti mcp__claude-in-chrome__*): è il Chrome dell'utente, che ti guarda lavorare. Crea una scheda nuova e lavora lì, senza toccare le schede già aperte; alla fine lasciala sull'ultima pagina utile.{}",
            if o.istruzioni.is_empty() { String::new() } else { format!("\n{}", o.istruzioni) }
        ));
    let modello = if !o.modello.is_empty() { o.modello.clone() } else { s.claude_code_model.clone() };
    if !modello.is_empty() && modello != "default" {
        cmd.args(["--model", &modello]);
    }
    for (k, v) in &o.ambiente {
        cmd.env(k, v);
    }
    for c in &o.cartelle {
        cmd.arg("--add-dir").arg(c);
    }
    if let Some(sid) = sessione {
        cmd.args(["--resume", sid]);
    }
    cmd
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
    let mut cmd = comando_headless(&bin, &cartella, str_arg(input, "richiesta"), Some(str_arg(input, "sessione").trim()).filter(|x| !x.is_empty()), &s);
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
        "apri_studio" => {
            let cartella = Some(str_arg(input, "cartella").trim()).filter(|x| !x.is_empty()).map(|c| percorsi::espandi(c).to_string_lossy().into_owned());
            let richiesta = Some(str_arg(input, "richiesta").trim()).filter(|x| !x.is_empty());
            match crate::studio::apri(app, cartella.as_deref(), richiesta) {
                Ok(()) => Esito::ok("Studio aperto: l'utente continua lì.", json!({ "kind": "text", "text": "Studio aperto" })),
                Err(e) => Esito::errore(e.to_string()),
            }
        }
        "editor_claude_scrivi" => scrivi_a_claude(input, radici, app).await,
        "editor_claude_stato" => claude_stato(input, radici, cancel, app, run_id).await,
        "avvia_processo" | "processi" | "log_processo" | "ferma_processo" => super::processi::esegui(nome, input, radici).await,
        _ => Esito::errore(format!("Strumento sconosciuto: {nome}")),
    }
}

// ---------- Claude nel pannello dell'editor: scrivere come l'utente, seguire il lavoro ----------

fn codifica_url(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Cartella in cui Claude Code salva le conversazioni di un progetto (~/.claude/projects/<percorso con i trattini>).
fn cartella_sessioni(progetto: &Path) -> Option<PathBuf> {
    let base = crate::shell_env::home()?.join(".claude/projects");
    let slug = |p: &Path| p.to_string_lossy().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect::<String>();
    let mut candidati = vec![base.join(slug(progetto))];
    if let Ok(c) = progetto.canonicalize() {
        candidati.push(base.join(slug(&c)));
    }
    candidati.into_iter().find(|d| d.is_dir()).or_else(|| Some(base.join(slug(progetto))))
}

/// Conversazioni del progetto, la più recente prima.
pub(crate) fn sessioni(progetto: &Path) -> Vec<(PathBuf, std::time::SystemTime)> {
    let Some(dir) = cartella_sessioni(progetto) else { return vec![] };
    let mut v: Vec<(PathBuf, std::time::SystemTime)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "jsonl"))
        .filter_map(|e| Some((e.path(), e.metadata().ok()?.modified().ok()?)))
        .collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    v
}

#[derive(Default)]
struct StatoSessione {
    id: String,
    ultima_richiesta: String,
    ultima_risposta: String,
    azioni: Vec<String>,
    in_sospeso: Vec<String>,
    finito: bool,
    fermo_da: u64,
    inattivo_da: u64,
    domanda: bool,
}

fn testo_contenuto(c: &Value) -> String {
    match c {
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().filter(|b| b["type"] == "text").filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("\n"),
        _ => String::new(),
    }
}

fn leggi_sessione(file: &Path) -> StatoSessione {
    let mut st = StatoSessione { id: file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(), ..Default::default() };
    let Ok(t) = std::fs::read_to_string(file) else { return st };
    let mut sospesi: Vec<(String, String)> = vec![];
    let mut ultimo_assistente_fine = false;
    let mut ultimo_tipo = String::new();
    for riga in t.lines() {
        let Ok(v) = serde_json::from_str::<Value>(riga) else { continue };
        match v["type"].as_str() {
            Some("user") => {
                let c = &v["message"]["content"];
                if let Some(a) = c.as_array() {
                    for b in a.iter().filter(|b| b["type"] == "tool_result") {
                        let id = b["tool_use_id"].as_str().unwrap_or_default();
                        sospesi.retain(|(x, _)| x != id);
                    }
                }
                let testo = testo_contenuto(c);
                if !testo.trim().is_empty() && !testo.starts_with("<") && v["isMeta"].as_bool() != Some(true) {
                    st.ultima_richiesta = testo;
                    st.ultima_risposta.clear();
                    ultimo_assistente_fine = false;
                }
                ultimo_tipo = "user".into();
            }
            Some("assistant") => {
                for b in v["message"]["content"].as_array().into_iter().flatten() {
                    match b["type"].as_str() {
                        Some("text") => st.ultima_risposta = b["text"].as_str().unwrap_or_default().to_string(),
                        Some("tool_use") => {
                            let nome = b["name"].as_str().unwrap_or("strumento").replace("mcp__", "").replace("__", " · ");
                            let descr = format!("{nome} {}", breve(&b["input"])).trim().to_string();
                            sospesi.push((b["id"].as_str().unwrap_or_default().to_string(), descr.clone()));
                            st.azioni.push(descr);
                        }
                        _ => {}
                    }
                }
                ultimo_assistente_fine = v["message"]["stop_reason"].as_str() == Some("end_turn");
                ultimo_tipo = "assistant".into();
            }
            _ => {}
        }
    }
    let inattivo = std::fs::metadata(file).and_then(|m| m.modified()).ok().and_then(|m| m.elapsed().ok()).map(|d| d.as_secs()).unwrap_or(0);
    st.inattivo_da = inattivo;
    st.in_sospeso = sospesi.into_iter().map(|(_, d)| d).collect();
    st.finito = st.in_sospeso.is_empty() && ultimo_tipo == "assistant" && (ultimo_assistente_fine || inattivo >= 6);
    if !st.in_sospeso.is_empty() {
        st.fermo_da = inattivo;
    }
    st.domanda = st.finito && st.ultima_risposta.trim_end().ends_with('?');
    st
}

fn descrivi(st: &StatoSessione) -> String {
    let stato = if st.finito {
        if st.domanda { "Claude ha finito e ti fa una domanda: rispondigli con «editor_claude_scrivi».".to_string() } else { "Claude ha finito il turno.".to_string() }
    } else if !st.in_sospeso.is_empty() && st.fermo_da >= 20 {
        format!(
            "Claude è fermo da {} s su «{}»: probabilmente aspetta un permesso nel pannello. Guarda lo schermo (schermo_screenshot) e rispondi come farebbe l'utente (per esempio «Sì» o «Sì, e non chiedere più» per comandi di sviluppo innocui), oppure il comando è lungo.",
            st.fermo_da,
            st.in_sospeso.last().cloned().unwrap_or_default()
        )
    } else if !st.in_sospeso.is_empty() {
        format!("Claude sta eseguendo «{}».", st.in_sospeso.last().cloned().unwrap_or_default())
    } else {
        "Claude sta lavorando.".to_string()
    };
    let azioni = st.azioni.iter().rev().take(12).rev().map(|a| format!("- {a}")).collect::<Vec<_>>().join("\n");
    format!(
        "Sessione {}\n{stato}\n\nUltima richiesta:\n{}\n\nAzioni recenti di Claude:\n{}\n\nUltima risposta di Claude:\n{}",
        st.id,
        st.ultima_richiesta.chars().take(600).collect::<String>(),
        if azioni.is_empty() { "(nessuna)".into() } else { azioni },
        if st.ultima_risposta.is_empty() { "(ancora nessuna)".into() } else { st.ultima_risposta.chars().take(4000).collect::<String>() }
    )
}

#[cfg(target_os = "macos")]
fn premi_invio() -> Result<(), String> {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};
    if !crate::permessi::stato().accessibility {
        return Err("serve il permesso Accessibilità per premere Invio".into());
    }
    let mut e = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    e.key(Key::Return, Direction::Click).map_err(|e| e.to_string())
}

#[cfg(not(target_os = "macos"))]
fn premi_invio() -> Result<(), String> {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};
    let mut e = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    e.key(Key::Return, Direction::Click).map_err(|e| e.to_string())
}

async fn claude_scrivi(input: &Value, radici: &[PathBuf], app: &AppHandle) -> Esito {
    let cartella = match percorsi::consentito(str_arg(input, "cartella"), radici) {
        Ok(p) if p.is_dir() => p,
        Ok(p) => return Esito::errore(format!("{} non è una cartella", p.display())),
        Err(e) => return Esito::errore(e),
    };
    let messaggio = str_arg(input, "messaggio").trim().to_string();
    if messaggio.is_empty() {
        return Esito::errore("Il messaggio è vuoto");
    }
    if messaggio.chars().count() > 6000 {
        return Esito::errore("Messaggio troppo lungo per il pannello (massimo 6000 caratteri): scrivi i dettagli in un file del progetto e chiedi a Claude di leggerlo.");
    }
    let s = app.state::<App>().store.settings();
    let Some(cli) = riga_di_comando(&s.editor_app) else {
        return Esito::errore("Nessun editor di codice trovato: sceglilo in Impostazioni → Strumenti esterni.");
    };
    let (nome_editor, schema) = info_editor(&cli);
    let nuova = input["nuova_conversazione"].as_bool().unwrap_or(false);
    let sessione = if nuova { None } else { Some(str_arg(input, "sessione").trim().to_string()).filter(|x| !x.is_empty()).or_else(|| sessioni(&cartella).first().and_then(|(p, _)| p.file_stem().map(|x| x.to_string_lossy().into_owned()))) };

    // 1. Il progetto in primo piano nell'editor
    mostra_cartella(&cli, &cartella).await;
    // 2. Il pannello di Claude con il messaggio già scritto (come se l'utente l'avesse digitato)
    let mut url = format!("{schema}://anthropic.claude-code/open?prompt={}", codifica_url(&messaggio));
    if let Some(sid) = &sessione {
        url.push_str(&format!("&session={sid}"));
    }
    let aperto = if cfg!(target_os = "macos") {
        matches!(tokio::process::Command::new("open").arg(&url).status().await, Ok(st) if st.success())
    } else {
        editor_cmd(&cli, &["--open-url", &url]).await
    };
    if !aperto {
        return Esito::errore(format!("Non riesco ad aprire il pannello di Claude in {nome_editor}"));
    }
    tokio::time::sleep(Duration::from_millis(2500)).await;
    // 3. Invio, con l'editor davanti
    #[cfg(target_os = "macos")]
    if let Some(app_editor) = cli.ancestors().find(|a| a.extension().is_some_and(|x| x == "app")) {
        let _ = tokio::process::Command::new("open").arg("-a").arg(app_editor).status().await;
        tokio::time::sleep(Duration::from_millis(600)).await;
    }
    let t0 = std::time::SystemTime::now() - Duration::from_secs(2);
    if let Err(e) = tokio::task::spawn_blocking(premi_invio).await.unwrap_or_else(|e| Err(e.to_string())) {
        return Esito::errore(format!("Il messaggio è scritto nel pannello di Claude in {nome_editor}, ma non posso inviarlo: {e}. Chiedi all'utente di premere Invio."));
    }
    // 4. Si controlla che Claude l'abbia ricevuto (la conversazione su disco)
    // L'inizio del messaggio così come compare nel file della conversazione (JSON)
    let prima_riga: String = messaggio.lines().next().unwrap_or_default().chars().take(40).collect();
    let inizio_msg = serde_json::to_string(&prima_riga).unwrap_or_default().trim_matches('"').to_string();
    let mut trovata: Option<String> = None;
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(600)).await;
        for (p, m) in sessioni(&cartella).into_iter().take(5) {
            if m < t0 {
                continue;
            }
            if std::fs::read_to_string(&p).is_ok_and(|t| t.lines().rev().take(60).any(|l| l.contains("\"type\":\"user\"") && l.contains(&inizio_msg))) {
                trovata = p.file_stem().map(|x| x.to_string_lossy().into_owned());
                break;
            }
        }
        if trovata.is_some() {
            break;
        }
    }
    match trovata {
        Some(sid) => {
            let t = format!("Messaggio inviato a Claude nel pannello di {nome_editor} (sessione {sid}). Claude sta lavorando: segui con «editor_claude_stato» (con attendi_secondi per aspettare la fine).");
            Esito::ok(t.clone(), json!({ "kind": "text", "text": t }))
        }
        None => Esito::errore(format!(
            "Ho scritto il messaggio nel pannello di Claude in {nome_editor} e premuto Invio, ma non risulta arrivato. Guarda lo schermo (schermo_screenshot): forse il pannello chiede qualcosa (accesso, fiducia nella cartella) o il cursore non era nel campo del messaggio. Clicca nel campo e premi Invio, o rispondi alla richiesta."
        )),
    }
}

async fn claude_stato(input: &Value, radici: &[PathBuf], cancel: &CancellationToken, app: &AppHandle, run_id: &str) -> Esito {
    let cartella = match percorsi::consentito(str_arg(input, "cartella"), radici) {
        Ok(p) => p,
        Err(e) => return Esito::errore(e),
    };
    let scelta = str_arg(input, "sessione").trim().to_string();
    let file = || -> Option<PathBuf> {
        let l = sessioni(&cartella);
        if scelta.is_empty() { l.first().map(|(p, _)| p.clone()) } else { l.into_iter().map(|(p, _)| p).find(|p| p.file_stem().is_some_and(|s| s.to_string_lossy() == scelta)) }
    };
    let Some(mut f) = file() else {
        return Esito::errore("Nessuna conversazione di Claude per questo progetto: inizia con «editor_claude_scrivi».");
    };
    let attesa = input["attendi_secondi"].as_u64().unwrap_or(0).min(1200);
    let inizio = std::time::Instant::now();
    crate::dev::segnala_progetto(app, &cartella);
    // In modalità sviluppo di Oltremare conta il processo: finché è vivo Claude sta lavorando
    let correggi = |st: &mut StatoSessione| match headless_attivo(&cartella) {
        Some(true) => {
            st.finito = false;
            st.fermo_da = 0;
        }
        Some(false) if st.in_sospeso.is_empty() => st.finito = true,
        _ => {}
    };
    let mut st = leggi_sessione(&f);
    correggi(&mut st);
    let mut ultima_azione = st.azioni.len();
    while attesa > 0 && inizio.elapsed() < Duration::from_secs(attesa) && !st.finito && !(st.fermo_da >= 25) {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(3)) => {}
            _ = cancel.cancelled() => return Esito::errore("Interrotto"),
        }
        if let Some(n) = file() {
            f = n;
        }
        st = leggi_sessione(&f);
        correggi(&mut st);
        if st.azioni.len() != ultima_azione {
            ultima_azione = st.azioni.len();
            if let Some(a) = st.azioni.last() {
                let _ = app.emit("run-progress", json!({ "runId": run_id, "text": format!("Claude nell'editor · {a}") }));
            }
        }
    }
    let t = descrivi(&st);
    let errore = !st.finito && st.fermo_da >= 25;
    let vista = json!({ "kind": "text", "text": t.chars().take(5000).collect::<String>() });
    if errore {
        let mut e = Esito::errore(t);
        e.risultato = vista;
        e
    } else {
        Esito::ok(t, vista)
    }
}

#[cfg(test)]
mod prova_sessioni {
    #[test]
    #[ignore]
    fn legge_l_ultima_sessione_reale() {
        let p = std::path::PathBuf::from(std::env::var("PROGETTO").unwrap_or_else(|_| ".".into()));
        let l = super::sessioni(&p);
        assert!(!l.is_empty(), "nessuna sessione per {}", p.display());
        let st = super::leggi_sessione(&l[0].0);
        println!("{}", super::descrivi(&st).chars().take(1500).collect::<String>());
        println!("finito={} sospesi={:?} inattivo={}", st.finito, st.in_sospeso, st.inattivo_da);
    }
}


// ---------- Modalità sviluppo di Oltremare: Claude Code in background, seguito nella vista «Sviluppo» ----------

struct Headless {
    figlio: std::sync::Arc<tokio::sync::Mutex<tokio::process::Child>>,
    pid: Option<u32>,
    sessione: std::sync::Arc<std::sync::Mutex<String>>,
    finito: std::sync::Arc<std::sync::Mutex<Option<(bool, String)>>>,
}

static HEADLESS: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<PathBuf, Headless>>> = std::sync::LazyLock::new(Default::default);

/// Claude in Chrome si collega solo se Chrome è aperto: se è chiuso lo si apre in background (senza rubare il fuoco).
async fn apri_chrome() {
    #[cfg(target_os = "macos")]
    {
        let aperto = tokio::process::Command::new("pgrep").args(["-x", "Google Chrome"]).output().await.is_ok_and(|o| o.status.success());
        if !aperto && Path::new("/Applications/Google Chrome.app").exists() {
            let _ = tokio::process::Command::new("open").args(["-ga", "Google Chrome"]).status().await;
        }
    }
}

/// Lavoro avviato da Oltremare sul progetto: (in corso, sessione).
fn headless_sessione(cartella: &Path) -> Option<(bool, String)> {
    let m = HEADLESS.lock().unwrap_or_else(|e| e.into_inner());
    m.get(cartella).map(|h| (h.finito.lock().map(|f| f.is_none()).unwrap_or(false), h.sessione.lock().map(|x| x.clone()).unwrap_or_default()))
}

/// Claude Code in background sta lavorando su questo progetto? (Some(true) in corso, Some(false) appena finito)
pub(crate) fn headless_attivo(cartella: &Path) -> Option<bool> {
    let m = HEADLESS.lock().unwrap_or_else(|e| e.into_inner());
    m.get(cartella).map(|h| h.finito.lock().map(|f| f.is_none()).unwrap_or(false))
}

/// Ferma Claude Code in background sul progetto. Il lavoro resta registrato come finito (fermato), così
/// la vista smette subito di mostrarlo «in corso» invece di aspettare che la sessione resti ferma per due minuti.
pub async fn ferma_headless(cartella: &Path) -> bool {
    let reale = cartella.canonicalize().unwrap_or_else(|_| cartella.to_path_buf());
    let trovato = {
        let m = HEADLESS.lock().unwrap_or_else(|e| e.into_inner());
        m.iter()
            .find(|(k, _)| k.as_path() == cartella || k.canonicalize().is_ok_and(|k| k == reale))
            .map(|(_, h)| (h.figlio.clone(), h.pid, h.finito.clone()))
    };
    let Some((figlio, pid, finito)) = trovato else { return false };
    {
        let mut f = finito.lock().unwrap_or_else(|e| e.into_inner());
        if f.is_none() {
            *f = Some((true, "Fermato dall'utente".into()));
        }
    }
    let mut f = figlio.lock().await;
    super::shell::uccidi(&mut f, pid).await;
    true
}

pub fn ferma_tutti_headless() {
    for (_, h) in HEADLESS.lock().unwrap_or_else(|e| e.into_inner()).drain() {
        #[cfg(unix)]
        if let Some(pid) = h.pid {
            unsafe {
                libc::kill(-(pid as i32), libc::SIGKILL);
            }
        }
        if let Ok(mut f) = h.figlio.try_lock() {
            let _ = f.start_kill();
        }
    }
}

async fn avvia_headless(app: &AppHandle, cartella: &Path, messaggio: &str, sessione: Option<String>) -> Result<String, String> {
    avvia_headless_con(app, cartella, messaggio, sessione, Opzioni::default()).await
}

pub async fn avvia_headless_con(app: &AppHandle, cartella: &Path, messaggio: &str, sessione: Option<String>, o: Opzioni) -> Result<String, String> {
    if headless_attivo(cartella) == Some(true) {
        return Err("Claude sta già lavorando su questo progetto: aspetta che finisca o fermalo.".into());
    }
    let s = app.state::<App>().store.settings();
    let bin = crate::providers::claude_code::trova(&s.claude_code_path).ok_or("Claude Code non è installato su questo computer (claude.com/download)")?;
    crate::squadra::installa_agenti();
    let mut cmd = comando_headless_con(&bin, cartella, messaggio, sessione.as_deref(), &s, &o);
    #[cfg(unix)]
    cmd.process_group(0);
    apri_chrome().await;
    let mut figlio = cmd.spawn().map_err(|e| format!("Non riesco ad avviare Claude Code: {e}"))?;
    let pid = figlio.id();
    let stdout = figlio.stdout.take().ok_or("uscita non disponibile")?;
    let sid = std::sync::Arc::new(std::sync::Mutex::new(sessione.clone().unwrap_or_default()));
    // Per le statistiche di utilizzo: chi ha lavorato (abbonamento Claude o un'altra AI via Claudex) e su cosa
    let inizio = crate::store::now_ms();
    let provider = if o.ambiente.iter().any(|(k, _)| k == "ANTHROPIC_BASE_URL") { "claudex" } else { "claude-code" };
    let titolo: String = messaggio.chars().take(80).collect();
    // Il lavoro fatto (non i piani) finisce nel diario del progetto in memoria
    let memoria = (o.modo != "plan" && !o.solo_lettura).then(|| crate::memoria::cartella(&s)).flatten();
    let richiesta = messaggio.to_string();
    let finito: std::sync::Arc<std::sync::Mutex<Option<(bool, String)>>> = Default::default();
    let (tx, rx) = tokio::sync::oneshot::channel::<String>();
    {
        let (sid, finito, app, memoria, richiesta) = (sid.clone(), finito.clone(), app.clone(), memoria.clone(), richiesta.clone());
        let cartella = cartella.to_path_buf();
        tokio::spawn(async move {
            let mut tx = Some(tx);
            let mut righe = BufReader::new(stdout).lines();
            while let Ok(Some(r)) = righe.next_line().await {
                let Ok(v) = serde_json::from_str::<Value>(&r) else { continue };
                if let Some(id) = v["session_id"].as_str().filter(|x| !x.is_empty()) {
                    *sid.lock().unwrap_or_else(|e| e.into_inner()) = id.to_string();
                    if let Some(t) = tx.take() {
                        let _ = t.send(id.to_string());
                    }
                }
                if v["type"] == "rate_limit_event" && provider == "claude-code" {
                    let mut info = v["rate_limit_info"].clone();
                    info["at"] = json!(crate::store::now_ms());
                    let _ = app.state::<App>().store.set_pref("abbonamento:limiti", info);
                }
                if v["type"] == "result" {
                    *finito.lock().unwrap_or_else(|e| e.into_inner()) = Some((v["is_error"].as_bool() == Some(true), v["result"].as_str().unwrap_or_default().to_string()));
                    let _ = app.emit("dev-session", json!({ "folder": cartella.to_string_lossy(), "done": true }));
                    // Consumi del lavoro, come per gli agenti
                    let u = &v["usage"];
                    let usage = crate::providers::Usage {
                        input_tokens: u["input_tokens"].as_u64().unwrap_or(0),
                        output_tokens: u["output_tokens"].as_u64().unwrap_or(0),
                        cache_read: u["cache_read_input_tokens"].as_u64().unwrap_or(0),
                        cache_write: u["cache_creation_input_tokens"].as_u64().unwrap_or(0),
                        web_search: u["server_tool_use"]["web_search_requests"].as_u64().unwrap_or(0),
                        valore: v["total_cost_usd"].as_f64().unwrap_or(0.0),
                    };
                    let modello = v["modelUsage"].as_object().and_then(|m| m.iter().max_by(|a, b| a.1["costUSD"].as_f64().unwrap_or(0.0).total_cmp(&b.1["costUSD"].as_f64().unwrap_or(0.0))).map(|(k, _)| k.clone()));
                    let sessione = sid.lock().map(|x| x.clone()).unwrap_or_default();
                    let stato = if v["is_error"].as_bool() == Some(true) { "error" } else { "done" };
                    if let Some(dir) = &memoria {
                        let nome = crate::studio::titolo_progetto(&crate::studio::radici(&app.state::<App>()), &cartella);
                        let _ = crate::memoria::lavoro_progetto(dir, &nome, &cartella, &richiesta, v["result"].as_str().unwrap_or_default(), stato == "done");
                    }
                    let _ = app.state::<App>().db.registra_esterna(
                        &uuid::Uuid::new_v4().to_string(), &sessione, "studio", &titolo, stato, inizio, crate::store::now_ms(), &usage, None, modello.as_deref(), provider,
                    );
                }
            }
            let mut f = finito.lock().unwrap_or_else(|e| e.into_inner());
            if f.is_none() {
                *f = Some((true, "Claude Code si è chiuso senza risultato".into()));
            }
        });
    }
    HEADLESS.lock().unwrap_or_else(|e| e.into_inner()).insert(cartella.to_path_buf(), Headless { figlio: std::sync::Arc::new(tokio::sync::Mutex::new(figlio)), pid, sessione: sid, finito });
    crate::dev::segnala_progetto(app, cartella);
    // Si aspetta la sessione (arriva subito con l'avvio)
    match tokio::time::timeout(Duration::from_secs(30), rx).await {
        Ok(Ok(id)) => Ok(id),
        _ => Ok(sessione.unwrap_or_default()),
    }
}

/// Scrivere a Claude: nel pannello dell'editor (come l'utente) oppure, se non si può o se si sceglie, in Oltremare.
pub async fn scrivi_a_claude(input: &Value, radici: &[PathBuf], app: &AppHandle) -> Esito {
    let cartella = match percorsi::consentito(str_arg(input, "cartella"), radici) {
        Ok(p) if p.is_dir() => p,
        Ok(p) => return Esito::errore(format!("{} non è una cartella", p.display())),
        Err(e) => return Esito::errore(e),
    };
    crate::dev::segnala_progetto(app, &cartella);
    let s = app.state::<App>().store.settings();
    let qui = input["qui"].as_bool().unwrap_or(false) || s.dev_mode == "oltremare";
    let mut nota = String::new();
    if !qui {
        let e = claude_scrivi(input, radici, app).await;
        if !e.is_error {
            return e;
        }
        nota = format!("Nell'editor non è andata ({}). ", e.testo.chars().take(240).collect::<String>());
    }
    let messaggio = str_arg(input, "messaggio").trim().to_string();
    if messaggio.is_empty() {
        return Esito::errore("Il messaggio è vuoto");
    }
    let nuova = input["nuova_conversazione"].as_bool().unwrap_or(false);
    let sessione = if nuova {
        None
    } else {
        Some(str_arg(input, "sessione").trim().to_string()).filter(|x| !x.is_empty()).or_else(|| sessioni(&cartella).first().and_then(|(p, _)| p.file_stem().map(|x| x.to_string_lossy().into_owned())))
    };
    match avvia_headless(app, &cartella, &messaggio, sessione).await {
        Ok(sid) => {
            let t = format!(
                "{nota}Claude lavora in Oltremare, modalità sviluppo (sessione {sid}): l'utente lo segue nella vista Sviluppo con file, differenze, processi e anteprima. Segui con «editor_claude_stato». In questa modalità Claude modifica i file e usa i comandi di sviluppo consentiti senza chiedere; push e pubblicazioni restano vietati."
            );
            Esito::ok(t.clone(), json!({ "kind": "text", "text": t }))
        }
        Err(e) => Esito::errore(format!("{nota}{e}")),
    }
}

/// La conversazione per la vista: messaggi, azioni con il loro esito, stato.
pub fn vista_sessione(cartella: &Path, scelta: Option<&str>) -> Value {
    let elenco = sessioni(cartella);
    // Se Claude lavora qui in background, si mostra la sua conversazione
    let in_corso = HEADLESS.lock().unwrap_or_else(|e| e.into_inner()).get(cartella).map(|h| h.sessione.lock().map(|x| x.clone()).unwrap_or_default()).filter(|x| !x.is_empty());
    let scelta = scelta.filter(|x| !x.is_empty()).map(str::to_string).or(in_corso);
    let scelta = scelta.as_deref();
    let file = match scelta.filter(|x| !x.is_empty()) {
        Some(id) => elenco.iter().map(|(p, _)| p.clone()).find(|p| p.file_stem().is_some_and(|s| s.to_string_lossy() == id)),
        // Senza una sessione scelta: l'ultima conversazione vera, non una revisione del CTO
        None => elenco.iter().map(|(p, _)| p.clone()).find(|p| !e_revisione(p)),
    };
    let sessioni_vista: Vec<Value> = elenco
        .iter()
        .take(25)
        .map(|(p, m)| {
            let titolo = primo_messaggio(p);
            json!({
                "id": p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
                "title": titolo.chars().take(80).collect::<String>(),
                "updatedAt": m.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0),
                "review": titolo.starts_with(MARCA_REVISIONE),
            })
        })
        .collect();
    let Some(file) = file else {
        return json!({ "id": null, "items": [], "sessions": sessioni_vista, "running": headless_attivo(cartella) == Some(true) });
    };
    let mut items: Vec<Value> = vec![];
    let mut indici: std::collections::HashMap<String, usize> = Default::default();
    if let Ok(t) = std::fs::read_to_string(&file) {
        for riga in t.lines() {
            let Ok(v) = serde_json::from_str::<Value>(riga) else { continue };
            let quando = v["timestamp"].as_str().unwrap_or_default().to_string();
            match v["type"].as_str() {
                Some("user") => {
                    let c = &v["message"]["content"];
                    if let Some(a) = c.as_array() {
                        for b in a.iter().filter(|b| b["type"] == "tool_result") {
                            if let Some(&i) = b["tool_use_id"].as_str().and_then(|id| indici.get(id)) {
                                let r = match &b["content"] {
                                    Value::String(s) => s.clone(),
                                    Value::Array(x) => x.iter().filter_map(|y| y["text"].as_str()).collect::<Vec<_>>().join("\n"),
                                    _ => String::new(),
                                };
                                items[i]["result"] = json!(r.chars().take(3000).collect::<String>());
                                items[i]["error"] = json!(b["is_error"].as_bool() == Some(true));
                                items[i]["done"] = json!(true);
                            }
                        }
                    }
                    let testo = testo_contenuto(c);
                    if !testo.trim().is_empty() && !testo.starts_with('<') && v["isMeta"].as_bool() != Some(true) {
                        items.push(json!({ "kind": "user", "text": testo, "at": quando }));
                    }
                }
                Some("assistant") => {
                    for b in v["message"]["content"].as_array().into_iter().flatten() {
                        match b["type"].as_str() {
                            Some("text") if !b["text"].as_str().unwrap_or_default().trim().is_empty() => items.push(json!({ "kind": "text", "text": b["text"], "at": quando })),
                            Some("tool_use") => {
                                indici.insert(b["id"].as_str().unwrap_or_default().to_string(), items.len());
                                let nome = b["name"].as_str().unwrap_or("strumento");
                                items.push(json!({ "kind": "tool", "tool": nome.replace("mcp__", "").replace("__", " · "), "title": breve(&b["input"]), "input": b["input"], "done": false, "at": quando }));
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
    }
    let salta = items.len().saturating_sub(400);
    let st = leggi_sessione(&file);
    // Se questa sessione l'ha avviata Oltremare si sa con certezza se è in corso (anche dopo «Ferma»);
    // altrimenti (es. nell'editor) si guarda l'attività della sessione
    let id_file = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let running = match headless_sessione(cartella) {
        Some((attivo, sid)) if sid == id_file => attivo,
        Some((true, _)) => true,
        _ => !st.finito && st.inattivo_da < 120,
    };
    json!({
        "id": file.file_stem().map(|s| s.to_string_lossy().into_owned()),
        "items": items.into_iter().skip(salta).collect::<Vec<_>>(),
        "sessions": sessioni_vista,
        "running": running,
        "here": headless_attivo(cartella).is_some(),
        "stuck": !st.finito && st.fermo_da >= 25 && headless_attivo(cartella) != Some(true),
    })
}

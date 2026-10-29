//! Oltremare Studio: la modalità sviluppo, in una finestra a sé come un IDE.
//! Progetti dalle cartelle di sviluppo, un orchestratore che sceglie modello e piano per ogni richiesta,
//! Claude Code in background (piano → approvazione → modifiche) e, su scelta dell'utente,
//! altre AI tramite Claudex (Codex/ChatGPT, Gemini, Grok, DeepSeek, modelli locali…).

use crate::error::{AppError, Result};
use crate::tools::sviluppo::{self, Opzioni};
use crate::App;
use base64::Engine;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder};

// ---------- Progetti ----------

const MARCATORI: &[&str] = &[".git", "package.json", "Cargo.toml", "pubspec.yaml", "go.mod", "pyproject.toml", "requirements.txt", "composer.json", "docker-compose.yml", "deno.json"];

fn e_progetto(p: &Path) -> bool {
    MARCATORI.iter().any(|m| p.join(m).exists())
        || std::fs::read_dir(p).map(|rd| rd.flatten().any(|e| e.file_name().to_string_lossy().ends_with(".sln") || e.file_name().to_string_lossy().ends_with(".xcodeproj"))).unwrap_or(false)
}

fn sotto_progetti(p: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(p)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|q| q.is_dir() && !q.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')) && e_progetto(q))
        .collect();
    v.sort();
    v
}

/// La cartella e le sue sottocartelle dirette (frontend/, backend/, apps/…): i monorepo tengono lì il codice.
fn livelli(p: &Path) -> Vec<PathBuf> {
    let mut v = vec![p.to_path_buf()];
    if let Ok(rd) = std::fs::read_dir(p) {
        let mut sotto: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|q| q.is_dir() && q.file_name().is_some_and(|n| { let n = n.to_string_lossy(); !n.starts_with('.') && !["node_modules", "target", "dist", "build", "docs", "deploy"].contains(&n.as_ref()) }))
            .collect();
        sotto.sort();
        // apps/* e packages/* dei monorepo
        for g in ["apps", "packages", "services"] {
            if let Ok(rd) = std::fs::read_dir(p.join(g)) {
                sotto.extend(rd.flatten().map(|e| e.path()).filter(|q| q.is_dir()));
            }
        }
        v.extend(sotto.into_iter().take(16));
    }
    v
}

fn stack(p: &Path) -> Vec<&'static str> {
    let mut v: Vec<&'static str> = vec![];
    for q in livelli(p) {
        for x in stack_di(&q) {
            if !v.contains(&x) {
                v.push(x);
            }
        }
    }
    v.truncate(6);
    v
}

fn stack_di(p: &Path) -> Vec<&'static str> {
    let mut v = vec![];
    let pkg = std::fs::read_to_string(p.join("package.json")).unwrap_or_default();
    let ha = |k: &str| pkg.contains(&format!("\"{k}"));
    if p.join("src-tauri").is_dir() || ha("@tauri-apps/api") {
        v.push("Tauri");
    }
    if ha("next") {
        v.push("Next.js");
    } else if ha("svelte") || ha("@sveltejs/kit") {
        v.push("Svelte");
    } else if ha("react") {
        v.push("React");
    } else if ha("vue") {
        v.push("Vue");
    } else if ha("@angular/core") {
        v.push("Angular");
    } else if ha("express") || ha("fastify") || ha("hono") {
        v.push("Node");
    }
    if ha("typescript") || p.join("tsconfig.json").exists() {
        v.push("TypeScript");
    }
    if p.join("Cargo.toml").exists() || p.join("src-tauri/Cargo.toml").exists() {
        v.push("Rust");
    }
    if p.join("pubspec.yaml").exists() {
        v.push("Flutter");
    }
    if p.join("pyproject.toml").exists() || p.join("requirements.txt").exists() {
        v.push("Python");
    }
    if p.join("go.mod").exists() {
        v.push("Go");
    }
    if p.join("composer.json").exists() {
        v.push("PHP");
    }
    if std::fs::read_dir(p).map(|rd| rd.flatten().any(|e| e.file_name().to_string_lossy().ends_with(".sln") || e.file_name().to_string_lossy().ends_with(".csproj"))).unwrap_or(false) {
        v.push(".NET");
    }
    if p.join("docker-compose.yml").exists() || p.join("Dockerfile").exists() {
        v.push("Docker");
    }
    if p.join("prisma").is_dir() {
        v.push("Prisma");
    }
    v
}

/// Icona del progetto (favicon, icona dell'app), piccola, come data URI.
fn icona(p: &Path) -> Option<String> {
    livelli(p).iter().find_map(|q| icona_di(q))
}

fn icona_di(p: &Path) -> Option<String> {
    const C: &[&str] = &["app-icon.svg", "public/favicon.svg", "public/icon.svg", "public/logo.svg", "static/favicon.svg", "src/app/icon.svg", "logo.svg", "src-tauri/icons/128x128.png", "public/apple-touch-icon.png", "public/icon-192.png", "public/favicon.png", "static/favicon.png", "assets/icon.png", "src/app/favicon.ico", "public/favicon.ico"];
    for c in C {
        let f = p.join(c);
        let Ok(m) = std::fs::metadata(&f) else { continue };
        if m.len() == 0 || m.len() > 120_000 {
            continue;
        }
        let b = std::fs::read(&f).ok()?;
        let tipo = match f.extension().and_then(|e| e.to_str()) {
            Some("svg") => "image/svg+xml",
            Some("png") => "image/png",
            Some("ico") => "image/x-icon",
            _ => continue,
        };
        return Some(format!("data:{tipo};base64,{}", base64::engine::general_purpose::STANDARD.encode(b)));
    }
    None
}

fn git_info(p: &Path) -> (String, i64) {
    let ramo: String = std::fs::read_to_string(p.join(".git/HEAD")).ok().map(|h| h.trim().trim_start_matches("ref: refs/heads/").chars().take(40).collect()).unwrap_or_default();
    if ramo.is_empty() {
        return (String::new(), -1);
    }
    let out = std::process::Command::new("git").args(["status", "--porcelain"]).current_dir(p).env("PATH", crate::shell_env::path_utente()).output();
    let n = out.ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).lines().count() as i64).unwrap_or(-1);
    (ramo, n)
}

fn ultima_modifica(p: &Path) -> i64 {
    let mut t = std::fs::metadata(p).and_then(|m| m.modified()).ok();
    for f in [".git/index", ".git/FETCH_HEAD", "package.json", "src"] {
        if let Ok(m) = std::fs::metadata(p.join(f)).and_then(|m| m.modified()) {
            if t.is_none_or(|x| m > x) {
                t = Some(m);
            }
        }
    }
    t.and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn scheda(nome: &str, gruppo: &str, radice: &str, cartella: &Path) -> Value {
    let sess = sviluppo::sessioni(cartella);
    let ultima_sessione = sess.first().and_then(|(_, m)| m.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64);
    let (ramo, modifiche) = git_info(cartella);
    let mut hash: u32 = 2166136261;
    for b in nome.bytes() {
        hash = (hash ^ b as u32).wrapping_mul(16777619);
    }
    json!({
        "name": nome,
        "group": gruppo,
        "root": radice,
        "path": cartella.to_string_lossy(),
        "stack": stack(cartella),
        "branch": ramo,
        "changes": modifiche,
        "updatedAt": ultima_modifica(cartella).max(ultima_sessione.unwrap_or(0)),
        "sessions": sess.len(),
        "lastSession": ultima_sessione,
        "icon": icona(cartella),
        "hue": (hash % 360) as i64,
        "readme": std::fs::read_to_string(cartella.join("README.md")).ok().and_then(|r| r.lines().find(|l| !l.trim().is_empty() && !l.starts_with('#') && !l.starts_with('<') && !l.starts_with('[') && !l.starts_with('!')).map(|l| l.trim().chars().take(140).collect::<String>())),
    })
}

/// Le cartelle con i progetti: Scrivania/Sviluppo e Scrivania/Personale (più quelle scelte nelle impostazioni).
pub(crate) fn radici(app: &App) -> Vec<(String, PathBuf)> {
    let mut v = radici_base();
    for extra in app.store.settings().studio_roots {
        let p = crate::tools::percorsi::espandi(&extra);
        let nome = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if p.is_dir() && !v.iter().any(|(_, x)| x == &p) {
            v.push((nome, p));
        }
    }
    v.into_iter().filter(|(_, p)| p.is_dir()).collect()
}

/// Tutti i progetti nelle radici: (nome, gruppo, radice, cartella). Ogni cartella di primo livello è un
/// progetto; se il repository sta un livello sotto si usa quello; se dentro ci sono più progetti diventa un gruppo.
pub fn elenca(radici: &[(String, PathBuf)]) -> Vec<(String, String, String, PathBuf)> {
    let mut lavori: Vec<(String, String, String, PathBuf)> = vec![];
    for (radice, base) in radici {
        let Ok(rd) = std::fs::read_dir(base) else { continue };
        let mut voci: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_dir() && !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.'))).collect();
        voci.sort();
        for p in voci {
            let nome = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if e_progetto(&p) {
                lavori.push((nome, String::new(), radice.clone(), p));
                continue;
            }
            let sotto = sotto_progetti(&p);
            match sotto.len() {
                0 => {}
                1 => lavori.push((nome, String::new(), radice.clone(), sotto[0].clone())),
                _ => {
                    for q in sotto {
                        let n = q.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                        lavori.push((n, nome.clone(), radice.clone(), q));
                    }
                }
            }
        }
    }
    lavori
}

/// Il nome con cui un progetto compare nello Studio e in memoria: la cartella, oppure «gruppo cartella»
/// quando sta dentro una cartella con più progetti (come fa `elenca`).
pub fn titolo_progetto(radici: &[(String, PathBuf)], p: &Path) -> String {
    let nome = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let Some(madre) = p.parent() else { return nome };
    if radici.iter().any(|(_, r)| r == madre) {
        return nome;
    }
    let nome_madre = madre.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if madre.parent().is_some_and(|n| radici.iter().any(|(_, r)| r == n)) {
        return if sotto_progetti(madre).len() <= 1 { nome_madre } else { format!("{nome_madre} {nome}") };
    }
    nome
}

/// Le radici predefinite (Scrivania/Sviluppo e Scrivania/Personale), senza le impostazioni.
pub fn radici_base() -> Vec<(String, PathBuf)> {
    let Some(h) = crate::shell_env::home() else { return vec![] };
    vec![("Sviluppo".into(), h.join("Desktop/Sviluppo")), ("Personale".into(), h.join("Desktop/Personale"))].into_iter().filter(|(_, p)| p.is_dir()).collect()
}

/// Prima stesura di «Com'è fatto» da ciò che lo Studio legge da solo: Claude la riscrive al primo lavoro.
pub fn descrizione_automatica(p: &Path) -> String {
    let mut out = String::from("_Bozza automatica di Oltremare (tecnologie, cartelle e comandi rilevati): Claude la riscrive al primo lavoro nello Studio._\n\n");
    let st = stack(p);
    if !st.is_empty() {
        out.push_str(&format!("- **Tecnologie:** {}\n", st.join(", ")));
    }
    let (ramo, _) = git_info(p);
    if !ramo.is_empty() {
        out.push_str(&format!("- **Git:** ramo `{ramo}`\n"));
    }
    if let Ok(t) = std::fs::read_to_string(p.join("package.json")) {
        let pkg: Value = serde_json::from_str(&t).unwrap_or(Value::Null);
        if let Some(d) = pkg["description"].as_str().filter(|d| !d.trim().is_empty()) {
            out.push_str(&format!("- **Descrizione (package.json):** {}\n", d.trim()));
        }
    }
    let mut cartelle: Vec<String> = std::fs::read_dir(p)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| !n.starts_with('.') && !["node_modules", "target", "dist", "build", "__pycache__", ".venv", "venv", "coverage", "Pods", "bin", "obj"].contains(&n.as_str()))
        .collect();
    cartelle.sort();
    if !cartelle.is_empty() {
        out.push_str(&format!("- **Cartelle:** {}\n", cartelle.iter().take(18).map(|c| format!("`{c}/`")).collect::<Vec<_>>().join(", ")));
    }
    let cmd = comandi(p);
    if !cmd.is_empty() {
        out.push_str("- **Comandi:**\n");
        for c in cmd.iter().take(8) {
            let rel = c["rel"].as_str().unwrap_or("");
            out.push_str(&format!("  - {}: `{}`{}\n", c["label"].as_str().unwrap_or(""), c["command"].as_str().unwrap_or(""), if rel.is_empty() { String::new() } else { format!(" (in `{rel}/`)") }));
        }
    }
    for f in ["CLAUDE.md", "AGENTS.md"] {
        if p.join(f).exists() {
            out.push_str(&format!("- Istruzioni per gli agenti in `{f}`.\n"));
        }
    }
    if let Ok(r) = std::fs::read_to_string(p.join("README.md")) {
        let righe: Vec<&str> = r.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('<') && !l.starts_with("![") && !l.starts_with("[!")).take(12).collect();
        let estratto: String = righe.join("\n").chars().take(900).collect();
        if !estratto.trim().is_empty() {
            out.push_str(&format!("\n**Dal README:**\n\n> {}\n", estratto.replace('\n', "\n> ")));
        }
    }
    out
}

/// Crea (o completa) la nota in memoria di ogni progetto dello Studio. Restituisce quante note ha scritto.
pub fn semina_memoria(radici: &[(String, PathBuf)], dir: &Path) -> usize {
    let mut n = 0;
    for (nome, gruppo, _, cartella) in elenca(radici) {
        let titolo = if gruppo.is_empty() { nome.clone() } else { format!("{gruppo} {nome}") };
        let Ok(nota) = crate::memoria::nota_progetto(dir, &titolo, &cartella) else { continue };
        let Ok(testo) = std::fs::read_to_string(&nota) else { continue };
        if testo.contains("_Da completare") {
            let nuovo = testo.replace("_Da completare: Claude la scrive al primo lavoro nello Studio (stack, struttura, comandi, convenzioni, punti delicati)._", &descrizione_automatica(&cartella));
            if std::fs::write(&nota, nuovo).is_ok() {
                n += 1;
            }
        }
    }
    n
}

/// Dalle impostazioni: le note di tutti i progetti, con la bozza automatica dove manca «Com'è fatto».
#[tauri::command]
pub async fn memory_seed(app: State<'_, App>) -> Result<usize> {
    let dir = crate::memoria::cartella(&app.store.settings()).ok_or_else(|| AppError::msg("La memoria non è attiva"))?;
    let radici = radici(&app);
    tokio::task::spawn_blocking(move || semina_memoria(&radici, &dir)).await.map_err(|e| AppError::msg(e.to_string()))
}

#[tauri::command]
pub async fn studio_projects(app: State<'_, App>) -> Result<Vec<Value>> {
    let radici = radici(&app);
    tokio::task::spawn_blocking(move || {
        let lavori = elenca(&radici);
        // In parallelo: git status su decine di progetti
        let schede: Vec<Value> = std::thread::scope(|sc| {
            let h: Vec<_> = lavori.iter().map(|(n, g, r, p)| sc.spawn(move || scheda(n, g, r, p))).collect();
            h.into_iter().filter_map(|x| x.join().ok()).collect()
        });
        schede
    })
    .await
    .map_err(|e| AppError::msg(e.to_string()))
}

// ---------- Finestra ----------

fn codifica(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~/".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// Apre (o porta avanti) la finestra dello Studio, eventualmente su un progetto e con una richiesta già scritta.
pub fn apri(app: &AppHandle, progetto: Option<&str>, richiesta: Option<&str>) -> Result<()> {
    use tauri::Emitter;
    if let Some(w) = app.get_webview_window("studio") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        if progetto.is_some() || richiesta.is_some() {
            let _ = app.emit_to("studio", "studio-open", json!({ "project": progetto, "prompt": richiesta }));
        }
        return Ok(());
    }
    let mut url = "index.html?view=studio".to_string();
    if let Some(p) = progetto {
        url.push_str(&format!("&project={}", codifica(p)));
    }
    if let Some(r) = richiesta {
        url.push_str(&format!("&prompt={}", codifica(r)));
    }
    let mut b = WebviewWindowBuilder::new(app, "studio", WebviewUrl::App(url.into())).title("Oltremare Studio").inner_size(1500.0, 940.0).min_inner_size(1100.0, 680.0);
    #[cfg(target_os = "macos")]
    {
        b = b.title_bar_style(tauri::TitleBarStyle::Overlay).hidden_title(true);
    }
    b.build().map_err(|e| AppError::msg(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub fn studio_open(handle: AppHandle, project: Option<String>, prompt: Option<String>) -> Result<()> {
    apri(&handle, project.as_deref(), prompt.as_deref())
}

// ---------- Orchestratore ----------

/// Decisione per una richiesta: tipo, modello, piano.
fn euristica(testo: &str) -> Value {
    let t = testo.to_lowercase();
    let parole = t.split_whitespace().count();
    let domanda = t.trim_end().ends_with('?') || ["come funziona", "perché", "spiegami", "cos'è", "dove si trova", "cosa fa", "quale", "come si"].iter().any(|k| t.contains(k));
    let grande = ["refactor", "architettura", "riscrivi", "migra", "nuova funzionalità", "nuovo modulo", "da zero", "integra", "autenticazione", "database", "redesign", "ristruttura"].iter().any(|k| t.contains(k)) || parole > 80;
    let piccola = ["typo", "refuso", "rinomina", "colore", "testo del", "etichetta", "margine", "padding", "commento", "aggiorna la versione", "piccola"].iter().any(|k| t.contains(k)) && parole < 40;
    let (tipo, modello, piano) = if domanda && !t.contains("aggiungi") && !t.contains("modifica") && !t.contains("crea") {
        ("domanda", "sonnet", false)
    } else if grande {
        ("grande", "opus", true)
    } else if piccola {
        ("piccola", "haiku", false)
    } else {
        ("media", "sonnet", parole > 35)
    };
    json!({ "kind": tipo, "model": modello, "plan": piano, "reason": "valutazione rapida", "source": "euristica" })
}

#[tauri::command]
pub async fn studio_classify(handle: AppHandle, project: String, text: String) -> Result<Value> {
    let s = handle.state::<App>().store.settings();
    let Some(bin) = crate::providers::claude_code::trova(&s.claude_code_path) else { return Ok(euristica(&text)) };
    let istr = "Sei l'orchestratore di un ambiente di sviluppo. Classifica la richiesta dell'utente e rispondi SOLO con JSON valido, senza testo attorno:\n\
{\"kind\": \"domanda\"|\"piccola\"|\"media\"|\"grande\", \"model\": \"haiku\"|\"sonnet\"|\"opus\"|\"fable\", \"plan\": true|false, \"reason\": \"una frase in italiano\"}\n\
Regole: domanda = chiede spiegazioni o informazioni, nessuna modifica (sonnet, plan false). \
piccola = modifica locale e ovvia in uno o due file (haiku, plan false). \
media = funzionalità o correzione circoscritta in pochi file (sonnet; plan true solo se ci sono scelte di progetto). \
grande = più parti del sistema, architettura, migrazioni, nuove funzionalità ampie o ambigue (opus, plan true). \
Usa fable (il modello più capace) solo per i lavori grandi più difficili: problemi complessi da diagnosticare, algoritmi delicati, sicurezza, ragionamento molto profondo (plan true).";
    let mut cmd = crate::providers::claude_code::comando(&bin);
    cmd.current_dir(&project)
        .args(["-p", &format!("Progetto: {project}\nRichiesta: {text}"), "--output-format", "json", "--model", "haiku", "--tools", "", "--setting-sources", "", "--strict-mcp-config", "--system-prompt", istr]);
    let out = match tokio::time::timeout(Duration::from_secs(40), cmd.output()).await {
        Ok(Ok(o)) if o.status.success() => o,
        _ => return Ok(euristica(&text)),
    };
    let v: Value = serde_json::from_slice(&out.stdout).unwrap_or(Value::Null);
    let testo = v["result"].as_str().unwrap_or_default();
    let inizio = testo.find('{');
    let fine = testo.rfind('}');
    let d: Value = match (inizio, fine) {
        (Some(a), Some(b)) if b > a => serde_json::from_str(&testo[a..=b]).unwrap_or(Value::Null),
        _ => Value::Null,
    };
    if !d["kind"].is_string() || !d["model"].is_string() {
        return Ok(euristica(&text));
    }
    let mut d = d;
    d["source"] = json!("orchestratore");
    Ok(d)
}

// ---------- Claudex: altre AI scelte dall'utente ----------

fn claudex_bin() -> Option<PathBuf> {
    let sep = if cfg!(windows) { ';' } else { ':' };
    let mut c: Vec<PathBuf> = crate::shell_env::path_utente().split(sep).map(|d| Path::new(d).join("claudex")).collect();
    if let Some(h) = crate::shell_env::home() {
        c.push(h.join(".cargo/bin/claudex"));
        c.push(h.join(".local/bin/claudex"));
    }
    c.into_iter().find(|p| p.is_file())
}

fn claudex_profili() -> Vec<Value> {
    let Some(bin) = claudex_bin() else { return vec![] };
    let out = std::process::Command::new(&bin).args(["config", "show", "--json"]).env("PATH", crate::shell_env::path_utente()).output();
    let Ok(o) = out else { return vec![] };
    let v: Value = serde_json::from_slice(&o.stdout).unwrap_or(Value::Null);
    v["profiles"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|p| p["enabled"].as_bool() != Some(false))
        .map(|p| json!({ "name": p["name"], "provider": p["provider_type"], "model": p["default_model"], "models": p["models"] }))
        .collect()
}

#[tauri::command]
pub async fn studio_claudex() -> Value {
    tokio::task::spawn_blocking(|| json!({ "installed": claudex_bin().is_some(), "profiles": claudex_profili() })).await.unwrap_or(Value::Null)
}

/// Accende il proxy di Claudex se non risponde già. Con `proxy start -d` il demone si spegneva insieme al
/// processo che lo avviava: lo si lancia in primo piano, in un gruppo di processi suo, e lo si lascia acceso.
async fn proxy_claudex(bin: &std::path::Path) -> std::result::Result<(), String> {
    let acceso = || async { tokio::net::TcpStream::connect("127.0.0.1:13456").await.is_ok() };
    if acceso().await {
        return Ok(());
    }
    let mut cmd = std::process::Command::new(bin);
    cmd.args(["proxy", "start"]).env("PATH", crate::shell_env::path_utente()).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000 | 0x0000_0200); // CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP
    }
    cmd.spawn().map_err(|e| format!("Non riesco ad avviare il proxy di Claudex: {e}"))?;
    for _ in 0..40 {
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        if acceso().await {
            return Ok(());
        }
    }
    Err("Il proxy di Claudex non risponde sulla porta 13456 (prova «claudex proxy start» dal Terminale per vedere l'errore)".into())
}

/// Ambiente per far lavorare Claude Code con un profilo di Claudex (proxy locale con traduzione del protocollo).
async fn ambiente_claudex(profilo: &str) -> std::result::Result<Vec<(String, String)>, String> {
    let bin = claudex_bin().ok_or("Claudex non è installato")?;
    let p = claudex_profili().into_iter().find(|x| x["name"] == profilo).ok_or_else(|| format!("Profilo Claudex «{profilo}» non trovato"))?;
    proxy_claudex(&bin).await?;
    let base = format!("http://127.0.0.1:13456/proxy/{profilo}");
    let modello = p["model"].as_str().unwrap_or_default().to_string();
    let slot = |k: &str| p["models"][k].as_str().map(str::to_string).unwrap_or_else(|| modello.clone());
    let mut env = vec![("ANTHROPIC_BASE_URL".to_string(), base), ("ANTHROPIC_AUTH_TOKEN".to_string(), "claudex-passthrough".to_string())];
    if !modello.is_empty() {
        env.push(("ANTHROPIC_MODEL".into(), modello.clone()));
        env.push(("ANTHROPIC_DEFAULT_HAIKU_MODEL".into(), slot("haiku")));
        env.push(("ANTHROPIC_DEFAULT_SONNET_MODEL".into(), slot("sonnet")));
        env.push(("ANTHROPIC_DEFAULT_OPUS_MODEL".into(), slot("opus")));
    }
    Ok(env)
}

// ---------- Lavoro: piano, approvazione, modifiche, revisione ----------

/// Come lavora chi sviluppa nello Studio: lo standard di uno sviluppatore senior.
const SENIOR: &str = "Lavori come uno sviluppatore senior, e il tuo lavoro sarà rivisto da un CTO esigente.\n\
- Prima capisci: leggi il codice coinvolto, le convenzioni del progetto (struttura, stile, librerie già in uso, test esistenti) e la memoria del progetto. Non introdurre librerie o pattern nuovi se ce n'è già uno in uso per lo stesso scopo.\n\
- Modifiche minime e mirate: risolvi la richiesta senza funzionalità in più, senza refactoring non richiesti e senza riformattare file interi; il diff deve leggersi in pochi minuti.\n\
- Qualità: gestisci errori e casi limite, niente valori fissi dove esiste una configurazione, attenzione a sicurezza (input esterni, segreti, permessi, iniezioni) e prestazioni (query, cicli, memoria, chiamate di rete).\n\
- Verifica sempre: esegui build, lint e test disponibili; per comportamenti nuovi o corretti aggiungi o aggiorna i test. Se qualcosa fallisce, correggilo prima di dichiarare concluso; non dichiarare verificato ciò che non hai eseguito.\n\
- Se la richiesta è ambigua, scegli l'interpretazione più ragionevole e dichiarala; non inventare requisiti e non fare domande.\n\
- Riepilogo finale, breve: cosa è cambiato (file), come l'hai verificato (comandi e risultato), rischi residui, cosa resta da fare.\n\
- Sei anche l'orchestratore del lavoro: quando una richiesta ha parti indipendenti (per esempio backend e frontend, più moduli che non si toccano, un'esplorazione ampia del codice, oppure scrivere i test mentre si scrive il codice) \
dividila e affida le parti a sotto-agenti in parallelo con lo strumento Agent, dando a ciascuno un incarico completo e preciso (obiettivo, file, vincoli, come verificare) e cartelle o file distinti perché non si sovrappongano. \
Tu resti il responsabile: leggi quello che hanno fatto, integra le parti, esegui build e test sul risultato complessivo e correggi ciò che non va. Per compiti piccoli o sequenziali lavora da solo: i sotto-agenti servono a fare prima, non a fare di più.";

/// Come rivede il CTO: giudizio concreto e un verdetto strutturato che Oltremare sa leggere.
const CTO: &str = "Sei il CTO e rivedi le modifiche appena fatte da uno sviluppatore senior. Non modificare nulla: solo leggere, eseguire i test e giudicare.\n\
Le modifiche sono quelle non ancora committate: guardale con git status e git diff (leggi per intero i file nuovi). Se i test o il controllo dei tipi sono rapidi, eseguili. Se le modifiche sono molte, dividi la lettura tra sotto-agenti (strumento Agent) e raccogli i loro rilievi, verificandoli tu.\n\
Valuta: correttezza (bug, casi limite, regressioni), coerenza con il progetto (convenzioni, architettura, duplicazioni, librerie), sicurezza, prestazioni, test (ci sono? coprono il cambiamento? passano?), leggibilità e manutenibilità, aderenza alla richiesta (né meno né più).\n\
Sii esigente ma concreto: segnala solo problemi reali e verificati, ognuno con file, riga e la correzione precisa. Niente osservazioni di stile se il progetto non ha una regola in merito.\n\
Chiudi con un giudizio di poche righe e poi, come ultima cosa, un blocco ```json con esattamente questa forma:\n\
{\"esito\": \"approvato\" | \"da correggere\", \"voto\": 1-10, \"sintesi\": \"una frase\", \"problemi\": [{\"gravita\": \"bloccante\" | \"importante\" | \"minore\", \"file\": \"percorso\", \"riga\": 0, \"problema\": \"...\", \"correzione\": \"...\"}]}\n\
«da correggere» solo se c'è almeno un problema bloccante o importante; i minori si elencano ma non bloccano.";

/// Memoria del progetto per Claude: la nota in Obsidian (com'è fatto, decisioni, ultime voci del diario) e,
/// se può scrivere, la cartella delle note e le istruzioni per aggiornarla alla fine.
fn memoria_progetto(st: &App, p: &Path, scrive: bool) -> (Vec<PathBuf>, Option<String>) {
    let mut cartelle = vec![];
    let testo = crate::memoria::cartella(&st.store.settings()).and_then(|dir| {
        let nome = titolo_progetto(&radici(st), p);
        let nota = crate::memoria::nota_progetto(&dir, &nome, p).ok()?;
        if scrive {
            cartelle.push(nota.parent()?.to_path_buf());
        }
        Some(format!(
            "\n\nMemoria del progetto (nota di Obsidian {}):\n<memoria_progetto>\n{}\n</memoria_progetto>\n\
Usala come punto di partenza invece di riesplorare tutto. {}",
            nota.display(),
            crate::memoria::contesto_progetto(&nota),
            if scrive {
                "Alla fine, se hai imparato qualcosa di stabile, aggiorna quella nota con Edit: la sezione «Com'è fatto» (stack, struttura delle cartelle, comandi per avviare, testare e compilare, convenzioni, punti delicati: concisa, da riscrivere se è superata) e «Decisioni» (scelte importanti con il perché, una riga ciascuna). Non toccare «Diario dei lavori»: lo aggiorna Oltremare."
            } else {
                "Non modificarla."
            }
        ))
    });
    (cartelle, testo)
}

/// Revisione del CTO sulle modifiche non committate: sessione a sé, sola lettura, verdetto in JSON alla fine.
#[tauri::command]
pub async fn studio_review(handle: AppHandle, project: String, text: String, model: String, profile: Option<String>) -> Result<Value> {
    let st = handle.state::<App>();
    let p = cartella(&st, &project)?;
    let ambiente = match profile.as_deref().filter(|x| !x.is_empty()) {
        Some(pr) => ambiente_claudex(pr).await.map_err(AppError::msg)?,
        None => vec![],
    };
    let (_, memoria) = memoria_progetto(&st, &p, false);
    let o = Opzioni {
        modello: match model.as_str() {
            "auto" | "" => "opus".into(),
            "fable" => "claude-fable-5-1".into(),
            _ => model,
        },
        modo: "default".into(),
        solo_lettura: true,
        ambiente,
        istruzioni: format!("{CTO}{}", memoria.unwrap_or_default()),
        cartelle: vec![],
    };
    let richiesta = format!("Rivedi le modifiche fatte per questa richiesta dell'utente:\n<richiesta>\n{}\n</richiesta>", text.trim());
    let sid = sviluppo::avvia_headless_con(&handle, &p, &richiesta, None, o).await.map_err(AppError::msg)?;
    Ok(json!({ "session": sid }))
}

/// Il verdetto del CTO: il blocco JSON nell'ultima risposta della sessione di revisione.
#[tauri::command]
pub fn studio_review_result(app: State<'_, App>, project: String, session: String) -> Result<Value> {
    let p = cartella(&app, &project)?;
    let file = sviluppo::sessioni(&p).into_iter().map(|(f, _)| f).find(|f| f.file_stem().is_some_and(|s| s.to_string_lossy() == session)).ok_or_else(|| AppError::msg("Revisione non trovata"))?;
    let t = std::fs::read_to_string(&file)?;
    let mut ultima = String::new();
    for riga in t.lines() {
        let Ok(v) = serde_json::from_str::<Value>(riga) else { continue };
        if v["type"] != "assistant" {
            continue;
        }
        for b in v["message"]["content"].as_array().into_iter().flatten() {
            if let (Some("text"), Some(x)) = (b["type"].as_str(), b["text"].as_str()) {
                if !x.trim().is_empty() {
                    ultima = x.to_string();
                }
            }
        }
    }
    Ok(leggi_verdetto(&ultima))
}

/// Estrae il verdetto dal testo del CTO; se manca o è rotto, resta il giudizio in prosa.
fn leggi_verdetto(testo: &str) -> Value {
    let blocco = testo.rfind("```json").and_then(|i| {
        let dopo = &testo[i + 7..];
        dopo.find("```").map(|j| dopo[..j].trim().to_string())
    });
    let prosa: String = testo.split("```json").next().unwrap_or(testo).trim().chars().take(4000).collect();
    match blocco.and_then(|b| serde_json::from_str::<Value>(&b).ok()) {
        Some(mut v) if v.is_object() => {
            let esito = match v["esito"].as_str() {
                Some("approvato") => "approvato",
                _ => "da correggere",
            };
            let problemi: Vec<Value> = v["problemi"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|x| {
                    json!({
                        "gravita": match x["gravita"].as_str() { Some("bloccante") => "bloccante", Some("importante") => "importante", _ => "minore" },
                        "file": x["file"].as_str().unwrap_or(""),
                        "riga": x["riga"].as_u64().unwrap_or(0),
                        "problema": x["problema"].as_str().unwrap_or(""),
                        "correzione": x["correzione"].as_str().unwrap_or(""),
                    })
                })
                .collect();
            let bloccanti = problemi.iter().any(|x| x["gravita"] != "minore");
            v["esito"] = json!(if esito == "approvato" && !bloccanti { "approvato" } else if !bloccanti { "approvato" } else { "da correggere" });
            v["problemi"] = json!(problemi);
            v["voto"] = json!(v["voto"].as_f64().map(|x| x.clamp(1.0, 10.0).round() as u64));
            v["sintesi"] = json!(v["sintesi"].as_str().unwrap_or("").to_string());
            v["testo"] = json!(prosa);
            v
        }
        _ => json!({ "esito": "sconosciuto", "sintesi": "", "problemi": [], "voto": null, "testo": prosa }),
    }
}

pub fn cartella_pub(app: &App, p: &str) -> Result<PathBuf> {
    cartella(app, p)
}

fn cartella(app: &App, p: &str) -> Result<PathBuf> {
    let reale = crate::tools::percorsi::espandi(p);
    let reale = reale.canonicalize().map_err(|_| AppError::msg("Cartella del progetto non trovata"))?;
    // Lo Studio lavora nelle cartelle dei progetti (e in quelle consentite allo Sviluppatore)
    let mut ok: Vec<PathBuf> = radici(app).into_iter().map(|(_, p)| p).collect();
    ok.extend(crate::tools::percorsi::radici(&app.store.agent("sviluppatore").map(|a| a.allowed_paths).unwrap_or_default()));
    if !ok.iter().any(|r| r.canonicalize().map(|r| reale.starts_with(r)).unwrap_or(false)) {
        return Err(AppError::msg("Il progetto è fuori dalle cartelle dello Studio"));
    }
    Ok(reale)
}

#[tauri::command]
pub async fn studio_run(handle: AppHandle, project: String, text: String, model: String, plan: bool, profile: Option<String>, session: Option<String>, new_session: Option<bool>) -> Result<Value> {
    let st = handle.state::<App>();
    let p = cartella(&st, &project)?;
    let ambiente = match profile.as_deref().filter(|x| !x.is_empty()) {
        Some(pr) => ambiente_claudex(pr).await.map_err(AppError::msg)?,
        None => vec![],
    };
    let sessione = if new_session.unwrap_or(false) { None } else { session.filter(|x| !x.is_empty()).or_else(|| sviluppo::sessioni(&p).first().and_then(|(f, _)| f.file_stem().map(|s| s.to_string_lossy().into_owned()))) };
    let collegamento = st.store.settings().studio_links.get(&project).cloned().map(|l| {
        format!(
            "\nQuesto progetto è collegato al gestionale «{}», progetto {} ({}). Usa i suoi strumenti (mcp__{}__*) per leggere le issue e la documentazione quando servono; se lavori su una issue, seguine le istruzioni (inizio lavoro, chiave nei commit, aggiornamento dello stato).",
            l.server, l.project, l.name, l.server
        )
    });
    let (cartelle, memoria) = memoria_progetto(&st, &p, !plan);
    let o = Opzioni {
        cartelle,
        solo_lettura: false,
        modello: match model.as_str() {
            "auto" => String::new(),
            // Fable non ha un alias in Claude Code: si usa il nome del modello
            "fable" => "claude-fable-5-1".into(),
            _ => model,
        },
        modo: if plan { "plan".into() } else { "acceptEdits".into() },
        ambiente,
        istruzioni: format!(
            "{SENIOR}\n{}{}{}",
            if plan { "Ora sei in modalità piano: esplora e proponi un piano concreto (file da toccare, passi, come verificare), senza modificare nulla. L'utente lo approverà prima dell'esecuzione. Scrivi il piano completo nel tuo ultimo messaggio: Oltremare lo mostra con il pulsante «Approva ed esegui» (non cercare né usare ExitPlanMode e non chiedere approvazione a parole). Se il lavoro ha parti indipendenti, nel piano indica quali e di' che le farai in parallelo con sotto-agenti." } else { "" },
            collegamento.unwrap_or_default(),
            memoria.unwrap_or_default()
        ),
    };
    let sid = sviluppo::avvia_headless_con(&handle, &p, &text, sessione, o).await.map_err(AppError::msg)?;
    Ok(json!({ "session": sid, "plan": plan }))
}

/// Il piano proposto nell'ultima esecuzione in modalità piano: il file del piano, oppure ExitPlanMode, oppure l'ultima risposta.
#[tauri::command]
pub fn studio_plan(app: State<'_, App>, project: String, session: String) -> Result<Value> {
    let p = cartella(&app, &project)?;
    let file = sviluppo::sessioni(&p).into_iter().map(|(f, _)| f).find(|f| f.file_stem().is_some_and(|s| s.to_string_lossy() == session)).ok_or_else(|| AppError::msg("Conversazione non trovata"))?;
    let t = std::fs::read_to_string(&file)?;
    let mut piano = String::new();
    let mut breve = String::new();
    let mut ultima = String::new();
    for riga in t.lines() {
        let Ok(v) = serde_json::from_str::<Value>(riga) else { continue };
        if v["type"] != "assistant" {
            continue;
        }
        for b in v["message"]["content"].as_array().into_iter().flatten() {
            match (b["type"].as_str(), b["name"].as_str()) {
                (Some("tool_use"), Some("Write")) if b["input"]["file_path"].as_str().is_some_and(|f| f.contains("/plans/")) => piano = b["input"]["content"].as_str().unwrap_or_default().to_string(),
                (Some("tool_use"), Some("ExitPlanMode")) => breve = b["input"]["plan"].as_str().unwrap_or_default().to_string(),
                (Some("text"), _) => ultima = b["text"].as_str().unwrap_or_default().to_string(),
                _ => {}
            }
        }
    }
    let testo = if !piano.is_empty() { piano } else if !breve.is_empty() { breve } else { ultima };
    Ok(json!({ "plan": testo }))
}

#[tauri::command]
pub async fn studio_approve(handle: AppHandle, project: String, session: String, model: String, profile: Option<String>, notes: Option<String>) -> Result<Value> {
    let testo = match notes.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        Some(n) => format!("L'utente ha approvato il piano con queste indicazioni: {n}\nProcedi con l'implementazione e verifica il risultato."),
        None => "L'utente ha approvato il piano: procedi con l'implementazione e verifica il risultato (build e test se ci sono).".into(),
    };
    studio_run(handle, project, testo, model, false, profile, Some(session), Some(false)).await
}

/// Secondo parere di un'altra AI (via Claudex) sul piano o sulle modifiche: solo lettura, su richiesta dell'utente.
#[tauri::command]
pub async fn studio_second_opinion(handle: AppHandle, project: String, profile: String, about: String, plan: Option<String>) -> Result<String> {
    let st = handle.state::<App>();
    let p = cartella(&st, &project)?;
    let s = st.store.settings();
    let bin = crate::providers::claude_code::trova(&s.claude_code_path).ok_or_else(|| AppError::msg("Claude Code non è installato"))?;
    let ambiente = ambiente_claudex(&profile).await.map_err(AppError::msg)?;
    let richiesta = if about == "plan" {
        format!("Fai una revisione critica di questo piano per il progetto nella cartella corrente: rischi, parti mancanti, alternative migliori, ordine dei passi. Sii concreto e breve, in italiano.\n\nPIANO:\n{}", plan.unwrap_or_default())
    } else {
        "Fai una code review delle modifiche non ancora committate nella cartella corrente (usa git diff): bug, casi limite, sicurezza, semplificazioni. Elenca i problemi in ordine di gravità, con file e riga, in italiano.".to_string()
    };
    let o = Opzioni { modo: "plan".into(), ambiente, ..Default::default() };
    let mut cmd = sviluppo::comando_headless_con(&bin, &p, &richiesta, None, &s, &o);
    cmd.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null());
    let out = tokio::time::timeout(Duration::from_secs(600), cmd.output()).await.map_err(|_| AppError::msg("Il secondo parere ha impiegato troppo tempo"))?.map_err(|e| AppError::msg(e.to_string()))?;
    let testo = String::from_utf8_lossy(&out.stdout);
    let risultato = testo.lines().rev().filter_map(|l| serde_json::from_str::<Value>(l).ok()).find(|v| v["type"] == "result").and_then(|v| v["result"].as_str().map(str::to_string));
    risultato.ok_or_else(|| AppError::msg("Nessuna risposta dal secondo parere"))
}

/// Salva un file modificato nell'editor dello Studio.
#[tauri::command]
pub fn studio_save(app: State<'_, App>, path: String, content: String) -> Result<()> {
    let p = crate::tools::percorsi::espandi(&path);
    let dir = p.parent().ok_or_else(|| AppError::msg("Percorso non valido"))?;
    cartella(&app, &dir.to_string_lossy())?;
    std::fs::write(&p, content)?;
    Ok(())
}

#[cfg(test)]
mod prove {
    #[test]
    fn euristica_sceglie_modello_e_piano() {
        assert_eq!(super::euristica("come funziona il login?")["kind"], "domanda");
        assert_eq!(super::euristica("correggi il refuso nel titolo")["model"], "haiku");
        let g = super::euristica("rifai l'architettura dell'autenticazione con un nuovo modulo");
        assert_eq!(g["model"], "opus");
        assert_eq!(g["plan"], true);
    }

    #[test]
    fn analizza_issue_e_progetti() {
        let re = regex::Regex::new(r"(?m)^-\s*([A-Z][A-Z0-9]{1,9}-\d+)\s*·\s*(.+?)\s+—\s+([^,\n]+)(?:,\s*(.+))?$").unwrap();
        let t = "Assegnate a te (2):\n- VICK-321 · Bug b2b Iphone 8 — Da fare\n- VICK-63 · App Merchant -  Iphone 12 mini — In corso, scadenza 3 nov";
        let v: Vec<_> = re.captures_iter(t).map(|c| (c[1].to_string(), c[2].trim().to_string(), c[3].trim().to_string())).collect();
        assert_eq!(v[0], ("VICK-321".into(), "Bug b2b Iphone 8".into(), "Da fare".into()));
        assert_eq!(v[1].2, "In corso");
        let rp = regex::Regex::new(r"(?m)^-\s*progetto\s+([A-Z][A-Z0-9]{1,9}):\s*(.+)$").unwrap();
        assert_eq!(&rp.captures("- progetto KEYO: KeyOps").unwrap()[1], "KEYO");
    }

    #[test]
    #[ignore]
    fn comandi_veri() {
        let h = crate::shell_env::home().unwrap();
        for p in ["Desktop/Sviluppo/FarmaQuest/FarmaQuest", "Desktop/Sviluppo/KeyOps", "Desktop/Personale/Suites/Oltremare"] {
            let c = super::comandi(&h.join(p));
            println!("{p}: {}", c.iter().take(6).map(|x| format!("{} [{}]", x["label"].as_str().unwrap(), x["command"].as_str().unwrap())).collect::<Vec<_>>().join(" | "));
        }
    }

    #[test]
    #[ignore]
    fn scansione_vera() {
        let h = crate::shell_env::home().unwrap();
        let mut n = 0;
        for base in ["Desktop/Sviluppo", "Desktop/Personale"] {
            for e in std::fs::read_dir(h.join(base)).unwrap().flatten() {
                let p = e.path();
                if !p.is_dir() {
                    continue;
                }
                let t = if super::e_progetto(&p) { vec![p.clone()] } else { super::sotto_progetti(&p) };
                for q in t {
                    let s = super::scheda(&q.file_name().unwrap().to_string_lossy(), "", base, &q);
                    n += 1;
                    if n <= 6 {
                        println!("{} {:?} ramo={} modifiche={} icona={}", s["name"], s["stack"], s["branch"], s["changes"], s["icon"].is_string());
                    }
                }
            }
        }
        println!("progetti: {n}");
        assert!(n > 10);
    }
}

// ---------- Gestionale delle issue (server MCP): progetto collegato, task e documentazione ----------

/// Server collegati che hanno strumenti per le issue.
fn server_issue(app: &App) -> Vec<String> {
    app.mcp_clienti.stato().into_iter().filter(|s| s.connected && s.tools.iter().any(|t| t.ends_with("list_my_issues") || t.ends_with("_search"))).map(|s| s.name).collect()
}

#[tauri::command]
pub fn studio_link(app: State<'_, App>, project: String) -> Value {
    let link = app.store.settings().studio_links.get(&project).cloned();
    json!({ "link": link, "servers": server_issue(&app), "canImport": !crate::mcp_client::nomi_claude_code().is_empty() })
}

#[tauri::command]
pub fn studio_link_set(handle: AppHandle, app: State<'_, App>, project: String, server: String, key: String, name: String) -> Result<()> {
    use tauri::Emitter;
    let mut s = app.store.settings();
    if key.trim().is_empty() {
        s.studio_links.remove(&project);
    } else {
        s.studio_links.insert(project, crate::store::StudioLink { server, project: key.trim().to_uppercase(), name });
    }
    app.store.save_settings(&s)?;
    let _ = handle.emit("settings-changed", crate::settings_state(&app));
    Ok(())
}

/// Progetti del gestionale che somigliano alla cartella (ricerca per nome).
#[tauri::command]
pub async fn studio_link_suggest(handle: AppHandle, project: String, server: String) -> Result<Vec<Value>> {
    let p = PathBuf::from(&project);
    let mut nomi: Vec<String> = vec![];
    for x in [p.file_name(), p.parent().and_then(|q| q.file_name())].into_iter().flatten() {
        let n = x.to_string_lossy().trim_start_matches('_').replace(['-', '_'], " ");
        if !n.is_empty() && !["Sviluppo", "Personale", "Suites", "Desktop"].contains(&n.as_str()) && !nomi.contains(&n) {
            nomi.push(n);
        }
    }
    let re = regex::Regex::new(r"(?m)^-\s*progetto\s+([A-Z][A-Z0-9]{1,9}):\s*(.+)$").map_err(|e| AppError::msg(e.to_string()))?;
    let mut v: Vec<Value> = vec![];
    for n in nomi {
        if let Ok(t) = crate::mcp_client::chiama_server(&handle, &server, "search", json!({ "query": n })).await {
            for c in re.captures_iter(&t) {
                if !v.iter().any(|x| x["key"] == c[1]) {
                    v.push(json!({ "key": &c[1], "name": c[2].trim() }));
                }
            }
        }
    }
    Ok(v)
}

/// Le issue del progetto collegato (assegnate all'utente o segnalate da lui).
#[tauri::command]
pub async fn studio_issues(handle: AppHandle, project: String) -> Result<Value> {
    let link = handle.state::<App>().store.settings().studio_links.get(&project).cloned().ok_or_else(|| AppError::msg("Progetto non collegato"))?;
    let t = crate::mcp_client::chiama_server(&handle, &link.server, "list_my_issues", json!({ "includeReported": true })).await.map_err(AppError::msg)?;
    let re = regex::Regex::new(r"(?m)^-\s*([A-Z][A-Z0-9]{1,9}-\d+)\s*·\s*(.+?)\s+—\s+([^,\n]+)(?:,\s*(.+))?$").map_err(|e| AppError::msg(e.to_string()))?;
    let prefisso = format!("{}-", link.project);
    let issue: Vec<Value> = re
        .captures_iter(&t)
        .filter(|c| c[1].starts_with(&prefisso))
        .map(|c| json!({ "key": &c[1], "title": c[2].trim(), "status": c[3].trim(), "extra": c.get(4).map(|m| m.as_str().trim()) }))
        .collect();
    Ok(json!({ "link": link, "issues": issue }))
}

#[tauri::command]
pub async fn studio_issue(handle: AppHandle, project: String, key: String) -> Result<String> {
    let link = handle.state::<App>().store.settings().studio_links.get(&project).cloned().ok_or_else(|| AppError::msg("Progetto non collegato"))?;
    crate::mcp_client::chiama_server(&handle, &link.server, "get_issue", json!({ "key": key })).await.map_err(AppError::msg)
}

// ---------- Avvio delle app in locale ----------

fn gestore_pacchetti(p: &Path) -> &'static str {
    if p.join("pnpm-lock.yaml").exists() {
        "pnpm"
    } else if p.join("yarn.lock").exists() {
        "yarn"
    } else if p.join("bun.lockb").exists() || p.join("bun.lock").exists() {
        "bun"
    } else {
        "npm"
    }
}

/// I comandi con cui il progetto (o una sua parte: frontend/, backend/…) si avvia in locale.
#[tauri::command]
pub fn studio_commands(app: State<'_, App>, project: String) -> Result<Vec<Value>> {
    let radice = cartella(&app, &project)?;
    Ok(comandi(&radice))
}

fn comandi(radice: &Path) -> Vec<Value> {
    let radice = radice.to_path_buf();
    let mut v: Vec<Value> = vec![];
    for dir in livelli(&radice) {
        let rel = dir.strip_prefix(&radice).map(|r| r.to_string_lossy().into_owned()).unwrap_or_default();
        let parte = if rel.is_empty() { String::new() } else { format!(" · {rel}") };
        let mut aggiungi = |etichetta: String, comando: String, tipo: &str, priorita: i64| {
            v.push(json!({ "label": etichetta, "command": comando, "cwd": dir.to_string_lossy(), "rel": rel, "kind": tipo, "priority": priorita }));
        };
        if let Ok(t) = std::fs::read_to_string(dir.join("package.json")) {
            let pkg: Value = serde_json::from_str(&t).unwrap_or(Value::Null);
            let pm = gestore_pacchetti(&dir).to_string();
            let esegui = |s: &str| if pm == "npm" { format!("npm run {s}") } else { format!("{pm} {s}") };
            if let Some(scripts) = pkg["scripts"].as_object() {
                if scripts.contains_key("tauri") && dir.join("src-tauri").is_dir() {
                    aggiungi(format!("App desktop{parte}"), esegui("tauri dev"), "tauri", 105);
                }
                for (nome, etichetta, prio) in [("dev", "Sviluppo", 100), ("start", "Avvia", 90), ("serve", "Server", 85), ("preview", "Anteprima della build", 60), ("storybook", "Storybook", 50), ("test", "Test", 40), ("build", "Build", 30), ("lint", "Lint", 20)] {
                    if scripts.contains_key(nome) {
                        aggiungi(format!("{etichetta}{parte}"), esegui(nome), if nome == "test" || nome == "build" || nome == "lint" { "task" } else { "web" }, prio - if rel.is_empty() { 0 } else { 5 });
                    }
                }
            }
            if !dir.join("node_modules").is_dir() {
                aggiungi(format!("Installa le dipendenze{parte}"), format!("{} install", gestore_pacchetti(&dir)), "setup", 10);
            }
        }
        if dir.join("docker-compose.yml").exists() || dir.join("compose.yaml").exists() {
            aggiungi(format!("Docker Compose{parte}"), "docker compose up".into(), "docker", 70);
        }
        if dir.join("Cargo.toml").exists() && !dir.join("package.json").exists() && !dir.ends_with("src-tauri") {
            aggiungi(format!("Cargo run{parte}"), "cargo run".into(), "rust", 65);
        }
        if dir.join("manage.py").exists() {
            aggiungi(format!("Django{parte}"), "python3 manage.py runserver".into(), "python", 80);
        } else if std::fs::read_to_string(dir.join("requirements.txt")).unwrap_or_default().to_lowercase().contains("fastapi") {
            let modulo = ["main.py", "app.py", "app/main.py"].iter().find(|f| dir.join(f).exists()).map(|f| f.trim_end_matches(".py").replace('/', ".")).unwrap_or_else(|| "main".into());
            aggiungi(format!("FastAPI{parte}"), format!("uvicorn {modulo}:app --reload"), "python", 80);
        }
        if dir.join("pubspec.yaml").exists() {
            aggiungi(format!("Flutter{parte}"), "flutter run -d macos".into(), "flutter", 75);
        }
        if std::fs::read_dir(&dir).map(|rd| rd.flatten().any(|e| e.file_name().to_string_lossy().ends_with(".csproj"))).unwrap_or(false) {
            aggiungi(format!(".NET{parte}"), "dotnet run".into(), "dotnet", 75);
        }
        if dir.join("go.mod").exists() {
            aggiungi(format!("Go{parte}"), "go run .".into(), "go", 70);
        }
    }
    v.sort_by(|a, b| b["priority"].as_i64().cmp(&a["priority"].as_i64()));
    v.truncate(16);
    v
}

#[tauri::command]
pub async fn studio_launch(app: State<'_, App>, project: String, cwd: String, command: String, name: String) -> Result<String> {
    let radice = cartella(&app, &project)?;
    let dir = cartella(&app, &cwd)?;
    if !dir.starts_with(&radice) {
        return Err(AppError::msg("La cartella non fa parte del progetto"));
    }
    let input = json!({ "comando": command, "cartella": dir.to_string_lossy(), "nome": name, "attendi_secondi": 12 });
    let e = crate::tools::processi::esegui("avvia_processo", &input, &[radice]).await;
    if e.is_error {
        Err(AppError::msg(e.testo))
    } else {
        Ok(e.testo)
    }
}

#[tauri::command]
pub async fn studio_stop(id: String) -> Result<()> {
    let e = crate::tools::processi::esegui("ferma_processo", &json!({ "id": id }), &[]).await;
    if e.is_error {
        Err(AppError::msg(e.testo))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod prove_claudex {
    #[test]
    #[ignore = "scrive le note di tutti i progetti nella memoria dell'utente"]
    fn semina_le_note_dei_progetti() {
        let dir = std::path::PathBuf::from(std::env::var("OLTREMARE_MEMORIA").expect("OLTREMARE_MEMORIA"));
        let radici = super::radici_base();
        let n = super::semina_memoria(&radici, &dir);
        eprintln!("progetti: {}, note scritte: {n}", super::elenca(&radici).len());
    }

    #[test]
    fn il_verdetto_si_legge_anche_se_impreciso() {
        let v = super::leggi_verdetto("Bene nel complesso.\n```json\n{\"esito\": \"approvato\", \"voto\": 8, \"sintesi\": \"ok\", \"problemi\": [{\"gravita\": \"minore\", \"file\": \"a.ts\", \"riga\": 3, \"problema\": \"nome\", \"correzione\": \"rinomina\"}]}\n```");
        assert_eq!(v["esito"], "approvato");
        assert_eq!(v["voto"], 8);
        // «approvato» con un problema importante diventa «da correggere»
        let v = super::leggi_verdetto("```json\n{\"esito\": \"approvato\", \"problemi\": [{\"gravita\": \"importante\", \"problema\": \"bug\"}]}\n```");
        assert_eq!(v["esito"], "da correggere");
        assert_eq!(super::leggi_verdetto("solo prosa")["esito"], "sconosciuto");
    }

    #[test]
    #[ignore = "dipende da Claudex installato sul computer"]
    fn legge_i_profili() {
        let p = super::claudex_profili();
        eprintln!("bin: {:?}, profili: {p:?}", super::claudex_bin());
        assert!(!p.is_empty());
    }
}

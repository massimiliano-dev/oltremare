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
fn radici(app: &App) -> Vec<(String, PathBuf)> {
    let Some(h) = crate::shell_env::home() else { return vec![] };
    let mut v: Vec<(String, PathBuf)> = vec![("Sviluppo".into(), h.join("Desktop/Sviluppo")), ("Personale".into(), h.join("Desktop/Personale"))];
    for extra in app.store.settings().studio_roots {
        let p = crate::tools::percorsi::espandi(&extra);
        let nome = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if p.is_dir() && !v.iter().any(|(_, x)| x == &p) {
            v.push((nome, p));
        }
    }
    v.into_iter().filter(|(_, p)| p.is_dir()).collect()
}

#[tauri::command]
pub async fn studio_projects(app: State<'_, App>) -> Result<Vec<Value>> {
    let radici = radici(&app);
    tokio::task::spawn_blocking(move || {
        // Ogni cartella di primo livello è un progetto; se il repository sta un livello sotto si usa quello;
        // se dentro ci sono più progetti diventa un gruppo
        let mut lavori: Vec<(String, String, String, PathBuf)> = vec![];
        for (radice, base) in &radici {
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

/// Ambiente per far lavorare Claude Code con un profilo di Claudex (proxy locale con traduzione del protocollo).
async fn ambiente_claudex(profilo: &str) -> std::result::Result<Vec<(String, String)>, String> {
    let bin = claudex_bin().ok_or("Claudex non è installato")?;
    let p = claudex_profili().into_iter().find(|x| x["name"] == profilo).ok_or_else(|| format!("Profilo Claudex «{profilo}» non trovato"))?;
    // Il proxy parte in background se non è già acceso
    let _ = tokio::process::Command::new(&bin).args(["proxy", "start", "-d"]).env("PATH", crate::shell_env::path_utente()).output().await;
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

// ---------- Lavoro: piano, approvazione, modifiche ----------

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
    let o = Opzioni {
        modello: match model.as_str() {
            "auto" => String::new(),
            // Fable non ha un alias in Claude Code: si usa il nome del modello
            "fable" => "claude-fable-5-1".into(),
            _ => model,
        },
        modo: if plan { "plan".into() } else { "acceptEdits".into() },
        ambiente,
        istruzioni: format!(
            "{}{}",
            if plan { "Ora sei in modalità piano: esplora e proponi un piano concreto (file da toccare, passi, come verificare), senza modificare nulla. L'utente lo approverà prima dell'esecuzione." } else { "" },
            collegamento.unwrap_or_default()
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

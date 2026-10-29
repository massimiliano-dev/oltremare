//! Vista «Sviluppo»: un piccolo IDE dentro Oltremare per seguire e guidare il lavoro su un progetto
//! quando non si usa (o non si riesce a usare) il pannello di Claude nell'editor.
//! File e differenze git del progetto, conversazione di Claude Code dal vivo, processi e anteprima.

use crate::error::{AppError, Result};
use crate::tools::{percorsi, sviluppo};
use crate::App;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager, State};

/// Cartelle in cui la vista può leggere: quelle dell'agente Sviluppatore (o le predefinite).
fn radici(app: &App) -> Vec<PathBuf> {
    let consentite = app.store.agent("sviluppatore").map(|a| a.allowed_paths).unwrap_or_default();
    percorsi::radici(&consentite)
}

fn dentro(app: &App, p: &str) -> Result<PathBuf> {
    percorsi::consentito(p, &radici(app)).map_err(AppError::msg)
}

const SALTA: &[&str] = &[".git", "node_modules", "target", "dist", "build", ".next", ".svelte-kit", "__pycache__", ".venv", "venv", ".turbo", ".cache", "coverage", "Pods", ".DS_Store"];

/// Il progetto su cui si sta lavorando: la vista lo apre, ed è ricordato tra i recenti.
pub fn segnala_progetto(app: &AppHandle, cartella: &Path) {
    let st = app.state::<App>();
    let p = cartella.to_string_lossy().into_owned();
    let mut recenti: Vec<String> = st.store.pref("dev:recenti").and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
    recenti.retain(|x| x != &p);
    recenti.insert(0, p.clone());
    recenti.truncate(12);
    let _ = st.store.set_pref("dev:recenti", json!(recenti));
    let _ = app.emit("dev-project", json!({ "folder": p }));
}

#[tauri::command]
pub fn dev_projects(app: State<'_, App>) -> Vec<String> {
    let l: Vec<String> = app.store.pref("dev:recenti").and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
    l.into_iter().filter(|p| Path::new(p).is_dir()).collect()
}

#[tauri::command]
pub fn dev_open(handle: AppHandle, app: State<'_, App>, folder: String) -> Result<String> {
    let p = dentro(&app, &folder)?;
    if !p.is_dir() {
        return Err(AppError::msg("Non è una cartella"));
    }
    segnala_progetto(&handle, &p);
    Ok(p.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn dev_tree(app: State<'_, App>, folder: String) -> Result<Vec<Value>> {
    let p = dentro(&app, &folder)?;
    let mut voci: Vec<(bool, String, PathBuf)> = std::fs::read_dir(&p)?
        .flatten()
        .filter_map(|e| {
            let nome = e.file_name().to_string_lossy().into_owned();
            if SALTA.contains(&nome.as_str()) {
                return None;
            }
            let dir = e.file_type().ok()?.is_dir();
            Some((dir, nome, e.path()))
        })
        .collect();
    voci.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase())));
    Ok(voci.into_iter().take(800).map(|(dir, name, path)| json!({ "name": name, "path": path.to_string_lossy(), "dir": dir })).collect())
}

#[tauri::command]
pub fn dev_file(app: State<'_, App>, path: String) -> Result<Value> {
    let p = dentro(&app, &path)?;
    let meta = std::fs::metadata(&p)?;
    const MAX: u64 = 512 * 1024;
    let bytes = {
        use std::io::Read;
        let mut b = Vec::new();
        std::fs::File::open(&p)?.take(MAX).read_to_end(&mut b)?;
        b
    };
    let binario = bytes.iter().take(8000).any(|&c| c == 0);
    Ok(json!({
        "path": p.to_string_lossy(),
        "content": if binario { String::new() } else { String::from_utf8_lossy(&bytes).into_owned() },
        "binary": binario,
        "truncated": meta.len() > MAX,
        "size": meta.len(),
        "ext": p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default(),
    }))
}

fn git(cartella: &Path, args: &[&str]) -> Option<String> {
    let o = std::process::Command::new("git").args(args).current_dir(cartella).env("PATH", crate::shell_env::path_utente()).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).into_owned())
}

#[tauri::command]
pub async fn dev_git(app: State<'_, App>, folder: String) -> Result<Value> {
    let p = dentro(&app, &folder)?;
    tokio::task::spawn_blocking(move || {
        let Some(stato) = git(&p, &["status", "--porcelain=v1", "-uall"]) else { return json!({ "repo": false }) };
        let ramo = git(&p, &["branch", "--show-current"]).map(|s| s.trim().to_string()).unwrap_or_default();
        let modifiche: Vec<Value> = stato
            .lines()
            .filter(|l| l.len() > 3)
            .take(500)
            .map(|l| {
                let (codice, file) = l.split_at(3);
                let file = file.rsplit(" -> ").next().unwrap_or(file).trim_matches('"');
                json!({ "status": codice.trim(), "path": p.join(file).to_string_lossy(), "rel": file })
            })
            .collect();
        let ultimo = git(&p, &["log", "-1", "--pretty=%h %s"]).map(|s| s.trim().to_string()).unwrap_or_default();
        json!({ "repo": true, "branch": ramo, "changes": modifiche, "last": ultimo })
    })
    .await
    .map_err(|e| AppError::msg(e.to_string()))
}

#[tauri::command]
pub async fn dev_diff(app: State<'_, App>, folder: String, file: Option<String>) -> Result<String> {
    let p = dentro(&app, &folder)?;
    let f = match file {
        Some(f) => Some(dentro(&app, &f)?),
        None => None,
    };
    tokio::task::spawn_blocking(move || {
        let mut args = vec!["diff", "HEAD", "--no-color", "--"];
        let rel;
        if let Some(f) = &f {
            rel = f.strip_prefix(&p).unwrap_or(f).to_string_lossy().into_owned();
            args.push(&rel);
        }
        let mut d = git(&p, &args).unwrap_or_default();
        // File nuovi non ancora in git: si mostrano per intero
        if d.trim().is_empty() {
            if let Some(f) = &f {
                if let Ok(t) = std::fs::read_to_string(f) {
                    d = format!("+++ {} (nuovo)\n{}", f.display(), t.lines().take(2000).map(|l| format!("+{l}")).collect::<Vec<_>>().join("\n"));
                }
            }
        }
        d.chars().take(400_000).collect()
    })
    .await
    .map_err(|e| AppError::msg(e.to_string()))
}

#[tauri::command]
pub fn dev_session(app: State<'_, App>, folder: String, session: Option<String>) -> Result<Value> {
    let p = dentro(&app, &folder)?;
    Ok(sviluppo::vista_sessione(&p, session.as_deref()))
}

#[tauri::command]
pub fn dev_processes() -> Vec<Value> {
    crate::tools::processi::vista()
}

/// Scrivere a Claude dalla vista: secondo l'impostazione nell'editor, o qui in background.
#[tauri::command]
pub async fn dev_send(handle: AppHandle, folder: String, message: String, session: Option<String>, new_session: Option<bool>, here: Option<bool>) -> Result<String> {
    let st = handle.state::<App>();
    let p = dentro(&st, &folder)?;
    let input = json!({ "cartella": p.to_string_lossy(), "messaggio": message, "sessione": session.unwrap_or_default(), "nuova_conversazione": new_session.unwrap_or(false), "qui": here.unwrap_or(true) });
    let e = sviluppo::scrivi_a_claude(&input, &radici(&st), &handle).await;
    if e.is_error {
        Err(AppError::msg(e.testo))
    } else {
        Ok(e.testo)
    }
}

#[tauri::command]
pub async fn dev_stop(app: State<'_, App>, folder: String) -> Result<bool> {
    // Il progetto può essere dello Studio (Scrivania/Sviluppo, Personale) o tra le cartelle dello Sviluppatore
    let p = crate::studio::cartella_pub(&app, &folder).or_else(|_| dentro(&app, &folder))?;
    Ok(sviluppo::ferma_headless(&p).await)
}

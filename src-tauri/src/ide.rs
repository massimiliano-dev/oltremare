//! Mini IDE dello Studio: elenco dei file (apertura rapida), ricerca nel progetto e operazioni sui file.
//! Tutto resta dentro la cartella del progetto; le eliminazioni vanno nel Cestino.

use crate::error::{AppError, Result};
use crate::App;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tauri::State;

const SALTA: &[&str] = &[".git", "node_modules", "target", "dist", "build", ".next", ".svelte-kit", "__pycache__", ".venv", "venv", ".turbo", ".cache", "coverage", "Pods", "bin", "obj"];

fn progetto(app: &App, folder: &str) -> Result<PathBuf> {
    crate::studio::cartella_pub(app, folder)
}

/// Il percorso sta dentro il progetto (anche se non esiste ancora: si controlla la cartella madre).
fn dentro(radice: &Path, p: &str) -> Result<PathBuf> {
    let p = PathBuf::from(p);
    let base = if p.exists() { p.canonicalize()? } else { p.parent().and_then(|m| m.canonicalize().ok()).map(|m| m.join(p.file_name().unwrap_or_default())).ok_or_else(|| AppError::msg("Percorso non valido"))? };
    if !base.starts_with(radice) || base == radice {
        return Err(AppError::msg("Il percorso è fuori dal progetto"));
    }
    Ok(base)
}

/// Esecuzione di git nella cartella del progetto (rispetta .gitignore); `None` se non è un repository.
fn git(radice: &Path, args: &[&str]) -> Option<String> {
    let o = std::process::Command::new("git").args(args).current_dir(radice).env("PATH", crate::shell_env::path_utente()).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).into_owned())
}

fn cammina(radice: &Path, max: usize) -> Vec<String> {
    let mut out = vec![];
    let mut pila = vec![radice.to_path_buf()];
    while let Some(d) = pila.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let nome = e.file_name().to_string_lossy().into_owned();
            let p = e.path();
            if p.is_dir() {
                if !SALTA.contains(&nome.as_str()) && !nome.starts_with('.') {
                    pila.push(p);
                }
            } else if let Ok(r) = p.strip_prefix(radice) {
                out.push(r.to_string_lossy().into_owned());
                if out.len() >= max {
                    return out;
                }
            }
        }
    }
    out
}

/// Tutti i file del progetto (relativi), per l'apertura rapida.
#[tauri::command]
pub async fn ide_files(app: State<'_, App>, folder: String) -> Result<Vec<String>> {
    let radice = progetto(&app, &folder)?;
    tokio::task::spawn_blocking(move || {
        let mut v: Vec<String> = match git(&radice, &["ls-files", "--cached", "--others", "--exclude-standard"]) {
            Some(t) => t.lines().map(str::to_string).collect(),
            None => cammina(&radice, 30_000),
        };
        v.truncate(30_000);
        v
    })
    .await
    .map_err(|e| AppError::msg(e.to_string()))
}

/// Ricerca testuale nel progetto: righe che contengono il testo (o l'espressione regolare).
#[tauri::command]
pub async fn ide_search(app: State<'_, App>, folder: String, query: String, regex: Option<bool>, case: Option<bool>) -> Result<Vec<Value>> {
    let radice = progetto(&app, &folder)?;
    if query.trim().is_empty() {
        return Ok(vec![]);
    }
    tokio::task::spawn_blocking(move || {
        let re = {
            let pat = if regex.unwrap_or(false) { query.clone() } else { regex::escape(&query) };
            regex::RegexBuilder::new(&pat).case_insensitive(!case.unwrap_or(false)).build().map_err(|e| AppError::msg(format!("Espressione non valida: {e}")))?
        };
        let file = git(&radice, &["ls-files", "--cached", "--others", "--exclude-standard"]).map(|t| t.lines().map(str::to_string).collect::<Vec<_>>()).unwrap_or_else(|| cammina(&radice, 30_000));
        let mut out = vec![];
        'file: for rel in file {
            let p = radice.join(&rel);
            let Ok(meta) = std::fs::metadata(&p) else { continue };
            if meta.len() > 2 * 1024 * 1024 {
                continue;
            }
            let Ok(testo) = std::fs::read_to_string(&p) else { continue }; // binari e non UTF-8: saltati
            for (i, riga) in testo.lines().enumerate() {
                if let Some(m) = re.find(riga) {
                    let t: String = riga.chars().take(300).collect();
                    out.push(json!({ "file": rel, "path": p.to_string_lossy(), "line": i + 1, "col": riga[..m.start()].chars().count() + 1, "text": t }));
                    if out.len() >= 2000 {
                        break 'file;
                    }
                }
            }
        }
        Ok(out)
    })
    .await
    .map_err(|e| AppError::msg(e.to_string()))?
}

/// Nuovo file (vuoto) o cartella.
#[tauri::command]
pub fn ide_create(app: State<'_, App>, folder: String, path: String, dir: bool) -> Result<String> {
    let radice = progetto(&app, &folder)?;
    let p = dentro(&radice, &path)?;
    if p.exists() {
        return Err(AppError::msg("Esiste già un file o una cartella con questo nome"));
    }
    if dir {
        std::fs::create_dir_all(&p)?;
    } else {
        if let Some(m) = p.parent() {
            std::fs::create_dir_all(m)?;
        }
        std::fs::write(&p, "")?;
    }
    Ok(p.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn ide_rename(app: State<'_, App>, folder: String, from: String, to: String) -> Result<String> {
    let radice = progetto(&app, &folder)?;
    let (a, b) = (dentro(&radice, &from)?, dentro(&radice, &to)?);
    if b.exists() {
        return Err(AppError::msg("Esiste già un file o una cartella con questo nome"));
    }
    std::fs::rename(&a, &b)?;
    Ok(b.to_string_lossy().into_owned())
}

/// Eliminazione: nel Cestino, mai definitiva.
#[tauri::command]
pub fn ide_trash(app: State<'_, App>, folder: String, path: String) -> Result<()> {
    let radice = progetto(&app, &folder)?;
    let p = dentro(&radice, &path)?;
    trash::delete(&p).map_err(|e| AppError::msg(format!("Non riesco a spostarlo nel Cestino: {e}")))
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn i_percorsi_restano_nel_progetto() {
        let r = std::env::temp_dir().join(format!("oltremare-ide-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(r.join("src")).unwrap();
        let r = r.canonicalize().unwrap();
        assert!(dentro(&r, &r.join("src/nuovo.ts").to_string_lossy()).is_ok());
        assert!(dentro(&r, &r.join("../fuori.ts").to_string_lossy()).is_err());
        assert!(dentro(&r, "/etc/passwd").is_err());
        assert!(dentro(&r, &r.to_string_lossy()).is_err(), "la radice stessa non si tocca");
        let _ = std::fs::remove_dir_all(&r);
    }
}

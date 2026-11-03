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

// ---------- Problemi: gli errori del compilatore del progetto ----------

/// Esegue un controllo con un tetto di tempo; restituisce l'uscita (stdout + stderr).
async fn esegui(dir: &Path, cmd: &[&str]) -> Option<String> {
    let mut c = tokio::process::Command::new(cmd[0]);
    c.args(&cmd[1..]).current_dir(dir).env("PATH", crate::shell_env::path_utente()).env("NO_COLOR", "1").kill_on_drop(true);
    let o = tokio::time::timeout(std::time::Duration::from_secs(180), c.output()).await.ok()?.ok()?;
    Some(format!("{}\n{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)))
}

/// Dall'uscita dei controlli ai problemi: file, riga, colonna, gravità, messaggio.
fn leggi_problemi(strumento: &str, dir: &Path, uscita: &str) -> Vec<Value> {
    let tsc = regex::Regex::new(r"^(.+?)\((\d+),(\d+)\): (error|warning) (TS\d+): (.*)$").expect("regex");
    let svelte = regex::Regex::new(r#"^\d+ (ERROR|WARNING) "(.+?)" (\d+):(\d+) "(.*)"$"#).expect("regex");
    let cargo = regex::Regex::new(r"^(.+?):(\d+):(\d+): (error|warning)(?:\[\w+\])?: (.*)$").expect("regex");
    let mut out = vec![];
    for riga in uscita.lines() {
        let (file, line, col, sev, msg) = match strumento {
            "tsc" => match tsc.captures(riga) {
                Some(c) => (c[1].to_string(), c[2].parse().unwrap_or(1), c[3].parse().unwrap_or(1), c[4].to_string(), format!("{} ({})", &c[6], &c[5])),
                None => continue,
            },
            "svelte-check" => match svelte.captures(riga) {
                Some(c) => (c[2].to_string(), c[3].parse().unwrap_or(1), c[4].parse().unwrap_or(1), c[1].to_lowercase(), c[5].replace("\\n", " ")),
                None => continue,
            },
            _ => match cargo.captures(riga) {
                Some(c) => (c[1].to_string(), c[2].parse().unwrap_or(1), c[3].parse().unwrap_or(1), c[4].to_string(), c[5].to_string()),
                None => continue,
            },
        };
        let p = if Path::new(&file).is_absolute() { PathBuf::from(&file) } else { dir.join(&file) };
        out.push(json!({ "path": p.to_string_lossy(), "line": line, "col": col, "severity": if sev == "error" { "error" } else { "warning" }, "message": msg, "tool": strumento }));
    }
    out
}

/// Controlla il progetto con i suoi strumenti (tsc, svelte-check, cargo check), anche nelle sottocartelle dei monorepo.
#[tauri::command]
pub async fn ide_check(app: State<'_, App>, folder: String) -> Result<Value> {
    let radice = progetto(&app, &folder)?;
    let mut problemi = vec![];
    let mut strumenti = vec![];
    for dir in crate::studio::livelli(&radice) {
        let pkg: Value = std::fs::read_to_string(dir.join("package.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
        let ha = |d: &str| pkg["dependencies"].get(d).is_some() || pkg["devDependencies"].get(d).is_some();
        let lavoro: Option<(&str, Vec<&str>)> = if dir.join("Cargo.toml").exists() {
            Some(("cargo", vec!["cargo", "check", "--message-format=short", "--quiet"]))
        } else if ha("svelte-check") {
            Some(("svelte-check", vec!["npx", "--no-install", "svelte-check", "--output", "machine"]))
        } else if ha("typescript") && dir.join("tsconfig.json").exists() {
            Some(("tsc", vec!["npx", "--no-install", "tsc", "--noEmit", "--pretty", "false"]))
        } else {
            None
        };
        let Some((nome, cmd)) = lavoro else { continue };
        if !dir.join("node_modules").exists() && nome != "cargo" {
            continue; // dipendenze non installate: il controllo fallirebbe per quello
        }
        let rel = dir.strip_prefix(&radice).map(|r| r.to_string_lossy().into_owned()).unwrap_or_default();
        strumenti.push(if rel.is_empty() { nome.to_string() } else { format!("{nome} ({rel})") });
        if let Some(u) = esegui(&dir, &cmd).await {
            problemi.extend(leggi_problemi(nome, &dir, &u));
        }
        if problemi.len() > 2000 {
            break;
        }
    }
    problemi.truncate(2000);
    Ok(json!({ "tools": strumenti, "problems": problemi }))
}

fn schema_definizione(name: &str) -> String {
    format!(
        r"(\b(function\*?|class|interface|type|enum|const|let|var|def|fn|struct|trait|mod|impl|func|namespace)\s+{0}\b)|(\b{0}\s*[:=]\s*(async\s*)?(\(|function|\w+\s*=>))|(^\s*(export\s+)?(async\s+)?{0}\s*\([^)]*\)\s*\{{)",
        regex::escape(&name)
    )
}

/// Vai alla definizione (senza language server): cerca dove il nome viene dichiarato.
#[tauri::command]
pub async fn ide_definition(app: State<'_, App>, folder: String, name: String) -> Result<Vec<Value>> {
    if !regex::Regex::new(r"^[A-Za-z_$][\w$]*$").expect("regex").is_match(&name) {
        return Ok(vec![]);
    }
    let pat = schema_definizione(&name);
    let mut v = ide_search(app, folder, pat, Some(true), Some(true)).await?;
    // Prima le dichiarazioni esplicite, poi il resto; niente file di test in cima
    v.sort_by_key(|r| {
        let t = r["text"].as_str().unwrap_or("");
        let f = r["file"].as_str().unwrap_or("");
        (!regex::Regex::new(r"\b(function|class|interface|type|enum|fn|struct|trait|def)\b").expect("regex").is_match(t), f.contains("test") || f.contains("spec"))
    });
    v.truncate(50);
    Ok(v)
}

#[cfg(test)]
mod prove {
    #[test]
    fn trova_le_dichiarazioni() {
        let re = regex::Regex::new(&super::schema_definizione("totale")).unwrap();
        for ok in ["export function totale(righe) {", "const totale = (r) => 1", "fn totale() -> u32 {", "  totale(a, b) {", "class totale {"] {
            assert!(re.is_match(ok), "{ok}");
        }
        assert!(!re.is_match("const x = totale(righe);"));
    }

    #[test]
    fn legge_gli_errori_dei_compilatori() {
        let d = std::path::Path::new("/p");
        let t = super::leggi_problemi("tsc", d, "src/a.ts(3,7): error TS2322: Type 'string' is not assignable to type 'number'.\naltro");
        assert_eq!((t.len(), t[0]["line"].as_u64(), t[0]["path"].as_str()), (1, Some(3), Some("/p/src/a.ts")));
        let s = super::leggi_problemi("svelte-check", d, "1590680326283 ERROR \"src/App.svelte\" 12:5 \"Cannot find name 'x'\"");
        assert_eq!((s[0]["line"].as_u64(), s[0]["severity"].as_str()), (Some(12), Some("error")));
        let c = super::leggi_problemi("cargo", d, "src/main.rs:4:9: warning: unused variable: `a`\nsrc/lib.rs:10:1: error[E0425]: cannot find value `b`");
        assert_eq!((c.len(), c[1]["severity"].as_str()), (2, Some("error")));
    }

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

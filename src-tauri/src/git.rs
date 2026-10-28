//! Git per lo Studio: stato con aree separate (pronte / modificate), stage, ripristino, commit,
//! rami, sincronizzazione con il remoto, stash e cronologia. Ogni comando gira nella cartella del progetto.

use crate::error::{AppError, Result};
use crate::App;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::{AppHandle, Manager, State};

fn progetto(app: &App, p: &str) -> Result<PathBuf> {
    crate::studio::cartella_pub(app, p)
}

async fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let mut c = tokio::process::Command::new("git");
    c.args(args).current_dir(dir).env("PATH", crate::shell_env::path_utente()).env("GIT_TERMINAL_PROMPT", "0").stdin(std::process::Stdio::null()).kill_on_drop(true);
    let o = tokio::time::timeout(Duration::from_secs(120), c.output()).await.map_err(|_| AppError::msg("git non ha risposto in tempo"))?.map_err(|e| AppError::msg(e.to_string()))?;
    if o.status.success() {
        Ok(String::from_utf8_lossy(&o.stdout).into_owned())
    } else {
        let err = String::from_utf8_lossy(&o.stderr).trim().to_string();
        let out = String::from_utf8_lossy(&o.stdout).trim().to_string();
        Err(AppError::msg(if err.is_empty() { out } else { err }))
    }
}

type Stato = (String, String, i64, i64, Vec<Value>, Vec<Value>);

fn analizza_stato(out: &str, dir: &Path) -> Stato {
    let mut ramo = String::new();
    let mut remoto = String::new();
    let (mut avanti, mut indietro) = (0i64, 0i64);
    let mut staged = vec![];
    let mut changed = vec![];
    let mut voci = out.split('\0').filter(|x| !x.is_empty());
    while let Some(v) = voci.next() {
        if let Some(b) = v.strip_prefix("## ") {
            // "main...origin/main [ahead 1, behind 2]" oppure "No commits yet on main"
            let (nomi, conti) = b.split_once(" [").unwrap_or((b, ""));
            let (r, re) = nomi.split_once("...").unwrap_or((nomi, ""));
            ramo = r.trim_start_matches("No commits yet on ").to_string();
            remoto = re.to_string();
            for p in conti.trim_end_matches(']').split(", ") {
                if let Some(n) = p.strip_prefix("ahead ") {
                    avanti = n.parse().unwrap_or(0);
                }
                if let Some(n) = p.strip_prefix("behind ") {
                    indietro = n.parse().unwrap_or(0);
                }
            }
            continue;
        }
        if v.len() < 4 {
            continue;
        }
        let x = &v[0..1];
        let y = &v[1..2];
        let mut file = v[3..].to_string();
        // Con -z la rinomina porta il vecchio nome nel campo successivo
        let vecchio = if x == "R" || x == "C" { voci.next().map(str::to_string) } else { None };
        if file.ends_with('/') {
            file.pop();
        }
        let assoluto = dir.join(&file).to_string_lossy().into_owned();
        if x != " " && x != "?" {
            staged.push(json!({ "rel": file, "path": assoluto, "status": x, "from": vecchio }));
        }
        if y != " " {
            changed.push(json!({ "rel": file, "path": assoluto, "status": if x == "?" { "?" } else { y } }));
        }
    }
    (ramo, remoto, avanti, indietro, staged, changed)
}

/// Stato completo: ramo, avanti/indietro rispetto al remoto, file pronti e modificati.
#[tauri::command]
pub async fn git_state(app: State<'_, App>, project: String) -> Result<Value> {
    let dir = progetto(&app, &project)?;
    if !dir.join(".git").exists() {
        if git(&dir, &["rev-parse", "--is-inside-work-tree"]).await.is_err() {
            return Ok(json!({ "repo": false }));
        }
    }
    let out = git(&dir, &["status", "--porcelain=v1", "-b", "-uall", "-z"]).await?;
    let (ramo, remoto, avanti, indietro, staged, changed) = analizza_stato(&out, &dir);
    let stash = git(&dir, &["stash", "list"]).await.map(|s| s.lines().count()).unwrap_or(0);
    let ultimo = git(&dir, &["log", "-1", "--pretty=%h %s"]).await.map(|s| s.trim().to_string()).unwrap_or_default();
    Ok(json!({ "repo": true, "branch": ramo, "upstream": remoto, "ahead": avanti, "behind": indietro, "staged": staged, "changed": changed, "stash": stash, "last": ultimo }))
}

/// Differenze di un file (o di tutti): non ancora pronte, oppure pronte per il commit.
#[tauri::command]
pub async fn git_diff(app: State<'_, App>, project: String, file: Option<String>, staged: bool) -> Result<String> {
    let dir = progetto(&app, &project)?;
    let mut args = vec!["diff", "--no-color", "--no-ext-diff", "-U3"];
    if staged {
        args.push("--cached");
    }
    let rel;
    if let Some(f) = &file {
        rel = Path::new(f).strip_prefix(&dir).map(|r| r.to_string_lossy().into_owned()).unwrap_or_else(|_| f.clone());
        args.push("--");
        args.push(&rel);
    }
    let mut d = git(&dir, &args).await.unwrap_or_default();
    // File nuovi non tracciati: si mostrano come aggiunti per intero
    if d.trim().is_empty() && !staged {
        if let Some(f) = &file {
            let p = dir.join(Path::new(f).strip_prefix(&dir).unwrap_or(Path::new(f)));
            if let Ok(t) = std::fs::read_to_string(&p) {
                let rel = p.strip_prefix(&dir).unwrap_or(&p).to_string_lossy().into_owned();
                let righe: Vec<&str> = t.lines().take(3000).collect();
                d = format!("diff --git a/{rel} b/{rel}\nnew file mode 100644\n--- /dev/null\n+++ b/{rel}\n@@ -0,0 +1,{} @@\n{}", righe.len(), righe.iter().map(|l| format!("+{l}")).collect::<Vec<_>>().join("\n"));
            }
        }
    }
    Ok(d.chars().take(800_000).collect())
}

fn relativi(dir: &Path, files: &[String]) -> Vec<String> {
    files.iter().map(|f| Path::new(f).strip_prefix(dir).map(|r| r.to_string_lossy().into_owned()).unwrap_or_else(|_| f.clone())).collect()
}

#[tauri::command]
pub async fn git_stage(app: State<'_, App>, project: String, files: Vec<String>) -> Result<()> {
    let dir = progetto(&app, &project)?;
    let r = relativi(&dir, &files);
    let mut args = vec!["add", "--"];
    if r.is_empty() {
        args = vec!["add", "-A"];
    }
    args.extend(r.iter().map(String::as_str));
    git(&dir, &args).await.map(|_| ())
}

#[tauri::command]
pub async fn git_unstage(app: State<'_, App>, project: String, files: Vec<String>) -> Result<()> {
    let dir = progetto(&app, &project)?;
    let r = relativi(&dir, &files);
    let mut args = vec!["restore", "--staged", "--"];
    if r.is_empty() {
        args.push(".");
    }
    args.extend(r.iter().map(String::as_str));
    git(&dir, &args).await.map(|_| ())
}

/// Annulla le modifiche: i file tracciati tornano all'ultimo commit, quelli nuovi vanno nel Cestino (mai cancellati).
#[tauri::command]
pub async fn git_discard(app: State<'_, App>, project: String, files: Vec<String>) -> Result<()> {
    let dir = progetto(&app, &project)?;
    let r = relativi(&dir, &files);
    let nuovi: Vec<String> = git(&dir, &["ls-files", "--others", "--exclude-standard"]).await.unwrap_or_default().lines().map(str::to_string).collect();
    let (da_cestinare, tracciati): (Vec<String>, Vec<String>) = r.into_iter().partition(|f| nuovi.contains(f));
    for f in &da_cestinare {
        trash::delete(dir.join(f)).map_err(|e| AppError::msg(format!("Non riesco a spostare nel Cestino {f}: {e}")))?;
    }
    if !tracciati.is_empty() {
        let mut args = vec!["restore", "--worktree", "--"];
        args.extend(tracciati.iter().map(String::as_str));
        git(&dir, &args).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn git_commit(app: State<'_, App>, project: String, message: String, amend: Option<bool>, all: Option<bool>) -> Result<String> {
    let dir = progetto(&app, &project)?;
    if message.trim().is_empty() && !amend.unwrap_or(false) {
        return Err(AppError::msg("Scrivi il messaggio del commit"));
    }
    if all.unwrap_or(false) {
        git(&dir, &["add", "-A"]).await?;
    }
    let mut args = vec!["commit"];
    if amend.unwrap_or(false) {
        args.push("--amend");
        if message.trim().is_empty() {
            args.push("--no-edit");
        }
    }
    if !message.trim().is_empty() {
        args.push("-m");
        args.push(message.trim());
    }
    git(&dir, &args).await?;
    Ok(git(&dir, &["log", "-1", "--pretty=%h %s"]).await.unwrap_or_default().trim().to_string())
}

/// Sincronizzazione: pull (solo avanzamento rapido), push (crea il ramo remoto se serve), fetch.
#[tauri::command]
pub async fn git_sync(app: State<'_, App>, project: String, action: String) -> Result<String> {
    let dir = progetto(&app, &project)?;
    match action.as_str() {
        "pull" => git(&dir, &["pull", "--ff-only"]).await,
        "fetch" => git(&dir, &["fetch", "--prune"]).await,
        "push" => {
            let su = git(&dir, &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]).await;
            if su.is_ok() {
                git(&dir, &["push"]).await
            } else {
                let ramo = git(&dir, &["branch", "--show-current"]).await?.trim().to_string();
                git(&dir, &["push", "-u", "origin", &ramo]).await
            }
        }
        _ => Err(AppError::msg("Azione sconosciuta")),
    }
    .map(|s| s.trim().to_string())
}

#[tauri::command]
pub async fn git_branches(app: State<'_, App>, project: String) -> Result<Value> {
    let dir = progetto(&app, &project)?;
    let locali = git(&dir, &["branch", "--format=%(refname:short)|%(HEAD)|%(committerdate:relative)|%(upstream:short)"]).await.unwrap_or_default();
    let rami: Vec<Value> = locali
        .lines()
        .filter_map(|l| {
            let p: Vec<&str> = l.split('|').collect();
            (p.len() >= 3).then(|| json!({ "name": p[0], "current": p[1] == "*", "when": p[2], "upstream": p.get(3).copied().unwrap_or("") }))
        })
        .collect();
    Ok(json!({ "branches": rami }))
}

#[tauri::command]
pub async fn git_checkout(app: State<'_, App>, project: String, branch: String, create: Option<bool>) -> Result<()> {
    let dir = progetto(&app, &project)?;
    let b = branch.trim();
    if b.is_empty() || b.contains(' ') || b.starts_with('-') {
        return Err(AppError::msg("Nome del ramo non valido"));
    }
    if create.unwrap_or(false) {
        git(&dir, &["switch", "-c", b]).await.map(|_| ())
    } else {
        git(&dir, &["switch", b]).await.map(|_| ())
    }
}

#[tauri::command]
pub async fn git_stash(app: State<'_, App>, project: String, pop: Option<bool>) -> Result<String> {
    let dir = progetto(&app, &project)?;
    if pop.unwrap_or(false) {
        git(&dir, &["stash", "pop"]).await
    } else {
        git(&dir, &["stash", "push", "-u", "-m", "Oltremare Studio"]).await
    }
    .map(|s| s.trim().to_string())
}

#[tauri::command]
pub async fn git_log(app: State<'_, App>, project: String) -> Result<Vec<Value>> {
    let dir = progetto(&app, &project)?;
    let out = git(&dir, &["log", "-40", "--pretty=format:%h\x1f%an\x1f%ar\x1f%s\x1f%D"]).await.unwrap_or_default();
    Ok(out
        .lines()
        .filter_map(|l| {
            let p: Vec<&str> = l.split('\x1f').collect();
            (p.len() >= 4).then(|| json!({ "hash": p[0], "author": p[1], "when": p[2], "subject": p[3], "refs": p.get(4).copied().unwrap_or("") }))
        })
        .collect())
}

/// Messaggio di commit proposto da Claude (Haiku) sulle modifiche pronte, con la chiave della issue se il ramo ne ha una.
#[tauri::command]
pub async fn git_suggest_message(handle: AppHandle, project: String) -> Result<String> {
    let st = handle.state::<App>();
    let dir = progetto(&st, &project)?;
    let mut diff = git(&dir, &["diff", "--cached", "--no-color", "--stat"]).await.unwrap_or_default();
    diff.push_str(&git(&dir, &["diff", "--cached", "--no-color", "-U1"]).await.unwrap_or_default());
    if diff.trim().is_empty() {
        return Err(AppError::msg("Prima metti dei file tra quelli pronti per il commit"));
    }
    let diff: String = diff.chars().take(24_000).collect();
    let ramo = git(&dir, &["branch", "--show-current"]).await.unwrap_or_default();
    let chiave = regex::Regex::new(r"[A-Z][A-Z0-9]{1,9}-\d+").ok().and_then(|r| r.find(&ramo).map(|m| m.as_str().to_string()));
    let recenti = git(&dir, &["log", "-8", "--pretty=%s"]).await.unwrap_or_default();
    let s = st.store.settings();
    let bin = crate::providers::claude_code::trova(&s.claude_code_path).ok_or_else(|| AppError::msg("Claude Code non è installato"))?;
    let istr = format!(
        "Scrivi il messaggio di commit per queste modifiche. Rispondi SOLO con il messaggio, senza virgolette né spiegazioni. \
Prima riga breve (al massimo 72 caratteri), all'imperativo, nella stessa lingua e nello stesso stile dei commit recenti; se serve, una riga vuota e un elenco breve.{}\n\nCommit recenti:\n{recenti}",
        chiave.as_ref().map(|k| format!(" Inizia con la chiave della issue «{k}».")).unwrap_or_default()
    );
    let mut cmd = crate::providers::claude_code::comando(&bin);
    cmd.current_dir(&dir).args(["-p", &diff, "--output-format", "json", "--model", "haiku", "--tools", "", "--setting-sources", "", "--strict-mcp-config", "--system-prompt", &istr]);
    let out = tokio::time::timeout(Duration::from_secs(60), cmd.output()).await.map_err(|_| AppError::msg("Nessuna risposta in tempo"))?.map_err(|e| AppError::msg(e.to_string()))?;
    let v: Value = serde_json::from_slice(&out.stdout).unwrap_or(Value::Null);
    let m = v["result"].as_str().unwrap_or_default().trim().trim_matches('`').trim().to_string();
    if m.is_empty() {
        return Err(AppError::msg("Claude non ha proposto un messaggio"));
    }
    Ok(m)
}

#[cfg(test)]
mod prove {
    #[test]
    fn analizza_lo_stato() {
        let out = "## main...origin/main [ahead 2, behind 1]\0M  pronto.ts\0 M modificato.ts\0MM entrambi.ts\0?? nuovo.ts\0R  nuovo_nome.ts\0vecchio_nome.ts\0";
        let (ramo, remoto, av, ind, staged, changed) = super::analizza_stato(out, std::path::Path::new("/p"));
        assert_eq!((ramo.as_str(), remoto.as_str(), av, ind), ("main", "origin/main", 2, 1));
        let s: Vec<&str> = staged.iter().map(|v| v["rel"].as_str().unwrap()).collect();
        let c: Vec<&str> = changed.iter().map(|v| v["rel"].as_str().unwrap()).collect();
        assert_eq!(s, ["pronto.ts", "entrambi.ts", "nuovo_nome.ts"]);
        assert_eq!(c, ["modificato.ts", "entrambi.ts", "nuovo.ts"]);
        assert_eq!(staged[2]["from"], "vecchio_nome.ts");
    }

    /// Repository di prova: stato, stage, commit, ripristino nel Cestino.
    #[tokio::test]
    async fn flusso_base() {
        let d = std::env::temp_dir().join(format!("oltremare-git-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&d).unwrap();
        let g = |a: &[&str]| std::process::Command::new("git").args(a).current_dir(&d).output().unwrap();
        g(&["init", "-q", "-b", "main"]);
        g(&["config", "user.email", "t@t"]);
        g(&["config", "user.name", "t"]);
        std::fs::write(d.join("a.txt"), "uno\n").unwrap();
        g(&["add", "."]);
        g(&["commit", "-qm", "init"]);
        std::fs::write(d.join("a.txt"), "uno\ndue\n").unwrap();
        std::fs::write(d.join("nuovo.txt"), "x\n").unwrap();
        let out = super::git(&d, &["status", "--porcelain=v1", "-b", "-uall", "-z"]).await.unwrap();
        assert!(out.contains(" M a.txt") && out.contains("?? nuovo.txt"), "{out}");
        super::git(&d, &["add", "--", "a.txt"]).await.unwrap();
        let diff = super::git(&d, &["diff", "--cached"]).await.unwrap();
        assert!(diff.contains("+due"));
        super::git(&d, &["commit", "-m", "due"]).await.unwrap();
        let log = super::git(&d, &["log", "-1", "--pretty=%s"]).await.unwrap();
        assert_eq!(log.trim(), "due");
        let _ = std::fs::remove_dir_all(&d);
    }
}

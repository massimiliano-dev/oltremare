//! Terminale dello Studio: la shell dell'utente in uno pseudo-terminale, nella cartella del progetto.
//! L'uscita arriva alla finestra come byte (base64) e la disegna xterm.js; i tasti tornano con `term_write`.

use crate::error::{AppError, Result};
use crate::App;
use base64::Engine;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde_json::json;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{LazyLock, Mutex};
use tauri::{AppHandle, Emitter, State};

struct Term {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
}

static TERMINALI: LazyLock<Mutex<HashMap<String, Term>>> = LazyLock::new(Default::default);

fn errore(e: impl std::fmt::Display) -> AppError {
    AppError::msg(format!("Terminale: {e}"))
}

fn shell() -> (String, Vec<&'static str>) {
    if cfg!(windows) {
        return ("powershell.exe".into(), vec!["-NoLogo"]);
    }
    // Le app aperte dal Finder non sempre hanno $SHELL: su macOS la shell predefinita è zsh
    let s = std::env::var("SHELL").ok().filter(|s| !s.is_empty() && std::path::Path::new(s).exists()).unwrap_or_else(|| "/bin/zsh".into());
    (s, vec!["-l"])
}

/// Apre un terminale nella cartella del progetto e ne restituisce l'id.
#[tauri::command]
pub fn term_open(handle: AppHandle, app: State<'_, App>, project: String, cols: u16, rows: u16) -> Result<String> {
    let dir = crate::studio::cartella_pub(&app, &project)?;
    let pair = native_pty_system().openpty(PtySize { rows: rows.max(5), cols: cols.max(20), pixel_width: 0, pixel_height: 0 }).map_err(errore)?;
    let (bin, args) = shell();
    let mut cmd = CommandBuilder::new(bin);
    cmd.args(args);
    cmd.cwd(&dir);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.env("PATH", crate::shell_env::path_utente());
    if std::env::var("LANG").is_err() {
        cmd.env("LANG", "it_IT.UTF-8");
    }
    let child = pair.slave.spawn_command(cmd).map_err(errore)?;
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().map_err(errore)?;
    let writer = pair.master.take_writer().map_err(errore)?;
    let id = uuid::Uuid::new_v4().to_string();
    TERMINALI.lock().unwrap_or_else(|e| e.into_inner()).insert(id.clone(), Term { master: pair.master, writer, child });

    let id2 = id.clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 16 * 1024];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let data = base64::engine::general_purpose::STANDARD.encode(&buf[..n]);
                    let _ = handle.emit("term-data", json!({ "id": id2, "data": data }));
                }
            }
        }
        let rimasto = TERMINALI.lock().unwrap_or_else(|e| e.into_inner()).remove(&id2);
        let codice = rimasto.and_then(|mut t| t.child.wait().ok()).map(|s| s.exit_code());
        let _ = handle.emit("term-exit", json!({ "id": id2, "code": codice }));
    });
    Ok(id)
}

/// Tasti e testo incollato verso la shell.
#[tauri::command]
pub fn term_write(id: String, data: String) -> Result<()> {
    let mut m = TERMINALI.lock().unwrap_or_else(|e| e.into_inner());
    let t = m.get_mut(&id).ok_or_else(|| AppError::msg("Terminale chiuso"))?;
    t.writer.write_all(data.as_bytes()).and_then(|_| t.writer.flush()).map_err(errore)
}

#[tauri::command]
pub fn term_resize(id: String, cols: u16, rows: u16) -> Result<()> {
    let m = TERMINALI.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(t) = m.get(&id) {
        t.master.resize(PtySize { rows: rows.max(5), cols: cols.max(20), pixel_width: 0, pixel_height: 0 }).map_err(errore)?;
    }
    Ok(())
}

/// Chiude il terminale (e ferma quello che ci gira dentro).
#[tauri::command]
pub fn term_close(id: String) -> Result<()> {
    if let Some(mut t) = TERMINALI.lock().unwrap_or_else(|e| e.into_inner()).remove(&id) {
        let _ = t.child.kill();
    }
    Ok(())
}

/// All'uscita dall'app non restano shell orfane.
pub fn chiudi_tutti() {
    for (_, mut t) in TERMINALI.lock().unwrap_or_else(|e| e.into_inner()).drain() {
        let _ = t.child.kill();
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn la_shell_risponde_nello_pseudo_terminale() {
        let pair = native_pty_system().openpty(PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 }).unwrap();
        let (bin, _) = shell();
        let mut cmd = CommandBuilder::new(bin);
        cmd.args(["-c", "echo ciao-$((40+2))"]);
        let mut child = pair.slave.spawn_command(cmd).unwrap();
        drop(pair.slave);
        let mut r = pair.master.try_clone_reader().unwrap();
        let mut out = String::new();
        let mut buf = [0u8; 1024];
        while let Ok(n) = r.read(&mut buf) {
            if n == 0 {
                break;
            }
            out.push_str(&String::from_utf8_lossy(&buf[..n]));
            if out.contains("ciao-42") {
                break;
            }
        }
        let _ = child.wait();
        assert!(out.contains("ciao-42"), "uscita: {out:?}");
    }
}

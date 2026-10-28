//! Ambiente della shell dell'utente: le app aperte dal Finder ricevono un PATH minimo.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::OnceLock;

pub fn home() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}

/// PATH della shell di login (letto una volta sola).
pub fn path_utente() -> &'static str {
    static P: OnceLock<String> = OnceLock::new();
    P.get_or_init(|| {
        let base = std::env::var("PATH").unwrap_or_default();
        if cfg!(windows) {
            return base;
        }
        let out = std::process::Command::new("/bin/zsh")
            .args(["-ilc", "printf %s \"$PATH\""])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output();
        match out {
            Ok(o) if o.status.success() && !o.stdout.is_empty() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
            _ => base,
        }
    })
}

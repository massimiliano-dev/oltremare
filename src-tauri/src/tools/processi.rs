//! Processi in background (backend, server di sviluppo, watcher): partono, restano vivi mentre
//! l'agente lavora, se ne leggono i log e gli indirizzi aperti, si fermano. Si chiudono con l'app.

use super::percorsi::{consentito, mostra};
use super::shell::bloccato;
use super::{str_arg, Esito};
use crate::shell_env::{home, path_utente};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};

const RIGHE_MAX: usize = 3000;

struct Processo {
    nome: String,
    comando: String,
    cartella: PathBuf,
    avvio: Instant,
    pid: Option<u32>,
    log: Arc<Mutex<VecDeque<String>>>,
    uscita: Arc<Mutex<Option<i32>>>,
    figlio: Arc<tokio::sync::Mutex<tokio::process::Child>>,
}

static PROCESSI: LazyLock<Mutex<HashMap<String, Processo>>> = LazyLock::new(Default::default);

fn indirizzi(log: &VecDeque<String>) -> Vec<String> {
    static RE: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"https?://(?:localhost|127\.0\.0\.1|0\.0\.0\.0|\[::1?\])(?::\d+)?[^\s\x1b'\x22)]*").unwrap());
    let mut v: Vec<String> = vec![];
    for r in log.iter() {
        for m in RE.find_iter(r) {
            let u = m.as_str().trim_end_matches(['.', ',', ';']).replace("0.0.0.0", "localhost");
            if !v.contains(&u) {
                v.push(u);
            }
        }
    }
    v.into_iter().take(6).collect()
}

/// Toglie i codici di colore dei terminali.
fn pulisci(s: &str) -> String {
    static RE: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"\x1b\[[0-9;?]*[A-Za-z]|\x1b\][^\x07]*\x07").unwrap());
    RE.replace_all(s, "").replace('\r', "")
}

fn coda(log: &Mutex<VecDeque<String>>, n: usize) -> String {
    let l = log.lock().unwrap_or_else(|e| e.into_inner());
    let salta = l.len().saturating_sub(n);
    l.iter().skip(salta).cloned().collect::<Vec<_>>().join("\n")
}

async fn avvia(input: &Value, radici: &[PathBuf]) -> Esito {
    let comando = str_arg(input, "comando").trim().to_string();
    if comando.is_empty() {
        return Esito::errore("Comando vuoto");
    }
    if let Some(m) = bloccato(&comando) {
        return Esito::errore(format!("Comando bloccato da Oltremare: {m}"));
    }
    let cartella = match consentito(str_arg(input, "cartella"), radici) {
        Ok(p) if p.is_dir() => p,
        Ok(p) => return Esito::errore(format!("{} non è una cartella", mostra(&p))),
        Err(e) => return Esito::errore(e),
    };
    let nome = Some(str_arg(input, "nome").trim()).filter(|n| !n.is_empty()).map(str::to_string).unwrap_or_else(|| comando.chars().take(40).collect());
    // Lo stesso nome nella stessa cartella: si ferma il precedente (riavvio)
    let vecchio = {
        let m = PROCESSI.lock().unwrap_or_else(|e| e.into_inner());
        m.iter().find(|(_, p)| p.nome == nome && p.cartella == cartella).map(|(id, _)| id.clone())
    };
    if let Some(id) = vecchio {
        ferma(&id).await;
    }

    #[cfg(windows)]
    let mut cmd = {
        let mut c = tokio::process::Command::new("powershell.exe");
        c.args(["-NoProfile", "-NonInteractive", "-Command"]).arg(&comando);
        c.creation_flags(0x0800_0000);
        c
    };
    #[cfg(not(windows))]
    let mut cmd = {
        // Shell di login: come nel Terminale (nvm, pyenv, variabili del profilo)
        let mut c = tokio::process::Command::new("/bin/zsh");
        c.arg("-lc").arg(&comando).process_group(0);
        c
    };
    cmd.current_dir(&cartella).env("PATH", path_utente()).env("FORCE_COLOR", "0").env("BROWSER", "none").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    if let Some(h) = home() {
        cmd.env(if cfg!(windows) { "USERPROFILE" } else { "HOME" }, h);
    }
    let mut figlio = match cmd.spawn() {
        Ok(f) => f,
        Err(e) => return Esito::errore(format!("Non riesco ad avviare il processo: {e}")),
    };
    let pid = figlio.id();
    let log: Arc<Mutex<VecDeque<String>>> = Default::default();
    for flusso in [figlio.stdout.take().map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Unpin + Send>), figlio.stderr.take().map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Unpin + Send>)].into_iter().flatten() {
        let log = log.clone();
        tokio::spawn(async move {
            let mut righe = BufReader::new(flusso).lines();
            while let Ok(Some(r)) = righe.next_line().await {
                let mut l = log.lock().unwrap_or_else(|e| e.into_inner());
                l.push_back(pulisci(&r));
                while l.len() > RIGHE_MAX {
                    l.pop_front();
                }
            }
        });
    }
    let uscita: Arc<Mutex<Option<i32>>> = Default::default();
    let figlio = Arc::new(tokio::sync::Mutex::new(figlio));
    {
        let (f, u) = (figlio.clone(), uscita.clone());
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(500)).await;
                let mut g = f.lock().await;
                match g.try_wait() {
                    Ok(Some(s)) => {
                        *u.lock().unwrap_or_else(|e| e.into_inner()) = Some(s.code().unwrap_or(-1));
                        break;
                    }
                    Ok(None) => {}
                    Err(_) => break,
                }
            }
        });
    }
    let id = uuid::Uuid::new_v4().simple().to_string()[..8].to_string();
    if let Some(app) = crate::APP.get() {
        crate::dev::segnala_progetto(app, &cartella);
    }
    PROCESSI.lock().unwrap_or_else(|e| e.into_inner()).insert(id.clone(), Processo { nome: nome.clone(), comando: comando.clone(), cartella: cartella.clone(), avvio: Instant::now(), pid, log: log.clone(), uscita: uscita.clone(), figlio });

    // Si aspetta che il processo si presenti: un indirizzo nei log, un'uscita, o al massimo il tempo dato
    let attesa = input["attendi_secondi"].as_u64().unwrap_or(15).clamp(1, 120);
    let inizio = Instant::now();
    while inizio.elapsed() < Duration::from_secs(attesa) {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if uscita.lock().map(|u| u.is_some()).unwrap_or(false) || !indirizzi(&log.lock().unwrap_or_else(|e| e.into_inner())).is_empty() {
            tokio::time::sleep(Duration::from_millis(800)).await;
            break;
        }
    }
    let url = indirizzi(&log.lock().unwrap_or_else(|e| e.into_inner()));
    let finito = *uscita.lock().unwrap_or_else(|e| e.into_inner());
    let testo = format!(
        "Processo «{nome}» (id {id}) in {}{}{}\n--- ultime righe ---\n{}",
        mostra(&cartella),
        match finito {
            Some(c) => format!(" — è già terminato (uscita {c}): se doveva restare acceso, leggi l'errore qui sotto"),
            None => " — in esecuzione".into(),
        },
        if url.is_empty() { String::new() } else { format!("\nIndirizzi: {}", url.join(", ")) },
        coda(&log, 40)
    );
    let vista = json!({ "kind": "shell", "command": comando, "cwd": mostra(&cartella), "exitCode": finito.unwrap_or(0), "stdout": coda(&log, 40), "stderr": "", "truncated": false, "timedOut": false });
    if finito.is_some_and(|c| c != 0) {
        let mut e = Esito::errore(testo);
        e.risultato = vista;
        e
    } else {
        Esito::ok(testo, vista)
    }
}

async fn ferma(id: &str) -> bool {
    let p = PROCESSI.lock().unwrap_or_else(|e| e.into_inner()).remove(id);
    match p {
        Some(p) => {
            let mut f = p.figlio.lock().await;
            super::shell::uccidi(&mut f, p.pid).await;
            true
        }
        None => false,
    }
}

/// Alla chiusura dell'app: niente server orfani.
pub fn ferma_tutti() {
    let tutti: Vec<Processo> = PROCESSI.lock().unwrap_or_else(|e| e.into_inner()).drain().map(|(_, p)| p).collect();
    for p in tutti {
        #[cfg(unix)]
        if let Some(pid) = p.pid {
            unsafe {
                libc::kill(-(pid as i32), libc::SIGKILL);
            }
        }
        if let Ok(mut f) = p.figlio.try_lock() {
            let _ = f.start_kill();
        }
    }
}

/// Per la vista Sviluppo: processi con stato, indirizzi e ultime righe.
pub fn vista() -> Vec<Value> {
    let m = PROCESSI.lock().unwrap_or_else(|e| e.into_inner());
    let mut v: Vec<(Instant, Value)> = m
        .iter()
        .map(|(id, p)| {
            let u = *p.uscita.lock().unwrap_or_else(|e| e.into_inner());
            (p.avvio, json!({
                "id": id, "name": p.nome, "command": p.comando, "folder": p.cartella.to_string_lossy(),
                "running": u.is_none(), "exitCode": u, "urls": indirizzi(&p.log.lock().unwrap_or_else(|e| e.into_inner())),
                "log": coda(&p.log, 400),
            }))
        })
        .collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v.into_iter().map(|(_, x)| x).collect()
}

fn elenco() -> Esito {
    let m = PROCESSI.lock().unwrap_or_else(|e| e.into_inner());
    if m.is_empty() {
        return Esito::ok("Nessun processo in background.", json!({ "kind": "text", "text": "Nessun processo" }));
    }
    let righe: Vec<String> = m
        .iter()
        .map(|(id, p)| {
            let u = *p.uscita.lock().unwrap_or_else(|e| e.into_inner());
            let url = indirizzi(&p.log.lock().unwrap_or_else(|e| e.into_inner()));
            format!(
                "• {} (id {id}) — {} — {} — attivo da {} s{}",
                p.nome,
                match u {
                    Some(c) => format!("terminato (uscita {c})"),
                    None => "in esecuzione".into(),
                },
                mostra(&p.cartella),
                p.avvio.elapsed().as_secs(),
                if url.is_empty() { String::new() } else { format!(" — {}", url.join(", ")) }
            ) + &format!("\n  $ {}", p.comando)
        })
        .collect();
    let t = righe.join("\n");
    Esito::ok(t.clone(), json!({ "kind": "text", "text": t }))
}

fn log_di(input: &Value) -> Esito {
    let id = str_arg(input, "id");
    let m = PROCESSI.lock().unwrap_or_else(|e| e.into_inner());
    let Some(p) = m.get(id).or_else(|| m.values().find(|p| p.nome == id)) else { return Esito::errore(format!("Nessun processo con id «{id}»: usa «processi» per l'elenco")) };
    let n = input["righe"].as_u64().unwrap_or(80).clamp(5, 1000) as usize;
    let mut t = coda(&p.log, n);
    if let Some(f) = Some(str_arg(input, "filtro")).filter(|f| !f.is_empty()) {
        let f = f.to_lowercase();
        t = t.lines().filter(|l| l.to_lowercase().contains(&f)).collect::<Vec<_>>().join("\n");
    }
    let stato = match *p.uscita.lock().unwrap_or_else(|e| e.into_inner()) {
        Some(c) => format!("terminato (uscita {c})"),
        None => "in esecuzione".into(),
    };
    let testo = format!("«{}» — {stato}\n{}", p.nome, if t.is_empty() { "(nessuna riga)".into() } else { t });
    Esito::ok(testo.clone(), json!({ "kind": "text", "text": testo.chars().take(6000).collect::<String>() }))
}

pub async fn esegui(nome: &str, input: &Value, radici: &[PathBuf]) -> Esito {
    match nome {
        "avvia_processo" => avvia(input, radici).await,
        "processi" => elenco(),
        "log_processo" => log_di(input),
        "ferma_processo" => {
            let id = str_arg(input, "id");
            let vero = {
                let m = PROCESSI.lock().unwrap_or_else(|e| e.into_inner());
                if m.contains_key(id) { Some(id.to_string()) } else { m.iter().find(|(_, p)| p.nome == id).map(|(k, _)| k.clone()) }
            };
            match vero {
                Some(k) if ferma(&k).await => Esito::ok("Processo fermato.", json!({ "kind": "text", "text": "Fermato" })),
                _ => Esito::errore(format!("Nessun processo con id «{id}»")),
            }
        }
        _ => Esito::errore(format!("Strumento sconosciuto: {nome}")),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn trova_gli_indirizzi() {
        let l: std::collections::VecDeque<String> = ["  ➜  Local:   http://localhost:5173/", "Server listening on http://0.0.0.0:3000.", "niente"].iter().map(|s| s.to_string()).collect();
        assert_eq!(super::indirizzi(&l), vec!["http://localhost:5173/", "http://localhost:3000"]);
        assert_eq!(super::pulisci("\x1b[32mok\x1b[0m\r"), "ok");
    }

    #[tokio::test]
    async fn avvia_legge_e_ferma() {
        let dir = std::env::temp_dir();
        let radici = vec![dir.canonicalize().unwrap()];
        let e = super::avvia(&serde_json::json!({ "comando": "echo pronto su http://localhost:4321; sleep 30", "cartella": dir.to_string_lossy(), "nome": "prova", "attendi_secondi": 5 }), &radici).await;
        assert!(!e.is_error, "{}", e.testo);
        assert!(e.testo.contains("http://localhost:4321"), "{}", e.testo);
        assert!(super::elenco().testo.contains("prova"));
        let r = super::esegui("ferma_processo", &serde_json::json!({ "id": "prova" }), &radici).await;
        assert!(!r.is_error, "{}", r.testo);
    }
}

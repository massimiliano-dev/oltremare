//! AI locale: il motore llama.cpp (`llama-server`) gira come processo separato, scaricato al
//! primo uso dalla release ufficiale (build fissata, impronta verificata). Parla l'API compatibile
//! OpenAI su 127.0.0.1, con una porta e una chiave casuali a ogni avvio.

pub mod binario;

use crate::error::{AppError, Result};
use crate::modelli::catalogo;
use crate::App;
use serde::Serialize;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};

#[derive(Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Stato {
    /// "stopped" | "installing" | "starting" | "loading" | "ready" | "error"
    pub state: String,
    pub port: Option<u16>,
    pub build: String,
    pub backend: String,
    pub model: Option<String>,
    pub message: Option<String>,
    pub installed: bool,
}

struct Processo {
    child: tokio::process::Child,
    porta: u16,
    chiave: String,
    modello: String,
}

pub struct Sidecar {
    stato: Mutex<Stato>,
    processo: tokio::sync::Mutex<Option<Processo>>,
    log: Mutex<VecDeque<String>>,
}

impl Default for Sidecar {
    fn default() -> Self {
        Self {
            stato: Mutex::new(Stato { state: "stopped".into(), build: binario::BUILD.into(), backend: binario::backend().into(), ..Default::default() }),
            processo: tokio::sync::Mutex::new(None),
            log: Mutex::new(VecDeque::new()),
        }
    }
}

pub fn cartella_modelli(app: &AppHandle) -> PathBuf {
    app.state::<App>().data_dir.join("modelli")
}

/// Percorsi dei file di un modello del catalogo, se scaricati.
pub fn file_modello(app: &AppHandle, id: &str) -> Option<(PathBuf, Option<PathBuf>)> {
    let dir = cartella_modelli(app);
    if let Some(nome) = id.strip_prefix("custom:") {
        let p = dir.join("personali").join(nome);
        return p.is_file().then_some((p, None));
    }
    let v = catalogo::voce(id)?;
    let base = dir.join(v.id);
    let principale = base.join(v.files.first()?);
    if !principale.is_file() {
        return None;
    }
    let mmproj = v.files.get(1).map(|f| base.join(f)).filter(|p| p.is_file());
    Some((principale, mmproj))
}

impl Sidecar {
    pub fn stato(&self) -> Stato {
        self.stato.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn imposta(&self, app: &AppHandle, f: impl FnOnce(&mut Stato)) {
        let mut s = self.stato.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut s);
        s.installed = binario::installato(app);
        let _ = app.emit("sidecar-status", s.clone());
    }

    pub fn log(&self) -> Vec<String> {
        self.log.lock().unwrap_or_else(|e| e.into_inner()).iter().cloned().collect()
    }

    fn scrivi_log(&self, riga: String) {
        let mut l = self.log.lock().unwrap_or_else(|e| e.into_inner());
        if l.len() >= 300 {
            l.pop_front();
        }
        l.push_back(riga);
    }

    /// Indirizzo e chiave del motore con il modello richiesto: lo avvia o lo cambia se serve.
    pub async fn endpoint(&self, app: &AppHandle, modello: &str) -> Result<(String, String)> {
        {
            let mut p = self.processo.lock().await;
            if let Some(proc) = p.as_mut() {
                let vivo = matches!(proc.child.try_wait(), Ok(None));
                if vivo && proc.modello == modello && self.stato().state == "ready" {
                    return Ok((format!("http://127.0.0.1:{}/v1", proc.porta), proc.chiave.clone()));
                }
            }
        }
        self.avvia(app, modello).await?;
        let p = self.processo.lock().await;
        let proc = p.as_ref().ok_or_else(|| AppError::msg("Il motore locale non è partito"))?;
        Ok((format!("http://127.0.0.1:{}/v1", proc.porta), proc.chiave.clone()))
    }

    pub async fn avvia(&self, app: &AppHandle, modello: &str) -> Result<()> {
        let mut guard = self.processo.lock().await;
        if let Some(mut vecchio) = guard.take() {
            let _ = vecchio.child.kill().await;
        }
        let (gguf, mmproj) = file_modello(app, modello)
            .ok_or_else(|| AppError::msg("Il modello locale non è ancora scaricato: scaricalo in Impostazioni → AI locale"))?;
        let ctx = catalogo::voce(modello).map_or(16384, |v| v.ctx);
        let bin = match binario::percorso(app) {
            Some(b) => b,
            None => {
                self.imposta(app, |s| {
                    s.state = "installing".into();
                    s.message = Some("Scarico il motore llama.cpp…".into());
                });
                match binario::installa(app).await {
                    Ok(b) => b,
                    Err(e) => {
                        self.imposta(app, |s| {
                            s.state = "error".into();
                            s.message = Some(e.to_string());
                        });
                        return Err(e);
                    }
                }
            }
        };
        self.imposta(app, |s| {
            s.state = "starting".into();
            s.model = Some(modello.into());
            s.message = None;
        });
        let porta = std::net::TcpListener::bind("127.0.0.1:0")?.local_addr()?.port();
        let chiave = format!("{}", uuid::Uuid::new_v4().simple());
        let mut cmd = tokio::process::Command::new(&bin);
        cmd.current_dir(bin.parent().unwrap_or(&bin))
            .arg("-m")
            .arg(&gguf)
            .args(["--host", "127.0.0.1", "--port", &porta.to_string(), "--api-key", &chiave, "--alias", modello])
            .args(["--jinja", "-ngl", "999", "-c", &ctx.to_string(), "-fa", "auto", "--parallel", "1", "--reasoning-format", "deepseek"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let Some(mm) = &mmproj {
            cmd.arg("--mmproj").arg(mm);
        }
        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000);
        let mut child = cmd.spawn().map_err(|e| AppError::msg(format!("Il motore locale non si avvia: {e}")))?;
        for stream in [child.stdout.take().map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Unpin + Send>), child.stderr.take().map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Unpin + Send>)].into_iter().flatten() {
            let app2 = app.clone();
            tauri::async_runtime::spawn(async move {
                let mut righe = BufReader::new(stream).lines();
                while let Ok(Some(r)) = righe.next_line().await {
                    app2.state::<App>().sidecar.scrivi_log(r);
                }
            });
        }
        *guard = Some(Processo { child, porta, chiave: chiave.clone(), modello: modello.to_string() });
        drop(guard);
        self.imposta(app, |s| {
            s.state = "loading".into();
            s.port = Some(porta);
            s.message = Some("Carico il modello in memoria…".into());
        });

        // Attende che il modello sia caricato: 30 s più 3 s per GB
        let gb = std::fs::metadata(&gguf).map(|m| m.len()).unwrap_or(0) / 1_000_000_000;
        let scadenza = std::time::Instant::now() + Duration::from_secs(30 + gb * 3);
        let url = format!("http://127.0.0.1:{porta}/health");
        loop {
            {
                let mut p = self.processo.lock().await;
                if let Some(proc) = p.as_mut() {
                    if let Ok(Some(stato)) = proc.child.try_wait() {
                        *p = None;
                        let ultime: Vec<String> = self.log().into_iter().rev().take(6).collect::<Vec<_>>().into_iter().rev().collect();
                        let msg = format!("Il motore locale si è chiuso ({stato}). {}", ultime.join(" · "));
                        self.imposta(app, |s| {
                            s.state = "error".into();
                            s.message = Some(msg.clone());
                        });
                        return Err(AppError::msg(msg));
                    }
                }
            }
            if let Ok(r) = crate::providers::client().get(&url).bearer_auth(&chiave).timeout(Duration::from_secs(2)).send().await {
                if r.status().is_success() {
                    break;
                }
            }
            if std::time::Instant::now() > scadenza {
                self.ferma(app).await;
                let msg = "Il modello impiega troppo a caricarsi: prova un modello più piccolo".to_string();
                self.imposta(app, |s| {
                    s.state = "error".into();
                    s.message = Some(msg.clone());
                });
                return Err(AppError::msg(msg));
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        self.imposta(app, |s| {
            s.state = "ready".into();
            s.message = None;
        });
        Ok(())
    }

    pub async fn ferma(&self, app: &AppHandle) {
        if let Some(mut p) = self.processo.lock().await.take() {
            let _ = p.child.kill().await;
        }
        self.imposta(app, |s| {
            s.state = "stopped".into();
            s.port = None;
            s.message = None;
        });
    }

    /// Alla chiusura dell'app: niente processi orfani.
    pub fn ferma_subito(&self) {
        if let Ok(mut g) = self.processo.try_lock() {
            if let Some(mut p) = g.take() {
                let _ = p.child.start_kill();
            }
        }
    }
}

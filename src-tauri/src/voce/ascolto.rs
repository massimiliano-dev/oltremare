//! Ascolto: microfono → rilevamento del parlato (Silero) → trascrizione (Whisper).
//! In attesa, una frase breve che inizia con «Ehi Oltremare» attiva l'assistente; con il tasto
//! (o il pulsante del microfono) si registra direttamente il comando.

use super::cattura::{self, RATE};
use super::testo;
use crate::avatar::Stato as Volto;
use crate::App;
use serde_json::json;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

pub enum Cmd {
    /// Tasto premuto / rilasciato
    Ptt(bool),
    /// Ascolta un comando (pulsante del microfono): finisce col silenzio
    Comando,
    /// Annulla l'ascolto in corso
    Annulla,
    Ferma,
}

#[derive(PartialEq)]
enum Modo {
    Attesa,
    Comando { scadenza: Instant },
    Ptt,
}

pub struct Config {
    pub cartella: PathBuf,
    pub stt: String,
    pub dispositivo: String,
    /// Ascolta sempre la parola di attivazione
    pub parola: bool,
}

fn whisper(cfg: &Config) -> Result<sherpa_rs::whisper::WhisperRecognizer, String> {
    let m = super::modelli::voce(&cfg.stt).ok_or("Modello di riconoscimento sconosciuto")?;
    let dir = cfg.cartella.join(m.id);
    let p = |i: usize| dir.join(m.files[i]).to_string_lossy().into_owned();
    if !dir.join(m.files[0]).is_file() {
        return Err("Scarica il riconoscimento vocale in Impostazioni → Voce".into());
    }
    sherpa_rs::whisper::WhisperRecognizer::new(sherpa_rs::whisper::WhisperConfig {
        encoder: p(0),
        decoder: p(1),
        tokens: p(2),
        language: "it".into(),
        // Riempimento finale automatico: senza, Whisper perde le ultime parole
        tail_paddings: Some(-1),
        num_threads: Some(4),
        provider: Some("cpu".into()),
        ..Default::default()
    })
    .map_err(|e| format!("Riconoscimento non disponibile: {e}"))
}

fn vad(cfg: &Config, silenzio: f32) -> Result<sherpa_rs::silero_vad::SileroVad, String> {
    let p = cfg.cartella.join("silero-vad").join("silero_vad.onnx");
    if !p.is_file() {
        return Err("Scarica il rilevamento del parlato in Impostazioni → Voce".into());
    }
    sherpa_rs::silero_vad::SileroVad::new(
        sherpa_rs::silero_vad::SileroVadConfig {
            model: p.to_string_lossy().into_owned(),
            min_silence_duration: silenzio,
            min_speech_duration: 0.25,
            max_speech_duration: 20.0,
            threshold: 0.5,
            sample_rate: RATE,
            window_size: 512,
            ..Default::default()
        },
        60.0,
    )
    .map_err(|e| format!("Rilevamento del parlato non disponibile: {e}"))
}

fn stato(app: &AppHandle, ascolto: bool, registra: bool, errore: Option<&str>) {
    let _ = app.emit("voice-status", json!({ "listening": ascolto, "recording": registra, "error": errore }));
}

/// Il thread di ascolto. Restituisce quando riceve `Ferma` o si chiude il canale.
pub fn avvia(app: AppHandle, cfg: Config, rx: Receiver<Cmd>) {
    std::thread::Builder::new()
        .name("oltremare-ascolto".into())
        .spawn(move || {
            if let Err(e) = ciclo(&app, &cfg, rx) {
                stato(&app, false, false, Some(&e));
                let _ = app.emit("voice-error", e);
            }
            app.state::<App>().avatar.clear(&app, "voce");
        })
        .ok();
}

fn ciclo(app: &AppHandle, cfg: &Config, rx: Receiver<Cmd>) -> Result<(), String> {
    let mut asr = whisper(cfg)?;
    let mut vad_attesa = vad(cfg, 0.5)?;
    let mut vad_comando = vad(cfg, 1.1)?;
    let (tx_audio, rx_audio): (Sender<Vec<f32>>, Receiver<Vec<f32>>) = std::sync::mpsc::channel();
    let _mic = cattura::avvia(&cfg.dispositivo, tx_audio)?;
    let st = app.state::<App>();
    let mut modo = if cfg.parola { Modo::Attesa } else { Modo::Comando { scadenza: Instant::now() + Duration::from_secs(8) } };
    let mut ptt: Vec<f32> = Vec::new();
    let mut ultimo_livello = Instant::now();
    stato(app, cfg.parola, modo != Modo::Attesa, None);
    if modo != Modo::Attesa {
        st.avatar.set(app, "voce", Volto::Listening);
    }

    let entra_comando = |modo: &mut Modo, vad: &mut sherpa_rs::silero_vad::SileroVad| {
        vad.clear();
        *modo = Modo::Comando { scadenza: Instant::now() + Duration::from_secs(8) };
        st.avatar.set(app, "voce", Volto::Listening);
        stato(app, cfg.parola, true, None);
        let _ = app.emit("voice-wakeword", ());
    };
    let esci = |modo: &mut Modo| {
        *modo = Modo::Attesa;
        st.avatar.clear(app, "voce");
        stato(app, cfg.parola, false, None);
    };

    loop {
        // Comandi
        while let Ok(c) = rx.try_recv() {
            match c {
                Cmd::Ferma => return Ok(()),
                Cmd::Ptt(true) => {
                    st.voce.ferma_lettura();
                    ptt.clear();
                    modo = Modo::Ptt;
                    st.avatar.set(app, "voce", Volto::Listening);
                    stato(app, cfg.parola, true, None);
                }
                Cmd::Ptt(false) => {
                    if modo == Modo::Ptt {
                        let audio = std::mem::take(&mut ptt);
                        esci(&mut modo);
                        if audio.len() > RATE as usize / 3 {
                            trascrivi_e_invia(app, &mut asr, &audio, false);
                        }
                    }
                }
                Cmd::Comando => {
                    st.voce.ferma_lettura();
                    entra_comando(&mut modo, &mut vad_comando);
                }
                Cmd::Annulla => esci(&mut modo),
            }
        }
        if !cfg.parola && modo == Modo::Attesa {
            // Ascolto "una tantum" finito
            return Ok(());
        }

        let blocco = match rx_audio.recv_timeout(Duration::from_millis(100)) {
            Ok(b) => b,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => return Err("Il microfono si è interrotto".into()),
        };
        // Bocca e antenna dell'avatar seguono la voce mentre ascolta
        if modo != Modo::Attesa && ultimo_livello.elapsed() >= Duration::from_millis(33) {
            ultimo_livello = Instant::now();
            st.avatar.livello(app, cattura::livello(&blocco));
        }

        match &modo {
            Modo::Ptt => {
                ptt.extend_from_slice(&blocco);
                // Al massimo 60 secondi di registrazione
                if ptt.len() > RATE as usize * 60 {
                    let audio = std::mem::take(&mut ptt);
                    esci(&mut modo);
                    trascrivi_e_invia(app, &mut asr, &audio, false);
                }
            }
            Modo::Comando { scadenza } => {
                let scaduto = Instant::now() > *scadenza && !vad_comando.is_speech();
                vad_comando.accept_waveform(blocco);
                if vad_comando.is_speech() {
                    // Chi parla allunga l'ascolto
                    modo = Modo::Comando { scadenza: Instant::now() + Duration::from_secs(8) };
                }
                if !vad_comando.is_empty() {
                    let seg = vad_comando.front();
                    vad_comando.pop();
                    vad_comando.clear();
                    esci(&mut modo);
                    trascrivi_e_invia(app, &mut asr, &seg.samples, false);
                } else if scaduto {
                    esci(&mut modo);
                }
            }
            Modo::Attesa => {
                vad_attesa.accept_waveform(blocco);
                while !vad_attesa.is_empty() {
                    let seg = vad_attesa.front();
                    vad_attesa.pop();
                    // Solo frasi brevi possono iniziare con la parola di attivazione
                    if seg.samples.len() > RATE as usize * 12 {
                        continue;
                    }
                    let t = asr.transcribe(RATE, &seg.samples).text;
                    let t = t.trim();
                    if testo::allucinazione(t) {
                        continue;
                    }
                    if st.voce.sta_parlando() && testo::e_stop(t) {
                        st.voce.ferma_lettura();
                        continue;
                    }
                    if let Some(resto) = testo::dopo_attivazione(t) {
                        st.voce.ferma_lettura();
                        if resto.trim().len() > 2 {
                            invia(app, &resto);
                        } else {
                            entra_comando(&mut modo, &mut vad_comando);
                        }
                        break;
                    }
                }
            }
        }
    }
}

fn trascrivi_e_invia(app: &AppHandle, asr: &mut sherpa_rs::whisper::WhisperRecognizer, audio: &[f32], _attesa: bool) {
    let st = app.state::<App>();
    st.avatar.set(app, "voce", Volto::Thinking);
    let t = asr.transcribe(RATE, audio).text;
    st.avatar.clear(app, "voce");
    let t = t.trim();
    if testo::allucinazione(t) {
        let _ = app.emit("voice-transcript", json!({ "text": "", "final": true, "empty": true }));
        return;
    }
    if testo::e_stop(t) {
        st.voce.ferma_lettura();
        return;
    }
    // "Ehi Oltremare" detto anche con il tasto: si toglie
    let comando = testo::dopo_attivazione(t).filter(|r| !r.is_empty()).unwrap_or_else(|| t.to_string());
    invia(app, &comando);
}

fn invia(app: &AppHandle, testo: &str) {
    let _ = app.emit("voice-transcript", json!({ "text": testo, "final": true }));
    app.state::<App>().voce.esegui_comando(app, testo);
}

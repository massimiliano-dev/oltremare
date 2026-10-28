//! Voce: «Ehi Oltremare» o un tasto per parlare, trascrizione locale (Whisper), risposte lette
//! con una voce neurale locale (Piper) o con quella di sistema. Tutto resta sul computer.

pub mod ascolto;
pub mod cattura;
pub mod modelli;
pub mod sintesi;
pub mod testo;

use crate::agent::ciclo::Avvio;
use crate::App;
use serde_json::{json, Value};
use std::sync::mpsc::Sender;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Default)]
pub struct Voce {
    sintesi: OnceLock<sintesi::Sintesi>,
    ascolto: Mutex<Option<(Sender<ascolto::Cmd>, bool)>>,
    conversazione: Mutex<Option<(String, String, Instant)>>,
    /// Richiesta di approvazione a cui si può rispondere a voce
    conferma: Mutex<Option<String>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Voce {
    pub fn init(&self, app: &AppHandle) {
        let dir = app.state::<App>().data_dir.join("modelli");
        let _ = self.sintesi.set(sintesi::Sintesi::avvia(app.clone(), dir));
        let s = app.state::<App>().store.settings();
        if s.voice_listen {
            if let Err(e) = self.avvia_ascolto(app, true) {
                let _ = app.emit("voice-error", e);
            }
        }
    }

    pub fn parla(&self, app: &AppHandle, testo: &str) {
        let s = app.state::<App>().store.settings();
        if let Some(t) = self.sintesi.get() {
            t.parla(testo, &s.voice_tts, s.voice_speed);
        }
    }

    pub fn ferma_lettura(&self) {
        if let Some(t) = self.sintesi.get() {
            if t.parlando.load(std::sync::atomic::Ordering::SeqCst) {
                t.stop();
            }
        }
    }

    pub fn sta_parlando(&self) -> bool {
        self.sintesi.get().is_some_and(|t| t.parlando.load(std::sync::atomic::Ordering::SeqCst))
    }

    fn config(app: &AppHandle, parola: bool) -> ascolto::Config {
        let st = app.state::<App>();
        let s = st.store.settings();
        ascolto::Config { cartella: st.data_dir.join("modelli"), stt: s.voice_stt, dispositivo: s.voice_device, parola }
    }

    /// Avvia l'ascolto. `parola`: resta in attesa di «Ehi Oltremare»; altrimenti ascolta un comando e si ferma.
    pub fn avvia_ascolto(&self, app: &AppHandle, parola: bool) -> Result<(), String> {
        let mut g = lock(&self.ascolto);
        if let Some((tx, attivo_parola)) = g.as_ref() {
            if *attivo_parola == parola || (*attivo_parola && !parola) {
                if !parola {
                    let _ = tx.send(ascolto::Cmd::Comando);
                }
                return Ok(());
            }
            let _ = tx.send(ascolto::Cmd::Ferma);
        }
        for id in ["silero-vad", &app.state::<App>().store.settings().voice_stt] {
            if !crate::modelli::installato(app, id) {
                return Err("Per la voce servono i modelli di riconoscimento: scaricali in Impostazioni → Voce".into());
            }
        }
        let (tx, rx) = std::sync::mpsc::channel();
        ascolto::avvia(app.clone(), Self::config(app, parola), rx);
        *g = Some((tx, parola));
        Ok(())
    }

    pub fn ferma_ascolto(&self) {
        if let Some((tx, _)) = lock(&self.ascolto).take() {
            let _ = tx.send(ascolto::Cmd::Ferma);
        }
    }

    /// Il thread di ascolto "una tantum" è finito: si libera il posto.
    fn canale(&self, app: &AppHandle) -> Result<Sender<ascolto::Cmd>, String> {
        let vivo = lock(&self.ascolto).as_ref().map(|(tx, _)| tx.send(ascolto::Cmd::Annulla).is_ok()).unwrap_or(false);
        if !vivo {
            *lock(&self.ascolto) = None;
            self.avvia_ascolto(app, false)?;
            // Il nuovo thread parte già in ascolto del comando
            return lock(&self.ascolto).as_ref().map(|(tx, _)| tx.clone()).ok_or_else(|| "Ascolto non disponibile".into());
        }
        lock(&self.ascolto).as_ref().map(|(tx, _)| tx.clone()).ok_or_else(|| "Ascolto non disponibile".into())
    }

    /// Pulsante del microfono: ascolta un comando, finisce quando smetti di parlare.
    pub fn comando(&self, app: &AppHandle) -> Result<(), String> {
        let nuovo = lock(&self.ascolto).is_none();
        let tx = self.canale(app)?;
        if !nuovo {
            let _ = tx.send(ascolto::Cmd::Comando);
        }
        Ok(())
    }

    pub fn annulla(&self) {
        if let Some((tx, _)) = lock(&self.ascolto).as_ref() {
            let _ = tx.send(ascolto::Cmd::Annulla);
        }
    }

    /// Tasto per parlare: premuto registra, rilasciato trascrive.
    pub fn ptt(&self, app: &AppHandle, premuto: bool) -> Result<(), String> {
        let tx = if premuto { self.canale(app)? } else { lock(&self.ascolto).as_ref().map(|(t, _)| t.clone()).ok_or("Ascolto non attivo")? };
        let _ = tx.send(ascolto::Cmd::Ptt(premuto));
        Ok(())
    }

    /// Un agente chiede conferma durante una richiesta fatta a voce: lo si chiede a voce.
    pub fn chiedi_conferma(&self, app: &AppHandle, approval_id: &str, titolo: &str) {
        *lock(&self.conferma) = Some(approval_id.to_string());
        self.parla(app, &format!("Posso procedere? {titolo}. Dimmi sì o no."));
        // Dopo la domanda si ascolta la risposta
        let app2 = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(600));
            let st = app2.state::<App>();
            while st.voce.sta_parlando() {
                std::thread::sleep(Duration::from_millis(100));
            }
            let _ = st.voce.comando(&app2);
        });
    }

    pub fn conferma_chiusa(&self, approval_id: &str) {
        let mut g = lock(&self.conferma);
        if g.as_deref() == Some(approval_id) {
            *g = None;
        }
    }

    /// Cosa fare con una frase detta: risposta a una conferma, oppure nuova richiesta a un agente.
    pub fn esegui_comando(&self, app: &AppHandle, testo: &str) {
        let st = app.state::<App>();
        if let Some(id) = lock(&self.conferma).clone() {
            let n = testo::normalizza(testo);
            let si = ["si", "sì", "ok", "va bene", "vai", "certo", "procedi", "fallo", "confermo", "si grazie"].iter().any(|x| n == *x || n.starts_with(&format!("{x} ")));
            let no = ["no", "annulla", "non farlo", "aspetta", "stop", "fermati", "no grazie"].iter().any(|x| n == *x || n.starts_with(&format!("{x} ")));
            if si || no {
                use crate::agent::approvazioni::Risposta;
                st.approvazioni.rispondi(&id, if si { Risposta::Consenti } else { Risposta::Nega(Some("detto a voce".into())) });
                *lock(&self.conferma) = None;
                return;
            }
        }
        let s = st.store.settings();
        let n = testo::normalizza(testo);
        // Un agente con una frase di attivazione a voce che compare nel comando
        let agenti = st.store.agents();
        let scelto = agenti
            .iter()
            .find(|a| {
                a.triggers.iter().any(|t| {
                    t["kind"] == "voice"
                        && t["phrases"].as_array().is_some_and(|f| f.iter().filter_map(Value::as_str).any(|p| !p.trim().is_empty() && n.contains(&testo::normalizza(p))))
                })
            })
            .map(|a| a.id.clone())
            .unwrap_or_else(|| if s.voice_agent.is_empty() { "assistente".into() } else { s.voice_agent.clone() });
        // Si continua la stessa conversazione per qualche minuto
        let conv = lock(&self.conversazione)
            .as_ref()
            .filter(|(a, _, t)| *a == scelto && t.elapsed() < Duration::from_secs(600))
            .map(|(_, c, _)| c.clone());
        match crate::agent::ciclo::avvia(app, Avvio { agent_id: scelto.clone(), conversation_id: conv, text: testo.to_string(), source: "voice".into(), attachments: vec![] }) {
            Ok(run) => {
                *lock(&self.conversazione) = Some((scelto.clone(), run.conversation_id.clone(), Instant::now()));
                let _ = app.emit("voice-run", json!({ "runId": run.id, "conversationId": run.conversation_id, "agentId": scelto }));
            }
            Err(e) => {
                let msg = e.to_string();
                let _ = app.emit("voice-error", &msg);
                self.parla(app, &msg);
            }
        }
    }

    pub fn stato(&self, app: &AppHandle) -> Value {
        let s = app.state::<App>().store.settings();
        let g = lock(&self.ascolto);
        json!({
            "wakeListening": g.as_ref().is_some_and(|(_, p)| *p),
            "active": g.is_some(),
            "speaking": self.sta_parlando(),
            "models": {
                "vad": crate::modelli::installato(app, "silero-vad"),
                "stt": crate::modelli::installato(app, &s.voice_stt),
                "tts": s.voice_tts == "sistema" || crate::modelli::installato(app, &s.voice_tts),
            },
        })
    }
}

/// Legge una risposta dell'agente (rispettando le impostazioni di voce).
pub fn parla_risposta(app: &AppHandle, testo: &str) {
    app.state::<App>().voce.parla(app, testo);
}

#[cfg(test)]
mod tests {
    /// Prova completa senza microfono: Piper dice una frase, Silero la trova, Whisper la trascrive.
    /// `OLTREMARE_TEST_MODELLI=~/Library/Application Support/app.oltremare/modelli cargo test voce_completa -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn voce_completa() {
        let Ok(dir) = std::env::var("OLTREMARE_TEST_MODELLI") else { return };
        let dir = std::path::PathBuf::from(dir);
        // Sintesi
        let p = dir.join("piper-paola");
        let mut tts = sherpa_rs::tts::VitsTts::new(sherpa_rs::tts::VitsTtsConfig {
            model: p.join("it_IT-paola-medium.onnx").to_string_lossy().into_owned(),
            tokens: p.join("tokens.txt").to_string_lossy().into_owned(),
            data_dir: p.join("espeak-ng-data").to_string_lossy().into_owned(),
            length_scale: 1.0,
            noise_scale: 0.667,
            noise_scale_w: 0.8,
            onnx_config: sherpa_rs::OnnxConfig { provider: "cpu".into(), debug: false, num_threads: 2 },
            ..Default::default()
        });
        let t0 = std::time::Instant::now();
        let audio = tts.create("Ehi Oltremare, che ore sono?", 0, 1.0).unwrap();
        println!("sintesi: {} campioni a {} Hz in {:?}", audio.samples.len(), audio.sample_rate, t0.elapsed());
        assert!(audio.samples.len() > audio.sample_rate as usize / 2);
        // Ricampionamento a 16 kHz, con un secondo di silenzio prima e dopo (come dal microfono)
        let passo = audio.sample_rate as f64 / 16_000.0;
        let mut a16: Vec<f32> = vec![0.0; 16_000];
        let mut pos = 0.0;
        while (pos as usize) < audio.samples.len() {
            a16.push(audio.samples[pos as usize]);
            pos += passo;
        }
        a16.extend(vec![0.0f32; 24_000]);
        // Rilevamento del parlato, a blocchi da 30 ms
        let cfg = super::ascolto::Config { cartella: dir.clone(), stt: "whisper-small".into(), dispositivo: String::new(), parola: true };
        let mut vad = sherpa_rs::silero_vad::SileroVad::new(
            sherpa_rs::silero_vad::SileroVadConfig {
                model: cfg.cartella.join("silero-vad/silero_vad.onnx").to_string_lossy().into_owned(),
                min_silence_duration: 0.5,
                min_speech_duration: 0.25,
                max_speech_duration: 20.0,
                threshold: 0.5,
                sample_rate: 16_000,
                window_size: 512,
                ..Default::default()
            },
            60.0,
        )
        .unwrap();
        let mut segmenti = vec![];
        for b in a16.chunks(480) {
            vad.accept_waveform(b.to_vec());
            while !vad.is_empty() {
                segmenti.push(vad.front().samples);
                vad.pop();
            }
        }
        assert_eq!(segmenti.len(), 1, "segmenti trovati: {}", segmenti.len());
        // Trascrizione
        let w = dir.join("whisper-small");
        let mut asr = sherpa_rs::whisper::WhisperRecognizer::new(sherpa_rs::whisper::WhisperConfig {
            encoder: w.join("small-encoder.int8.onnx").to_string_lossy().into_owned(),
            decoder: w.join("small-decoder.int8.onnx").to_string_lossy().into_owned(),
            tokens: w.join("small-tokens.txt").to_string_lossy().into_owned(),
            language: "it".into(),
            tail_paddings: Some(-1),
            num_threads: Some(4),
            provider: Some("cpu".into()),
            ..Default::default()
        })
        .unwrap();
        let t1 = std::time::Instant::now();
        let testo = asr.transcribe(16_000, &segmenti[0]).text;
        println!("trascrizione: «{}» in {:?}", testo.trim(), t1.elapsed());
        let comando = super::testo::dopo_attivazione(&testo);
        assert!(comando.as_deref().is_some_and(|c| { let n = super::testo::normalizza(c); n.contains("che or") && n.contains("sono") }), "trascritto: {testo}");
    }
}

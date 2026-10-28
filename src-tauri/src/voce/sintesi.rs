//! Sintesi vocale: voci Piper locali (sherpa-onnx) riprodotte con rodio, frase per frase.
//! Il volume di ciò che si sente guida la bocca dell'avatar. Senza modello, voce di sistema.

use super::testo;
use crate::avatar::Stato as Volto;
use crate::App;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

enum Cmd {
    Parla { testo: String, voce: String, velocita: f32 },
    Stop,
}

/// Inviluppo del volume da riprodurre: (inizio, livelli ogni 33 ms).
type Programma = Arc<Mutex<VecDeque<(Instant, Vec<f32>)>>>;

pub struct Sintesi {
    tx: Sender<Cmd>,
    pub parlando: Arc<AtomicBool>,
    generazione: Arc<AtomicU64>,
}

const PASSO: Duration = Duration::from_millis(33);

fn inviluppo(campioni: &[f32], rate: u32) -> Vec<f32> {
    let n = (rate as f64 * PASSO.as_secs_f64()) as usize;
    campioni.chunks(n.max(1)).map(super::cattura::livello).collect()
}

impl Sintesi {
    pub fn avvia(app: AppHandle, cartella_modelli: PathBuf) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let parlando = Arc::new(AtomicBool::new(false));
        let generazione = Arc::new(AtomicU64::new(0));
        let programma: Programma = Arc::new(Mutex::new(VecDeque::new()));
        {
            let (app, programma, parlando) = (app.clone(), programma.clone(), parlando.clone());
            std::thread::Builder::new()
                .name("oltremare-labiale".into())
                .spawn(move || labiale(app, programma, parlando))
                .ok();
        }
        {
            let (parlando, generazione) = (parlando.clone(), generazione.clone());
            std::thread::Builder::new()
                .name("oltremare-voce".into())
                .spawn(move || ciclo(app, cartella_modelli, rx, parlando, generazione, programma))
                .ok();
        }
        Self { tx, parlando, generazione }
    }

    pub fn parla(&self, testo: &str, voce: &str, velocita: f32) {
        let _ = self.tx.send(Cmd::Parla { testo: testo.to_string(), voce: voce.to_string(), velocita });
    }

    pub fn stop(&self) {
        self.generazione.fetch_add(1, Ordering::SeqCst);
        let _ = self.tx.send(Cmd::Stop);
    }
}

/// Manda all'avatar il volume di ciò che si sente, ogni 33 ms.
fn labiale(app: AppHandle, programma: Programma, parlando: Arc<AtomicBool>) {
    let mut ultimo = 0.0f32;
    loop {
        std::thread::sleep(PASSO);
        let ora = Instant::now();
        let livello = {
            let mut p = programma.lock().unwrap_or_else(|e| e.into_inner());
            while p.front().is_some_and(|(t, env)| *t + PASSO * env.len() as u32 <= ora) {
                p.pop_front();
            }
            p.front().and_then(|(t, env)| (ora >= *t).then(|| env[((ora - *t).as_millis() / PASSO.as_millis()) as usize % env.len().max(1)]))
        }
        .unwrap_or(0.0);
        if parlando.load(Ordering::Relaxed) || ultimo > 0.0 {
            if (livello - ultimo).abs() > 0.02 || (livello == 0.0 && ultimo != 0.0) {
                app.state::<App>().avatar.livello(&app, livello);
                ultimo = livello;
            }
        }
    }
}

struct Voce {
    id: String,
    tts: sherpa_rs::tts::VitsTts,
}

fn carica(cartella: &PathBuf, id: &str) -> Option<Voce> {
    let m = super::modelli::voce(id)?;
    let dir = cartella.join(m.id);
    let onnx = dir.join(m.files.first()?);
    if !onnx.is_file() {
        return None;
    }
    let cfg = sherpa_rs::tts::VitsTtsConfig {
        model: onnx.to_string_lossy().into_owned(),
        tokens: dir.join("tokens.txt").to_string_lossy().into_owned(),
        data_dir: dir.join("espeak-ng-data").to_string_lossy().into_owned(),
        length_scale: 1.0,
        noise_scale: 0.667,
        noise_scale_w: 0.8,
        silence_scale: 0.2,
        onnx_config: sherpa_rs::OnnxConfig { provider: "cpu".into(), debug: false, num_threads: 2 },
        ..Default::default()
    };
    Some(Voce { id: id.to_string(), tts: sherpa_rs::tts::VitsTts::new(cfg) })
}

#[cfg(target_os = "macos")]
fn voce_sistema(testo: &str) -> Option<std::process::Child> {
    std::process::Command::new("say").args(["-v", "Alice", testo]).spawn().ok()
}

#[cfg(windows)]
fn voce_sistema(testo: &str) -> Option<std::process::Child> {
    use std::os::windows::process::CommandExt;
    let script = format!(
        "Add-Type -AssemblyName System.Speech; $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; \
         $v = $s.GetInstalledVoices() | Where-Object {{ $_.VoiceInfo.Culture.Name -eq 'it-IT' }} | Select-Object -First 1; \
         if ($v) {{ $s.SelectVoice($v.VoiceInfo.Name) }}; $s.Speak('{}')",
        testo.replace('\'', "''")
    );
    std::process::Command::new("powershell").args(["-NoProfile", "-Command", &script]).creation_flags(0x0800_0000).spawn().ok()
}

#[cfg(not(any(target_os = "macos", windows)))]
fn voce_sistema(_testo: &str) -> Option<std::process::Child> {
    None
}

fn ciclo(app: AppHandle, cartella: PathBuf, rx: Receiver<Cmd>, parlando: Arc<AtomicBool>, generazione: Arc<AtomicU64>, programma: Programma) {
    let mut uscita: Option<rodio::MixerDeviceSink> = None;
    let mut player: Option<rodio::Player> = None;
    let mut voce: Option<Voce> = None;
    let mut sistema: Option<std::process::Child> = None;
    let mut coda: VecDeque<Cmd> = VecDeque::new();
    let avatar = |app: &AppHandle, on: bool| {
        let st = app.state::<App>();
        if on {
            st.avatar.set(app, "voce-sintesi", Volto::Speaking);
        } else {
            st.avatar.clear(app, "voce-sintesi");
        }
    };
    let ferma = |player: &Option<rodio::Player>, sistema: &mut Option<std::process::Child>, programma: &Programma| {
        if let Some(p) = player {
            p.clear();
            p.play();
        }
        if let Some(mut c) = sistema.take() {
            let _ = c.kill();
        }
        programma.lock().unwrap_or_else(|e| e.into_inner()).clear();
    };

    loop {
        let cmd = match coda.pop_front() {
            Some(c) => Some(c),
            None => match rx.recv_timeout(Duration::from_millis(120)) {
                Ok(c) => Some(c),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => None,
                Err(_) => return,
            },
        };
        match cmd {
            Some(Cmd::Stop) => {
                ferma(&player, &mut sistema, &programma);
                if parlando.swap(false, Ordering::SeqCst) {
                    avatar(&app, false);
                }
            }
            Some(Cmd::Parla { testo: t, voce: id, velocita }) => {
                let gen = generazione.load(Ordering::SeqCst);
                let parlato = testo::parlato(&t);
                if parlato.trim().is_empty() {
                    continue;
                }
                parlando.store(true, Ordering::SeqCst);
                avatar(&app, true);
                if voce.as_ref().is_none_or(|v| v.id != id) {
                    voce = carica(&cartella, &id);
                }
                if uscita.is_none() && voce.is_some() {
                    if let Ok(mut s) = rodio::DeviceSinkBuilder::open_default_sink() {
                        s.log_on_drop(false);
                        player = Some(rodio::Player::connect_new(s.mixer()));
                        uscita = Some(s);
                    }
                }
                match (&mut voce, &player) {
                    (Some(v), Some(p)) => {
                        let mut fine = Instant::now();
                        for frase in testo::frasi(&parlato) {
                            // Uno "stop" arrivato nel frattempo interrompe la lettura
                            while let Ok(c) = rx.try_recv() {
                                if !matches!(c, Cmd::Stop) {
                                    coda.push_back(c);
                                }
                            }
                            if generazione.load(Ordering::SeqCst) != gen {
                                break;
                            }
                            let Ok(audio) = v.tts.create(&frase, 0, velocita.clamp(0.6, 1.6)) else { continue };
                            if audio.samples.is_empty() {
                                continue;
                            }
                            let durata = Duration::from_secs_f64(audio.samples.len() as f64 / audio.sample_rate as f64);
                            let inizio = fine.max(Instant::now());
                            programma.lock().unwrap_or_else(|e| e.into_inner()).push_back((inizio, inviluppo(&audio.samples, audio.sample_rate)));
                            fine = inizio + durata;
                            let (Some(ch), Some(sr)) = (std::num::NonZero::new(1u16), std::num::NonZero::new(audio.sample_rate)) else { continue };
                            p.append(rodio::buffer::SamplesBuffer::new(ch, sr, audio.samples));
                        }
                        if generazione.load(Ordering::SeqCst) != gen {
                            ferma(&player, &mut sistema, &programma);
                        }
                    }
                    _ => {
                        // Senza voce neurale: voce di sistema, con una bocca simulata
                        sistema = voce_sistema(&parlato);
                        let finto: Vec<f32> = (0..(parlato.len() as f32 / 14.0 * 30.0) as usize).map(|i| 0.35 + 0.4 * ((i as f32 * 0.9).sin().abs())).collect();
                        programma.lock().unwrap_or_else(|e| e.into_inner()).push_back((Instant::now(), finto));
                    }
                }
            }
            None => {
                if parlando.load(Ordering::SeqCst) {
                    let finito_player = player.as_ref().is_none_or(|p| p.empty());
                    let finito_sistema = sistema.as_mut().is_none_or(|c| matches!(c.try_wait(), Ok(Some(_))));
                    if finito_player && finito_sistema {
                        sistema = None;
                        programma.lock().unwrap_or_else(|e| e.into_inner()).clear();
                        parlando.store(false, Ordering::SeqCst);
                        avatar(&app, false);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    /// Apre l'uscita audio e riproduce mezzo secondo di silenzio: `cargo test uscita_audio -- --ignored`
    #[test]
    #[ignore]
    fn uscita_audio() {
        let mut s = rodio::DeviceSinkBuilder::open_default_sink().expect("uscita audio");
        s.log_on_drop(false);
        let p = rodio::Player::connect_new(s.mixer());
        let silenzio = vec![0.0f32; 11_025];
        p.append(rodio::buffer::SamplesBuffer::new(std::num::NonZero::new(1).unwrap(), std::num::NonZero::new(22_050).unwrap(), silenzio));
        let t = std::time::Instant::now();
        p.sleep_until_end();
        assert!(t.elapsed() >= std::time::Duration::from_millis(400), "riproduzione troppo breve: {:?}", t.elapsed());
    }
}

//! Microfono: cattura con cpal, mono a 16 kHz in blocchi da ~30 ms.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

pub const RATE: u32 = 16_000;

/// Ricampionamento lineare, con lo stato tra un blocco e l'altro.
struct Ricampiona {
    passo: f64,
    pos: f64,
    ultimo: f32,
}

impl Ricampiona {
    fn new(da: u32) -> Self {
        Self { passo: da as f64 / RATE as f64, pos: 0.0, ultimo: 0.0 }
    }

    fn processa(&mut self, mono: &[f32], out: &mut Vec<f32>) {
        // pos è relativo al campione "ultimo" (indice -1)
        while self.pos < mono.len() as f64 {
            let i = self.pos.floor();
            let f = (self.pos - i) as f32;
            let a = if i < 1.0 { self.ultimo } else { mono[i as usize - 1] };
            let b = mono[(i as usize).min(mono.len() - 1)];
            out.push(a + (b - a) * f);
            self.pos += self.passo;
        }
        self.pos -= mono.len() as f64;
        if let Some(&l) = mono.last() {
            self.ultimo = l;
        }
    }
}

pub struct Cattura {
    stop: Arc<AtomicBool>,
}

impl Drop for Cattura {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

pub fn dispositivi() -> Vec<String> {
    let host = cpal::default_host();
    host.input_devices()
        .map(|d| d.filter_map(|x| x.description().ok().map(|n| n.name().to_string())).collect())
        .unwrap_or_default()
}

/// Avvia la cattura in un thread dedicato (lo stream audio non può cambiare thread).
pub fn avvia(dispositivo: &str, tx: Sender<Vec<f32>>) -> Result<Cattura, String> {
    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = stop.clone();
    let nome = dispositivo.to_string();
    let (pronto_tx, pronto_rx) = std::sync::mpsc::channel::<Result<(), String>>();
    std::thread::Builder::new()
        .name("oltremare-microfono".into())
        .spawn(move || {
            let host = cpal::default_host();
            let dev = if nome.is_empty() {
                host.default_input_device()
            } else {
                host.input_devices().ok().and_then(|mut d| d.find(|x| x.description().map(|n| n.name() == nome).unwrap_or(false))).or_else(|| host.default_input_device())
            };
            let Some(dev) = dev else {
                let _ = pronto_tx.send(Err("Nessun microfono trovato".into()));
                return;
            };
            let cfg = match dev.default_input_config() {
                Ok(c) => c,
                Err(e) => {
                    let _ = pronto_tx.send(Err(format!("Microfono non disponibile: {e}")));
                    return;
                }
            };
            let canali = cfg.channels() as usize;
            let mut rs = Ricampiona::new(cfg.sample_rate());
            let mut buf: Vec<f32> = Vec::with_capacity(1024);
            let mut accumula = move |dati: &mut dyn Iterator<Item = f32>| {
                let campioni: Vec<f32> = dati.collect();
                let mono: Vec<f32> = campioni.chunks(canali.max(1)).map(|c| c.iter().sum::<f32>() / c.len() as f32).collect();
                rs.processa(&mono, &mut buf);
                if buf.len() >= 480 {
                    let _ = tx.send(std::mem::take(&mut buf));
                }
            };
            let err = |e| eprintln!("microfono: {e}");
            let config = cfg.config();
            let stream = match cfg.sample_format() {
                cpal::SampleFormat::F32 => dev.build_input_stream::<f32, _, _>(config, move |d, _| accumula(&mut d.iter().copied()), err, None),
                cpal::SampleFormat::I16 => dev.build_input_stream::<i16, _, _>(config, move |d, _| accumula(&mut d.iter().map(|&s| s as f32 / 32768.0)), err, None),
                cpal::SampleFormat::I32 => dev.build_input_stream::<i32, _, _>(config, move |d, _| accumula(&mut d.iter().map(|&s| s as f32 / 2_147_483_648.0)), err, None),
                cpal::SampleFormat::U16 => dev.build_input_stream::<u16, _, _>(config, move |d, _| accumula(&mut d.iter().map(|&s| (s as f32 - 32768.0) / 32768.0)), err, None),
                f => {
                    let _ = pronto_tx.send(Err(format!("Formato audio non supportato: {f:?}")));
                    return;
                }
            };
            let stream = match stream {
                Ok(s) => s,
                Err(e) => {
                    let _ = pronto_tx.send(Err(format!("Il microfono non si apre: {e}. Controlla il permesso in Impostazioni di Sistema → Privacy → Microfono")));
                    return;
                }
            };
            if let Err(e) = stream.play() {
                let _ = pronto_tx.send(Err(format!("Il microfono non parte: {e}")));
                return;
            }
            let _ = pronto_tx.send(Ok(()));
            while !stop2.load(Ordering::SeqCst) {
                std::thread::sleep(std::time::Duration::from_millis(60));
            }
            drop(stream);
        })
        .map_err(|e| e.to_string())?;
    pronto_rx.recv_timeout(std::time::Duration::from_secs(5)).map_err(|_| "Il microfono non risponde".to_string())??;
    Ok(Cattura { stop })
}

/// Volume del blocco (RMS), già scalato 0..1 per l'avatar.
pub fn livello(campioni: &[f32]) -> f32 {
    if campioni.is_empty() {
        return 0.0;
    }
    let rms = (campioni.iter().map(|x| x * x).sum::<f32>() / campioni.len() as f32).sqrt();
    (rms * 7.0).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resampling_keeps_duration() {
        let mut r = Ricampiona::new(48_000);
        let mut out = Vec::new();
        // 1 secondo a 48 kHz in blocchi irregolari
        let tono: Vec<f32> = (0..48_000).map(|i| (i as f32 * 0.01).sin()).collect();
        for c in tono.chunks(731) {
            r.processa(c, &mut out);
        }
        assert!((out.len() as i64 - 16_000).abs() <= 2, "{}", out.len());
    }
}

//! Download con ripresa, avanzamento e verifica dell'impronta SHA-256.

use crate::error::{AppError, Result};
use crate::providers::client;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::Instant;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;

pub struct File {
    pub url: String,
    pub dest: PathBuf,
    /// Impronta attesa (esadecimale), se nota
    pub sha256: Option<String>,
    pub size: Option<u64>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Avanzamento<'a> {
    pub id: &'a str,
    pub received: u64,
    pub total: u64,
    pub bytes_per_sec: u64,
    pub done: bool,
    pub error: Option<String>,
    pub phase: &'a str,
}

/// Impronte e dimensioni dei file di un repository Hugging Face.
pub async fn hf_impronte(repo: &str) -> Result<Vec<(String, String, u64)>> {
    let resp = client().get(format!("https://huggingface.co/api/models/{repo}/tree/main")).send().await?;
    if !resp.status().is_success() {
        return Err(AppError::msg(format!("Hugging Face non risponde ({})", resp.status())));
    }
    let v: serde_json::Value = resp.json().await?;
    Ok(v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|f| {
            let p = f["path"].as_str()?.to_string();
            let sha = f["lfs"]["oid"].as_str().unwrap_or_default().to_string();
            let size = f["lfs"]["size"].as_u64().or(f["size"].as_u64()).unwrap_or(0);
            Some((p, sha, size))
        })
        .collect())
}

pub fn hf_url(repo: &str, file: &str) -> String {
    format!("https://huggingface.co/{repo}/resolve/main/{file}")
}

async fn impronta_parziale(path: &Path, h: &mut Sha256) -> Result<u64> {
    let mut f = tokio::fs::File::open(path).await?;
    let mut buf = vec![0u8; 1 << 20];
    let mut n = 0u64;
    loop {
        let k = f.read(&mut buf).await?;
        if k == 0 {
            return Ok(n);
        }
        h.update(&buf[..k]);
        n += k as u64;
    }
}

/// Scarica i file in ordine. I file già completi si saltano; i download interrotti riprendono.
pub async fn scarica(app: &AppHandle, id: &str, files: &[File], cancel: &CancellationToken) -> Result<()> {
    let total: u64 = files.iter().map(|f| f.size.unwrap_or(0)).sum();
    let mut base = 0u64;
    let mut ultimo = Instant::now();
    let inizio = Instant::now();
    let emetti = |received: u64, done: bool, error: Option<String>, phase: &str| {
        let secs = inizio.elapsed().as_secs_f64().max(0.1);
        let _ = app.emit(
            "model-download",
            Avanzamento { id, received, total, bytes_per_sec: (received as f64 / secs) as u64, done, error, phase },
        );
    };
    for f in files {
        if f.dest.exists() && f.size.is_none_or(|s| std::fs::metadata(&f.dest).map(|m| m.len() == s).unwrap_or(false)) {
            base += f.size.unwrap_or(0);
            continue;
        }
        if let Some(dir) = f.dest.parent() {
            tokio::fs::create_dir_all(dir).await?;
        }
        let part = f.dest.with_extension(format!("{}.part", f.dest.extension().and_then(|e| e.to_str()).unwrap_or("")));
        let mut hasher = Sha256::new();
        let mut gia = if part.exists() { impronta_parziale(&part, &mut hasher).await? } else { 0 };
        let mut req = client().get(&f.url);
        if gia > 0 {
            req = req.header("Range", format!("bytes={gia}-"));
        }
        let resp = tokio::select! {
            _ = cancel.cancelled() => return Err(AppError::Annullato),
            r = req.send() => r?,
        };
        if resp.status().as_u16() == 200 && gia > 0 {
            // Il server non riprende: si ricomincia
            gia = 0;
            hasher = Sha256::new();
            let _ = tokio::fs::remove_file(&part).await;
        } else if !resp.status().is_success() {
            return Err(AppError::msg(format!("Download non riuscito ({}) da {}", resp.status(), f.url)));
        }
        let mut out = tokio::fs::OpenOptions::new().create(true).append(true).open(&part).await?;
        let mut stream = resp.bytes_stream();
        let mut ricevuti = gia;
        while let Some(chunk) = crate::providers::prossimo(&mut stream, cancel).await? {
            hasher.update(&chunk);
            out.write_all(&chunk).await?;
            ricevuti += chunk.len() as u64;
            if ultimo.elapsed().as_millis() >= 250 {
                ultimo = Instant::now();
                emetti(base + ricevuti, false, None, "download");
            }
        }
        out.flush().await?;
        drop(out);
        if let Some(atteso) = &f.sha256 {
            let calcolato: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
            if !calcolato.eq_ignore_ascii_case(atteso.trim_start_matches("sha256:")) {
                let _ = tokio::fs::remove_file(&part).await;
                return Err(AppError::msg("Il file scaricato è danneggiato (impronta diversa): riprova"));
            }
        }
        tokio::fs::rename(&part, &f.dest).await?;
        base += f.size.unwrap_or(ricevuti);
    }
    emetti(base.max(total), true, None, "done");
    Ok(())
}

/// Segnala un errore di download alla finestra.
pub fn errore(app: &AppHandle, id: &str, e: &AppError) {
    let _ = app.emit(
        "model-download",
        Avanzamento { id, received: 0, total: 0, bytes_per_sec: 0, done: true, error: Some(e.to_string()), phase: "error" },
    );
}

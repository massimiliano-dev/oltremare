//! Modelli scaricabili (AI locale, voce): catalogo, download verificati, stato di installazione.

pub mod catalogo;
pub mod download;

use serde::Serialize;

/// RAM e memoria disponibile, per consigliare i modelli adatti.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Hardware {
    pub ram_total: u64,
    pub ram_available: u64,
    pub cpu: String,
    pub backend: String,
    pub os: String,
}

pub fn hardware(backend: &str) -> Hardware {
    use sysinfo::System;
    let mut s = System::new();
    s.refresh_memory();
    s.refresh_cpu_all();
    let cpu = s.cpus().first().map(|c| c.brand().trim().to_string()).unwrap_or_default();
    Hardware {
        ram_total: s.total_memory(),
        ram_available: s.available_memory(),
        cpu,
        backend: backend.to_string(),
        os: std::env::consts::OS.to_string(),
    }
}

use crate::error::{AppError, Result};
use crate::App;
use catalogo::Vista;
use tauri::{AppHandle, Manager};

/// Catalogo con lo stato di installazione e i consigli in base alla RAM.
pub fn elenco(app: &AppHandle, kind: Option<&str>) -> Vec<Vista> {
    let ram_gb = (hardware("").ram_total / 1_000_000_000) as u32;
    // Il modello consigliato: il più capace che sta comodo in questa RAM
    let consigliato = catalogo::MODELLI
        .iter()
        .filter(|m| m.consigliato_da_gb <= ram_gb && m.ram_gb as f64 <= ram_gb as f64 * 0.75)
        .max_by_key(|m| m.consigliato_da_gb)
        .map(|m| m.id);
    let mut v: Vec<Vista> = catalogo::tutte()
        .filter(|m| kind.is_none_or(|k| m.kind == k))
        .map(|m| Vista {
            id: m.id.into(),
            kind: m.kind.into(),
            label: m.label.into(),
            note: m.nota.into(),
            repo: m.repo.into(),
            size_bytes: m.dimensione,
            ram_needed_gb: m.ram_gb,
            ctx: m.ctx,
            supports_tools: m.strumenti,
            supports_vision: m.visione,
            recommended: Some(m.id) == consigliato || (m.kind != "llm" && m.consigliato_da_gb == 0),
            installed: installato(app, m.id),
            custom: false,
        })
        .collect();
    if kind.is_none_or(|k| k == "llm") {
        let dir = crate::sidecar::cartella_modelli(app).join("personali");
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let nome = e.file_name().to_string_lossy().to_string();
                if nome.to_lowercase().ends_with(".gguf") {
                    v.push(Vista {
                        id: format!("custom:{nome}"),
                        kind: "llm".into(),
                        label: nome.trim_end_matches(".gguf").to_string(),
                        note: "Importato da te".into(),
                        repo: String::new(),
                        size_bytes: e.metadata().map(|m| m.len()).unwrap_or(0),
                        ram_needed_gb: 0,
                        ctx: 16384,
                        supports_tools: true,
                        supports_vision: false,
                        recommended: false,
                        installed: true,
                        custom: true,
                    });
                }
            }
        }
    }
    v
}

pub fn installato(app: &AppHandle, id: &str) -> bool {
    let Some(m) = catalogo::voce(id) else { return id.starts_with("custom:") && crate::sidecar::file_modello(app, id).is_some() };
    let dir = crate::sidecar::cartella_modelli(app).join(m.id);
    m.files.iter().all(|f| dir.join(f).is_file())
}

/// Avvia il download di un modello del catalogo in background.
pub fn scarica(app: &AppHandle, id: &str) -> Result<()> {
    let m = catalogo::voce(id).ok_or_else(|| AppError::msg("Modello sconosciuto"))?;
    let st = app.state::<App>();
    let cancel = tokio_util::sync::CancellationToken::new();
    {
        let mut d = st.download.lock().unwrap_or_else(|e| e.into_inner());
        if d.contains_key(id) {
            return Ok(());
        }
        d.insert(id.to_string(), cancel.clone());
    }
    let app = app.clone();
    let id = id.to_string();
    tauri::async_runtime::spawn(async move {
        let esito: Result<()> = async {
            let dir = crate::sidecar::cartella_modelli(&app).join(m.id);
            if !m.url.is_empty() {
                let sha = (!m.sha.is_empty()).then(|| m.sha.to_string());
                if m.url.ends_with(".tar.bz2") {
                    // Archivio: si scarica, si estrae togliendo la cartella iniziale, si elimina
                    let archivio = dir.join("_archivio.tar.bz2");
                    let f = download::File { url: m.url.into(), dest: archivio.clone(), sha256: sha, size: Some(m.dimensione) };
                    download::scarica(&app, &id, &[f], &cancel).await?;
                    let d = dir.clone();
                    tokio::task::spawn_blocking(move || estrai_bz2(&archivio, &d))
                        .await
                        .map_err(|e| AppError::msg(e.to_string()))??;
                    return Ok(());
                }
                let nome = m.files.first().copied().unwrap_or("file");
                let f = download::File { url: m.url.into(), dest: dir.join(nome), sha256: sha, size: Some(m.dimensione) };
                return download::scarica(&app, &id, &[f], &cancel).await;
            }
            let impronte = download::hf_impronte(m.repo).await?;
            let files: Vec<download::File> = m
                .files
                .iter()
                .map(|f| {
                    let info = impronte.iter().find(|(p, _, _)| p == f);
                    download::File {
                        url: download::hf_url(m.repo, f),
                        dest: dir.join(f),
                        sha256: info.map(|(_, s, _)| s.clone()).filter(|s| !s.is_empty()),
                        size: info.map(|(_, _, n)| *n),
                    }
                })
                .collect();
            download::scarica(&app, &id, &files, &cancel).await
        }
        .await;
        app.state::<App>().download.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
        if let Err(e) = esito {
            download::errore(&app, &id, &e);
        }
    });
    Ok(())
}

pub fn annulla(app: &AppHandle, id: &str) {
    if let Some(c) = app.state::<App>().download.lock().unwrap_or_else(|e| e.into_inner()).remove(id) {
        c.cancel();
    }
}

pub fn elimina(app: &AppHandle, id: &str) -> Result<()> {
    let dir = crate::sidecar::cartella_modelli(app);
    if let Some(nome) = id.strip_prefix("custom:") {
        if nome.contains(['/', '\\']) {
            return Err(AppError::msg("Nome non valido"));
        }
        std::fs::remove_file(dir.join("personali").join(nome))?;
        return Ok(());
    }
    let m = catalogo::voce(id).ok_or_else(|| AppError::msg("Modello sconosciuto"))?;
    let _ = std::fs::remove_dir_all(dir.join(m.id));
    Ok(())
}

/// Copia un file GGUF scelto dall'utente tra i modelli personali.
pub fn importa(app: &AppHandle, path: &str) -> Result<String> {
    let src = std::path::Path::new(path);
    let nome = src.file_name().and_then(|n| n.to_str()).ok_or_else(|| AppError::msg("File non valido"))?.to_string();
    if !nome.to_lowercase().ends_with(".gguf") {
        return Err(AppError::msg("Serve un file .gguf"));
    }
    let dir = crate::sidecar::cartella_modelli(app).join("personali");
    std::fs::create_dir_all(&dir)?;
    let dest = dir.join(&nome);
    // Collegamento fisso se possibile (niente copia di decine di GB), altrimenti copia
    if std::fs::hard_link(src, &dest).is_err() {
        std::fs::copy(src, &dest)?;
    }
    Ok(format!("custom:{nome}"))
}

/// Estrae un archivio .tar.bz2 nella cartella del modello, senza la cartella iniziale.
fn estrai_bz2(archivio: &std::path::Path, dest: &std::path::Path) -> Result<()> {
    let f = std::fs::File::open(archivio)?;
    let mut t = tar::Archive::new(bzip2::read::BzDecoder::new(f));
    for e in t.entries()? {
        let mut e = e?;
        let p = e.path()?.into_owned();
        let rel: std::path::PathBuf = p.components().skip(1).collect();
        if rel.as_os_str().is_empty() || rel.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
            continue;
        }
        let out = dest.join(&rel);
        if let Some(d) = out.parent() {
            std::fs::create_dir_all(d)?;
        }
        e.unpack(&out)?;
    }
    let _ = std::fs::remove_file(archivio);
    Ok(())
}

//! Il motore llama.cpp: build ufficiale fissata, scaricata e verificata con l'impronta SHA-256
//! pubblicata da GitHub, estratta nella cartella dati dell'app.

use crate::error::{AppError, Result};
use crate::modelli::download::{self, File};
use crate::App;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

pub const BUILD: &str = "b11577";

/// (nome dell'archivio, impronta SHA-256) per la piattaforma corrente.
fn archivi() -> Vec<(&'static str, &'static str)> {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        vec![("llama-b11577-bin-macos-arm64.tar.gz", "fa6526afe087293ac83709fe6529fa087e7070f606e30dd452173c16ccebed8c")]
    } else if cfg!(target_os = "macos") {
        vec![("llama-b11577-bin-macos-x64.tar.gz", "a623d9b4a3bf6c4d1c3447b6d7d52b43976ab4b624426158d9ee4ec0a13370ea")]
    } else if cfg!(windows) {
        match variante_windows() {
            "cuda" => vec![
                ("llama-b11577-bin-win-cuda-12.4-x64.zip", "24f5885d2d67b9a61da5f9dcde93d74ae3820b4bb4fcaa738f29f7ab4ba7436c"),
                ("cudart-llama-bin-win-cuda-12.4-x64.zip", "8c79a9b226de4b3cacfd1f83d24f962d0773be79f1e7b75c6af4ded7e32ae1d6"),
            ],
            "vulkan" => vec![("llama-b11577-bin-win-vulkan-x64.zip", "1d9a92d7bd92d41ace1ac21fb3ac2241a8175b932b9229b0d54e8bf625df8a81")],
            _ => vec![("llama-b11577-bin-win-cpu-x64.zip", "c48e5762bb466a2faba1b880ca8b328a152e7f48e738431eb918ac9078f82ef1")],
        }
    } else {
        vec![]
    }
}

/// Su Windows: CUDA se c'è una scheda NVIDIA, altrimenti Vulkan (funziona su quasi tutte le GPU).
fn variante_windows() -> &'static str {
    if !cfg!(windows) {
        return "";
    }
    let nvidia = std::process::Command::new("nvidia-smi").arg("-L").output().map(|o| o.status.success()).unwrap_or(false);
    if nvidia { "cuda" } else { "vulkan" }
}

pub fn backend() -> &'static str {
    if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") { "metal" } else { "cpu" }
    } else if cfg!(windows) {
        match variante_windows() {
            "cuda" => "cuda",
            "vulkan" => "vulkan",
            _ => "cpu",
        }
    } else {
        "cpu"
    }
}

fn cartella(app: &AppHandle) -> PathBuf {
    app.state::<App>().data_dir.join("llama").join(format!("{BUILD}-{}", backend()))
}

fn eseguibile(dir: &Path) -> PathBuf {
    dir.join(if cfg!(windows) { "llama-server.exe" } else { "llama-server" })
}

pub fn percorso(app: &AppHandle) -> Option<PathBuf> {
    let p = eseguibile(&cartella(app));
    p.is_file().then_some(p)
}

pub fn installato(app: &AppHandle) -> bool {
    percorso(app).is_some()
}

/// Estrae l'archivio togliendo la cartella iniziale "llama-bNNNN/" se presente.
fn estrai(archivio: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)?;
    let nome = archivio.to_string_lossy().to_string();
    let togli = |p: &Path| -> PathBuf {
        let mut c = p.components();
        match c.next() {
            Some(std::path::Component::Normal(first)) if first.to_string_lossy().starts_with("llama-") && p.components().count() > 1 => c.as_path().to_path_buf(),
            _ => p.to_path_buf(),
        }
    };
    if nome.ends_with(".tar.gz") {
        let f = std::fs::File::open(archivio)?;
        let mut t = tar::Archive::new(flate2::read::GzDecoder::new(f));
        t.set_preserve_permissions(true);
        for e in t.entries()? {
            let mut e = e?;
            let rel = togli(&e.path()?.into_owned());
            if rel.as_os_str().is_empty() || rel.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
                continue;
            }
            e.unpack(dest.join(rel))?;
        }
    } else {
        let f = std::fs::File::open(archivio)?;
        let mut z = zip::ZipArchive::new(f).map_err(|e| AppError::msg(format!("Archivio non valido: {e}")))?;
        for i in 0..z.len() {
            let mut e = z.by_index(i).map_err(|e| AppError::msg(e.to_string()))?;
            let Some(p) = e.enclosed_name() else { continue };
            let rel = togli(&p);
            let out = dest.join(rel);
            if e.is_dir() {
                std::fs::create_dir_all(&out)?;
                continue;
            }
            if let Some(d) = out.parent() {
                std::fs::create_dir_all(d)?;
            }
            let mut w = std::fs::File::create(&out)?;
            std::io::copy(&mut e, &mut w)?;
        }
    }
    Ok(())
}

pub async fn installa(app: &AppHandle) -> Result<PathBuf> {
    let lista = archivi();
    if lista.is_empty() {
        return Err(AppError::msg("L'AI locale per ora è disponibile su macOS e Windows"));
    }
    let dir = cartella(app);
    let tmp = app.state::<App>().data_dir.join("llama").join("download");
    let files: Vec<File> = lista
        .iter()
        .map(|(nome, sha)| File {
            url: format!("https://github.com/ggml-org/llama.cpp/releases/download/{BUILD}/{nome}"),
            dest: tmp.join(nome),
            sha256: Some(sha.to_string()),
            size: None,
        })
        .collect();
    let cancel = tokio_util::sync::CancellationToken::new();
    download::scarica(app, "motore", &files, &cancel).await?;
    let dir2 = dir.clone();
    let archivi: Vec<PathBuf> = files.iter().map(|f| f.dest.clone()).collect();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let _ = std::fs::remove_dir_all(&dir2);
        for a in &archivi {
            estrai(a, &dir2)?;
        }
        Ok(())
    })
    .await
    .map_err(|e| AppError::msg(e.to_string()))??;
    let _ = std::fs::remove_dir_all(&tmp);
    let exe = eseguibile(&dir);
    if !exe.is_file() {
        return Err(AppError::msg("Il motore scaricato non contiene llama-server"));
    }
    Ok(exe)
}

pub fn percorso_da_stato(app: &App) -> bool {
    eseguibile(&app.data_dir.join("llama").join(format!("{BUILD}-{}", backend()))).is_file()
}

#[cfg(test)]
mod tests {
    /// `OLTREMARE_TEST_LLAMA_TGZ=/percorso/llama-bNNNN-bin-macos-arm64.tar.gz cargo test estrai_archivio_vero -- --ignored`
    #[test]
    #[ignore]
    fn estrai_archivio_vero() {
        let Ok(a) = std::env::var("OLTREMARE_TEST_LLAMA_TGZ") else { return };
        let dest = std::env::temp_dir().join(format!("oltremare-llama-{}", uuid::Uuid::new_v4()));
        super::estrai(std::path::Path::new(&a), &dest).unwrap();
        let exe = super::eseguibile(&dest);
        assert!(exe.is_file(), "manca llama-server in {}", dest.display());
        let out = std::process::Command::new(&exe).arg("--version").output().unwrap();
        let v = String::from_utf8_lossy(&out.stderr).to_string() + &String::from_utf8_lossy(&out.stdout);
        assert!(v.contains("build 11577"), "{v}");
    }
}

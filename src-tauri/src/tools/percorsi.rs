//! Dove possono lavorare gli strumenti sui file: solo nelle cartelle consentite all'agente,
//! mai in quelle sensibili (chiavi, portachiavi, sistema), anche se dentro una consentita.

use crate::shell_env::home;
use std::path::{Component, Path, PathBuf};

/// Cartelle consentite: quelle dell'agente, oppure Scrivania, Documenti e Download.
pub fn radici(consentite: &[String]) -> Vec<PathBuf> {
    let base: Vec<PathBuf> = if consentite.is_empty() {
        home().map(|h| vec![h.join("Desktop"), h.join("Documents"), h.join("Downloads")]).unwrap_or_default()
    } else {
        consentite.iter().map(|p| espandi(p)).collect()
    };
    base.into_iter().filter_map(|p| p.canonicalize().ok()).collect()
}

/// `~/x` → cartella home dell'utente.
pub fn espandi(p: &str) -> PathBuf {
    let p = p.trim();
    if p == "~" {
        return home().unwrap_or_default();
    }
    if let Some(rest) = p.strip_prefix("~/").or_else(|| p.strip_prefix("~\\")) {
        return home().unwrap_or_default().join(rest);
    }
    PathBuf::from(p)
}

/// Mai toccare queste, anche dentro una cartella consentita.
fn vietate() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = Vec::new();
    if let Some(h) = home() {
        for d in [".ssh", ".aws", ".gnupg", ".kube", ".docker", ".config/gcloud", ".azure", "Library/Keychains", "Library/Cookies",
            "Library/Application Support/Google/Chrome", "Library/Application Support/Firefox", "Library/Safari",
            "Library/Application Support/app.oltremare", "Library/Mail", "Library/Messages",
            "AppData/Roaming/Microsoft/Credentials", "AppData/Local/Google/Chrome", "AppData/Roaming/app.oltremare"]
        {
            v.push(h.join(d));
        }
    }
    for d in ["/System", "/Library", "/usr", "/bin", "/sbin", "/etc", "/private/etc", "C:\\Windows", "C:\\Program Files", "C:\\Program Files (x86)"] {
        v.push(PathBuf::from(d));
    }
    v.into_iter().map(|p| p.canonicalize().unwrap_or(p)).collect()
}

/// Percorso assoluto e reale (link simbolici risolti) se è dentro una radice consentita.
/// Per file ancora da creare si risolve la cartella esistente più vicina.
pub fn consentito(p: &str, radici: &[PathBuf]) -> Result<PathBuf, String> {
    let grezzo = espandi(p);
    if !grezzo.is_absolute() {
        return Err(format!("Usa un percorso completo (es. ~/Documents/file.txt), non «{p}»"));
    }
    if grezzo.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err("Il percorso non può contenere «..»".into());
    }
    // Risale fino a una parte esistente, la risolve e riattacca il resto
    let mut esistente = grezzo.clone();
    let mut resto: Vec<std::ffi::OsString> = Vec::new();
    while !esistente.exists() {
        match (esistente.file_name(), esistente.parent()) {
            (Some(n), Some(par)) => {
                resto.push(n.to_os_string());
                esistente = par.to_path_buf();
            }
            _ => return Err(format!("Percorso non valido: {p}")),
        }
    }
    let mut reale = esistente.canonicalize().map_err(|e| format!("Percorso non leggibile: {e}"))?;
    for n in resto.into_iter().rev() {
        reale.push(n);
    }
    if !radici.iter().any(|r| reale.starts_with(r)) {
        let elenco: Vec<String> = radici.iter().map(|r| r.display().to_string()).collect();
        return Err(format!("{} è fuori dalle cartelle consentite a questo agente ({})", reale.display(), elenco.join(", ")));
    }
    if vietate().iter().any(|v| reale.starts_with(v)) {
        return Err(format!("{} è una cartella protetta: non posso accedervi", reale.display()));
    }
    Ok(reale)
}

pub fn mostra(p: &Path) -> String {
    if let Some(h) = home() {
        if let Ok(rest) = p.strip_prefix(&h) {
            return format!("~/{}", rest.display());
        }
    }
    p.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_stay_inside_allowed_roots() {
        let base = std::env::temp_dir().join(format!("oltremare-percorsi-{}", uuid::Uuid::new_v4()));
        let dentro = base.join("consentita");
        let fuori = base.join("fuori");
        std::fs::create_dir_all(&dentro).unwrap();
        std::fs::create_dir_all(&fuori).unwrap();
        std::fs::write(fuori.join("segreto.txt"), "x").unwrap();
        let radici = vec![dentro.canonicalize().unwrap()];

        assert!(consentito(dentro.join("nuovo/file.txt").to_str().unwrap(), &radici).is_ok());
        assert!(consentito(fuori.join("segreto.txt").to_str().unwrap(), &radici).is_err());
        // Uscita con ".."
        assert!(consentito(&format!("{}/../fuori/segreto.txt", dentro.display()), &radici).is_err());
        // Uscita con un link simbolico che punta fuori
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&fuori, dentro.join("scorciatoia")).unwrap();
            assert!(consentito(dentro.join("scorciatoia/segreto.txt").to_str().unwrap(), &radici).is_err());
        }
        assert!(consentito("relativo.txt", &radici).is_err());
    }
}

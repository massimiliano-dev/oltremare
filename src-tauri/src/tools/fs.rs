//! Strumenti sui file. Le scritture sono atomiche, le eliminazioni vanno nel Cestino.

use super::percorsi::{consentito, mostra};
use super::{str_arg, Esito};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const MAX_LETTURA: u64 = 1024 * 1024;
const MAX_VOCI: usize = 500;
const MAX_RISULTATI: usize = 200;
const IGNORA: &[&str] = &["node_modules", ".git", "target", ".Trash", "Library", ".cache", "__pycache__", ".venv"];

fn data(t: std::io::Result<SystemTime>) -> String {
    t.ok()
        .map(|t| chrono::DateTime::<chrono::Local>::from(t).format("%d/%m/%Y %H:%M").to_string())
        .unwrap_or_default()
}

fn dim(b: u64) -> String {
    if b < 1024 {
        format!("{b} B")
    } else if b < 1024 * 1024 {
        format!("{:.0} KB", b as f64 / 1024.0)
    } else {
        format!("{:.1} MB", b as f64 / 1024.0 / 1024.0)
    }
}

fn binario(bytes: &[u8]) -> bool {
    bytes.iter().take(8000).any(|&b| b == 0)
}

pub fn esegui(nome: &str, input: &Value, radici: &[PathBuf]) -> Esito {
    let r = match nome {
        "leggi_file" => leggi(input, radici),
        "elenca_cartella" => elenca(input, radici),
        "cerca_file" => cerca(input, radici),
        "info_file" => info(input, radici),
        "scrivi_file" => scrivi(input, radici),
        "crea_cartella" => crea_cartella(input, radici),
        "sposta_file" => sposta(input, radici),
        "cestina_file" => cestina(input, radici),
        _ => Err("strumento sconosciuto".into()),
    };
    r.unwrap_or_else(Esito::errore)
}

fn leggi(input: &Value, radici: &[PathBuf]) -> Result<Esito, String> {
    let p = consentito(str_arg(input, "percorso"), radici)?;
    let meta = std::fs::metadata(&p).map_err(|_| format!("File non trovato: {}", mostra(&p)))?;
    if meta.is_dir() {
        return Err(format!("{} è una cartella: usa elenca_cartella", mostra(&p)));
    }
    if meta.len() > MAX_LETTURA * 20 {
        return Err(format!("File troppo grande ({}): leggine una parte con offset e limite su un file più piccolo", dim(meta.len())));
    }
    let bytes = std::fs::read(&p).map_err(|e| e.to_string())?;
    if binario(&bytes) {
        return Err(format!("{} non è un file di testo (PDF, immagini e Office si allegano alla chat)", mostra(&p)));
    }
    let testo = String::from_utf8_lossy(&bytes);
    let righe: Vec<&str> = testo.lines().collect();
    let offset = input["offset"].as_u64().unwrap_or(1).max(1) as usize;
    let limite = input["limite"].as_u64().unwrap_or(400).clamp(1, 4000) as usize;
    let fine = (offset - 1 + limite).min(righe.len());
    let mut out = format!("{} — {} righe totali", mostra(&p), righe.len());
    if offset > 1 || fine < righe.len() {
        out.push_str(&format!(", mostrate {}–{}", offset.min(righe.len()), fine));
    }
    out.push('\n');
    let mut corpo = String::new();
    for (k, r) in righe.iter().enumerate().take(fine).skip(offset - 1) {
        corpo.push_str(&format!("{:>5}\t{}\n", k + 1, r));
        if corpo.len() > MAX_LETTURA as usize {
            corpo.push_str("[…troncato…]\n");
            break;
        }
    }
    out.push_str(&corpo);
    let anteprima: String = righe.iter().skip(offset - 1).take(12).cloned().collect::<Vec<_>>().join("\n");
    Ok(Esito::ok(out, json!({ "kind": "fs", "op": "read", "path": mostra(&p), "bytes": meta.len(), "lines": righe.len(), "text": anteprima })))
}

fn elenca(input: &Value, radici: &[PathBuf]) -> Result<Esito, String> {
    let p = consentito(str_arg(input, "percorso"), radici)?;
    if !p.is_dir() {
        return Err(format!("{} non è una cartella", mostra(&p)));
    }
    let ricorsivo = input["ricorsivo"].as_bool().unwrap_or(false);
    let mut voci: Vec<String> = Vec::new();
    fn giro(dir: &Path, base: &Path, livello: usize, max: usize, voci: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        let mut ent: Vec<_> = rd.flatten().collect();
        ent.sort_by_key(|e| (!e.path().is_dir(), e.file_name().to_string_lossy().to_lowercase()));
        for e in ent {
            if voci.len() >= MAX_VOCI {
                return;
            }
            let nome = e.file_name().to_string_lossy().to_string();
            if nome.starts_with('.') {
                continue;
            }
            let path = e.path();
            let rel = path.strip_prefix(base).unwrap_or(&path).display().to_string();
            let meta = e.metadata().ok();
            if path.is_dir() {
                voci.push(format!("{rel}/"));
                if livello < max && !IGNORA.contains(&nome.as_str()) {
                    giro(&path, base, livello + 1, max, voci);
                }
            } else {
                let m = meta.as_ref();
                voci.push(format!("{rel}  ({}, {})", dim(m.map_or(0, |m| m.len())), data(m.map_or(Err(std::io::Error::other("")), |m| m.modified()))));
            }
        }
    }
    giro(&p, &p, 1, if ricorsivo { 3 } else { 1 }, &mut voci);
    let mut out = format!("{} — {} voci{}\n", mostra(&p), voci.len(), if voci.len() >= MAX_VOCI { " (elenco troncato)" } else { "" });
    out.push_str(&voci.join("\n"));
    let entries: Vec<&String> = voci.iter().take(200).collect();
    Ok(Esito::ok(out, json!({ "kind": "fs", "op": "list", "path": mostra(&p), "entries": entries })))
}

/// `*` e `?` sul nome del file, senza distinzione tra maiuscole e minuscole.
pub fn corrisponde(schema: &str, nome: &str) -> bool {
    fn m(p: &[char], n: &[char]) -> bool {
        match (p.first(), n.first()) {
            (None, None) => true,
            (Some('*'), _) => m(&p[1..], n) || (!n.is_empty() && m(p, &n[1..])),
            (Some('?'), Some(_)) => m(&p[1..], &n[1..]),
            (Some(a), Some(b)) => a == b && m(&p[1..], &n[1..]),
            _ => false,
        }
    }
    let p: Vec<char> = schema.to_lowercase().chars().collect();
    let n: Vec<char> = nome.to_lowercase().chars().collect();
    if !schema.contains(['*', '?']) {
        return nome.to_lowercase().contains(&schema.to_lowercase());
    }
    m(&p, &n)
}

fn cerca(input: &Value, radici: &[PathBuf]) -> Result<Esito, String> {
    let dir = consentito(str_arg(input, "cartella"), radici)?;
    let nome = str_arg(input, "nome").trim().to_string();
    let testo = str_arg(input, "testo").trim().to_lowercase();
    if nome.is_empty() && testo.is_empty() {
        return Err("Indica il nome da cercare, il testo, o entrambi".into());
    }
    let mut trovati: Vec<String> = Vec::new();
    let mut visitati = 0usize;
    let mut pila = vec![(dir.clone(), 0usize)];
    while let Some((d, livello)) = pila.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            visitati += 1;
            if trovati.len() >= MAX_RISULTATI || visitati > 50_000 {
                break;
            }
            let fname = e.file_name().to_string_lossy().to_string();
            if fname.starts_with('.') {
                continue;
            }
            let path = e.path();
            if path.is_dir() {
                if livello < 8 && !IGNORA.contains(&fname.as_str()) {
                    pila.push((path, livello + 1));
                }
                continue;
            }
            if !nome.is_empty() && !corrisponde(&nome, &fname) {
                continue;
            }
            if !testo.is_empty() {
                let Ok(meta) = e.metadata() else { continue };
                if meta.len() > 2 * 1024 * 1024 {
                    continue;
                }
                let Ok(bytes) = std::fs::read(&path) else { continue };
                if binario(&bytes) {
                    continue;
                }
                let t = String::from_utf8_lossy(&bytes);
                let Some((riga, contenuto)) = t.lines().enumerate().find(|(_, l)| l.to_lowercase().contains(&testo)) else { continue };
                trovati.push(format!("{}:{}: {}", mostra(&path), riga + 1, contenuto.trim().chars().take(160).collect::<String>()));
            } else {
                trovati.push(mostra(&path));
            }
        }
    }
    let mut out = format!("{} risultati in {}\n", trovati.len(), mostra(&dir));
    out.push_str(&trovati.join("\n"));
    Ok(Esito::ok(out, json!({ "kind": "fs", "op": "search", "path": mostra(&dir), "entries": trovati.iter().take(100).collect::<Vec<_>>() })))
}

fn info(input: &Value, radici: &[PathBuf]) -> Result<Esito, String> {
    let p = consentito(str_arg(input, "percorso"), radici)?;
    let m = std::fs::metadata(&p).map_err(|_| format!("Non trovato: {}", mostra(&p)))?;
    let tipo = if m.is_dir() { "cartella".to_string() } else { format!("file .{}", p.extension().and_then(|e| e.to_str()).unwrap_or("")) };
    let t = format!(
        "{}\nTipo: {tipo}\nDimensione: {}\nCreato: {}\nModificato: {}",
        mostra(&p),
        dim(m.len()),
        data(m.created()),
        data(m.modified())
    );
    Ok(Esito::ok(t.clone(), json!({ "kind": "text", "text": t })))
}

fn diff(prima: &str, dopo: &str) -> String {
    let d = similar::TextDiff::from_lines(prima, dopo);
    let mut s = d.unified_diff().context_radius(2).header("prima", "dopo").to_string();
    let righe: Vec<&str> = s.lines().collect();
    if righe.len() > 400 {
        s = righe[..400].join("\n") + "\n[…altre differenze…]";
    }
    s
}

/// Cosa cambierà, da mostrare prima di scrivere.
pub fn anteprima_scrittura(input: &Value, radici: &[PathBuf]) -> (String, Option<String>) {
    let contenuto = str_arg(input, "contenuto");
    let modalita = str_arg(input, "modalita");
    let Ok(p) = consentito(str_arg(input, "percorso"), radici) else { return (str_arg(input, "percorso").to_string(), None) };
    let prima = std::fs::read_to_string(&p).ok();
    let dopo = match (modalita, &prima) {
        ("aggiungi", Some(v)) => format!("{v}{contenuto}"),
        _ => contenuto.to_string(),
    };
    let righe = dopo.lines().count();
    match prima {
        Some(v) => (format!("Modifica {} ({} → {} righe)", mostra(&p), v.lines().count(), righe), Some(diff(&v, &dopo))),
        None => (format!("Nuovo file {} ({righe} righe)", mostra(&p)), Some(diff("", &dopo))),
    }
}

fn scrivi(input: &Value, radici: &[PathBuf]) -> Result<Esito, String> {
    let p = consentito(str_arg(input, "percorso"), radici)?;
    let contenuto = str_arg(input, "contenuto");
    let modalita = match str_arg(input, "modalita") {
        "" => "sovrascrivi",
        m => m,
    };
    if p.is_dir() {
        return Err(format!("{} è una cartella", mostra(&p)));
    }
    let prima = std::fs::read_to_string(&p).ok();
    if modalita == "nuovo" && p.exists() {
        return Err(format!("{} esiste già: usa modalita «sovrascrivi» o «aggiungi»", mostra(&p)));
    }
    let dopo = match (modalita, &prima) {
        ("aggiungi", Some(v)) => format!("{v}{contenuto}"),
        _ => contenuto.to_string(),
    };
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = p.with_extension(format!("{}.oltremare-tmp", p.extension().and_then(|e| e.to_str()).unwrap_or("")));
    std::fs::write(&tmp, &dopo).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &p).map_err(|e| e.to_string())?;
    let d = diff(prima.as_deref().unwrap_or(""), &dopo);
    let verbo = if prima.is_some() { "Aggiornato" } else { "Creato" };
    Ok(Esito::ok(
        format!("{verbo} {} ({} righe, {})", mostra(&p), dopo.lines().count(), dim(dopo.len() as u64)),
        json!({ "kind": "fs", "op": "write", "path": mostra(&p), "bytes": dopo.len(), "diff": d }),
    ))
}

fn crea_cartella(input: &Value, radici: &[PathBuf]) -> Result<Esito, String> {
    let p = consentito(str_arg(input, "percorso"), radici)?;
    std::fs::create_dir_all(&p).map_err(|e| e.to_string())?;
    Ok(Esito::ok(format!("Cartella pronta: {}", mostra(&p)), json!({ "kind": "fs", "op": "mkdir", "path": mostra(&p) })))
}

fn sposta(input: &Value, radici: &[PathBuf]) -> Result<Esito, String> {
    let da = consentito(str_arg(input, "da"), radici)?;
    let mut a = consentito(str_arg(input, "a"), radici)?;
    if !da.exists() {
        return Err(format!("Non trovato: {}", mostra(&da)));
    }
    // Destinazione cartella esistente: ci si sposta dentro
    if a.is_dir() {
        a = a.join(da.file_name().unwrap_or_default());
    }
    if a.exists() {
        return Err(format!("{} esiste già: scegli un altro nome", mostra(&a)));
    }
    if let Some(dir) = a.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&da, &a).map_err(|e| e.to_string())?;
    Ok(Esito::ok(format!("Spostato {} → {}", mostra(&da), mostra(&a)), json!({ "kind": "fs", "op": "move", "path": mostra(&a), "from": mostra(&da) })))
}

fn cestina(input: &Value, radici: &[PathBuf]) -> Result<Esito, String> {
    let p = consentito(str_arg(input, "percorso"), radici)?;
    if !p.exists() {
        return Err(format!("Non trovato: {}", mostra(&p)));
    }
    if radici.iter().any(|r| r == &p) {
        return Err("Non posso cestinare una delle cartelle consentite".into());
    }
    trash::delete(&p).map_err(|e| format!("Non riesco a spostarlo nel Cestino: {e}"))?;
    Ok(Esito::ok(format!("Spostato nel Cestino: {}", mostra(&p)), json!({ "kind": "fs", "op": "trash", "path": mostra(&p) })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_matching() {
        assert!(corrisponde("*.pdf", "Fattura.PDF"));
        assert!(corrisponde("fatt?ra*", "fattura_2026.pdf"));
        assert!(!corrisponde("*.pdf", "nota.txt"));
        assert!(corrisponde("fattura", "Vecchia fattura.pdf"));
    }

    #[test]
    fn write_read_list_move() {
        let d = std::env::temp_dir().join(format!("oltremare-fs-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        let r = vec![d.canonicalize().unwrap()];
        let f = d.join("sotto/nota.txt");
        let w = esegui("scrivi_file", &json!({"percorso": f, "contenuto": "uno\ndue\n", "modalita": "nuovo"}), &r);
        assert!(!w.is_error, "{}", w.testo);
        let again = esegui("scrivi_file", &json!({"percorso": f, "contenuto": "x", "modalita": "nuovo"}), &r);
        assert!(again.is_error);
        esegui("scrivi_file", &json!({"percorso": f, "contenuto": "tre\n", "modalita": "aggiungi"}), &r);
        let l = esegui("leggi_file", &json!({"percorso": f, "offset": 2}), &r);
        assert!(l.testo.contains("3 righe") && l.testo.contains("due") && !l.testo.contains("\tuno"), "{}", l.testo);
        let s = esegui("cerca_file", &json!({"cartella": d, "testo": "TRE"}), &r);
        assert!(s.testo.contains("nota.txt:3"), "{}", s.testo);
        let m = esegui("sposta_file", &json!({"da": f, "a": d.join("archivio/nota.txt")}), &r);
        assert!(!m.is_error && d.join("archivio/nota.txt").exists());
        let e = esegui("elenca_cartella", &json!({"percorso": d, "ricorsivo": true}), &r);
        assert!(e.testo.contains("archivio/nota.txt"), "{}", e.testo);
        let (sum, diff) = anteprima_scrittura(&json!({"percorso": d.join("archivio/nota.txt"), "contenuto": "nuovo\n"}), &r);
        assert!(sum.starts_with("Modifica") && diff.unwrap().contains("+nuovo"));
    }
}

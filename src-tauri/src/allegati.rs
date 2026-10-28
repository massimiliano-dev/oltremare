//! File allegati ai messaggi: PDF, immagini, fogli di calcolo, documenti Office, testo.
//!
//! Ogni file viene copiato nella cartella dati dell'app (resta disponibile anche se l'originale
//! si sposta). PDF e immagini vanno ai modelli così come sono; Excel, Word, PowerPoint e i file
//! di testo diventano testo, che ogni provider sa leggere.

use crate::agent::messaggi::Blocco;
use crate::error::{AppError, Result};
use base64::Engine;
use serde_json::{json, Value};
use std::io::Read;
use std::path::{Path, PathBuf};

const MAX_PDF: u64 = 30 * 1024 * 1024;
const MAX_IMMAGINE: u64 = 5 * 1024 * 1024;
const MAX_FILE: u64 = 50 * 1024 * 1024;
const MAX_TESTO: usize = 400_000;

const TESTO: &[&str] = &[
    "txt", "md", "markdown", "csv", "tsv", "xml", "json", "jsonl", "yaml", "yml", "toml", "ini", "cfg", "conf", "log", "html", "htm",
    "css", "js", "ts", "jsx", "tsx", "svelte", "vue", "py", "rs", "go", "java", "kt", "swift", "c", "h", "cpp", "hpp", "cs", "rb", "php",
    "sql", "sh", "zsh", "ps1", "bat", "r", "tex", "srt", "vtt", "eml", "ics", "vcf", "xsd", "xsl", "svg", "plist", "env",
];
const FOGLI: &[&str] = &["xlsx", "xlsm", "xlsb", "xls", "ods"];

fn estensione(p: &Path) -> String {
    p.extension().and_then(|e| e.to_str()).unwrap_or_default().to_lowercase()
}

fn mime_immagine(ext: &str) -> Option<&'static str> {
    match ext {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

/// Formati accettati, per il selettore di file del frontend.
pub fn estensioni_supportate() -> Vec<&'static str> {
    let mut v = vec!["pdf", "png", "jpg", "jpeg", "gif", "webp", "docx", "pptx", "odt"];
    v.extend_from_slice(FOGLI);
    v.extend_from_slice(TESTO);
    v
}

fn tronca(mut t: String) -> String {
    if t.len() > MAX_TESTO {
        let mut cut = MAX_TESTO;
        while !t.is_char_boundary(cut) {
            cut -= 1;
        }
        t.truncate(cut);
        t.push_str("\n\n[…file troncato: era troppo lungo…]");
    }
    t
}

fn xml_testo(xml: &str, paragrafo: &str, tab: Option<&str>) -> String {
    let mut s = xml.replace(paragrafo, "\n");
    if let Some(tab) = tab {
        s = s.replace(tab, "\t");
    }
    let re = regex::Regex::new(r"<[^>]+>").expect("regex");
    let s = re.replace_all(&s, "");
    let s = s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&");
    let righe: Vec<&str> = s.lines().map(str::trim_end).collect();
    righe.join("\n").trim().to_string()
}

fn office(path: &Path, ext: &str) -> Result<String> {
    let file = std::fs::File::open(path)?;
    let mut z = zip::ZipArchive::new(file).map_err(|_| AppError::msg("Il documento è danneggiato o protetto"))?;
    let leggi = |z: &mut zip::ZipArchive<std::fs::File>, nome: &str| -> Option<String> {
        let mut f = z.by_name(nome).ok()?;
        let mut s = String::new();
        f.read_to_string(&mut s).ok()?;
        Some(s)
    };
    match ext {
        "docx" => {
            let xml = leggi(&mut z, "word/document.xml").ok_or_else(|| AppError::msg("Documento Word non leggibile"))?;
            Ok(xml_testo(&xml, "</w:p>", Some("<w:tab/>")))
        }
        "odt" => {
            let xml = leggi(&mut z, "content.xml").ok_or_else(|| AppError::msg("Documento non leggibile"))?;
            Ok(xml_testo(&xml.replace("</text:h>", "</text:p>"), "</text:p>", Some("<text:tab/>")))
        }
        "pptx" => {
            let mut slide: Vec<(usize, String)> = (0..z.len())
                .filter_map(|i| {
                    let n = z.by_index(i).ok()?.name().ok()?.to_string();
                    let num = n.strip_prefix("ppt/slides/slide")?.strip_suffix(".xml")?.parse().ok()?;
                    Some((num, n))
                })
                .collect();
            slide.sort();
            let mut out = String::new();
            for (num, nome) in slide {
                if let Some(xml) = leggi(&mut z, &nome) {
                    out.push_str(&format!("### Diapositiva {num}\n{}\n\n", xml_testo(&xml, "</a:p>", None)));
                }
            }
            Ok(out)
        }
        _ => Err(AppError::msg("Formato non supportato")),
    }
}

fn foglio(path: &Path) -> Result<String> {
    use calamine::{open_workbook_auto, Data, Reader};
    let mut wb = open_workbook_auto(path).map_err(|e| AppError::msg(format!("Foglio di calcolo non leggibile: {e}")))?;
    let mut out = String::new();
    for nome in wb.sheet_names().to_owned() {
        let Ok(range) = wb.worksheet_range(&nome) else { continue };
        out.push_str(&format!("### Foglio «{nome}» ({} righe × {} colonne)\n", range.height(), range.width()));
        for riga in range.rows() {
            let celle: Vec<String> = riga
                .iter()
                .map(|c| match c {
                    Data::Empty => String::new(),
                    Data::String(s) => s.replace(['\t', '\n'], " "),
                    Data::Float(f) => {
                        if f.fract() == 0.0 && f.abs() < 1e15 { format!("{}", *f as i64) } else { f.to_string() }
                    }
                    Data::Int(i) => i.to_string(),
                    Data::Bool(b) => if *b { "VERO".into() } else { "FALSO".into() },
                    Data::DateTime(d) => {
                        let (a, m, g, h, mi, se, _) = d.to_ymd_hms_milli();
                        if (h, mi, se) == (0, 0, 0) { format!("{g:02}/{m:02}/{a}") } else { format!("{g:02}/{m:02}/{a} {h:02}:{mi:02}") }
                    }
                    other => other.to_string(),
                })
                .collect();
            if celle.iter().any(|c| !c.is_empty()) {
                out.push_str(&celle.join("\t"));
                out.push('\n');
            }
            if out.len() > MAX_TESTO {
                break;
            }
        }
        out.push('\n');
    }
    Ok(out)
}

fn nome_sicuro(nome: &str) -> String {
    nome.chars().map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c }).collect()
}

/// Copia e converte un file, restituendo il blocco da mettere nel messaggio dell'utente.
pub fn prepara(src: &Path, dest: &Path) -> Result<Blocco> {
    let meta = std::fs::metadata(src).map_err(|_| AppError::msg(format!("File non trovato: {}", src.display())))?;
    if !meta.is_file() {
        return Err(AppError::msg("Si possono allegare solo file, non cartelle"));
    }
    let nome = src.file_name().and_then(|n| n.to_str()).unwrap_or("file").to_string();
    let ext = estensione(src);
    let byte = meta.len();
    if byte > MAX_FILE {
        return Err(AppError::msg(format!("«{nome}» è troppo grande (massimo 50 MB)")));
    }
    std::fs::create_dir_all(dest)?;
    let id = &uuid::Uuid::new_v4().to_string()[..8];
    let base = dest.join(format!("{id}-{}", nome_sicuro(&nome)));

    let (tipo, mime, percorso): (&str, String, PathBuf) = if ext == "pdf" {
        if byte > MAX_PDF {
            return Err(AppError::msg(format!("«{nome}» è troppo grande: i PDF possono arrivare a 30 MB")));
        }
        std::fs::copy(src, &base)?;
        ("pdf", "application/pdf".into(), base)
    } else if let Some(m) = mime_immagine(&ext) {
        if byte > MAX_IMMAGINE {
            return Err(AppError::msg(format!("«{nome}» è troppo grande: le immagini possono arrivare a 5 MB")));
        }
        std::fs::copy(src, &base)?;
        ("image", m.into(), base)
    } else {
        let testo = if FOGLI.contains(&ext.as_str()) {
            foglio(src)?
        } else if matches!(ext.as_str(), "docx" | "pptx" | "odt") {
            office(src, &ext)?
        } else if TESTO.contains(&ext.as_str()) || ext.is_empty() {
            let bytes = std::fs::read(src)?;
            if bytes.iter().take(8000).any(|&b| b == 0) {
                return Err(AppError::msg(format!("«{nome}» non sembra un file di testo")));
            }
            String::from_utf8_lossy(&bytes).into_owned()
        } else {
            return Err(AppError::msg(format!("Formato .{ext} non supportato: allega PDF, immagini, Excel, Word, PowerPoint o file di testo")));
        };
        if testo.trim().is_empty() {
            return Err(AppError::msg(format!("«{nome}» non contiene testo leggibile")));
        }
        let txt = base.with_file_name(format!("{}.txt", base.file_name().unwrap_or_default().to_string_lossy()));
        std::fs::write(&txt, tronca(testo))?;
        ("text", "text/plain".into(), txt)
    };
    Ok(Blocco::Allegato { nome, mime, formato: tipo.into(), percorso: percorso.to_string_lossy().into_owned(), byte })
}

/// Il contenuto pronto per il provider.
pub enum Contenuto {
    Pdf(String),
    Immagine { mime: String, base64: String },
    Testo(String),
    Mancante,
}

pub fn carica(tipo: &str, mime: &str, percorso: &str) -> Contenuto {
    let Ok(bytes) = std::fs::read(percorso) else { return Contenuto::Mancante };
    let b64 = || base64::engine::general_purpose::STANDARD.encode(&bytes);
    match tipo {
        "pdf" => Contenuto::Pdf(b64()),
        "image" => Contenuto::Immagine { mime: mime.to_string(), base64: b64() },
        _ => Contenuto::Testo(String::from_utf8_lossy(&bytes).into_owned()),
    }
}

/// Descrizione per la timeline: nome, dimensione, tipo.
pub fn vista(b: &Blocco) -> Option<Value> {
    if let Blocco::Allegato { nome, formato: tipo, byte, .. } = b {
        Some(json!({ "name": nome, "size": byte, "kind": tipo }))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(n: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("oltremare-allegati-{n}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn text_files_are_copied_as_text() {
        let d = tmp("txt");
        let src = d.join("dati.xml");
        std::fs::write(&src, "<a>ciao</a>").unwrap();
        let b = prepara(&src, &d.join("out")).unwrap();
        let Blocco::Allegato { formato: tipo, percorso, nome, .. } = b else { panic!() };
        assert_eq!((tipo.as_str(), nome.as_str()), ("text", "dati.xml"));
        assert!(matches!(carica("text", "text/plain", &percorso), Contenuto::Testo(t) if t == "<a>ciao</a>"));
    }

    #[test]
    fn word_document_becomes_text() {
        let d = tmp("docx");
        let src = d.join("lettera.docx");
        let f = std::fs::File::create(&src).unwrap();
        let mut z = zip::ZipWriter::new(f);
        z.start_file("word/document.xml", zip::write::SimpleFileOptions::default()).unwrap();
        std::io::Write::write_all(&mut z, br#"<w:document><w:body><w:p><w:r><w:t>Gentile Mario,</w:t></w:r></w:p><w:p><w:r><w:t>prezzo &lt; 10 &amp; IVA</w:t></w:r></w:p></w:body></w:document>"#).unwrap();
        z.finish().unwrap();
        let Blocco::Allegato { percorso, .. } = prepara(&src, &d.join("out")).unwrap() else { panic!() };
        assert_eq!(std::fs::read_to_string(percorso).unwrap(), "Gentile Mario,\nprezzo < 10 & IVA");
    }

    #[test]
    fn excel_sheet_becomes_tab_separated_text() {
        let d = tmp("xlsx");
        let src = d.join("listino.xlsx");
        let f = std::fs::File::create(&src).unwrap();
        let mut z = zip::ZipWriter::new(f);
        let o = zip::write::SimpleFileOptions::default();
        let files: [(&str, &str); 5] = [
            ("[Content_Types].xml", r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>"#),
            ("_rels/.rels", r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#),
            ("xl/workbook.xml", r#"<?xml version="1.0"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Prezzi" sheetId="1" r:id="rId1"/></sheets></workbook>"#),
            ("xl/_rels/workbook.xml.rels", r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#),
            ("xl/worksheets/sheet1.xml", r#"<?xml version="1.0"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>Prodotto</t></is></c><c r="B1" t="inlineStr"><is><t>Prezzo</t></is></c></row><row r="2"><c r="A2" t="inlineStr"><is><t>Switch</t></is></c><c r="B2"><v>149.5</v></c></row><row r="3"><c r="A3" t="inlineStr"><is><t>Cavo</t></is></c><c r="B3"><v>12</v></c></row></sheetData></worksheet>"#),
        ];
        for (n, c) in files {
            z.start_file(n, o).unwrap();
            std::io::Write::write_all(&mut z, c.as_bytes()).unwrap();
        }
        z.finish().unwrap();
        let Blocco::Allegato { percorso, formato, .. } = prepara(&src, &d.join("out")).unwrap() else { panic!() };
        assert_eq!(formato, "text");
        let t = std::fs::read_to_string(percorso).unwrap();
        assert!(t.contains("### Foglio «Prezzi» (3 righe × 2 colonne)"), "{t}");
        assert!(t.contains("Switch\t149.5") && t.contains("Cavo\t12"), "{t}");
    }

    #[test]
    fn unsupported_and_binary_files_are_refused() {
        let d = tmp("bin");
        let exe = d.join("prog.exe");
        std::fs::write(&exe, [0u8, 1, 2]).unwrap();
        assert!(prepara(&exe, &d).unwrap_err().to_string().contains("non supportato"));
        let log = d.join("x.log");
        std::fs::write(&log, [b'a', 0, b'b']).unwrap();
        assert!(prepara(&log, &d).unwrap_err().to_string().contains("non sembra"));
    }
}

//! Creare contenuti: PDF, presentazioni, immagini, video. Tutto in locale quando si può:
//! PDF con il Chrome già usato dagli agenti, presentazioni con Marp, immagini con Draw Things
//! (o OpenAI se c'è la chiave), video con FFmpeg.

use super::percorsi::{consentito, mostra};
use super::{str_arg, Esito};
use crate::shell_env::{home, path_utente};
use base64::Engine;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Percorso di uscita: quello chiesto (dentro le cartelle consentite) oppure Documenti/Oltremare/<nome>.
fn uscita(input: &Value, radici: &[PathBuf], base: &str, est: &str) -> Result<PathBuf, String> {
    let p = str_arg(input, "uscita").trim();
    let percorso = if p.is_empty() {
        let dir = home().ok_or("cartella utente non trovata")?.join("Documents").join("Oltremare");
        let stamp = chrono::Local::now().format("%Y-%m-%d %H.%M.%S");
        dir.join(format!("{base} {stamp}.{est}")).to_string_lossy().into_owned()
    } else {
        p.to_string()
    };
    let reale = consentito(&percorso, radici)?;
    if let Some(d) = reale.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("Non riesco a creare la cartella: {e}"))?;
    }
    Ok(reale)
}

fn esito_file(cosa: &str, p: &Path, extra: &str) -> Esito {
    let t = format!("{cosa} creato: {}{extra}", mostra(p));
    Esito::ok(t.clone(), json!({ "kind": "fs", "op": "write", "path": mostra(p), "text": t }))
}

const CSS_DOC: &str = r#"
@page { margin: 18mm 16mm; }
body { font: 11pt/1.55 -apple-system, "Helvetica Neue", Arial, sans-serif; color: #1d1b2a; max-width: 100%; }
h1 { font-size: 22pt; margin: 0 0 10pt; color: #2b2373; } h2 { font-size: 15pt; margin: 18pt 0 6pt; color: #2b2373; }
h3 { font-size: 12.5pt; margin: 14pt 0 4pt; } p, ul, ol { margin: 0 0 8pt; } li { margin: 2pt 0; }
table { border-collapse: collapse; width: 100%; margin: 8pt 0 12pt; font-size: 10pt; }
th, td { border: 1px solid #d9d6ea; padding: 5pt 7pt; text-align: left; vertical-align: top; } th { background: #f1effa; }
code { font: 9.5pt ui-monospace, Menlo, monospace; background: #f4f3f9; padding: 1pt 3pt; border-radius: 3pt; }
pre { background: #f4f3f9; padding: 8pt 10pt; border-radius: 5pt; overflow: hidden; white-space: pre-wrap; } pre code { background: none; padding: 0; }
blockquote { margin: 8pt 0; padding: 2pt 10pt; border-left: 3pt solid #6d5fd8; color: #4a4760; }
img { max-width: 100%; } hr { border: 0; border-top: 1px solid #d9d6ea; margin: 14pt 0; } a { color: #4b3fc4; }
"#;

pub fn markdown_html(md: &str) -> String {
    use pulldown_cmark::{html, Options, Parser};
    let mut opz = Options::empty();
    opz.insert(Options::ENABLE_TABLES);
    opz.insert(Options::ENABLE_STRIKETHROUGH);
    opz.insert(Options::ENABLE_TASKLISTS);
    let mut out = String::new();
    html::push_html(&mut out, Parser::new_ext(md, opz));
    out
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// HTML → PDF con Chrome senza finestra (un'istanza a parte, con un profilo temporaneo).
async fn stampa_pdf(html: &str, carta: (f64, f64), orizzontale: bool, margini: bool) -> Result<Vec<u8>, String> {
    use chromiumoxide::cdp::browser_protocol::page::PrintToPdfParams;
    use chromiumoxide::{Browser, BrowserConfig};
    use futures::StreamExt;
    let exe = super::browser::trova_browser().ok_or("Serve Chrome o Edge per creare i PDF")?;
    let tmp = std::env::temp_dir().join(format!("oltremare-pdf-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let file = tmp.join("documento.html");
    std::fs::write(&file, html).map_err(|e| e.to_string())?;
    let cfg = BrowserConfig::builder()
        .chrome_executable(exe)
        .user_data_dir(tmp.join("profilo"))
        .new_headless_mode()
        .args(["--no-first-run", "--allow-file-access-from-files"])
        .build()
        .map_err(|e| e.to_string())?;
    let (mut browser, mut handler) = Browser::launch(cfg).await.map_err(|e| format!("Chrome non si avvia: {e}"))?;
    let h = tokio::spawn(async move { while let Some(x) = handler.next().await { if x.is_err() { break; } } });
    let risultato = async {
        let page = browser.new_page(format!("file://{}", file.display())).await.map_err(|e| e.to_string())?;
        let _ = page.wait_for_navigation().await;
        tokio::time::sleep(Duration::from_millis(400)).await; // immagini e font
        let m = if margini { 0.6 } else { 0.0 };
        let params = PrintToPdfParams::builder()
            .print_background(true)
            .landscape(orizzontale)
            .paper_width(carta.0)
            .paper_height(carta.1)
            .margin_top(m)
            .margin_bottom(m)
            .margin_left(m)
            .margin_right(m)
            .prefer_css_page_size(!margini)
            .build();
        page.pdf(params).await.map_err(|e| format!("Stampa in PDF non riuscita: {e}"))
    }
    .await;
    let _ = browser.close().await;
    h.abort();
    let _ = std::fs::remove_dir_all(&tmp);
    risultato
}

async fn crea_pdf(input: &Value, radici: &[PathBuf]) -> Esito {
    let contenuto = str_arg(input, "contenuto");
    if contenuto.trim().is_empty() {
        return Esito::errore("Contenuto vuoto");
    }
    let out = match uscita(input, radici, "Documento", "pdf") {
        Ok(p) => p,
        Err(e) => return Esito::errore(e),
    };
    let titolo = str_arg(input, "titolo");
    let corpo = if str_arg(input, "formato") == "html" { contenuto.to_string() } else { markdown_html(contenuto) };
    let html = if corpo.contains("<html") {
        corpo
    } else {
        format!("<!doctype html><html lang=\"it\"><head><meta charset=\"utf-8\"><title>{}</title><style>{CSS_DOC}</style></head><body>{corpo}</body></html>", escape(titolo))
    };
    let carta = if str_arg(input, "carta").eq_ignore_ascii_case("letter") { (8.5, 11.0) } else { (8.27, 11.69) };
    match stampa_pdf(&html, carta, input["orizzontale"].as_bool().unwrap_or(false), true).await {
        Ok(b) => match std::fs::write(&out, &b) {
            Ok(()) => esito_file("PDF", &out, &format!(" ({} KB)", b.len() / 1024)),
            Err(e) => Esito::errore(e.to_string()),
        },
        Err(e) => Esito::errore(e),
    }
}

fn marp() -> Option<PathBuf> {
    let sep = if cfg!(windows) { ';' } else { ':' };
    path_utente().split(sep).map(|d| Path::new(d).join(if cfg!(windows) { "marp.cmd" } else { "marp" })).find(|p| p.is_file())
}

async fn crea_presentazione(input: &Value, radici: &[PathBuf]) -> Esito {
    let slide = str_arg(input, "slide");
    if slide.trim().is_empty() {
        return Esito::errore("Nessuna slide: scrivile in Markdown separate da una riga con ---");
    }
    let formato = match str_arg(input, "formato") {
        "" => "pdf",
        f => f,
    };
    if !matches!(formato, "pdf" | "pptx" | "html") {
        return Esito::errore("Formato non valido: pdf, pptx o html");
    }
    let out = match uscita(input, radici, "Presentazione", formato) {
        Ok(p) => p,
        Err(e) => return Esito::errore(e),
    };
    let tema = match str_arg(input, "tema") {
        "scuro" => ("default", "class: scuro"),
        "minimale" => ("uncover", ""),
        _ => ("default", ""),
    };
    // Lo stile di Oltremare sopra al tema scelto
    let stile = "style: |\n  section { font-family: -apple-system, 'Helvetica Neue', Arial, sans-serif; }\n  h1, h2 { color: #3b2fb8; }\n  section.scuro { background: radial-gradient(circle at 20% 0%, #2c2380, #120e33 70%); color: #ece9ff; }\n  section.scuro h1, section.scuro h2 { color: #c9c3ff; }\n  section.scuro a { color: #b3aaff; }\n  table { font-size: 0.8em; }";
    let md = format!("---\nmarp: true\ntheme: {}\npaginate: true\nsize: 16:9\n{}\n{stile}\n---\n\n{}", tema.0, tema.1, slide.trim().trim_start_matches("---").trim());
    let Some(bin) = marp() else {
        if formato == "pptx" {
            return Esito::errore("Per il PowerPoint serve Marp CLI (npm i -g @marp-team/marp-cli). In alternativa crea il PDF.");
        }
        // Ripiego senza Marp: una slide per pagina 16:9
        let sezioni: Vec<String> = slide.split("\n---").map(|s| format!("<section>{}</section>", markdown_html(s))).collect();
        let html = format!("<!doctype html><html><head><meta charset=\"utf-8\"><style>@page {{ size: 13.333in 7.5in; margin: 0 }} body {{ margin: 0; font-family: -apple-system, Arial }} section {{ width: 13.333in; height: 7.5in; box-sizing: border-box; padding: 0.7in 0.9in; page-break-after: always; font-size: 22pt }} h1, h2 {{ color: #3b2fb8 }}</style></head><body>{}</body></html>", sezioni.join(""));
        return match formato {
            "html" => match std::fs::write(&out, html) {
                Ok(()) => esito_file("Presentazione", &out, ""),
                Err(e) => Esito::errore(e.to_string()),
            },
            _ => match stampa_pdf(&html, (13.333, 7.5), false, false).await {
                Ok(b) => match std::fs::write(&out, b) {
                    Ok(()) => esito_file("Presentazione", &out, ""),
                    Err(e) => Esito::errore(e.to_string()),
                },
                Err(e) => Esito::errore(e),
            },
        };
    };
    let tmp = std::env::temp_dir().join(format!("oltremare-slide-{}.md", uuid::Uuid::new_v4().simple()));
    if let Err(e) = std::fs::write(&tmp, md) {
        return Esito::errore(e.to_string());
    }
    // L'uscita va su file: il Chrome usato da Marp lascia processi di servizio che terrebbero
    // aperto un canale letto da qui, e l'attesa non finirebbe mai
    let log = tmp.with_extension("log");
    let Ok(log_f) = std::fs::File::create(&log) else { return Esito::errore("Non riesco a creare il registro di Marp") };
    let mut cmd = tokio::process::Command::new(bin);
    cmd.arg(&tmp)
        .arg(format!("--{formato}"))
        .arg("--allow-local-files")
        .args(["--browser-timeout", "60"])
        .arg("-o")
        .arg(&out)
        .env("PATH", path_utente())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(log_f)
        .kill_on_drop(true);
    if let Some(c) = super::browser::trova_browser() {
        cmd.env("CHROME_PATH", c);
    }
    let r = tokio::time::timeout(Duration::from_secs(120), cmd.status()).await;
    let errori = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_file(&tmp);
    let _ = std::fs::remove_file(&log);
    match r {
        Ok(Ok(st)) if st.success() && out.exists() => {
            let n = slide.matches("\n---").count() + 1;
            esito_file("Presentazione", &out, &format!(" ({n} slide)"))
        }
        Ok(Ok(_)) => Esito::errore(format!("Marp non è riuscito: {}", errori.lines().filter(|l| !l.trim().is_empty()).last().unwrap_or_default())),
        Ok(Err(e)) => Esito::errore(e.to_string()),
        Err(_) => Esito::errore("La presentazione ha impiegato troppo tempo"),
    }
}

/// Immagini: Draw Things sul Mac (gratis, locale) oppure OpenAI se c'è la chiave.
async fn genera_immagine(input: &Value, radici: &[PathBuf]) -> Esito {
    let prompt = str_arg(input, "prompt").trim().to_string();
    if prompt.is_empty() {
        return Esito::errore("Descrivi l'immagine da generare");
    }
    let out = match uscita(input, radici, "Immagine", "png") {
        Ok(p) => p,
        Err(e) => return Esito::errore(e),
    };
    let w = input["larghezza"].as_u64().unwrap_or(1024).clamp(256, 2048) / 64 * 64;
    let h = input["altezza"].as_u64().unwrap_or(1024).clamp(256, 2048) / 64 * 64;
    let client = reqwest::Client::new();
    // 1. Draw Things (API compatibile con A1111 su localhost:7860)
    let dt = client
        .post("http://127.0.0.1:7860/sdapi/v1/txt2img")
        .timeout(Duration::from_secs(600))
        .json(&json!({ "prompt": prompt, "negative_prompt": str_arg(input, "negativo"), "width": w, "height": h, "steps": input["passi"].as_u64().unwrap_or(0), "seed": -1 }))
        .send()
        .await;
    let (b64, motore) = match dt {
        Ok(r) if r.status().is_success() => match r.json::<Value>().await.ok().and_then(|v| v["images"][0].as_str().map(str::to_string)) {
            Some(b) => (b, "Draw Things, sul Mac"),
            None => return Esito::errore("Draw Things non ha restituito un'immagine"),
        },
        _ => {
            // 2. OpenAI (a pagamento) se c'è la chiave
            let Ok(key) = crate::store::get_secret(&crate::store::key_name("openai")) else {
                return Esito::errore(
                    "Nessun generatore di immagini disponibile. Gratis e in locale: installa Draw Things dal Mac App Store, scarica un modello (per esempio FLUX.1 schnell) e attiva in Impostazioni → API Server (porta 7860). In alternativa salva una chiave OpenAI in Oltremare.",
                );
            };
            let size = if w > h { "1536x1024" } else if h > w { "1024x1536" } else { "1024x1024" };
            let r = client
                .post("https://api.openai.com/v1/images/generations")
                .bearer_auth(key)
                .timeout(Duration::from_secs(300))
                .json(&json!({ "model": "gpt-image-1", "prompt": prompt, "size": size, "n": 1 }))
                .send()
                .await;
            match r {
                Ok(r) if r.status().is_success() => match r.json::<Value>().await.ok().and_then(|v| v["data"][0]["b64_json"].as_str().map(str::to_string)) {
                    Some(b) => (b, "OpenAI (a pagamento)"),
                    None => return Esito::errore("OpenAI non ha restituito un'immagine"),
                },
                Ok(r) => return Esito::errore(format!("OpenAI ha risposto {}: {}", r.status(), r.text().await.unwrap_or_default().chars().take(200).collect::<String>())),
                Err(e) => return Esito::errore(format!("OpenAI non raggiungibile: {e}")),
            }
        }
    };
    let bytes = match base64::engine::general_purpose::STANDARD.decode(b64.trim()) {
        Ok(b) => b,
        Err(_) => return Esito::errore("Immagine non valida"),
    };
    if let Err(e) = std::fs::write(&out, &bytes) {
        return Esito::errore(e.to_string());
    }
    // Il modello la vede in piccolo, per controllarla
    let anteprima = image::load_from_memory(&bytes).ok().map(|i| {
        let p = i.thumbnail(768, 768);
        let mut buf = std::io::Cursor::new(Vec::new());
        let _ = p.write_to(&mut buf, image::ImageFormat::Png);
        base64::engine::general_purpose::STANDARD.encode(buf.into_inner())
    });
    let mut e = Esito::ok(
        format!("Immagine creata con {motore}: {}", mostra(&out)),
        json!({ "kind": "computer", "action": "image", "screenshotPath": out.to_string_lossy() }),
    );
    e.immagine = anteprima;
    e
}

fn ffmpeg() -> Option<PathBuf> {
    let sep = if cfg!(windows) { ';' } else { ':' };
    path_utente().split(sep).map(|d| Path::new(d).join(if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" })).find(|p| p.is_file())
}

/// Video da immagini (e audio facoltativo), con dissolvenze: per presentazioni, riassunti, social.
async fn crea_video(input: &Value, radici: &[PathBuf]) -> Esito {
    let Some(bin) = ffmpeg() else { return Esito::errore("Serve FFmpeg (brew install ffmpeg)") };
    let immagini: Vec<PathBuf> = match input["immagini"].as_array() {
        Some(a) if !a.is_empty() => match a.iter().filter_map(Value::as_str).map(|p| consentito(p, radici)).collect::<Result<Vec<_>, _>>() {
            Ok(v) => v,
            Err(e) => return Esito::errore(e),
        },
        _ => return Esito::errore("Indica le immagini del video, in ordine"),
    };
    if let Some(m) = immagini.iter().find(|p| !p.is_file()) {
        return Esito::errore(format!("Immagine non trovata: {}", mostra(m)));
    }
    let out = match uscita(input, radici, "Video", "mp4") {
        Ok(p) => p,
        Err(e) => return Esito::errore(e),
    };
    let secondi = input["secondi"].as_f64().unwrap_or(3.0).clamp(0.5, 60.0);
    let (w, h) = match str_arg(input, "formato") {
        "verticale" => (1080, 1920),
        "quadrato" => (1080, 1080),
        _ => (1920, 1080),
    };
    let dissolvenza = if input["dissolvenze"].as_bool().unwrap_or(true) && immagini.len() > 1 { 0.6_f64.min(secondi / 3.0) } else { 0.0 };
    let mut args: Vec<String> = vec!["-y".into()];
    for p in &immagini {
        args.extend(["-loop".into(), "1".into(), "-t".into(), format!("{:.2}", secondi + dissolvenza), "-i".into(), p.to_string_lossy().into_owned()]);
    }
    let audio = match str_arg(input, "audio") {
        "" => None,
        a => match consentito(a, radici) {
            Ok(p) if p.is_file() => Some(p),
            Ok(p) => return Esito::errore(format!("Audio non trovato: {}", mostra(&p))),
            Err(e) => return Esito::errore(e),
        },
    };
    if let Some(a) = &audio {
        args.extend(["-i".into(), a.to_string_lossy().into_owned()]);
    }
    // Ogni immagine si adatta al formato con bande, poi le dissolvenze in catena
    let mut filtro = String::new();
    for i in 0..immagini.len() {
        filtro.push_str(&format!("[{i}:v]scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2:color=black,setsar=1,fps=30,format=yuv420p[v{i}];"));
    }
    let mut ultimo = "v0".to_string();
    if dissolvenza > 0.0 {
        for i in 1..immagini.len() {
            let off = secondi * i as f64;
            let nuovo = format!("x{i}");
            filtro.push_str(&format!("[{ultimo}][v{i}]xfade=transition=fade:duration={dissolvenza:.2}:offset={off:.2}[{nuovo}];"));
            ultimo = nuovo;
        }
    } else if immagini.len() > 1 {
        let ingressi: String = (0..immagini.len()).map(|i| format!("[v{i}]")).collect();
        filtro.push_str(&format!("{ingressi}concat=n={}:v=1:a=0[cat];", immagini.len()));
        ultimo = "cat".into();
    }
    let filtro = filtro.trim_end_matches(';').to_string();
    args.extend(["-filter_complex".into(), filtro, "-map".into(), format!("[{ultimo}]")]);
    if audio.is_some() {
        args.extend(["-map".into(), format!("{}:a", immagini.len()), "-c:a".into(), "aac".into(), "-shortest".into()]);
    }
    args.extend(["-c:v".into(), "libx264".into(), "-preset".into(), "medium".into(), "-crf".into(), "20".into(), "-movflags".into(), "+faststart".into(), out.to_string_lossy().into_owned()]);
    let mut cmd = tokio::process::Command::new(bin);
    cmd.args(&args).env("PATH", path_utente()).kill_on_drop(true);
    match tokio::time::timeout(Duration::from_secs(900), cmd.output()).await {
        Ok(Ok(o)) if o.status.success() => {
            let durata = secondi * immagini.len() as f64;
            esito_file("Video", &out, &format!(" ({} immagini, circa {:.0} s, {w}×{h})", immagini.len(), durata))
        }
        Ok(Ok(o)) => Esito::errore(format!("FFmpeg non è riuscito: {}", String::from_utf8_lossy(&o.stderr).lines().rev().take(3).collect::<Vec<_>>().join(" | "))),
        Ok(Err(e)) => Esito::errore(e.to_string()),
        Err(_) => Esito::errore("Il video ha impiegato troppo tempo"),
    }
}

pub async fn esegui(nome: &str, input: &Value, radici: &[PathBuf]) -> Esito {
    match nome {
        "crea_pdf" => crea_pdf(input, radici).await,
        "crea_presentazione" => crea_presentazione(input, radici).await,
        "genera_immagine" => genera_immagine(input, radici).await,
        "crea_video" => crea_video(input, radici).await,
        _ => Esito::errore(format!("Strumento sconosciuto: {nome}")),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn markdown_con_tabelle() {
        let h = super::markdown_html("| a | b |\n|---|---|\n| 1 | 2 |\n\n**x**");
        assert!(h.contains("<table>") && h.contains("<strong>x</strong>"), "{h}");
    }

    /// Prove vere: PDF con Chrome, presentazione con Marp, video con FFmpeg (nella cartella temporanea).
    #[tokio::test]
    #[ignore]
    async fn crea_davvero() {
        let dir = std::env::temp_dir().join("oltremare-media-prova");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let radici = vec![dir.canonicalize().unwrap()];
        let d = radici[0].to_string_lossy().to_string();
        let pdf = super::crea_pdf(&serde_json::json!({ "uscita": format!("{d}/doc.pdf"), "contenuto": "# Preventivo\n\n| Voce | Prezzo |\n|---|---|\n| Vetrata | € 229 |\n\nTesto **importante**.", "titolo": "Prova" }), &radici).await;
        assert!(!pdf.is_error, "{}", pdf.testo);
        let pres = super::crea_presentazione(&serde_json::json!({ "uscita": format!("{d}/slide.pptx"), "formato": "pptx", "slide": "# Oltremare\nAssistente personale\n\n---\n\n## Cosa fa\n- Agenti\n- Voce" }), &radici).await;
        assert!(!pres.is_error, "{}", pres.testo);
        let pres_pdf = super::crea_presentazione(&serde_json::json!({ "uscita": format!("{d}/slide.pdf"), "tema": "scuro", "slide": "# Titolo\n\n---\n\n## Seconda" }), &radici).await;
        assert!(!pres_pdf.is_error, "{}", pres_pdf.testo);
        // Due immagini semplici per il video
        for (i, c) in [(0u8, [40u8, 30, 160]), (1, [220, 200, 255])] {
            image::RgbImage::from_pixel(640, 360, image::Rgb(c)).save(dir.join(format!("{i}.png"))).unwrap();
        }
        let v = super::crea_video(&serde_json::json!({ "uscita": format!("{d}/video.mp4"), "immagini": [format!("{d}/0.png"), format!("{d}/1.png")], "secondi": 1.5 }), &radici).await;
        assert!(!v.is_error, "{}", v.testo);
        for f in ["doc.pdf", "slide.pptx", "slide.pdf", "video.mp4"] {
            let m = std::fs::metadata(dir.join(f)).unwrap().len();
            println!("{f}: {m} byte");
            assert!(m > 1000);
        }
    }
}

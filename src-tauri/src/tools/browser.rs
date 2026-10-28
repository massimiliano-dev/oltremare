//! Browser: Chrome o Edge installati, guidati con il protocollo DevTools, su un profilo dedicato
//! (separato da quello dell'utente). L'agente legge la pagina come elenco di elementi numerati
//! e agisce indicando il numero. Le password non si leggono e non si scrivono.

use super::{str_arg, Esito};
use base64::Engine;
use chromiumoxide::page::ScreenshotParams;
use chromiumoxide::{Browser, BrowserConfig, Page};
use futures::StreamExt;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

const SNAPSHOT: &str = r#"(() => {
  const vis = el => { const r = el.getBoundingClientRect(); const s = getComputedStyle(el); return r.width > 1 && r.height > 1 && s.visibility !== 'hidden' && s.display !== 'none'; };
  document.querySelectorAll('[data-olt]').forEach(e => e.removeAttribute('data-olt'));
  const sel = 'a[href],button,input:not([type=hidden]),select,textarea,[role=button],[role=link],[role=checkbox],[role=tab],[role=menuitem],[role=option],[contenteditable=true],summary';
  const out = []; let n = 0;
  const label = el => (el.getAttribute('aria-label') || (el.labels && el.labels[0] && el.labels[0].innerText) || el.placeholder || el.title || el.alt || (el.tagName === 'SELECT' ? '' : el.innerText) || (el.type !== 'password' ? el.value : '') || el.name || '').trim().replace(/\s+/g, ' ').slice(0, 80);
  for (const el of document.querySelectorAll('h1,h2,h3,' + sel)) {
    if (!vis(el)) continue;
    const tag = el.tagName.toLowerCase();
    if (/^h[1-3]$/.test(tag)) { const t = el.innerText.trim().replace(/\s+/g, ' ').slice(0, 100); if (t) out.push('# ' + t); continue; }
    n++; el.setAttribute('data-olt', String(n));
    const role = el.getAttribute('role');
    const tipi = { checkbox: 'casella', radio: 'opzione', submit: 'pulsante', button: 'pulsante', search: 'ricerca', email: 'email', password: 'password', number: 'numero', date: 'data', tel: 'telefono', url: 'indirizzo' };
    const tipo = tag === 'a' ? 'link' : (tag === 'button' || role === 'button') ? 'pulsante' : tag === 'select' ? 'menu' : tag === 'textarea' ? 'testo' : tag === 'input' ? (tipi[el.type] || 'campo') : (role || tag);
    let extra = '';
    if (tag === 'input' || tag === 'textarea') extra = el.type === 'password' ? (el.value ? ' (compilato)' : ' (vuoto)') : (el.value ? ' = «' + el.value.slice(0, 40) + '»' : ' (vuoto)');
    if (el.type === 'checkbox' || el.type === 'radio') extra = el.checked ? ' [x]' : ' [ ]';
    if (tag === 'select') extra = ' = «' + ((el.selectedOptions[0] && el.selectedOptions[0].text) || '') + '»';
    if (tag === 'a') { const h = el.getAttribute('href'); if (h && !h.startsWith('javascript')) extra += ' → ' + h.slice(0, 80); }
    if (el.disabled) extra += ' (disattivato)';
    out.push('[' + n + '] ' + tipo + ' «' + label(el) + '»' + extra);
    if (out.length > 450) { out.push('… (pagina lunga: scorri o cerca con web_trova)'); break; }
  }
  return { url: location.href, title: document.title, lines: out };
})()"#;

pub struct Sessione {
    inner: tokio::sync::Mutex<Option<(Browser, Page)>>,
}

pub fn sessione() -> &'static Sessione {
    static S: std::sync::OnceLock<Sessione> = std::sync::OnceLock::new();
    S.get_or_init(|| Sessione { inner: tokio::sync::Mutex::new(None) })
}

/// Chrome, Edge, Brave o Chromium installati.
pub fn trova_browser() -> Option<PathBuf> {
    let candidati: Vec<PathBuf> = if cfg!(target_os = "macos") {
        ["Google Chrome", "Microsoft Edge", "Brave Browser", "Chromium"]
            .iter()
            .map(|a| PathBuf::from(format!("/Applications/{a}.app/Contents/MacOS/{a}")))
            .chain(crate::shell_env::home().map(|h| h.join("Applications/Google Chrome.app/Contents/MacOS/Google Chrome")))
            .collect()
    } else if cfg!(windows) {
        let pf = std::env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".into());
        let pf86 = std::env::var("ProgramFiles(x86)").unwrap_or_else(|_| "C:\\Program Files (x86)".into());
        let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
        vec![
            PathBuf::from(&pf).join("Google\\Chrome\\Application\\chrome.exe"),
            PathBuf::from(&pf86).join("Google\\Chrome\\Application\\chrome.exe"),
            PathBuf::from(&local).join("Google\\Chrome\\Application\\chrome.exe"),
            PathBuf::from(&pf86).join("Microsoft\\Edge\\Application\\msedge.exe"),
            PathBuf::from(&pf).join("Microsoft\\Edge\\Application\\msedge.exe"),
        ]
    } else {
        ["/usr/bin/google-chrome", "/usr/bin/chromium", "/usr/bin/chromium-browser"].iter().map(PathBuf::from).collect()
    };
    candidati.into_iter().find(|p| p.is_file())
}

fn dominio(url: &str) -> String {
    url.split("://").nth(1).unwrap_or(url).split(['/', '?', '#', ':']).next().unwrap_or("").to_lowercase()
}

/// Dominio consentito: elenco vuoto = tutti; altrimenti il dominio o un suo sottodominio.
pub fn consentito(url: &str, domini: &[String]) -> bool {
    if domini.is_empty() {
        return true;
    }
    let d = dominio(url);
    domini.iter().map(|x| x.trim().trim_start_matches("*.").to_lowercase()).any(|x| d == x || d.ends_with(&format!(".{x}")))
}

impl Sessione {
    async fn pagina(&self, profilo: &Path) -> Result<Page, String> {
        let mut g = self.inner.lock().await;
        if let Some((_, p)) = g.as_ref() {
            // Il browser può essere stato chiuso dall'utente
            if p.url().await.is_ok() {
                return Ok(p.clone());
            }
            *g = None;
        }
        let exe = trova_browser().ok_or("Non trovo Chrome né Edge: installane uno per usare il browser")?;
        std::fs::create_dir_all(profilo).map_err(|e| e.to_string())?;
        let cfg = BrowserConfig::builder()
            .chrome_executable(exe)
            .user_data_dir(profilo)
            .with_head()
            .window_size(1280, 900)
            .viewport(None)
            .args(["--no-first-run", "--no-default-browser-check", "--lang=it-IT"])
            .build()
            .map_err(|e| format!("Configurazione del browser non valida: {e}"))?;
        let (browser, mut handler) = Browser::launch(cfg).await.map_err(|e| format!("Il browser non si avvia: {e}"))?;
        tauri::async_runtime::spawn(async move { while let Some(h) = handler.next().await { if h.is_err() { break; } } });
        let page = match browser.pages().await.ok().and_then(|p| p.into_iter().next()) {
            Some(p) => p,
            None => browser.new_page("about:blank").await.map_err(|e| e.to_string())?,
        };
        *g = Some((browser, page.clone()));
        Ok(page)
    }

    pub async fn chiudi(&self) {
        if let Some((mut b, _)) = self.inner.lock().await.take() {
            let _ = b.close().await;
        }
    }

    pub async fn attivo(&self) -> bool {
        self.inner.lock().await.is_some()
    }
}

async fn js(p: &Page, codice: &str) -> Result<Value, String> {
    // Uno script che non restituisce nulla (undefined) vale null
    Ok(p.evaluate(codice).await.map_err(|e| format!("Errore nella pagina: {e}"))?.value().cloned().unwrap_or(Value::Null))
}

async fn elemento(p: &Page, rif: i64) -> Result<chromiumoxide::Element, String> {
    p.find_element(format!("[data-olt=\"{rif}\"]")).await.map_err(|_| format!("Elemento [{rif}] non trovato: la pagina è cambiata, rifai web_snapshot"))
}

async fn snapshot(p: &Page) -> Result<Esito, String> {
    let v = js(p, SNAPSHOT).await?;
    let righe: Vec<String> = v["lines"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_string)).collect();
    let mut t = format!("{}\n{}\n\n", v["title"].as_str().unwrap_or_default(), v["url"].as_str().unwrap_or_default());
    t.push_str(&righe.join("\n"));
    if t.len() > 30_000 {
        t.truncate(t.char_indices().take_while(|(i, _)| *i < 30_000).last().map_or(0, |(i, _)| i));
        t.push_str("\n…");
    }
    Ok(Esito::ok(t, json!({ "kind": "browser", "url": v["url"], "title": v["title"], "text": righe.iter().take(30).cloned().collect::<Vec<_>>().join("\n") })))
}

async fn attendi_pagina(p: &Page) {
    // Un attimo perché la pagina reagisca, poi aspetta che finisca di caricare (al massimo 8 s)
    tokio::time::sleep(Duration::from_millis(500)).await;
    for _ in 0..16 {
        if js(p, "document.readyState").await.ok().and_then(|v| v.as_str().map(|s| s == "complete")).unwrap_or(true) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

pub async fn esegui(nome: &str, input: &Value, profilo: &Path, domini: &[String], cartella: Option<PathBuf>) -> Esito {
    match esegui_r(nome, input, profilo, domini, cartella).await {
        Ok(e) => e,
        Err(e) => Esito::errore(e),
    }
}

async fn esegui_r(nome: &str, input: &Value, profilo: &Path, domini: &[String], cartella: Option<PathBuf>) -> Result<Esito, String> {
    let p = sessione().pagina(profilo).await?;
    let corrente = p.url().await.ok().flatten().unwrap_or_default();
    let modifica = !matches!(nome, "web_snapshot" | "web_testo" | "web_screenshot" | "web_trova" | "web_apri");
    if modifica && corrente.starts_with("http") && !consentito(&corrente, domini) {
        return Err(format!("Questo agente non può agire su {}", dominio(&corrente)));
    }
    let rif = input["ref"].as_i64().unwrap_or(-1);
    match nome {
        "web_apri" => {
            let url = str_arg(input, "url").trim();
            let url = if url.contains("://") { url.to_string() } else { format!("https://{url}") };
            if !(url.starts_with("https://") || url.starts_with("http://")) {
                return Err("Apro solo indirizzi http(s)".into());
            }
            if !consentito(&url, domini) {
                return Err(format!("Questo agente non può aprire {} (domini consentiti: {})", dominio(&url), domini.join(", ")));
            }
            p.goto(url.as_str()).await.map_err(|e| format!("Pagina non raggiungibile: {e}"))?;
            attendi_pagina(&p).await;
            let _ = p.bring_to_front().await;
            snapshot(&p).await
        }
        "web_snapshot" => snapshot(&p).await,
        "web_testo" => {
            let v = js(&p, "document.body ? document.body.innerText : ''").await?;
            let t: String = v.as_str().unwrap_or_default().chars().take(40_000).collect();
            Ok(Esito::ok(format!("{corrente}\n\n{t}"), json!({ "kind": "browser", "url": corrente, "text": t.chars().take(600).collect::<String>() })))
        }
        "web_trova" => {
            let q = str_arg(input, "testo").to_lowercase();
            let v = js(&p, "document.body ? document.body.innerText : ''").await?;
            let righe: Vec<&str> = v.as_str().unwrap_or_default().lines().filter(|l| l.to_lowercase().contains(&q)).take(40).collect();
            let t = if righe.is_empty() { format!("«{q}» non compare nella pagina") } else { righe.join("\n") };
            Ok(Esito::ok(t.clone(), json!({ "kind": "browser", "url": corrente, "text": t })))
        }
        "web_screenshot" => {
            let png = p.screenshot(ScreenshotParams::builder().build()).await.map_err(|e| e.to_string())?;
            let img = image::load_from_memory(&png).map_err(|e| e.to_string())?;
            let k = (1280.0 / img.width().max(img.height()) as f64).min(1.0);
            let img = img.resize((img.width() as f64 * k) as u32, (img.height() as f64 * k) as u32, image::imageops::FilterType::Triangle);
            let mut buf = std::io::Cursor::new(Vec::new());
            img.write_to(&mut buf, image::ImageFormat::Png).map_err(|e| e.to_string())?;
            let bytes = buf.into_inner();
            let path = cartella.and_then(|c| {
                std::fs::create_dir_all(&c).ok()?;
                let f = c.join(format!("web-{}.png", uuid::Uuid::new_v4().simple()));
                std::fs::write(&f, &bytes).ok().map(|_| f)
            });
            Ok(Esito {
                testo: format!("Screenshot della pagina {corrente}"),
                is_error: false,
                risultato: json!({ "kind": "browser", "url": corrente, "screenshotPath": path.map(|p| p.to_string_lossy().into_owned()) }),
                immagine: Some(base64::engine::general_purpose::STANDARD.encode(&bytes)),
            })
        }
        "web_clicca" => {
            elemento(&p, rif).await?.click().await.map_err(|e| format!("Clic non riuscito: {e}"))?;
            attendi_pagina(&p).await;
            snapshot(&p).await.map(|mut e| {
                e.testo = format!("Cliccato [{rif}]. Pagina attuale:\n{}", e.testo);
                e
            })
        }
        "web_compila" => {
            let t = str_arg(input, "testo");
            let tipo = js(&p, &format!("(() => {{ const e = document.querySelector('[data-olt=\"{rif}\"]'); return e ? (e.type || e.tagName) : null; }})()")).await?;
            if tipo.as_str() == Some("password") {
                return Err("Non inserisco password: chiedi all'utente di scriverla lui".into());
            }
            let el = elemento(&p, rif).await?;
            el.click().await.map_err(|e| e.to_string())?;
            js(&p, &format!("(() => {{ const e = document.querySelector('[data-olt=\"{rif}\"]'); if (e && 'value' in e) {{ e.value = ''; e.dispatchEvent(new Event('input', {{ bubbles: true }})); }} }})()")).await?;
            el.type_str(t).await.map_err(|e| format!("Scrittura non riuscita: {e}"))?;
            Ok(Esito::ok(format!("Scritto nel campo [{rif}]"), json!({ "kind": "browser", "url": corrente, "text": format!("[{rif}] = «{}»", t.chars().take(60).collect::<String>()) })))
        }
        "web_seleziona" => {
            let val = str_arg(input, "valore").replace('\\', "\\\\").replace('\'', "\\'");
            let r = js(
                &p,
                &format!(
                    "(() => {{ const e = document.querySelector('[data-olt=\"{rif}\"]'); if (!e || !e.options) return 'NO'; \
                     const o = [...e.options].find(o => o.value === '{val}' || o.text.trim().toLowerCase() === '{val}'.toLowerCase()); \
                     if (!o) return 'NOOPT'; e.value = o.value; e.dispatchEvent(new Event('change', {{ bubbles: true }})); return o.text; }})()"
                ),
            )
            .await?;
            match r.as_str() {
                Some("NO") => Err(format!("[{rif}] non è un menu")),
                Some("NOOPT") => Err("Opzione non trovata nel menu".into()),
                Some(t) => Ok(Esito::ok(format!("Selezionato «{t}»"), json!({ "kind": "browser", "url": corrente, "text": format!("[{rif}] = «{t}»") }))),
                None => Err("Selezione non riuscita".into()),
            }
        }
        "web_invia_form" => {
            let r = js(
                &p,
                &format!("(() => {{ const e = document.querySelector('[data-olt=\"{rif}\"]'); if (!e) return 'NO'; const f = e.form || e.closest('form'); if (f) {{ f.requestSubmit ? f.requestSubmit() : f.submit(); return 'FORM'; }} e.click(); return 'CLICK'; }})()"),
            )
            .await?;
            if r.as_str() == Some("NO") {
                return Err(format!("Elemento [{rif}] non trovato: rifai web_snapshot"));
            }
            attendi_pagina(&p).await;
            snapshot(&p).await.map(|mut e| {
                e.testo = format!("Modulo inviato. Pagina attuale:\n{}", e.testo);
                e
            })
        }
        "web_indietro" => {
            js(&p, "history.back()").await?;
            attendi_pagina(&p).await;
            snapshot(&p).await
        }
        _ => Err(format!("Azione sconosciuta: {nome}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_allowlist() {
        let d = vec!["comune.milano.it".to_string(), "*.esempio.com".to_string()];
        assert!(consentito("https://www.comune.milano.it/servizi", &d));
        assert!(consentito("https://shop.esempio.com", &d));
        assert!(!consentito("https://comune.milano.it.truffa.net", &d));
        assert!(!consentito("https://altro.it", &d));
        assert!(consentito("https://qualsiasi.it", &[]));
    }

    /// Prova vera con Chrome: `cargo test browser_vero -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn browser_vero() {
        let profilo = std::env::temp_dir().join(format!("oltremare-browser-{}", uuid::Uuid::new_v4()));
        let pagina = std::env::temp_dir().join("oltremare-form.html");
        std::fs::write(&pagina, r#"<html><head><title>Prova</title></head><body><h1>Iscrizione</h1><form onsubmit="document.title='Inviato: '+document.getElementById('n').value; return false"><label>Nome <input id="n" name="nome"></label><label>Password <input type="password" id="p"></label><select id="s"><option>Uno</option><option>Due</option></select><button type="submit">Invia</button></form></body></html>"#).unwrap();
        let url = format!("file://{}", pagina.display());
        let p = sessione().pagina(&profilo).await.unwrap();
        p.goto(url.as_str()).await.unwrap();
        let s = snapshot(&p).await.unwrap();
        println!("{}", s.testo);
        assert!(s.testo.contains("# Iscrizione") && s.testo.contains("[1] campo «Nome"));
        let e = esegui("web_compila", &json!({"ref": 1, "testo": "Mario"}), &profilo, &[], None).await;
        assert!(!e.is_error, "{}", e.testo);
        let pw = esegui("web_compila", &json!({"ref": 2, "testo": "segreto"}), &profilo, &[], None).await;
        assert!(pw.is_error && pw.testo.contains("password"));
        let sel = esegui("web_seleziona", &json!({"ref": 3, "valore": "due"}), &profilo, &[], None).await;
        assert!(!sel.is_error, "{}", sel.testo);
        let inv = esegui("web_invia_form", &json!({"ref": 4}), &profilo, &[], None).await;
        assert!(inv.testo.contains("Inviato: Mario"), "{}", inv.testo);
        sessione().chiudi().await;
    }
}

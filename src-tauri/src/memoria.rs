//! Memoria a lungo termine: note Markdown collegate (compatibili con Obsidian) in una cartella dell'utente.
//! Gli agenti cercano, leggono e scrivono note; il briefing e i lavori dello Studio finiscono nel diario e
//! nelle note dei progetti. Non si cancella mai nulla: una nota riscritta lascia la versione vecchia in `.trash`.

use crate::store::Settings;
use serde_json::{json, Value};
use std::path::{Component, Path, PathBuf};

pub const CARTELLE: &[&str] = &["Persone", "Progetti", "Argomenti", "Diario"];
const SALTA: &[&str] = &[".obsidian", ".trash", ".git", "node_modules"];

/// Cartella della memoria, se attiva.
pub fn cartella(s: &Settings) -> Option<PathBuf> {
    if !s.memory_enabled || s.memory_path.trim().is_empty() {
        return None;
    }
    let p = crate::tools::percorsi::espandi(&s.memory_path);
    if !p.is_dir() {
        return None;
    }
    if !p.join("Oltremare.md").exists() {
        prepara(&p);
    }
    Some(p)
}

/// Una cartella «Obsidian» sulla Scrivania o in Documenti (o il vault iCloud di Obsidian), se c'è.
pub fn trova_vault() -> Option<PathBuf> {
    let home = crate::shell_env::home()?;
    [
        home.join("Desktop/Obsidian"),
        home.join("Documents/Obsidian"),
        home.join("Library/Mobile Documents/iCloud~md~obsidian/Documents"),
    ]
    .into_iter()
    .find(|p| p.is_dir())
}

fn oggi() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// Prepara le cartelle e le note di partenza (solo quelle che mancano).
pub fn prepara(dir: &Path) {
    for c in CARTELLE {
        let _ = std::fs::create_dir_all(dir.join(c));
    }
    let casa = dir.join("Oltremare.md");
    if !casa.exists() {
        let _ = std::fs::write(
            &casa,
            format!(
                "---\ncreato: {}\nfonte: Oltremare\n---\n# Oltremare\n\nLa memoria di Oltremare: note scritte dagli agenti e da te, collegate tra loro.\n\n\
- [[Preferenze]]: come ti piace che si lavori\n- Persone: chi è chi, ruoli, contatti di lavoro\n- Progetti: stato, decisioni, diario dei lavori dello Studio\n\
- Argomenti: procedure, appunti, cose da ricordare\n- Diario: una nota al giorno con il briefing e le cose fatte\n\n\
Puoi modificare o cancellare qualsiasi nota: gli agenti leggono quello che trovano.\n",
                oggi()
            ),
        );
    }
    let pref = dir.join("Preferenze.md");
    if !pref.exists() {
        let _ = std::fs::write(&pref, format!("---\ncreato: {}\nfonte: Oltremare\n---\n# Preferenze\n\nCome l'utente preferisce che si lavori (lingua, tono, strumenti, abitudini).\n\n", oggi()));
    }
}

/// Tutte le note (percorso relativo, percorso), escluse le cartelle di servizio.
fn note(dir: &Path) -> Vec<(String, PathBuf)> {
    let mut out = vec![];
    let mut pila = vec![dir.to_path_buf()];
    while let Some(d) = pila.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            let nome = e.file_name().to_string_lossy().into_owned();
            if p.is_dir() {
                if !SALTA.contains(&nome.as_str()) && !nome.starts_with('.') {
                    pila.push(p);
                }
            } else if nome.ends_with(".md") {
                let rel = p.strip_prefix(dir).unwrap_or(&p).to_string_lossy().replace('\\', "/");
                out.push((rel, p));
            }
        }
        if out.len() > 5000 {
            break;
        }
    }
    out.sort();
    out
}

fn titolo_di(rel: &str) -> String {
    rel.rsplit('/').next().unwrap_or(rel).trim_end_matches(".md").to_string()
}

/// Trova una nota per titolo («Mario Rossi», anche «[[Mario Rossi]]») o per percorso («Persone/Mario Rossi»).
fn risolvi(dir: &Path, nota: &str) -> Option<PathBuf> {
    let n = nota.trim().trim_start_matches("[[").trim_end_matches("]]").split('|').next().unwrap_or("").trim().trim_end_matches(".md").to_string();
    if n.is_empty() {
        return None;
    }
    if n.contains('/') {
        let p = dir.join(format!("{n}.md"));
        return dentro(dir, &p).then_some(p).filter(|p| p.is_file());
    }
    let basso = n.to_lowercase();
    note(dir).into_iter().find(|(rel, _)| titolo_di(rel).to_lowercase() == basso).map(|(_, p)| p)
}

/// Il percorso resta nella cartella della memoria (niente «..» né percorsi assoluti).
fn dentro(dir: &Path, p: &Path) -> bool {
    p.starts_with(dir) && !p.strip_prefix(dir).map(|r| r.components().any(|c| !matches!(c, Component::Normal(_)))).unwrap_or(true)
}

/// Nome di file valido per una nota.
fn nome_file(t: &str) -> String {
    t.chars().map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '#' | '^' | '[' | ']') { '-' } else { c }).collect::<String>().trim().to_string()
}

fn collegamenti(testo: &str) -> Vec<String> {
    let re = regex::Regex::new(r"\[\[([^\]|#]+)").expect("regex");
    let mut v: Vec<String> = re.captures_iter(testo).map(|c| c[1].trim().to_string()).collect();
    v.dedup();
    v
}

/// Testi che non devono finire in memoria (chiavi, password, carte).
fn sensibile(t: &str) -> bool {
    let re = regex::Regex::new(r"(?i)(sk-[a-z0-9_-]{16,}|ghp_[a-z0-9]{20,}|xox[bp]-[a-z0-9-]{10,}|-----BEGIN [A-Z ]*PRIVATE KEY|\b(password|passwd|pwd)\s*[:=]\s*\S{4,}|\b(?:\d[ -]?){15,16}\b)").expect("regex");
    re.is_match(t)
}

pub fn cerca(dir: &Path, query: &str, max: usize) -> Value {
    let termini: Vec<String> = query.to_lowercase().split_whitespace().filter(|t| t.chars().count() > 1).map(str::to_string).collect();
    let mut trovate: Vec<(i64, String, String)> = vec![];
    for (rel, p) in note(dir) {
        let Ok(testo) = std::fs::read_to_string(&p) else { continue };
        let basso = testo.to_lowercase();
        let tit = titolo_di(&rel).to_lowercase();
        let mut punti = 0i64;
        for t in &termini {
            if tit.contains(t.as_str()) {
                punti += 10;
            }
            punti += basso.matches(t.as_str()).count().min(10) as i64;
        }
        if punti == 0 && !termini.is_empty() {
            continue;
        }
        // Estratto: la riga col primo termine trovato
        let estratto = testo
            .lines()
            .find(|l| termini.iter().any(|t| l.to_lowercase().contains(t.as_str())) && !l.starts_with("---"))
            .or_else(|| testo.lines().find(|l| !l.trim().is_empty() && !l.starts_with("---") && !l.contains(':')))
            .unwrap_or("")
            .chars()
            .take(220)
            .collect::<String>();
        trovate.push((punti, rel, estratto));
    }
    trovate.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    trovate.truncate(max.clamp(1, 30));
    json!(trovate.into_iter().map(|(_, rel, e)| json!({ "nota": titolo_di(&rel), "percorso": rel, "estratto": e })).collect::<Vec<_>>())
}

pub fn leggi(dir: &Path, nota: &str) -> Result<Value, String> {
    let p = risolvi(dir, nota).ok_or_else(|| format!("Nessuna nota «{nota}» in memoria: cercala con memoria_cerca o creala con memoria_scrivi"))?;
    let testo = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
    let rel = p.strip_prefix(dir).unwrap_or(&p).to_string_lossy().replace('\\', "/");
    let tit = titolo_di(&rel);
    // Note che citano questa
    let cerca_link = format!("[[{}", tit.to_lowercase());
    let citata_da: Vec<String> = note(dir)
        .into_iter()
        .filter(|(r, _)| r != &rel)
        .filter(|(_, q)| std::fs::read_to_string(q).is_ok_and(|t| t.to_lowercase().contains(&cerca_link)))
        .map(|(r, _)| titolo_di(&r))
        .take(30)
        .collect();
    Ok(json!({ "nota": tit, "percorso": rel, "testo": testo.chars().take(30_000).collect::<String>(), "collegamenti": collegamenti(&testo), "citata_da": citata_da }))
}

/// Aggiorna (o aggiunge) la data «aggiornato» nelle proprietà della nota.
fn segna_aggiornato(testo: &str) -> String {
    let d = oggi();
    if let Some(resto) = testo.strip_prefix("---\n") {
        if let Some(fine) = resto.find("\n---") {
            let (front, dopo) = resto.split_at(fine);
            let righe: Vec<String> = front.lines().filter(|l| !l.starts_with("aggiornato:")).map(str::to_string).chain([format!("aggiornato: {d}")]).collect();
            return format!("---\n{}{}", righe.join("\n"), dopo);
        }
    }
    testo.to_string()
}

/// Scrive in memoria. `modo`: "crea" (nuova nota), "aggiungi" (in fondo, crea se manca), "sostituisci" (riscrive, la vecchia va in .trash).
pub fn scrivi(dir: &Path, nota: &str, contenuto: &str, modo: &str, cartella: &str) -> Result<Value, String> {
    if sensibile(contenuto) {
        return Err("Il testo sembra contenere password, chiavi o numeri di carta: in memoria non si salvano. Togli la parte sensibile e riprova.".into());
    }
    let titolo = nota.trim().trim_start_matches("[[").trim_end_matches("]]").trim_end_matches(".md").rsplit('/').next().unwrap_or("").trim().to_string();
    if titolo.is_empty() {
        return Err("Serve il titolo della nota".into());
    }
    let esistente = risolvi(dir, nota);
    let p = match &esistente {
        Some(p) => p.clone(),
        None => {
            let sotto = if nota.contains('/') {
                nota.rsplit_once('/').map(|(c, _)| c.to_string()).unwrap_or_default()
            } else if CARTELLE.contains(&cartella) {
                cartella.to_string()
            } else {
                "Argomenti".into()
            };
            let p = dir.join(&sotto).join(format!("{}.md", nome_file(&titolo)));
            if !dentro(dir, &p) {
                return Err("Percorso della nota non valido".into());
            }
            p
        }
    };
    let rel = p.strip_prefix(dir).unwrap_or(&p).to_string_lossy().replace('\\', "/");
    let corpo = contenuto.trim_end();
    let nuovo = |c: &str| {
        let c = c.trim_start();
        let intestazione = if c.starts_with("# ") { String::new() } else { format!("# {titolo}\n\n") };
        format!("---\ncreato: {}\naggiornato: {}\nfonte: Oltremare\n---\n{intestazione}{c}\n", oggi(), oggi())
    };
    let (testo, azione) = match (modo, &esistente) {
        ("crea", Some(_)) => return Err(format!("La nota «{titolo}» esiste già ({rel}): usa modo «aggiungi» o «sostituisci»")),
        (_, None) => (nuovo(corpo), "creata"),
        ("sostituisci", Some(_)) => {
            let vecchio = std::fs::read_to_string(&p).unwrap_or_default();
            let cestino = dir.join(".trash");
            let _ = std::fs::create_dir_all(&cestino);
            let _ = std::fs::write(cestino.join(format!("{} {}.md", nome_file(&titolo), chrono::Local::now().format("%Y-%m-%d %H%M%S"))), &vecchio);
            // Le proprietà della nota (creato…) restano
            let front = vecchio.strip_prefix("---\n").and_then(|r| r.find("\n---\n").map(|i| format!("---\n{}\n---\n", &r[..i])));
            match front {
                Some(f) => (segna_aggiornato(&format!("{f}{}\n", corpo.trim_start())), "riscritta (la versione precedente è in .trash)"),
                None => (nuovo(corpo), "riscritta (la versione precedente è in .trash)"),
            }
        }
        _ => {
            let vecchio = std::fs::read_to_string(&p).unwrap_or_default();
            (segna_aggiornato(&format!("{}\n\n{corpo}\n", vecchio.trim_end())), "aggiornata")
        }
    };
    if let Some(c) = p.parent() {
        std::fs::create_dir_all(c).map_err(|e| e.to_string())?;
    }
    let tmp = p.with_extension("md.tmp");
    std::fs::write(&tmp, &testo).and_then(|_| std::fs::rename(&tmp, &p)).map_err(|e| format!("Non riesco a scrivere la nota: {e}"))?;
    Ok(json!({ "nota": titolo, "percorso": rel, "azione": azione }))
}

/// Aggiunge una voce alla nota del giorno (Diario/AAAA-MM-GG).
pub fn diario(dir: &Path, titolo: &str, testo: &str) -> Result<(), String> {
    let ora = chrono::Local::now().format("%H:%M");
    scrivi(dir, &format!("Diario/{}", oggi()), &format!("## {ora} · {titolo}\n\n{}", testo.trim()), "aggiungi", "Diario").map(|_| ())
}

/// Dove sta la nota-cardine del progetto: `Progetti/<nome>/<nome>.md`. Le note vecchie, piatte
/// (`Progetti/<nome>.md`), vengono spostate nella cartella: i collegamenti [[nome]] restano validi.
fn percorso_progetto(dir: &Path, titolo: &str) -> PathBuf {
    let n = nome_file(titolo);
    let nuovo = dir.join("Progetti").join(&n).join(format!("{n}.md"));
    let vecchio = dir.join("Progetti").join(format!("{n}.md"));
    if !nuovo.exists() && vecchio.is_file() {
        let _ = std::fs::create_dir_all(nuovo.parent().unwrap_or(dir));
        let _ = std::fs::rename(&vecchio, &nuovo);
    }
    nuovo
}

/// Sposta tutte le note piatte dei progetti nelle loro cartelle (una volta, all'avvio della memoria).
pub fn migra_progetti(dir: &Path) -> usize {
    let mut n = 0;
    if let Ok(rd) = std::fs::read_dir(dir.join("Progetti")) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_file() && p.extension().is_some_and(|x| x == "md") {
                let titolo = p.file_stem().map(|x| x.to_string_lossy().into_owned()).unwrap_or_default();
                if percorso_progetto(dir, &titolo).exists() && !p.exists() {
                    n += 1;
                }
            }
        }
    }
    n
}

fn cartella_di(testo: &str) -> Option<String> {
    testo.lines().take(10).find_map(|l| l.strip_prefix("cartella: ")).map(|c| c.trim().to_string())
}

/// Nota-cardine di un progetto dello Studio: la crea con le sezioni se manca.
pub fn nota_progetto(dir: &Path, progetto: &str, cartella: &Path) -> Result<PathBuf, String> {
    let mut titolo = progetto.to_string();
    let p = percorso_progetto(dir, &titolo);
    if p.is_file() {
        // Due progetti con lo stesso nome: la nota dice di quale cartella è
        let t = std::fs::read_to_string(&p).unwrap_or_default();
        let altra = cartella_di(&t).is_some_and(|c| c != cartella.display().to_string());
        if !altra {
            return Ok(p);
        }
        let madre = cartella.parent().and_then(|m| m.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        titolo = format!("{progetto} ({madre})");
        let p2 = percorso_progetto(dir, &titolo);
        if p2.is_file() {
            return Ok(p2);
        }
    }
    let progetto = titolo.as_str();
    let p = percorso_progetto(dir, progetto);
    if let Some(c) = p.parent() {
        std::fs::create_dir_all(c).map_err(|e| e.to_string())?;
    }
    let testo = format!(
        "---\ncreato: {d}\naggiornato: {d}\nfonte: Oltremare\ncartella: {c}\ntags: [progetto]\n---\n# {progetto}\n\n\
## Com'è fatto\n_Da completare: Claude la scrive al primo lavoro nello Studio (stack, struttura, comandi, convenzioni, punti delicati)._\n\n\
## Decisioni\n\n## Diario dei lavori\n",
        d = oggi(),
        c = cartella.display()
    );
    std::fs::write(&p, testo).map_err(|e| e.to_string())?;
    Ok(p)
}

/// Le note «Lavori <giorno>» del progetto, dalla più recente.
fn note_lavori(nota: &Path) -> Vec<PathBuf> {
    let Some(dir) = nota.parent() else { return vec![] };
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("Lavori ")) && p.extension().is_some_and(|x| x == "md")).collect();
    v.sort();
    v.reverse();
    v
}

/// Contenuto della nota del progetto per Claude: la nota-cardine (senza le voci vecchie del diario)
/// e le ultime voci di lavoro dalle note «Lavori <giorno>».
pub fn contesto_progetto(nota: &Path) -> String {
    let t = std::fs::read_to_string(nota).unwrap_or_default();
    let corpo = t.strip_prefix("---\n").and_then(|r| r.find("\n---\n").map(|i| r[i + 5..].to_string())).unwrap_or(t);
    let (prima, diario_vecchio) = corpo.split_once("## Diario dei lavori").unwrap_or((&corpo, ""));
    let mut voci: Vec<String> = vec![];
    for f in note_lavori(nota).into_iter().take(3) {
        let Ok(tx) = std::fs::read_to_string(&f) else { continue };
        for v in tx.split("\n### ").skip(1) {
            voci.push(format!("### {}", v.trim()));
        }
    }
    // Le voci rimaste nella nota-cardine (prima della nuova forma) contano ancora
    for v in diario_vecchio.split("\n### ").skip(1) {
        voci.push(format!("### {}", v.trim()));
    }
    let ultime: Vec<String> = voci.into_iter().take(8).collect();
    let mut out: String = prima.trim().chars().take(12_000).collect();
    if !ultime.is_empty() {
        out.push_str(&format!("\n\n## Diario dei lavori (ultime voci)\n{}", ultime.join("\n\n")));
    }
    out
}

/// Voce nel diario dei lavori di un progetto: nella nota «Lavori <giorno>» del progetto (collegata alla
/// nota-cardine) e una riga nel diario generale del giorno. Così in Obsidian ogni progetto ha il suo grafo.
pub fn lavoro_progetto(dir: &Path, progetto: &str, cartella: &Path, richiesta: &str, esito: &str, ok: bool) -> Result<(), String> {
    let ora = chrono::Local::now().format("%H:%M");
    let giorno = oggi();
    let r: String = richiesta.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(200).collect();
    let e: String = esito.trim().chars().take(800).collect();
    let voce = format!("### {giorno} {ora} · {}\n**Richiesta:** {r}\n\n{}", if ok { "fatto" } else { "non riuscito" }, e);
    let hub = nota_progetto(dir, progetto, cartella)?;
    let titolo = hub.file_stem().map(|x| x.to_string_lossy().into_owned()).unwrap_or_else(|| progetto.to_string());
    let lavori = hub.with_file_name(format!("Lavori {giorno}.md"));
    let rel_lavori = lavori.strip_prefix(dir).unwrap_or(&lavori).to_string_lossy().replace('\\', "/").trim_end_matches(".md").to_string();
    if lavori.is_file() {
        scrivi(dir, &rel_lavori, &voce, "aggiungi", "Progetti")?;
    } else {
        scrivi(dir, &rel_lavori, &format!("Lavori su [[{titolo}]] del {giorno} · vedi anche [[{giorno}]].\n\n{voce}"), "crea", "Progetti")?;
        // La nota-cardine rimanda al giorno di lavoro
        let t = std::fs::read_to_string(&hub).unwrap_or_default();
        let link = format!("- [[Lavori {giorno}]]");
        if !t.contains(&link) {
            let nuovo = if t.contains("## Diario dei lavori") { format!("{}\n{link}\n", t.trim_end()) } else { format!("{}\n\n## Diario dei lavori\n{link}\n", t.trim_end()) };
            std::fs::write(&hub, segna_aggiornato(&nuovo)).map_err(|e| e.to_string())?;
        }
    }
    // Il diario generale del giorno rimanda al progetto
    let _ = scrivi(dir, &format!("Diario/{giorno}"), &format!("- {ora} · lavoro su [[{titolo}]]: {} → [[Lavori {giorno}]]", r.chars().take(90).collect::<String>()), "aggiungi", "Diario");
    Ok(())
}

/// Cosa gli agenti sanno sempre: le preferenze e l'elenco delle note (per cercarle e collegarle).
pub fn contesto(dir: &Path) -> String {
    let mut out = String::new();
    if let Ok(t) = std::fs::read_to_string(dir.join("Preferenze.md")) {
        let corpo: String = t.split("\n---\n").last().unwrap_or(&t).lines().filter(|l| !l.starts_with("# ")).collect::<Vec<_>>().join("\n").trim().chars().take(2000).collect();
        if !corpo.is_empty() {
            out.push_str(&format!("Preferenze dell'utente (dalla nota [[Preferenze]]):\n{corpo}\n"));
        }
    }
    let tutte = note(dir);
    let mut gruppi: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    for (rel, _) in &tutte {
        let cart = rel.rsplit_once('/').map(|(c, _)| c.to_string()).unwrap_or_else(|| "(principale)".into());
        if cart == "Diario" {
            continue;
        }
        gruppi.entry(cart).or_default().push(titolo_di(rel));
    }
    let mut righe = vec![];
    for (c, v) in gruppi {
        let n = v.len();
        let mostra: Vec<String> = v.into_iter().take(40).map(|t| format!("[[{t}]]")).collect();
        righe.push(format!("- {c}: {}{}", mostra.join(", "), if n > 40 { format!(" (+{} altre)", n - 40) } else { String::new() }));
    }
    if !righe.is_empty() {
        out.push_str(&format!("Note in memoria:\n{}\n", righe.join("\n")));
    }
    out
}

/// Il grafo della memoria: note (nodi) e collegamenti [[...]] (archi), come la vista a grafo di Obsidian.
pub fn grafo(dir: &Path) -> Value {
    let tutte = note(dir);
    // Titolo (minuscolo) → percorso relativo, per risolvere i collegamenti
    let per_titolo: std::collections::HashMap<String, String> = tutte.iter().map(|(rel, _)| (titolo_di(rel).to_lowercase(), rel.clone())).collect();
    let mut nodi: Vec<Value> = vec![];
    let mut archi: Vec<(String, String)> = vec![];
    let mut mancanti: std::collections::BTreeSet<String> = Default::default();
    for (rel, p) in &tutte {
        let testo = std::fs::read_to_string(p).unwrap_or_default();
        // La cartella principale (Persone, Progetti, …) dà il colore; quella completa serve a raggruppare un progetto
        let cartella = rel.split_once('/').map(|(c, _)| c.to_string()).unwrap_or_default();
        let gruppo = rel.rsplit_once('/').map(|(c, _)| c.to_string()).unwrap_or_default();
        let aggiornato = testo.lines().take(8).find_map(|l| l.strip_prefix("aggiornato: ")).map(|x| x.trim().to_string());
        nodi.push(json!({ "id": rel, "title": titolo_di(rel), "folder": cartella, "group": gruppo, "size": testo.chars().count(), "updated": aggiornato }));
        for l in collegamenti(&testo) {
            let chiave = l.to_lowercase();
            match per_titolo.get(&chiave) {
                Some(dest) if dest != rel => archi.push((rel.clone(), dest.clone())),
                Some(_) => {}
                None => {
                    mancanti.insert(l.clone());
                    archi.push((rel.clone(), format!("?{l}")));
                }
            }
        }
    }
    for m in mancanti {
        nodi.push(json!({ "id": format!("?{m}"), "title": m, "folder": "", "size": 0, "updated": null, "missing": true }));
    }
    archi.sort();
    archi.dedup();
    json!({ "nodes": nodi, "edges": archi.into_iter().map(|(a, b)| json!({ "from": a, "to": b })).collect::<Vec<_>>() })
}

#[tauri::command]
pub fn memory_graph(app: tauri::State<'_, crate::App>) -> crate::error::Result<Value> {
    let dir = cartella(&app.store.settings()).ok_or_else(|| crate::error::AppError::msg("La memoria non è attiva"))?;
    Ok(grafo(&dir))
}

/// Una nota per l'interfaccia (per percorso o titolo): testo, collegamenti, chi la cita.
#[tauri::command]
pub fn memory_read(app: tauri::State<'_, crate::App>, note: String) -> crate::error::Result<Value> {
    let dir = cartella(&app.store.settings()).ok_or_else(|| crate::error::AppError::msg("La memoria non è attiva"))?;
    let mut v = leggi(&dir, &note).map_err(crate::error::AppError::msg)?;
    v["fullPath"] = json!(dir.join(v["percorso"].as_str().unwrap_or("")).to_string_lossy());
    Ok(v)
}

// ---------- Comandi per l'interfaccia ----------

/// Stato della memoria: cartella, quante note, se Obsidian è installato.
#[tauri::command]
pub fn memory_info(app: tauri::State<'_, crate::App>, project: Option<String>) -> Value {
    let s = app.store.settings();
    let dir = cartella(&s);
    let obsidian = Path::new("/Applications/Obsidian.app").exists();
    // La nota del progetto ha lo stesso nome che usa lo Studio (gruppo + cartella); se manca si crea
    let nota = match (&dir, project.as_deref()) {
        (Some(d), Some(p)) => {
            let cartella = Path::new(p);
            let titolo = crate::studio::titolo_progetto(&crate::studio::radici(&app), cartella);
            nota_progetto(d, &titolo, cartella).ok().map(|x| x.to_string_lossy().into_owned())
        }
        _ => None,
    };
    json!({
        "enabled": s.memory_enabled,
        "path": s.memory_path,
        "ready": dir.is_some(),
        "notes": dir.as_ref().map(|d| note(d).len()).unwrap_or(0),
        "obsidian": obsidian,
        "projectNote": nota,
        "suggested": trova_vault().map(|p| p.to_string_lossy().into_owned()),
    })
}

/// Apre una nota (o la cartella della memoria) in Obsidian se c'è, altrimenti con l'app predefinita.
#[tauri::command]
pub fn memory_open(handle: tauri::AppHandle, app: tauri::State<'_, crate::App>, path: Option<String>) -> crate::error::Result<()> {
    use tauri_plugin_opener::OpenerExt;
    let dir = cartella(&app.store.settings()).ok_or_else(|| crate::error::AppError::msg("La memoria non è attiva"))?;
    let bersaglio = path.map(PathBuf::from).filter(|p| p.starts_with(&dir)).unwrap_or_else(|| dir.join("Oltremare.md"));
    if Path::new("/Applications/Obsidian.app").exists() {
        let url = format!("obsidian://open?path={}", crate::oauth::url_encode(&bersaglio.to_string_lossy()));
        handle.opener().open_url(url, None::<&str>).map_err(|e| crate::error::AppError::msg(e.to_string()))
    } else {
        handle.opener().open_path(bersaglio.to_string_lossy(), None::<&str>).map_err(|e| crate::error::AppError::msg(e.to_string()))
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn scrive_legge_e_cerca() {
        let dir = std::env::temp_dir().join(format!("oltremare-memoria-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        prepara(&dir);
        let r = scrivi(&dir, "Anna Bianchi", "Responsabile acquisti di [[Negozio]]. Preferisce le mail brevi.", "crea", "Persone").unwrap();
        assert_eq!(r["percorso"], "Persone/Anna Bianchi.md");
        assert!(scrivi(&dir, "Anna Bianchi", "x", "crea", "Persone").is_err(), "crea non sovrascrive");
        scrivi(&dir, "[[anna bianchi]]", "Telefono dell'ufficio in rubrica.", "aggiungi", "").unwrap();
        let l = leggi(&dir, "Anna Bianchi").unwrap();
        assert!(l["testo"].as_str().unwrap().contains("Telefono"));
        assert_eq!(l["collegamenti"][0], "Negozio");
        let c = cerca(&dir, "acquisti", 5);
        assert_eq!(c[0]["nota"], "Anna Bianchi");
        scrivi(&dir, "Anna Bianchi", "Nuovo testo.", "sostituisci", "").unwrap();
        assert!(std::fs::read_to_string(dir.join("Persone/Anna Bianchi.md")).unwrap().contains("creato:"), "le proprietà restano");
        assert_eq!(std::fs::read_dir(dir.join(".trash")).unwrap().count(), 1, "la vecchia versione va in .trash");
        assert!(scrivi(&dir, "Chiavi", "la chiave è sk-ant-abcdefghijklmnopqrstuv", "crea", "").is_err());
        assert!(scrivi(&dir, "../fuori", "x", "crea", "").is_err() || !dir.parent().unwrap().join("fuori.md").exists());
        lavoro_progetto(&dir, "negozio", Path::new("/tmp/negozio"), "aggiungi sconti", "Fatto: sconti con test.", true).unwrap();
        lavoro_progetto(&dir, "negozio", Path::new("/tmp/negozio"), "correggi arrotondamento", "Fatto.", true).unwrap();
        let hub = dir.join("Progetti/negozio/negozio.md");
        let p = std::fs::read_to_string(&hub).unwrap();
        assert!(p.contains("## Com'è fatto") && p.contains("[[Lavori ") , "la nota-cardine rimanda al giorno di lavoro: {p}");
        let lavori = note_lavori(&hub);
        assert_eq!(lavori.len(), 1);
        let l = std::fs::read_to_string(&lavori[0]).unwrap();
        assert!(l.contains("[[negozio]]") && l.contains("aggiungi sconti") && l.contains("correggi arrotondamento"));
        let c = contesto_progetto(&hub);
        assert!(c.contains("Com'è fatto") && c.contains("correggi arrotondamento") && !c.contains("creato:"));
        // Una nota piatta vecchia finisce nella cartella del progetto
        std::fs::write(dir.join("Progetti/Vecchio.md"), "---\ncartella: /tmp/vecchio\n---\n# Vecchio\n\n## Com'è fatto\nroba\n\n## Diario dei lavori\n### 2026-01-01 · fatto\nx\n").unwrap();
        let v = nota_progetto(&dir, "Vecchio", Path::new("/tmp/vecchio")).unwrap();
        assert_eq!(v, dir.join("Progetti/Vecchio/Vecchio.md"));
        assert!(!dir.join("Progetti/Vecchio.md").exists());
        assert!(contesto_progetto(&v).contains("2026-01-01"));
        diario(&dir, "Briefing", "Buongiorno.").unwrap();
        assert!(contesto(&dir).contains("[[Anna Bianchi]]"));
        let g = grafo(&dir);
        assert!(g["nodes"].as_array().unwrap().iter().any(|n| n["title"] == "Anna Bianchi"));
        scrivi(&dir, "Luca Verdi", "Lavora con [[Anna Bianchi]] e [[Fornitore Rossi]].", "crea", "Persone").unwrap();
        let g = grafo(&dir);
        assert!(g["edges"].as_array().unwrap().iter().any(|e| e["from"] == "Persone/Luca Verdi.md" && e["to"] == "Persone/Anna Bianchi.md"));
        assert!(g["nodes"].as_array().unwrap().iter().any(|n| n["id"] == "?Fornitore Rossi" && n["missing"] == true), "un collegamento a una nota che non c'è resta come nodo mancante");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

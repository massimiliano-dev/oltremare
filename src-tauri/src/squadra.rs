//! La squadra dello Studio: i sotto-agenti specializzati che il senior può usare (definiti in
//! `~/.claude/agents/`, così valgono in ogni progetto) e la lettura in diretta di chi sta facendo cosa.

use crate::error::{AppError, Result};
use crate::tools::sviluppo;
use crate::App;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tauri::State;

const VERSIONE: u32 = 1;

/// Gli agenti di Oltremare: (nome del file, contenuto).
fn definizioni() -> Vec<(&'static str, String)> {
    let comune = "Lavori come sotto-agente di uno sviluppatore senior che coordina il lavoro e ne risponde a un CTO: ricevi un incarico preciso, lo porti a termine nei file e nelle cartelle che ti sono stati assegnati e riferisci.\n\
- Prima leggi il codice coinvolto e le convenzioni del progetto (struttura, stile, librerie già in uso, test); segui quelle, non le tue abitudini.\n\
- Resta nel perimetro dell'incarico: niente modifiche fuori dai file assegnati, niente funzionalità in più. Se per finire servirebbe toccare altro, non farlo: segnalalo nel rapporto.\n\
- Verifica quello che fai (build, test, controllo dei tipi disponibili); se qualcosa fallisce, correggilo o dillo.\n\
- Non fare domande: scegli l'interpretazione più ragionevole e dichiarala.\n\
- Rapporto finale, breve e concreto: file toccati, cosa hai fatto, come l'hai verificato (comandi e risultato), dubbi o cose lasciate a chi coordina.";
    vec![
        (
            "oltremare-backend.md",
            format!(
                "---\nname: oltremare-backend\ndescription: Sviluppatore backend senior per API, logica di dominio, accesso ai dati, migrazioni, integrazioni, job e sicurezza lato server. Da usare per incarichi su server, database e servizi; non per interfaccia.\ntools: Read, Edit, Write, MultiEdit, Grep, Glob, Bash, WebFetch, WebSearch, TodoWrite, Skill\nmodel: inherit\nmaxTurns: 60\n---\n<!-- oltremare-agent-version: {VERSIONE} -->\n# Backend\n\n{comune}\n\n\
Competenze da far valere: contratti delle API chiari e coerenti con quelli esistenti, validazione degli input ai confini, errori espliciti e gestiti, transazioni e migrazioni reversibili, query efficienti (niente N+1, indici dove servono), nessun segreto nel codice, log utili senza dati sensibili, test sui comportamenti che cambi. Preferisci le librerie già nel progetto."
            ),
        ),
        (
            "oltremare-frontend.md",
            format!(
                "---\nname: oltremare-frontend\ndescription: Sviluppatore frontend senior con occhio da designer per interfacce, componenti, stile, accessibilità, stati e copy. Da usare per incarichi su schermate, componenti e CSS; non per server o database.\ntools: Read, Edit, Write, MultiEdit, Grep, Glob, Bash, WebFetch, WebSearch, TodoWrite, Skill\nmodel: inherit\nmaxTurns: 60\n---\n<!-- oltremare-agent-version: {VERSIONE} -->\n# Frontend\n\n{comune}\n\n\
Prima di toccare l'interfaccia carica con lo strumento Skill le skill `impeccable` (design e qualità dell'interfaccia), `taste-skill` e `humanizer`, se sono installate: segui le loro indicazioni per direzione visiva, gerarchia, spaziature, stati (hover, disabilitato, caricamento, errore, vuoto), copy naturale e non robotico. Se una skill non c'è, vai avanti senza.\n\
Competenze da far valere: componenti coerenti con quelli esistenti (riusali prima di crearne), token e variabili del progetto invece di valori fissi, accessibilità (tastiera, contrasto, etichette, focus visibile), responsività, nessun layout che salta durante il caricamento, testi nella lingua del prodotto. Controlla il risultato nel browser quando è possibile (Claude in Chrome, se disponibile)."
            ),
        ),
    ]
}

/// Scrive gli agenti in `~/.claude/agents/` se mancano o sono di una versione precedente (le modifiche
/// dell'utente a una versione uguale restano).
pub fn installa_agenti() {
    let Some(home) = crate::shell_env::home() else { return };
    let dir = home.join(".claude/agents");
    let _ = std::fs::create_dir_all(&dir);
    let marca = format!("oltremare-agent-version: {VERSIONE}");
    for (nome, testo) in definizioni() {
        let p = dir.join(nome);
        let vecchio = std::fs::read_to_string(&p).unwrap_or_default();
        if vecchio.contains(&marca) {
            continue;
        }
        let _ = std::fs::write(&p, testo);
    }
}

fn breve(t: &str, n: usize) -> String {
    let t = t.trim();
    if t.chars().count() > n { format!("{}…", t.chars().take(n).collect::<String>().trim_end()) } else { t.to_string() }
}

/// Il rapporto del sotto-agente, senza la coda tecnica (agentId, usage) che Claude Code aggiunge.
fn rapporto(contenuto: &Value) -> (String, Option<i64>, Option<i64>) {
    let mut testo = String::new();
    let mut token = None;
    let mut durata = None;
    for b in contenuto.as_array().into_iter().flatten().filter(|b| b["type"] == "text") {
        let x = b["text"].as_str().unwrap_or_default();
        if x.starts_with("agentId:") {
            token = x.split("subagent_tokens:").nth(1).and_then(|s| s.trim().split_whitespace().next()).and_then(|s| s.parse().ok());
            durata = x.split("duration_ms:").nth(1).and_then(|s| s.trim().split(|c: char| !c.is_ascii_digit()).next()).and_then(|s| s.parse().ok());
        } else {
            testo.push_str(x);
        }
    }
    if let Value::String(s) = contenuto {
        testo = s.clone();
    }
    (testo, token, durata)
}

fn quando(v: &Value) -> i64 {
    v["timestamp"].as_str().and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok()).map(|d| d.timestamp_millis()).unwrap_or(0)
}

/// Cosa stanno facendo i sotto-agenti della sessione: incarichi, azioni, rapporti.
#[tauri::command]
pub fn studio_team(app: State<'_, App>, project: String, session: Option<String>) -> Result<Value> {
    let p = crate::studio::cartella_pub(&app, &project)?;
    let file: Option<PathBuf> = match session.filter(|s| !s.is_empty()) {
        Some(id) => sviluppo::sessioni(&p).into_iter().map(|(f, _)| f).find(|f| f.file_stem().is_some_and(|s| s.to_string_lossy() == id)),
        None => sviluppo::ultima_conversazione(&p).and_then(|id| sviluppo::sessioni(&p).into_iter().map(|(f, _)| f).find(|f| f.file_stem().is_some_and(|s| s.to_string_lossy() == id))),
    };
    let Some(file) = file else { return Ok(json!({ "session": null, "agents": [] })) };
    let attivo = sviluppo::headless_attivo(&p) == Some(true);
    leggi_squadra(&file, attivo)
}

/// Legge incarichi, azioni e rapporti dei sotto-agenti dal transcript della sessione e dai loro.
pub fn leggi_squadra(file: &Path, attivo: bool) -> Result<Value> {
    let t = std::fs::read_to_string(file).map_err(|e| AppError::msg(e.to_string()))?;
    // Incarichi (tool_use Agent) e rapporti (tool_result) nella conversazione principale
    let mut agenti: Vec<Value> = vec![];
    let mut indice: std::collections::HashMap<String, usize> = Default::default();
    for riga in t.lines() {
        let Ok(v) = serde_json::from_str::<Value>(riga) else { continue };
        match v["type"].as_str() {
            Some("assistant") => {
                for b in v["message"]["content"].as_array().into_iter().flatten() {
                    if b["type"] == "tool_use" && matches!(b["name"].as_str(), Some("Agent") | Some("Task")) {
                        let id = b["id"].as_str().unwrap_or_default().to_string();
                        let input = &b["input"];
                        indice.insert(id.clone(), agenti.len());
                        agenti.push(json!({
                            "toolUseId": id,
                            "type": input["subagent_type"].as_str().unwrap_or("general-purpose"),
                            "description": input["description"].as_str().unwrap_or(""),
                            "prompt": breve(input["prompt"].as_str().unwrap_or(""), 600),
                            "startedAt": quando(&v),
                            "endedAt": null,
                            "running": attivo,
                            "report": "",
                            "steps": [],
                            "toolUses": 0,
                            "tokens": null,
                        }));
                    }
                }
            }
            Some("user") => {
                for b in v["message"]["content"].as_array().into_iter().flatten().filter(|b| b["type"] == "tool_result") {
                    if let Some(&i) = b["tool_use_id"].as_str().and_then(|id| indice.get(id)) {
                        let (testo, token, durata) = rapporto(&b["content"]);
                        // Lanciato in background: la risposta immediata è solo un avviso tecnico, il rapporto arriva dal suo transcript
                        if testo.trim_start().starts_with("Async agent launched") {
                            agenti[i]["background"] = json!(true);
                            continue;
                        }
                        agenti[i]["report"] = json!(breve(&testo, 2500));
                        agenti[i]["endedAt"] = json!(quando(&v));
                        agenti[i]["running"] = json!(false);
                        agenti[i]["tokens"] = json!(token);
                        agenti[i]["durationMs"] = json!(durata);
                        agenti[i]["error"] = json!(b["is_error"].as_bool() == Some(true));
                    }
                }
            }
            _ => {}
        }
    }
    // Il lavoro di ciascuno, dal suo transcript
    let dir = file.with_extension("").join("subagents");
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let m = e.path();
            if !m.to_string_lossy().ends_with(".meta.json") {
                continue;
            }
            let Ok(meta) = std::fs::read_to_string(&m).and_then(|s| serde_json::from_str::<Value>(&s).map_err(std::io::Error::other)) else { continue };
            let Some(&i) = meta["toolUseId"].as_str().and_then(|id| indice.get(id)) else { continue };
            let jsonl = PathBuf::from(m.to_string_lossy().replace(".meta.json", ".jsonl"));
            agenti[i]["agentId"] = json!(m.file_name().map(|n| n.to_string_lossy().trim_start_matches("agent-").trim_end_matches(".meta.json").to_string()));
            if let Ok(s) = std::fs::read_to_string(&jsonl) {
                let mut passi: Vec<Value> = vec![];
                let mut ultimo_testo = String::new();
                let mut ultimo = 0;
                // L'ultimo messaggio è solo testo (nessuno strumento): il sotto-agente ha concluso con il rapporto
                let mut chiuso_con_testo = false;
                for riga in s.lines() {
                    let Ok(v) = serde_json::from_str::<Value>(riga) else { continue };
                    if v["type"] != "assistant" {
                        continue;
                    }
                    ultimo = quando(&v).max(ultimo);
                    let mut usa_strumenti = false;
                    for b in v["message"]["content"].as_array().into_iter().flatten() {
                        match b["type"].as_str() {
                            Some("tool_use") => {
                                usa_strumenti = true;
                                passi.push(json!({ "tool": b["name"].as_str().unwrap_or("").replace("mcp__", "").replace("__", " · "), "title": sviluppo::titolo_breve(&b["input"]) }));
                            }
                            Some("text") => ultimo_testo = b["text"].as_str().unwrap_or("").to_string(),
                            _ => {}
                        }
                    }
                    chiuso_con_testo = !usa_strumenti && !ultimo_testo.trim().is_empty();
                }
                agenti[i]["toolUses"] = json!(passi.len());
                let salta = passi.len().saturating_sub(8);
                agenti[i]["steps"] = json!(passi.into_iter().skip(salta).collect::<Vec<_>>());
                agenti[i]["lastAt"] = json!(ultimo);
                let in_background = agenti[i]["background"].as_bool() == Some(true);
                if in_background && (chiuso_con_testo || !attivo) {
                    agenti[i]["running"] = json!(false);
                    agenti[i]["endedAt"] = json!(ultimo);
                    agenti[i]["report"] = json!(breve(&ultimo_testo, 2500));
                } else if agenti[i]["report"].as_str().unwrap_or("").is_empty() && !ultimo_testo.trim().is_empty() {
                    agenti[i]["progress"] = json!(breve(&ultimo_testo, 300));
                }
            }
        }
    }
    if !attivo {
        for a in agenti.iter_mut() {
            a["running"] = json!(false);
        }
    }
    Ok(json!({ "session": file.file_stem().map(|s| s.to_string_lossy().into_owned()), "agents": agenti, "running": attivo }))
}

/// Gli agenti definiti (di Oltremare e dell'utente), per mostrarli e per sapere quali tipi esistono.
#[tauri::command]
pub fn studio_agents(project: String) -> Vec<Value> {
    let mut out = vec![];
    let mut dirs: Vec<PathBuf> = vec![];
    if let Some(h) = crate::shell_env::home() {
        dirs.push(h.join(".claude/agents"));
    }
    dirs.push(Path::new(&project).join(".claude/agents"));
    for d in dirs {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().is_none_or(|x| x != "md") {
                continue;
            }
            let Ok(t) = std::fs::read_to_string(&p) else { continue };
            let campo = |k: &str| t.lines().take(20).find_map(|l| l.strip_prefix(&format!("{k}:"))).map(|v| v.trim().to_string()).unwrap_or_default();
            let nome = if campo("name").is_empty() { p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default() } else { campo("name") };
            out.push(json!({ "name": nome, "description": campo("description"), "oltremare": t.contains("oltremare-agent-version"), "project": d.starts_with(&project) }));
        }
    }
    out
}

#[cfg(test)]
mod prove {
    #[test]
    #[ignore = "legge una sessione vera di Claude Code (OLTREMARE_SESSIONE=file.jsonl)"]
    fn legge_la_squadra() {
        let f = std::path::PathBuf::from(std::env::var("OLTREMARE_SESSIONE").unwrap());
        let v = super::leggi_squadra(&f, false).unwrap();
        println!("{}", serde_json::to_string_pretty(&v).unwrap());
        assert!(v["agents"].as_array().map(|a| !a.is_empty()).unwrap_or(false));
    }

    #[test]
    fn gli_agenti_hanno_frontmatter_valido() {
        for (nome, testo) in super::definizioni() {
            assert!(testo.starts_with("---\nname: "), "{nome}");
            assert!(testo.contains("\ndescription: ") && testo.contains("\ntools: "), "{nome}");
            assert!(testo.contains("oltremare-agent-version"), "{nome}");
        }
    }
}

/// I plugin di Claude Code che Oltremare sa usare: quali sono installati e come si installano.
#[tauri::command]
pub fn claude_plugins() -> Vec<Value> {
    let home = crate::shell_env::home().unwrap_or_default();
    let installati = std::fs::read_to_string(home.join(".claude/plugins/installed_plugins.json")).unwrap_or_default();
    let settings = std::fs::read_to_string(home.join(".claude/settings.json")).unwrap_or_default();
    let marketplaces = std::fs::read_to_string(home.join(".claude/plugins/known_marketplaces.json")).unwrap_or_default();
    let ha = |nome: &str| installati.contains(&format!("\"{nome}@")) || settings.contains(&format!("\"{nome}@"));
    let mk = |nome: &str| marketplaces.contains(&format!("\"{nome}\""));
    [
        ("ponytail", "Ponytail", "Fa scrivere meno codice: regole del «senior più pigro» in ogni sessione, più /ponytail-review sul diff.", "ponytail", "DietrichGebert/ponytail", "ponytail@ponytail", ""),
        ("mattpocock-skills", "Wayfinder (skill di Matt Pocock)", "Pianifica lavori troppo grandi per una sessione come mappa di decisioni sul tracker: nello Studio scrivi «/wayfinder …».", "mattpocock", "mattpocock/skills", "mattpocock-skills@mattpocock", ""),
        ("open-code-review", "Open Code Review (Alibaba)", "Revisione del codice con regole: il CTO la usa in modalità delega. Serve anche la CLI: npm install -g @alibaba-group/open-code-review", "open-code-review", "alibaba/open-code-review", "open-code-review@open-code-review", "ocr"),
        ("omniroute", "OmniRoute", "Gateway AI locale con molti provider e fallback, collegato a Claude Code come server MCP. Serve il gateway acceso (porta 20128).", "", "", "", "omniroute"),
    ]
    .into_iter()
    .map(|(id, nome, desc, mp, sorgente, pacchetto, cli)| {
        let cli_ok = cli.is_empty() || crate::shell_env::path_utente().split(':').any(|d| Path::new(d).join(cli).is_file()) || home.join(".local/bin").join(cli).is_file();
        // OmniRoute non è un plugin: è un server MCP registrato in ~/.claude.json più il programma da avviare
        if id == "omniroute" {
            let claude_json = std::fs::read_to_string(home.join(".claude.json")).unwrap_or_default();
            let registrato = claude_json.contains("\"omniroute\"");
            return json!({
                "id": id, "name": nome, "description": desc,
                "installed": registrato, "marketplace": registrato,
                "cli": cli, "cliOk": cli_ok,
                "commands": if registrato { vec!["npm install -g omniroute".to_string(), "omniroute".to_string()] } else { vec!["claude mcp add --transport http -s user omniroute http://localhost:20128/api/mcp/stream".to_string(), "npm install -g omniroute".to_string()] },
            });
        }
        json!({
            "id": id, "name": nome, "description": desc,
            "installed": ha(id), "marketplace": mk(mp),
            "cli": cli, "cliOk": cli_ok,
            "commands": [format!("claude plugin marketplace add {sorgente}"), format!("claude plugin install {pacchetto}")],
        })
    })
    .collect()
}

//! Configurazione locale: impostazioni e agenti in JSON, segreti nel portachiavi di sistema.

use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const KEYRING_SERVICE: &str = "app.oltremare";

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Come ti chiama l'assistente
    pub user_name: String,
    pub language: String,
    /// Modelli predefiniti per i nuovi agenti
    pub anthropic_model: String,
    pub openai_model: String,
    /// Permette endpoint compatibili OpenAI
    pub openai_base_url: String,
    /// Percorso di Claude Code (vuoto = cerca da solo)
    pub claude_code_path: String,
    /// Modello per Claude con l'abbonamento: alias come "opus" o "default" per quello del piano
    pub claude_code_model: String,
    /// Modello locale predefinito (id del catalogo o "custom:nome.gguf")
    pub local_model: String,
    /// "system" | "light" | "dark"
    pub theme: String,
    /// "character" | "orb"
    pub avatar_face: String,
    pub send_with_enter: bool,
    pub show_thinking: bool,
    /// Voce: ascolta sempre «Ehi Oltremare»
    pub voice_listen: bool,
    /// Modello di riconoscimento: "whisper-small" | "whisper-turbo"
    pub voice_stt: String,
    /// Voce delle risposte: "piper-paola" | "piper-miro" | "sistema"
    pub voice_tts: String,
    pub voice_speed: f32,
    /// Microfono (vuoto = predefinito)
    pub voice_device: String,
    /// Agente che riceve i comandi vocali
    pub voice_agent: String,
    /// Scorciatoia per parlare tenendo premuto
    pub voice_hotkey: String,
    /// Avatar flottante sempre in primo piano
    pub companion: bool,
    /// ID delle applicazioni OAuth (vuoti = quelli inclusi nella build, se presenti)
    pub ms_client_id: String,
    pub google_client_id: String,
    pub google_client_secret: String,
    /// Fonti di notifiche
    pub notifications_system: bool,
    pub notifications_teams: bool,
    pub notifications_slack: bool,
    pub notifications_mail: bool,
    /// Le notifiche di sistema si danno solo agli agenti con AI locale
    pub notifications_local_only: bool,
    pub notification_rules: Vec<crate::notifiche::Regola>,
    /// Fascia di silenzio (niente avvisi né voce), es. "22:00"–"07:00"
    pub quiet_from: String,
    pub quiet_to: String,
    /// Briefing del mattino
    pub briefing_enabled: bool,
    pub briefing_time: String,
    /// Giorni: 1 = lunedì … 7 = domenica
    pub briefing_days: Vec<u32>,
    /// Server MCP esterni (gestione issue, servizi…): i loro strumenti diventano strumenti degli agenti
    pub mcp_servers: Vec<ServerMcp>,
    /// Editor per lo sviluppo (app in stile VS Code); vuoto = il più usato tra quelli installati
    pub editor_app: String,
    /// Aspetto
    pub accent_hue: f64,
    pub accent_chroma: f64,
    pub density: String,
    pub font_scale: f64,
    /// Finestre
    pub palette_hotkey: String,
    pub autostart: bool,
    pub layout: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ServerMcp {
    pub id: String,
    pub name: String,
    /// "http" | "stdio"
    pub kind: String,
    pub url: String,
    /// Comando che stampa le intestazioni HTTP in JSON (come in Claude Code)
    pub headers_helper: String,
    pub command: String,
    pub args: Vec<String>,
    pub enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            user_name: String::new(),
            language: "italiano".into(),
            anthropic_model: "claude-opus-5-5".into(),
            openai_model: "gpt-5".into(),
            openai_base_url: "https://api.openai.com/v1".into(),
            claude_code_path: String::new(),
            claude_code_model: "default".into(),
            local_model: String::new(),
            theme: "system".into(),
            avatar_face: "character".into(),
            send_with_enter: true,
            show_thinking: true,
            voice_listen: false,
            voice_stt: "whisper-small".into(),
            voice_tts: "piper-paola".into(),
            voice_speed: 1.0,
            voice_device: String::new(),
            voice_agent: "assistente".into(),
            voice_hotkey: "Alt+Shift+Space".into(),
            companion: false,
            ms_client_id: String::new(),
            google_client_id: String::new(),
            google_client_secret: String::new(),
            notifications_system: false,
            notifications_teams: true,
            notifications_slack: true,
            notifications_mail: true,
            notifications_local_only: false,
            notification_rules: vec![],
            quiet_from: "22:00".into(),
            quiet_to: "07:30".into(),
            briefing_enabled: true,
            briefing_time: "08:00".into(),
            briefing_days: vec![1, 2, 3, 4, 5],
            mcp_servers: vec![],
            editor_app: String::new(),
            accent_hue: 275.0,
            accent_chroma: 0.2,
            density: "comfortable".into(),
            font_scale: 1.0,
            palette_hotkey: if cfg!(target_os = "macos") { "Alt+Space".into() } else { "Alt+Space".into() },
            autostart: false,
            layout: "operator".into(),
        }
    }
}

/// Cosa può fare un agente senza chiedere. Gli strumenti arrivano nelle fasi successive.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Policy {
    pub read_default: String,
    pub write_default: String,
    pub danger_default: String,
    pub per_tool: BTreeMap<String, String>,
    pub approval_timeout_s: u32,
    pub allow_session: bool,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            read_default: "allow".into(),
            write_default: "ask".into(),
            danger_default: "ask".into(),
            per_tool: BTreeMap::new(),
            approval_timeout_s: 180,
            allow_session: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub hue: f32,
    pub description: String,
    pub system_prompt: String,
    /// "anthropic" | "claude-code" | "openai" | "local"
    pub provider: String,
    pub model: String,
    /// Solo Claude: low | medium | high | xhigh | max
    pub effort: String,
    pub max_tokens: u32,
    pub max_turns: u32,
    pub tools: Vec<String>,
    pub policy: Policy,
    pub allowed_paths: Vec<String>,
    pub mail_accounts: Vec<String>,
    pub browser_domains: Vec<String>,
    /// "always" | "whenAsked" | "never"
    pub voice_reply: String,
    pub triggers: Vec<Value>,
    /// Agenti di sistema (Assistente, Briefing): non si eliminano
    pub system: bool,
    pub created_at: i64,
    pub updated_at: i64,
    /// Versione del formato, per aggiornare gli agenti creati da versioni precedenti
    /// (assente nei file vecchi: vale 0)
    #[serde(default)]
    pub version: u32,
}

pub const AGENT_VERSION: u32 = 4;
/// Strumenti degli agenti di sistema (le azioni che modificano chiedono conferma)
pub const STRUMENTI_BASE: &[&str] = &["fs", "shell", "apps", "clipboard", "mail", "calendar", "tasks", "notifications"];

impl Default for Agent {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: "Nuovo agente".into(),
            icon: "spark".into(),
            hue: 275.0,
            description: String::new(),
            system_prompt: String::new(),
            provider: "anthropic".into(),
            model: "claude-opus-5-5".into(),
            effort: "medium".into(),
            max_tokens: 32000,
            max_turns: 40,
            tools: vec![],
            policy: Policy::default(),
            allowed_paths: vec![],
            mail_accounts: vec![],
            browser_domains: vec![],
            voice_reply: "whenAsked".into(),
            triggers: vec![],
            system: false,
            created_at: 0,
            updated_at: 0,
            version: AGENT_VERSION,
        }
    }
}

pub fn agente_briefing() -> Agent {
    let t = now_ms();
    Agent {
        id: "briefing".into(),
        name: "Briefing".into(),
        icon: "sun".into(),
        hue: 230.0,
        description: "Ogni mattina: impegni, cose da fare, email e notifiche, letti a voce".into(),
        system_prompt: "Sei il briefing del mattino di Oltremare. Ricevi i dati già raccolti (calendario, attività, email da leggere, notifiche) \
e scrivi un briefing in italiano parlato, pensato per essere letto ad alta voce in meno di due minuti.\n\
Ordine: un saluto breve con il nome dell'utente; gli impegni di oggi con gli orari (e di domattina presto, se rilevanti); \
le cose da fare, numerate, le più urgenti prima; le email che richiedono una risposta o un'azione; le notifiche degne di nota. \
Sintetizza: niente elenchi lunghi, niente codici o indirizzi web. Se una sezione è vuota, saltala. \
Chiudi chiedendo se vuole che ti occupi di qualcosa. Se ti servono dettagli, usa gli strumenti di sola lettura."
            .into(),
        tools: ["calendario_eventi", "attivita_elenca", "mail_cerca", "mail_leggi", "notifiche_recenti", "account_posta"].iter().map(|s| s.to_string()).collect(),
        voice_reply: "always".into(),
        effort: "low".into(),
        system: true,
        created_at: t,
        updated_at: t,
        ..Agent::default()
    }
}

pub fn agente_sviluppatore() -> Agent {
    let t = now_ms();
    let mut a = Agent {
        id: "sviluppatore".into(),
        name: "Sviluppatore".into(),
        icon: "terminal".into(),
        hue: 300.0,
        description: "Sviluppa come faresti tu: Claude nell'editor, backend acceso, prove nel browser".into(),
        system_prompt: "Sei lo sviluppatore di fiducia dell'utente e lavori sui suoi progetti come lavorerebbe lui: dentro l'editor, parlando con Claude nel pannello di Claude Code, provando il risultato.\n\nCome lavori:\n1. Capisci il progetto: trova la cartella (di solito nelle cartelle di sviluppo), guarda README, script di avvio e struttura.\n2. Se sono collegati strumenti di gestione delle issue (server MCP), usali secondo le loro istruzioni: cerca o crea la issue, segna l'inizio del lavoro, metti la chiave della issue nei commit e aggiorna lo stato alla fine.\n3. Affida il lavoro sul codice a Claude nell'editor con «editor_claude_scrivi»: scrivi come scriverebbe l'utente (obiettivo, vincoli, come verificare). Poi segui con «editor_claude_stato» usando attendi_secondi (per esempio 300) invece di controllare di continuo.\n4. Quando Claude è fermo su una richiesta di permesso, guarda lo schermo (schermo_screenshot) e rispondi come farebbe l'utente: consenti comandi di sviluppo normali (build, test, avvio, git locale); non consentire push, pubblicazioni, cancellazioni di massa o comandi fuori dal progetto: in quei casi chiedi all'utente. Quando Claude fa una domanda, rispondi tu se la risposta è chiara dalla richiesta, altrimenti chiedi all'utente.\n5. Prova davvero: avvia backend e frontend con «avvia_processo» (leggi gli script del progetto per il comando giusto), controlla i log con «log_processo», apri l'app nel browser con gli strumenti web (web_apri sull'indirizzo locale, web_snapshot, web_screenshot, clic e compilazione) e verifica che la funzione richiesta funzioni.\n6. Se qualcosa non va, scrivi a Claude nel pannello cosa hai visto (errore, log, comportamento) e ripeti finché funziona.\n7. Alla fine ferma i processi che non servono più e riassumi all'utente: cosa è cambiato, come l'hai provato, cosa resta. Non fare push e non pubblicare senza una richiesta esplicita.".into(),
        tools: ["fs", "shell", "apps", "mcp", "computer", "browser", "apri_in_editor", "editor_claude_scrivi", "editor_claude_stato", "avvia_processo", "processi", "log_processo", "ferma_processo"].iter().map(|s| s.to_string()).collect(),
        voice_reply: "whenAsked".into(),
        effort: "medium".into(),
        max_turns: 200,
        system: false,
        created_at: t,
        updated_at: t,
        ..Agent::default()
    };
    // Scrivere a Claude e seguirlo è il suo lavoro: niente conferme a ogni messaggio
    for k in ["editor_claude_scrivi", "editor_claude_stato", "processi", "log_processo", "ferma_processo", "apri_in_editor"] {
        a.policy.per_tool.insert(k.to_string(), "allow".to_string());
    }
    a
}

fn default_agents() -> Vec<Agent> {
    let t = now_ms();
    vec![agente_briefing(), Agent {
        id: "assistente".into(),
        name: "Assistente".into(),
        icon: "spark".into(),
        hue: 275.0,
        description: "Il tuo assistente personale: chiedigli qualsiasi cosa".into(),
        system_prompt: "Sei Oltremare, l'assistente personale dell'utente. Sei cordiale, diretto e concreto. \
Rispondi in modo breve quando la domanda è semplice e approfondisci solo quando serve. \
Se non sai qualcosa, dillo."
            .into(),
        system: true,
        tools: STRUMENTI_BASE.iter().map(|s| s.to_string()).collect(),
        created_at: t,
        updated_at: t,
        ..Agent::default()
    }]
}

pub struct Store {
    config_dir: PathBuf,
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(tmp, path)?;
    Ok(())
}

impl Store {
    pub fn new(config_dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(config_dir.join("agenti"));
        let s = Self { config_dir };
        if s.agents().is_empty() {
            for a in default_agents() {
                let _ = s.save_agent(&a);
            }
        }
        // Agenti di sistema delle versioni precedenti: ricevono gli strumenti
        for mut a in s.agents() {
            if a.version < AGENT_VERSION {
                if a.system && a.id == "assistente" {
                    for t in ["fs", "shell", "apps", "clipboard", "mail", "calendar", "tasks", "notifications"] {
                        if !a.tools.iter().any(|x| x == t) {
                            a.tools.push(t.to_string());
                        }
                    }
                }
                a.version = AGENT_VERSION;
                let _ = s.save_agent(&a);
            }
        }
        // Versione 4: arriva lo Sviluppatore (una volta sola; se l'utente lo elimina non torna)
        if s.pref("migrazione:sviluppatore").is_none() {
            if s.agent("sviluppatore").is_err() {
                let mut d = agente_sviluppatore();
                if let Ok(a) = s.agent("assistente") {
                    d.provider = a.provider;
                    d.model = a.model;
                }
                let _ = s.save_agent(&d);
            }
            let _ = s.set_pref("migrazione:sviluppatore", serde_json::json!(true));
        }
        // Il briefing c'è sempre
        if s.agent("briefing").is_err() {
            let mut b = agente_briefing();
            // Stesso provider dell'Assistente, se già configurato
            if let Ok(a) = s.agent("assistente") {
                b.provider = a.provider;
                b.model = a.model;
            }
            let _ = s.save_agent(&b);
        }
        s
    }

    fn read_json<T: for<'de> Deserialize<'de> + Default>(&self, file: &str) -> T {
        std::fs::read(self.config_dir.join(file))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    fn write_json<T: Serialize>(&self, file: &str, value: &T) -> Result<()> {
        write_atomic(&self.config_dir.join(file), &serde_json::to_vec_pretty(value)?)
    }

    pub fn settings(&self) -> Settings {
        self.read_json("settings.json")
    }

    pub fn save_settings(&self, s: &Settings) -> Result<()> {
        self.write_json("settings.json", s)
    }

    // ---------- Preferenze dell'interfaccia (layout, tema…): chiave → JSON ----------

    pub fn pref(&self, key: &str) -> Option<Value> {
        let all: BTreeMap<String, Value> = self.read_json("prefs.json");
        all.get(key).cloned()
    }

    pub fn set_pref(&self, key: &str, value: Value) -> Result<()> {
        let mut all: BTreeMap<String, Value> = self.read_json("prefs.json");
        all.insert(key.to_string(), value);
        self.write_json("prefs.json", &all)
    }

    // ---------- Agenti: un file per agente ----------

    fn agent_path(&self, id: &str) -> Result<PathBuf> {
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return Err(AppError::msg("Identificativo agente non valido"));
        }
        Ok(self.config_dir.join("agenti").join(format!("{id}.json")))
    }

    pub fn agents(&self) -> Vec<Agent> {
        let Ok(dir) = std::fs::read_dir(self.config_dir.join("agenti")) else { return vec![] };
        let mut all: Vec<Agent> = dir
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
            .filter_map(|e| serde_json::from_slice(&std::fs::read(e.path()).ok()?).ok())
            .collect();
        // Gli agenti di sistema in cima, poi in ordine di creazione
        all.sort_by_key(|a| (a.id != "assistente", !a.system, a.created_at, a.name.clone()));
        all
    }

    pub fn agent(&self, id: &str) -> Result<Agent> {
        let bytes = std::fs::read(self.agent_path(id)?).map_err(|_| AppError::msg("Agente non trovato"))?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub fn save_agent(&self, a: &Agent) -> Result<()> {
        write_atomic(&self.agent_path(&a.id)?, &serde_json::to_vec_pretty(a)?)
    }

    pub fn delete_agent(&self, id: &str) -> Result<()> {
        if self.agent(id)?.system {
            return Err(AppError::msg("Gli agenti di sistema non si possono eliminare"));
        }
        std::fs::remove_file(self.agent_path(id)?)?;
        Ok(())
    }
}

// ---------- Account (Microsoft, Google) ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    /// "microsoft" | "google"
    pub provider: String,
    pub email: String,
    pub name: String,
    pub created_at: i64,
    /// Teams collegato (notifiche delle chat)
    #[serde(default)]
    pub teams: bool,
}

impl Store {
    pub fn accounts(&self) -> Vec<Account> {
        self.read_json("accounts.json")
    }

    pub fn save_account(&self, a: Account) -> Result<()> {
        let mut all = self.accounts();
        match all.iter_mut().find(|x| x.id == a.id) {
            Some(x) => *x = a,
            None => all.push(a),
        }
        self.write_json("accounts.json", &all)
    }

    pub fn remove_account(&self, id: &str) -> Result<()> {
        let all: Vec<Account> = self.accounts().into_iter().filter(|a| a.id != id).collect();
        let _ = delete_secret(&format!("oauth:{id}"));
        let _ = delete_secret(&format!("oauth-teams:{id}"));
        self.write_json("accounts.json", &all)
    }

    /// Account per email o id; senza indicazione, il primo.
    pub fn account(&self, rif: &str) -> Result<Account> {
        let all = self.accounts();
        let r = rif.trim().to_lowercase();
        if r.is_empty() {
            return all.into_iter().next().ok_or_else(|| AppError::msg("Nessun account collegato: collegane uno in Impostazioni → Account"));
        }
        all.into_iter()
            .find(|a| a.id == rif || a.email.to_lowercase() == r)
            .ok_or_else(|| AppError::msg(format!("Account «{rif}» non trovato")))
    }
}

// ---------- Segreti ----------

pub fn set_secret(key: &str, value: &str) -> Result<()> {
    keyring::Entry::new(KEYRING_SERVICE, key)?.set_password(value)?;
    Ok(())
}

pub fn get_secret(key: &str) -> Result<String> {
    Ok(keyring::Entry::new(KEYRING_SERVICE, key)?.get_password()?)
}

pub fn has_secret(key: &str) -> bool {
    get_secret(key).map(|s| !s.is_empty()).unwrap_or(false)
}

pub fn delete_secret(key: &str) -> Result<()> {
    keyring::Entry::new(KEYRING_SERVICE, key)?.delete_credential()?;
    Ok(())
}

pub fn key_name(provider: &str) -> String {
    format!("ai:{provider}")
}

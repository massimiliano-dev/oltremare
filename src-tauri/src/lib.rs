mod agent;
mod allegati;
mod avatar;
mod briefing;
mod db;
mod dev;
mod error;
mod finestre;
mod git;
mod isola;
mod mcp;
mod mcp_client;
mod permessi;
mod modelli;
mod notifiche;
mod oauth;
mod providers;
mod shell_env;
mod sidecar;
mod store;
mod studio;
mod tools;
mod triggers;
mod voce;

use agent::ciclo::Avvio;
use agent::{RunView, Runs};
use error::{AppError, Result};
use serde::Serialize;
use serde_json::Value;
use store::{now_ms, Agent, Settings, Store};
use tauri::{AppHandle, Emitter, Manager, State};

pub struct App {
    pub store: Store,
    pub data_dir: std::path::PathBuf,
    pub db: db::Db,
    pub runs: Runs,
    pub avatar: avatar::Avatar,
    pub approvazioni: agent::approvazioni::Approvazioni,
    pub mcp: mcp::Mcp,
    pub sidecar: sidecar::Sidecar,
    pub voce: voce::Voce,
    /// Accesso OAuth in attesa del browser: chiudere il canale lo annulla
    pub oauth_attesa: std::sync::Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    pub osservatori: triggers::Osservatori,
    pub mcp_clienti: mcp_client::Clienti,
    /// Download di modelli in corso
    pub download: std::sync::Mutex<std::collections::HashMap<String, tokio_util::sync::CancellationToken>>,
}

// ---------- Impostazioni e provider ----------

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Keys {
    anthropic: bool,
    openai: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsState {
    settings: Settings,
    keys: Keys,
}

pub(crate) fn settings_state(app: &App) -> SettingsState {
    SettingsState {
        settings: app.store.settings(),
        keys: Keys {
            anthropic: store::has_secret(&store::key_name("anthropic")),
            openai: store::has_secret(&store::key_name("openai")),
        },
    }
}

#[tauri::command]
fn settings_get(app: State<'_, App>) -> SettingsState {
    settings_state(&app)
}

#[tauri::command]
fn settings_save(handle: AppHandle, app: State<'_, App>, settings: Settings) -> Result<()> {
    let prima = app.store.settings();
    app.store.save_settings(&settings)?;
    let _ = handle.emit("settings-changed", settings_state(&app));
    if prima.mcp_servers != settings.mcp_servers {
        let h = handle.clone();
        tauri::async_runtime::spawn(async move { mcp_client::ricarica(&h).await });
    }
    finestre::impostazioni_cambiate(&handle, &prima, &settings);
    Ok(())
}

fn provider_valido(p: &str) -> Result<()> {
    if matches!(p, "anthropic" | "openai") {
        Ok(())
    } else {
        Err(AppError::msg("Provider sconosciuto"))
    }
}

#[tauri::command]
fn provider_key_set(handle: AppHandle, app: State<'_, App>, provider: String, key: String) -> Result<()> {
    provider_valido(&provider)?;
    let name = store::key_name(&provider);
    if key.trim().is_empty() {
        let _ = store::delete_secret(&name);
    } else {
        store::set_secret(&name, key.trim())?;
    }
    let _ = handle.emit("settings-changed", settings_state(&app));
    Ok(())
}

#[tauri::command]
async fn provider_models(app: State<'_, App>, provider: String) -> Result<Vec<String>> {
    provider_valido(&provider)?;
    let key = store::get_secret(&store::key_name(&provider)).map_err(|_| AppError::msg("Salva prima la chiave API"))?;
    providers::models(&provider, &key, &app.store.settings().openai_base_url).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderTest {
    ok: bool,
    ms: u64,
    models: usize,
    error: Option<String>,
}

#[tauri::command]
async fn provider_test(app: State<'_, App>, provider: String) -> Result<ProviderTest> {
    let t = std::time::Instant::now();
    let r = provider_models(app, provider).await;
    let ms = t.elapsed().as_millis() as u64;
    Ok(match r {
        Ok(m) => ProviderTest { ok: true, ms, models: m.len(), error: None },
        Err(e) => ProviderTest { ok: false, ms, models: 0, error: Some(e.to_string()) },
    })
}

// ---------- Preferenze dell'interfaccia ----------

#[tauri::command]
fn prefs_get(app: State<'_, App>, key: String) -> Option<Value> {
    app.store.pref(&key)
}

#[tauri::command]
fn prefs_set(handle: AppHandle, app: State<'_, App>, key: String, value: Value) -> Result<()> {
    app.store.set_pref(&key, value)?;
    let _ = handle.emit("prefs-changed", serde_json::json!({ "key": key }));
    Ok(())
}

// ---------- Agenti ----------

#[tauri::command]
fn agents_list(app: State<'_, App>) -> Vec<Agent> {
    app.store.agents()
}

#[tauri::command]
fn agent_save(handle: AppHandle, app: State<'_, App>, mut agent: Agent) -> Result<Agent> {
    agent.name = agent.name.trim().to_string();
    if agent.name.is_empty() {
        return Err(AppError::msg("Dai un nome all'agente"));
    }
    if !matches!(agent.provider.as_str(), "anthropic" | "claude-code" | "openai" | "local") {
        return Err(AppError::msg("Provider sconosciuto"));
    }
    let t = now_ms();
    match app.store.agent(&agent.id) {
        Ok(existing) => {
            // Gli agenti di sistema restano di sistema
            agent.system = existing.system;
            agent.created_at = existing.created_at;
        }
        Err(_) => {
            agent.id = uuid::Uuid::new_v4().to_string();
            agent.system = false;
            agent.created_at = t;
        }
    }
    agent.updated_at = t;
    app.store.save_agent(&agent)?;
    triggers::ricarica_cartelle(&handle);
    finestre::registra_scorciatoie(&handle);
    let _ = handle.emit("agents-changed", ());
    Ok(agent)
}

#[tauri::command]
fn agent_delete(handle: AppHandle, app: State<'_, App>, id: String) -> Result<()> {
    app.store.delete_agent(&id)?;
    for c in app.db.elimina_conversazioni_agente(&id)? {
        rimuovi_allegati(&app, &c);
    }
    let _ = handle.emit("agents-changed", ());
    Ok(())
}

#[tauri::command]
fn agent_duplicate(handle: AppHandle, app: State<'_, App>, id: String) -> Result<Agent> {
    let mut a = app.store.agent(&id)?;
    let t = now_ms();
    a.id = uuid::Uuid::new_v4().to_string();
    a.name = format!("{} (copia)", a.name);
    a.system = false;
    a.created_at = t;
    a.updated_at = t;
    app.store.save_agent(&a)?;
    let _ = handle.emit("agents-changed", ());
    Ok(a)
}

// ---------- Esecuzioni e conversazioni ----------

#[tauri::command]
fn run_start(handle: AppHandle, input: Avvio) -> Result<RunView> {
    agent::ciclo::avvia(&handle, input)
}

#[tauri::command]
fn run_cancel(app: State<'_, App>, run_id: String) {
    app.runs.annulla(&run_id);
}

#[tauri::command]
fn runs_cancel_all(app: State<'_, App>) {
    app.runs.annulla_tutte();
}

#[tauri::command]
fn runs_active(app: State<'_, App>) -> Vec<String> {
    app.runs.attive()
}

#[tauri::command]
fn run_get(app: State<'_, App>, run_id: String) -> Result<RunView> {
    app.db.run(&run_id)
}

#[tauri::command]
fn runs_list(app: State<'_, App>, agent_id: Option<String>, limit: Option<u32>, offset: Option<u32>) -> Result<Vec<RunView>> {
    app.db.runs(agent_id.as_deref(), limit.unwrap_or(50).min(200), offset.unwrap_or(0))
}

#[tauri::command]
fn conversations_list(app: State<'_, App>, agent_id: String) -> Result<Vec<db::Conversazione>> {
    app.db.conversazioni(&agent_id, 100)
}

#[tauri::command]
fn conversation_runs(app: State<'_, App>, conversation_id: String) -> Result<Vec<RunView>> {
    app.db.runs_conversazione(&conversation_id)
}

#[tauri::command]
fn conversation_delete(app: State<'_, App>, conversation_id: String) -> Result<()> {
    app.db.elimina_conversazione(&conversation_id)?;
    rimuovi_allegati(&app, &conversation_id);
    Ok(())
}

fn rimuovi_allegati(app: &App, conversation_id: &str) {
    if !conversation_id.is_empty() && conversation_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        let _ = std::fs::remove_dir_all(app.data_dir.join("allegati").join(conversation_id));
    }
}

/// Estensioni dei file che si possono allegare (per il selettore di file).
#[tauri::command]
fn attachment_types() -> Vec<&'static str> {
    allegati::estensioni_supportate()
}

/// Claude con l'abbonamento: Claude Code è installato e collegato?
#[tauri::command]
async fn claude_code_status(app: State<'_, App>) -> Result<providers::claude_code::StatoAccount> {
    Ok(providers::claude_code::stato(&app.store.settings().claude_code_path).await)
}

#[tauri::command]
fn avatar_state(app: State<'_, App>) -> avatar::Stato {
    app.avatar.corrente()
}

/// Ferma tutto: esecuzioni in corso e richieste di approvazione.
pub(crate) fn ferma_tutto(app: &AppHandle) {
    let st = app.state::<App>();
    st.approvazioni.nega_tutte();
    st.runs.annulla_tutte();
    st.voce.ferma_lettura();
    tools::computer::rilascia_tutto();
}

/// L'app, per i moduli che non ricevono l'AppHandle (es. i processi in background)
pub static APP: std::sync::OnceLock<AppHandle> = std::sync::OnceLock::new();

#[cfg(target_os = "macos")]
pub(crate) const KILL_SWITCH: &str = "Command+Shift+Escape";
#[cfg(not(target_os = "macos"))]
pub(crate) const KILL_SWITCH: &str = "Control+Alt+Escape";

// ---------- Strumenti e approvazioni ----------

#[tauri::command]
fn tool_catalog() -> Vec<Value> {
    tools::catalogo_vista()
}

#[tauri::command]
fn approval_answer(app: State<'_, App>, approval_id: String, decision: String, reason: Option<String>) -> Result<()> {
    use agent::approvazioni::Risposta;
    let r = match decision.as_str() {
        "allow" => Risposta::Consenti,
        "allowSession" => Risposta::ConsentiSessione,
        "allowAlways" => Risposta::ConsentiSempre,
        _ => Risposta::Nega(reason),
    };
    if app.approvazioni.rispondi(&approval_id, r) {
        Ok(())
    } else {
        Err(AppError::msg("Questa richiesta non è più in attesa"))
    }
}

#[tauri::command]
fn approvals_pending(app: State<'_, App>) -> Vec<Value> {
    app.approvazioni.pendenti()
}

#[tauri::command]
fn stop_everything(handle: AppHandle) {
    ferma_tutto(&handle);
}

#[tauri::command]
fn audit_list(app: State<'_, App>, limit: Option<u32>, offset: Option<u32>) -> Result<Vec<Value>> {
    app.db.audit_lista(limit.unwrap_or(100).min(500), offset.unwrap_or(0))
}

#[tauri::command]
fn storage_purge(app: State<'_, App>, days: u32) -> Result<usize> {
    let limite = now_ms() - days.max(1) as i64 * 86_400_000;
    app.db.pulisci(limite)
}

// ---------- AI locale ----------

#[tauri::command]
fn llama_status(app: State<'_, App>) -> sidecar::Stato {
    let mut s = app.sidecar.stato();
    s.installed = sidecar::binario::percorso_da_stato(&app);
    s
}

#[tauri::command]
async fn llama_start(handle: AppHandle, app: State<'_, App>, model_id: String) -> Result<()> {
    app.sidecar.avvia(&handle, &model_id).await
}

#[tauri::command]
async fn llama_stop(handle: AppHandle, app: State<'_, App>) -> Result<()> {
    app.sidecar.ferma(&handle).await;
    Ok(())
}

#[tauri::command]
fn llama_log(app: State<'_, App>) -> Vec<String> {
    app.sidecar.log()
}

#[tauri::command]
fn models_catalog(handle: AppHandle, kind: Option<String>) -> Vec<modelli::catalogo::Vista> {
    modelli::elenco(&handle, kind.as_deref())
}

#[tauri::command]
fn model_download(handle: AppHandle, id: String) -> Result<()> {
    modelli::scarica(&handle, &id)
}

#[tauri::command]
fn model_download_cancel(handle: AppHandle, id: String) {
    modelli::annulla(&handle, &id);
}

#[tauri::command]
async fn model_delete(handle: AppHandle, app: State<'_, App>, id: String) -> Result<()> {
    if app.sidecar.stato().model.as_deref() == Some(id.as_str()) {
        app.sidecar.ferma(&handle).await;
    }
    modelli::elimina(&handle, &id)
}

#[tauri::command]
fn model_import(handle: AppHandle, path: String) -> Result<String> {
    modelli::importa(&handle, &path)
}

#[tauri::command]
fn hw_info() -> modelli::Hardware {
    modelli::hardware(sidecar::binario::backend())
}

#[tauri::command]
fn models_folder(app: State<'_, App>) -> String {
    app.data_dir.join("modelli").to_string_lossy().into_owned()
}

// ---------- Notifiche e briefing ----------

#[tauri::command]
fn notifications_list(app: State<'_, App>, hours: Option<i64>, app_name: Option<String>, query: Option<String>) -> Result<Vec<notifiche::Notifica>> {
    app.db.notifiche(notifiche::ore_fa(hours.unwrap_or(48)), app_name.as_deref().filter(|a| !a.is_empty()), query.as_deref().filter(|q| !q.is_empty()), 300)
}

#[tauri::command]
fn notifications_mark_read(app: State<'_, App>) -> Result<usize> {
    app.db.notifiche_lette(now_ms())
}

#[tauri::command]
async fn notifications_sources(app: State<'_, App>) -> Result<Value> {
    let slack = match store::get_secret("slack:token") {
        Ok(t) if !t.is_empty() => notifiche::slack::verifica(&t).await.ok(),
        _ => None,
    };
    Ok(serde_json::json!({
        "systemSupported": cfg!(target_os = "macos"),
        "fullDisk": permessi::accesso_disco(),
        "teamsAccounts": app.store.accounts().iter().filter(|a| a.teams).map(|a| a.email.clone()).collect::<Vec<_>>(),
        "slack": slack,
    }))
}

#[tauri::command]
async fn slack_token_set(token: String) -> Result<String> {
    if token.trim().is_empty() {
        let _ = store::delete_secret("slack:token");
        return Ok(String::new());
    }
    let chi = notifiche::slack::verifica(token.trim()).await.map_err(AppError::msg)?;
    store::set_secret("slack:token", token.trim())?;
    Ok(chi)
}

#[tauri::command]
async fn briefing_now(handle: AppHandle) -> Result<RunView> {
    briefing::esegui(&handle, true).await
}

#[tauri::command]
fn briefing_latest(app: State<'_, App>) -> Option<Value> {
    app.store.pref("briefing:ultimo")
}

#[tauri::command]
fn hotkey_check(handle: AppHandle, accelerator: String, agent_id: Option<String>) -> Option<String> {
    finestre::controlla(&handle, &accelerator, agent_id.as_deref())
}

#[tauri::command]
fn palette_toggle(handle: AppHandle) {
    finestre::palette_alterna(&handle);
}

#[tauri::command]
fn palette_hide(handle: AppHandle) {
    if let Some(w) = handle.get_webview_window("palette") {
        let _ = w.hide();
    }
}

/// Dalla palette: apre l'esecuzione nella finestra principale.
#[tauri::command]
fn palette_open_run(handle: AppHandle, run_id: String, conversation_id: String, agent_id: String) {
    palette_hide(handle.clone());
    mostra_principale(&handle);
    let _ = handle.emit("open-run", serde_json::json!({ "runId": run_id, "conversationId": conversation_id, "agentId": agent_id }));
}

#[tauri::command]
fn autostart_set(handle: AppHandle, enabled: bool) -> Result<()> {
    finestre::imposta_avvio(&handle, enabled).map_err(AppError::msg)
}

/// Utilizzo dei provider (ultimi 30 giorni) e stato dei limiti dell'abbonamento.
#[tauri::command]
fn usage_stats(app: State<'_, App>) -> Result<Value> {
    let oggi = chrono::Local::now().date_naive().and_hms_opt(0, 0, 0).and_then(|d| d.and_local_timezone(chrono::Local).single()).map(|d| d.timestamp_millis()).unwrap_or_else(now_ms);
    let mut v = app.db.utilizzo(oggi, 30)?;
    v["subscription"] = app.store.pref("abbonamento:limiti").unwrap_or(Value::Null);
    Ok(v)
}

#[tauri::command]
fn cron_preview(expr: String) -> Result<Vec<i64>> {
    triggers::prossime(&expr, 3).map_err(AppError::msg)
}

// ---------- Strumenti esterni (MCP) e sviluppo ----------

#[tauri::command]
fn mcp_status(app: State<'_, App>) -> Vec<mcp_client::StatoServer> {
    app.mcp_clienti.stato()
}

#[tauri::command]
async fn mcp_reload(handle: AppHandle) -> Result<Vec<mcp_client::StatoServer>> {
    mcp_client::ricarica(&handle).await;
    Ok(handle.state::<App>().mcp_clienti.stato())
}

/// Intestazioni fisse (HTTP) o variabili d'ambiente (processo locale) di un server: nel portachiavi.
#[tauri::command]
fn mcp_secret_set(id: String, kind: String, value: String) -> Result<()> {
    let chiave = match kind.as_str() {
        "headers" => format!("mcp-headers:{id}"),
        "env" => format!("mcp-env:{id}"),
        _ => return Err(AppError::msg("Tipo sconosciuto")),
    };
    if value.trim().is_empty() {
        let _ = store::delete_secret(&chiave);
        return Ok(());
    }
    let v: Value = serde_json::from_str(&value).map_err(|_| AppError::msg("Scrivi un oggetto JSON, es. {\"Authorization\": \"Bearer …\"}"))?;
    if !v.is_object() {
        return Err(AppError::msg("Scrivi un oggetto JSON"));
    }
    store::set_secret(&chiave, &v.to_string())
}

#[tauri::command]
fn mcp_secret_has(id: String) -> Value {
    serde_json::json!({
        "headers": store::get_secret(&format!("mcp-headers:{id}")).is_ok_and(|x| !x.is_empty()),
        "env": store::get_secret(&format!("mcp-env:{id}")).is_ok_and(|x| !x.is_empty()),
    })
}

/// Importa i server MCP configurati in Claude Code (ambito utente). Restituisce i nomi aggiunti.
#[tauri::command]
fn mcp_import_claude(handle: AppHandle, app: State<'_, App>) -> Result<Vec<String>> {
    let mut s = app.store.settings();
    let mut aggiunti = vec![];
    for (server, headers, env) in mcp_client::da_claude_code() {
        if s.mcp_servers.iter().any(|x| x.name.eq_ignore_ascii_case(&server.name)) {
            continue;
        }
        if let Some(h) = headers {
            store::set_secret(&format!("mcp-headers:{}", server.id), &h.to_string())?;
        }
        if let Some(e) = env {
            store::set_secret(&format!("mcp-env:{}", server.id), &e.to_string())?;
        }
        aggiunti.push(server.name.clone());
        s.mcp_servers.push(server);
    }
    if !aggiunti.is_empty() {
        settings_save(handle, app, s)?;
    }
    Ok(aggiunti)
}

#[tauri::command]
fn editors_list() -> Vec<Value> {
    tools::sviluppo::editor_installati().into_iter().map(|(nome, percorso)| serde_json::json!({ "name": nome, "path": percorso })).collect()
}

// ---------- Account ----------

#[tauri::command]
fn accounts_list(app: State<'_, App>) -> Vec<store::Account> {
    app.store.accounts()
}

#[tauri::command]
fn oauth_available(app: State<'_, App>) -> Vec<&'static str> {
    oauth::disponibili(&app.store.settings())
}

/// Collega un account (o Teams, per un account Microsoft già collegato).
#[tauri::command]
async fn oauth_connect(handle: AppHandle, app: State<'_, App>, provider: String, teams_for: Option<String>) -> Result<store::Account> {
    use tauri_plugin_opener::OpenerExt;
    let (tx, rx) = tokio::sync::oneshot::channel();
    if let Ok(mut p) = app.oauth_attesa.lock() {
        *p = Some(tx);
    }
    let settings = app.store.settings();
    let esistente = teams_for.as_deref().map(|id| app.store.account(id)).transpose()?;
    let tag = if esistente.is_some() { "teams" } else { "" };
    let acc = oauth::autorizza(
        &provider,
        tag,
        &settings,
        |url| {
            let _ = handle.emit("oauth-url", url);
            handle.opener().open_url(url, None::<&str>).map_err(|e| AppError::msg(format!("Impossibile aprire il browser: {e}")))
        },
        rx,
        esistente.as_ref().map(|a| a.id.as_str()),
    )
    .await?;
    let finale = match esistente {
        // Teams: si segna sull'account già collegato
        Some(mut e) => {
            e.teams = true;
            e
        }
        None => {
            // Stesso indirizzo già presente: si aggiornano i token di quello
            if let Some(dup) = app.store.accounts().into_iter().find(|a| a.email.eq_ignore_ascii_case(&acc.email) && a.provider == acc.provider) {
                let t = store::get_secret(&format!("oauth:{}", acc.id))?;
                store::set_secret(&format!("oauth:{}", dup.id), &t)?;
                let _ = store::delete_secret(&format!("oauth:{}", acc.id));
                dup
            } else {
                acc
            }
        }
    };
    app.store.save_account(finale.clone())?;
    let _ = handle.emit("accounts-changed", ());
    Ok(finale)
}

#[tauri::command]
fn oauth_cancel(app: State<'_, App>) {
    if let Ok(mut p) = app.oauth_attesa.lock() {
        p.take();
    }
}

#[tauri::command]
fn account_remove(handle: AppHandle, app: State<'_, App>, id: String) -> Result<()> {
    app.store.remove_account(&id)?;
    let _ = handle.emit("accounts-changed", ());
    Ok(())
}

// ---------- Browser ----------

#[tauri::command]
async fn browser_status() -> Value {
    serde_json::json!({ "installed": tools::browser::trova_browser().map(|p| p.to_string_lossy().into_owned()), "running": tools::browser::sessione().attivo().await })
}

#[tauri::command]
async fn browser_close() {
    tools::browser::sessione().chiudi().await;
}

/// Cancella cookie e accessi del profilo del browser di Oltremare.
#[tauri::command]
async fn browser_reset_profile(app: State<'_, App>) -> Result<()> {
    tools::browser::sessione().chiudi().await;
    let p = app.data_dir.join("browser");
    if p.exists() {
        std::fs::remove_dir_all(p)?;
    }
    Ok(())
}

// ---------- Permessi ----------

#[tauri::command]
fn permissions_status() -> permessi::Stato {
    permessi::stato()
}

#[tauri::command]
fn permissions_request(handle: AppHandle, kind: String) {
    use tauri_plugin_opener::OpenerExt;
    if let Some(url) = permessi::richiedi(&kind) {
        let _ = handle.opener().open_url(url, None::<&str>);
    }
}

// ---------- Finestre ----------

#[tauri::command]
fn companion_set(handle: AppHandle, visible: bool) -> Result<()> {
    imposta_compagno(&handle, visible)
}

pub fn imposta_compagno(handle: &AppHandle, visible: bool) -> Result<()> {
    let app = handle.state::<App>();
    applica_compagno(handle, visible, &app.store.settings().companion_style);
    let mut s = app.store.settings();
    s.companion = visible;
    app.store.save_settings(&s)?;
    let _ = handle.emit("settings-changed", settings_state(&app));
    finestre::aggiorna_menu(handle);
    Ok(())
}

#[tauri::command]
fn main_show(handle: AppHandle) {
    mostra_principale(&handle);
}

pub fn mostra_principale(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Mostra il compagno nello stile scelto: avatar flottante o isola in alto.
pub fn applica_compagno(handle: &AppHandle, visible: bool, stile: &str) {
    let isola = stile == "island";
    if let Some(w) = handle.get_webview_window("companion") {
        if visible && !isola {
            let _ = w.show();
        } else {
            let _ = w.hide();
        }
    }
    isola::mostra(handle, visible && isola);
}

#[tauri::command]
fn island_info(handle: AppHandle) -> isola::Notch {
    isola::notch(&handle)
}

#[tauri::command]
fn island_resize(handle: AppHandle, width: f64, height: f64) {
    isola::dimensiona(&handle, width.clamp(60.0, 900.0), height.clamp(20.0, 600.0));
}

// ---------- Voce ----------

#[tauri::command]
fn voice_status(handle: AppHandle, app: State<'_, App>) -> Value {
    app.voce.stato(&handle)
}

#[tauri::command]
fn voice_devices() -> Vec<String> {
    voce::cattura::dispositivi()
}

#[tauri::command]
fn voice_listen_set(handle: AppHandle, app: State<'_, App>, enabled: bool) -> Result<()> {
    let mut s = app.store.settings();
    if enabled {
        app.voce.avvia_ascolto(&handle, true).map_err(AppError::msg)?;
    } else {
        app.voce.ferma_ascolto();
    }
    s.voice_listen = enabled;
    app.store.save_settings(&s)?;
    let _ = handle.emit("settings-changed", settings_state(&app));
    finestre::aggiorna_menu(&handle);
    Ok(())
}

#[tauri::command]
fn voice_command(handle: AppHandle, app: State<'_, App>) -> Result<()> {
    app.voce.comando(&handle).map_err(AppError::msg)
}

#[tauri::command]
fn voice_cancel(app: State<'_, App>) {
    app.voce.annulla();
}

#[tauri::command]
fn voice_ptt(handle: AppHandle, app: State<'_, App>, active: bool) -> Result<()> {
    app.voce.ptt(&handle, active).map_err(AppError::msg)
}

#[tauri::command]
fn voice_speak(handle: AppHandle, app: State<'_, App>, text: String) {
    app.voce.parla(&handle, &text);
}

#[tauri::command]
fn voice_stop(app: State<'_, App>) {
    app.voce.ferma_lettura();
}

/// Cambia la scorciatoia per parlare (la vecchia si libera).
#[tauri::command]
fn voice_hotkey_set(handle: AppHandle, app: State<'_, App>, hotkey: String) -> Result<()> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let mut s = app.store.settings();
    if !s.voice_hotkey.is_empty() {
        let _ = handle.global_shortcut().unregister(s.voice_hotkey.as_str());
    }
    if !hotkey.is_empty() {
        if let Some(motivo) = finestre::controlla(&handle, &hotkey, None).filter(|m| !m.starts_with("È già il tasto per parlare")) {
            return Err(AppError::msg(motivo));
        }
        handle.global_shortcut().register(hotkey.as_str()).map_err(|e| AppError::msg(format!("Scorciatoia non disponibile: {e}")))?;
    }
    s.voice_hotkey = hotkey;
    app.store.save_settings(&s)?;
    finestre::registra_scorciatoie(&handle);
    let _ = handle.emit("settings-changed", settings_state(&app));
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Una sola istanza: aprirla di nuovo porta in primo piano quella attiva
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| mostra_principale(app)))
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--background"])))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    use tauri_plugin_global_shortcut::{Shortcut, ShortcutState};
                    let premuto = event.state() == ShortcutState::Pressed;
                    if KILL_SWITCH.parse::<Shortcut>().is_ok_and(|k| &k == shortcut) {
                        if premuto {
                            ferma_tutto(app);
                        }
                        return;
                    }
                    if finestre::scorciatoia(app, shortcut, premuto) {
                        return;
                    }
                    let st = app.state::<App>();
                    let ptt = st.store.settings().voice_hotkey;
                    if ptt.parse::<Shortcut>().is_ok_and(|k| &k == shortcut) {
                        if let Err(e) = st.voce.ptt(app, premuto) {
                            let _ = app.emit("voice-error", e);
                        }
                    }
                })
                .build(),
        )
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let data_dir = app.path().app_data_dir()?;
            let db = db::Db::open(&data_dir.join("oltremare.db"))?;
            let mcp = mcp::Mcp::avvia(app.handle().clone())?;
            app.manage(App {
                store: Store::new(config_dir),
                data_dir,
                db,
                runs: Runs::default(),
                avatar: avatar::Avatar::default(),
                approvazioni: Default::default(),
                mcp,
                sidecar: Default::default(),
                voce: Default::default(),
                oauth_attesa: Default::default(),
                osservatori: Default::default(),
                mcp_clienti: Default::default(),
                download: Default::default(),
            });
            let _ = APP.set(app.handle().clone());
            app.state::<App>().voce.init(app.handle());
            notifiche::avvia(app.handle().clone());
            triggers::avvia(app.handle().clone());
            triggers::ricarica_cartelle(app.handle());
            {
                let h = app.handle().clone();
                tauri::async_runtime::spawn(async move { mcp_client::ricarica(&h).await });
            }
            {
                let s = app.state::<App>().store.settings();
                if s.companion {
                    applica_compagno(app.handle(), true, &s.companion_style);
                }
            }
            // Scorciatoie globali, icona nella barra dei menu, finestra principale (non all'avvio automatico)
            finestre::registra_scorciatoie(app.handle());
            if let Err(e) = finestre::crea_tray(app.handle()) {
                eprintln!("icona nella barra dei menu non disponibile: {e}");
            }
            if !std::env::args().any(|a| a == "--background") {
                mostra_principale(app.handle());
            }
            Ok(())
        })
        .on_window_event(|w, e| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = e {
                if w.label() == "main" {
                    api.prevent_close();
                    let _ = w.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            settings_get,
            settings_save,
            provider_key_set,
            provider_models,
            provider_test,
            prefs_get,
            prefs_set,
            agents_list,
            agent_save,
            agent_delete,
            agent_duplicate,
            run_start,
            run_cancel,
            runs_cancel_all,
            runs_active,
            run_get,
            runs_list,
            conversations_list,
            conversation_runs,
            conversation_delete,
            avatar_state,
            claude_code_status,
            attachment_types,
            tool_catalog,
            approval_answer,
            approvals_pending,
            stop_everything,
            audit_list,
            storage_purge,
            llama_status,
            llama_start,
            llama_stop,
            llama_log,
            models_catalog,
            model_download,
            model_download_cancel,
            model_delete,
            model_import,
            hw_info,
            models_folder,
            voice_status,
            voice_devices,
            voice_listen_set,
            voice_command,
            voice_cancel,
            voice_ptt,
            voice_speak,
            voice_stop,
            voice_hotkey_set,
            companion_set,
            island_info,
            island_resize,
            permissions_status,
            browser_status,
            accounts_list,
            mcp_status,
            dev::dev_projects,
            studio::studio_projects,
            studio::studio_open,
            studio::studio_classify,
            studio::studio_claudex,
            studio::studio_run,
            studio::studio_plan,
            studio::studio_approve,
            studio::studio_second_opinion,
            studio::studio_save,
            studio::studio_link,
            studio::studio_link_set,
            studio::studio_link_suggest,
            studio::studio_issues,
            studio::studio_issue,
            studio::studio_commands,
            git::git_state,
            git::git_diff,
            git::git_stage,
            git::git_unstage,
            git::git_discard,
            git::git_commit,
            git::git_sync,
            git::git_branches,
            git::git_checkout,
            git::git_stash,
            git::git_log,
            git::git_suggest_message,
            studio::studio_launch,
            studio::studio_stop,
            dev::dev_open,
            dev::dev_tree,
            dev::dev_file,
            dev::dev_git,
            dev::dev_diff,
            dev::dev_session,
            dev::dev_processes,
            dev::dev_send,
            dev::dev_stop,
            mcp_reload,
            mcp_secret_set,
            mcp_secret_has,
            mcp_import_claude,
            editors_list,
            notifications_list,
            notifications_mark_read,
            notifications_sources,
            slack_token_set,
            briefing_now,
            briefing_latest,
            cron_preview,
            usage_stats,
            hotkey_check,
            palette_toggle,
            palette_hide,
            palette_open_run,
            autostart_set,
            oauth_available,
            oauth_connect,
            oauth_cancel,
            account_remove,
            browser_close,
            browser_reset_profile,
            permissions_request,
            main_show,
        ])
        .build(tauri::generate_context!())
        .expect("errore all'avvio di Oltremare")
        .run(|app, event| {
            // Clic sull'icona nel Dock con la finestra nascosta
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { has_visible_windows: false, .. } = event {
                mostra_principale(app);
            }
            if let tauri::RunEvent::Exit = event {
                // Niente processi orfani: il motore locale si chiude con l'app
                if let Some(st) = app.try_state::<App>() {
                    st.sidecar.ferma_subito();
                    tools::processi::ferma_tutti();
                    tools::sviluppo::ferma_tutti_headless();
                    st.runs.annulla_tutte();
                }
            }
        });
}

#[cfg(test)]
mod tests {
    /// Regressione: i comandi sincroni girano sul thread principale, fuori dal runtime.
    /// Lì tokio::spawn va in panic; il runtime di Tauri invece funziona.
    #[test]
    fn spawning_from_a_plain_thread_does_not_panic() {
        let outcome = std::thread::spawn(|| {
            let (tx, rx) = std::sync::mpsc::channel();
            tauri::async_runtime::spawn(async move {
                tx.send(42).unwrap();
            });
            rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap()
        })
        .join();
        assert_eq!(outcome.unwrap(), 42);
    }
}

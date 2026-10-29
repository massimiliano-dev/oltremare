//! Accesso OAuth2 a Microsoft (Graph) e Google, da app desktop: PKCE + redirect su loopback
//! (127.0.0.1, porta casuale), come raccomandato da RFC 8252. I token stanno nel portachiavi.

use crate::error::{AppError, Result};
use crate::store::{self, now_ms, Account, Settings};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

// ID inclusi nella build (src-tauri/.cargo/config.toml); le impostazioni possono sostituirli
const MS_CLIENT_ID: Option<&str> = option_env!("OLTREMARE_MS_CLIENT_ID");
const GOOGLE_CLIENT_ID: Option<&str> = option_env!("OLTREMARE_GOOGLE_CLIENT_ID");
const GOOGLE_CLIENT_SECRET: Option<&str> = option_env!("OLTREMARE_GOOGLE_CLIENT_SECRET");

pub const MS_SCOPE: &str = "https://graph.microsoft.com/Mail.ReadWrite https://graph.microsoft.com/Mail.Send https://graph.microsoft.com/Calendars.ReadWrite https://graph.microsoft.com/Contacts.Read https://graph.microsoft.com/Tasks.ReadWrite https://graph.microsoft.com/User.Read offline_access openid email profile";
/// Teams si collega a parte: le chat esistono solo negli account di lavoro o scuola
pub const TEAMS_SCOPE: &str = "https://graph.microsoft.com/Chat.Read https://graph.microsoft.com/User.Read offline_access openid email profile";
pub const GOOGLE_SCOPE: &str = "https://www.googleapis.com/auth/gmail.modify https://www.googleapis.com/auth/gmail.send https://www.googleapis.com/auth/calendar https://www.googleapis.com/auth/tasks https://www.googleapis.com/auth/contacts.readonly openid email profile";

struct Provider {
    id: &'static str,
    auth_url: &'static str,
    token_url: &'static str,
}

const MICROSOFT: Provider = Provider {
    id: "microsoft",
    auth_url: "https://login.microsoftonline.com/common/oauth2/v2.0/authorize",
    token_url: "https://login.microsoftonline.com/common/oauth2/v2.0/token",
};
const GOOGLE: Provider = Provider { id: "google", auth_url: "https://accounts.google.com/o/oauth2/v2/auth", token_url: "https://oauth2.googleapis.com/token" };

fn provider(id: &str) -> Result<&'static Provider> {
    match id {
        "microsoft" => Ok(&MICROSOFT),
        "google" => Ok(&GOOGLE),
        _ => Err(AppError::msg("Provider sconosciuto")),
    }
}

fn pieno(v: &str) -> Option<String> {
    let t = v.trim();
    (!t.is_empty()).then(|| t.to_string())
}

/// (client id, client secret) per il provider: impostazioni, poi build.
fn credenziali(p: &str, s: &Settings) -> Option<(String, Option<String>)> {
    match p {
        "microsoft" => pieno(&s.ms_client_id).or_else(|| MS_CLIENT_ID.and_then(pieno)).map(|id| (id, None)),
        "google" => pieno(&s.google_client_id)
            .or_else(|| GOOGLE_CLIENT_ID.and_then(pieno))
            .map(|id| (id, pieno(&s.google_client_secret).or_else(|| GOOGLE_CLIENT_SECRET.and_then(pieno)))),
        _ => None,
    }
}

pub fn disponibili(s: &Settings) -> Vec<&'static str> {
    ["microsoft", "google"].into_iter().filter(|p| credenziali(p, s).is_some()).collect()
}

#[derive(Serialize, Deserialize)]
struct Tokens {
    access_token: String,
    refresh_token: String,
    expires_at: u64,
    scope: String,
    /// L'applicazione che ha emesso i token: serve la stessa per rinnovarli (anche da una build senza ID)
    #[serde(default)]
    client_id: String,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn random_token() -> String {
    format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
}

fn url_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn url_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => match u8::from_str_radix(&s[i + 1..i + 3], 16) {
                Ok(v) => {
                    out.push(v);
                    i += 2;
                }
                Err(_) => out.push(b'%'),
            },
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn query_param(q: &str, k: &str) -> Option<String> {
    q.split('&').find_map(|p| {
        let (a, b) = p.split_once('=')?;
        (a == k).then(|| url_decode(b))
    })
}

const PAGINA: &str = r#"<!doctype html><html lang="it"><meta charset="utf-8"><title>Oltremare</title>
<body style="margin:0;height:100vh;display:grid;place-items:center;background:#2a1f8f;color:#f4f3ff;font-family:-apple-system,'Segoe UI',sans-serif">
<div style="text-align:center"><div style="font-family:Georgia,serif;font-size:42px;margin-bottom:8px">TITOLO</div>
<div style="opacity:.8">TESTO</div></div></body></html>"#;

async fn attendi_redirect(listener: TcpListener) -> Result<String> {
    loop {
        let (mut sock, _) = listener.accept().await?;
        let mut buf = vec![0u8; 8192];
        let n = sock.read(&mut buf).await?;
        let req = String::from_utf8_lossy(&buf[..n]);
        let path = req.split_whitespace().nth(1).unwrap_or("");
        let Some(query) = path.strip_prefix("/?") else {
            let _ = sock.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n").await;
            continue;
        };
        let ok = query_param(query, "code").is_some();
        let pagina = if ok {
            PAGINA.replace("TITOLO", "Fatto.").replace("TESTO", "Puoi chiudere questa scheda e tornare a Oltremare.")
        } else {
            PAGINA.replace("TITOLO", "Accesso non completato").replace("TESTO", "Torna a Oltremare e riprova.")
        };
        let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{pagina}", pagina.len());
        let _ = sock.write_all(resp.as_bytes()).await;
        let _ = sock.shutdown().await;
        return Ok(query.to_string());
    }
}

async fn token_request(token_url: &str, id: &str, secret: Option<&str>, form: &[(&str, &str)]) -> Result<Value> {
    let mut params: Vec<(&str, &str)> = form.to_vec();
    params.push(("client_id", id));
    if let Some(s) = secret {
        params.push(("client_secret", s));
    }
    let body = params.iter().map(|(k, v)| format!("{}={}", url_encode(k), url_encode(v))).collect::<Vec<_>>().join("&");
    let resp = crate::providers::client().post(token_url).header("Content-Type", "application/x-www-form-urlencoded").body(body).send().await?;
    let status = resp.status();
    let v: Value = resp.json().await?;
    if !status.is_success() {
        let msg = v["error_description"].as_str().or_else(|| v["error"].as_str()).unwrap_or("errore sconosciuto");
        return Err(AppError::msg(format!("Accesso rifiutato: {}", msg.lines().next().unwrap_or(msg))));
    }
    Ok(v)
}

fn identita(v: &Value) -> (String, String) {
    let claims: Value = v["id_token"]
        .as_str()
        .and_then(|t| t.split('.').nth(1))
        .and_then(|p| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(p.trim_end_matches('=')).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(Value::Null);
    let email = claims["email"].as_str().or_else(|| claims["preferred_username"].as_str()).unwrap_or_default().to_string();
    (email, claims["name"].as_str().unwrap_or_default().to_string())
}

fn chiave(tag: &str, account_id: &str) -> String {
    if tag == "teams" { format!("oauth-teams:{account_id}") } else { format!("oauth:{account_id}") }
}

fn salva_token(tag: &str, account_id: &str, v: &Value, scope: &str, refresh_prec: Option<&str>, client_id: &str) -> Result<String> {
    let access = v["access_token"].as_str().ok_or_else(|| AppError::msg("Risposta OAuth senza access token"))?.to_string();
    let refresh = v["refresh_token"].as_str().or(refresh_prec).ok_or_else(|| AppError::msg("Risposta OAuth senza refresh token"))?.to_string();
    let t = Tokens { access_token: access.clone(), refresh_token: refresh, expires_at: now() + v["expires_in"].as_u64().unwrap_or(3600), scope: scope.into(), client_id: client_id.into() };
    store::set_secret(&chiave(tag, account_id), &serde_json::to_string(&t)?)?;
    Ok(access)
}

pub const ANNULLATO: &str = "Accesso annullato";

/// Apre il browser per il consenso. `tag`: "" per l'account, "teams" per le chat di Teams.
pub async fn autorizza(
    provider_id: &str,
    tag: &str,
    s: &Settings,
    apri: impl FnOnce(&str) -> Result<()>,
    annulla: tokio::sync::oneshot::Receiver<()>,
    account_esistente: Option<&str>,
) -> Result<Account> {
    let p = provider(provider_id)?;
    let (id, secret) = credenziali(provider_id, s)
        .ok_or_else(|| AppError::msg("Accesso non configurato: inserisci l'ID dell'applicazione in Impostazioni → Account"))?;
    let scope = match (provider_id, tag) {
        ("microsoft", "teams") => TEAMS_SCOPE,
        ("microsoft", _) => MS_SCOPE,
        _ => GOOGLE_SCOPE,
    };
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let redirect = format!("http://localhost:{}", listener.local_addr()?.port());
    let verifier = random_token();
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let state = random_token();
    let mut url = format!(
        "{}?client_id={}&response_type=code&redirect_uri={}&scope={}&state={state}&code_challenge={challenge}&code_challenge_method=S256",
        p.auth_url,
        url_encode(&id),
        url_encode(&redirect),
        url_encode(scope)
    );
    url.push_str(if p.id == "google" { "&access_type=offline&prompt=consent" } else { "&prompt=select_account" });
    apri(&url)?;
    let query = tokio::select! {
        r = attendi_redirect(listener) => r?,
        _ = annulla => return Err(AppError::msg(ANNULLATO)),
        _ = tokio::time::sleep(Duration::from_secs(300)) => return Err(AppError::msg("Tempo scaduto: l'accesso nel browser non è stato completato")),
    };
    if query_param(&query, "state").as_deref() != Some(state.as_str()) {
        return Err(AppError::msg("Risposta OAuth non valida (state)"));
    }
    if let Some(err) = query_param(&query, "error_description").or_else(|| query_param(&query, "error")) {
        return Err(AppError::msg(format!("{ANNULLATO}: {err}")));
    }
    let code = query_param(&query, "code").ok_or_else(|| AppError::msg("Nessun codice di autorizzazione"))?;
    let v = token_request(
        p.token_url,
        &id,
        secret.as_deref(),
        &[("grant_type", "authorization_code"), ("code", &code), ("redirect_uri", &redirect), ("code_verifier", &verifier)],
    )
    .await?;
    let (email, nome) = identita(&v);
    if email.is_empty() {
        return Err(AppError::msg("Il provider non ha restituito l'indirizzo email"));
    }
    let account_id = account_esistente.map(str::to_string).unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    salva_token(tag, &account_id, &v, scope, None, &id)?;
    Ok(Account {
        id: account_id,
        provider: p.id.into(),
        name: if nome.is_empty() { email.split('@').next().unwrap_or_default().into() } else { nome },
        email,
        created_at: now_ms(),
        teams: tag == "teams",
    })
}

static RINNOVO: Mutex<()> = Mutex::const_new(());

/// Access token valido per l'account (rinnovato se in scadenza).
pub async fn token(acc: &Account, tag: &str, s: &Settings) -> Result<String> {
    let p = provider(&acc.provider)?;
    let _g = RINNOVO.lock().await;
    let raw = store::get_secret(&chiave(tag, &acc.id)).map_err(|_| AppError::msg(format!("Accesso a {} scaduto: ricollega l'account", acc.email)))?;
    let t: Tokens = serde_json::from_str(&raw)?;
    if t.expires_at > now() + 120 {
        return Ok(t.access_token);
    }
    // Si rinnova con l'applicazione che ha emesso i token; per Google serve anche il secret configurato
    let (id, secret) = match credenziali(&acc.provider, s) {
        Some((id, secret)) if t.client_id.is_empty() || t.client_id == id => (id, secret),
        Some((_, secret)) => (t.client_id.clone(), secret),
        None if !t.client_id.is_empty() && acc.provider == "microsoft" => (t.client_id.clone(), None),
        None => return Err(AppError::msg("manca l'ID dell'applicazione per rinnovare l'accesso (Impostazioni → Account → Applicazioni OAuth)")),
    };
    // Dopo il risveglio la rete può non essere ancora pronta: si riprova prima di arrendersi
    let form = [("grant_type", "refresh_token"), ("refresh_token", t.refresh_token.as_str()), ("scope", t.scope.as_str())];
    let mut tentativo = 0;
    let v = loop {
        match token_request(p.token_url, &id, secret.as_deref(), &form).await {
            Ok(v) => break v,
            Err(e) if e.e_rete() && tentativo < 3 => {
                tentativo += 1;
                tokio::time::sleep(std::time::Duration::from_secs([3, 6, 12][tentativo - 1])).await;
            }
            // Rete: l'accesso è ancora valido, non serve ricollegare
            Err(e) if e.e_rete() => return Err(e),
            Err(e) => return Err(AppError::msg(format!("{e} — ricollega {} in Impostazioni → Account", acc.email))),
        }
    };
    salva_token(tag, &acc.id, &v, &t.scope, Some(&t.refresh_token), &id)
}

#[cfg(test)]
mod tests {
    #[test]
    fn query_parsing() {
        assert_eq!(super::query_param("code=a%2Fb&state=x+y", "code").as_deref(), Some("a/b"));
        assert_eq!(super::query_param("code=a&state=x+y", "state").as_deref(), Some("x y"));
        assert_eq!(super::url_encode("a b/c"), "a%20b%2Fc");
    }
}

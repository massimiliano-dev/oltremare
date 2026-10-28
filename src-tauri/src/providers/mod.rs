//! Provider dei modelli: Claude (Anthropic) e qualsiasi endpoint compatibile OpenAI
//! (OpenAI stesso e, più avanti, llama-server locale), più Claude Code con l'abbonamento dell'utente.

pub mod anthropic;
pub mod claude_code;
pub mod openai_compat;
pub mod sse;

use crate::agent::messaggi::{Blocco, Messaggio};
use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

pub enum Delta {
    Testo(String),
    Pensiero(String),
}

pub struct Richiesta<'a> {
    pub system: &'a str,
    pub messaggi: &'a [Messaggio],
    pub modello: &'a str,
    pub effort: &'a str,
    pub max_tokens: u32,
    pub chiave: &'a str,
    pub base_url: &'a str,
    /// Definizioni degli strumenti già nel formato del provider
    pub tools: &'a [Value],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stop {
    Fine,
    ToolUse,
    MaxTokens,
    Rifiuto,
    Pausa,
    Altro(String),
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

impl Usage {
    pub fn add(&mut self, o: &Usage) {
        self.input_tokens += o.input_tokens;
        self.output_tokens += o.output_tokens;
        self.cache_read += o.cache_read;
        self.cache_write += o.cache_write;
    }
}

pub struct Risposta {
    pub blocchi: Vec<Blocco>,
    pub raw: Value,
    pub stop: Stop,
    pub stop_details: Option<Value>,
    pub usage: Usage,
    pub modello: String,
}

pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap_or_default()
}

pub async fn stream(provider: &str, req: Richiesta<'_>, on: &mut (dyn FnMut(Delta) + Send), cancel: &CancellationToken) -> Result<Risposta> {
    match provider {
        "anthropic" => anthropic::stream(req, on, cancel).await,
        "openai" => openai_compat::stream(req, openai_compat::Sapore::OpenAi, on, cancel).await,
        "local" => openai_compat::stream(req, openai_compat::Sapore::Llama, on, cancel).await,
        _ => Err(AppError::msg("Provider sconosciuto")),
    }
}

pub async fn models(provider: &str, key: &str, base_url: &str) -> Result<Vec<String>> {
    match provider {
        "anthropic" => anthropic::models(key).await,
        "openai" => openai_compat::models(key, base_url).await,
        _ => Ok(vec![]),
    }
}

/// Messaggio d'errore leggibile da una risposta HTTP non riuscita.
pub async fn api_error(resp: reqwest::Response) -> AppError {
    let status = resp.status();
    let body: Value = resp.json().await.unwrap_or(Value::Null);
    let detail = body["error"]["message"].as_str().unwrap_or("errore sconosciuto").to_string();
    let hint = match status.as_u16() {
        401 | 403 => " — controlla la chiave API nelle impostazioni",
        404 => " — il modello scelto non esiste o non è abilitato",
        429 => " — limite di richieste raggiunto, riprova tra poco",
        529 => " — il servizio è sovraccarico, riprova tra poco",
        _ => "",
    };
    AppError::msg(format!("{detail}{hint}"))
}

/// Prossimo pezzo dello stream, oppure `Annullato` se l'utente ferma la risposta.
pub async fn prossimo<S>(stream: &mut S, cancel: &CancellationToken) -> Result<Option<bytes::Bytes>>
where
    S: futures::Stream<Item = std::result::Result<bytes::Bytes, reqwest::Error>> + Unpin,
{
    use futures::StreamExt;
    tokio::select! {
        _ = cancel.cancelled() => Err(AppError::Annullato),
        chunk = stream.next() => Ok(chunk.transpose()?),
    }
}

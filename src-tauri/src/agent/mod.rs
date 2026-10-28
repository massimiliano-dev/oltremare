//! Agenti: esecuzioni in corso, passi mostrati nella timeline, avvio del ciclo.

pub mod approvazioni;
pub mod ciclo;
pub mod costi;
pub mod messaggi;
pub mod policy;
pub mod strumenti;
pub mod timeline;

use crate::error::{AppError, Result};
use crate::providers::Usage;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;
use tokio_util::sync::CancellationToken;

/// Un passo della timeline, nella forma che legge il frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Passo {
    pub id: String,
    /// "user" | "text" | "thinking" | "tool" | "notice"
    pub kind: String,
    #[serde(default)]
    pub text: String,
    pub at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via_voice: Option<bool>,
    /// Allegati del messaggio dell'utente: nome, dimensione, tipo
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Passo {
    pub fn nuovo(kind: &str, text: &str, at: i64) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            kind: kind.into(),
            text: text.into(),
            at,
            ended_at: None,
            via_voice: None,
            attachments: None,
            tool: None,
            category: None,
            title: None,
            input: None,
            status: None,
            result: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunView {
    pub id: String,
    pub agent_id: String,
    pub conversation_id: String,
    pub title: String,
    /// Da dove è partita: "main" | "palette" | "voice" | "trigger"
    pub trigger: String,
    /// "running" | "awaitingApproval" | "done" | "error" | "cancelled" | "refused"
    pub status: String,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub error: Option<String>,
    pub steps: Vec<Passo>,
    pub usage: Option<Usage>,
    pub cost_estimate: Option<f64>,
    pub served_model: Option<String>,
}

/// Esecuzioni in corso: si fermano con il loro token.
#[derive(Default)]
pub struct Runs(Mutex<HashMap<String, (String, CancellationToken)>>);

impl Runs {
    /// Una conversazione risponde a un messaggio alla volta.
    pub fn registra(&self, run_id: &str, conversazione: &str, token: CancellationToken) -> Result<()> {
        let mut m = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if m.values().any(|(c, _)| c == conversazione) {
            return Err(AppError::msg("Sto ancora lavorando alla richiesta precedente: aspetta o fermala"));
        }
        m.insert(run_id.to_string(), (conversazione.to_string(), token));
        Ok(())
    }

    pub fn rimuovi(&self, run_id: &str) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).remove(run_id);
    }

    pub fn annulla(&self, run_id: &str) {
        if let Some((_, t)) = self.0.lock().unwrap_or_else(|e| e.into_inner()).get(run_id) {
            t.cancel();
        }
    }

    pub fn annulla_tutte(&self) {
        for (_, t) in self.0.lock().unwrap_or_else(|e| e.into_inner()).values() {
            t.cancel();
        }
    }

    pub fn attive(&self) -> Vec<String> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).keys().cloned().collect()
    }
}

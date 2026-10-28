//! Modello canonico della conversazione, indipendente dal provider.
//!
//! Le risposte dell'assistente conservano anche il contenuto grezzo del provider (`raw`):
//! con Claude va rimandato identico (blocchi di ragionamento, firme, fallback),
//! perché la cronologia deve crescere solo in coda, senza modifiche ai turni passati.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ruolo {
    Utente,
    Assistente,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum Blocco {
    Testo { testo: String },
    Pensiero { testo: String },
    ToolUse {
        id: String,
        nome: String,
        input: Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        toolset: Option<String>,
    },
    /// File allegato dall'utente (copia nella cartella dati dell'app)
    Allegato { nome: String, mime: String, formato: String, percorso: String, byte: u64 },
    ToolResult {
        tool_use_id: String,
        contenuto: String,
        is_error: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        toolset: Option<String>,
        /// Immagine del risultato: percorso di un PNG salvato, oppure "data:" con il base64
        #[serde(default, skip_serializing_if = "Option::is_none")]
        immagine: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Messaggio {
    pub ruolo: Ruolo,
    pub blocchi: Vec<Blocco>,
    /// Contenuto grezzo dell'assistente, da rimandare tale e quale allo stesso provider
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_provider: Option<String>,
}

impl Messaggio {
    pub fn utente(testo: &str) -> Self {
        Self { ruolo: Ruolo::Utente, blocchi: vec![Blocco::Testo { testo: testo.to_string() }], raw: None, raw_provider: None }
    }
}

/// Aggiunge il messaggio dell'utente (allegati e testo) in coda. Se l'ultimo turno è già
/// dell'utente (risposta precedente annullata), si accoda a quello: due turni utente di fila non servono.
pub fn accoda_utente(messaggi: &mut Vec<Messaggio>, blocchi: Vec<Blocco>) {
    match messaggi.last_mut() {
        Some(m) if m.ruolo == Ruolo::Utente && m.blocchi.iter().all(|b| matches!(b, Blocco::Testo { .. } | Blocco::Allegato { .. })) => {
            m.blocchi.extend(blocchi);
        }
        _ => messaggi.push(Messaggio { ruolo: Ruolo::Utente, blocchi, raw: None, raw_provider: None }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_turn_after_cancelled_reply_is_merged() {
        let mut m = vec![];
        accoda_utente(&mut m, vec![Blocco::Testo { testo: "ciao".into() }]);
        accoda_utente(&mut m, vec![Blocco::Testo { testo: "ci sei?".into() }]);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].blocchi.len(), 2);
    }

    #[test]
    fn raw_content_survives_a_round_trip() {
        let raw = serde_json::json!([{"type":"thinking","thinking":"","signature":"abc=="},{"type":"text","text":"Ciao"}]);
        let m = Messaggio {
            ruolo: Ruolo::Assistente,
            blocchi: vec![Blocco::Testo { testo: "Ciao".into() }],
            raw: Some(raw.clone()),
            raw_provider: Some("anthropic".into()),
        };
        let back: Messaggio = serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
        assert_eq!(back.raw.unwrap(), raw);
    }
}

/// Il PNG di un risultato in base64 (dal file salvato o dal dato in linea).
pub fn immagine_base64(rif: &str) -> Option<String> {
    use base64::Engine;
    if let Some(d) = rif.strip_prefix("data:") {
        return Some(d.to_string());
    }
    std::fs::read(rif).ok().map(|b| base64::engine::general_purpose::STANDARD.encode(b))
}

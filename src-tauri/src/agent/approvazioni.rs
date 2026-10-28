//! Richieste di approvazione in attesa della risposta dell'utente.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::oneshot;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Risposta {
    Consenti,
    ConsentiSessione,
    Nega(Option<String>),
}

#[derive(Default)]
pub struct Approvazioni(Mutex<HashMap<String, (String, Value, oneshot::Sender<Risposta>)>>);

impl Approvazioni {
    fn m(&self) -> std::sync::MutexGuard<'_, HashMap<String, (String, Value, oneshot::Sender<Risposta>)>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn registra(&self, id: &str, run_id: &str, richiesta: Value) -> oneshot::Receiver<Risposta> {
        let (tx, rx) = oneshot::channel();
        self.m().insert(id.to_string(), (run_id.to_string(), richiesta, tx));
        rx
    }

    pub fn rispondi(&self, id: &str, r: Risposta) -> bool {
        match self.m().remove(id) {
            Some((_, _, tx)) => tx.send(r).is_ok(),
            None => false,
        }
    }

    pub fn rimuovi(&self, id: &str) {
        self.m().remove(id);
    }

    pub fn pendenti(&self) -> Vec<Value> {
        self.m().values().map(|(_, v, _)| v.clone()).collect()
    }

    /// Nega tutto (interruttore di emergenza).
    pub fn nega_tutte(&self) {
        let tutte: Vec<_> = self.m().drain().collect();
        for (_, (_, _, tx)) in tutte {
            let _ = tx.send(Risposta::Nega(Some("fermato con l'interruttore di emergenza".into())));
        }
    }
}

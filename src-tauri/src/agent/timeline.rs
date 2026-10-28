//! La timeline di un'esecuzione: passi salvati nell'archivio e inviati alla finestra.
//! Condivisa tra il ciclo (testo in streaming) e gli strumenti (anche quelli chiamati da Claude Code).

use super::Passo;
use crate::avatar::Stato as Volto;
use crate::store::now_ms;
use crate::App;
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct EvDelta<'a> {
    run_id: &'a str,
    step_id: &'a str,
    kind: &'a str,
    delta: &'a str,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct EvStep<'a> {
    run_id: &'a str,
    step: &'a Passo,
}

struct Interno {
    indice: usize,
    corrente: Option<Passo>,
}

pub struct Timeline {
    app: AppHandle,
    pub run_id: String,
    inner: Mutex<Interno>,
}

impl Timeline {
    pub fn new(app: AppHandle, run_id: &str, indice: usize) -> Self {
        Self { app, run_id: run_id.to_string(), inner: Mutex::new(Interno { indice, corrente: None }) }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Interno> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn salva(&self, indice: usize, p: &Passo) {
        let _ = self.app.state::<App>().db.salva_passo(&self.run_id, indice, p);
        let _ = self.app.emit("run-step", EvStep { run_id: &self.run_id, step: p });
    }

    pub fn delta(&self, kind: &str, t: &str) {
        if t.is_empty() {
            return;
        }
        let mut g = self.lock();
        if g.corrente.as_ref().is_none_or(|p| p.kind != kind) {
            if let Some(mut p) = g.corrente.take() {
                p.ended_at = Some(now_ms());
                let i = g.indice;
                g.indice += 1;
                self.salva(i, &p);
            }
            g.corrente = Some(Passo::nuovo(kind, "", now_ms()));
            let volto = if kind == "text" { Volto::Speaking } else { Volto::Thinking };
            self.app.state::<App>().avatar.set(&self.app, &self.run_id, volto);
        }
        let p = g.corrente.as_mut().expect("passo corrente");
        p.text.push_str(t);
        let _ = self.app.emit("run-delta", EvDelta { run_id: &self.run_id, step_id: &p.id, kind, delta: t });
    }

    pub fn chiudi(&self) {
        let mut g = self.lock();
        if let Some(mut p) = g.corrente.take() {
            p.ended_at = Some(now_ms());
            let i = g.indice;
            g.indice += 1;
            self.salva(i, &p);
        }
    }

    /// Nuovo passo (strumento, nota): chiude il testo in corso e restituisce la posizione.
    pub fn aggiungi(&self, p: &Passo) -> usize {
        let mut g = self.lock();
        if let Some(mut c) = g.corrente.take() {
            c.ended_at = Some(now_ms());
            let i = g.indice;
            g.indice += 1;
            self.salva(i, &c);
        }
        let i = g.indice;
        g.indice += 1;
        drop(g);
        self.salva(i, p);
        i
    }

    pub fn aggiorna(&self, indice: usize, p: &Passo) {
        self.salva(indice, p);
    }

    pub fn nota(&self, testo: &str) {
        let mut p = Passo::nuovo("notice", testo, now_ms());
        p.ended_at = Some(p.at);
        self.aggiungi(&p);
    }

    /// Avviso di stato che non resta nella cronologia (es. "avvio l'AI locale").
    pub fn nota_temporanea(&self, testo: &str) {
        let _ = self.app.emit("run-progress", serde_json::json!({ "runId": self.run_id, "text": testo }));
    }
}

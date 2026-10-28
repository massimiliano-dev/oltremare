//! Lo stato dell'avatar: un'unica espressione per tutta l'app, scelta per priorità
//! tra quello che stanno facendo le esecuzioni (e, più avanti, voce e notifiche).

use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Stato {
    Sleep,
    Idle,
    Listening,
    Thinking,
    Speaking,
    Working,
    Asking,
    Alert,
    Error,
}

impl Stato {
    fn priorita(self) -> u8 {
        match self {
            Stato::Error => 9,
            Stato::Asking => 8,
            Stato::Alert => 7,
            Stato::Speaking => 6,
            Stato::Listening => 5,
            Stato::Thinking => 4,
            Stato::Working => 3,
            Stato::Idle => 1,
            Stato::Sleep => 0,
        }
    }
}

#[derive(Clone, Serialize)]
pub struct Evento {
    pub state: Stato,
    pub level: f32,
}

#[derive(Default)]
struct Interno {
    per_fonte: HashMap<String, Stato>,
    errore_fino: Option<Instant>,
    ultimo: Option<Stato>,
}

#[derive(Default)]
pub struct Avatar(Mutex<Interno>);

impl Avatar {
    fn calcola(i: &Interno) -> Stato {
        if i.errore_fino.is_some_and(|t| t > Instant::now()) {
            return Stato::Error;
        }
        i.per_fonte.values().copied().max_by_key(|s| s.priorita()).unwrap_or(Stato::Idle)
    }

    fn aggiorna(&self, app: &AppHandle, f: impl FnOnce(&mut Interno)) {
        let mut i = self.0.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut i);
        let s = Self::calcola(&i);
        if i.ultimo != Some(s) {
            i.ultimo = Some(s);
            let _ = app.emit("avatar-state", Evento { state: s, level: 0.0 });
            if let Some(nome) = serde_json::to_value(s).ok().and_then(|v| v.as_str().map(str::to_string)) {
                crate::finestre::stato_tray(app, &nome);
            }
        }
    }

    /// `fonte` è l'id di un'esecuzione, oppure "voce", "notifiche"…
    pub fn set(&self, app: &AppHandle, fonte: &str, s: Stato) {
        self.aggiorna(app, |i| {
            i.per_fonte.insert(fonte.to_string(), s);
        });
    }

    pub fn clear(&self, app: &AppHandle, fonte: &str) {
        self.aggiorna(app, |i| {
            i.per_fonte.remove(fonte);
        });
    }

    /// Espressione di errore per qualche secondo, poi si torna allo stato precedente.
    pub fn errore(&self, app: &AppHandle) {
        let durata = Duration::from_millis(2600);
        self.aggiorna(app, |i| i.errore_fino = Some(Instant::now() + durata));
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(durata + Duration::from_millis(50)).await;
            app.state::<crate::App>().avatar.aggiorna(&app, |_| {});
        });
    }

    /// Volume della voce (microfono o lettura): muove bocca e antenna senza cambiare stato.
    pub fn livello(&self, app: &AppHandle, livello: f32) {
        let s = Self::calcola(&self.0.lock().unwrap_or_else(|e| e.into_inner()));
        let _ = app.emit("avatar-state", Evento { state: s, level: livello });
    }

    pub fn corrente(&self) -> Stato {
        Self::calcola(&self.0.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

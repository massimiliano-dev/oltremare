//! Appunti di sistema (solo testo).

use super::{str_arg, Esito};
use serde_json::{json, Value};

pub fn esegui(nome: &str, input: &Value) -> Esito {
    let mut cb = match arboard::Clipboard::new() {
        Ok(c) => c,
        Err(e) => return Esito::errore(format!("Appunti non disponibili: {e}")),
    };
    match nome {
        "leggi_appunti" => match cb.get_text() {
            Ok(t) if !t.is_empty() => {
                let t: String = t.chars().take(100_000).collect();
                Esito::ok(t.clone(), json!({ "kind": "text", "text": t.chars().take(600).collect::<String>() }))
            }
            _ => Esito::ok("Negli appunti non c'è testo.", json!({ "kind": "text", "text": "Appunti vuoti" })),
        },
        "scrivi_appunti" => {
            let t = str_arg(input, "testo");
            match cb.set_text(t.to_string()) {
                Ok(()) => Esito::ok("Copiato negli appunti.", json!({ "kind": "text", "text": t.chars().take(600).collect::<String>() })),
                Err(e) => Esito::errore(format!("Non riesco a copiare: {e}")),
            }
        }
        _ => Esito::errore("strumento sconosciuto"),
    }
}

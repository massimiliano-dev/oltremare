//! Aprire app, file, cartelle e link.

use super::percorsi::{consentito, mostra};
use super::{str_arg, Ambiente, Esito};
use serde_json::{json, Value};
use tauri_plugin_opener::OpenerExt;

pub async fn esegui(nome: &str, input: &Value, amb: &Ambiente) -> Esito {
    match nome {
        "apri_app" => apri_app(str_arg(input, "nome").trim()).await,
        "apri_percorso" => match consentito(str_arg(input, "percorso"), &amb.radici) {
            Ok(p) if p.exists() => match amb.app.opener().open_path(p.to_string_lossy(), None::<&str>) {
                Ok(()) => Esito::ok(format!("Aperto {}", mostra(&p)), json!({ "kind": "text", "text": format!("Aperto {}", mostra(&p)) })),
                Err(e) => Esito::errore(format!("Non riesco ad aprirlo: {e}")),
            },
            Ok(p) => Esito::errore(format!("Non trovato: {}", mostra(&p))),
            Err(e) => Esito::errore(e),
        },
        "apri_url" => {
            let url = str_arg(input, "url").trim();
            if !(url.starts_with("https://") || url.starts_with("http://")) {
                return Esito::errore("Apro solo indirizzi http:// o https://");
            }
            match amb.app.opener().open_url(url, None::<&str>) {
                Ok(()) => Esito::ok(format!("Aperto {url}"), json!({ "kind": "text", "text": format!("Aperto {url}") })),
                Err(e) => Esito::errore(format!("Non riesco ad aprire il link: {e}")),
            }
        }
        _ => Esito::errore("strumento sconosciuto"),
    }
}

async fn apri_app(nome: &str) -> Esito {
    if nome.is_empty() || nome.contains(['\n', '\r', '"']) {
        return Esito::errore("Nome dell'app non valido");
    }
    #[cfg(target_os = "macos")]
    let out = tokio::process::Command::new("open").args(["-a", nome]).output().await;
    #[cfg(windows)]
    let out = tokio::process::Command::new("cmd").args(["/C", "start", "", nome]).creation_flags(0x0800_0000).output().await;
    #[cfg(all(unix, not(target_os = "macos")))]
    let out = tokio::process::Command::new("gtk-launch").arg(nome).output().await;
    match out {
        Ok(o) if o.status.success() => Esito::ok(format!("Aperta {nome}"), json!({ "kind": "text", "text": format!("Aperta {nome}") })),
        Ok(_) => Esito::errore(format!("Non trovo l'app «{nome}» su questo computer")),
        Err(e) => Esito::errore(format!("Non riesco ad aprire «{nome}»: {e}")),
    }
}

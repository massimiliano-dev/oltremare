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


/// AppleScript / JavaScript for Automation: le app si comandano senza mouse né schermo.
pub async fn automazione(input: &Value, cancel: &tokio_util::sync::CancellationToken) -> Esito {
    if !cfg!(target_os = "macos") {
        return Esito::errore("L'automazione delle app con AppleScript esiste solo su macOS");
    }
    let script = input["script"].as_str().unwrap_or_default();
    if script.trim().is_empty() {
        return Esito::errore("Script vuoto");
    }
    let basso = script.to_lowercase();
    for (vietato, motivo) in [
        ("administrator privileges", "chiede i privilegi di amministratore"),
        ("keychain", "tocca il portachiavi"),
        ("portachiavi", "tocca il portachiavi"),
        ("1password", "tocca un gestore di password"),
        ("security find-", "legge segreti dal portachiavi"),
    ] {
        if basso.contains(vietato) {
            return Esito::errore(format!("Script bloccato da Oltremare: {motivo}. Chiedi all'utente di farlo lui."));
        }
    }
    let lingua = if input["linguaggio"].as_str() == Some("javascript") { "JavaScript" } else { "AppleScript" };
    let mut cmd = tokio::process::Command::new("/usr/bin/osascript");
    cmd.args(["-l", lingua, "-e", script]).stdin(std::process::Stdio::null()).kill_on_drop(true);
    let r = tokio::select! {
        r = tokio::time::timeout(std::time::Duration::from_secs(90), cmd.output()) => r,
        _ = cancel.cancelled() => return Esito::errore("Interrotto"),
    };
    match r {
        Ok(Ok(o)) => {
            let out = String::from_utf8_lossy(&o.stdout).trim().to_string();
            let err = String::from_utf8_lossy(&o.stderr).trim().to_string();
            if o.status.success() {
                let t = if out.is_empty() { "Fatto.".to_string() } else { out.chars().take(20_000).collect() };
                Esito::ok(t.clone(), serde_json::json!({ "kind": "text", "text": t.chars().take(4000).collect::<String>() }))
            } else if err.contains("-1743") || err.contains("Not authorized") {
                Esito::errore("macOS non ha ancora autorizzato Oltremare a comandare questa app: l'utente deve consentirlo nella richiesta che compare (o in Impostazioni di Sistema → Privacy e sicurezza → Automazione).")
            } else {
                Esito::errore(format!("Errore dello script: {err}"))
            }
        }
        Ok(Err(e)) => Esito::errore(e.to_string()),
        Err(_) => Esito::errore("Lo script non ha finito in 90 secondi"),
    }
}

#[cfg(all(test, target_os = "macos"))]
mod prova_automazione {
    #[tokio::test]
    async fn applescript_e_blocchi() {
        let c = tokio_util::sync::CancellationToken::new();
        let e = super::automazione(&serde_json::json!({ "script": "return 40 + 2" }), &c).await;
        assert_eq!(e.testo, "42", "{}", e.testo);
        let j = super::automazione(&serde_json::json!({ "script": "6 * 7", "linguaggio": "javascript" }), &c).await;
        assert_eq!(j.testo, "42", "{}", j.testo);
        let b = super::automazione(&serde_json::json!({ "script": "do shell script \"ls\" with administrator privileges" }), &c).await;
        assert!(b.is_error);
    }
}

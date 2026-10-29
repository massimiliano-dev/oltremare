//! Cosa può fare un agente senza chiedere: regola per strumento, poi regola per livello di rischio.

use crate::store::Agent;
use crate::tools::{Rischio, ToolSpec};
use std::collections::HashSet;

/// "allow" | "ask" | "deny"
pub fn decisione(agent: &Agent, t: &ToolSpec, consentiti_sessione: &HashSet<String>) -> &'static str {
    // La memoria è un'area sicura (non si cancella nulla, le riscritture lasciano una copia): si scrive senza chiedere
    let regola = agent.policy.per_tool.get(t.nome).map(String::as_str).unwrap_or(match t.rischio {
        _ if t.categoria == "memoria" => "allow",
        Rischio::Read => agent.policy.read_default.as_str(),
        Rischio::Write => agent.policy.write_default.as_str(),
        Rischio::Danger => agent.policy.danger_default.as_str(),
    });
    match regola {
        "deny" => "deny",
        "allow" => "allow",
        // "Sempre in questa esecuzione" vale fino alla fine del lavoro in corso
        _ if consentiti_sessione.contains(t.nome) => "allow",
        _ => "ask",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::spec;

    #[test]
    fn resolution_order() {
        let mut a = Agent::default();
        let mut s = HashSet::new();
        assert_eq!(decisione(&a, spec("leggi_file").unwrap(), &s), "allow");
        assert_eq!(decisione(&a, spec("scrivi_file").unwrap(), &s), "ask");
        assert_eq!(decisione(&a, spec("esegui_comando").unwrap(), &s), "ask");
        s.insert("scrivi_file".to_string());
        assert_eq!(decisione(&a, spec("scrivi_file").unwrap(), &s), "allow");
        a.policy.per_tool.insert("scrivi_file".into(), "deny".into());
        assert_eq!(decisione(&a, spec("scrivi_file").unwrap(), &s), "deny", "il divieto vince sul permesso di sessione");
        a.policy.per_tool.insert("leggi_file".into(), "ask".into());
        assert_eq!(decisione(&a, spec("leggi_file").unwrap(), &HashSet::new()), "ask");
    }
}

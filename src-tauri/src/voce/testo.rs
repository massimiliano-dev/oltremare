//! Testo e voce: parola di attivazione, comandi brevi, testo da leggere ad alta voce.

/// Minuscolo, senza accenti né punteggiatura, spazi singoli.
pub fn normalizza(t: &str) -> String {
    let s: String = t
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'à' | 'á' | 'â' => 'a',
            'è' | 'é' | 'ê' => 'e',
            'ì' | 'í' | 'î' => 'i',
            'ò' | 'ó' | 'ô' => 'o',
            'ù' | 'ú' | 'û' => 'u',
            c if c.is_alphanumeric() => c,
            _ => ' ',
        })
        .collect();
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

const SALUTI: &[&str] = &["ehi", "ei", "hey", "hei", "ehy", "e", "ok", "okay", "ciao", "senti", "hai", "eh"];
const MARE: &[&str] = &["mare", "mar", "mari", "amare", "male"];

/// Distanza di Levenshtein tra due parole.
fn distanza(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prec: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            cur.push((prec[j] + usize::from(ca != cb)).min(prec[j + 1] + 1).min(cur[j] + 1));
        }
        prec = cur;
    }
    prec[b.len()]
}

fn somiglianza(a: &str, b: &str) -> f32 {
    1.0 - distanza(a, b) as f32 / a.chars().count().max(b.chars().count()).max(1) as f32
}

/// Se la frase inizia con la parola di attivazione, restituisce il resto (il comando, anche vuoto).
/// Tollerante: Whisper può sentire "ultimare", "oltre mare", "altremare"…
pub fn dopo_attivazione(testo: &str) -> Option<String> {
    let n = normalizza(testo);
    let mut parole: Vec<&str> = n.split(' ').filter(|p| !p.is_empty()).collect();
    // Al massimo un saluto prima del nome
    if parole.len() > 1 && SALUTI.contains(&parole[0]) {
        parole.remove(0);
    }
    let usate = match parole.as_slice() {
        [p, ..] if p.chars().count() >= 6 && somiglianza(p, "oltremare") >= 0.62 => 1,
        [p, m, ..] if somiglianza(p, "oltre") >= 0.75 && MARE.contains(m) => 2,
        [p, a, m, ..] if somiglianza(p, "oltre") >= 0.75 && matches!(*a, "a" | "e" | "il") && MARE.contains(m) => 3,
        _ => return None,
    };
    let resto = parole[usate..].join(" ");
    Some(if resto.is_empty() { String::new() } else { resto_originale(testo, &resto) })
}

fn resto_originale(originale: &str, resto_norm: &str) -> String {
    let n_parole = resto_norm.split_whitespace().count();
    let parole: Vec<&str> = originale.split_whitespace().collect();
    let inizio = parole.len().saturating_sub(n_parole);
    let r = parole[inizio..].join(" ");
    r.trim_start_matches([',', '.', ':', ';', '!', ' ']).trim().to_string()
}

/// "Basta", "stop"…: interrompe la lettura.
pub fn e_stop(testo: &str) -> bool {
    let n = normalizza(testo);
    let n = dopo_attivazione(&n).filter(|r| !r.is_empty()).map(|r| normalizza(&r)).unwrap_or(n);
    matches!(n.as_str(), "basta" | "stop" | "zitto" | "zitta" | "silenzio" | "fermati" | "ferma" | "basta cosi" | "ok basta" | "grazie basta")
}

/// Whisper a volte "sente" frasi fantasma sul silenzio o sul rumore.
pub fn allucinazione(testo: &str) -> bool {
    let n = normalizza(testo);
    n.is_empty()
        || n.len() < 2
        || ["sottotitoli", "amara org", "grazie per la visione", "iscriviti al canale", "musica", "applausi", "sottotitoli creati dalla comunita"]
            .iter()
            .any(|x| n.contains(x))
}

/// Testo adatto alla lettura: senza Markdown, con numeri e simboli pronunciabili.
pub fn parlato(t: &str) -> String {
    let re_link = regex::Regex::new(r"\[([^\]]+)\]\([^)]+\)").expect("regex");
    let re_ora = regex::Regex::new(r"\b(\d{1,2})[:.](\d{2})\b").expect("regex");
    let re_url = regex::Regex::new(r"https?://\S+").expect("regex");
    let mut dentro_codice = false;
    let righe: Vec<String> = t
        .lines()
        .filter_map(|l| {
            if l.trim_start().starts_with("```") {
                dentro_codice = !dentro_codice;
                return None;
            }
            if dentro_codice {
                return None;
            }
            let l = l.trim().trim_start_matches(['#', '>', '-', '*', '•']).trim();
            let l = l.trim_start_matches(|c: char| c.is_ascii_digit()).trim_start_matches(['.', ')']).trim();
            (!l.is_empty()).then(|| l.replace("**", "").replace(['`', '*', '_'], ""))
        })
        .collect();
    let mut s = righe.join(". ").replace("..", ".").replace(":.", ":");
    s = re_link.replace_all(&s, "$1").into_owned();
    s = re_url.replace_all(&s, "il link").into_owned();
    s = re_ora.replace_all(&s, |c: &regex::Captures| if &c[2] == "00" { c[1].to_string() } else { format!("{} e {}", &c[1], &c[2]) }).into_owned();
    s.replace('€', " euro").replace('%', " per cento").replace(" & ", " e ").replace('/', " su ")
}

/// Frasi brevi, per iniziare a parlare prima che tutto il testo sia sintetizzato.
pub fn frasi(t: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in t.chars() {
        cur.push(c);
        if matches!(c, '.' | '!' | '?' | ';' | ':') && cur.trim().len() > 12 {
            out.push(cur.trim().to_string());
            cur.clear();
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wake_phrase_variants() {
        assert_eq!(dopo_attivazione("Ehi Oltremare, che ore sono?").as_deref(), Some("che ore sono?"));
        assert_eq!(dopo_attivazione("Oltremare.").as_deref(), Some(""));
        assert_eq!(dopo_attivazione("hey oltre mare apri le note").as_deref(), Some("apri le note"));
        assert_eq!(dopo_attivazione("Ciao Oltremare! Accendi la luce").as_deref(), Some("Accendi la luce"));
        assert_eq!(dopo_attivazione("e ultimare, che ore sono?").as_deref(), Some("che ore sono?"));
        assert_eq!(dopo_attivazione("Altremare apri Safari").as_deref(), Some("apri Safari"));
        assert_eq!(dopo_attivazione("E oltre a mare che ore sono").as_deref(), Some("che ore sono"));
        assert!(dopo_attivazione("Il mare è oltre la collina").is_none());
        assert!(dopo_attivazione("Oltre la collina c'è il mare").is_none());
        assert!(dopo_attivazione("domani vado a oltremare").is_none());
        assert!(dopo_attivazione("ultimamente sono stanco").is_none());
    }

    #[test]
    fn stop_and_hallucinations() {
        assert!(e_stop("Basta!"));
        assert!(e_stop("Oltremare, stop."));
        assert!(!e_stop("basta pasta per stasera"));
        assert!(allucinazione("Sottotitoli creati dalla comunità Amara.org"));
        assert!(!allucinazione("Che tempo fa domani?"));
    }

    #[test]
    fn speakable_text() {
        let t = parlato("## Oggi\n- **Riunione** alle 14:30\n- Pranzo alle 13:00\n```\ncodice\n```\nCosto 20€, sconto 10%");
        assert!(t.contains("Riunione alle 14 e 30"), "{t}");
        assert!(t.contains("alle 13") && !t.contains("codice") && t.contains("20 euro") && t.contains("10 per cento"), "{t}");
        assert_eq!(frasi("Ciao a tutti quanti. Come state oggi? Bene").len(), 3);
    }
}

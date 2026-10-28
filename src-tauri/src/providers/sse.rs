//! Lettura dei flussi in streaming: righe complete da un buffer di byte.

/// Accumula byte e restituisce le righe complete. Un carattere UTF-8 o una riga
/// possono arrivare spezzati tra due pacchetti: si aspetta il resto.
#[derive(Default)]
pub struct Righe {
    buf: Vec<u8>,
}

impl Righe {
    pub fn push(&mut self, bytes: &[u8]) -> Vec<String> {
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        while let Some(pos) = self.buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = self.buf.drain(..=pos).collect();
            let line = String::from_utf8_lossy(&line).trim().to_string();
            if !line.is_empty() {
                out.push(line);
            }
        }
        out
    }
}

/// Il contenuto di una riga SSE `data: …`.
pub fn dati(line: &str) -> Option<&str> {
    line.strip_prefix("data:").map(str::trim)
}

/// Toglie i blocchi `<think>…</think>` che alcuni modelli locali emettono nel testo.
#[derive(Default)]
pub struct ThinkFilter {
    inside: bool,
    pending: String,
}

impl ThinkFilter {
    /// Restituisce (testo visibile, ragionamento)
    pub fn push(&mut self, delta: &str) -> (String, String) {
        self.pending.push_str(delta);
        let mut out = String::new();
        let mut think = String::new();
        loop {
            let tag = if self.inside { "</think>" } else { "<think>" };
            if let Some(pos) = self.pending.find(tag) {
                if self.inside { think.push_str(&self.pending[..pos]) } else { out.push_str(&self.pending[..pos]) }
                self.pending.drain(..pos + tag.len());
                self.inside = !self.inside;
                continue;
            }
            // Tiene da parte un possibile inizio di tag spezzato tra due pezzi
            let keep = (1..tag.len()).rev().find(|&n| self.pending.ends_with(&tag[..n])).unwrap_or(0);
            let cut = self.pending.len() - keep;
            if self.inside { think.push_str(&self.pending[..cut]) } else { out.push_str(&self.pending[..cut]) }
            self.pending.drain(..cut);
            return (out, think);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_and_lines_split_across_packets() {
        let mut r = Righe::default();
        let all = "data: {\"t\":\"caffè\"}\n\ndata: [DONE]\n".as_bytes();
        let cut = all.iter().position(|&b| b == 0xC3).unwrap() + 1; // a metà della "è"
        let mut lines = r.push(&all[..cut]);
        assert!(lines.is_empty());
        lines.extend(r.push(&all[cut..cut + 3]));
        lines.extend(r.push(&all[cut + 3..]));
        assert_eq!(lines, vec!["data: {\"t\":\"caffè\"}", "data: [DONE]"]);
        assert_eq!(dati(&lines[1]), Some("[DONE]"));
    }

    #[test]
    fn think_blocks_are_separated_even_when_split() {
        let mut f = ThinkFilter::default();
        let (mut out, mut think) = (String::new(), String::new());
        for piece in ["Ciao <thi", "nk>ragiono…</th", "ink>Mario", ", a presto"] {
            let (o, t) = f.push(piece);
            out.push_str(&o);
            think.push_str(&t);
        }
        assert_eq!(out, "Ciao Mario, a presto");
        assert_eq!(think, "ragiono…");
    }
}

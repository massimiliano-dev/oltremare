//! Stima del costo di un'esecuzione con Claude (dollari per milione di token).

use crate::providers::Usage;

struct Prezzo {
    input: f64,
    output: f64,
    cache_read: f64,
}

fn prezzo(model: &str) -> Option<Prezzo> {
    let p = |input: f64, output: f64, cache_read: f64| Some(Prezzo { input, output, cache_read });
    let m = model;
    if m.starts_with("claude-fable-5-1") || m.starts_with("claude-mythos-5-1") {
        p(10.0, 50.0, 0.25)
    } else if m.starts_with("claude-fable") || m.starts_with("claude-mythos") {
        p(10.0, 50.0, 1.0)
    } else if m.starts_with("claude-opus-5-5") {
        p(4.0, 20.0, 0.20)
    } else if m.starts_with("claude-opus") {
        p(5.0, 25.0, 0.50)
    } else if m.starts_with("claude-sonnet-5") {
        p(2.0, 10.0, 0.20)
    } else if m.starts_with("claude-sonnet") {
        p(3.0, 15.0, 0.30)
    } else if m.starts_with("claude-haiku-5-5") {
        p(0.10, 0.50, 0.01)
    } else if m.starts_with("claude-haiku") {
        p(1.0, 5.0, 0.10)
    } else {
        None
    }
}

pub fn stima(model: &str, u: &Usage) -> Option<f64> {
    let p = prezzo(model)?;
    let mtok = 1_000_000.0;
    Some(
        u.input_tokens as f64 * p.input / mtok
            + u.output_tokens as f64 * p.output / mtok
            + u.cache_read as f64 * p.cache_read / mtok
            // La scrittura in cache costa il 25% in più dell'input
            + u.cache_write as f64 * p.input * 1.25 / mtok,
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn opus_5_5_price() {
        let u = crate::providers::Usage { input_tokens: 1_000_000, output_tokens: 1_000_000, cache_read: 0, cache_write: 0 };
        assert!((super::stima("claude-opus-5-5", &u).unwrap() - 24.0).abs() < 1e-9);
        assert!(super::stima("gpt-5", &u).is_none());
    }
}

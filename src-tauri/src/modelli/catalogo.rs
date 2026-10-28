//! Catalogo dei modelli scaricabili. I file arrivano da Hugging Face; l'impronta SHA-256 si legge
//! dall'elenco dei file del repository al momento del download.

use serde::Serialize;

pub struct Voce {
    pub id: &'static str,
    /// "llm" | "stt" | "tts" | "vad"
    pub kind: &'static str,
    pub label: &'static str,
    pub nota: &'static str,
    /// Repository Hugging Face (vuoto se il modello arriva da un indirizzo diretto)
    pub repo: &'static str,
    /// Indirizzo diretto di un file o di un archivio .tar.bz2, con la sua impronta (se nota)
    pub url: &'static str,
    pub sha: &'static str,
    /// File che devono esserci a installazione completata
    pub files: &'static [&'static str],
    pub dimensione: u64,
    pub ram_gb: u32,
    pub ctx: u32,
    pub strumenti: bool,
    pub visione: bool,
    /// Consigliato da questa quantità di RAM in su (GB)
    pub consigliato_da_gb: u32,
}

pub static MODELLI: &[Voce] = &[
    Voce {
        id: "gemma-4-e4b", kind: "llm", label: "Gemma 4 E4B", nota: "Piccolo e veloce, vede le immagini. Per computer con 8–16 GB",
        url: "", sha: "", repo: "unsloth/gemma-4-E4B-it-GGUF", files: &["gemma-4-E4B-it-Q4_K_M.gguf", "mmproj-F16.gguf"],
        dimensione: 5_970_000_000, ram_gb: 8, ctx: 32768, strumenti: true, visione: true, consigliato_da_gb: 8,
    },
    Voce {
        id: "qwen3.5-4b", kind: "llm", label: "Qwen 3.5 4B", nota: "Il più leggero: buono per compiti semplici",
        url: "", sha: "", repo: "unsloth/Qwen3.5-4B-GGUF", files: &["Qwen3.5-4B-Q4_K_M.gguf", "mmproj-F16.gguf"],
        dimensione: 3_410_000_000, ram_gb: 6, ctx: 32768, strumenti: true, visione: true, consigliato_da_gb: 0,
    },
    Voce {
        id: "gemma-4-12b", kind: "llm", label: "Gemma 4 12B", nota: "Equilibrato: buon italiano, vede le immagini. Da 16 GB",
        url: "", sha: "", repo: "unsloth/gemma-4-12b-it-GGUF", files: &["gemma-4-12b-it-Q4_K_M.gguf", "mmproj-F16.gguf"],
        dimensione: 7_300_000_000, ram_gb: 12, ctx: 32768, strumenti: true, visione: true, consigliato_da_gb: 16,
    },
    Voce {
        id: "gemma-4-26b-a4b", kind: "llm", label: "Gemma 4 26B", nota: "Consigliato con 32 GB: veloce (MoE) e ottimo italiano",
        url: "", sha: "", repo: "unsloth/gemma-4-26B-A4B-it-GGUF", files: &["gemma-4-26B-A4B-it-UD-Q4_K_M.gguf", "mmproj-F16.gguf"],
        dimensione: 18_140_000_000, ram_gb: 22, ctx: 32768, strumenti: true, visione: true, consigliato_da_gb: 32,
    },
    Voce {
        id: "qwen3.8-27b", kind: "llm", label: "Qwen 3.8 27B", nota: "Il più preciso nei compiti con strumenti, più lento",
        url: "", sha: "", repo: "unsloth/Qwen3.8-27B-GGUF", files: &["Qwen3.8-27B-UD-Q4_K_M.gguf", "mmproj-F16.gguf"],
        dimensione: 17_390_000_000, ram_gb: 22, ctx: 32768, strumenti: true, visione: true, consigliato_da_gb: 48,
    },
];

pub fn voce(id: &str) -> Option<&'static Voce> {
    MODELLI.iter().find(|m| m.id == id).or_else(|| crate::voce::modelli::voce(id))
}

pub fn tutte() -> impl Iterator<Item = &'static Voce> {
    MODELLI.iter().chain(crate::voce::modelli::TUTTI.iter())
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Vista {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub note: String,
    pub repo: String,
    pub size_bytes: u64,
    pub ram_needed_gb: u32,
    pub ctx: u32,
    pub supports_tools: bool,
    pub supports_vision: bool,
    pub recommended: bool,
    pub installed: bool,
    pub custom: bool,
}

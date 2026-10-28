//! Modelli per la voce: rilevamento del parlato, riconoscimento (Whisper) e sintesi (Piper).

use crate::modelli::catalogo::Voce;

pub static TUTTI: &[Voce] = &[
    Voce {
        id: "silero-vad", kind: "vad", label: "Rilevamento del parlato", nota: "Capisce quando qualcuno parla (Silero)",
        repo: "", url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx",
        sha: "9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6", files: &["silero_vad.onnx"],
        dimensione: 643_854, ram_gb: 0, ctx: 0, strumenti: false, visione: false, consigliato_da_gb: 0,
    },
    Voce {
        id: "whisper-small", kind: "stt", label: "Whisper small", nota: "Riconoscimento veloce, buono per i comandi",
        repo: "csukuangfj/sherpa-onnx-whisper-small", url: "", sha: "",
        files: &["small-encoder.int8.onnx", "small-decoder.int8.onnx", "small-tokens.txt"],
        dimensione: 375_485_327, ram_gb: 1, ctx: 0, strumenti: false, visione: false, consigliato_da_gb: 0,
    },
    Voce {
        id: "whisper-turbo", kind: "stt", label: "Whisper turbo", nota: "Più preciso anche per dettature lunghe, più pesante",
        repo: "csukuangfj/sherpa-onnx-whisper-turbo", url: "", sha: "",
        files: &["turbo-encoder.int8.onnx", "turbo-decoder.int8.onnx", "turbo-tokens.txt"],
        dimensione: 1_036_613_791, ram_gb: 2, ctx: 0, strumenti: false, visione: false, consigliato_da_gb: 99,
    },
    Voce {
        id: "piper-paola", kind: "tts", label: "Paola", nota: "Voce femminile italiana (Piper)",
        repo: "", url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/vits-piper-it_IT-paola-medium-int8.tar.bz2",
        sha: "2b975ed305391c056944a4dde67ee754dd824099503a860295bb4c1d724662d8",
        files: &["it_IT-paola-medium.onnx", "tokens.txt", "espeak-ng-data/phontab"],
        dimensione: 21_143_212, ram_gb: 0, ctx: 0, strumenti: false, visione: false, consigliato_da_gb: 0,
    },
    Voce {
        id: "piper-miro", kind: "tts", label: "Miro", nota: "Voce maschile italiana (Piper)",
        repo: "", url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/vits-piper-it_IT-miro-high-int8.tar.bz2",
        sha: "c3c193381d9fadd3cc9ad03c230e04a29a7b893343b162d127fd59e03a4a74b1",
        files: &["it_IT-miro-high.onnx", "tokens.txt", "espeak-ng-data/phontab"],
        dimensione: 21_238_206, ram_gb: 0, ctx: 0, strumenti: false, visione: false, consigliato_da_gb: 99,
    },
];

pub fn voce(id: &str) -> Option<&'static Voce> {
    TUTTI.iter().find(|m| m.id == id)
}

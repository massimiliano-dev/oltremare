use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Msg(String),
    #[error("Operazione annullata")]
    Annullato,
    #[error("File: {0}")]
    Io(#[from] std::io::Error),
    #[error("Rete: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Portachiavi: {0}")]
    Keyring(#[from] keyring::Error),
    #[error("Dati: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Archivio: {0}")]
    Db(#[from] rusqlite::Error),
}

impl AppError {
    pub fn msg(s: impl Into<String>) -> Self {
        AppError::Msg(s.into())
    }

    /// Errore di rete passeggero (Wi-Fi non ancora pronto dopo il risveglio, server irraggiungibile).
    pub fn e_rete(&self) -> bool {
        matches!(self, AppError::Http(e) if e.is_connect() || e.is_timeout() || e.is_request())
    }
}

// Il frontend riceve sempre una stringa leggibile
impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AppError>;

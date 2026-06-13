use thiserror::Error;
use serde::{Deserialize, Serialize};

#[derive(Error, Debug, Serialize, Deserialize, Clone)]
pub enum IziError {
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Profile error: {0}")]
    Profile(String),
    
    #[error("OCR error: {0}")]
    Ocr(String),
    
    #[error("ASR error: {0}")]
    Asr(String),
    
    #[error("LLM error: {0}")]
    Llm(String),
    
    #[error("D-Bus error: {0}")]
    Dbus(String),
    
    #[error("Portal error: {0}")]
    Portal(String),
    
    #[error("PipeWire error: {0}")]
    PipeWire(String),
    
    #[error("Keyring error: {0}")]
    Keyring(String),
    
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<zbus::Error> for IziError {
    fn from(err: zbus::Error) -> Self {
        IziError::Dbus(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, IziError>;

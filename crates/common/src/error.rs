use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug, Serialize, Deserialize, Clone)]
pub enum IziError {
    #[error("Ошибка конфигурации: {0}")]
    Config(String),

    #[error("Ошибка профиля: {0}")]
    Profile(String),

    #[error("Ошибка OCR: {0}")]
    Ocr(String),

    #[error("Ошибка ASR: {0}")]
    Asr(String),

    #[error("Ошибка LLM: {0}")]
    Llm(String),

    #[error("Ошибка D-Bus: {0}")]
    Dbus(String),

    #[error("Ошибка портала: {0}")]
    Portal(String),

    #[error("Ошибка PipeWire: {0}")]
    PipeWire(String),

    #[error("Ошибка связки ключей: {0}")]
    Keyring(String),

    #[error("Внутренняя ошибка: {0}")]
    Internal(String),
}

impl From<zbus::Error> for IziError {
    fn from(err: zbus::Error) -> Self {
        IziError::Dbus(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, IziError>;

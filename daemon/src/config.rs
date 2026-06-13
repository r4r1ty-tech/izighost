use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::fs;
use anyhow::{Context, Result};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GeneralConfig {
    pub log_level: String,
    pub data_dir: String,
    pub socket_activation: bool,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            log_level: "info".to_string(),
            data_dir: "~/.local/share/izighost".to_string(),
            socket_activation: false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OcrPreprocessingConfig {
    pub upscale: bool,
    pub grayscale: bool,
    pub deskew: bool,
}

impl Default for OcrPreprocessingConfig {
    fn default() -> Self {
        Self {
            upscale: true,
            grayscale: true,
            deskew: true,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OcrConfig {
    pub engine: String,
    pub binary: String,
    pub language: String,
    pub psm: i32,
    pub preprocessing: OcrPreprocessingConfig,
}

impl Default for OcrConfig {
    fn default() -> Self {
        Self {
            engine: "tesseract".to_string(),
            binary: "/usr/bin/tesseract".to_string(),
            language: "eng+rus".to_string(),
            psm: 6,
            preprocessing: OcrPreprocessingConfig::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VadConfig {
    pub enabled: bool,
    pub energy_threshold: f32,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            energy_threshold: 0.01,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PipewireConfig {
    pub target_sink: Option<String>,
    pub target_source: Option<String>,
}

impl Default for PipewireConfig {
    fn default() -> Self {
        Self {
            target_sink: None,
            target_source: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AudioConfig {
    pub source: String,
    pub chunk_duration_ms: u32,
    pub vad: VadConfig,
    pub pipewire: PipewireConfig,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            source: "both".to_string(),
            chunk_duration_ms: 1500,
            vad: VadConfig::default(),
            pipewire: PipewireConfig::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LlmDefaultsConfig {
    pub streaming: bool,
    pub max_context_messages: u32,
    pub temperature: f64,
    pub request_timeout_sec: u64,
}

impl Default for LlmDefaultsConfig {
    fn default() -> Self {
        Self {
            streaming: true,
            max_context_messages: 20,
            temperature: 0.7,
            request_timeout_sec: 60,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AsrDefaultsConfig {
    pub chunk_duration_sec: f64,
    pub language: String,
}

impl Default for AsrDefaultsConfig {
    fn default() -> Self {
        Self {
            chunk_duration_sec: 1.5,
            language: "auto".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct DaemonConfig {
    pub general: GeneralConfig,
    pub ocr: OcrConfig,
    pub audio: AudioConfig,
    pub llm_defaults: LlmDefaultsConfig,
    pub asr_defaults: AsrDefaultsConfig,
}

pub fn resolve_path(path: &str) -> PathBuf {
    if path.starts_with("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(&path[2..]);
        }
    }
    PathBuf::from(path)
}

impl DaemonConfig {
    pub fn load() -> Result<Self> {
        let config_dir = resolve_path("~/.config/izighost");
        let config_file = config_dir.join("daemon.yaml");
        
        if !config_file.exists() {
            // Create config dir and write default config
            fs::create_dir_all(&config_dir).context("Failed to create config directory")?;
            let default_config = DaemonConfig::default();
            let yaml = serde_yaml::to_string(&default_config).context("Failed to serialize default config")?;
            fs::write(&config_file, yaml).context("Failed to write default config file")?;
            return Ok(default_config);
        }
        
        let content = fs::read_to_string(&config_file).context("Failed to read config file")?;
        let config: DaemonConfig = serde_yaml::from_str(&content).context("Failed to parse config file")?;
        Ok(config)
    }
}

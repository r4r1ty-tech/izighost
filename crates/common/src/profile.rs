use serde::{Deserialize, Serialize};
use zvariant::Type;

#[derive(Serialize, Deserialize, Type, Debug, Clone)]
pub struct LlmConfig {
    pub provider: String,
    pub model: String,
    pub base_url: String,
    pub temperature: f64,
    pub max_context_messages: u32,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: "openai_compat".to_string(),
            model: "gpt-4o-mini".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            temperature: 0.7,
            max_context_messages: 20,
        }
    }
}

#[derive(Serialize, Deserialize, Type, Debug, Clone)]
pub struct AsrConfig {
    pub provider: String,
    pub model: String,
    pub base_url: String,
}

impl Default for AsrConfig {
    fn default() -> Self {
        Self {
            provider: "openai_compat".to_string(),
            model: "whisper-1".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
        }
    }
}

fn default_use_ocr_prompt() -> bool {
    true
}

fn default_ocr_prompt() -> String {
    "Extract all text from this image exactly as it appears. Do not add any introduction, explanations, comments, or markdown code blocks. Just output the extracted text.".to_string()
}

#[derive(Serialize, Deserialize, Type, Debug, Clone)]
pub struct VisionConfig {
    pub provider: String,
    pub model: String,
    pub base_url: String,
    #[serde(default = "default_use_ocr_prompt")]
    pub use_ocr_prompt: bool,
    #[serde(default = "default_ocr_prompt")]
    pub ocr_prompt: String,
}

impl Default for VisionConfig {
    fn default() -> Self {
        Self {
            provider: "openai_compat".to_string(),
            model: "meta-llama/llama-4-scout-17b-16e-instruct".to_string(),
            base_url: "https://api.groq.com/openai/v1".to_string(),
            use_ocr_prompt: default_use_ocr_prompt(),
            ocr_prompt: default_ocr_prompt(),
        }
    }
}

#[derive(Serialize, Deserialize, Type, Debug, Clone)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub cv_path: String,
    pub cv_text: String,
    pub vacancy_path: String,
    pub vacancy_text: String,
    pub extra: String,
    pub facts: String,
    pub system_prompt: String,
    pub llm: LlmConfig,
    pub asr: AsrConfig,
    #[serde(default)]
    pub vision: VisionConfig,
    pub created: String,
    pub last_used: String,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            id: "".to_string(),
            name: "Новый профиль".to_string(),
            cv_path: "".to_string(),
            cv_text: "".to_string(),
            vacancy_path: "".to_string(),
            vacancy_text: "".to_string(),
            extra: "".to_string(),
            facts: "".to_string(),
            system_prompt: "Ты senior-ментор для подготовки к собеседованию.\nОтвечай кратко, по делу, на русском.\nЕсли не знаешь ответа — скажи прямо.".to_string(),
            llm: LlmConfig::default(),
            asr: AsrConfig::default(),
            vision: VisionConfig::default(),
            created: "".to_string(),
            last_used: "".to_string(),
        }
    }
}

use serde::{Deserialize, Serialize};
use zvariant::Type;

/// Конфигурация для языковых моделей (LLM).
#[derive(Serialize, Deserialize, Type, Debug, Clone)]
pub struct LlmConfig {
    /// Имя провайдера (например, "openai", "groq", "openai_compat").
    pub provider: String,
    /// Имя используемой модели.
    pub model: String,
    /// Базовый URL API для запросов.
    pub base_url: String,
    /// Температура генерации (креативность ответов).
    pub temperature: f64,
    /// Максимальное число сообщений контекста, хранящихся в истории.
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

/// Конфигурация для распознавания речи (ASR / Whisper).
#[derive(Serialize, Deserialize, Type, Debug, Clone)]
pub struct AsrConfig {
    /// Имя провайдера (например, "openai", "groq", "local").
    pub provider: String,
    /// Имя используемой модели.
    pub model: String,
    /// Базовый URL API для отправки аудио.
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

/// Конфигурация для Vision LLM (обработка изображений).
#[derive(Serialize, Deserialize, Type, Debug, Clone)]
pub struct VisionConfig {
    /// Имя провайдера для обработки картинок.
    pub provider: String,
    /// Имя мультимодальной модели.
    pub model: String,
    /// Базовый URL API.
    pub base_url: String,
    /// Использовать ли специальный OCR-промпт при отправке изображения.
    #[serde(default = "default_use_ocr_prompt")]
    pub use_ocr_prompt: bool,
    /// Инструкция для модели при обработке изображения.
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

/// Профиль пользователя, содержащий системные инструкции, резюме, вакансию и настройки моделей.
#[derive(Serialize, Deserialize, Type, Debug, Clone)]
pub struct Profile {
    /// Уникальный идентификатор профиля.
    pub id: String,
    /// Человекочитаемое название профиля.
    pub name: String,
    /// Путь к файлу резюме.
    pub cv_path: String,
    /// Текстовое содержимое резюме.
    pub cv_text: String,
    /// Путь к файлу описания вакансии.
    pub vacancy_path: String,
    /// Текстовое содержимое вакансии.
    pub vacancy_text: String,
    /// Дополнительный контекст.
    pub extra: String,
    /// Факты о кандидате.
    pub facts: String,
    /// Системные инструкции для AI (System Prompt).
    pub system_prompt: String,
    /// Конфигурация текстовой языковой модели.
    pub llm: LlmConfig,
    /// Конфигурация модели распознавания речи.
    pub asr: AsrConfig,
    /// Конфигурация мультимодальной модели.
    #[serde(default)]
    pub vision: VisionConfig,
    /// Время создания профиля.
    pub created: String,
    /// Время последнего использования профиля.
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

use izighost_common::Profile;

pub fn assemble_system_prompt(profile: &Profile) -> String {
    let mut parts = Vec::new();

    // 1. Base system prompt
    parts.push("Ты — IziGhost, AI-ассистент для подготовки к собеседованиям. Твоя задача — помогать пользователю отвечать на вопросы, решать задачи и тренироваться перед интервью. Отвечай точно, структурированно и профессионально.".to_string());

    // 2. Profile system prompt
    if !profile.system_prompt.is_empty() {
        parts.push(format!("Инструкции профиля:\n{}", profile.system_prompt));
    }

    // 3. Facts
    if !profile.facts.is_empty() {
        parts.push(format!("Факты о кандидате:\n{}", profile.facts));
    }

    // 4. CV Text (truncated to avoid overwhelming LLM context)
    if !profile.cv_text.is_empty() {
        let truncated_cv = truncate_text(&profile.cv_text, 8000);
        parts.push(format!("Резюме кандидата:\n{}", truncated_cv));
    }

    // 5. Vacancy Text (truncated)
    if !profile.vacancy_text.is_empty() {
        let truncated_vacancy = truncate_text(&profile.vacancy_text, 8000);
        parts.push(format!("Описание вакансии:\n{}", truncated_vacancy));
    }

    // 6. Extra context
    if !profile.extra.is_empty() {
        parts.push(format!("Дополнительные факты:\n{}", profile.extra));
    }

    parts.join("\n\n")
}

fn truncate_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_string()
    } else {
        let truncated: String = text.chars().take(max_chars).collect();
        format!("{}... [Текст обрезан для экономии контекста]", truncated)
    }
}

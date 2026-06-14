use std::path::Path;
use reqwest::multipart;
use serde_json::Value;

pub async fn transcribe_audio(
    wav_path: &Path,
    profile: Option<&izighost_common::Profile>,
    api_key: &str,
) -> Result<String, anyhow::Error> {
    if !api_key.is_empty() {
        if let Some(p) = profile {
            tracing::info!(
                "Попытка отправки аудио на Whisper API (базовый URL: {}, модель: {})",
                p.asr.base_url,
                p.asr.model
            );
            match run_whisper_api(wav_path, &p.asr.base_url, &p.asr.model, api_key).await {
                Ok(text) => return Ok(text),
                Err(e) => {
                    tracing::warn!("Ошибка Whisper API, откат на локальный ASR: {:?}", e);
                }
            }
        }
    }

    // Если ключа нет или API завершилось ошибкой — запускаем локальный откат
    run_local_asr_fallback(wav_path).await
}

async fn run_whisper_api(
    wav_path: &Path,
    base_url: &str,
    model: &str,
    api_key: &str,
) -> Result<String, anyhow::Error> {
    let client = crate::get_http_client();
    let url = format!("{}/audio/transcriptions", base_url.trim_end_matches('/'));

    let file_bytes = tokio::fs::read(wav_path).await?;
    let part = multipart::Part::bytes(file_bytes)
        .file_name("recording.wav")
        .mime_str("audio/wav")?;

    let form = multipart::Form::new()
        .part("file", part)
        .text("model", model.to_string());

    let res = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .await?;

    if !res.status().is_success() {
        let err_text = res.text().await?;
        return Err(anyhow::anyhow!("Whisper API Error ({}): {}", url, err_text));
    }

    let json: Value = res.json().await?;
    let text = json["text"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Не удалось найти поле 'text' в ответе: {}", json))?
        .to_string();

    Ok(text)
}

async fn run_local_asr_fallback(wav_path: &Path) -> Result<String, anyhow::Error> {
    tracing::info!("Запуск локального ASR-отката для {:?}", wav_path);

    let home = std::env::var("HOME")
        .map_err(|_| anyhow::anyhow!("Переменная HOME не определена"))?;
    let cache_dir = std::path::PathBuf::from(home).join(".cache/izighost");
    tokio::fs::create_dir_all(&cache_dir).await?;

    let script_path = cache_dir.join("asr_fallback.py");
    let script_content = include_str!("audio/asr_fallback.py");
    tokio::fs::write(&script_path, script_content).await?;

    let output = tokio::process::Command::new("python3")
        .arg(&script_path)
        .arg(wav_path)
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Ошибка Python скрипта ASR: {}", stderr));
    }

    let text = String::from_utf8(output.stdout)?
        .trim()
        .to_string();

    Ok(text)
}

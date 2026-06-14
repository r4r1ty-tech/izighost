use std::path::{Path, PathBuf};
use std::process::Command;

struct DeleteOnDrop(Option<PathBuf>);
impl Drop for DeleteOnDrop {
    fn drop(&mut self) {
        if let Some(ref path) = self.0 {
            if path.exists() {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

/// Захват скриншота с виртуального монитора через GStreamer и pipewiresrc.
/// Сохраняет временный PNG файл и возвращает путь к нему.
pub fn capture_screenshot(node_id: u32) -> Result<PathBuf, anyhow::Error> {
    let timestamp = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let temp_path = std::env::temp_dir().join(format!("izighost_ocr_raw_{}.png", timestamp));

    tracing::info!(
        "Запуск захвата кадра с PipeWire ID {} в {:?}",
        node_id,
        temp_path
    );

    let status = Command::new("gst-launch-1.0")
        .arg("pipewiresrc")
        .arg(format!("path={}", node_id))
        .arg("num-buffers=1")
        .arg("!")
        .arg("videoconvert")
        .arg("!")
        .arg("pngenc")
        .arg("!")
        .arg("filesink")
        .arg(format!("location={}", temp_path.to_string_lossy()))
        .status()?;

    if !status.success() {
        return Err(anyhow::anyhow!("GStreamer pipeline завершился с ошибкой"));
    }

    Ok(temp_path)
}

/// Предобработка скриншота для повышения качества распознавания текста (OCR):
/// конвертация в Grayscale и бинаризация (порог яркости).
pub fn preprocess_image(input_path: &Path) -> Result<PathBuf, anyhow::Error> {
    let timestamp = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let output_path =
        std::env::temp_dir().join(format!("izighost_ocr_preprocessed_{}.png", timestamp));

    tracing::info!(
        "Предобработка скриншота из {:?} в {:?}",
        input_path,
        output_path
    );

    // Открываем исходное изображение
    let img = image::open(input_path)?;

    // Конвертируем в оттенки серого
    let mut grayscale = img.into_luma8();

    // Простая бинаризация по среднему порогу (128)
    for pixel in grayscale.pixels_mut() {
        if pixel.0[0] > 128 {
            pixel.0[0] = 255;
        } else {
            pixel.0[0] = 0;
        }
    }

    // Сохраняем обработанное изображение
    grayscale.save(&output_path)?;

    Ok(output_path)
}

/// Запуск Tesseract OCR на подготовленном скриншоте.
pub fn run_ocr(image_path: &Path, tessdata_dir: Option<PathBuf>) -> Result<String, anyhow::Error> {
    let fallback_dir = PathBuf::from("/usr/share/tesseract/tessdata");
    let actual_dir = match tessdata_dir {
        Some(ref d) => d.as_path(),
        None => fallback_dir.as_path(),
    };

    let has_rus = actual_dir.join("rus.traineddata").exists();
    let langs = if has_rus { "eng+rus" } else { "eng" };

    tracing::info!("Запуск Tesseract OCR (язык: {}) на {:?}", langs, image_path);

    let mut lt = leptess::LepTess::new(
        Some(
            actual_dir
                .to_str()
                .unwrap_or("/usr/share/tesseract/tessdata"),
        ),
        langs,
    )
    .map_err(|e| anyhow::anyhow!("Не удалось инициализировать Tesseract: {:?}", e))?;

    lt.set_image(image_path)
        .map_err(|e| anyhow::anyhow!("Не удалось загрузить изображение в Tesseract: {:?}", e))?;

    let text = lt
        .get_utf8_text()
        .map_err(|e| anyhow::anyhow!("Ошибка извлечения текста Tesseract: {:?}", e))?;

    Ok(text)
}

/// Автоматическое скачивание файлов Tesseract traineddata, если они отсутствуют.
async fn ensure_tessdata_downloaded() -> Result<PathBuf, anyhow::Error> {
    let home = std::env::var("HOME")
        .map_err(|_| anyhow::anyhow!("Переменная окружения HOME не задана"))?;
    let tessdata_dir = std::path::PathBuf::from(home).join(".cache/izighost/tessdata");

    std::fs::create_dir_all(&tessdata_dir)?;

    let files = ["eng.traineddata", "rus.traineddata"];
    for file in &files {
        let file_path = tessdata_dir.join(file);
        if !file_path.exists() {
            tracing::info!("Загрузка файла {} в {:?}", file, file_path);
            let url = format!(
                "https://github.com/tesseract-ocr/tessdata_fast/raw/main/{}",
                file
            );
            let response = reqwest::get(&url).await?;
            if !response.status().is_success() {
                return Err(anyhow::anyhow!(
                    "Ошибка загрузки файла tessdata ({}): {}",
                    file,
                    response.status()
                ));
            }
            let content = response.bytes().await?;
            std::fs::write(&file_path, content)?;
        }
    }

    Ok(tessdata_dir)
}

/// Весь пайплайн: захват -> предобработка -> OCR -> очистка временных файлов.
pub async fn trigger_ocr_pipeline(
    node_id: u32,
    profile: Option<izighost_common::Profile>,
) -> Result<String, anyhow::Error> {
    // 1. Захватываем кадр
    let raw_img_path = capture_screenshot(node_id)?;

    // 2. Распознаем
    run_ocr_on_file(raw_img_path, profile).await
}

/// Запуск OCR на готовом файле изображения (через Vision API или локально, с последующим удалением файлов).
pub async fn run_ocr_on_file(
    img_path: PathBuf,
    profile: Option<izighost_common::Profile>,
) -> Result<String, anyhow::Error> {
    let _img_guard = DeleteOnDrop(Some(img_path.clone()));

    // 1. Извлекаем API ключ из Keyring для Vision конфига
    let api_key = if let Some(ref p) = profile {
        if !p.id.is_empty() {
            izighost_common::KeyringStore::get_password(&format!("vision_api_key_{}", p.id))
                .await
                .ok()
                .flatten()
                .unwrap_or_default()
        } else {
            "".to_string()
        }
    } else {
        "".to_string()
    };

    let ocr_result = if !api_key.is_empty() {
        if let Some(ref p) = profile {
            // Пробуем распознать через Vision API (Groq/OpenAI) с настройками из vision конфига
            match run_vision_api_ocr(&img_path, &p.vision, &api_key).await {
                Ok(text) => {
                    let _ = std::fs::remove_file(&img_path);
                    text
                }
                Err(e) => {
                    // В случае ошибки (например, сбой сети) откатываемся на локальный Tesseract
                    tracing::warn!("Ошибка Vision API OCR, откат на Tesseract: {:?}", e);
                    run_local_tesseract_ocr_pipeline(img_path).await?
                }
            }
        } else {
            run_local_tesseract_ocr_pipeline(img_path).await?
        }
    } else {
        // Если ключ не задан, используем локальный Tesseract
        run_local_tesseract_ocr_pipeline(img_path).await?
    };

    Ok(ocr_result)
}

async fn run_local_tesseract_ocr_pipeline(img_path: PathBuf) -> Result<String, anyhow::Error> {
    let _img_guard = DeleteOnDrop(Some(img_path.clone()));

    // 1. Обеспечиваем наличие языковых файлов rus/eng
    let tessdata_dir = match ensure_tessdata_downloaded().await {
        Ok(dir) => Some(dir),
        Err(e) => {
            tracing::warn!(
                "Не удалось загрузить локальные файлы Tesseract: {:?}, используем системные",
                e
            );
            None
        }
    };

    // 2. Предобрабатываем
    let preprocessed_img_path = preprocess_image(&img_path)?;
    let _ = std::fs::remove_file(&img_path);

    // 3. Запускаем OCR
    let _prep_guard = DeleteOnDrop(Some(preprocessed_img_path.clone()));
    let ocr_result = run_ocr(&preprocessed_img_path, tessdata_dir)?;

    Ok(ocr_result)
}

/// Вызов Vision API (Groq / OpenAI) для распознавания текста на картинке
async fn run_vision_api_ocr(
    img_path: &Path,
    vision_config: &izighost_common::VisionConfig,
    api_key: &str,
) -> Result<String, anyhow::Error> {
    use base64::Engine;

    // 1. Считываем картинку и кодируем в Base64
    let img_bytes = std::fs::read(img_path)?;
    let base64_data = base64::engine::general_purpose::STANDARD.encode(img_bytes);

    // 2. Выбираем модель (для Groq используем Llama 4 Scout, для OpenAI — gpt-4o-mini)
    let configured_model = &vision_config.model;
    let base_url = &vision_config.base_url;
    let model = if configured_model.contains("vision")
        || configured_model.contains("gpt-4o")
        || configured_model.contains("scout")
    {
        configured_model
    } else if base_url.contains("groq") {
        "meta-llama/llama-4-scout-17b-16e-instruct"
    } else {
        "gpt-4o-mini"
    };

    // 3. Формируем запрос
    let client = reqwest::Client::new();
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));

    let image_url = format!("data:image/png;base64,{}", base64_data);

    // Строим content массив в зависимости от use_ocr_prompt
    let mut content = serde_json::json!([]);
    if vision_config.use_ocr_prompt {
        content.as_array_mut().unwrap().push(serde_json::json!({
            "type": "text",
            "text": vision_config.ocr_prompt
        }));
    }
    content.as_array_mut().unwrap().push(serde_json::json!({
        "type": "image_url",
        "image_url": {
            "url": image_url
        }
    }));

    let payload = serde_json::json!({
        "model": model,
        "messages": [
            {
                "role": "user",
                "content": content
            }
        ],
        "temperature": 0.0
    });

    let res = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&payload)
        .send()
        .await?;

    if !res.status().is_success() {
        let err_text = res.text().await?;
        return Err(anyhow::anyhow!("API Error ({}): {}", url, err_text));
    }

    let json: serde_json::Value = res.json().await?;
    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid response format: {}", json))?
        .trim()
        .to_string();

    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preprocess_and_ocr_with_mock_image() {
        // Создаем тестовое изображение (белый фон, черный квадрат/линии)
        let mut img = image::ImageBuffer::new(100, 100);
        for (x, y, pixel) in img.enumerate_pixels_mut() {
            if x > 30 && x < 70 && y > 30 && y < 70 {
                *pixel = image::Rgb([0u8, 0u8, 0u8]); // черный квадрат
            } else {
                *pixel = image::Rgb([255u8, 255u8, 255u8]); // белый фон
            }
        }

        let temp_raw = std::env::temp_dir().join("test_ocr_mock_raw.png");
        img.save(&temp_raw)
            .expect("Не удалось сохранить тестовую картинку");

        // Тестируем предобработку
        let preprocessed = preprocess_image(&temp_raw);
        assert!(preprocessed.is_ok());
        let preprocessed_path = preprocessed.unwrap();
        assert!(preprocessed_path.exists());

        // Запуск OCR на заглушке (может вернуть пустую строку, так как текста нет, но не должно падать)
        let ocr_res = run_ocr(&preprocessed_path, None);
        assert!(ocr_res.is_ok());

        // Чистим за собой
        let _ = std::fs::remove_file(&temp_raw);
        let _ = std::fs::remove_file(&preprocessed_path);
    }
}

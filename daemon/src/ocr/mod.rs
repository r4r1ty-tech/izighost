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

/// Захват скриншота с виртуального монитора через расширение GNOME Shell (или откат через GStreamer и pipewiresrc).
/// Сохраняет временный PNG файл и возвращает путь к нему.
pub async fn capture_screenshot(node_id: u32) -> Result<PathBuf, anyhow::Error> {
    static FILE_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let count = FILE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let pid = std::process::id();
    let timestamp = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let temp_path = std::env::temp_dir().join(format!(
        "izighost_ocr_raw_{}_{}_{}.png",
        timestamp, pid, count
    ));

    tracing::info!(
        "[ОКР] Запуск захвата кадра для PipeWire ID {} в {:?}",
        node_id,
        temp_path
    );

    // 1. Пытаемся захватить кадр через расширение GNOME
    let temp_path_str = temp_path.to_string_lossy().to_string();
    let mut ext_success = false;

    #[zbus::proxy(
        interface = "org.gnome.Shell.Extensions.WindowPinBridge",
        default_service = "org.gnome.Shell",
        default_path = "/org/gnome/Shell/Extensions/WindowPinBridge"
    )]
    pub trait WindowPinBridge {
        async fn capture_virtual_monitor(&self, filepath: &str) -> zbus::Result<bool>;
    }

    match zbus::Connection::session().await {
        Ok(conn) => match WindowPinBridgeProxy::new(&conn).await {
            Ok(proxy) => {
                tracing::info!(
                    "[ОКР] Отправка D-Bus запроса CaptureVirtualMonitor для файла: {}",
                    temp_path_str
                );
                match proxy.capture_virtual_monitor(&temp_path_str).await {
                    Ok(true) => {
                        tracing::info!("[ОКР] D-Bus запрос к CaptureVirtualMonitor успешно выполнен (вернул true).");
                        ext_success = true;
                    }
                    Ok(false) => {
                        tracing::warn!("[ОКР] Расширение GNOME вернуло false при вызове CaptureVirtualMonitor.");
                    }
                    Err(e) => {
                        tracing::warn!(
                            "[ОКР] Ошибка при вызове CaptureVirtualMonitor через D-Bus: {:?}",
                            e
                        );
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    "[ОКР] Не удалось создать WindowPinBridge D-Bus прокси: {:?}",
                    e
                );
            }
        },
        Err(e) => {
            tracing::warn!(
                "[ОКР] Не удалось установить сессионное D-Bus соединение для захвата: {:?}",
                e
            );
        }
    }

    if ext_success {
        tracing::info!(
            "[ОКР] Начало ожидания записи файла скриншота расширением GNOME (макс. 2 секунды)..."
        );
        let mut file_ready = false;
        for i in 0..200 {
            if temp_path.exists() {
                match tokio::fs::metadata(&temp_path).await {
                    Ok(metadata) => {
                        let size = metadata.len();
                        if size > 0 {
                            tracing::info!(
                                "[ОКР] Файл скриншота обнаружен на итерации {} (размер: {} байт).",
                                i,
                                size
                            );
                            file_ready = true;
                            break;
                        } else {
                            tracing::debug!(
                                "[ОКР] Файл создан, но его размер все еще 0 байт (попытка {})...",
                                i
                            );
                        }
                    }
                    Err(e) => {
                        tracing::debug!(
                            "[ОКР] Не удалось получить метаданные файла на итерации {}: {:?}",
                            i,
                            e
                        );
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        if file_ready {
            tracing::info!("[ОКР] Скриншот успешно записан расширением GNOME и готов к обработке.");
            return Ok(temp_path);
        } else {
            tracing::warn!("[ОКР] Превышено время ожидания записи файла расширением GNOME. Откатываемся на gst-launch-1.0.");
            let _ = tokio::fs::remove_file(&temp_path).await;
        }
    }

    // 2. Откат на gst-launch-1.0 (старый метод)
    tracing::info!("[ОКР] Запуск gst-launch-1.0 для захвата кадра...");
    let output = Command::new("gst-launch-1.0")
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
        .stderr(std::process::Stdio::piped())
        .output()?;

    if !output.status.success() {
        let stderr_msg = String::from_utf8_lossy(&output.stderr).to_string();
        let err_msg = format!(
            "GStreamer pipeline завершился с ошибкой: {}",
            stderr_msg.trim()
        );
        tracing::error!("[ОКР] {}", err_msg);
        return Err(anyhow::anyhow!(err_msg));
    }

    tracing::info!("[ОКР] Скриншот успешно захвачен через GStreamer.");
    Ok(temp_path)
}

/// Предобработка скриншота для повышения качества распознавания текста (OCR):
/// конвертация в Grayscale и бинаризация (порог яркости).
pub fn preprocess_image(input_path: &Path) -> Result<PathBuf, anyhow::Error> {
    let timestamp = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let output_path =
        std::env::temp_dir().join(format!("izighost_ocr_preprocessed_{}.png", timestamp));

    tracing::info!(
        "[ОКР:Предобработка] Начало обработки изображения. Исходный файл: {:?}, Целевой: {:?}",
        input_path,
        output_path
    );

    // Открываем исходное изображение
    let img = image::open(input_path).map_err(|e| {
        tracing::error!(
            "[ОКР:Предобработка] Не удалось открыть исходное изображение: {:?}",
            e
        );
        e
    })?;

    let width = img.width();
    let height = img.height();
    tracing::info!(
        "[ОКР:Предобработка] Изображение успешно загружено. Разрешение: {}x{}.",
        width,
        height
    );

    tracing::info!("[ОКР:Предобработка] Конвертация в оттенки серого (Grayscale)...");
    let mut grayscale = img.into_luma8();

    tracing::info!("[ОКР:Предобработка] Выполнение бинаризации пикселей с порогом яркости 128...");
    let mut black_pixels = 0;
    let mut white_pixels = 0;
    for pixel in grayscale.pixels_mut() {
        if pixel.0[0] > 128 {
            pixel.0[0] = 255;
            white_pixels += 1;
        } else {
            pixel.0[0] = 0;
            black_pixels += 1;
        }
    }
    tracing::info!(
        "[ОКР:Предобработка] Бинаризация завершена. Белых пикселей: {}, Черных: {}.",
        white_pixels,
        black_pixels
    );

    // Сохраняем обработанное изображение
    tracing::info!("[ОКР:Предобработка] Сохранение предобработанного файла на диск...");
    grayscale.save(&output_path).map_err(|e| {
        tracing::error!(
            "[ОКР:Предобработка] Ошибка сохранения обработанного файла: {:?}",
            e
        );
        e
    })?;

    tracing::info!("[ОКР:Предобработка] Предобработка успешно завершена.");
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

    tracing::info!(
        "[ОКР:Тессеракт] Инициализация Tesseract OCR (путь к данным: {:?}, языки: {}) на {:?}",
        actual_dir,
        langs,
        image_path
    );

    let mut lt = leptess::LepTess::new(
        Some(
            actual_dir
                .to_str()
                .unwrap_or("/usr/share/tesseract/tessdata"),
        ),
        langs,
    )
    .map_err(|e| {
        let err_msg = format!("Не удалось инициализировать Tesseract: {:?}", e);
        tracing::error!("[ОКР:Тессеракт] {}", err_msg);
        anyhow::anyhow!(err_msg)
    })?;

    tracing::info!("[ОКР:Тессеракт] Загрузка изображения в Tesseract...");
    lt.set_image(image_path).map_err(|e| {
        let err_msg = format!("Не удалось загрузить изображение в Tesseract: {:?}", e);
        tracing::error!("[ОКР:Тессеракт] {}", err_msg);
        anyhow::anyhow!(err_msg)
    })?;

    tracing::info!("[ОКР:Тессеракт] Извлечение текста (OCR)...");
    let text = lt.get_utf8_text().map_err(|e| {
        let err_msg = format!("Ошибка извлечения текста в Tesseract: {:?}", e);
        tracing::error!("[ОКР:Тессеракт] {}", err_msg);
        anyhow::anyhow!(err_msg)
    })?;

    tracing::info!(
        "[ОКР:Тессеракт] Tesseract OCR успешно выполнен. Извлечено {} символов.",
        text.len()
    );
    tracing::debug!("[ОКР:Тессеракт] Распознанный текст: {:?}", text);
    Ok(text)
}

fn get_expected_sha256(file: &str) -> &'static str {
    match file {
        "eng.traineddata" => "7d4322bd2a7749724879683fc3912cb542f19906c83bcc1a52132556427170b2",
        "rus.traineddata" => "e16e5e036cce1d9ec2b00063cf8b54472625b9e14d893a169e2b0dedeb4df225",
        _ => "",
    }
}

fn verify_file_sha256(path: &Path, expected_hex: &str) -> bool {
    use sha2::{Digest, Sha256};
    use std::io::Read;

    if expected_hex.is_empty() {
        return true;
    }

    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };

    let mut hasher = Sha256::new();
    let mut buffer = vec![0; 65536];
    loop {
        match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => hasher.update(&buffer[..n]),
            Err(_) => return false,
        }
    }

    let result = hasher.finalize();
    let hex_result = format!("{:x}", result);
    hex_result == expected_hex
}

/// Автоматическое скачивание файлов Tesseract traineddata, если они отсутствуют.
async fn ensure_tessdata_downloaded(cache_dir: &str) -> Result<PathBuf, anyhow::Error> {
    let tessdata_dir = crate::config::resolve_path(cache_dir).join("tessdata");

    tokio::fs::create_dir_all(&tessdata_dir).await?;

    let client = crate::get_http_client();

    let files = ["eng.traineddata", "rus.traineddata"];
    for file in &files {
        let file_path = tessdata_dir.join(file);
        let expected_hash = get_expected_sha256(file);

        let mut needs_download = true;
        if file_path.exists() {
            if verify_file_sha256(&file_path, expected_hash) {
                needs_download = false;
            } else {
                tracing::warn!(
                    "Файл {:?} поврежден или имеет неверный хэш. Повторное скачивание.",
                    file_path
                );
                let _ = tokio::fs::remove_file(&file_path).await;
            }
        }

        if needs_download {
            tracing::info!("Загрузка файла {} в {:?}", file, file_path);
            let url = format!(
                "https://github.com/tesseract-ocr/tessdata_fast/raw/main/{}",
                file
            );
            let response = client.get(&url).send().await?;
            if !response.status().is_success() {
                return Err(anyhow::anyhow!(
                    "Ошибка загрузки файла tessdata ({}): {}",
                    file,
                    response.status()
                ));
            }
            let content = response.bytes().await?;

            let temp_file_path = file_path.with_extension("download");
            tokio::fs::write(&temp_file_path, content).await?;

            if !verify_file_sha256(&temp_file_path, expected_hash) {
                let _ = tokio::fs::remove_file(&temp_file_path).await;
                return Err(anyhow::anyhow!(
                    "Ошибка целостности: хэш скачанного файла {} не совпадает с ожидаемым!",
                    file
                ));
            }

            tokio::fs::rename(&temp_file_path, &file_path).await?;
        }
    }

    Ok(tessdata_dir)
}

/// Весь пайплайн: захват -> предобработка -> OCR -> очистка временных файлов.
pub async fn trigger_ocr_pipeline(
    node_id: u32,
    profile: Option<izighost_common::Profile>,
    cache_dir: &str,
) -> Result<String, anyhow::Error> {
    // 1. Захватываем кадр
    let raw_img_path = capture_screenshot(node_id).await?;

    // 2. Распознаем
    run_ocr_on_file(raw_img_path, profile, cache_dir).await
}

/// Запуск OCR на готовом файле изображения (через Vision API или локально, с последующим удалением файлов).
pub async fn run_ocr_on_file(
    img_path: PathBuf,
    profile: Option<izighost_common::Profile>,
    cache_dir: &str,
) -> Result<String, anyhow::Error> {
    tracing::info!("[ОКР] Запуск распознавания (OCR) на файле: {:?}", img_path);
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
            tracing::info!(
                "[ОКР] Найден API ключ для Vision API. Попытка удаленного распознавания..."
            );
            // Пробуем распознать через Vision API (Groq/OpenAI) с настройками из vision конфига
            match run_vision_api_ocr(&img_path, &p.vision, &api_key).await {
                Ok(text) => text,
                Err(e) => {
                    // В случае ошибки (например, сбой сети) откатываемся на локальный Tesseract
                    tracing::warn!("[ОКР] Ошибка Vision API OCR ({:?}), выполняем откат на локальный Tesseract...", e);
                    run_local_tesseract_ocr_pipeline(img_path, cache_dir).await?
                }
            }
        } else {
            tracing::info!("[ОКР] Профиль отсутствует, используем локальный Tesseract...");
            run_local_tesseract_ocr_pipeline(img_path, cache_dir).await?
        }
    } else {
        // Если ключ не задан, используем локальный Tesseract
        tracing::info!("[ОКР] API ключ для Vision API не задан, используем локальный Tesseract...");
        run_local_tesseract_ocr_pipeline(img_path, cache_dir).await?
    };

    tracing::info!(
        "[ОКР] Распознавание завершено. Извлечено {} символов.",
        ocr_result.len()
    );
    Ok(ocr_result)
}

async fn run_local_tesseract_ocr_pipeline(
    img_path: PathBuf,
    cache_dir: &str,
) -> Result<String, anyhow::Error> {
    tracing::info!(
        "[ОКР] Запуск локального пайплайна Tesseract для изображения {:?}",
        img_path
    );
    // 1. Обеспечиваем наличие языковых файлов rus/eng
    let tessdata_dir = match ensure_tessdata_downloaded(cache_dir).await {
        Ok(dir) => Some(dir),
        Err(e) => {
            tracing::warn!(
                "[ОКР] Не удалось загрузить локальные файлы Tesseract: {:?}, используем системные пути",
                e
            );
            None
        }
    };

    // 2. Предобрабатываем
    let preprocessed_img_path = preprocess_image(&img_path)?;

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
    let img_bytes = tokio::fs::read(img_path).await?;
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

    tracing::info!(
        "[ОКР:VisionAPI] Отправка запроса к Vision API. Узел: {}, Модель: {}, Использовать OCR промпт: {}",
        base_url,
        model,
        vision_config.use_ocr_prompt
    );

    // 3. Формируем запрос
    let client = crate::get_http_client();
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
        let status = res.status();
        let err_text = res.text().await?;
        tracing::error!(
            "[ОКР:VisionAPI] Ошибка ответа Vision API ({}): {}",
            status,
            err_text
        );
        return Err(anyhow::anyhow!("API Error ({}): {}", url, err_text));
    }

    let json: serde_json::Value = res.json().await?;
    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| {
            let err_msg = format!("Неверный формат ответа API: {}", json);
            tracing::error!("[ОКР:VisionAPI] {}", err_msg);
            anyhow::anyhow!(err_msg)
        })?
        .trim()
        .to_string();

    tracing::info!(
        "[ОКР:VisionAPI] Ответ от Vision API успешно получен (символов: {}).",
        content.len()
    );
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

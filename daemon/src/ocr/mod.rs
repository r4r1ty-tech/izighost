use std::path::{Path, PathBuf};
use std::process::Command;

/// Захват скриншота с виртуального монитора через GStreamer и pipewiresrc.
/// Сохраняет временный PNG файл и возвращает путь к нему.
pub fn capture_screenshot(node_id: u32) -> Result<PathBuf, anyhow::Error> {
    let timestamp = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let temp_path = std::env::temp_dir().join(format!("izighost_ocr_raw_{}.png", timestamp));

    tracing::info!("Запуск захвата кадра с PipeWire ID {} в {:?}", node_id, temp_path);

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
    let output_path = std::env::temp_dir().join(format!("izighost_ocr_preprocessed_{}.png", timestamp));

    tracing::info!("Предобработка скриншота из {:?} в {:?}", input_path, output_path);

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
pub fn run_ocr(image_path: &Path) -> Result<String, anyhow::Error> {
    let tessdata_dir = "/usr/share/tesseract/tessdata";
    let has_rus = Path::new(tessdata_dir).join("rus.traineddata").exists();
    let langs = if has_rus { "eng+rus" } else { "eng" };

    tracing::info!("Запуск Tesseract OCR (язык: {}) на {:?}", langs, image_path);

    let mut lt = leptess::LepTess::new(Some(tessdata_dir), langs)
        .map_err(|e| anyhow::anyhow!("Не удалось инициализировать Tesseract: {:?}", e))?;

    lt.set_image(image_path)
        .map_err(|e| anyhow::anyhow!("Не удалось загрузить изображение в Tesseract: {:?}", e))?;

    let text = lt.get_utf8_text()
        .map_err(|e| anyhow::anyhow!("Ошибка извлечения текста Tesseract: {:?}", e))?;

    Ok(text)
}

/// Весь пайплайн: захват -> предобработка -> OCR -> очистка временных файлов.
pub async fn trigger_ocr_pipeline(node_id: u32) -> Result<String, anyhow::Error> {
    // 1. Захватываем кадр
    let raw_img_path = capture_screenshot(node_id)?;

    // 2. Предобрабатываем
    let preprocessed_img_path = match preprocess_image(&raw_img_path) {
        Ok(path) => {
            let _ = std::fs::remove_file(&raw_img_path);
            path
        }
        Err(e) => {
            let _ = std::fs::remove_file(&raw_img_path);
            return Err(e);
        }
    };

    // 3. Запускаем OCR
    let ocr_result = match run_ocr(&preprocessed_img_path) {
        Ok(text) => {
            let _ = std::fs::remove_file(&preprocessed_img_path);
            text
        }
        Err(e) => {
            let _ = std::fs::remove_file(&preprocessed_img_path);
            return Err(e);
        }
    };

    Ok(ocr_result)
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
        img.save(&temp_raw).expect("Не удалось сохранить тестовую картинку");

        // Тестируем предобработку
        let preprocessed = preprocess_image(&temp_raw);
        assert!(preprocessed.is_ok());
        let preprocessed_path = preprocessed.unwrap();
        assert!(preprocessed_path.exists());

        // Запуск OCR на заглушке (может вернуть пустую строку, так как текста нет, но не должно падать)
        let ocr_res = run_ocr(&preprocessed_path);
        assert!(ocr_res.is_ok());

        // Чистим за собой
        let _ = std::fs::remove_file(&temp_raw);
        let _ = std::fs::remove_file(&preprocessed_path);
    }
}

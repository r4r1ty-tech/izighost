use crate::config::resolve_path;
use izighost_common::IziError;
use kreuzberg::{extract_file, ExtractionConfig};

pub async fn parse_file(path_str: &str) -> Result<String, IziError> {
    let path = resolve_path(path_str);
    if !path.exists() {
        return Err(IziError::Profile(format!(
            "File not found: {}",
            path.display()
        )));
    }

    let config = ExtractionConfig::default();

    let result = extract_file(&path, None, &config).await.map_err(|e| {
        IziError::Profile(format!(
            "Failed to extract text from file {}: {}",
            path.display(),
            e
        ))
    })?;

    Ok(result.content)
}

pub mod parser;

use crate::config::{resolve_path, DaemonConfig};
use izighost_common::{IziError, Profile};
use std::fs;
use std::path::PathBuf;

pub struct ProfileManager {
    profiles_dir: PathBuf,
}

impl Default for ProfileManager {
    fn default() -> Self {
        Self::new(&DaemonConfig::default())
    }
}

impl ProfileManager {
    pub fn new(config: &DaemonConfig) -> Self {
        let profiles_dir = resolve_path(&config.general.data_dir).join("profiles");
        Self { profiles_dir }
    }

    pub fn list_profiles(&self) -> Result<Vec<String>, IziError> {
        if !self.profiles_dir.exists() {
            return Ok(Vec::new());
        }

        let mut ids = Vec::new();
        let entries = fs::read_dir(&self.profiles_dir).map_err(|e| {
            IziError::Profile(format!("Не удалось прочитать директорию профилей: {}", e))
        })?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file()
                && path
                    .extension()
                    .is_some_and(|ext| ext == "yaml" || ext == "yml")
            {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    ids.push(stem.to_string());
                }
            }
        }
        ids.sort();
        Ok(ids)
    }

    pub fn get_profile(&self, id: &str) -> Result<Profile, IziError> {
        let file_path = self.profiles_dir.join(format!("{}.yaml", id));
        if !file_path.exists() {
            return Err(IziError::Profile(format!("Профиль '{}' не найден", id)));
        }

        let content = fs::read_to_string(&file_path)
            .map_err(|e| IziError::Profile(format!("Не удалось прочитать файл профиля: {}", e)))?;

        let profile: Profile = serde_yaml::from_str(&content)
            .map_err(|e| IziError::Profile(format!("Не удалось разобрать YAML профиля: {}", e)))?;

        Ok(profile)
    }

    pub async fn save_profile(&self, mut profile: Profile) -> Result<Profile, IziError> {
        if !self.profiles_dir.exists() {
            fs::create_dir_all(&self.profiles_dir).map_err(|e| {
                IziError::Profile(format!("Не удалось создать директорию профилей: {}", e))
            })?;
        }

        let existing_profile = self.get_profile(&profile.id).ok();

        // Handle parsing CV if path is set and text is empty (or path changed)
        if !profile.cv_path.is_empty() {
            let parse_needed = match &existing_profile {
                Some(existing) => existing.cv_path != profile.cv_path || profile.cv_text.is_empty(),
                None => true,
            };
            if parse_needed {
                match parser::parse_file(&profile.cv_path).await {
                    Ok(text) => profile.cv_text = text,
                    Err(e) => {
                        return Err(IziError::Profile(format!(
                            "Ошибка парсинга резюме (CV): {}",
                            e
                        )));
                    }
                }
            }
        }

        // Handle parsing vacancy if path is set and text is empty
        if !profile.vacancy_path.is_empty() {
            let parse_needed = match &existing_profile {
                Some(existing) => {
                    existing.vacancy_path != profile.vacancy_path || profile.vacancy_text.is_empty()
                }
                None => true,
            };
            if parse_needed {
                match parser::parse_file(&profile.vacancy_path).await {
                    Ok(text) => profile.vacancy_text = text,
                    Err(e) => {
                        return Err(IziError::Profile(format!(
                            "Ошибка парсинга вакансии: {}",
                            e
                        )));
                    }
                }
            }
        }

        let file_path = self.profiles_dir.join(format!("{}.yaml", profile.id));
        let content = serde_yaml::to_string(&profile).map_err(|e| {
            IziError::Profile(format!("Не удалось сериализовать профиль в YAML: {}", e))
        })?;

        fs::write(&file_path, content)
            .map_err(|e| IziError::Profile(format!("Не удалось записать файл профиля: {}", e)))?;

        Ok(profile)
    }

    pub fn delete_profile(&self, id: &str) -> Result<(), IziError> {
        let file_path = self.profiles_dir.join(format!("{}.yaml", id));
        if file_path.exists() {
            fs::remove_file(&file_path).map_err(|e| {
                IziError::Profile(format!("Не удалось удалить файл профиля: {}", e))
            })?;
        }
        Ok(())
    }
}

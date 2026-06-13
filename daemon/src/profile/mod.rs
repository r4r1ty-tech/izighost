pub mod parser;

use std::fs;
use std::path::PathBuf;
use izighost_common::{Profile, IziError};
use crate::config::resolve_path;

pub struct ProfileManager {
    profiles_dir: PathBuf,
}

impl ProfileManager {
    pub fn new() -> Self {
        let profiles_dir = resolve_path("~/.config/izighost/profiles");
        Self { profiles_dir }
    }

    pub fn list_profiles(&self) -> Result<Vec<String>, IziError> {
        if !self.profiles_dir.exists() {
            return Ok(Vec::new());
        }
        
        let mut ids = Vec::new();
        let entries = fs::read_dir(&self.profiles_dir)
            .map_err(|e| IziError::Profile(format!("Failed to read profiles directory: {}", e)))?;
            
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_file() && path.extension().map_or(false, |ext| ext == "yaml" || ext == "yml") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        ids.push(stem.to_string());
                    }
                }
            }
        }
        ids.sort();
        Ok(ids)
    }

    pub fn get_profile(&self, id: &str) -> Result<Profile, IziError> {
        let file_path = self.profiles_dir.join(format!("{}.yaml", id));
        if !file_path.exists() {
            return Err(IziError::Profile(format!("Profile '{}' not found", id)));
        }
        
        let content = fs::read_to_string(&file_path)
            .map_err(|e| IziError::Profile(format!("Failed to read profile file: {}", e)))?;
            
        let profile: Profile = serde_yaml::from_str(&content)
            .map_err(|e| IziError::Profile(format!("Failed to parse profile YAML: {}", e)))?;
            
        Ok(profile)
    }

    pub async fn save_profile(&self, mut profile: Profile) -> Result<Profile, IziError> {
        if !self.profiles_dir.exists() {
            fs::create_dir_all(&self.profiles_dir)
                .map_err(|e| IziError::Profile(format!("Failed to create profiles directory: {}", e)))?;
        }
        
        // Handle parsing CV if path is set and text is empty (or path changed)
        if !profile.cv_path.is_empty() {
            let parse_needed = match self.get_profile(&profile.id) {
                Ok(existing) => existing.cv_path != profile.cv_path || profile.cv_text.is_empty(),
                Err(_) => true,
            };
            if parse_needed {
                match parser::parse_file(&profile.cv_path).await {
                    Ok(text) => profile.cv_text = text,
                    Err(e) => {
                        return Err(IziError::Profile(format!("CV parsing failed: {}", e)));
                    }
                }
            }
        } else {
            profile.cv_text = "".to_string();
        }

        // Handle parsing vacancy if path is set and text is empty
        if !profile.vacancy_path.is_empty() {
            let parse_needed = match self.get_profile(&profile.id) {
                Ok(existing) => existing.vacancy_path != profile.vacancy_path || profile.vacancy_text.is_empty(),
                Err(_) => true,
            };
            if parse_needed {
                match parser::parse_file(&profile.vacancy_path).await {
                    Ok(text) => profile.vacancy_text = text,
                    Err(e) => {
                        return Err(IziError::Profile(format!("Vacancy parsing failed: {}", e)));
                    }
                }
            }
        } else {
            profile.vacancy_text = "".to_string();
        }

        let file_path = self.profiles_dir.join(format!("{}.yaml", profile.id));
        let content = serde_yaml::to_string(&profile)
            .map_err(|e| IziError::Profile(format!("Failed to serialize profile to YAML: {}", e)))?;
            
        fs::write(&file_path, content)
            .map_err(|e| IziError::Profile(format!("Failed to write profile file: {}", e)))?;
            
        Ok(profile)
    }

    pub fn delete_profile(&self, id: &str) -> Result<(), IziError> {
        let file_path = self.profiles_dir.join(format!("{}.yaml", id));
        if file_path.exists() {
            fs::remove_file(&file_path)
                .map_err(|e| IziError::Profile(format!("Failed to delete profile file: {}", e)))?;
        }
        Ok(())
    }
}

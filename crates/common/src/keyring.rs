use crate::error::{IziError, Result};
use secret_service::{EncryptionType, SecretService};
use std::collections::HashMap;

pub struct KeyringStore;

impl KeyringStore {
    pub async fn get_password(key: &str) -> Result<Option<String>> {
        let ss = SecretService::connect(EncryptionType::Dh)
            .await
            .map_err(|e| {
                IziError::Keyring(format!("Failed to connect to Secret Service: {}", e))
            })?;

        let collection = ss
            .get_default_collection()
            .await
            .map_err(|e| IziError::Keyring(format!("Failed to get default collection: {}", e)))?;

        let mut attributes = HashMap::new();
        attributes.insert("application", "izighost");
        attributes.insert("key", key);

        let items = collection
            .search_items(attributes)
            .await
            .map_err(|e| IziError::Keyring(format!("Failed to search items: {}", e)))?;

        if let Some(item) = items.first() {
            let secret_bytes = item
                .get_secret()
                .await
                .map_err(|e| IziError::Keyring(format!("Failed to get secret: {}", e)))?;
            let secret = String::from_utf8(secret_bytes).map_err(|e| {
                IziError::Keyring(format!("Failed to parse secret as UTF-8: {}", e))
            })?;
            Ok(Some(secret))
        } else {
            Ok(None)
        }
    }

    pub async fn set_password(key: &str, password: &str) -> Result<()> {
        let ss = SecretService::connect(EncryptionType::Dh)
            .await
            .map_err(|e| {
                IziError::Keyring(format!("Failed to connect to Secret Service: {}", e))
            })?;

        let collection = ss
            .get_default_collection()
            .await
            .map_err(|e| IziError::Keyring(format!("Failed to get default collection: {}", e)))?;

        let mut attributes = HashMap::new();
        attributes.insert("application", "izighost");
        attributes.insert("key", key);

        let items = collection
            .search_items(attributes.clone())
            .await
            .map_err(|e| IziError::Keyring(format!("Failed to search items: {}", e)))?;
        for item in items {
            item.delete()
                .await
                .map_err(|e| IziError::Keyring(format!("Failed to delete old key: {}", e)))?;
        }

        let label = format!("IziGhost Key: {}", key);
        collection
            .create_item(
                &label,
                attributes,
                password.as_bytes(),
                true, // replace
                "text/plain",
            )
            .await
            .map_err(|e| IziError::Keyring(format!("Failed to create keyring item: {}", e)))?;

        Ok(())
    }

    pub async fn delete_password(key: &str) -> Result<()> {
        let ss = SecretService::connect(EncryptionType::Dh)
            .await
            .map_err(|e| {
                IziError::Keyring(format!("Failed to connect to Secret Service: {}", e))
            })?;

        let collection = ss
            .get_default_collection()
            .await
            .map_err(|e| IziError::Keyring(format!("Failed to get default collection: {}", e)))?;

        let mut attributes = HashMap::new();
        attributes.insert("application", "izighost");
        attributes.insert("key", key);

        let items = collection
            .search_items(attributes)
            .await
            .map_err(|e| IziError::Keyring(format!("Failed to search items: {}", e)))?;
        for item in items {
            item.delete()
                .await
                .map_err(|e| IziError::Keyring(format!("Failed to delete key: {}", e)))?;
        }

        Ok(())
    }
}

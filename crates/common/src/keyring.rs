use crate::error::{IziError, Result};
use secret_service::{EncryptionType, SecretService};
use std::collections::HashMap;

/// Хранилище конфиденциальных данных и API-ключей на базе системной службы Secret Service (GNOME Keyring).
pub struct KeyringStore;

impl KeyringStore {
    /// Получить сохраненный пароль/API-ключ из системной связки ключей по заданному ключу.
    ///
    /// # Аргументы
    /// * `key` - Имя ключа для поиска.
    pub async fn get_password(key: &str) -> Result<Option<String>> {
        let ss = SecretService::connect(EncryptionType::Dh)
            .await
            .map_err(|e| {
                IziError::Keyring(format!("Не удалось подключиться к Secret Service: {}", e))
            })?;

        let collection = ss
            .get_default_collection()
            .await
            .map_err(|e| IziError::Keyring(format!("Не удалось получить стандартную коллекцию: {}", e)))?;

        let mut attributes = HashMap::new();
        attributes.insert("application", "izighost");
        attributes.insert("key", key);

        let items = collection
            .search_items(attributes)
            .await
            .map_err(|e| IziError::Keyring(format!("Не удалось выполнить поиск элементов: {}", e)))?;

        if let Some(item) = items.first() {
            let secret_bytes = item
                .get_secret()
                .await
                .map_err(|e| IziError::Keyring(format!("Не удалось получить секрет: {}", e)))?;
            let secret = String::from_utf8(secret_bytes).map_err(|e| {
                IziError::Keyring(format!("Не удалось преобразовать секрет в UTF-8: {}", e))
            })?;
            Ok(Some(secret))
        } else {
            Ok(None)
        }
    }

    /// Сохранить пароль/API-ключ в системной связке ключей с перезаписью существующего значения.
    ///
    /// # Аргументы
    /// * `key` - Имя ключа.
    /// * `password` - Значение пароля/ключа для сохранения.
    pub async fn set_password(key: &str, password: &str) -> Result<()> {
        let ss = SecretService::connect(EncryptionType::Dh)
            .await
            .map_err(|e| {
                IziError::Keyring(format!("Не удалось подключиться к Secret Service: {}", e))
            })?;

        let collection = ss
            .get_default_collection()
            .await
            .map_err(|e| IziError::Keyring(format!("Не удалось получить стандартную коллекцию: {}", e)))?;

        let mut attributes = HashMap::new();
        attributes.insert("application", "izighost");
        attributes.insert("key", key);

        let items = collection
            .search_items(attributes.clone())
            .await
            .map_err(|e| IziError::Keyring(format!("Не удалось выполнить поиск элементов: {}", e)))?;
        for item in items {
            item.delete()
                .await
                .map_err(|e| IziError::Keyring(format!("Не удалось удалить старый ключ: {}", e)))?;
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
            .map_err(|e| IziError::Keyring(format!("Не удалось создать элемент в связке ключей: {}", e)))?;

        Ok(())
    }

    /// Удалить пароль/API-ключ из системной связки ключей.
    ///
    /// # Аргументы
    /// * `key` - Имя ключа для удаления.
    pub async fn delete_password(key: &str) -> Result<()> {
        let ss = SecretService::connect(EncryptionType::Dh)
            .await
            .map_err(|e| {
                IziError::Keyring(format!("Не удалось подключиться к Secret Service: {}", e))
            })?;

        let collection = ss
            .get_default_collection()
            .await
            .map_err(|e| IziError::Keyring(format!("Не удалось получить стандартную коллекцию: {}", e)))?;

        let mut attributes = HashMap::new();
        attributes.insert("application", "izighost");
        attributes.insert("key", key);

        let items = collection
            .search_items(attributes)
            .await
            .map_err(|e| IziError::Keyring(format!("Не удалось выполнить поиск элементов: {}", e)))?;
        for item in items {
            item.delete()
                .await
                .map_err(|e| IziError::Keyring(format!("Не удалось удалить ключ: {}", e)))?;
        }

        Ok(())
    }
}

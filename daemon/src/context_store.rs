use izighost_common::Profile;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::io::AsyncWriteExt;
use aes::cipher::KeyIvInit;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

pub struct ContextStoreInner {
    pub active_profile: Option<Profile>,
    pub chat_history: Arc<Vec<ChatMessage>>,
    pub last_preview: Option<String>,
    pub data_dir: String,
}

#[derive(Clone)]
pub struct ContextStore {
    inner: Arc<RwLock<ContextStoreInner>>,
}

impl Default for ContextStore {
    fn default() -> Self {
        Self::new("~/.local/share/izighost")
    }
}

impl ContextStore {
    pub fn new(data_dir: &str) -> Self {
        Self {
            inner: Arc::new(RwLock::new(ContextStoreInner {
                active_profile: None,
                chat_history: Arc::new(Vec::new()),
                last_preview: None,
                data_dir: data_dir.to_string(),
            })),
        }
    }

    pub async fn set_active_profile(&self, profile: Option<Profile>) {
        let mut inner = self.inner.write().await;
        inner.active_profile = profile;
        inner.chat_history = Arc::new(Vec::new());
        inner.last_preview = None;

        if let Some(ref p) = inner.active_profile {
            let history = Self::load_history_from_disk(&inner.data_dir, &p.id).await;
            inner.chat_history = Arc::new(history);
        }
    }

    pub async fn get_active_profile(&self) -> Option<Profile> {
        let inner = self.inner.read().await;
        inner.active_profile.clone()
    }

    pub async fn add_message(&self, role: String, content: String) {
        let mut inner = self.inner.write().await;
        
        let max_messages = inner
            .active_profile
            .as_ref()
            .map(|p| p.llm.max_context_messages as usize)
            .unwrap_or(20);

        let active_profile_id = inner.active_profile.as_ref().map(|p| p.id.clone());
        let data_dir = inner.data_dir.clone();

        let history_vec = Arc::make_mut(&mut inner.chat_history);
        history_vec.push(ChatMessage { role, content });

        if history_vec.len() > max_messages {
            let drain_count = history_vec.len() - max_messages;
            history_vec.drain(0..drain_count);
        }

        if let Some(profile_id) = active_profile_id {
            let chat_history = inner.chat_history.clone();
            drop(inner);
            Self::save_history_to_disk(&data_dir, &profile_id, &chat_history).await;
        }
    }

    pub async fn get_history(&self) -> Arc<Vec<ChatMessage>> {
        let inner = self.inner.read().await;
        inner.chat_history.clone()
    }

    pub async fn clear_chat(&self) {
        let mut inner = self.inner.write().await;
        inner.chat_history = Arc::new(Vec::new());
        inner.last_preview = None;
        if let Some(ref p) = inner.active_profile {
            Self::save_history_to_disk(&inner.data_dir, &p.id, &inner.chat_history).await;
        }
    }

    pub async fn set_last_preview(&self, preview: Option<String>) {
        let mut inner = self.inner.write().await;
        inner.last_preview = preview;
    }

    pub async fn get_last_preview(&self) -> Option<String> {
        let inner = self.inner.read().await;
        inner.last_preview.clone()
    }

    pub async fn take_last_preview(&self) -> Option<String> {
        let mut inner = self.inner.write().await;
        inner.last_preview.take()
    }

    // --- Disk operations ---

    async fn save_history_to_disk(data_dir: &str, profile_id: &str, history: &[ChatMessage]) {
        let path = crate::config::resolve_path(data_dir);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = std::fs::DirBuilder::new();
            builder.recursive(true).mode(0o700);
            if let Err(e) = builder.create(&path) {
                tracing::error!("Failed to create data directory {:?}: {:?}", path, e);
                return;
            }
        }
        #[cfg(not(unix))]
        {
            if let Err(e) = tokio::fs::create_dir_all(&path).await {
                tracing::error!("Failed to create data directory {:?}: {:?}", path, e);
                return;
            }
        }

        let file_path = path.join(format!("history_{}.json", profile_id));
        let json_str = match serde_json::to_string(history) {
            Ok(json) => json,
            Err(e) => {
                tracing::error!("Failed to serialize history: {:?}", e);
                return;
            }
        };

        let mut final_data = Vec::new();
        if let Some(key) = get_or_create_history_key(profile_id).await {
            let mut iv = vec![0u8; 16];
            let mut rng_file = match std::fs::File::open("/dev/urandom") {
                Ok(f) => f,
                Err(_) => {
                    tracing::error!("Failed to open /dev/urandom for IV generation");
                    return;
                }
            };
            use std::io::Read;
            if rng_file.read_exact(&mut iv).is_ok() {
                let ciphertext = encrypt_aes256_cbc(&key, &iv, json_str.as_bytes());
                final_data.extend_from_slice(ENCRYPTED_MAGIC);
                final_data.extend_from_slice(&iv);
                final_data.extend_from_slice(&ciphertext);
            } else {
                final_data.extend_from_slice(json_str.as_bytes());
            }
        } else {
            final_data.extend_from_slice(json_str.as_bytes());
        }

        let mut file_opts = tokio::fs::OpenOptions::new();
        file_opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            file_opts.mode(0o600);
        }
        let mut file = match file_opts.open(&file_path).await {
            Ok(f) => f,
            Err(e) => {
                tracing::error!("Failed to open/create history file {:?}: {:?}", file_path, e);
                return;
            }
        };
        if let Err(e) = file.write_all(&final_data).await {
            tracing::error!("Failed to write history file {:?}: {:?}", file_path, e);
        }
    }

    async fn load_history_from_disk(data_dir: &str, profile_id: &str) -> Vec<ChatMessage> {
        let file_path = crate::config::resolve_path(data_dir).join(format!("history_{}.json", profile_id));
        if !file_path.exists() {
            return Vec::new();
        }
        let content_bytes = match tokio::fs::read(&file_path).await {
            Ok(bytes) => bytes,
            Err(e) => {
                tracing::error!("Failed to read history file {:?}: {:?}", file_path, e);
                return Vec::new();
            }
        };

        if content_bytes.starts_with(ENCRYPTED_MAGIC) {
            if content_bytes.len() < ENCRYPTED_MAGIC.len() + 16 {
                tracing::error!("Corrupted encrypted history file: too short");
                return Vec::new();
            }
            if let Some(key) = get_or_create_history_key(profile_id).await {
                let iv = &content_bytes[ENCRYPTED_MAGIC.len()..ENCRYPTED_MAGIC.len() + 16];
                let ciphertext = &content_bytes[ENCRYPTED_MAGIC.len() + 16..];
                if let Some(plaintext) = decrypt_aes256_cbc(&key, iv, ciphertext) {
                    match serde_json::from_slice(&plaintext) {
                        Ok(history) => history,
                        Err(e) => {
                            tracing::error!("Failed to parse decrypted history: {:?}", e);
                            Vec::new()
                        }
                    }
                } else {
                    tracing::error!("Failed to decrypt history file {:?}", file_path);
                    Vec::new()
                }
            } else {
                tracing::error!("Keyring key for history encryption is not available. Cannot decrypt history file {:?}", file_path);
                Vec::new()
            }
        } else {
            match serde_json::from_slice(&content_bytes) {
                Ok(history) => history,
                Err(e) => {
                    tracing::error!("Failed to parse legacy history file {:?}: {:?}", file_path, e);
                    Vec::new()
                }
            }
        }
    }
}

const ENCRYPTED_MAGIC: &[u8] = b"IZIGH_ENC_V1";

fn decode_hex(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    if s.len() % 2 != 0 {
        return None;
    }
    let mut bytes = Vec::new();
    for i in (0..s.len()).step_by(2) {
        let res = u8::from_str_radix(&s[i..i+2], 16).ok()?;
        bytes.push(res);
    }
    Some(bytes)
}

fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn encrypt_aes256_cbc(key: &[u8], iv: &[u8], plaintext: &[u8]) -> Vec<u8> {
    use aes::cipher::{block_padding::Pkcs7, BlockEncryptMut};
    type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;

    let enc = Aes256CbcEnc::new(key.into(), iv.into());
    enc.encrypt_padded_vec_mut::<Pkcs7>(plaintext)
}

fn decrypt_aes256_cbc(key: &[u8], iv: &[u8], ciphertext: &[u8]) -> Option<Vec<u8>> {
    use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut};
    type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

    let dec = Aes256CbcDec::new(key.into(), iv.into());
    dec.decrypt_padded_vec_mut::<Pkcs7>(ciphertext).ok()
}

async fn get_or_create_history_key(profile_id: &str) -> Option<Vec<u8>> {
    let key_name = format!("history_key_{}", profile_id);
    match izighost_common::KeyringStore::get_password(&key_name).await {
        Ok(Some(hex_key)) => {
            if let Some(key) = decode_hex(&hex_key) {
                if key.len() == 32 {
                    return Some(key);
                }
            }
        }
        _ => {}
    }

    // Generate a new 32-byte key
    let mut key = vec![0u8; 32];
    let mut rng_file = match std::fs::File::open("/dev/urandom") {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!("Failed to open /dev/urandom: {:?}", e);
            return None;
        }
    };
    use std::io::Read;
    if rng_file.read_exact(&mut key).is_err() {
        return None;
    }

    let hex_key = encode_hex(&key);
    if izighost_common::KeyringStore::set_password(&key_name, &hex_key).await.is_ok() {
        Some(key)
    } else {
        tracing::warn!("Failed to store history key in keyring. Storing history without encryption.");
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_history_encryption_roundtrip() {
        let temp_dir = std::env::temp_dir().join(format!("izighost_test_{}", chrono::Utc::now().timestamp_millis()));
        let data_dir = temp_dir.to_str().unwrap();

        let history = vec![
            ChatMessage {
                role: "user".to_string(),
                content: "Hello, world!".to_string(),
            },
            ChatMessage {
                role: "assistant".to_string(),
                content: "Hi there!".to_string(),
            },
        ];

        let profile_id = "test_profile";

        // Save history.
        ContextStore::save_history_to_disk(data_dir, profile_id, &history).await;

        let loaded = ContextStore::load_history_from_disk(data_dir, profile_id).await;
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].role, "user");
        assert_eq!(loaded[0].content, "Hello, world!");
        assert_eq!(loaded[1].role, "assistant");
        assert_eq!(loaded[1].content, "Hi there!");

        // Test legacy plaintext fallback.
        let file_path = temp_dir.join(format!("history_{}.json", profile_id));
        let plaintext_json = serde_json::to_string(&history).unwrap();
        tokio::fs::write(&file_path, plaintext_json).await.unwrap();

        // Load legacy plaintext.
        let loaded_legacy = ContextStore::load_history_from_disk(data_dir, profile_id).await;
        assert_eq!(loaded_legacy.len(), 2);
        assert_eq!(loaded_legacy[0].role, "user");

        // Clean up.
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }

    #[test]
    fn test_hex_encoding() {
        let bytes = b"hello world";
        let hex_str = encode_hex(bytes);
        assert_eq!(hex_str, "68656c6c6f20776f726c64");
        let decoded = decode_hex(&hex_str).unwrap();
        assert_eq!(&decoded, bytes);
    }

    #[test]
    fn test_aes_encryption_decryption() {
        let key = vec![0x42u8; 32];
        let iv = vec![0x24u8; 16];
        let plaintext = b"This is a secret message that needs to be encrypted.";

        let encrypted = encrypt_aes256_cbc(&key, &iv, plaintext);
        assert_ne!(encrypted, plaintext);

        let decrypted = decrypt_aes256_cbc(&key, &iv, &encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }
}

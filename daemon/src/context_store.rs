use izighost_common::Profile;
use std::sync::Arc;
use tokio::sync::RwLock;

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
        if let Err(e) = tokio::fs::create_dir_all(&path).await {
            tracing::error!("Failed to create data directory {:?}: {:?}", path, e);
            return;
        }
        let file_path = path.join(format!("history_{}.json", profile_id));
        match serde_json::to_string(history) {
            Ok(json) => {
                if let Err(e) = tokio::fs::write(&file_path, json).await {
                    tracing::error!("Failed to write history file {:?}: {:?}", file_path, e);
                }
            }
            Err(e) => {
                tracing::error!("Failed to serialize history: {:?}", e);
            }
        }
    }

    async fn load_history_from_disk(data_dir: &str, profile_id: &str) -> Vec<ChatMessage> {
        let file_path = crate::config::resolve_path(data_dir).join(format!("history_{}.json", profile_id));
        if !file_path.exists() {
            return Vec::new();
        }
        match tokio::fs::read_to_string(&file_path).await {
            Ok(content) => {
                match serde_json::from_str(&content) {
                    Ok(history) => history,
                    Err(e) => {
                        tracing::error!("Failed to parse history file {:?}: {:?}", file_path, e);
                        Vec::new()
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to read history file {:?}: {:?}", file_path, e);
                Vec::new()
            }
        }
    }
}

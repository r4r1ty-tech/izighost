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
    pub chat_history: Vec<ChatMessage>,
    pub last_preview: Option<String>,
}

#[derive(Clone)]
pub struct ContextStore {
    inner: Arc<RwLock<ContextStoreInner>>,
}

impl Default for ContextStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextStore {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(ContextStoreInner {
                active_profile: None,
                chat_history: Vec::new(),
                last_preview: None,
            })),
        }
    }

    pub async fn set_active_profile(&self, profile: Option<Profile>) {
        let mut inner = self.inner.write().await;
        inner.active_profile = profile;
        // Optionally clear history when changing profile
        inner.chat_history.clear();
        inner.last_preview = None;
    }

    pub async fn get_active_profile(&self) -> Option<Profile> {
        let inner = self.inner.read().await;
        inner.active_profile.clone()
    }

    pub async fn add_message(&self, role: String, content: String) {
        let mut inner = self.inner.write().await;
        inner.chat_history.push(ChatMessage { role, content });
    }

    pub async fn get_history(&self) -> Vec<ChatMessage> {
        let inner = self.inner.read().await;
        inner.chat_history.clone()
    }

    pub async fn clear_chat(&self) {
        let mut inner = self.inner.write().await;
        inner.chat_history.clear();
        inner.last_preview = None;
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
}

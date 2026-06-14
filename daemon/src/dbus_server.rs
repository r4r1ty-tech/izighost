use crate::context_store::ContextStore;
use crate::profile::ProfileManager;
use crate::rvms::RvmsManager;
use izighost_common::Profile;
use zbus::{interface, object_server::SignalEmitter};

pub struct DaemonInterface {
    profile_manager: ProfileManager,
    context_store: ContextStore,
    rvms_manager: RvmsManager,
}

impl DaemonInterface {
    pub fn new(
        profile_manager: ProfileManager,
        context_store: ContextStore,
        rvms_manager: RvmsManager,
    ) -> Self {
        Self {
            profile_manager,
            context_store,
            rvms_manager,
        }
    }
}

#[interface(name = "com.izighost.Daemon")]
impl DaemonInterface {
    async fn start_rvms(&self) -> zbus::fdo::Result<u32> {
        self.rvms_manager
            .start()
            .await
            .map_err(zbus::fdo::Error::Failed)
    }

    async fn stop_rvms(&self) -> zbus::fdo::Result<()> {
        self.rvms_manager
            .stop()
            .await
            .map_err(zbus::fdo::Error::Failed)
    }

    async fn send_chat_message(
        &self,
        text: String,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        // Сохраняем сообщение пользователя в историю
        self.context_store
            .add_message("user".to_string(), text.clone())
            .await;

        // Имитируем стриминг ответа от LLM
        let response_text = format!("Эхо-ответ от демона на ваш вопрос: '{}'", text);

        // Отправляем ответ по словам (чанками)
        for word in response_text.split_whitespace() {
            let chunk = format!("{} ", word);
            Self::chat_chunk(&emitter, &chunk)
                .await
                .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        self.context_store
            .add_message("assistant".to_string(), response_text)
            .await;

        Self::chat_completed(&emitter)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        Ok(())
    }

    async fn trigger_ocr(
        &self,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        let node_id = match self.rvms_manager.get_pipewire_node_id().await {
            Some(id) => id,
            None => {
                let err_msg =
                    "Виртуальный экран не активен. Сначала запустите RVMS сессию в настройках.";
                let _ = Self::error_occurred(&emitter, err_msg).await;
                return Err(zbus::fdo::Error::Failed(err_msg.to_string()));
            }
        };

        // Запускаем OCR пайплайн в фоновом Tokio-потоке
        let emitter_clone = emitter.clone().into_owned();
        let context_store = self.context_store.clone();
        let profile = self.context_store.get_active_profile().await;
        tokio::spawn(async move {
            match crate::ocr::trigger_ocr_pipeline(node_id, profile).await {
                Ok(text) => {
                    context_store.set_last_preview(Some(text.clone())).await;
                    if let Err(e) = Self::ocr_completed(&emitter_clone, &text).await {
                        tracing::error!("Ошибка отправки D-Bus сигнала ocr_completed: {:?}", e);
                    }
                }
                Err(e) => {
                    let err_msg = format!("Ошибка распознавания текста (OCR): {}", e);
                    tracing::error!("{}", err_msg);
                    if let Err(sig_err) = Self::error_occurred(&emitter_clone, &err_msg).await {
                        tracing::error!(
                            "Ошибка отправки D-Bus сигнала error_occurred: {:?}",
                            sig_err
                        );
                    }
                }
            }
        });

        Ok(())
    }

    async fn trigger_ocr_from_file(
        &self,
        file_path: String,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        let emitter_clone = emitter.clone().into_owned();
        let context_store = self.context_store.clone();
        let path = std::path::PathBuf::from(file_path);
        let profile = self.context_store.get_active_profile().await;

        tokio::spawn(async move {
            match crate::ocr::run_ocr_on_file(path, profile).await {
                Ok(text) => {
                    context_store.set_last_preview(Some(text.clone())).await;
                    if let Err(e) = Self::ocr_completed(&emitter_clone, &text).await {
                        tracing::error!("Ошибка отправки D-Bus сигнала ocr_completed: {:?}", e);
                    }
                }
                Err(e) => {
                    let err_msg = format!("Ошибка распознавания текста (OCR): {}", e);
                    tracing::error!("{}", err_msg);
                    if let Err(sig_err) = Self::error_occurred(&emitter_clone, &err_msg).await {
                        tracing::error!(
                            "Ошибка отправки D-Bus сигнала error_occurred: {:?}",
                            sig_err
                        );
                    }
                }
            }
        });

        Ok(())
    }

    async fn start_listening(&self) -> zbus::fdo::Result<()> {
        Ok(())
    }

    async fn stop_listening(
        &self,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        // Имитируем распознавание речи
        let mock_asr = "Это текст, распознанный из вашего голоса (ASR заглушка).";

        self.context_store
            .set_last_preview(Some(mock_asr.to_string()))
            .await;

        Self::asr_completed(&emitter, mock_asr)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        Ok(())
    }

    // --- Profile Management ---

    async fn list_profiles(&self) -> zbus::fdo::Result<Vec<String>> {
        self.profile_manager
            .list_profiles()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    async fn get_profile(&self, id: String) -> zbus::fdo::Result<Profile> {
        self.profile_manager
            .get_profile(&id)
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    async fn save_profile(&self, profile: Profile) -> zbus::fdo::Result<Profile> {
        self.profile_manager
            .save_profile(profile)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    async fn delete_profile(&self, id: String) -> zbus::fdo::Result<()> {
        self.profile_manager
            .delete_profile(&id)
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    async fn set_active_profile(&self, id: String) -> zbus::fdo::Result<()> {
        let profile = self
            .profile_manager
            .get_profile(&id)
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        self.context_store.set_active_profile(Some(profile)).await;
        Ok(())
    }

    async fn get_active_profile(&self) -> zbus::fdo::Result<Profile> {
        Ok(self
            .context_store
            .get_active_profile()
            .await
            .unwrap_or_default())
    }

    // --- Signals ---

    #[zbus(signal)]
    pub async fn chat_chunk(emitter: &SignalEmitter<'_>, delta_text: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn chat_completed(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn ocr_completed(emitter: &SignalEmitter<'_>, text: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn asr_completed(emitter: &SignalEmitter<'_>, text: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn error_occurred(emitter: &SignalEmitter<'_>, message: &str) -> zbus::Result<()>;
}

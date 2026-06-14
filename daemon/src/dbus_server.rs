use crate::config::DaemonConfig;
use crate::context_store::ContextStore;
use crate::profile::ProfileManager;
use crate::rvms::RvmsManager;
use izighost_common::Profile;
use zbus::{interface, object_server::SignalEmitter};

/// Интерфейс D-Bus сервера IziGhost для связи между GUI и демоном.
pub struct DaemonInterface {
    profile_manager: ProfileManager,
    context_store: ContextStore,
    rvms_manager: RvmsManager,
    _config: DaemonConfig,
    cancel_generation: std::sync::Arc<std::sync::atomic::AtomicBool>,
    recording_state: tokio::sync::Mutex<Option<(tokio::process::Child, std::path::PathBuf)>>,
}

impl DaemonInterface {
    pub fn new(
        profile_manager: ProfileManager,
        context_store: ContextStore,
        rvms_manager: RvmsManager,
        config: DaemonConfig,
    ) -> Self {
        Self {
            profile_manager,
            context_store,
            rvms_manager,
            _config: config,
            cancel_generation: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            recording_state: tokio::sync::Mutex::new(None),
        }
    }

    pub async fn shutdown(&self) {
        tracing::info!("Запуск graceful shutdown демона...");
        if let Err(e) = self.rvms_manager.stop().await {
            tracing::error!("Ошибка при остановке RVMS: {}", e);
        }
        let mut state = self.recording_state.lock().await;
        if let Some((mut child, path)) = state.take() {
            tracing::info!("Остановка фонового процесса записи звука...");
            if let Some(pid) = child.id() {
                match child.try_wait() {
                    Ok(None) => {
                        // SAFETY: Мы отправляем сигнал SIGINT нашему активному дочернему процессу gst-launch-1.0.
                        // Перед отправкой проверяем статус с помощью try_wait(), чтобы избежать гонки PID.
                        unsafe {
                            libc::kill(pid as libc::pid_t, libc::SIGINT);
                        }
                        if let Err(e) = child.wait().await {
                            tracing::error!("Ошибка при ожидании завершения процесса записи звука: {:?}", e);
                        }
                    }
                    _ => {}
                }
            }
            if let Err(e) = tokio::fs::remove_file(&path).await {
                if e.kind() != std::io::ErrorKind::NotFound {
                    tracing::warn!("Не удалось удалить временный аудиофайл {:?}: {:?}", path, e);
                }
            }
        }
    }
}

#[interface(name = "com.izighost.Daemon")]
impl DaemonInterface {
    /// Запустить виртуальный экран RVMS. Возвращает ID PipeWire источника для трансляции.
    async fn start_rvms(&self) -> zbus::fdo::Result<u32> {
        self.rvms_manager
            .start()
            .await
            .map_err(zbus::fdo::Error::Failed)
    }

    /// Остановить трансляцию виртуального экрана RVMS.
    async fn stop_rvms(&self) -> zbus::fdo::Result<()> {
        self.rvms_manager
            .stop()
            .await
            .map_err(zbus::fdo::Error::Failed)
    }

    /// Отправить сообщение в чат LLM. Ответ возвращается инкрементально через zbus-сигналы.
    async fn send_chat_message(
        &self,
        text: String,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        let profile = self.context_store.get_active_profile().await
            .ok_or_else(|| zbus::fdo::Error::Failed("Нет активного профиля".to_string()))?;

        // Сохраняем сообщение пользователя в историю
        self.context_store
            .add_message("user".to_string(), text.clone())
            .await;

        let api_key = if !profile.id.is_empty() {
            izighost_common::KeyringStore::get_password(&format!("llm_api_key_{}", profile.id))
                .await
                .ok()
                .flatten()
                .unwrap_or_default()
        } else {
            "".to_string()
        };

        let system_prompt = crate::prompt_assembler::assemble_system_prompt(&profile);
        let history = self.context_store.get_history().await;

        // Сбрасываем флаг отмены перед началом новой генерации
        self.cancel_generation.store(false, std::sync::atomic::Ordering::SeqCst);

        let mut full_response = String::new();
        match crate::llm::stream_chat_completion(
            &profile.llm.base_url,
            &profile.llm.model,
            &api_key,
            profile.llm.temperature,
            &history,
            &system_prompt,
        ).await {
            Ok(mut stream) => {
                use futures::StreamExt;
                while let Some(chunk_res) = stream.next().await {
                    if self.cancel_generation.load(std::sync::atomic::Ordering::SeqCst) {
                        tracing::info!("Генерация прервана пользователем.");
                        break;
                    }
                    match chunk_res {
                        Ok(chunk) => {
                            full_response.push_str(&chunk);
                            if let Err(err) = Self::chat_chunk(&emitter, &chunk).await {
                                tracing::error!("Ошибка отправки D-Bus сигнала chat_chunk: {:?}", err);
                            }
                        }
                        Err(e) => {
                            let err_msg = format!("Ошибка во время стриминга LLM: {}", e);
                            tracing::error!("{}", err_msg);
                            if let Err(err) = Self::error_occurred(&emitter, &err_msg).await {
                                tracing::error!("Ошибка отправки D-Bus сигнала error_occurred: {:?}", err);
                            }
                            return Err(zbus::fdo::Error::Failed(err_msg));
                        }
                    }
                }
            }
            Err(e) => {
                let err_msg = format!("Не удалось выполнить LLM запрос: {}", e);
                tracing::error!("{}", err_msg);
                if let Err(err) = Self::error_occurred(&emitter, &err_msg).await {
                    tracing::error!("Ошибка отправки D-Bus сигнала error_occurred: {:?}", err);
                }
                return Err(zbus::fdo::Error::Failed(err_msg));
            }
        }

        self.context_store
            .add_message("assistant".to_string(), full_response)
            .await;

        Self::chat_completed(&emitter)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        Ok(())
    }

    async fn cancel_generation(&self) -> zbus::fdo::Result<()> {
        self.cancel_generation.store(true, std::sync::atomic::Ordering::SeqCst);
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
                if let Err(err) = Self::error_occurred(&emitter, err_msg).await {
                    tracing::error!("Ошибка отправки D-Bus сигнала error_occurred: {:?}", err);
                }
                return Err(zbus::fdo::Error::Failed(err_msg.to_string()));
            }
        };

        let profile = self.context_store.get_active_profile().await;
        match crate::ocr::trigger_ocr_pipeline(node_id, profile, &self._config.general.cache_dir).await {
            Ok(text) => {
                self.context_store.set_last_preview(Some(text.clone())).await;
                if let Err(e) = Self::ocr_completed(&emitter, &text).await {
                    tracing::error!("Ошибка отправки D-Bus сигнала ocr_completed: {:?}", e);
                }
                Ok(())
            }
            Err(e) => {
                let err_msg = format!("Ошибка распознавания текста (OCR): {}", e);
                tracing::error!("{}", err_msg);
                if let Err(err) = Self::error_occurred(&emitter, &err_msg).await {
                    tracing::error!("Ошибка отправки D-Bus сигнала error_occurred: {:?}", err);
                }
                Err(zbus::fdo::Error::Failed(err_msg))
            }
        }
    }

    async fn trigger_ocr_from_file(
        &self,
        file_path: String,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        let path = std::path::PathBuf::from(file_path);
        if !path.exists() {
            let err_msg = "Указанный файл не существует".to_string();
            if let Err(err) = Self::error_occurred(&emitter, &err_msg).await {
                tracing::error!("Ошибка отправки D-Bus сигнала error_occurred: {:?}", err);
            }
            return Err(zbus::fdo::Error::Failed(err_msg));
        }

        let profile = self.context_store.get_active_profile().await;
        match crate::ocr::run_ocr_on_file(path, profile, &self._config.general.cache_dir).await {
            Ok(text) => {
                self.context_store.set_last_preview(Some(text.clone())).await;
                if let Err(e) = Self::ocr_completed(&emitter, &text).await {
                    tracing::error!("Ошибка отправки D-Bus сигнала ocr_completed: {:?}", e);
                }
                Ok(())
            }
            Err(e) => {
                let err_msg = format!("Ошибка распознавания текста (OCR): {}", e);
                tracing::error!("{}", err_msg);
                if let Err(err) = Self::error_occurred(&emitter, &err_msg).await {
                    tracing::error!("Ошибка отправки D-Bus сигнала error_occurred: {:?}", err);
                }
                Err(zbus::fdo::Error::Failed(err_msg))
            }
        }
    }

    async fn start_listening(&self) -> zbus::fdo::Result<()> {
        static FILE_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let count = FILE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let pid = std::process::id();
        let timestamp = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let temp_path = std::env::temp_dir().join(format!(
            "izighost_recording_{}_{}_{}.wav",
            timestamp, pid, count
        ));

        tracing::info!("Запуск записи звука в {:?}", temp_path);

        // Перед запуском новой записи останавливаем предыдущую, если она зависла
        {
            let mut state = self.recording_state.lock().await;
            if let Some((mut child, path)) = state.take() {
                tracing::warn!("Остановка зависшей записи: {:?}", path);
                if let Err(e) = child.kill().await {
                    tracing::warn!("Не удалось убить зависший процесс записи звука: {:?}", e);
                }
                if let Err(e) = child.wait().await {
                    tracing::warn!("Ошибка при ожидании зависшего процесса записи звука: {:?}", e);
                }
                if let Err(e) = tokio::fs::remove_file(&path).await {
                    if e.kind() != std::io::ErrorKind::NotFound {
                        tracing::warn!("Не удалось удалить временный аудиофайл {:?}: {:?}", path, e);
                    }
                }
            }
        }

        // Запуск GStreamer для записи в 16кГц 16-бит моно PCM WAV
        let child = tokio::process::Command::new("gst-launch-1.0")
            .arg("autoaudiosrc")
            .arg("!")
            .arg("audioconvert")
            .arg("!")
            .arg("audioresample")
            .arg("!")
            .arg("audio/x-raw,format=S16LE,channels=1,rate=16000")
            .arg("!")
            .arg("wavenc")
            .arg("!")
            .arg("filesink")
            .arg(format!("location={}", temp_path.to_string_lossy()))
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| zbus::fdo::Error::Failed(format!("Не удалось запустить gst-launch-1.0: {}", e)))?;

        {
            let mut state = self.recording_state.lock().await;
            *state = Some((child, temp_path));
        }

        Ok(())
    }

    async fn stop_listening(
        &self,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        let recording = {
            let mut state = self.recording_state.lock().await;
            state.take()
        };

        let Some((mut child, path)) = recording else {
            return Err(zbus::fdo::Error::Failed("Запись звука не запущена".to_string()));
        };

        tracing::info!("Остановка записи звука...");

        // Отправляем SIGINT (kill -2), чтобы gst-launch завершил запись с записью wav-заголовков
        if let Some(pid) = child.id() {
            // SAFETY: Мы отправляем сигнал SIGINT нашему активному дочернему процессу gst-launch-1.0.
            // Перед отправкой проверяем статус с помощью try_wait(), чтобы избежать гонки PID.
            match child.try_wait() {
                Ok(None) => {
                    unsafe {
                        libc::kill(pid as libc::pid_t, libc::SIGINT);
                    }
                }
                Ok(Some(_status)) => {
                    tracing::info!("Запись звука gst-launch уже завершилась");
                }
                Err(e) => {
                    tracing::error!("Ошибка при проверке статуса записи звука: {:?}", e);
                }
            }
        }

        // Ожидаем завершения процесса асинхронно
        if let Err(e) = child.wait().await {
            tracing::error!("Ошибка при ожидании завершения процесса записи звука в stop_listening: {:?}", e);
        }

        // Загружаем профиль и API-ключ для распознавания
        let profile = self.context_store.get_active_profile().await;
        
        let api_key = if let Some(ref p) = profile {
            if !p.id.is_empty() {
                izighost_common::KeyringStore::get_password(&format!("asr_api_key_{}", p.id))
                    .await
                    .ok()
                    .flatten()
                    .unwrap_or_default()
            } else {
                "".to_string()
            }
        } else {
            "".to_string()
        };

        // Запускаем ASR в фоновом потоке
        let emitter_clone = emitter.clone().into_owned();
        let context_store = self.context_store.clone();

        tokio::spawn(async move {
            let path_clone = path.clone();
            let result = crate::audio::transcribe_audio(&path_clone, profile.as_ref(), &api_key).await;
            
            if let Err(e) = tokio::fs::remove_file(&path).await {
                if e.kind() != std::io::ErrorKind::NotFound {
                    tracing::warn!("Не удалось удалить временный аудиофайл {:?}: {:?}", path, e);
                }
            }

            match result {
                Ok(text) => {
                    tracing::info!("Успешно распознано: {}", text);
                    context_store.set_last_preview(Some(text.clone())).await;
                    if let Err(e) = Self::asr_completed(&emitter_clone, &text).await {
                        tracing::error!("Ошибка отправки D-Bus сигнала asr_completed: {:?}", e);
                    }
                }
                Err(e) => {
                    let err_msg = format!("Ошибка распознавания речи (ASR): {}", e);
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

    async fn get_chat_history(&self) -> zbus::fdo::Result<Vec<(String, String)>> {
        let history = self.context_store.get_history().await;
        let result = history
            .iter()
            .map(|msg| (msg.role.clone(), msg.content.clone()))
            .collect();
        Ok(result)
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

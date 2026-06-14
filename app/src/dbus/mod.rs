use izighost_common::Profile;
use tokio::sync::mpsc::{Sender, Receiver};
use zbus::{proxy, Connection};

#[derive(Clone, Debug)]
pub enum DaemonSignal {
    ChatChunk(String),
    ChatCompleted,
    OcrCompleted(String),
    AsrCompleted(String),
    ErrorOccurred(String),
    ScreenshotCaptured(String),
}

#[proxy(
    interface = "com.izighost.Daemon",
    default_service = "com.izighost.Daemon",
    default_path = "/com/izighost/Daemon"
)]
pub trait Daemon {
    async fn start_rvms(&self) -> zbus::Result<u32>;
    async fn stop_rvms(&self) -> zbus::Result<()>;
    async fn send_chat_message(&self, text: &str) -> zbus::Result<()>;
    async fn trigger_ocr(&self) -> zbus::Result<()>;
    async fn trigger_ocr_from_file(&self, file_path: &str) -> zbus::Result<()>;
    async fn capture_virtual_screenshot(&self) -> zbus::Result<String>;
    async fn run_ocr_on_file(&self, file_path: &str) -> zbus::Result<String>;
    async fn start_listening(&self) -> zbus::Result<()>;
    async fn stop_listening(&self) -> zbus::Result<()>;
    async fn list_profiles(&self) -> zbus::Result<Vec<String>>;
    async fn get_profile(&self, id: &str) -> zbus::Result<Profile>;
    async fn save_profile(&self, profile: &Profile) -> zbus::Result<Profile>;
    async fn delete_profile(&self, id: &str) -> zbus::Result<()>;
    async fn set_active_profile(&self, id: &str) -> zbus::Result<()>;
    async fn get_active_profile(&self) -> zbus::Result<Profile>;
    async fn cancel_generation(&self) -> zbus::Result<()>;
    async fn get_chat_history(&self) -> zbus::Result<Vec<(String, String)>>;

    #[zbus(signal)]
    async fn chat_chunk(&self, delta_text: String) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn chat_completed(&self) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn ocr_completed(&self, text: String) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn asr_completed(&self, text: String) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn error_occurred(&self, message: String) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn screenshot_captured(&self, filepath: String) -> zbus::Result<()>;
}

#[proxy(
    interface = "org.gnome.Shell.Extensions.WindowPinBridge",
    default_service = "org.gnome.Shell",
    default_path = "/org/gnome/Shell/Extensions/WindowPinBridge"
)]
pub trait WindowPinBridge {
    async fn pin_window_by_pid(&self, pid: u32) -> zbus::Result<bool>;
    async fn unpin_window_by_pid(&self, pid: u32) -> zbus::Result<bool>;
    async fn capture_screenshot(&self, monitor_index: u32, filepath: &str) -> zbus::Result<bool>;
    async fn capture_virtual_monitor(&self, filepath: &str) -> zbus::Result<bool>;
    async fn warp_cursor(&self, x: i32, y: i32) -> zbus::Result<bool>;
    async fn save_cursor_position(&self) -> zbus::Result<bool>;
    async fn restore_cursor_position(&self) -> zbus::Result<bool>;
    async fn warp_to_monitor(&self, monitor_index: u32) -> zbus::Result<bool>;
    async fn warp_to_virtual_monitor(&self) -> zbus::Result<bool>;
}

pub struct DaemonClient {
    proxy: DaemonProxy<'static>,
    pin_proxy: Option<WindowPinBridgeProxy<'static>>,
}

impl DaemonClient {
    pub async fn connect() -> zbus::Result<(Self, Receiver<DaemonSignal>)> {
        let conn = Connection::session().await?;
        let proxy = DaemonProxy::new(&conn).await?;
        let pin_proxy = match WindowPinBridgeProxy::new(&conn).await {
            Ok(p) => Some(p),
            Err(e) => {
                tracing::warn!("Failed to connect to WindowPinBridge GNOME extension: {:?}", e);
                None
            }
        };
        let (tx, rx) = tokio::sync::mpsc::channel(100);

        // Запуск фоновой задачи для прослушивания сигналов с автоматическим переподключением
        let conn_clone = conn.clone();
        tokio::spawn(async move {
            listen_to_signals(conn_clone, tx).await;
        });

        Ok((Self { proxy, pin_proxy }, rx))
    }

    pub async fn start_rvms(&self) -> zbus::Result<u32> {
        self.proxy.start_rvms().await
    }

    pub async fn stop_rvms(&self) -> zbus::Result<()> {
        self.proxy.stop_rvms().await
    }

    pub async fn send_chat_message(&self, text: &str) -> zbus::Result<()> {
        self.proxy.send_chat_message(text).await
    }

    #[allow(dead_code)]
    pub async fn trigger_ocr(&self) -> zbus::Result<()> {
        self.proxy.trigger_ocr().await
    }

    pub async fn trigger_ocr_from_file(&self, file_path: &str) -> zbus::Result<()> {
        self.proxy.trigger_ocr_from_file(file_path).await
    }

    pub async fn capture_virtual_screenshot(&self) -> zbus::Result<String> {
        self.proxy.capture_virtual_screenshot().await
    }

    pub async fn run_ocr_on_file(&self, file_path: &str) -> zbus::Result<String> {
        self.proxy.run_ocr_on_file(file_path).await
    }

    pub async fn start_listening(&self) -> zbus::Result<()> {
        self.proxy.start_listening().await
    }

    pub async fn stop_listening(&self) -> zbus::Result<()> {
        self.proxy.stop_listening().await
    }

    pub async fn list_profiles(&self) -> zbus::Result<Vec<String>> {
        self.proxy.list_profiles().await
    }

    pub async fn get_profile(&self, id: &str) -> zbus::Result<Profile> {
        self.proxy.get_profile(id).await
    }

    pub async fn save_profile(&self, profile: &Profile) -> zbus::Result<Profile> {
        self.proxy.save_profile(profile).await
    }

    pub async fn delete_profile(&self, id: &str) -> zbus::Result<()> {
        self.proxy.delete_profile(id).await
    }

    pub async fn set_active_profile(&self, id: &str) -> zbus::Result<()> {
        self.proxy.set_active_profile(id).await
    }

    pub async fn get_active_profile(&self) -> zbus::Result<Profile> {
        self.proxy.get_active_profile().await
    }

    pub async fn cancel_generation(&self) -> zbus::Result<()> {
        self.proxy.cancel_generation().await
    }

    pub async fn get_chat_history(&self) -> zbus::Result<Vec<(String, String)>> {
        self.proxy.get_chat_history().await
    }

    pub async fn pin_window_by_pid(&self, pid: u32) -> zbus::Result<bool> {
        if let Some(ref pin_proxy) = self.pin_proxy {
            pin_proxy.pin_window_by_pid(pid).await
        } else {
            Ok(false)
        }
    }

    pub async fn unpin_window_by_pid(&self, pid: u32) -> zbus::Result<bool> {
        if let Some(ref pin_proxy) = self.pin_proxy {
            pin_proxy.unpin_window_by_pid(pid).await
        } else {
            Ok(false)
        }
    }

    #[allow(dead_code)]
    pub async fn capture_screenshot(&self, monitor_index: u32, filepath: &str) -> zbus::Result<bool> {
        if let Some(ref pin_proxy) = self.pin_proxy {
            pin_proxy.capture_screenshot(monitor_index, filepath).await
        } else {
            Ok(false)
        }
    }

    #[allow(dead_code)]
    pub async fn capture_virtual_monitor(&self, filepath: &str) -> zbus::Result<bool> {
        if let Some(ref pin_proxy) = self.pin_proxy {
            pin_proxy.capture_virtual_monitor(filepath).await
        } else {
            Ok(false)
        }
    }

    #[allow(dead_code)]
    pub async fn warp_cursor(&self, x: i32, y: i32) -> zbus::Result<bool> {
        if let Some(ref pin_proxy) = self.pin_proxy {
            pin_proxy.warp_cursor(x, y).await
        } else {
            Ok(false)
        }
    }

    pub async fn save_cursor_position(&self) -> zbus::Result<bool> {
        if let Some(ref pin_proxy) = self.pin_proxy {
            pin_proxy.save_cursor_position().await
        } else {
            Ok(false)
        }
    }

    pub async fn restore_cursor_position(&self) -> zbus::Result<bool> {
        if let Some(ref pin_proxy) = self.pin_proxy {
            pin_proxy.restore_cursor_position().await
        } else {
            Ok(false)
        }
    }

    #[allow(dead_code)]
    pub async fn warp_to_monitor(&self, monitor_index: u32) -> zbus::Result<bool> {
        if let Some(ref pin_proxy) = self.pin_proxy {
            pin_proxy.warp_to_monitor(monitor_index).await
        } else {
            Ok(false)
        }
    }

    pub async fn warp_to_virtual_monitor(&self) -> zbus::Result<bool> {
        if let Some(ref pin_proxy) = self.pin_proxy {
            pin_proxy.warp_to_virtual_monitor().await
        } else {
            Ok(false)
        }
    }
}

async fn listen_to_signals(
    conn: Connection,
    tx: Sender<DaemonSignal>,
) {
    use futures::StreamExt;
    let mut delay = std::time::Duration::from_secs(1);

    loop {
        match DaemonProxy::new(&conn).await {
            Ok(proxy) => {
                let chat_chunks_res = proxy.receive_chat_chunk().await;
                let chat_completeds_res = proxy.receive_chat_completed().await;
                let ocr_completeds_res = proxy.receive_ocr_completed().await;
                let asr_completeds_res = proxy.receive_asr_completed().await;
                let error_occurreds_res = proxy.receive_error_occurred().await;
                let screenshot_captureds_res = proxy.receive_screenshot_captured().await;

                if let (Ok(mut chat_chunks), Ok(mut chat_completeds), Ok(mut ocr_completeds), Ok(mut asr_completeds), Ok(mut error_occurreds), Ok(mut screenshot_captureds)) =
                    (chat_chunks_res, chat_completeds_res, ocr_completeds_res, asr_completeds_res, error_occurreds_res, screenshot_captureds_res)
                {
                    tracing::info!("Успешно подключились к сигналам D-Bus демона.");
                    delay = std::time::Duration::from_secs(1); // сброс задержки

                    loop {
                        tokio::select! {
                            msg = chat_chunks.next() => {
                                match msg {
                                    Some(msg) => {
                                        if let Ok(args) = msg.args() {
                                            if tx.send(DaemonSignal::ChatChunk(args.delta_text)).await.is_err() { return; }
                                        }
                                    }
                                    None => break,
                                }
                            }
                            msg = chat_completeds.next() => {
                                match msg {
                                    Some(_) => {
                                        if tx.send(DaemonSignal::ChatCompleted).await.is_err() { return; }
                                    }
                                    None => break,
                                }
                            }
                            msg = ocr_completeds.next() => {
                                match msg {
                                    Some(msg) => {
                                        if let Ok(args) = msg.args() {
                                            if tx.send(DaemonSignal::OcrCompleted(args.text)).await.is_err() { return; }
                                        }
                                    }
                                    None => break,
                                }
                            }
                            msg = asr_completeds.next() => {
                                match msg {
                                    Some(msg) => {
                                        if let Ok(args) = msg.args() {
                                            if tx.send(DaemonSignal::AsrCompleted(args.text)).await.is_err() { return; }
                                        }
                                    }
                                    None => break,
                                }
                            }
                            msg = error_occurreds.next() => {
                                match msg {
                                    Some(msg) => {
                                        if let Ok(args) = msg.args() {
                                            if tx.send(DaemonSignal::ErrorOccurred(args.message)).await.is_err() { return; }
                                        }
                                    }
                                    None => break,
                                }
                            }
                            msg = screenshot_captureds.next() => {
                                match msg {
                                    Some(msg) => {
                                        if let Ok(args) = msg.args() {
                                            if tx.send(DaemonSignal::ScreenshotCaptured(args.filepath)).await.is_err() { return; }
                                        }
                                    }
                                    None => break,
                                }
                            }
                        }
                    }
                    tracing::warn!("Соединение с сигналами D-Bus потеряно. Попытка переподключения...");
                } else {
                    tracing::warn!("Не удалось подписаться на один или несколько сигналов D-Bus.");
                }
            }
            Err(e) => {
                tracing::warn!("Не удалось получить D-Bus прокси для сигналов: {:?}. Повтор через {:?}", e, delay);
            }
        }

        tokio::time::sleep(delay).await;
        delay = std::cmp::min(delay * 2, std::time::Duration::from_secs(30));
    }
}

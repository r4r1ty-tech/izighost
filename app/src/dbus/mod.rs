use izighost_common::Profile;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use zbus::{proxy, Connection};

#[derive(Clone, Debug)]
pub enum DaemonSignal {
    ChatChunk(String),
    ChatCompleted,
    OcrCompleted(String),
    AsrCompleted(String),
    ErrorOccurred(String),
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
    async fn start_listening(&self) -> zbus::Result<()>;
    async fn stop_listening(&self) -> zbus::Result<()>;
    async fn list_profiles(&self) -> zbus::Result<Vec<String>>;
    async fn get_profile(&self, id: &str) -> zbus::Result<Profile>;
    async fn save_profile(&self, profile: &Profile) -> zbus::Result<Profile>;
    async fn delete_profile(&self, id: &str) -> zbus::Result<()>;
    async fn set_active_profile(&self, id: &str) -> zbus::Result<()>;
    async fn get_active_profile(&self) -> zbus::Result<Profile>;

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
}

pub struct DaemonClient {
    proxy: DaemonProxy<'static>,
}

impl DaemonClient {
    pub async fn connect() -> zbus::Result<(Self, UnboundedReceiver<DaemonSignal>)> {
        let conn = Connection::session().await?;
        let proxy = DaemonProxy::new(&conn).await?;
        let (tx, rx) = unbounded_channel();

        // Запуск фоновой задачи для прослушивания сигналов
        let proxy_clone = proxy.clone();
        tokio::spawn(async move {
            if let Err(e) = listen_to_signals(proxy_clone, tx).await {
                eprintln!("Ошибка при прослушивании сигналов D-Bus: {:?}", e);
            }
        });

        Ok((Self { proxy }, rx))
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

    pub async fn trigger_ocr(&self) -> zbus::Result<()> {
        self.proxy.trigger_ocr().await
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
}

async fn listen_to_signals(
    proxy: DaemonProxy<'static>,
    tx: UnboundedSender<DaemonSignal>,
) -> zbus::Result<()> {
    use futures::StreamExt;

    let mut chat_chunks = proxy.receive_chat_chunk().await?;
    let mut chat_completeds = proxy.receive_chat_completed().await?;
    let mut ocr_completeds = proxy.receive_ocr_completed().await?;
    let mut asr_completeds = proxy.receive_asr_completed().await?;
    let mut error_occurreds = proxy.receive_error_occurred().await?;

    loop {
        tokio::select! {
            Some(msg) = chat_chunks.next() => {
                if let Ok(args) = msg.args() {
                    let _ = tx.send(DaemonSignal::ChatChunk(args.delta_text));
                }
            }
            Some(_) = chat_completeds.next() => {
                let _ = tx.send(DaemonSignal::ChatCompleted);
            }
            Some(msg) = ocr_completeds.next() => {
                if let Ok(args) = msg.args() {
                    let _ = tx.send(DaemonSignal::OcrCompleted(args.text));
                }
            }
            Some(msg) = asr_completeds.next() => {
                if let Ok(args) = msg.args() {
                    let _ = tx.send(DaemonSignal::AsrCompleted(args.text));
                }
            }
            Some(msg) = error_occurreds.next() => {
                if let Ok(args) = msg.args() {
                    let _ = tx.send(DaemonSignal::ErrorOccurred(args.message));
                }
            }
        }
    }
}

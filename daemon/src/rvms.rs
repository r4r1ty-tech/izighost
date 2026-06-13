use futures::StreamExt;
use std::collections::HashMap;
use std::process::{Child, Command};
use std::sync::Arc;
use tokio::sync::Mutex;
use zbus::{proxy, Connection};
use zvariant::{OwnedObjectPath, Value};

// --- ScreenCast D-Bus Interface ---

#[proxy(
    interface = "org.gnome.Mutter.ScreenCast",
    default_service = "org.gnome.Mutter.ScreenCast",
    default_path = "/org/gnome/Mutter/ScreenCast"
)]
pub trait ScreenCast {
    async fn create_session(
        &self,
        properties: HashMap<&str, Value<'_>>,
    ) -> zbus::Result<OwnedObjectPath>;
}

#[proxy(
    interface = "org.gnome.Mutter.ScreenCast.Session",
    default_service = "org.gnome.Mutter.ScreenCast"
)]
pub trait ScreenCastSession {
    async fn start(&self) -> zbus::Result<()>;
    async fn stop(&self) -> zbus::Result<()>;
    async fn record_virtual(
        &self,
        properties: HashMap<&str, Value<'_>>,
    ) -> zbus::Result<OwnedObjectPath>;
}

#[proxy(
    interface = "org.gnome.Mutter.ScreenCast.Stream",
    default_service = "org.gnome.Mutter.ScreenCast"
)]
pub trait ScreenCastStream {
    async fn start(&self) -> zbus::Result<()>;
    async fn stop(&self) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn pipe_wire_stream_added(&self, node_id: u32) -> zbus::Result<()>;
}

// --- RemoteDesktop D-Bus Interface ---

#[proxy(
    interface = "org.gnome.Mutter.RemoteDesktop",
    default_service = "org.gnome.Mutter.RemoteDesktop",
    default_path = "/org/gnome/Mutter/RemoteDesktop"
)]
pub trait RemoteDesktop {
    async fn create_session(&self) -> zbus::Result<OwnedObjectPath>;
}

#[proxy(
    interface = "org.gnome.Mutter.RemoteDesktop.Session",
    default_service = "org.gnome.Mutter.RemoteDesktop"
)]
pub trait RemoteDesktopSession {
    async fn start(&self) -> zbus::Result<()>;
    async fn stop(&self) -> zbus::Result<()>;
}

// --- RvmsManager State ---

pub struct RvmsState {
    connection: Option<Connection>,
    session_path: Option<OwnedObjectPath>,
    stream_path: Option<OwnedObjectPath>,
    rd_session_path: Option<OwnedObjectPath>,
    gstreamer_process: Option<Child>,
    pipewire_node_id: Option<u32>,
}

#[derive(Clone)]
pub struct RvmsManager {
    state: Arc<Mutex<RvmsState>>,
}

impl Default for RvmsManager {
    fn default() -> Self {
        Self::new()
    }
}

impl RvmsManager {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(RvmsState {
                connection: None,
                session_path: None,
                stream_path: None,
                rd_session_path: None,
                gstreamer_process: None,
                pipewire_node_id: None,
            })),
        }
    }

    /// Получить активный PipeWire Node ID виртуального монитора.
    pub async fn get_pipewire_node_id(&self) -> Option<u32> {
        let state = self.state.lock().await;
        state.pipewire_node_id
    }

    pub async fn start(&self) -> Result<u32, String> {
        let mut state = self.state.lock().await;

        if state.pipewire_node_id.is_some() {
            return Err("RVMS сессия уже активна".to_string());
        }

        // 1. Подключение к D-Bus
        let conn = Connection::session()
            .await
            .map_err(|e| format!("Не удалось подключиться к сессионной шине D-Bus: {}", e))?;

        // 2. Создание RemoteDesktop сессии для эмуляции ввода (EIS)
        let rd_proxy = RemoteDesktopProxy::new(&conn)
            .await
            .map_err(|e| format!("Не удалось создать RemoteDesktop прокси: {}", e))?;
        let rd_session_path = rd_proxy
            .create_session()
            .await
            .map_err(|e| format!("Не удалось создать RemoteDesktop сессию: {}", e))?;
        let rd_session_proxy = RemoteDesktopSessionProxy::builder(&conn)
            .path(&rd_session_path)
            .map_err(|e| format!("Не удалось установить путь RemoteDesktop сессии: {}", e))?
            .build()
            .await
            .map_err(|e| format!("Не удалось построить прокси RemoteDesktopSession: {}", e))?;

        // 3. Создание ScreenCast прокси
        let screencast_proxy = ScreenCastProxy::new(&conn)
            .await
            .map_err(|e| format!("Не удалось создать ScreenCast прокси: {}", e))?;

        // 4. Создание ScreenCast сессии
        let mut session_props = HashMap::new();
        session_props.insert("cursor-mode", Value::from(1u32)); // отображать курсор
        let session_path = screencast_proxy
            .create_session(session_props)
            .await
            .map_err(|e| format!("Не удалось создать ScreenCast сессию: {}", e))?;

        let session_proxy = ScreenCastSessionProxy::builder(&conn)
            .path(&session_path)
            .map_err(|e| format!("Не удалось установить путь ScreenCast сессии: {}", e))?
            .build()
            .await
            .map_err(|e| format!("Не удалось построить прокси ScreenCastSession: {}", e))?;

        // 5. Добавление виртуального монитора
        let mut record_props = HashMap::new();
        record_props.insert("width", Value::from(1920i32));
        record_props.insert("height", Value::from(1080i32));
        record_props.insert("scale", Value::from(1.0f64));
        record_props.insert("cursor-mode", Value::from(1u32));

        let stream_path = session_proxy
            .record_virtual(record_props)
            .await
            .map_err(|e| format!("Метод RecordVirtual завершился с ошибкой: {}", e))?;

        let stream_proxy = ScreenCastStreamProxy::builder(&conn)
            .path(&stream_path)
            .map_err(|e| format!("Не удалось установить путь стрима: {}", e))?
            .build()
            .await
            .map_err(|e| format!("Не удалось построить прокси ScreenCastStream: {}", e))?;

        // 6. Подписка на сигнал добавления PipeWire стрима
        let mut signal_stream = stream_proxy
            .receive_pipe_wire_stream_added()
            .await
            .map_err(|e| {
                format!(
                    "Не удалось подписаться на сигнал PipeWireStreamAdded: {}",
                    e
                )
            })?;

        // 7. Запуск сессий ввода и трансляции
        rd_session_proxy
            .start()
            .await
            .map_err(|e| format!("Не удалось запустить RemoteDesktop сессию: {}", e))?;
        session_proxy
            .start()
            .await
            .map_err(|e| format!("Не удалось запустить ScreenCast сессию: {}", e))?;

        // 8. Ожидание сигнала с PipeWire Node ID
        let node_id = tokio::select! {
            Some(msg) = signal_stream.next() => {
                match msg.args() {
                    Ok(args) => args.node_id,
                    Err(e) => return Err(format!("Ошибка распаковки аргументов сигнала: {}", e)),
                }
            }
            _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
                return Err("Таймаут ожидания сигнала PipeWireStreamAdded".to_string());
            }
        };

        // 9. Запуск интерактивного loopback-зеркала с поддержкой EIS ввода
        let gst_child = Command::new("python3")
            .arg("daemon/src/rvms_loopback.py")
            .arg(format!("{}", node_id))
            .arg(rd_session_path.as_str())
            .arg(stream_path.as_str())
            .spawn()
            .map_err(|e| format!("Не удалось запустить rvms_loopback.py: {}", e))?;

        // Сохраняем состояние сессии
        state.connection = Some(conn);
        state.session_path = Some(session_path);
        state.stream_path = Some(stream_path);
        state.rd_session_path = Some(rd_session_path);
        state.gstreamer_process = Some(gst_child);
        state.pipewire_node_id = Some(node_id);

        Ok(node_id)
    }

    pub async fn stop(&self) -> Result<(), String> {
        let mut state = self.state.lock().await;

        if state.pipewire_node_id.is_none() {
            return Ok(());
        }

        // Останавливаем процесс rvms_loopback.py
        if let Some(mut child) = state.gstreamer_process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }

        // Вызываем метод Stop на объекте ScreenCast сессии
        if let (Some(ref conn), Some(ref session_path)) = (&state.connection, &state.session_path) {
            if let Ok(session_proxy) = ScreenCastSessionProxy::builder(conn).path(session_path) {
                if let Ok(proxy) = session_proxy.build().await {
                    let _ = proxy.stop().await;
                }
            }
        }

        // Вызываем метод Stop на объекте RemoteDesktop сессии
        if let (Some(ref conn), Some(ref rd_session_path)) =
            (&state.connection, &state.rd_session_path)
        {
            if let Ok(rd_session_proxy) =
                RemoteDesktopSessionProxy::builder(conn).path(rd_session_path)
            {
                if let Ok(proxy) = rd_session_proxy.build().await {
                    let _ = proxy.stop().await;
                }
            }
        }

        // Сбрасываем D-Bus прокси и соединение, что заставит Mutter закрыть сессии
        state.connection = None;
        state.session_path = None;
        state.stream_path = None;
        state.rd_session_path = None;
        state.pipewire_node_id = None;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rvms_manager_get_node_id_initially_none() {
        let manager = RvmsManager::new();
        assert_eq!(manager.get_pipewire_node_id().await, None);
    }
}

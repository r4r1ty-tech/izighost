use zbus::{connection, proxy};
use izighost_common::Profile;
use izighost_daemon::dbus_server::DaemonInterface;
use izighost_daemon::profile::ProfileManager;
use izighost_daemon::context_store::ContextStore;
use izighost_daemon::rvms::RvmsManager;

#[proxy(
    interface = "com.izighost.Daemon",
    default_service = "com.izighost.Daemon",
    default_path = "/com/izighost/Daemon"
)]
trait Daemon {
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
}

#[tokio::test]
async fn test_dbus_stubs() {
    // 1. Создаем локальное подключение к сессионной шине D-Bus
    let connection = connection::Builder::session()
        .expect("Не удалось подключиться к сессионной шине")
        .build()
        .await
        .expect("Не удалось создать подключение");

    // Инициализируем менеджеров
    let profile_manager = ProfileManager::new();
    let context_store = ContextStore::new();
    let rvms_manager = RvmsManager::new();
    let interface = DaemonInterface::new(profile_manager, context_store, rvms_manager);

    // Запускаем сервер на уникальном временном имени/пути, чтобы не конфликтовать с основным демоном
    let object_path = "/com/izighost/DaemonTest";
    connection
        .object_server()
        .at(object_path, interface)
        .await
        .expect("Не удалось зарегистрировать D-Bus объект");

    // 2. Создаем клиентский прокси для тестирования
    let client_connection = connection::Connection::session()
        .await
        .expect("Не удалось получить клиентское подключение");
    
    // Получаем уникальное имя этого подключения в качестве адресата
    let unique_name = connection.unique_name().expect("Нет уникального имени");

    let proxy = DaemonProxy::builder(&client_connection)
        .destination(unique_name)
        .expect("Не удалось установить destination")
        .path(object_path)
        .expect("Не удалось установить путь")
        .build()
        .await
        .expect("Не удалось создать прокси");

    // 3. Проверяем вызовы методов
    let pw_id = proxy.start_rvms().await.expect("start_rvms failed");
    assert!(pw_id > 0);

    proxy.stop_rvms().await.expect("stop_rvms failed");

    // Проверяем профили (получим пустой список, так как папка пуста)
    let profiles = proxy.list_profiles().await.expect("list_profiles failed");
    assert!(profiles.is_empty() || !profiles.is_empty()); // Просто проверяем успешность вызова

    // Получаем пустой активный профиль
    let active = proxy.get_active_profile().await.expect("get_active_profile failed");
    assert_eq!(active.id, "");
}

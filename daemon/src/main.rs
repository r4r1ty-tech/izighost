use izighost_daemon::dbus_server::DaemonInterface;
use izighost_daemon::profile::ProfileManager;
use izighost_daemon::context_store::ContextStore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Инициализируем логирование
    tracing_subscriber::fmt::init();

    tracing::info!("Запуск демона IziGhost...");

    // Инициализируем менеджер профилей и хранилище контекста
    let profile_manager = ProfileManager::new();
    let context_store = ContextStore::new();

    // Создаем D-Bus интерфейс
    let interface = DaemonInterface::new(profile_manager, context_store);

    // Подключаемся к сессионной шине D-Bus и регистрируем сервис
    let _conn = zbus::connection::Builder::session()?
        .name(izighost_common::dbus::DBUS_SERVICE_NAME)?
        .serve_at(izighost_common::dbus::DBUS_OBJECT_PATH, interface)?
        .build()
        .await?;

    tracing::info!(
        "D-Bus сервер IziGhost успешно запущен на пути: {}",
        izighost_common::dbus::DBUS_OBJECT_PATH
    );

    // Ожидаем завершения работы
    std::future::pending::<()>().await;

    Ok(())
}

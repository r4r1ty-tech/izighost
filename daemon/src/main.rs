use izighost_daemon::config::DaemonConfig;
use izighost_daemon::context_store::ContextStore;
use izighost_daemon::dbus_server::DaemonInterface;
use izighost_daemon::profile::ProfileManager;
use izighost_daemon::rvms::RvmsManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Загружаем конфигурацию
    let config = match DaemonConfig::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Предупреждение: Не удалось загрузить конфигурацию daemon.yaml. Ошибка: {}", e);
            DaemonConfig::default()
        }
    };

    // Инициализируем логирование с фильтром по умолчанию из конфигурации
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&config.general.log_level));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .init();

    tracing::info!("Запуск демона IziGhost...");

    // Инициализируем менеджеры
    let profile_manager = ProfileManager::new(&config);
    let context_store = ContextStore::new();
    let rvms_manager = RvmsManager::new();

    // Создаем D-Bus интерфейс
    let interface = DaemonInterface::new(profile_manager, context_store, rvms_manager, config);

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

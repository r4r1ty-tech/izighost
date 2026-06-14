use clap::Parser;
use std::path::PathBuf;
use izighost_daemon::config::DaemonConfig;
use izighost_daemon::context_store::ContextStore;
use izighost_daemon::dbus_server::DaemonInterface;
use izighost_daemon::profile::ProfileManager;
use izighost_daemon::rvms::RvmsManager;

#[derive(Parser, Debug)]
#[command(author, version, about = "IziGhost Daemon - Desktop Assistant", long_about = None)]
struct Args {
    /// Путь к конфигурационному файлу YAML
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Включить подробный вывод логов (уровень debug)
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Загружаем конфигурацию
    let config = match DaemonConfig::load(args.config) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Предупреждение: Не удалось загрузить конфигурацию daemon.yaml. Ошибка: {}", e);
            DaemonConfig::default()
        }
    };

    // Инициализируем логирование с фильтром по умолчанию из конфигурации или флага verbose
    let log_level = if args.verbose {
        "debug"
    } else {
        &config.general.log_level
    };

    use tracing_subscriber::prelude::*;

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level));

    let cache_dir = izighost_daemon::config::resolve_path(&config.general.cache_dir);
    let logs_dir = cache_dir.join("logs");
    let _ = std::fs::create_dir_all(&logs_dir);

    let file_appender = tracing_appender::rolling::daily(&logs_dir, "izighost-daemon.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let console_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr);
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false);

    tracing_subscriber::registry()
        .with(filter)
        .with(console_layer)
        .with(file_layer)
        .init();

    tracing::info!("Запуск демона IziGhost...");

    // Инициализируем менеджеры
    let profile_manager = ProfileManager::new(&config);
    let context_store = ContextStore::new(&config.general.data_dir);
    let rvms_manager = RvmsManager::new(&config.general.cache_dir);

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

    // Ожидаем завершения работы по сигналам SIGINT / SIGTERM
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Получен сигнал SIGINT (Ctrl+C). Завершение работы...");
        }
        _ = async {
            #[cfg(unix)]
            {
                let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
                sigterm.recv().await;
            }
            #[cfg(not(unix))]
            {
                std::future::pending::<()>().await;
            }
        } => {
            tracing::info!("Получен сигнал SIGTERM. Завершение работы...");
        }
    }

    // Вызываем Graceful Shutdown интерфейса
    let object_server = _conn.object_server();
    if let Ok(interface_ref) = object_server.interface::<_, DaemonInterface>(izighost_common::dbus::DBUS_OBJECT_PATH).await {
        interface_ref.get().await.shutdown().await;
    }

    Ok(())
}

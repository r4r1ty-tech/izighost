use crate::dbus::DaemonClient;
use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
use futures::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;

/// Инициализация глобального хоткея Super+Shift+S через XDG Desktop Portal.
/// При активации хоткея отправляет D-Bus команду TriggerOcr на сторону демона.
#[allow(dead_code)]
pub async fn init_hotkeys(dbus_client: Option<Arc<DaemonClient>>) -> Result<(), ashpd::Error> {
    let client = match dbus_client {
        Some(c) => c,
        None => {
            tracing::warn!("D-Bus клиент не инициализирован, запуск глобального хоткея пропущен");
            return Ok(());
        }
    };

    // 1. Создаем .desktop файл динамически для ассоциации с App ID
    if let Ok(home) = std::env::var("HOME") {
        let app_dir = std::path::PathBuf::from(home).join(".local/share/applications");
        let _ = std::fs::create_dir_all(&app_dir);
        let desktop_file = app_dir.join("com.izighost.App.desktop");
        if let Ok(exe_path) = std::env::current_exe() {
            let content = format!(
                "[Desktop Entry]\n\
                 Type=Application\n\
                 Name=IziGhost\n\
                 Exec={}\n\
                 Icon=system-run\n\
                 Terminal=false\n\
                 Categories=Utility;\n\
                 StartupWMClass=izighost\n",
                exe_path.to_string_lossy()
            );
            if let Err(e) = std::fs::write(&desktop_file, content) {
                tracing::error!("Не удалось записать .desktop файл: {:?}", e);
            } else {
                tracing::info!("Создан .desktop файл для порталов в {:?}", desktop_file);
            }
        }
    }

    // Создаем прокси для портала GlobalShortcuts
    let global_shortcuts = GlobalShortcuts::new().await?;

    // 2. Выполняем рукопожатие RegisterHostApp для обхода ошибки "An app id is required" в xdg-desktop-portal 1.20+
    let conn = global_shortcuts.connection();
    let options: HashMap<String, zbus::zvariant::Value<'_>> = HashMap::new();
    match conn
        .call_method(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            Some("org.freedesktop.host.portal.Registry"),
            "Register",
            &("com.izighost.App", &options),
        )
        .await
    {
        Ok(_) => tracing::info!("Успешная регистрация App ID (com.izighost.App) в портале"),
        Err(e) => {
            tracing::warn!(
                "Не удалось вызвать Register в портале (возможно, старая версия): {:?}",
                e
            );
        }
    }

    // Создаем сессию для биндинга хоткеев
    let session = global_shortcuts.create_session().await?;

    // Описываем хоткей Super+Shift+S (в спецификации XDG как <Super><Shift>s)
    let shortcut = NewShortcut::new("trigger_ocr", "Сделать скриншот и распознать текст")
        .preferred_trigger(Some("<Super><Shift>s"));

    tracing::info!("Регистрация глобального хоткея в XDG Desktop Portal...");

    // Биндим хоткеи
    let request = global_shortcuts
        .bind_shortcuts(&session, &[shortcut], None)
        .await?;
    let _response = request.response()?;

    tracing::info!("Глобальный хоткей успешно зарегистрирован. Запуск слушателя сигналов...");

    // Подписываемся на сигналы активации хоткеев
    let mut stream = global_shortcuts.receive_activated().await?;

    tokio::spawn(async move {
        while let Some(activated) = stream.next().await {
            if activated.shortcut_id() == "trigger_ocr" {
                tracing::info!("Хоткей Trigger OCR активирован пользователем!");
                let client_clone = client.clone();
                tokio::spawn(async move {
                    if let Err(e) = client_clone.trigger_ocr().await {
                        tracing::error!("Ошибка вызова TriggerOcr по хоткею: {:?}", e);
                    }
                });
            }
        }
    });

    Ok(())
}

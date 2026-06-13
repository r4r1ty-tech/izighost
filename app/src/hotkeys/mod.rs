use std::sync::Arc;
use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
use futures::StreamExt;
use crate::dbus::DaemonClient;

/// Инициализация глобального хоткея Super+Shift+S через XDG Desktop Portal.
/// При активации хоткея отправляет D-Bus команду TriggerOcr на сторону демона.
pub async fn init_hotkeys(dbus_client: Option<Arc<DaemonClient>>) -> Result<(), ashpd::Error> {
    let client = match dbus_client {
        Some(c) => c,
        None => {
            tracing::warn!("D-Bus клиент не инициализирован, запуск глобального хоткея пропущен");
            return Ok(());
        }
    };

    // Создаем прокси для портала GlobalShortcuts
    let global_shortcuts = GlobalShortcuts::new().await?;

    // Создаем сессию для биндинга хоткеев
    let session = global_shortcuts.create_session().await?;

    // Описываем хоткей Super+Shift+S (в спецификации XDG как <Super><Shift>s)
    let shortcut = NewShortcut::new("trigger_ocr", "Сделать скриншот и распознать текст")
        .preferred_trigger(Some("<Super><Shift>s"));

    tracing::info!("Регистрация глобального хоткея в XDG Desktop Portal...");

    // Биндим хоткеи
    let request = global_shortcuts.bind_shortcuts(&session, &[shortcut], None).await?;
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

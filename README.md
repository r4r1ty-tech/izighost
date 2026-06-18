# IziGhost

> Десктопный AI-ассистент для подготовки к техническим собеседованиям на Linux.
> Невидим для шаринга экрана, подтягивает контекст из резюме и описания вакансии.

IziGhost — нативный Linux-ассистент, который помогает готовиться к техническим
собеседованиям и проходить их. Работает как два кооперирующихся Rust-процесса —
GUI/HUD-приложение и фоновый демон — общающихся через D-Bus. Во время шаринга
экрана или записи HUD гарантированно не попадает в кадр независимо от того,
как Discord / Zoom / браузер захватывают рабочий стол.

---

## Ключевые возможности

- **Три режима ввода** для общения с AI: чат текстом, скриншот → локальный OCR,
  голос → Whisper ASR.
- **Профили с контекстом**: загружаешь резюме (PDF / DOCX / MD), вставляешь
  описание целевой вакансии, добавляешь факты о себе, пишешь кастомный
  system prompt — демон собирает всё это в запрос к LLM.
- **Стриминг ответов LLM** через D-Bus-сигналы, история чата шифруется на диске.
- **Невидимость при шаринге через RVMS** — виртуальный монитор, создаваемый
  через Mutter ScreenCast D-Bus API, отдаётся в шаринг; HUD живёт на
  физическом мониторе и в кадр никогда не попадает.
- **Секреты через GNOME Keyring** (`secret-service`).
- **Сборка в RPM** для Fedora 44+.

## Технологический стек

Проект — это Cargo workspace (Rust 2021 edition). Тяжёлых GUI-тулкитов
специально избегаем. Всё перечисленное реально используется в коде.

### GUI-приложение — `izighost` (бинарь)

| Назначение | Крейт | Версия | Заметки |
|---|---|---|---|
| Окна и рендеринг | `eframe` / `egui` | `0.34` | Immediate-mode GUI. Используется и для окна настроек, и для прозрачного безрамочного HUD-оверлея. Бэкенды: `glow`, `wgpu`; транспорты: `wayland`, `x11`. |
| XDG Desktop Portals | `ashpd` | `0.10` | Биндинги к GlobalShortcuts + ScreenshotPortal. |
| Подсветка синтаксиса | `syntect` | `5` | Pure-Rust, без привязки к GTK. |
| Рендеринг Markdown | `pulldown-cmark` | `0.12` | Общий с рендерингом промптов на стороне демона. |
| Буфер обмена | `arboard` | `3.6` | Копирование ответов и OCR-сниппетов. |
| Нативные файловые диалоги | `rfd` | `0.15` | Выбор файлов резюме / вакансии. |
| Async-рантайм | `tokio` | `1.40` | С фичей `full`. |
| IPC-клиент | `zbus` | `5` | Общение с демоном через session bus. |
| Логирование | `tracing` + `tracing-subscriber` + `tracing-appender` | `0.1` / `0.3` / `0.2` | JSON + env-filter, appender для ротации. |

### Фоновый демон — `izighost-daemon` (бинарь)

| Назначение | Крейт | Версия | Заметки |
|---|---|---|---|
| Async-рантайм | `tokio` | `1.40` | С фичей `full`. |
| IPC-сервер | `zbus` + `zvariant` | `5` | Реализует `com.izighost.Daemon` на session bus. |
| HTTP-клиент | `reqwest` | `0.12` | `rustls-tls` (без OpenSSL), `json`, `stream`, `multipart`. Для вызовов LLM и Whisper. |
| Парсинг документов | `kreuzberg` | `5.0.0-rc.10` | Pure-Rust, MIT, async. Извлечение текста из PDF / Office / изображений для загрузки резюме и вакансий. |
| OCR | `leptess` | `0.14` | Rust-биндинги к Tesseract 5 — работает in-process, не через shell. Модели скачиваются с проверкой SHA-256. |
| Хранилище секретов | `secret-service` | `5` | С фичей `rt-tokio-crypto-rust`. Доступ к GNOME Keyring. |
| Сериализация | `serde` + `serde_json` + `serde_yaml` + `toml` | `1` / `1` / `0.9` / `0.8` | Профили в YAML, конфиг в TOML, IPC в JSON. |
| Работа с изображениями | `image` | `0.25` | Декодирование PNG для препроцессинга OCR; только фича `png`. |
| Криптография | `aes` + `cbc` + `sha2` + `base64` | `0.8` / `0.1` / `0.10` / `0.22` | AES-256-CBC для шифрования истории чата на диске; SHA-256 для контроля целостности моделей. |
| Процессы / FS | `libc` | `0.2` | Низкоуровневые операции с правами файлов (например, `0o700` на каталог истории). |
| Время | `chrono` | `0.4` | Таймстампы в истории чата. |
| Ошибки | `anyhow` + `thiserror` | `1` / `1` | Ошибки приложения vs. библиотечные типы ошибок. |
| CLI-аргументы | `clap` | `4` | С фичей `derive`. |
| Логирование | `tracing` + `tracing-subscriber` + `tracing-appender` | `0.1` / `0.3` / `0.2` | Тот же стек, что и в GUI. |

### Интеграция с Linux / системой

- **Wayland / GNOME 48+** — целевая платформа (Wayland-сессия, PipeWire,
  Mutter 48+).
- **Mutter ScreenCast D-Bus API** (`org.gnome.Mutter.ScreenCast`) —
  создание виртуального монитора и получение PipeWire node ID для
  loopback-потока RVMS.
- **Mutter RemoteDesktop / EIS** — проксирование ввода из окна-зеркала
  в виртуальный монитор.
- **PipeWire + GStreamer** — захват аудио для ASR и транспорт видео для
  loopback-потока.
- **GNOME Shell Extension** (`extension/`,
  `window-pin-bridge@gnome.extension`) — небольшой D-Bus-мост, который
  удерживает HUD поверх остальных окон на Wayland, где `wlr-layer-shell`
  недоступен.
- **XDG Desktop Portals** — `GlobalShortcuts` (для триггеров вроде
  `Super+Shift+S`) и `Screenshot` portal.
- **systemd --user** — демон работает как юзер-сервис.
- **GNOME Keyring / Secret Service** — хранение API-ключей.

### LLM / ML

- **OpenAI-совместимые LLM-клиенты** — pluggable, стриминговые ответы
  по HTTP.
- **Whisper ASR** через OpenAI-совместимый API; в качестве fallback
  поставляется Python-скрипт `faster-whisper` в `daemon/src/audio/`.

### Сборка и дистрибуция

- **Cargo workspace** из трёх членов: `crates/common`, `app`, `daemon`.
- **Rust 2021 edition**.
- **RPM**-упаковка для **Fedora 44+** (`packaging/fedora/sources/`).
- Один `cargo build --release` собирает оба бинаря.

## Архитектура

```
                Физический монитор (Экран 1) — то, что видишь ты
        ┌──────────────────────────────────────────────────┐
        │   Окно-зеркало (PipeWire / GStreamer)            │
        │   ┌──────────────────────────────────────────┐   │
        │   │  IDE, браузер и т.д. (работают на Экране 2)│  │
        │   │       ┌──────────────────────────┐       │   │
        │   │       │  HUD-оверлей (egui)      │       │   │
        │   │       │  виден только тебе       │       │   │
        │   │       └──────────────────────────┘       │   │
        │   └──────────────────────────────────────────┘   │
        └──────────────────────────────────────────────────┘
                                ▲ PipeWire-поток
                                │
                Виртуальный монитор (Экран 2) — то, что шарится
        ┌──────────────────────────────────────────────────┐
        │   IDE / браузер — больше ничего здесь нет        │
        └──────────────────────────────────────────────────┘

        ┌──────────────────────┐     D-Bus (session bus)     ┌──────────────────────────┐
        │  izighost (egui)     │ ◀────────────────────────▶ │  izighost-daemon         │
        │  - Окно настроек     │   com.izighost.Daemon v1    │  - tokio runtime         │
        │  - HUD-оверлей       │                             │  - zbus-сервер           │
        │  - chat, hotkeys,    │                             │  - kreuzberg / leptess   │
        │    screenshot,       │                             │  - reqwest (LLM + ASR)  │
        │    profile, dbus     │                             │  - PipeWire / GStreamer  │
        └──────────────────────┘                             │  - RVMS (Mutter)         │
                                                            └──────────────────────────┘
```

**D-Bus интерфейс** (`com.izighost.Daemon`, путь объекта
`/com/izighost/Daemon`):

- Методы: `StartRvms`, `StopRvms`, `SendChatMessage`, `TriggerOcr`,
  `StartListening`, `StopListening`.
- Сигналы: `ChatChunk`, `ChatCompleted`, `OcrCompleted`, `AsrCompleted`,
  `ErrorOccurred`.

## Структура репозитория

```
izighost/
├── crates/common/         # общая библиотека (izighost-common)
├── app/                   # izighost — eframe GUI (Настройки + HUD)
│   └── src/
│       ├── main.rs        # точка входа, ещё ставит GNOME Shell ext
│       ├── window/        # onboarding, preferences, HUD
│       ├── chat/          # рендеринг чата и его состояние
│       ├── hotkeys/       # интеграция с XDG GlobalShortcuts
│       ├── screenshot/    # клиент XDG Screenshot portal
│       ├── profile/       # редактор профилей
│       ├── visibility/    # стейт-машина show/hide HUD
│       ├── markdown/      # рендерер pulldown-cmark + syntect
│       ├── dbus/          # D-Bus прокси-клиент
│       ├── error/         # унифицированный отчёт об ошибках
│       └── onboarding/    # мастер первого запуска
├── daemon/                # izighost-daemon — zbus + tokio фоновый сервис
│   └── src/
│       ├── main.rs        # точка входа tokio runtime
│       ├── lib.rs         # библиотека для интеграционных тестов
│       ├── dbus_server.rs # реализация D-Bus интерфейса
│       ├── config.rs      # YAML/TOML конфиг
│       ├── context_store.rs # зашифрованная история чата
│       ├── prompt_assembler.rs
│       ├── rvms.rs        # виртуальный монитор через Mutter ScreenCast
│       ├── audio.rs       # захват PipeWire + Whisper
│       ├── profile/       # IO профилей в YAML
│       ├── llm/           # OpenAI-совместимый LLM-клиент
│       └── ocr/           # воркер leptess
├── extension/             # GNOME Shell extension (window-pin bridge)
├── installer/
│   ├── systemd/           # izighost-daemon.service
│   └── dbus/              # com.izighost.Daemon.service
└── packaging/
    └── fedora/sources/    # RPM-упаковка
```

## Требования

- Fedora 44+ (или любой дистрибутив с GNOME 48+, Wayland, PipeWire).
- Tesseract 5 с языковыми пакетами `eng` и `rus` (скачиваются автоматически
  при первом запуске, проверяются по SHA-256).
- OpenAI-совместимый эндпоинт для стриминга LLM; опционально — отдельный
  OpenAI-совместимый ключ для Whisper ASR.

## Сборка

```sh
git clone https://github.com/r4r1ty-tech/izighost
cd izighost
cargo build --release
./target/release/izighost          # GUI + HUD
./target/release/izighost-daemon   # фоновый демон
```

RPM-пакеты лежат в `packaging/fedora/sources/`.

## Лицензия

[WTFPL](https://en.wikipedia.org/wiki/WTFPL) — Do What The Fuck You Want to
Public License. См. [`LICENSE`](./LICENSE).

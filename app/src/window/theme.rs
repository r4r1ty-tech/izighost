/// Единая дизайн-система IziGhost.
/// Все цвета, фреймы и стилевые утилиты для консистентного UI.
use eframe::egui;
use eframe::egui::{Color32, RichText, Stroke, Vec2};

// ─── Палитра цветов ──────────────────────────────────────────────────────────

/// Основной фон приложения
pub const BG_PRIMARY: Color32 = Color32::from_rgb(20, 20, 22);
/// Фон боковых панелей (чуть темнее)
pub const BG_SIDEBAR: Color32 = Color32::from_rgb(15, 15, 17);
/// Фон карточек и секций
pub const BG_CARD: Color32 = Color32::from_rgb(28, 28, 32);
/// Фон кнопок неактивных
pub const BG_BUTTON: Color32 = Color32::from_rgb(40, 40, 44);
/// Акцентный цвет (индиго)
pub const ACCENT: Color32 = Color32::from_rgb(99, 102, 241);
/// Зелёный (успех, активный)
pub const GREEN: Color32 = Color32::from_rgb(16, 185, 129);
/// Красный (ошибка, удаление)
pub const RED: Color32 = Color32::from_rgb(220, 38, 38);
/// Красный мягкий (предупреждение)
pub const RED_SOFT: Color32 = Color32::from_rgb(180, 40, 40);
/// Оранжевый (предупреждение фон)
pub const WARN_BG: Color32 = Color32::from_rgb(100, 40, 10);
/// Текст основной
pub const TEXT_PRIMARY: Color32 = Color32::WHITE;
/// Текст вторичный
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(180, 180, 185);
/// Текст приглушённый (заголовки секций)
pub const TEXT_MUTED: Color32 = Color32::from_rgb(110, 110, 120);
/// Текст совсем тусклый (подсказки)
pub const TEXT_HINT: Color32 = Color32::from_rgb(90, 90, 100);
/// Граница неактивная
pub const BORDER_SUBTLE: Color32 = Color32::from_rgb(45, 45, 50);

// ─── Фреймы ──────────────────────────────────────────────────────────────────

/// Секционный фрейм с заголовком — для группировки полей формы
pub fn section_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(BG_CARD)
        .stroke(Stroke::new(1.0, BORDER_SUBTLE))
        .inner_margin(12.0)
        .corner_radius(8.0)
}

/// Фрейм для предупреждений/уведомлений
pub fn notification_frame(is_error: bool) -> egui::Frame {
    let bg = if is_error {
        Color32::from_rgba_unmultiplied(220, 38, 38, 40)
    } else {
        Color32::from_rgba_unmultiplied(16, 185, 129, 40)
    };
    let border = if is_error { RED } else { GREEN };

    egui::Frame::NONE
        .fill(bg)
        .stroke(Stroke::new(1.0, border))
        .inner_margin(8.0)
        .corner_radius(6.0)
}

// ─── Виджеты ─────────────────────────────────────────────────────────────────

/// Заголовок секции (приглушённый, капсом)
pub fn section_heading(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).strong().size(11.0).color(TEXT_MUTED));
    ui.add_space(4.0);
}

/// Подзаголовок секции (белый, жирный)
pub fn section_title(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).strong().size(13.0).color(TEXT_PRIMARY));
    ui.add_space(6.0);
}

/// Стандартная акцентная кнопка (индиго)
pub fn accent_button(ui: &mut egui::Ui, text: &str, width: f32) -> egui::Response {
    ui.add_sized(
        [width, 34.0],
        egui::Button::new(RichText::new(text).strong().color(TEXT_PRIMARY))
            .fill(ACCENT)
            .corner_radius(6.0),
    )
}

/// Зелёная кнопка (успех/подтверждение)
pub fn green_button(ui: &mut egui::Ui, text: &str, width: f32) -> egui::Response {
    ui.add_sized(
        [width, 34.0],
        egui::Button::new(RichText::new(text).strong().color(TEXT_PRIMARY))
            .fill(GREEN)
            .corner_radius(6.0),
    )
}

/// Красная кнопка (удаление/опасное действие)
pub fn danger_button(ui: &mut egui::Ui, text: &str, width: f32) -> egui::Response {
    ui.add_sized(
        [width, 34.0],
        egui::Button::new(RichText::new(text).strong().color(TEXT_PRIMARY))
            .fill(RED)
            .corner_radius(6.0),
    )
}

/// Подпись к полю формы
pub fn field_label(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).color(TEXT_SECONDARY));
}

/// Строка формы: подпись + текстовое поле
pub fn form_row(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [120.0, 20.0],
            egui::Label::new(RichText::new(label).color(TEXT_SECONDARY)),
        );
        ui.add(egui::TextEdit::singleline(value).desired_width(ui.available_width()));
    });
}

/// Строка формы с паролем: подпись + скрытое поле + кнопка показать/скрыть
pub fn form_password_row(ui: &mut egui::Ui, label: &str, value: &mut String, visible: &mut bool) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [120.0, 20.0],
            egui::Label::new(RichText::new(label).color(TEXT_SECONDARY)),
        );
        ui.add(
            egui::TextEdit::singleline(value)
                .password(!*visible)
                .desired_width(ui.available_width() - 76.0),
        );
        if ui
            .add(
                egui::Button::new(
                    RichText::new(if *visible {
                        "Скрыть"
                    } else {
                        "Показать"
                    })
                    .size(11.0)
                    .color(TEXT_SECONDARY),
                )
                .fill(BG_BUTTON)
                .corner_radius(4.0),
            )
            .clicked()
        {
            *visible = !*visible;
        }
    });
}

/// Статус индикатор (цветная точка + текст)
pub fn status_indicator(ui: &mut egui::Ui, label: &str, value: &str, is_ok: bool) {
    ui.horizontal(|ui| {
        let color = if is_ok { GREEN } else { RED };
        let (rect, _) = ui.allocate_exact_size(Vec2::new(8.0, 8.0), egui::Sense::hover());
        ui.painter().circle_filled(rect.center(), 4.0, color);
        field_label(ui, label);
        ui.label(RichText::new(value).strong().color(color));
    });
}

/// Разделитель с отступами
pub fn spaced_separator(ui: &mut egui::Ui) {
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(6.0);
}

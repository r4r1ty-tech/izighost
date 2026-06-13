import Clutter from 'gi://Clutter';
import St from 'gi://St';
import GObject from 'gi://GObject';
import Cogl from 'gi://Cogl';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

// Кастомный виджет оверлея, который скрывается из записи экрана (screencast)
export const IziGhostHUDWidget = GObject.registerClass(
    {
        GTypeName: 'IziGhostHUDWidget',
    },
    class IziGhostHUDWidget extends St.BoxLayout {
        _init(hudManager) {
            super._init({
                style_class: 'izighost-hud-window',
                vertical: true,
                reactive: true,
            });

            this._hudManager = hudManager;

            // Заголовок
            let headerBox = new St.BoxLayout({
                style_class: 'izighost-hud-header',
                vertical: false,
            });
            
            let titleLabel = new St.Label({
                text: 'IziGhost HUD: Hello World!',
                style_class: 'izighost-hud-title',
            });
            headerBox.add_child(titleLabel);
            this.add_child(headerBox);

            // Текстовое описание
            let description = new St.Label({
                text: 'Этот оверлей виден вам, но полностью невидим при трансляции или записи экрана.',
            });
            this.add_child(description);
        }

        // Переопределение низкоуровневой функции отрисовки актора
        vfunc_paint(paintContext) {
            let fb = paintContext.get_framebuffer();

            // Рисуем виджет ТОЛЬКО на Onscreen буфере (вывод на физический монитор)
            if (fb instanceof Cogl.Onscreen) {
                super.vfunc_paint(paintContext);
            } else {
                // Игнорируем отрисовку на Offscreen буфере (PipeWire screencast/запись)
            }
        }
    }
);

export class IziGhostHUD {
    constructor(extension) {
        this._extension = extension;
        this._widget = null;
        this._visible = false;
        
        // Размещаем по умолчанию в левом верхнем углу
        this._x = 100;
        this._y = 100;
    }

    show() {
        if (this._visible) return;

        if (!this._widget) {
            this._widget = new IziGhostHUDWidget(this);
            this._widget.set_position(this._x, this._y);
        }

        Main.layoutManager.uiGroup.add_child(this._widget);
        this._visible = true;
        this._extension.indicator.updateLamp(true);
    }

    hide() {
        if (!this._visible || !this._widget) return;
        
        Main.layoutManager.uiGroup.remove_child(this._widget);
        this._visible = false;
        this._extension.indicator.updateLamp(false);
    }

    toggle() {
        if (this._visible) {
            this.hide();
        } else {
            this.show();
        }
    }

    destroy() {
        this.hide();
        if (this._widget) {
            this._widget.destroy();
            this._widget = null;
        }
    }
}

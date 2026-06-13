import GObject from 'gi://GObject';
import St from 'gi://St';
import Clutter from 'gi://Clutter';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

export const IziGhostIndicator = GObject.registerClass(
    {
        GTypeName: 'IziGhostIndicator',
    },
    class IziGhostIndicator extends St.Bin {
        _init(extension) {
            super._init({
                style_class: 'panel-button',
                reactive: true,
                can_focus: true,
                track_hover: true,
            });
            this._extension = extension;

            // Изначально выключен — красный статус 🔴 Off
            this._statusLabel = new St.Label({
                text: '🔴 Off',
                y_align: Clutter.ActorAlign.CENTER,
            });
            this.set_child(this._statusLabel);

            // Клик мыши напрямую переключает оверлей чата
            this.connect('button-press-event', (actor, event) => {
                this._extension.hud.toggle();
                return Clutter.EVENT_STOP;
            });

            // Добавляем напрямую в статус-бар GNOME
            Main.panel._rightBox.insert_child_at_index(this, 0);
        }

        updateLamp(visible) {
            if (visible) {
                this._statusLabel.set_text('🟢 On'); // Включен — зеленый
            } else {
                this._statusLabel.set_text('🔴 Off'); // Выключен — красный
            }
        }

        destroy() {
            // Удаляем виджет с панели при отключении расширения
            Main.panel._rightBox.remove_child(this);
            super.destroy();
        }
    }
);

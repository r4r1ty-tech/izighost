import GObject from 'gi://GObject';
import St from 'gi://St';
import Clutter from 'gi://Clutter';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';

export const IziGhostIndicator = GObject.registerClass(
    {
        GTypeName: 'IziGhostIndicator',
    },
    class IziGhostIndicator extends PanelMenu.Button {
        _init(extension) {
            super._init(0.5, 'IziGhost Indicator', false);
            this._extension = extension;

            // Лампочка-индикатор (🟢 = скрыто, 🔴 = активно)
            this._lamp = new St.Label({
                text: '🟢',
                y_align: Clutter.ActorAlign.CENTER,
            });
            this.add_child(this._lamp);

            // Клик по индикатору переключает видимость HUD
            this.connect('button-press-event', () => {
                this._extension.hud.toggle();
                return Clutter.EVENT_STOP;
            });

            // Добавляем в статус-бар
            Main.panel.addToStatusArea('izighost-indicator', this);
        }

        updateLamp(visible) {
            if (visible) {
                this._lamp.set_text('🔴');
            } else {
                this._lamp.set_text('🟢');
            }
        }

        destroy() {
            super.destroy();
        }
    }
);

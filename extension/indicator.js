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

            // Текстовый статус на панели статус-бара
            this._statusLabel = new St.Label({
                text: 'IziGhost: 🟢 Off',
                y_align: Clutter.ActorAlign.CENTER,
            });
            this.add_child(this._statusLabel);

            // Клик по индикатору переключает видимость HUD
            this.connect('button-press-event', () => {
                this._extension.hud.toggle();
                return Clutter.EVENT_STOP;
            });

            // Добавляем в статус-бар GNOME
            Main.panel.addToStatusArea('izighost-indicator', this);
        }

        updateLamp(visible) {
            if (visible) {
                this._statusLabel.set_text('IziGhost: 🔴 On');
            } else {
                this._statusLabel.set_text('IziGhost: 🟢 Off');
            }
        }

        destroy() {
            super.destroy();
        }
    }
);

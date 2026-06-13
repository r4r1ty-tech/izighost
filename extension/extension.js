import { Extension } from 'resource:///org/gnome/shell/extensions/extension.js';
import { IziGhostIndicator } from './indicator.js';
import { IziGhostHUD } from './hud.js';
import { IziGhostDBus } from './dbus.js';

export default class IziGhostExtension extends Extension {
    enable() {
        console.log('IziGhost: Включение расширения...');
        
        // Инициализируем D-Bus прокси
        this._dbus = new IziGhostDBus(this);
        
        // Создаем HUD (оверлей чата)
        this._hud = new IziGhostHUD(this);
        
        // Создаем индикатор в верхней панели
        this._indicator = new IziGhostIndicator(this);
    }

    disable() {
        console.log('IziGhost: Отключение расширения...');
        
        if (this._indicator) {
            this._indicator.destroy();
            this._indicator = null;
        }
        
        if (this._hud) {
            this._hud.destroy();
            this._hud = null;
        }
        
        if (this._dbus) {
            this._dbus.destroy();
            this._dbus = null;
        }
    }

    get dbus() {
        return this._dbus;
    }

    get hud() {
        return this._hud;
    }

    get indicator() {
        return this._indicator;
    }
}

import Gio from 'gi://Gio';
import Meta from 'gi://Meta';
import { Extension } from 'resource:///org/gnome/shell/extensions/extension.js';

const XML_INTERFACE_SPECIFICATION = `
<node>
  <interface name="org.gnome.Shell.Extensions.WindowPinBridge">
    <method name="PinWindowByPid">
      <arg type="u" direction="in" name="pid"/>
      <arg type="b" direction="out" name="success"/>
    </method>
    <method name="UnpinWindowByPid">
      <arg type="u" direction="in" name="pid"/>
      <arg type="b" direction="out" name="success"/>
    </method>
  </interface>
</node>`;

export default class WindowPinBridgeExtension extends Extension {
    enable() {
        this._dbusObject = Gio.DBusExportedObject.wrapJSObject(XML_INTERFACE_SPECIFICATION, this);
        this._dbusObject.export(Gio.DBus.session, '/org/gnome/Shell/Extensions/WindowPinBridge');
    }

    disable() {
        if (this._dbusObject) {
            this._dbusObject.unexport();
            this._dbusObject = null;
        }
    }

    PinWindowByPid(pid) {
        const windowList = global.display.get_tab_list(Meta.TabList.NORMAL_ALL, null);
        for (const windowInstance of windowList) {
            if (windowInstance.get_pid() === pid) {
                windowInstance.make_above();
                return [true];
            }
        }
        return [false];
    }

    UnpinWindowByPid(pid) {
        const windowList = global.display.get_tab_list(Meta.TabList.NORMAL_ALL, null);
        for (const windowInstance of windowList) {
            if (windowInstance.get_pid() === pid) {
                windowInstance.unmake_above();
                return [true];
            }
        }
        return [false];
    }
}

import Gio from 'gi://Gio';
import Meta from 'gi://Meta';
import GLib from 'gi://GLib';
import Clutter from 'gi://Clutter';
import Shell from 'gi://Shell';
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
    <method name="CaptureScreenshot">
      <arg type="u" direction="in" name="monitor_index"/>
      <arg type="s" direction="in" name="filepath"/>
      <arg type="b" direction="out" name="success"/>
    </method>
    <method name="CaptureVirtualMonitor">
      <arg type="s" direction="in" name="filepath"/>
      <arg type="b" direction="out" name="success"/>
    </method>
    <method name="WarpCursor">
      <arg type="i" direction="in" name="x"/>
      <arg type="i" direction="in" name="y"/>
      <arg type="b" direction="out" name="success"/>
    </method>
    <method name="SaveCursorPosition">
      <arg type="b" direction="out" name="success"/>
    </method>
    <method name="RestoreCursorPosition">
      <arg type="b" direction="out" name="success"/>
    </method>
    <method name="WarpToMonitor">
      <arg type="u" direction="in" name="monitor_index"/>
      <arg type="b" direction="out" name="success"/>
    </method>
    <method name="WarpToVirtualMonitor">
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

    CaptureScreenshot(monitor_index, filepath, invocation) {
        try {
            console.log(`[WindowPinBridge] CaptureScreenshot: monitor_index=${monitor_index}, filepath=${filepath}`);
            const nMonitors = global.display.get_n_monitors();
            if (monitor_index >= nMonitors) {
                console.log(`[WindowPinBridge] CaptureScreenshot error: monitor_index out of bounds (nMonitors=${nMonitors})`);
                invocation.return_value(new GLib.Variant('(b)', [false]));
                return;
            }
            const geometry = global.display.get_monitor_geometry(monitor_index);
            const file = Gio.File.new_for_path(filepath);
            const screenshot = new Shell.Screenshot();
            console.log(`[WindowPinBridge] CaptureScreenshot: geometry={x:${geometry.x}, y:${geometry.y}, w:${geometry.width}, h:${geometry.height}}`);
            screenshot.screenshot_area(
                geometry.x,
                geometry.y,
                geometry.width,
                geometry.height,
                file,
                (obj, res) => {
                    try {
                        const success = obj.screenshot_area_finish(res);
                        console.log(`[WindowPinBridge] CaptureScreenshot callback: success=${success}`);
                        invocation.return_value(new GLib.Variant('(b)', [success]));
                    } catch (err) {
                        console.log(`[WindowPinBridge] CaptureScreenshot callback error: ${err}`);
                        logError(err);
                        invocation.return_value(new GLib.Variant('(b)', [false]));
                    }
                }
            );
        } catch (e) {
            console.log(`[WindowPinBridge] CaptureScreenshot catch error: ${e}`);
            logError(e);
            invocation.return_value(new GLib.Variant('(b)', [false]));
        }
    }

    CaptureVirtualMonitor(filepath, invocation) {
        try {
            console.log(`[WindowPinBridge] CaptureVirtualMonitor called: filepath=${filepath}`);
            const nMonitors = global.display.get_n_monitors();
            const primaryIndex = global.display.get_primary_monitor();
            let targetIndex = primaryIndex;
            for (let i = 0; i < nMonitors; i++) {
                if (i !== primaryIndex) {
                    targetIndex = i;
                    break;
                }
            }
            console.log(`[WindowPinBridge] CaptureVirtualMonitor targetIndex determined: ${targetIndex}`);
            this.CaptureScreenshot(targetIndex, filepath, invocation);
        } catch (e) {
            console.log(`[WindowPinBridge] CaptureVirtualMonitor catch error: ${e}`);
            logError(e);
            invocation.return_value(new GLib.Variant('(b)', [false]));
        }
    }

    WarpCursor(x, y) {
        try {
            const seat = Clutter.get_default_backend().get_default_seat();
            seat.warp_pointer(x, y);
            return [true];
        } catch (e) {
            logError(e);
            return [false];
        }
    }

    SaveCursorPosition() {
        try {
            const [x, y, mask] = global.get_pointer();
            this._savedCursorX = x;
            this._savedCursorY = y;
            return [true];
        } catch (e) {
            logError(e);
            return [false];
        }
    }

    RestoreCursorPosition() {
        try {
            if (this._savedCursorX !== undefined && this._savedCursorY !== undefined) {
                const seat = Clutter.get_default_backend().get_default_seat();
                seat.warp_pointer(this._savedCursorX, this._savedCursorY);
                return [true];
            }
            return [false];
        } catch (e) {
            logError(e);
            return [false];
        }
    }

    WarpToMonitor(monitor_index) {
        try {
            const nMonitors = global.display.get_n_monitors();
            if (monitor_index >= nMonitors) {
                return [false];
            }
            const geometry = global.display.get_monitor_geometry(monitor_index);
            const targetX = Math.round(geometry.x + geometry.width / 2);
            const targetY = Math.round(geometry.y + geometry.height / 2);
            const seat = Clutter.get_default_backend().get_default_seat();
            seat.warp_pointer(targetX, targetY);
            return [true];
        } catch (e) {
            logError(e);
            return [false];
        }
    }

    WarpToVirtualMonitor() {
        try {
            const nMonitors = global.display.get_n_monitors();
            const primaryIndex = global.display.get_primary_monitor();
            let targetIndex = primaryIndex;
            for (let i = 0; i < nMonitors; i++) {
                if (i !== primaryIndex) {
                    targetIndex = i;
                    break;
                }
            }
            return this.WarpToMonitor(targetIndex);
        } catch (e) {
            logError(e);
            return [false];
        }
    }
}

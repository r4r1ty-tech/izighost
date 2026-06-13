import sys
import gi
gi.require_version('Gtk', '3.0')
gi.require_version('Gst', '1.0')
from gi.repository import Gtk, Gst, Gio, GLib

def main():
    if len(sys.argv) < 4:
        print("Usage: python3 rvms_loopback.py <node_id> <rd_session_path> <stream_path>")
        sys.exit(1)
        
    node_id = int(sys.argv[1])
    rd_session_path = sys.argv[2]
    stream_path = sys.argv[3]
    
    print(f"Loopback mirror starting with PipeWire ID: {node_id}")
    print(f"RemoteDesktop session: {rd_session_path}")
    print(f"ScreenCast stream: {stream_path}")
    
    # Initialize GStreamer and GTK
    Gst.init(None)
    Gtk.init(None)
    
    # Connect to D-Bus Session Bus
    bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    
    # Create proxy for RemoteDesktop session to inject inputs
    rd_session = Gio.DBusProxy.new_sync(
        bus,
        Gio.DBusProxyFlags.NONE,
        None,
        "org.gnome.Mutter.RemoteDesktop",
        rd_session_path,
        "org.gnome.Mutter.RemoteDesktop.Session",
        None
    )
    
    # Create GTK Window
    window = Gtk.Window(title="IziGhost Loopback Mirror")
    window.set_default_size(960, 540) # 16:9 ratio
    
    # Set window events
    window.add_events(
        gi.repository.Gdk.EventMask.POINTER_MOTION_MASK |
        gi.repository.Gdk.EventMask.BUTTON_PRESS_MASK |
        gi.repository.Gdk.EventMask.BUTTON_RELEASE_MASK |
        gi.repository.Gdk.EventMask.KEY_PRESS_MASK |
        gi.repository.Gdk.EventMask.KEY_RELEASE_MASK
    )
    
    # Setup GStreamer pipeline
    pipeline_str = f"pipewiresrc path={node_id} ! queue ! videoconvert ! gtksink name=sink"
    pipeline = Gst.parse_launch(pipeline_str)
    sink = pipeline.get_by_name("sink")
    
    # Embed GTK sink widget directly
    video_widget = sink.get_property("widget")
    window.add(video_widget)
    video_widget.show()
    
    # Event Handlers
    def on_motion(widget, event):
        width = window.get_allocated_width()
        height = window.get_allocated_height()
        if width > 0 and height > 0:
            # Normalize to virtual screen coordinates (1920x1080)
            rx = (event.x / width) * 1920.0
            ry = (event.y / height) * 1080.0
            
            try:
                rd_session.call_sync(
                    "NotifyPointerMotionAbsolute",
                    GLib.Variant('(sdd)', (stream_path, rx, ry)),
                    Gio.DBusCallFlags.NONE,
                    -1,
                    None
                )
            except Exception as e:
                pass # Ignore errors if session closed

    def on_button(widget, event):
        # GTK mouse buttons: 1 -> Left, 2 -> Middle, 3 -> Right
        # Linux input button codes (from <linux/input-event-codes.h>):
        # BTN_LEFT: 272 (0x110), BTN_RIGHT: 273 (0x111), BTN_MIDDLE: 274 (0x112)
        btn_map = {1: 272, 2: 274, 3: 273}
        linux_btn = btn_map.get(event.button, 272)
        is_pressed = event.type == gi.repository.Gdk.EventType.BUTTON_PRESS
        
        try:
            rd_session.call_sync(
                "NotifyPointerButton",
                GLib.Variant('(ib)', (linux_btn, is_pressed)),
                Gio.DBusCallFlags.NONE,
                -1,
                None
            )
        except Exception as e:
            pass

    def on_key(widget, event):
        # Keysym value (e.g. Gdk.KEY_a, Gdk.KEY_Return, etc.)
        keysym = event.keyval
        is_pressed = event.type == gi.repository.Gdk.EventType.KEY_PRESS
        
        try:
            rd_session.call_sync(
                "NotifyKeyboardKeysym",
                GLib.Variant('(ub)', (keysym, is_pressed)),
                Gio.DBusCallFlags.NONE,
                -1,
                None
            )
        except Exception as e:
            pass
            
    # Connect signals
    window.connect("motion-notify-event", on_motion)
    window.connect("button-press-event", on_button)
    window.connect("button-release-event", on_button)
    window.connect("key-press-event", on_key)
    window.connect("key-release-event", on_key)
    
    # Close handler
    def on_destroy(widget):
        print("Mirror window closed, stopping GStreamer...")
        pipeline.set_state(Gst.State.NULL)
        Gtk.main_quit()
        
    window.connect("destroy", on_destroy)
    
    # Start playback
    pipeline.set_state(Gst.State.PLAYING)
    window.show_all()
    
    # Run GTK loop
    Gtk.main()

if __name__ == '__main__':
    main()

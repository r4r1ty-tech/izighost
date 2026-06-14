import sys
import gi
gi.require_version('Gtk', '3.0')
gi.require_version('Gst', '1.0')
from gi.repository import Gtk, Gst, Gio, GLib

def main():
    print("[РВМС] Запуск скрипта rvms_loopback.py...")
    if len(sys.argv) < 4:
        print("[РВМС] Ошибка: Недостаточно аргументов для запуска!")
        print("Использование: python3 rvms_loopback.py <node_id> <rd_session_path> <stream_path>")
        sys.exit(1)
        
    node_id = int(sys.argv[1])
    rd_session_path = sys.argv[2]
    stream_path = sys.argv[3]
    
    print(f"[РВМС] Идентификатор PipeWire ID: {node_id}")
    print(f"[РВМС] Путь RemoteDesktop сессии: {rd_session_path}")
    print(f"[РВМС] Путь ScreenCast стрима: {stream_path}")
    
    # Инициализация GStreamer и GTK
    print("[РВМС] Инициализация GStreamer и GTK...")
    Gst.init(None)
    Gtk.init(None)
    
    # Подключение к D-Bus Session Bus
    print("[РВМС] Подключение к сессионной шине D-Bus...")
    bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    
    # Создание прокси для RemoteDesktop сессии для инъекции событий ввода
    print("[РВМС] Создание D-Bus прокси для org.gnome.Mutter.RemoteDesktop.Session...")
    rd_session = Gio.DBusProxy.new_sync(
        bus,
        Gio.DBusProxyFlags.NONE,
        None,
        "org.gnome.Mutter.RemoteDesktop",
        rd_session_path,
        "org.gnome.Mutter.RemoteDesktop.Session",
        None
    )
    
    # Создание окна GTK
    print("[РВМС] Создание главного окна GTK...")
    window = Gtk.Window(title="IziGhost Loopback Mirror")
    window.set_default_size(960, 540) # Соотношение 16:9
    
    # Настройка отслеживания событий окна
    window.add_events(
        gi.repository.Gdk.EventMask.POINTER_MOTION_MASK |
        gi.repository.Gdk.EventMask.BUTTON_PRESS_MASK |
        gi.repository.Gdk.EventMask.BUTTON_RELEASE_MASK |
        gi.repository.Gdk.EventMask.KEY_PRESS_MASK |
        gi.repository.Gdk.EventMask.KEY_RELEASE_MASK
    )
    
    # Настройка GStreamer конвейера
    pipeline_str = (
        f"pipewiresrc path={node_id} keepalive-time=1000 ! "
        f"queue max-size-buffers=3 max-size-bytes=0 max-size-time=0 leaky=downstream ! "
        f"video/x-raw,width=1920,height=1080,max-framerate=60/1 ! "
        f"videoconvert ! "
        f"gtksink name=sink enable-last-sample=false sync=false async=false"
    )
    print(f"[РВМС] Сборка GStreamer конвейера: {pipeline_str}")
    pipeline = Gst.parse_launch(pipeline_str)
    sink = pipeline.get_by_name("sink")
    
    # Отслеживание событий на шине GStreamer
    bus_gst = pipeline.get_bus()
    bus_gst.add_signal_watch()
    
    def on_bus_message(bus, message):
        t = message.type
        if t == Gst.MessageType.ERROR:
            err, debug = message.parse_error()
            print(f"[РВМС:Ошибка GStreamer] Ошибка: {err}, Подробности: {debug}")
        elif t == Gst.MessageType.WARNING:
            err, debug = message.parse_warning()
            print(f"[РВМС:Предупреждение GStreamer] Предупреждение: {err}, Подробности: {debug}")
        elif t == Gst.MessageType.EOS:
            print("[РВМС:GStreamer] Получен маркер EOS (Конец потока)")
            
    bus_gst.connect("message", on_bus_message)
    
    # Встраивание виджета gtksink напрямую в окно
    print("[РВМС] Встраивание видео-виджета gtksink в окно...")
    video_widget = sink.get_property("widget")
    window.add(video_widget)
    video_widget.show()
    
    # Обработчики событий
    def on_motion(widget, event):
        width = window.get_allocated_width()
        height = window.get_allocated_height()
        if width > 0 and height > 0:
            # Масштабирование в координаты виртуального экрана (1920x1080)
            rx = (event.x / width) * 1920.0
            ry = (event.y / height) * 1080.0
            
            try:
                print(f"[РВМС] Движение мыши: x={event.x:.1f}, y={event.y:.1f} -> в вирт: rx={rx:.1f}, ry={ry:.1f}")
                rd_session.call(
                    "NotifyPointerMotionAbsolute",
                    GLib.Variant('(sdd)', (stream_path, rx, ry)),
                    Gio.DBusCallFlags.NONE,
                    -1,
                    None,
                    None,
                    None
                )
            except Exception as e:
                print(f"[РВМС] Ошибка инъекции движения мыши: {e}")
 
    def on_button(widget, event):
        # Коды кнопок мыши Linux (из <linux/input-event-codes.h>):
        # BTN_LEFT: 272, BTN_RIGHT: 273, BTN_MIDDLE: 274
        btn_map = {1: 272, 2: 274, 3: 273}
        linux_btn = btn_map.get(event.button, 272)
        is_pressed = event.type == gi.repository.Gdk.EventType.BUTTON_PRESS
        
        try:
            print(f"[РВМС] Кнопка мыши: код={linux_btn}, зажата={is_pressed}")
            rd_session.call(
                "NotifyPointerButton",
                GLib.Variant('(ib)', (linux_btn, is_pressed)),
                Gio.DBusCallFlags.NONE,
                -1,
                None,
                None,
                None
            )
        except Exception as e:
            print(f"[РВМС] Ошибка инъекции нажатия мыши: {e}")
 
    def on_key(widget, event):
        keysym = event.keyval
        is_pressed = event.type == gi.repository.Gdk.EventType.KEY_PRESS
        
        try:
            print(f"[РВМС] Клавиатура: keysym={keysym}, зажата={is_pressed}")
            rd_session.call(
                "NotifyKeyboardKeysym",
                GLib.Variant('(ub)', (keysym, is_pressed)),
                Gio.DBusCallFlags.NONE,
                -1,
                None,
                None,
                None
            )
        except Exception as e:
            print(f"[РВМС] Ошибка инъекции клавиши: {e}")
            
    # Подключение сигналов событий ввода
    print("[РВМС] Подключение обработчиков сигналов GTK...")
    window.connect("motion-notify-event", on_motion)
    window.connect("button-press-event", on_button)
    window.connect("button-release-event", on_button)
    window.connect("key-press-event", on_key)
    window.connect("key-release-event", on_key)
    
    # Обработчик закрытия окна
    def on_destroy(widget):
        print("[РВМС] Окно зеркала закрыто пользователем, остановка GStreamer конвейера...")
        pipeline.set_state(Gst.State.NULL)
        Gtk.main_quit()
        
    window.connect("destroy", on_destroy)
    
    # Запуск воспроизведения
    print("[РВМС] Запуск воспроизведения GStreamer конвейера...")
    pipeline.set_state(Gst.State.PLAYING)
    
    print("[РВМС] Отображение окна GTK...")
    window.show_all()
    
    # Запуск главного цикла GTK
    print("[РВМС] Запуск главного цикла обработки событий GTK...")
    Gtk.main()
    print("[РВМС] Скрипт rvms_loopback.py успешно завершил работу.")

if __name__ == '__main__':
    main()

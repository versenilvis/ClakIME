import os
import sys
import signal
import gi
gi.require_version('Gtk', '3.0')
from gi.repository import Gtk, GLib, Gdk

output_file = sys.argv[1] if len(sys.argv) > 1 else "/tmp/gtk_oracle.out"
ready_file = sys.argv[2] if len(sys.argv) > 2 else "/tmp/gtk_oracle.ready"

class OracleWindow(Gtk.Window):
    def __init__(self):
        super().__init__(title="clak_gtk_oracle")
        self.set_default_size(500, 300)
        self.text_view = Gtk.TextView()
        self.buffer = self.text_view.get_buffer()
        self.add(self.text_view)

        self.connect("key-press-event", self.on_key_press)
        self.connect("destroy", self.on_destroy)

        GLib.unix_signal_add(GLib.PRIORITY_DEFAULT, signal.SIGTERM, self.save_and_quit)
        GLib.unix_signal_add(GLib.PRIORITY_DEFAULT, signal.SIGINT, self.save_and_quit)
        GLib.idle_add(self.signal_ready)

    def signal_ready(self):
        self.text_view.grab_focus()
        with open(ready_file, "w") as f:
            f.write("ready\n")
        return False

    def on_key_press(self, widget, event):
        # ctrl+d to save and exit
        if event.state & Gdk.ModifierType.CONTROL_MASK and event.keyval in (Gdk.KEY_d, Gdk.KEY_D):
            self.save_and_quit()
            return True
        return False

    def on_destroy(self, *args):
        self.save_and_quit()

    def save_and_quit(self):
        start = self.buffer.get_start_iter()
        end = self.buffer.get_end_iter()
        text = self.buffer.get_text(start, end, True)
        with open(output_file, "w", encoding="utf-8") as f:
            f.write(text)
        Gtk.main_quit()
        return False

def main():
    win = OracleWindow()
    win.show_all()
    Gtk.main()

if __name__ == "__main__":
    main()

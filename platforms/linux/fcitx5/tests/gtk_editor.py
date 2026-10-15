"""Synthetic keystrokes into a real Gtk.Entry on an isolated Xvfb display."""
import os
import re
import subprocess

import dbus
import gi

gi.require_version("Gtk", "3.0")
gi.require_version("GdkX11", "3.0")
from gi.repository import Gtk, GdkX11
from daemon import wait


window = Gtk.Window(title="MSIME synthetic GTK editor")
entry = Gtk.Entry()
module = os.environ.get("GTK_IM_MODULE", "fcitx")
entry.set_property("im-module", module)
window.add(entry)
window.show_all()
entry.grab_focus()
wait(lambda: window.get_window() is not None)
xid = GdkX11.X11Window.get_xid(window.get_window())
subprocess.run(["xdotool", "windowfocus", "--sync", str(xid)], check=True, timeout=5)
wait(lambda: entry.has_focus())
bus = dbus.SessionBus()
control = dbus.Interface(bus.get_object("org.fcitx.Fcitx5", "/controller"),
                         "org.fcitx.Fcitx.Controller1")
# GTK 在 X 焦点之后异步建立上下文；旧上下文仍可能返回输入法名称，必须等新前端获得焦点。
frontend = "ibus" if module == "ibus" else "dbus"
wait(lambda: re.search(rf"frontend:{frontend}\b.*\bfocus:1\b", str(control.DebugInfo())))
control.SetCurrentIM("msime")
control.Activate()
wait(lambda: str(control.CurrentInputMethod()) == "msime")
typing = subprocess.Popen(["xdotool", "type", "--clearmodifiers", "--delay", "100", "nihao "])
try:
    wait(lambda: entry.get_text() == "你好")
    assert typing.wait(timeout=5) == 0, "Synthetic key injection failed"
finally:
    if typing.poll() is None:
        typing.terminate()
        typing.wait(timeout=5)
    window.destroy()
print(f"Fcitx5 GTK3 {module} 编辑器上屏验收通过")

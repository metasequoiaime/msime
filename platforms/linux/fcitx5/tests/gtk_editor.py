"""Synthetic keystrokes into a real Gtk.Entry on an isolated Xvfb display."""
import os
import re
import subprocess
import sys

import dbus
import gi

gi.require_version("Gtk", "3.0")
gi.require_version("GdkX11", "3.0")
from gi.repository import Gtk, GdkX11
from daemon import wait


window = Gtk.Window(title="MSIME synthetic GTK editor")
entry = Gtk.Entry()
preedits = []
entry.connect("preedit-changed", lambda widget, text: preedits.append(text))
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


def type_text(text):
    typing = subprocess.Popen(["xdotool", "type", "--clearmodifiers", "--delay", "100", text])
    try:
        wait(lambda: typing.poll() is not None)
        assert typing.wait(timeout=5) == 0, "合成按键注入失败"
    finally:
        if typing.poll() is None:
            typing.terminate()
            typing.wait(timeout=5)


try:
    if "--shuangpin-helpcode" in sys.argv[1:]:
        # 自然码完整读法与首右 2.0 直接辅助码存在歧义；按键必须经过 GTK、daemon 和真实插件。
        cases = (("uurufa", "输入法"), ("kkuiffuu", "考试分数"), ("nixihrwoma", "你喜欢我吗"))
        expected = ""
        for _ in range(2):
            for code, phrase in cases:
                preedits.clear()
                type_text(code)
                wait(lambda: preedits and preedits[-1])
                assert entry.get_text() == expected, "组字期间不能提前提交歧义短词"
                subprocess.run(["xdotool", "key", "--clearmodifiers", "space"], check=True, timeout=5)
                wait(lambda: entry.get_text() != expected)
                expected += phrase
                assert entry.get_text() == expected, f"{code} 的首选上屏错误：{entry.get_text()!r}"
                wait(lambda: preedits and not preedits[-1])
        print("Fcitx5 自然码与首右 2.0 的三组词句重复上屏通过")
    else:
        type_text("nihao ")
        wait(lambda: entry.get_text() == "你好")
finally:
    window.destroy()
print(f"Fcitx5 GTK3 {module} 编辑器上屏验收通过")

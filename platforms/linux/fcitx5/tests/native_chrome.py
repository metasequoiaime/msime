"""在隔离的 X11 显示上验收默认主题的原生画布：system 主题也由水杉自己的 X11 窗口按 macOS 结构画候选窗（首行的翻页按钮、悬停、展开网格、点选上屏）。"""
import ctypes
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import time

import dbus
import dbus.mainloop.glib
import gi

gi.require_version("Gtk", "3.0")
gi.require_version("Gdk", "3.0")
gi.require_version("GdkX11", "3.0")
from gi.repository import Gdk, GdkX11, GLib, Gtk

from daemon import daemon_environment, wait
from canvas_editor import color_bounds, native_window, palette_visible, screenshot, window_geometry

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
from host_api import ResponseDecoder

LAYOUT = os.environ.get("MSIME_TEST_CANVAS_LAYOUT", "vertical")
SCALE = int(os.environ.get("GDK_SCALE", "1"))
# Adwaita 的选中底色和浅色卡片上的悬停（黑 6% 叠在白底上）。
SELECTED = (53, 132, 228)
HOVER = (240, 240, 240)


def save_image(name):
    destination = os.environ.get("MSIME_TEST_CANVAS_ARTIFACTS")
    if destination:
        directory = Path(destination) / f"native-chrome-{SCALE}x" / LAYOUT
        directory.mkdir(parents=True, exist_ok=True)
        screenshot()[0].savev(str(directory / f"{name}.png"), "png", [], [])


def pump():
    while GLib.MainContext.default().iteration(False):
        pass


def main():
    resources, library, addon = map(Path, sys.argv[1:4])
    dbus.mainloop.glib.DBusGMainLoop(set_as_default=True)
    bus = dbus.SessionBus()
    assert not bus.name_has_owner("org.fcitx.Fcitx5"), "必须使用独立 D-Bus 会话"
    bus.request_name("org.freedesktop.portal.Desktop")
    host = ctypes.CDLL(str(library))
    responses = ResponseDecoder(host)
    for name in ("msime_client_prepare_host", "msime_client_load_preferences"):
        function = getattr(host, name)
        function.argtypes = [ctypes.c_char_p, ctypes.c_size_t]
        function.restype = ctypes.c_void_p
    host.msime_client_save_preferences.argtypes = [
        ctypes.c_char_p, ctypes.c_size_t, ctypes.c_uint64, ctypes.c_char_p, ctypes.c_size_t]
    host.msime_client_save_preferences.restype = ctypes.c_void_p

    def value(pointer):
        result = responses.decode(pointer)
        assert result["ok"], "共享 Host API 请求失败"
        return result["value"]

    with tempfile.TemporaryDirectory(prefix="msime-native-chrome-") as directory:
        root = Path(directory)
        request = json.dumps({"resources": str(resources), "state_root": str(root / "state")}).encode()
        options = value(host.msime_client_prepare_host(request, len(request)))
        preferences = options["preferences"]
        preferences.update(learning=False, cloud_candidates=False, usage_reporting=False, global_theme="system",
                           candidate_theme="light", candidate_layout=LAYOUT, candidate_font_size=20,
                           candidate_translations=False, candidate_english_gloss=False, show_app_logo=True,
                           expand_candidates=True)
        preferences["navigation"]["mouse_wheel"] = True

        def publish():
            store = options["preferences_directory"].encode()
            snapshot = value(host.msime_client_load_preferences(store, len(store)))
            snapshot["preferences"] = preferences
            document = json.dumps(snapshot).encode()
            value(host.msime_client_save_preferences(store, len(store), snapshot["revision"], document, len(document)))
            (root / "options.json").write_text(json.dumps(options))

        publish()
        env = daemon_environment(root, addon)
        env.pop("MSIME_FCITX5_CANDIDATE_RENDERER", None)
        os.environ.update(env)
        os.environ["GTK_IM_MODULE"] = "fcitx"
        os.environ["GDK_BACKEND"] = "x11"
        window = control = None
        with (root / "daemon.log").open("w") as log:
            daemon = subprocess.Popen(["fcitx5", "-D", "--keep"], env=env, stdout=log, stderr=log)
            try:
                wait(lambda: bus.name_has_owner("org.fcitx.Fcitx5"))
                control = dbus.Interface(bus.get_object("org.fcitx.Fcitx5", "/controller"),
                                         "org.fcitx.Fcitx.Controller1")
                window = Gtk.Window(title="MSIME synthetic native chrome editor")
                entry = Gtk.Entry()
                entry.set_property("im-module", "fcitx")
                entry.set_valign(Gtk.Align.START)
                window.add(entry)
                window.show_all()
                entry.grab_focus()
                wait(lambda: window.get_window() is not None and window.get_window().is_viewable())
                Gdk.Display.get_default().sync()
                xid = GdkX11.X11Window.get_xid(window.get_window())
                root_xid = GdkX11.X11Window.get_xid(Gdk.get_default_root_window())
                subprocess.run(["xdotool", "windowfocus", "--sync", str(xid)], check=True, timeout=5)
                wait(lambda: "frontend:dbus" in str(control.DebugInfo()))
                next_retry = 0.0

                def ime_focused():
                    nonlocal next_retry
                    if re.search(r"frontend:dbus\b.*\bfocus:1\b", str(control.DebugInfo())):
                        return True
                    # GTK 异步建立远端上下文；握手完成之前的焦点可能丢失，只重发真实窗口焦点。
                    if time.monotonic() >= next_retry:
                        subprocess.run(["xdotool", "windowfocus", "--sync", str(root_xid)], check=True, timeout=5)
                        subprocess.run(["xdotool", "windowfocus", "--sync", str(xid)], check=True, timeout=5)
                        next_retry = time.monotonic() + 0.5
                    return False

                wait(ime_focused)
                control.SetCurrentIM("msime")
                control.Activate()
                wait(lambda: str(control.CurrentInputMethod()) == "msime")
                subprocess.run(["xdotool", "type", "--clearmodifiers", "--delay", "100", "ni"], check=True, timeout=5)
                # system 主题在 X11 上不再交给 classicui，而是水杉自己的原生窗口画。
                wait(native_window)
                wait(lambda: palette_visible((SELECTED,)))
                assert entry.get_text() == "", "预编辑文字提前进入编辑器"
                save_image("initial")
                bounds = window_geometry(native_window())
                left, top = int(bounds["X"]), int(bounds["Y"])
                width = int(bounds["WIDTH"])
                selected = color_bounds(SELECTED)

                def selected_pixels():
                    image, _ = screenshot()
                    data, stride, channels = image.get_pixels(), image.get_rowstride(), image.get_n_channels()
                    l, t, r, b = selected
                    return b"".join(data[y * stride + l * channels:y * stride + r * channels] for y in range(t, b))

                first_page = selected_pixels()
                # 首行右端的 ›：内边距 4pt，横排时最右边是展开键；按钮 28x26，首行从卡片顶边 4pt 处开始。
                arrow = 28 * SCALE
                right = left + width - 4 * SCALE - (arrow if LAYOUT == "horizontal" else 0)
                next_point = (right - arrow // 2, top + (4 + 13) * SCALE)
                previous_point = (next_point[0] - arrow, next_point[1])
                subprocess.run(["xdotool", "mousemove", *map(str, next_point), "click", "1"], check=True, timeout=5)
                wait(lambda: selected_pixels() != first_page)
                assert entry.get_text() == "", "翻页按钮错误地上屏"
                save_image("next-page")
                subprocess.run(["xdotool", "mousemove", *map(str, previous_point), "click", "1"], check=True, timeout=5)
                wait(lambda: selected_pixels() == first_page)
                # 悬停：指针移到另一行时画悬停色，键盘选中不变。
                l, t, r, b = selected
                hover_point = (r + 20 * SCALE, (t + b) // 2) if LAYOUT == "horizontal" else ((l + r) // 2, b + 20 * SCALE)
                subprocess.run(["xdotool", "mousemove", *map(str, hover_point)], check=True, timeout=5)
                wait(lambda: palette_visible((HOVER,)))
                assert color_bounds(SELECTED) == selected, "悬停改变了键盘选中项"
                save_image("hover")
                subprocess.run(["xdotool", "mousemove", "0", "0"], check=True, timeout=5)
                wait(lambda: not palette_visible((HOVER,)))
                if LAYOUT == "horizontal":
                    # 「展开候选」打开时 ↓ 把全部候选排成多行网格，Esc 收起而组字保留。
                    height = int(window_geometry(native_window())["HEIGHT"])
                    subprocess.run(["xdotool", "key", "Down"], check=True, timeout=5)
                    wait(lambda: int(window_geometry(native_window())["HEIGHT"]) > height + 20 * SCALE)
                    save_image("grid")
                    subprocess.run(["xdotool", "key", "Right"], check=True, timeout=5)
                    pump()
                    assert entry.get_text() == "", "网格里移动高亮错误地上屏"
                    subprocess.run(["xdotool", "key", "Escape"], check=True, timeout=5)
                    wait(lambda: int(window_geometry(native_window())["HEIGHT"]) == height)
                    assert entry.get_text() == "", "收起网格错误地上屏或清掉了组字"
                # 点选高亮的候选上屏，候选窗随之收起。
                selected = color_bounds(SELECTED)
                subprocess.run(["xdotool", "mousemove", str((selected[0] + selected[2]) // 2),
                                str((selected[1] + selected[3]) // 2), "click", "1"], check=True, timeout=5)
                wait(lambda: entry.get_text() == "你")
                wait(lambda: native_window() is None)
                control.Exit()
                assert daemon.wait(timeout=10) == 0, "Fcitx5 未正常退出"
                print(f"真实 GTK／X11 system 主题原生画布（{LAYOUT}）的翻页、悬停、网格与点选通过")
            finally:
                if sys.exc_info()[0] is not None:
                    save_image("failed")
                    if control is not None:
                        print(control.DebugInfo(), file=sys.stderr)
                if window is not None:
                    window.destroy()
                if daemon.poll() is None:
                    daemon.terminate()
                    daemon.wait(timeout=10)
                if sys.exc_info()[0] is not None:
                    print((root / "daemon.log").read_text(), file=sys.stderr)


if __name__ == "__main__":
    main()

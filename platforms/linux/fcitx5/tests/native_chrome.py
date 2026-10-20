"""在隔离的 X11 显示上验收默认主题的原生画布：system 主题也由水杉自己的 X11 窗口按 macOS 结构画候选窗（首行的翻页按钮、悬停、展开网格、点选上屏）。MSIME_TEST_COMPOSITED=1 时先起 xcompmgr，卡片四周多出投影边距，按钮、选中行和悬停都只在卡片里找，并检查卡片下方确有投影。"""
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
from canvas_editor import color_bounds, native_window, palette_visible, png, screenshot, window_geometry

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
from host_api import ResponseDecoder

LAYOUT = os.environ.get("MSIME_TEST_CANVAS_LAYOUT", "vertical")
SCALE = int(os.environ.get("GDK_SCALE", "1"))
COMPOSITED = os.environ.get("MSIME_TEST_COMPOSITED") == "1"
# 有合成器时卡片四周的投影边距（逻辑像素，NativeCanvasChrome.h 的 native_canvas_shadow_insets）：左、上、右、下；装饰带够高时上边不多留。
SHADOW = (12, 8, 12, 16) if COMPOSITED else (0, 0, 0, 0)
# Adwaita 的选中底色和浅色卡片上的悬停（黑 6% 叠在白底上）。
SELECTED = (53, 132, 228)
HOVER = (240, 240, 240)


def save_image(name):
    destination = os.environ.get("MSIME_TEST_CANVAS_ARTIFACTS")
    if destination:
        directory = Path(destination) / (f"native-chrome-{SCALE}x" + ("-composited" if COMPOSITED else "")) / LAYOUT
        directory.mkdir(parents=True, exist_ok=True)
        screenshot()[0].savev(str(directory / f"{name}.png"), "png", [], [])


def card_rect(band=0):
    """卡片在屏幕上的范围：窗口去掉投影边距和顶上的装饰带。有装饰带或投影时窗口向上盖住编辑器，编辑器的焦点框也是 Adwaita 蓝，投影边距里还透出编辑器，只看卡片里面。"""
    bounds = window_geometry(native_window())
    left, top = int(bounds["X"]), int(bounds["Y"])
    width, height = int(bounds["WIDTH"]), int(bounds["HEIGHT"])
    above = max(0, SHADOW[1] - band) + band
    return (left + SHADOW[0] * SCALE, top + above * SCALE,
            left + width - SHADOW[2] * SCALE, top + height - SHADOW[3] * SCALE)


def card_points(color, band=0):
    image, _ = screenshot()
    channels, stride, data = image.get_n_channels(), image.get_rowstride(), image.get_pixels()
    left, top, right, bottom = card_rect(band)
    return [(x, y) for y in range(top, bottom) for x in range(left, right)
            if data[y * stride + x * channels:y * stride + x * channels + 3] == bytes(color)]


def card_bounds(color, band=0):
    points = card_points(color, band)
    assert points, f"原生窗口的卡片里缺少配色 {color}"
    return min(x for x, _ in points), min(y for _, y in points), max(x for x, _ in points) + 1, max(y for _, y in points) + 1


def card_visible(color):
    return native_window() is not None and len(card_points(color)) >= 16


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
        window = control = compositor = None
        with (root / "daemon.log").open("w") as log:
            daemon = subprocess.Popen(["fcitx5", "-D", "--keep"], env=env, stdout=log, stderr=log)
            try:
                if COMPOSITED:
                    # -n 由 xcompmgr 自己按 alpha 合成到根窗口上；-a 交给服务器自动重定向，不按 alpha 混合，投影边距会成一圈黑，看不出投影。
                    compositor = subprocess.Popen(["xcompmgr", "-n"], env=env, stdout=log, stderr=log)
                    wait(lambda: Gdk.selection_owner_get(Gdk.Atom.intern("_NET_WM_CM_S0", False)) is not None)
                wait(lambda: bus.name_has_owner("org.fcitx.Fcitx5"))
                control = dbus.Interface(bus.get_object("org.fcitx.Fcitx5", "/controller"),
                                         "org.fcitx.Fcitx.Controller1")
                window = Gtk.Window(title="MSIME synthetic native chrome editor")
                entry = Gtk.Entry()
                entry.set_property("im-module", "fcitx")
                entry.set_valign(Gtk.Align.START)
                if COMPOSITED:
                    # 投影透出下面的东西：给编辑器一块够大的均匀底色，卡片下方的投影才比得出来。
                    window.set_default_size(640, 400)
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

                def settle():
                    # GTK 在打完字之后才补报光标矩形，候选窗随之横移；等窗口位置连续几次不变再取坐标。
                    previous, steady = None, 0
                    deadline = time.monotonic() + 5
                    while steady < 3 and time.monotonic() < deadline:
                        pump()
                        current = window_geometry(native_window())
                        steady = steady + 1 if current == previous else 0
                        previous = current
                        time.sleep(0.1)
                    return previous

                settle()
                save_image("initial")

                def selected_pixels():
                    # 每次按当前的选中行取像素，窗口随光标或宽度变化移动时比较的仍是同一行的内容。
                    image, _ = screenshot()
                    data, stride, channels = image.get_pixels(), image.get_rowstride(), image.get_n_channels()
                    l, t, r, b = card_bounds(SELECTED)
                    return b"".join(data[y * stride + l * channels:y * stride + r * channels] for y in range(t, b))

                def arrows():
                    # 首行右端的 ‹ ›：内边距 4pt，横排时最右边是展开键；按钮 28x26，首行从卡片顶边 4pt 处开始。
                    settle()
                    _, top, card_right, _ = card_rect()
                    arrow = 28 * SCALE
                    right = card_right - 4 * SCALE - (arrow if LAYOUT == "horizontal" else 0)
                    next_point = (right - arrow // 2, top + (4 + 13) * SCALE)
                    return (next_point[0] - arrow, next_point[1]), next_point

                first_page = selected_pixels()
                _, next_point = arrows()
                subprocess.run(["xdotool", "mousemove", *map(str, next_point), "click", "1"], check=True, timeout=5)
                wait(lambda: selected_pixels() != first_page)
                assert entry.get_text() == "", "翻页按钮错误地上屏"
                save_image("next-page")
                previous_point, _ = arrows()
                subprocess.run(["xdotool", "mousemove", *map(str, previous_point), "click", "1"], check=True, timeout=5)
                wait(lambda: selected_pixels() == first_page)
                selected = card_bounds(SELECTED)
                # 悬停：指针移到另一行时画悬停色，键盘选中不变。
                l, t, r, b = selected
                hover_point = (r + 20 * SCALE, (t + b) // 2) if LAYOUT == "horizontal" else ((l + r) // 2, b + 20 * SCALE)
                subprocess.run(["xdotool", "mousemove", *map(str, hover_point)], check=True, timeout=5)
                wait(lambda: card_visible(HOVER))
                assert card_bounds(SELECTED) == selected, "悬停改变了键盘选中项"
                save_image("hover")
                subprocess.run(["xdotool", "mousemove", "0", "0"], check=True, timeout=5)
                wait(lambda: not card_visible(HOVER))
                if COMPOSITED:
                    # 卡片正下方 3px 处压着投影，比窗口下面同一列的编辑器底色暗，但不是黑的；离卡片 11px 的左边距几乎就是编辑器底色（投影边距按 alpha 合成，不是一圈不透明的底）。
                    image, _ = screenshot()
                    channels, stride, data = image.get_n_channels(), image.get_rowstride(), image.get_pixels()
                    left, top, right, bottom = card_rect()
                    bounds = window_geometry(native_window())
                    column = (left + right) // 2
                    beneath = int(bounds["Y"]) + int(bounds["HEIGHT"]) + 2 * SCALE

                    def brightness(x, y):
                        return sum(data[y * stride + x * channels:y * stride + x * channels + 3])

                    background = brightness(column, beneath)
                    shaded = brightness(column, bottom + 3 * SCALE)
                    assert background - 250 < shaded < background - 30, f"卡片下方没有投影：{shaded} 对 {background}"
                    margin = brightness(left - 11 * SCALE, (top + bottom) // 2)
                    assert margin > background - 20, f"投影边距没有按 alpha 合成：{margin} 对 {background}"
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
                selected = card_bounds(SELECTED)
                subprocess.run(["xdotool", "mousemove", str((selected[0] + selected[2]) // 2),
                                str((selected[1] + selected[3]) // 2), "click", "1"], check=True, timeout=5)
                wait(lambda: entry.get_text() == "你")
                wait(lambda: native_window() is None)
                # 非 canvas 皮肤包的顶带装饰：画在卡片上方的透明带里，窗口比卡片高出这条带；装饰图有待机动效时照样能选词。
                band = (255, 0, 171)
                skins = root / "skins"
                skin = skins / "band-fixture"
                skin.mkdir(parents=True)
                (skin / "band.png").write_bytes(png((band,) * 9))
                (skin / "skin.toml").write_text(
                    "schema_version = 2\nid = 'band-fixture'\nname = 'Band fixture'\nversion = '1'\nbase = 'system'\n"
                    "[supports]\nlayouts = ['horizontal', 'vertical']\nthemes = ['light', 'dark']\n"
                    "[candidate_window.decoration]\nimage = 'band.png'\ntop_inset_dip = 24\nwidth_dip = 30\nmotion = 'float'\n")
                options["candidate_skin_catalog"] = {"directory": str(skins), "packages": [{
                    "id": "band-fixture", "title": "Band fixture", "base": "system", "layouts": ["horizontal", "vertical"],
                    "candidate": {"light": {}, "dark": {}}, "decoration_top_dip": 24, "decoration_width_dip": 30,
                    "decoration_image": str(skin / "band.png")}]}
                preferences["global_theme"] = "custom"
                preferences["custom_theme"].update(base="system", candidate_skin="band-fixture", candidate_skin_dark="band-fixture")
                publish()
                entry.set_text("")
                subprocess.run(["xdotool", "type", "--clearmodifiers", "--delay", "100", "ni"], check=True, timeout=5)
                wait(native_window)
                wait(lambda: palette_visible((band, SELECTED)))
                settle()
                save_image("band")
                window_bounds = window_geometry(native_window())
                band_box = color_bounds(band)
                # 装饰图在窗口顶部 24pt 的带里（浮动时上下不超过留出的 4pt 余量），底边压进卡片顶边一个内边距；选中行在卡片里、装饰图下面。带比投影的上边距高，上边不再多留。
                card_top = card_rect(24)[1]
                assert card_top == int(window_bounds["Y"]) + 24 * SCALE, "装饰带上方多留了投影边距"
                assert band_box[1] >= int(window_bounds["Y"]), "装饰图伸出了窗口"
                assert band_box[3] <= card_top + (4 + 3) * SCALE + 1, "装饰图没有画在卡片上方的带里"
                selected_box = card_bounds(SELECTED, 24)
                assert band_box[3] <= selected_box[1], "装饰图压到了候选行上"
                subprocess.run(["xdotool", "mousemove", str((selected_box[0] + selected_box[2]) // 2),
                                str((selected_box[1] + selected_box[3]) // 2), "click", "1"], check=True, timeout=5)
                wait(lambda: entry.get_text() == "你")
                wait(lambda: native_window() is None)
                control.Exit()
                assert daemon.wait(timeout=10) == 0, "Fcitx5 未正常退出"
                print(f"真实 GTK／X11 system 主题原生画布（{LAYOUT}{'，合成器' if COMPOSITED else ''}）的翻页、悬停、网格与点选通过")
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
                if compositor is not None:
                    compositor.terminate()
                    compositor.wait(timeout=10)
                if sys.exc_info()[0] is not None:
                    print((root / "daemon.log").read_text(), file=sys.stderr)


if __name__ == "__main__":
    main()

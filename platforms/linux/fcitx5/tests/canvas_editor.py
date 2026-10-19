"""在隔离 X11／Wayland 显示上检查真实 GTK 编辑器与原生 canvas 像素。"""
import ctypes
import configparser
from collections import Counter
from contextlib import nullcontext
import io
import json
import os
from pathlib import Path
import re
import struct
import subprocess
import sys
import tempfile
import zlib

import dbus
import dbus.mainloop.glib
import gi

gi.require_version("Gtk", "3.0")
gi.require_version("Gdk", "3.0")
gi.require_version("GdkPixbuf", "2.0")
from gi.repository import Gdk, GdkPixbuf, GLib, Gtk

WAYLAND = "--wayland" in sys.argv
if not WAYLAND:
    gi.require_version("GdkX11", "3.0")
    from gi.repository import GdkX11
from daemon import daemon_environment, wait

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
from host_api import ResponseDecoder


DARK = ((144, 17, 34), (18, 144, 52), (52, 37, 144),
        (20, 92, 114), (113, 90, 35), (139, 79, 49),
        (49, 127, 139), (116, 145, 38), (98, 66, 117))
LIGHT = tuple(tuple(channel + 80 for channel in color) for color in DARK)
FORMAT = os.environ.get("MSIME_TEST_CANVAS_FORMAT", "png")


def png(colors):
    """生成九块纯色的合成 PNG，不依赖外部资源或图片库。"""
    def chunk(kind, body):
        return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body))
    rows = b"".join(b"\0" + b"".join(bytes(colors[(y // 20) * 3 + x // 20])
                                      for x in range(60)) for y in range(60))
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 60, 60, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(rows)) + chunk(b"IEND", b""))


def panel_image(colors):
    if FORMAT == "png":
        return png(colors)
    try:
        from PIL import Image
    except ImportError:
        print("skipped: 非 PNG 的合成验收图片需要 Pillow")
        sys.exit(77)
    result = io.BytesIO()
    Image.open(io.BytesIO(png(colors))).save(
        result, format={"gif": "GIF", "webp": "WEBP"}[FORMAT],
        lossless=True)
    return result.getvalue()


def screenshot():
    if WAYLAND:
        loader = GdkPixbuf.PixbufLoader.new_with_type("png")
        loader.write(subprocess.check_output(["grim", "-"], timeout=5))
        loader.close()
        image = loader.get_pixbuf()
    else:
        root = Gdk.get_default_root_window()
        image = Gdk.pixbuf_get_from_window(root, 0, 0, root.get_width(), root.get_height())
    assert image is not None, "无法读取隔离显示的像素"
    channels, stride = image.get_n_channels(), image.get_rowstride()
    data = image.get_pixels()
    packed = b"".join(data[y * stride:y * stride + image.get_width() * channels]
                      for y in range(image.get_height()))
    return image, Counter(zip(packed[0::channels], packed[1::channels], packed[2::channels]))


def palette_visible(colors):
    _, counts = screenshot()
    return all(counts[color] >= 16 for color in colors)


def palette_top(color):
    image, _ = screenshot()
    channels, stride = image.get_n_channels(), image.get_rowstride()
    pixel = bytes(color) + (b"\xff" if channels == 4 else b"")
    data = image.get_pixels()
    for y in range(image.get_height()):
        if pixel in data[y * stride:y * stride + image.get_width() * channels]:
            return y
    return None


def save_image(name):
    destination = os.environ.get("MSIME_TEST_CANVAS_ARTIFACTS")
    if destination:
        directory = (Path(destination) / ("wayland" if WAYLAND else "x11") / FORMAT
                     / os.environ.get("MSIME_TEST_CANVAS_LAYOUT", "vertical"))
        directory.mkdir(parents=True, exist_ok=True)
        screenshot()[0].savev(str(directory / f"{name}.png"), "png", [], [])


def fallback_visible(theme):
    config = configparser.ConfigParser(interpolation=None)
    config.read(theme)
    if not config["InputPanel/Background"]["Image"].startswith("shape-"):
        return False
    color = config["InputPanel/Background"]["Color"].lstrip("#")
    rgb = tuple(int(color[at:at + 2], 16) for at in (0, 2, 4))
    return screenshot()[1][rgb] >= 100


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

    # Wayland 模块与编辑器同进程，GLib 读取前就要固定 XDG 目录；X11 面板在独立 daemon 中。
    profile = os.environ.get("MSIME_TEST_CANVAS_ROOT") if WAYLAND else None
    if WAYLAND:
        assert profile and os.environ.get("XDG_DATA_HOME") == str(Path(profile) / "data"), "需要启动前固定的合成配置目录"
    profile_context = nullcontext(profile) if profile else tempfile.TemporaryDirectory(prefix="msime-canvas-editor-")
    with profile_context as directory:
        root = Path(directory)
        if WAYLAND:
            assert GLib.get_user_data_dir() == str(root / "data"), "GLib 缓存了夹具以外的数据目录"
            assert GLib.get_user_config_dir() == str(root / "config"), "GLib 缓存了夹具以外的配置目录"
        request = json.dumps({"resources": str(resources), "state_root": str(root / "state")}).encode()
        options = value(host.msime_client_prepare_host(request, len(request)))
        skin = root / "skins" / "canvas-fixture"
        skin.mkdir(parents=True)
        panel_file, light_file = f"panel.{FORMAT}", f"light.{FORMAT}"
        (skin / panel_file).write_bytes(panel_image(DARK))
        (skin / light_file).write_bytes(panel_image(LIGHT))
        (skin / "skin.toml").write_text(
            "schema_version = 2\nid = 'canvas-fixture'\nname = 'Canvas fixture'\nversion = '1'\nbase = 'canvas'\n"
            "[supports]\nlayouts = ['horizontal', 'vertical']\nthemes = ['light', 'dark']\n"
            "[candidate_window]\ncontent_insets_dip = { top = 12, right = 12, bottom = 12, left = 12 }\n"
            "outer_insets_dip = { top = 4, right = 4, bottom = 4, left = 4 }\n"
            f"[candidate_window.border_image]\nsource = '{panel_file}'\nlight_source = '{light_file}'\n"
            "slice_dip = [20]\nwidth_dip = [8]\n[candidate.light]\ntext = '#010203'\n"
            "[candidate.dark]\ntext = '#FDFCFB'\n")
        options["candidate_skin_catalog"] = {
            "directory": str(skin.parent), "packages": [{
                "id": "canvas-fixture", "title": "Canvas fixture", "base": "system",
                "layouts": ["horizontal", "vertical"], "candidate": {"light": {}, "dark": {}}}]}
        preferences = options["preferences"]
        preferences.update(learning=False, cloud_candidates=False, usage_reporting=False,
                           global_theme="custom", candidate_theme="light",
                           candidate_layout=os.environ.get("MSIME_TEST_CANVAS_LAYOUT", "vertical"))
        preferences["custom_theme"].update(base="system", candidate_skin="canvas-fixture",
                                           candidate_skin_dark="canvas-fixture")

        def publish():
            store = options["preferences_directory"].encode()
            snapshot = value(host.msime_client_load_preferences(store, len(store)))
            snapshot["preferences"] = preferences
            document = json.dumps(snapshot).encode()
            value(host.msime_client_save_preferences(store, len(store), snapshot["revision"], document, len(document)))
            (root / "options.json").write_text(json.dumps(options))

        publish()
        env = daemon_environment(root, addon)
        # GTK 模块在创建上下文时才连接 daemon，环境必须在创建编辑器之前生效。
        os.environ.update(env)
        os.environ["GTK_IM_MODULE"] = "fcitx"
        os.environ["GDK_BACKEND"] = "wayland" if WAYLAND else "x11"
        if WAYLAND:
            assert Path(env.get("WAYLAND_DISPLAY", "")).is_absolute(), "需要隔离合成器的绝对 socket 路径"
        window = keyboard = None
        with (root / "daemon.log").open("w") as log:
            daemon = subprocess.Popen(["fcitx5", "-D", "--keep"], env=env, stdout=log, stderr=log)
            try:
                wait(lambda: bus.name_has_owner("org.fcitx.Fcitx5"))
                control = dbus.Interface(bus.get_object("org.fcitx.Fcitx5", "/controller"),
                                         "org.fcitx.Fcitx.Controller1")
                window = Gtk.Window(title="MSIME synthetic canvas editor")
                entry = Gtk.Entry()
                entry.set_property("im-module", "fcitx")
                # 平铺合成器会放大窗口；输入框保持自然高度，候选锚点不能被拉到屏幕底部。
                entry.set_valign(Gtk.Align.START)
                if WAYLAND:
                    entry.set_margin_top(80)
                window.add(entry)
                window.show_all()
                entry.grab_focus()
                wait(lambda: window.get_window() is not None)
                if WAYLAND:
                    assert "Wayland" in type(Gdk.Display.get_default()).__name__, "GTK 未使用 Wayland"
                    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tests/runtime"))
                    from wayland_keyboard import WaylandKeyboard

                    def pump():
                        while GLib.MainContext.default().iteration(False):
                            pass

                    keyboard = WaylandKeyboard(pump, lambda predicate, _description: wait(predicate))
                    wait(window.is_active)
                else:
                    xid = GdkX11.X11Window.get_xid(window.get_window())
                    subprocess.run(["xdotool", "windowfocus", "--sync", str(xid)], check=True, timeout=5)

                def focused():
                    if WAYLAND:
                        return window.is_active() and entry.has_focus()
                    return int(subprocess.check_output(["xdotool", "getwindowfocus"])) == xid
                wait(lambda: entry.has_focus())
                wait(lambda: re.search(r"frontend:dbus\b.*\bfocus:1\b", str(control.DebugInfo())))
                control.SetCurrentIM("msime")
                control.Activate()
                wait(lambda: str(control.CurrentInputMethod()) == "msime")
                if WAYLAND:
                    keyboard.keys(*"nihao")
                else:
                    subprocess.run(["xdotool", "type", "--clearmodifiers", "--delay", "100", "nihao"],
                                   check=True, timeout=5)
                wait(lambda: palette_visible(LIGHT))
                assert entry.get_text() == "", "预编辑文字提前进入编辑器"
                assert focused(), "候选窗口抢走焦点"
                save_image("canvas-light")
                if WAYLAND:
                    # 窗口留在同一输出；GTK 下一次输入上报新光标矩形后，真实 popup 必须跟随。
                    original_top = palette_top(LIGHT[0])
                    assert original_top is not None
                    entry.set_margin_top(200)
                    wait(lambda: entry.get_allocation().y >= 200)
                    keyboard.keys("BackSpace", "o")
                    wait(lambda: (palette_top(LIGHT[0]) or 0) >= original_top + 100)
                    assert focused(), "移动候选锚点后编辑器失焦"
                    save_image("canvas-moved")
                    entry.set_margin_top(80)
                    wait(lambda: entry.get_allocation().y < 200)
                    keyboard.keys("BackSpace", "o")
                    wait(lambda: palette_top(LIGHT[0]) == original_top)
                preferences["candidate_theme"] = "dark"
                publish()
                # 设置页由偏好轮询热更新；ReloadAddonConfig 表示重启服务，会主动清空组字。
                wait(lambda: palette_visible(DARK) and not palette_visible(LIGHT))
                save_image("canvas-dark")
                # 损坏图片必须让真实窗口回退；修复后不得一直沿用缓存的纯色卡片。
                (skin / panel_file).write_bytes(b"broken")
                wait(lambda: not palette_visible(DARK) and fallback_visible(root / "data/fcitx5/themes/msime/theme.conf"))
                save_image("canvas-broken")
                (skin / panel_file).write_bytes(panel_image(DARK))
                wait(lambda: palette_visible(DARK))
                save_image("canvas-repaired")
                assert focused(), "换肤后编辑器失焦"
                if WAYLAND:
                    keyboard.keys("space")
                else:
                    subprocess.run(["xdotool", "key", "space"], check=True, timeout=5)
                wait(lambda: entry.get_text() == "你好")
                wait(lambda: not palette_visible(DARK))
                control.Exit()
                assert daemon.wait(timeout=10) == 0, "Fcitx5 未正常退出"
                backend = "Wayland 客户端面板" if WAYLAND else "X11 classicui"
                print(f"真实 GTK／{backend} {FORMAT} canvas 像素、明暗、坏图回退、修复、焦点与选词通过")
            finally:
                if sys.exc_info()[0] is not None:
                    save_image("canvas-failed")
                    theme = root / "data/fcitx5/themes/msime/theme.conf"
                    if theme.exists():
                        print(theme.read_text(), file=sys.stderr)
                    classicui = root / "config/fcitx5/conf/classicui.conf"
                    if classicui.exists():
                        print(classicui.read_text(), file=sys.stderr)
                if WAYLAND and keyboard is not None:
                    keyboard.close()
                if window is not None:
                    window.destroy()
                if daemon.poll() is None:
                    daemon.terminate()
                    daemon.wait(timeout=10)
                if sys.exc_info()[0] is not None:
                    # 此日志仅含隔离的合成状态，不读取用户输入或用户配置。
                    print((root / "daemon.log").read_text(), file=sys.stderr)


if __name__ == "__main__":
    main()

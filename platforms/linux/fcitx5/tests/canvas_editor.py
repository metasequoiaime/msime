"""在隔离 X11 显示上检查真实 GTK 编辑器与 classicui canvas 像素。"""
import ctypes
import configparser
from collections import Counter
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
gi.require_version("GdkX11", "3.0")
from gi.repository import Gdk, GdkX11, Gtk
from daemon import daemon_environment, wait

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
from host_api import ResponseDecoder


DARK = ((144, 17, 34), (18, 144, 52), (52, 37, 144),
        (20, 92, 114), (113, 90, 35), (139, 79, 49),
        (49, 127, 139), (116, 145, 38), (98, 66, 117))
LIGHT = tuple(tuple(channel + 80 for channel in color) for color in DARK)


def png(colors):
    """生成九块纯色的合成 PNG，不依赖外部资源或图片库。"""
    def chunk(kind, body):
        return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body))
    rows = b"".join(b"\0" + b"".join(bytes(colors[(y // 20) * 3 + x // 20])
                                      for x in range(60)) for y in range(60))
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 60, 60, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(rows)) + chunk(b"IEND", b""))


def screenshot():
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


def save_image(name):
    destination = os.environ.get("MSIME_TEST_CANVAS_ARTIFACTS")
    if destination:
        directory = Path(destination) / os.environ.get("MSIME_TEST_CANVAS_LAYOUT", "vertical")
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

    with tempfile.TemporaryDirectory(prefix="msime-canvas-editor-") as directory:
        root = Path(directory)
        request = json.dumps({"resources": str(resources), "state_root": str(root / "state")}).encode()
        options = value(host.msime_client_prepare_host(request, len(request)))
        skin = root / "skins" / "canvas-fixture"
        skin.mkdir(parents=True)
        (skin / "panel.png").write_bytes(png(DARK))
        (skin / "light.png").write_bytes(png(LIGHT))
        (skin / "skin.toml").write_text(
            "schema_version = 2\nid = 'canvas-fixture'\nname = 'Canvas fixture'\nversion = '1'\nbase = 'canvas'\n"
            "[supports]\nlayouts = ['horizontal', 'vertical']\nthemes = ['light', 'dark']\n"
            "[candidate_window]\ncontent_insets_dip = { top = 12, right = 12, bottom = 12, left = 12 }\n"
            "outer_insets_dip = { top = 4, right = 4, bottom = 4, left = 4 }\n"
            "[candidate_window.border_image]\nsource = 'panel.png'\nlight_source = 'light.png'\n"
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
        os.environ["GDK_BACKEND"] = "x11"
        window = None
        with (root / "daemon.log").open("w") as log:
            daemon = subprocess.Popen(["fcitx5", "-D", "--keep"], env=env, stdout=log, stderr=log)
            try:
                wait(lambda: bus.name_has_owner("org.fcitx.Fcitx5"))
                control = dbus.Interface(bus.get_object("org.fcitx.Fcitx5", "/controller"),
                                         "org.fcitx.Fcitx.Controller1")
                window = Gtk.Window(title="MSIME synthetic canvas editor")
                entry = Gtk.Entry()
                entry.set_property("im-module", "fcitx")
                window.add(entry)
                window.show_all()
                entry.grab_focus()
                wait(lambda: window.get_window() is not None)
                xid = GdkX11.X11Window.get_xid(window.get_window())
                subprocess.run(["xdotool", "windowfocus", "--sync", str(xid)], check=True, timeout=5)
                wait(lambda: entry.has_focus())
                wait(lambda: re.search(r"frontend:dbus\b.*\bfocus:1\b", str(control.DebugInfo())))
                control.SetCurrentIM("msime")
                control.Activate()
                wait(lambda: str(control.CurrentInputMethod()) == "msime")
                subprocess.run(["xdotool", "type", "--clearmodifiers", "--delay", "100", "nihao"],
                               check=True, timeout=5)
                wait(lambda: palette_visible(LIGHT))
                assert entry.get_text() == "", "预编辑文字提前进入编辑器"
                assert int(subprocess.check_output(["xdotool", "getwindowfocus"])) == xid, "候选窗口抢走焦点"
                save_image("canvas-light")
                preferences["candidate_theme"] = "dark"
                publish()
                # 设置页由偏好轮询热更新；ReloadAddonConfig 表示重启服务，会主动清空组字。
                wait(lambda: palette_visible(DARK) and not palette_visible(LIGHT))
                save_image("canvas-dark")
                # 损坏图片必须让真实窗口回退；修复后不得一直沿用缓存的纯色卡片。
                (skin / "panel.png").write_bytes(b"broken")
                wait(lambda: not palette_visible(DARK) and fallback_visible(root / "data/fcitx5/themes/msime/theme.conf"))
                save_image("canvas-broken")
                (skin / "panel.png").write_bytes(png(DARK))
                wait(lambda: palette_visible(DARK))
                save_image("canvas-repaired")
                assert int(subprocess.check_output(["xdotool", "getwindowfocus"])) == xid, "换肤后编辑器失焦"
                subprocess.run(["xdotool", "key", "space"], check=True, timeout=5)
                wait(lambda: entry.get_text() == "你好")
                wait(lambda: not palette_visible(DARK))
                control.Exit()
                assert daemon.wait(timeout=10) == 0, "Fcitx5 未正常退出"
                print("真实 GTK／classicui canvas 像素、明暗、坏图回退、修复、焦点与选词通过")
            finally:
                if sys.exc_info()[0] is not None:
                    save_image("canvas-failed")
                    theme = root / "data/fcitx5/themes/msime/theme.conf"
                    if theme.exists():
                        print(theme.read_text(), file=sys.stderr)
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

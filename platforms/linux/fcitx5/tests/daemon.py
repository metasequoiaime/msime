"""Run under dbus-run-session; real daemon/frontend, synthetic input only."""
import ctypes
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
import time

import dbus
import dbus.mainloop.glib
from gi.repository import GLib

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
from host_api import ResponseDecoder


def wait(predicate):
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        while GLib.MainContext.default().iteration(False):
            pass
        if predicate():
            return
        time.sleep(0.02)
    raise AssertionError("Expected frontend state was not observed")


def daemon_environment(root, addon):
    """把真实 daemon 的目录、插件与状态限制在合成夹具内。"""
    config = root / "config" / "fcitx5"
    config.mkdir(parents=True, exist_ok=True)
    (config / "profile").write_text(
        "[Groups/0]\nName=Default\nDefault Layout=us\nDefaultIM=msime\n"
        "[Groups/0/Items/0]\nName=keyboard-us\nLayout=\n"
        "[Groups/0/Items/1]\nName=msime\nLayout=\n[GroupOrder]\n0=Default\n")
    runtime = root / "run"
    runtime.mkdir(mode=0o700)
    data = root / "data" / "fcitx5"
    (data / "addon").mkdir(parents=True)
    (data / "inputmethod").mkdir()
    source = Path(__file__).resolve().parents[1]
    (data / "addon" / "msime.conf").write_bytes((source / "msime.conf").read_bytes())
    (data / "inputmethod" / "msime.conf").write_bytes((source / "msime-inputmethod.conf").read_bytes())
    system_lib = subprocess.check_output(
        ["pkg-config", "--variable=libdir", "Fcitx5Core"], text=True).strip()
    return dict(os.environ, XDG_CONFIG_HOME=str(root / "config"),
               XDG_DATA_HOME=str(root / "data"), XDG_STATE_HOME=str(root / "xdg-state"),
               XDG_RUNTIME_DIR=str(runtime),
               MSIME_FCITX5_OPTIONS=str(root / "options.json"),
               FCITX_ADDON_DIRS=f"{addon.parent}:{system_lib}/fcitx5")


def check_ibus_frontend(bus, control):
    """经过真实 IBus 兼容前端，覆盖有无客户端预编辑、玲珑报告的 0x52 能力及上下文重建。"""
    service = "org.freedesktop.IBus"
    wait(lambda: bus.name_has_owner(service))
    frontend = dbus.Interface(bus.get_object(service, "/org/freedesktop/IBus"), service)
    # Fcitx5 的 DebugInfo 按十六进制打印能力；玲珑报告的 cap:52 是 0x52，包含预编辑、格式化预编辑和周边文本。
    for capabilities in (1 | 8 | 16 | 32, 1 | 8 | 16, 8 | 16 | 32, 0x52):
        path = frontend.CreateInputContext("msime-synthetic-ibus-editor")
        proxy = bus.get_object(service, path)
        context = dbus.Interface(proxy, "org.freedesktop.IBus.InputContext")
        commits, preedits = [], []
        signals = [
            context.connect_to_signal("CommitText", lambda text: commits.append(str(text[2]))),
            context.connect_to_signal("UpdatePreeditText",
                                      lambda text, cursor, visible: preedits.append(str(text[2]))),
        ]
        try:
            context.SetCapabilities(dbus.UInt32(capabilities))
            context.FocusIn()
            assert "frontend:ibus" in str(control.DebugInfo()), "没有经过 IBus 前端"
            control.SetCurrentIM("msime")
            control.Activate()
            for character in "nihao":
                assert context.ProcessKeyEvent(ord(character), 0, 0), "IBus 组字按键未接收"
            if capabilities & 1:
                wait(lambda: "nihao" in preedits)
            assert context.ProcessKeyEvent(32, 0, 0), "IBus 空格上屏未接收"
            wait(lambda: len(commits) == 1)
            assert commits == ["你好"], "IBus 前端上屏结果不符"
            context.FocusOut()
        finally:
            for signal in signals:
                signal.remove()
            dbus.Interface(proxy, "org.freedesktop.IBus.Service").Destroy()


def main():
    resources, library, addon = map(Path, sys.argv[1:4])
    dbus.mainloop.glib.DBusGMainLoop(set_as_default=True)
    bus = dbus.SessionBus()
    assert not bus.name_has_owner("org.fcitx.Fcitx5"), "Requires isolated D-Bus session"
    if "--page-number" in sys.argv[4:]:
        # The fixture has no desktop portal; prevent a theme probe from starting one.
        bus.request_name("org.freedesktop.portal.Desktop")
    with tempfile.TemporaryDirectory(prefix="msime-fcitx-daemon-") as directory:
        root = Path(directory)
        host = ctypes.CDLL(str(library))
        host.msime_client_prepare_host.argtypes = [ctypes.c_char_p, ctypes.c_size_t]
        host.msime_client_prepare_host.restype = ctypes.c_void_p
        responses = ResponseDecoder(host)
        request = json.dumps({"resources": str(resources), "state_root": str(root / "state")}).encode()
        raw = host.msime_client_prepare_host(request, len(request))
        result = responses.decode(raw)
        assert result["ok"], "Host preparation failed"
        options = result["value"]
        for name in ("msime_client_load_preferences", "msime_client_save_preferences"):
            getattr(host, name).restype = ctypes.c_void_p
        host.msime_client_load_preferences.argtypes = [ctypes.c_char_p, ctypes.c_size_t]
        host.msime_client_save_preferences.argtypes = [
            ctypes.c_char_p, ctypes.c_size_t, ctypes.c_uint64, ctypes.c_char_p, ctypes.c_size_t]

        def call(raw):
            assert raw, "Host API returned no result"
            try:
                reply = json.loads(ctypes.string_at(raw))
            finally:
                host.msime_client_string_free(raw)
            assert reply["ok"], "Host API request failed"
            return reply["value"]

        def publish():
            # 设置页的做法：先写偏好存储，再把同一份偏好抄进 runtime options。插件新建会话时以存储为准，只改文件的偏好不会生效。
            store = options["preferences_directory"].encode()
            snapshot = call(host.msime_client_load_preferences(store, len(store)))
            revision = snapshot["revision"]
            snapshot["preferences"] = options["preferences"]
            document = json.dumps(snapshot).encode()
            call(host.msime_client_save_preferences(store, len(store), revision, document, len(document)))
            (root / "options.json").write_text(json.dumps(options))

        options["preferences"].update(learning=False, cloud_candidates=False)
        shuangpin_helpcode = None
        if "--shuangpin-helpcode" in sys.argv[4:]:
            shuangpin_helpcode = sys.argv[sys.argv.index("--shuangpin-helpcode") + 1]
            assert shuangpin_helpcode in ("on", "off"), "需要明确辅助码开关状态"
            assert (resources / "helpcodes" / "shouyou2_0_helpcode.txt").is_file(), "验收资源缺少真实首右 2.0 码表"
            options["preferences"].update(
                scheme="shuangpin", shuangpin_profile="ziranma",
                candidate_translations=False, candidate_english_gloss=False,
                shuangpin_helpcode={"enabled": shuangpin_helpcode == "on", "schema": "shouyou2_0",
                                    "show_in_candidate_window": True, "modes": {"direct_helpcode": True}})
        # 悬浮工具栏在有 DISPLAY 时会画在屏幕右下角；这些用例只看候选和组字，不让它多出一块窗口。
        options["preferences"]["floating_toolbar"]["enabled"] = False
        if "--wayland-punctuation" in sys.argv[4:]:
            options["preferences"]["usage_reporting"] = False
        publish()
        env = daemon_environment(root, addon)
        runtime = root / "run"
        source = Path(__file__).resolve().parents[1]
        if "--page-number" in sys.argv[4:]:
            env["FCITX_X11_USE_CLIENT_SIDE_UI"] = "1"
        if "--wayland-punctuation" in sys.argv[4:]:
            assert env.get("MSIME_ISOLATED_LINUX_TEST") == "1"
            assert Path(env.get("WAYLAND_DISPLAY", "")).is_absolute(), "需要独立合成器的绝对 socket 路径"
            env.pop("DISPLAY", None)
        with (root / "daemon.log").open("w") as log:
            command = ["fcitx5", "-D", "--keep"]
            if "--ibus-frontend" in sys.argv[4:]:
                command.append("--enable=ibusfrontend")
            daemon = subprocess.Popen(command, env=env,
                                      stdout=log, stderr=log)
            try:
                wait(lambda: bus.name_has_owner("org.fcitx.Fcitx5"))
                service = "org.fcitx.Fcitx5"
                control = dbus.Interface(bus.get_object(service, "/controller"),
                                         "org.fcitx.Fcitx.Controller1")
                if shuangpin_helpcode is not None:
                    subprocess.run([sys.executable, str(source / "tests" / "gtk_editor.py"),
                                    "--shuangpin-helpcode"],
                                   env=dict(env, GTK_IM_MODULE="fcitx", GDK_BACKEND="x11"),
                                   check=True, timeout=30)
                    control.Exit()
                    assert daemon.wait(timeout=10) == 0, "双拼验收 daemon 未正常退出"
                    print(f"Fcitx5 自然码/首右 2.0 编辑器验收通过：辅助码 {shuangpin_helpcode}")
                    return
                if "--ibus-frontend" in sys.argv[4:]:
                    check_ibus_frontend(bus, control)
                    # GTK 从地址文件连接 IBus；该文件由 Fcitx5 的延迟初始化发布。
                    wait(lambda: list((root / "config" / "ibus" / "bus").glob("*")))
                    subprocess.run([sys.executable, str(source / "tests" / "gtk_editor.py")],
                                   env=dict(env, GTK_IM_MODULE="ibus", GDK_BACKEND="x11"),
                                   check=True, timeout=30)
                    control.Exit()
                    assert daemon.wait(timeout=10) == 0, "IBus 前端测试 daemon 未正常退出"
                    print("Fcitx5 IBus 前端与 GTK 编辑器组字上屏验收通过")
                    return
                if "--wayland-punctuation" in sys.argv[4:]:
                    # 复用隔离 daemon 和合成配置，但按键走真实 Wayland/Chrome，不走 D-Bus 模拟前端。
                    subprocess.run(["node", str(source / "tests" / "wayland_punctuation.mjs")],
                                   cwd=source.parents[2], env=env, check=True, timeout=60)
                    control.Exit()
                    assert daemon.wait(timeout=10) == 0, "测试 daemon 未正常退出"
                    return
                frontend = dbus.Interface(bus.get_object(service, "/org/freedesktop/portal/inputmethod"),
                                          "org.fcitx.Fcitx.InputMethod1")
                path, _ = frontend.CreateInputContext([("program", "msime-synthetic-editor")])
                context = dbus.Interface(bus.get_object(service, path), "org.fcitx.Fcitx.InputContext1")
                commits = []
                preedits = []
                context.connect_to_signal("CommitString", lambda text: commits.append(str(text)))
                context.connect_to_signal("UpdateFormattedPreedit",
                                          lambda parts, cursor: preedits.append("".join(str(p[0]) for p in parts)))
                context.SetCapability(dbus.UInt64(2 | 16 | 64))
                context.FocusIn()
                control.SetCurrentIM("msime")
                control.Activate()
                wait(lambda: str(control.CurrentInputMethod()) == "msime")
                for character in "nihao":
                    assert context.ProcessKeyEvent(ord(character), 0, 0, False, 0), "Composition key rejected"
                wait(lambda: "nihao" in preedits)
                assert context.ProcessKeyEvent(32, 0, 0, False, 0), "Commit key rejected"
                wait(lambda: len(commits) == 1)
                assert commits == ["你好"], "Unexpected committed result"
                if "--page-number" in sys.argv[4:]:
                    panels = []
                    context.connect_to_signal("UpdateClientSideUI", lambda *args: panels.append(args))
                    context.FocusOut()
                    context.SetCapability(dbus.UInt64(2 | 16 | 64 | (1 << 39)))
                    context.FocusIn()

                    def compose(show_page, preedit="empty"):
                        options["preferences"]["candidate_preedit_style"] = preedit
                        options["preferences"]["navigation"]["brackets"] = True
                        options["preferences"]["word_character"]["enabled"] = False
                        if show_page is None:
                            options["preferences"].pop("show_candidate_page_number", None)
                        else:
                            options["preferences"]["show_candidate_page_number"] = show_page
                        publish()
                        control.ReloadAddonConfig("msime")
                        panels.clear()
                        for character in "nihao":
                            assert context.ProcessKeyEvent(ord(character), 0, 0, False, 0)
                        wait(lambda: panels and panels[-1][4])
                        return panels[-1]

                    def aux(panel):
                        return "".join(str(part[0]) for parts in (panel[2], panel[3]) for part in parts)

                    # Missing keys retain the old page indicator, independently of reading text.
                    assert aux(compose(None)).startswith("1 / "), "Legacy page indicator missing"
                    first = compose(False)
                    assert aux(first) == "", "Hidden page/preedit left an auxiliary line"
                    assert context.ProcessKeyEvent(ord("]"), 0, 0, False, 0)
                    wait(lambda: panels[-1][4] != first[4])
                    assert aux(panels[-1]) == "", "Paging restored the hidden indicator"
                    assert context.ProcessKeyEvent(ord("["), 0, 0, False, 0)
                    wait(lambda: panels[-1][4] == first[4])
                    assert context.ProcessKeyEvent(32, 0, 0, False, 0)
                    wait(lambda: len(commits) == 2)
                    assert commits[-1] == "你好", "Hidden indicator broke candidate selection"
                    assert aux(compose(True)).startswith("1 / "), "Re-enabled page indicator missing"
                    reading = aux(compose(False, "pinyin"))
                    assert "ni" in reading and "/" not in reading and not reading.startswith(" · "), \
                        "Page visibility changed the independent reading or left a separator"
                    context.FocusOut()
                    context.DestroyIC()
                    control.Exit()
                    assert daemon.wait(timeout=10) == 0, "Daemon failed on shutdown"
                    print("Fcitx5 page-number visibility, paging and selection passed")
                    return
                # Screen keyboard keys come over panel-input.sock and go through MSIME before the editor, the way SendInput passes through the IME on Windows. Keycodes are evdev codes, as the panel sends them.
                forwarded = []
                context.connect_to_signal("ForwardKey", lambda sym, states, release: forwarded.append((int(sym), bool(release))))
                panel_socket = runtime / "msime-client" / "panel-input.sock"
                wait(panel_socket.exists)

                def panel(request):
                    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
                        client.settimeout(5)
                        client.connect(str(panel_socket))
                        client.sendall((json.dumps(request) + "\n").encode())
                        return json.loads(client.makefile().readline())

                def panel_key(name, keycode):
                    reply = panel({"op": "key", "key": name, "keycode": keycode})
                    assert reply == {"ok": True}, f"Screen keyboard {name} refused"

                for name, keycode in (("n", 49), ("i", 23), ("h", 35), ("a", 30), ("o", 24)):
                    panel_key(name, keycode)
                wait(lambda: preedits and preedits[-1] == "nihao")
                panel_key("space", 57)
                wait(lambda: len(commits) == 2)
                assert commits[1] == "你好" and not forwarded, "Screen keyboard typed raw letters instead of composing"
                # A composition started on the physical keyboard: the screen keyboard's digits select from it and its BackSpace edits it.
                for character in "nihao":
                    assert context.ProcessKeyEvent(ord(character), 0, 0, False, 0), "Composition key rejected"
                wait(lambda: preedits[-1] == "nihao")
                panel_key("2", 3)
                wait(lambda: len(commits) == 3)
                assert commits[2] != "你好" and all(ord(c) > 0x7f for c in commits[2]) and not forwarded, \
                    "Screen keyboard digit did not select the second candidate"
                # A candidate shorter than the reading leaves the rest composing.
                context.ProcessKeyEvent(0xff1b, 0, 0, False, 0)
                for character in "nihao":
                    assert context.ProcessKeyEvent(ord(character), 0, 0, False, 0), "Composition key rejected"
                wait(lambda: preedits[-1] == "nihao")
                panel_key("BackSpace", 14)
                wait(lambda: preedits[-1] == "niha")
                assert not forwarded, "Screen keyboard BackSpace left the composition"
                assert context.ProcessKeyEvent(0xff1b, 0, 0, False, 0), "Cancel key rejected"
                # With nothing to compose, the whole stroke goes on to the editor.
                panel_key("BackSpace", 14)
                wait(lambda: len(forwarded) == 2)
                assert forwarded == [(0xff08, False), (0xff08, True)], "Idle BackSpace did not reach the editor as one stroke"
                # Handwriting, emoji and voice text is committed as it is.
                assert panel({"op": "text", "text": "好"}) == {"ok": True}, "Panel text refused"
                wait(lambda: len(commits) == 4)
                assert commits[3] == "好" and len(forwarded) == 2, "Panel text did not commit as it is"
                # Under CapsLock a physical letter arrives uppercase and goes to the editor; the screen keyboard's lowercase letter has to do the same rather than start a composition. X LockMask is 2.
                assert not context.ProcessKeyEvent(ord("A"), 38, 2, False, 0), "CapsLock letter was composed"
                context.ProcessKeyEvent(ord("A"), 38, 2, True, 0)
                shown = len(preedits)
                panel_key("a", 30)
                wait(lambda: len(forwarded) == 4)
                assert forwarded[2:] == [(ord("A"), False), (ord("A"), True)] and not any(preedits[shown:]), \
                    "Screen keyboard letter under CapsLock did not reach the editor uppercase"
                # A key without the lock reports it off again.
                context.ProcessKeyEvent(0xff1b, 9, 0, False, 0)
                context.ProcessKeyEvent(0xff1b, 9, 0, True, 0)
                # In English mode the engine passes the letter on untouched, as one whole stroke. Ctrl+Alt+Space, since Fcitx5 itself claims Ctrl+Space and Shift_L. Ctrl is 4 and Alt 8.
                assert context.ProcessKeyEvent(0x20, 65, 4 | 8, False, 0), "Ctrl+Alt+Space rejected"
                context.ProcessKeyEvent(0x20, 65, 4 | 8, True, 0)
                shown = len(preedits)
                panel_key("n", 49)
                wait(lambda: len(forwarded) == 6)
                assert forwarded[4:] == [(ord("n"), False), (ord("n"), True)] and not any(preedits[shown:]), \
                    "Screen keyboard letter in English mode did not reach the editor as one stroke"
                assert context.ProcessKeyEvent(0x20, 65, 4 | 8, False, 0), "Ctrl+Alt+Space could not restore Chinese"
                context.ProcessKeyEvent(0x20, 65, 4 | 8, True, 0)
                # 从共享设置热切换智能 ABC，经真实 Fcitx5 前端组词上屏，再恢复全拼。
                options["preferences"].update(scheme="shuangpin", shuangpin_profile="smart_abc")
                publish()
                control.ReloadAddonConfig("msime")
                for code, expected_text in (("nihk", "你好"), ("asgo", "中国"), ("vurufa", "输入法")):
                    expected = commits + [expected_text]
                    for character in code:
                        assert context.ProcessKeyEvent(ord(character), 0, 0, False, 0), "智能 ABC 按键未被处理"
                    assert context.ProcessKeyEvent(32, 0, 0, False, 0)
                    wait(lambda: commits == expected)
                options["preferences"].update(scheme="quanpin", shuangpin_profile="xiaohe")
                publish()
                control.ReloadAddonConfig("msime")
                print("Fcitx5 智能 ABC 偏好热切换与组词上屏通过")
                # 周边文本的光标为 0 时没有前一字符，不能读取负索引或回退到旧的输入影子。
                for document, cursor in (("", 0), ("abc", 0), ("合成", 0), ("合成", 2)):
                    for key, modifiers, punctuation in ((ord("."), 0, "。"), (ord("?"), 1, "？")):
                        context.SetSurroundingText(document, dbus.UInt32(cursor), dbus.UInt32(cursor))
                        expected = commits + [punctuation]
                        assert context.ProcessKeyEvent(key, 0, modifiers, False, 0), \
                            f"中文标点按键未被处理：text={document!r}, cursor={cursor}, key={chr(key)}"
                        context.ProcessKeyEvent(key, 0, modifiers, True, 0)
                        wait(lambda: commits == expected)
                print("Fcitx5 文本开头与中文前文的句号、问号上屏通过")
                context.FocusOut()
                context.DestroyIC()
                if os.environ.get("MSIME_TEST_GTK") == "1":
                    subprocess.run([sys.executable, str(source / "tests" / "gtk_editor.py")],
                                   env=dict(env, GTK_IM_MODULE="fcitx", GDK_BACKEND="x11"),
                                   check=True, timeout=30)
                control.Exit()
                assert daemon.wait(timeout=10) == 0, "Daemon failed on shutdown"
            finally:
                if daemon.poll() is None:
                    daemon.terminate()
                    try:
                        daemon.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        daemon.kill()
                        daemon.wait()
    print("Fcitx5 daemon/frontend composition and commit passed")


if __name__ == "__main__":
    main()

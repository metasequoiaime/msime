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


def wait(predicate):
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        while GLib.MainContext.default().iteration(False):
            pass
        if predicate():
            return
        time.sleep(0.02)
    raise AssertionError("Expected frontend state was not observed")


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
        host.msime_client_string_free.argtypes = [ctypes.c_void_p]
        request = json.dumps({"resources": str(resources), "state_root": str(root / "state")}).encode()
        raw = host.msime_client_prepare_host(request, len(request))
        assert raw, "Host preparation returned no result"
        try:
            result = json.loads(ctypes.string_at(raw))
        finally:
            host.msime_client_string_free(raw)
        assert result["ok"], "Host preparation failed"
        options = result["value"]
        options["preferences"].update(learning=False, cloud_candidates=False)
        if "--wayland-punctuation" in sys.argv[4:]:
            options["preferences"]["usage_reporting"] = False
        (root / "options.json").write_text(json.dumps(options))
        config = root / "config" / "fcitx5"
        config.mkdir(parents=True)
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
        env = dict(os.environ, XDG_CONFIG_HOME=str(root / "config"),
                   XDG_DATA_HOME=str(root / "data"), XDG_RUNTIME_DIR=str(runtime),
                   MSIME_FCITX5_OPTIONS=str(root / "options.json"),
                   FCITX_ADDON_DIRS=f"{addon.parent}:{system_lib}/fcitx5")
        if "--page-number" in sys.argv[4:]:
            env["FCITX_X11_USE_CLIENT_SIDE_UI"] = "1"
        if "--wayland-punctuation" in sys.argv[4:]:
            assert env.get("MSIME_ISOLATED_LINUX_TEST") == "1"
            assert Path(env.get("WAYLAND_DISPLAY", "")).is_absolute(), "需要独立合成器的绝对 socket 路径"
            env.pop("DISPLAY", None)
        with (root / "daemon.log").open("w") as log:
            daemon = subprocess.Popen(["fcitx5", "-D", "--keep"], env=env,
                                      stdout=log, stderr=log)
            try:
                wait(lambda: bus.name_has_owner("org.fcitx.Fcitx5"))
                service = "org.fcitx.Fcitx5"
                control = dbus.Interface(bus.get_object(service, "/controller"),
                                         "org.fcitx.Fcitx.Controller1")
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
                        (root / "options.json").write_text(json.dumps(options))
                        control.ReloadAddonConfig("msime")
                        panels.clear()
                        for character in "nihao":
                            assert context.ProcessKeyEvent(ord(character), 0, 0, False, 0)
                        wait(lambda: panels and panels[-1][4])
                        return panels[-1]

                    def aux(panel):
                        return "".join(str(part[0]) for parts in (panel[2], panel[3]) for part in parts)

                    # Missing keys retain the old page indicator, independently of reading text.
                    assert aux(compose(None)).startswith("1/"), "Legacy page indicator missing"
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
                    assert aux(compose(True)).startswith("1/"), "Re-enabled page indicator missing"
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

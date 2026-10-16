"""Real IBus daemon/client acceptance inside an isolated D-Bus session only."""
import os
import signal
import subprocess
import sys
import time

import gi

gi.require_version("IBus", "1.0")
from gi.repository import GLib, IBus

IBus.init()
bus = IBus.Bus.new()
if not bus.is_connected():
    sys.exit("Test IBus daemon is unavailable")


def wait(predicate, timeout=10):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        while GLib.MainContext.default().iteration(False):
            pass
        if predicate():
            return
        time.sleep(0.02)
    raise AssertionError("Expected IBus state was not observed")


def running(pid):
    try:
        with open(f"/proc/{pid}/stat") as stat:
            fields = stat.read().rsplit(")", 1)[1].split()
    except OSError:
        return None
    return fields if fields[0] != "Z" else None


def hosts(parent=None):
    """Live msime-linux-ibus processes, optionally only the children of one supervisor."""
    found = []
    for entry in os.listdir("/proc"):
        if not entry.isdigit():
            continue
        fields = running(int(entry))
        if fields is None or (parent is not None and int(fields[1]) != parent):
            continue
        try:
            with open(f"/proc/{entry}/cmdline", "rb") as cmdline:
                arguments = cmdline.read().decode(errors="replace").split("\0")
        except OSError:
            continue
        if os.path.basename(arguments[0]) == "msime-linux-ibus":
            found.append((int(entry), arguments[1:]))
    return found


def host_of(supervisor):
    children = hosts(supervisor)
    return children[0] if len(children) == 1 else (None, None)


wait(lambda: any(engine.get_name() == "msime-linux" for engine in bus.list_active_engines()))
context = bus.create_input_context("msime-synthetic-editor")
commits = []
context.connect("commit-text", lambda _context, text: commits.append(text.get_text()))
context.set_capabilities(IBus.Capabilite.FOCUS | IBus.Capabilite.PREEDIT_TEXT | IBus.Capabilite.LOOKUP_TABLE)
context.focus_in()
assert bus.set_global_engine("msime-linux"), "Global engine activation failed"
wait(lambda: context.get_engine() is not None and context.get_engine().get_name() == "msime-linux")
context.property_activate("ChinesePunctuation", IBus.PropState.UNCHECKED)
context.property_activate("EnglishCandidates", IBus.PropState.CHECKED)
context.property_activate("EmojiCandidates", IBus.PropState.CHECKED)
context.property_activate("KaomojiCandidates", IBus.PropState.CHECKED)
# The fixture carries the shipped default_ime_mode, which is Chinese as on Windows (in-container.sh checks it), so the first letter composes without touching the mode.
assert context.process_key_event(ord("n"), 0, 0), "Shipped Chinese default passed input through"
for character in "ihao":
    assert context.process_key_event(ord(character), 0, 0)
assert context.process_key_event(IBus.KEY_space, 0, 0)
wait(lambda: commits == ["你好"])
# This fixture uses global CN/EN mode. A different input source must clear that authority, so returning starts from the configured Chinese default rather than the English the user left it in.
context.property_activate("InputMode", IBus.PropState.UNCHECKED)
assert not context.process_key_event(ord("n"), 0, 0), "InputMode property did not leave Chinese"
assert bus.set_global_engine("xkb:us::eng"), "US input source activation failed"
wait(lambda: context.get_engine() is not None and context.get_engine().get_name() == "xkb:us::eng")
assert bus.set_global_engine("msime-linux"), "Input source reactivation failed"
wait(lambda: context.get_engine() is not None and context.get_engine().get_name() == "msime-linux")
for character in "nihao":
    assert context.process_key_event(ord(character), 0, 0), "Source switch retained global English mode"
assert context.process_key_event(IBus.KEY_space, 0, 0)
wait(lambda: commits == ["你好", "你好"])

# 从真实 IBus 属性菜单选择内置方案，验证特色声母和韵母，而非仅检查菜单标签。
context.property_activate("Scheme/Shuangpin", IBus.PropState.CHECKED)
context.property_activate("ShuangpinProfile/smart_abc", IBus.PropState.CHECKED)
for code, expected_text in (("nihk", "你好"), ("asgo", "中国"), ("vurufa", "输入法")):
    expected = commits + [expected_text]
    for character in code:
        assert context.process_key_event(ord(character), 0, 0), "智能 ABC 按键未被处理"
    assert context.process_key_event(IBus.KEY_space, 0, 0)
    wait(lambda: commits == expected)
context.property_activate("Scheme/Quanpin", IBus.PropState.CHECKED)
print("IBus 智能 ABC 菜单切换与组词上屏通过")

supervisor = int(os.environ.get("MSIME_SMOKE_SUPERVISOR_PID", "0"))
if supervisor:
    # ibus-daemon never respawns a dead component. The launcher's supervisor does, and the restarted host puts MSIME back on the focused context, so typing resumes without the user reselecting the input source. The second crash lands inside the backoff window and waits 4 s instead of 2 s.
    for crash in (signal.SIGSEGV, signal.SIGKILL):
        crashed, _ = host_of(supervisor)
        assert crashed, "supervised host is not running"
        os.kill(crashed, crash)
        wait(lambda: host_of(supervisor)[0] not in (None, crashed), timeout=15)
        _, arguments = host_of(supervisor)
        assert arguments[0] == "--recovered", arguments

        def typing_resumed():
            if context.get_engine() is None or context.get_engine().get_name() != "msime-linux":
                return False
            context.property_activate("InputMode", IBus.PropState.CHECKED)
            return context.process_key_event(ord("n"), 0, 0)

        expected = commits + ["你好"]
        wait(typing_resumed, timeout=15)
        for character in "ihao":
            assert context.process_key_event(ord(character), 0, 0)
        assert context.process_key_event(IBus.KEY_space, 0, 0)
        wait(lambda: commits == expected)
        print(f"Host restarted after {crash.name} and typing resumed without reselecting")

    # Ctrl+Shift+Alt+T is a deliberate stop: the supervisor lets the host go and ends too, and nothing comes back after the first restart delay would have elapsed.
    stopped, _ = host_of(supervisor)
    context.process_key_event(IBus.KEY_t, 0, IBus.ModifierType.CONTROL_MASK | IBus.ModifierType.SHIFT_MASK | IBus.ModifierType.MOD1_MASK)
    wait(lambda: running(stopped) is None and running(supervisor) is None)
    time.sleep(3)
    assert hosts() == [], hosts()
    print("Maintenance stop ended the host without a restart")

context.focus_out()
context.destroy()

if supervisor:
    # ibus exit disconnects the host, which returns 0; the supervisor does not restart it and exits too. Nothing starts again, since no daemon is left.
    environment = dict(os.environ, MSIME_IBUS_OPTIONS=os.environ["MSIME_SMOKE_OPTIONS"])
    launcher = subprocess.Popen([os.environ["MSIME_SMOKE_LAUNCHER"]], env=environment)
    try:
        wait(lambda: host_of(launcher.pid)[0] is not None
             and any(engine.get_name() == "msime-linux" for engine in bus.list_active_engines()))
        bus.exit(False)  # What `ibus exit` sends.
        assert launcher.wait(timeout=10) == 0, launcher.returncode
    finally:
        if launcher.poll() is None:
            launcher.terminate()
            launcher.wait(timeout=10)
    assert hosts() == [], hosts()
    print("ibus exit ended the supervisor")

print("IBus daemon factory and input-context acceptance passed")

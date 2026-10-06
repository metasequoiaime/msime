#!/usr/bin/env python3
"""msime-linux-setup --unregister puts back the desktop candidate panel settings the hosts took over, the Linux side of the Windows uninstaller removing everything MSIME left: Fcitx5 classicui's Theme, DarkTheme, Font and WheelForPaging, and the IBus panel's custom-font and use-custom-font. A setting is restored only while it still holds what MSIME wrote; one the user changed since is kept. The theme MSIME generated for Fcitx5 is removed.

A stub stands in for gdbus and gsettings: it logs each call and keeps Fcitx5's bus state and the IBus panel keys in a JSON file. Each case runs against a scratch HOME holding a classicui.conf and a restore record, with the record written in the shape the hosts write it (src/candidates/PanelRestoreRecord.h).
"""
import fcntl
import importlib.util
from importlib.machinery import SourceFileLoader
import json
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/msime-linux-setup"
PANEL = "org.freedesktop.ibus.panel"
SET_CONFIG = "org.fcitx.Fcitx.Controller1.SetConfig"

_setup_loader = SourceFileLoader("msime_linux_setup", str(SCRIPT))
_setup_spec = importlib.util.spec_from_loader("msime_linux_setup", _setup_loader)
assert _setup_spec and _setup_spec.loader
_setup_module = importlib.util.module_from_spec(_setup_spec)
_setup_spec.loader.exec_module(_setup_module)

# 首行用跑测试的同一个解释器，不经 /usr/bin/env：没有 FHS 布局的环境（Nix 构建沙箱）里没有它。
STUB = f"#!{sys.executable}\n" + r'''import ast, json, os, re, sys
from pathlib import Path

name = Path(sys.argv[0]).name
arguments = sys.argv[1:]
state_file = Path(os.environ["STUB_STATE"])
state = json.loads(state_file.read_text())
with open(os.environ["STUB_LOG"], "a") as log:
    log.write(json.dumps([name, *arguments], ensure_ascii=False) + "\n")


def literal(text):
    # GVariant text for the shapes written here: strings, booleans, and variants of an a{sv} map of strings.
    return ast.literal_eval(re.sub(r"[<>]", "", text).replace("true", "True").replace("false", "False"))


def save():
    state_file.write_text(json.dumps(state, ensure_ascii=False))


if name == "gdbus":
    method = arguments[arguments.index("--method") + 1].rsplit(".", 1)[1]
    values = [literal(argument) for argument in arguments[arguments.index("--method") + 2:]]
    if method == "NameHasOwner":
        print("(true,)" if state["fcitx5_running"] else "(false,)")
    elif method == "SetConfig" and state.get("set_config_fails"):
        print("Error: GDBus.Error:org.freedesktop.DBus.Error.UnknownMethod: No such method", file=sys.stderr)
        sys.exit(1)
    elif method == "SetConfig":
        uri, options = values
        state["set_config"].append([uri, options])
        save()
        # A host still running records a change while Fcitx5 applies the restored options.
        if "record_during_set_config" in state:
            Path(os.environ["HOME"], ".local/state/msime-client/panel-restore.json").write_text(state["record_during_set_config"])
        print("()")
    else:
        sys.exit(2)
    sys.exit(0)
if name == "gsettings":
    panel = state["panel"]
    if arguments[0] == "list-schemas":
        print("\n".join(state["schemas"]))
    elif arguments[0] == "get":
        value = panel["user"].get(arguments[2], panel["defaults"][arguments[2]])
        print(("true" if value else "false") if isinstance(value, bool) else repr(value))
    elif arguments[0] == "set":
        panel["user"][arguments[2]] = literal(arguments[3])
        save()
    elif arguments[0] == "reset":
        panel["user"].pop(arguments[2], None)
        save()
    sys.exit(0)
sys.exit(127)
'''

CLASSICUI = """# Fcitx5 writes every option
Vertical Candidate List=False
WheelForPaging=False
Font="Noto Sans SC, Microsoft YaHei 18px"
MenuFont="Sans 10"
Theme=msime
DarkTheme=msime
UseDarkTheme=False

[Extra]
Theme=kept
"""

RECORD = {
    "fcitx5": {
        "Theme": {"prior": "default", "written": "msime"},
        "DarkTheme": {"prior": "default-dark", "written": "msime"},
        "Font": {"prior": "Sans 10", "written": "Noto Sans SC, Microsoft YaHei 18px"},
        # Turned off again in fcitx5-configtool after MSIME turned it on.
        "WheelForPaging": {"prior": "False", "written": "True"},
    },
    "ibus": {
        "custom-font": {"prior": "Serif 11", "written": "Noto Sans SC 18px"},
        "use-custom-font": {"prior": None, "written": True},
    },
}


class Harness:
    def __init__(self, scratch: Path):
        self.scratch = scratch
        self.home = scratch / "home"
        self.setup = scratch / "prefix/bin/msime-linux-setup"
        self.setup.parent.mkdir(parents=True)
        self.setup.write_text(SCRIPT.read_text())
        self.setup.chmod(0o755)
        tools = scratch / "tools"
        tools.mkdir()
        (tools / "stub").write_text(STUB)
        (tools / "stub").chmod(0o755)
        for tool in ("gdbus", "gsettings"):
            (tools / tool).symlink_to(tools / "stub")
        self.state_file = scratch / "stub-state.json"
        self.log = scratch / "calls.log"
        self.environment = {key: value for key, value in os.environ.items()
                            if not key.startswith(("MSIME_", "XDG_", "DBUS_"))}
        self.environment.update(
            PATH=f"{tools}:{os.environ.get('PATH', '/usr/bin:/bin')}", HOME=str(self.home),
            STUB_STATE=str(self.state_file), STUB_LOG=str(self.log),
        )
        self.classicui = self.home / ".config/fcitx5/conf/classicui.conf"
        self.record = self.home / ".local/state/msime-client/panel-restore.json"
        self.theme = self.home / ".local/share/fcitx5/themes/msime"

    def world(self, record=RECORD, classicui=CLASSICUI, fcitx5_running=False, panel_user=None, **extra) -> None:
        for path in (self.classicui, self.record, self.theme / "theme.conf"):
            path.parent.mkdir(parents=True, exist_ok=True)
        self.classicui.write_text(classicui)
        if record is None:
            self.record.unlink(missing_ok=True)
        else:
            self.record.write_text(json.dumps(record))
        (self.theme / "theme.conf").write_text("[Metadata]\nName=MSIME\n")
        (self.theme / "decoration-0123.png").write_bytes(b"\x89PNG")
        self.state_file.write_text(json.dumps({
            "fcitx5_running": fcitx5_running,
            "set_config": [],
            "schemas": [PANEL],
            "panel": {
                "defaults": {"custom-font": "Sans 10", "use-custom-font": False},
                "user": {"custom-font": "Noto Sans SC 18px", "use-custom-font": True} if panel_user is None else panel_user,
            },
            **extra,
        }))
        self.log.write_text("")

    def unregister(self) -> subprocess.CompletedProcess:
        result = subprocess.run([str(self.setup), "--unregister"], env=self.environment,
                                capture_output=True, text=True, timeout=30)
        assert result.returncode == 0, result
        assert "Traceback" not in result.stderr, result.stderr
        return result

    def state(self) -> dict:
        return json.loads(self.state_file.read_text())

    def calls(self, name: str) -> list:
        return [call[1:] for call in map(json.loads, self.log.read_text().splitlines()) if call[0] == name]


def restored_file(**values: str) -> str:
    text = CLASSICUI
    for key, value in values.items():
        text = text.replace(next(line for line in CLASSICUI.splitlines() if line.startswith(f"{key}=")), f"{key}={value}", 1)
    return text


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="msime-panel-lock-") as temporary:
        root = Path(temporary)
        target = root / "outside.lock"
        target.write_text("synthetic-lock-target")
        linked = root / "panel-restore.lock"
        linked.symlink_to(target)
        try:
            with _setup_module.panel_restore_lock(root / "panel-restore"):
                raise AssertionError("panel restore lock followed a symlink")
        except OSError:
            pass
        assert target.read_text() == "synthetic-lock-target"

    with tempfile.TemporaryDirectory() as name:
        harness = Harness(Path(name))

        # Fcitx5 not running: classicui.conf is edited in place. The options still holding MSIME's values go back, WheelForPaging, which the user turned off again, is left, and every other line, including a same-named key in a later section, is untouched.
        harness.world()
        result = harness.unregister()
        assert harness.classicui.read_text() == restored_file(Theme="default", DarkTheme="default-dark", Font='"Sans 10"'), harness.classicui.read_text()
        assert "恢复 Fcitx5 候选面板设置 Theme、DarkTheme、Font" in result.stdout, result.stdout
        assert "WheelForPaging 在水杉写入之后已被改动，保持不变" in result.stdout, result.stdout
        assert not any(SET_CONFIG in call for call in harness.calls("gdbus")), harness.calls("gdbus")
        # IBus: the font the user had goes back; use-custom-font, which they had never set, is reset to the schema default.
        assert harness.state()["panel"]["user"] == {"custom-font": "Serif 11"}, harness.state()["panel"]
        assert ["reset", PANEL, "use-custom-font"] in harness.calls("gsettings"), harness.calls("gsettings")
        assert "已恢复 IBus 候选面板设置 custom-font、use-custom-font" in result.stdout, result.stdout
        # The generated theme and the record are gone.
        assert not harness.theme.exists() and not harness.record.exists(), result.stdout
        assert result.stderr == "", result.stderr

        # Run again: nothing recorded, nothing written; only the input method lists are looked at.
        before = harness.classicui.read_text()
        harness.log.write_text("")
        result = harness.unregister()
        assert harness.classicui.read_text() == before
        assert not any(call[0] in ("set", "reset") for call in harness.calls("gsettings")), harness.calls("gsettings")
        assert not any(SET_CONFIG in call for call in harness.calls("gdbus"))
        assert "候选面板" not in result.stdout, result.stdout

        # Fcitx5 running: the options go over D-Bus, as fcitx5-configtool sends them, so Fcitx5 applies and saves them itself; the file is not touched here.
        harness.world(fcitx5_running=True)
        harness.unregister()
        assert harness.state()["set_config"] == [["fcitx://config/addon/classicui", {
            "Theme": "default", "DarkTheme": "default-dark", "Font": "Sans 10",
        }]], harness.state()["set_config"]
        assert harness.classicui.read_text() == CLASSICUI
        assert not harness.record.exists() and not harness.theme.exists()

        # SetConfig fails: say so and edit the file instead.
        harness.world(fcitx5_running=True, set_config_fails=True)
        result = harness.unregister()
        assert "未能经 D-Bus 恢复 Fcitx5 候选面板设置" in result.stderr, result.stderr
        assert harness.classicui.read_text() == restored_file(Theme="default", DarkTheme="default-dark", Font='"Sans 10"')

        # The user picked other settings after MSIME wrote its own: all of them stay.
        changed = restored_file(Theme="nord", DarkTheme="nord-dark", Font='"Serif 12"')
        harness.world(classicui=changed, panel_user={"custom-font": "Monospace 9", "use-custom-font": False})
        result = harness.unregister()
        assert harness.classicui.read_text() == changed
        assert harness.state()["panel"]["user"] == {"custom-font": "Monospace 9", "use-custom-font": False}
        assert not any(call[0] in ("set", "reset") for call in harness.calls("gsettings")), harness.calls("gsettings")
        assert "Fcitx5 候选面板设置无需恢复" in result.stdout and "IBus 候选面板设置无需恢复" in result.stdout, result.stdout
        assert not harness.record.exists() and not harness.theme.exists()

        # A value with quotes and backslashes is written back the way Fcitx5 escapes it.
        harness.world(
            record={"fcitx5": {"Font": {"prior": 'Odd "Face" \\ 9', "written": "Noto Sans SC, Microsoft YaHei 18px"}}},
            panel_user={},
        )
        harness.unregister()
        # The theme options, which the record does not mention, still name MSIME's theme and go back to the stock ones (see below).
        assert harness.classicui.read_text() == restored_file(
            Theme="default", DarkTheme="default-dark", Font='"Odd \\"Face\\" \\\\ 9"'
        ), harness.classicui.read_text()

        # MSIME's theme is removed, so no theme option is left naming it. A record from before the hosts mapped it kept "msime" as the value replaced: the stock theme goes back instead, over D-Bus as well as in the file.
        own = {"fcitx5": {"Theme": {"prior": "msime", "written": "msime"}, "DarkTheme": {"prior": "msime", "written": "msime"}}}
        harness.world(record=own, panel_user={})
        result = harness.unregister()
        assert harness.classicui.read_text() == restored_file(Theme="default", DarkTheme="default-dark"), harness.classicui.read_text()
        assert "恢复 Fcitx5 候选面板设置 Theme、DarkTheme" in result.stdout, result.stdout
        assert not harness.record.exists() and not harness.theme.exists()
        harness.world(record=own, fcitx5_running=True, panel_user={})
        harness.unregister()
        assert harness.state()["set_config"] == [["fcitx://config/addon/classicui", {
            "Theme": "default", "DarkTheme": "default-dark",
        }]], harness.state()["set_config"]
        # A build before the record set the theme without one: the same, and no record appears.
        harness.world(record=None, panel_user={})
        result = harness.unregister()
        assert harness.classicui.read_text() == restored_file(Theme="default", DarkTheme="default-dark"), harness.classicui.read_text()
        assert "恢复 Fcitx5 候选面板设置 Theme、DarkTheme" in result.stdout, result.stdout
        assert not harness.record.exists() and not harness.theme.exists() and result.stderr == "", result.stderr
        # A theme the user picked is not touched without a record either.
        picked = restored_file(Theme="nord", DarkTheme="nord-dark")
        harness.world(record=None, classicui=picked, panel_user={})
        result = harness.unregister()
        assert harness.classicui.read_text() == picked and "候选面板设置" not in result.stdout, result.stdout

        # The record is removed under the hosts' lock, whose file stays so every writer keeps locking the same one.
        harness.world()
        lock_file = harness.record.with_name(harness.record.name + ".lock")
        lock = os.open(lock_file, os.O_RDWR | os.O_CREAT, 0o600)
        fcntl.flock(lock, fcntl.LOCK_EX)
        process = subprocess.Popen([str(harness.setup), "--unregister"], env=harness.environment,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        deadline = time.monotonic() + 30
        while harness.theme.exists() and process.poll() is None and time.monotonic() < deadline:
            time.sleep(0.05)
        time.sleep(0.5)
        assert process.poll() is None and harness.record.exists(), process.communicate()
        os.close(lock)
        _, stderr = process.communicate(timeout=30)
        assert process.returncode == 0 and "Traceback" not in stderr, stderr
        assert not harness.record.exists() and lock_file.exists()

        # A host still running records a change while the settings are restored: the record it wrote is kept for a later --unregister.
        later = json.dumps({"fcitx5": {"Font": {"prior": "Sans 10", "written": "Serif 20px"}}})
        harness.world(fcitx5_running=True, record_during_set_config=later)
        result = harness.unregister()
        assert harness.record.read_text() == later, harness.record.read_text()
        assert "恢复期间宿主又记下了候选面板设置的改动" in result.stderr, result.stderr

        # A concurrent host may replace the record with an oversized document between the first
        # read and the locked compare. Keep that record and finish uninstall cleanly.
        later = json.dumps({"fcitx5": {"Font": {"prior": "x" * (64 * 1024), "written": "msime"}}})
        harness.world(fcitx5_running=True, record_during_set_config=later)
        result = harness.unregister()
        assert "Traceback" not in result.stderr, result.stderr
        assert harness.record.read_text() == later, harness.record.read_text()

        # Without gsettings the IBus keys cannot be restored: say so, keep the record for a later run, still exit 0. The Fcitx5 options, and the theme, are dealt with regardless.
        python_only = Path(name) / "python-only"
        python_only.mkdir()
        (python_only / "python3").symlink_to(sys.executable)
        harness.world()
        environment = harness.environment
        harness.environment = {**environment, "PATH": str(python_only)}
        result = harness.unregister()
        harness.environment = environment
        assert "未能恢复候选面板设置：无法执行 gsettings" in result.stderr, result.stderr
        assert "恢复记录保留在" in result.stderr, result.stderr
        assert harness.record.exists() and not harness.theme.exists()
        assert harness.classicui.read_text() == restored_file(Theme="default", DarkTheme="default-dark", Font='"Sans 10"')

        # The hosts cap the restore record at 64 KiB. An oversized record is kept for a later
        # repair instead of being loaded without a bound or treated as a valid restore document.
        huge = {"fcitx5": {"Font": {"prior": "x" * (64 * 1024), "written": "msime"}}}
        harness.world(record=huge)
        result = harness.unregister()
        assert "无法读取候选面板设置的恢复记录" in result.stderr, result.stderr
        assert harness.record.exists() and not harness.theme.exists()

        # XDG_STATE_HOME, XDG_CONFIG_HOME and XDG_DATA_HOME are honoured when absolute, as the hosts resolve them.
        harness.world()
        state, config, data = (Path(name) / part for part in ("xdg-state", "xdg-config", "xdg-data"))
        (state / "msime-client").mkdir(parents=True)
        harness.record.rename(state / "msime-client/panel-restore.json")
        (config / "fcitx5/conf").mkdir(parents=True)
        harness.classicui.rename(config / "fcitx5/conf/classicui.conf")
        (data / "fcitx5/themes").mkdir(parents=True)
        harness.theme.rename(data / "fcitx5/themes/msime")
        harness.environment.update(XDG_STATE_HOME=str(state), XDG_CONFIG_HOME=str(config), XDG_DATA_HOME=str(data))
        harness.unregister()
        assert (config / "fcitx5/conf/classicui.conf").read_text() == restored_file(Theme="default", DarkTheme="default-dark", Font='"Sans 10"')
        assert not (state / "msime-client/panel-restore.json").exists() and not (data / "fcitx5/themes/msime").exists()

    print("setup --unregister restores the candidate panel settings the hosts replaced")
    return 0


if __name__ == "__main__":
    sys.exit(main())

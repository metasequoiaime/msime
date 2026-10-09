#!/usr/bin/env python3
"""msime-linux-setup 在状态目录就绪后把输入法加入正在运行的宿主的输入法列表，--unregister 在卸载时把它从这些列表里移除，--register 在发行版的包替换另一个包之后把两者都恢复。

用桩代替 pgrep、gdbus、gsettings、ibus、systemctl 和 msime-linux-prepare：桩把收到的调用记进日志，把 Fcitx5 输入法组、dconf 设置和 IBus 已知的引擎存在一份 JSON 里。不需要词库、不联网，也不碰真实的 D-Bus 会话或 dconf。
"""
import hashlib
import importlib.machinery
import importlib.util
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/msime-linux-setup"

# One stub serves every tool; it dispatches on the name it was invoked as. Arguments the setup script writes are Python literals once GVariant's text form is read back, so the stub parses them with ast rather than reusing the code under test.
# 首行用跑测试的同一个解释器，不经 /usr/bin/env：没有 FHS 布局的环境（Nix 构建沙箱）里没有它。
STUB = f"#!{sys.executable}\n" + r'''import ast, json, os, re, sys
from pathlib import Path

name = Path(sys.argv[0]).name
arguments = sys.argv[1:]
state_file = Path(os.environ["STUB_STATE"])
state = json.loads(state_file.read_text())
with open(os.environ["STUB_LOG"], "a") as log:
    log.write(json.dumps([name, *arguments], ensure_ascii=False) + "\n")
    # Which session bus the call would have reached, so the tests can see the address --unregister derives when the environment has none.
    log.write(json.dumps(["bus", name, os.environ.get("DBUS_SESSION_BUS_ADDRESS")]) + "\n")


def text(value):
    if isinstance(value, str):
        return "'" + value.replace("\\", "\\\\").replace("'", "\\'") + "'"
    if isinstance(value, tuple):
        return "(" + ", ".join(text(item) for item in value) + ("," if len(value) == 1 else "") + ")"
    return "[" + ", ".join(text(item) for item in value) + "]"


def listing(value, element):
    # gsettings and gdbus annotate an empty array with its type, since [] alone does not say what it holds.
    return text(value) if value else f"@a{element} []"


def save():
    state_file.write_text(json.dumps(state, ensure_ascii=False))


if name == "pgrep":
    if arguments[0] == "-f":
        sys.exit(0 if any(re.search(arguments[-1], command) for command in state["commands"]) else 1)
    sys.exit(0 if arguments[-1] in state["running"] else 1)
if name in ("systemctl", "msime-linux-prepare"):
    if name == "msime-linux-prepare":
        Path(arguments[-1]).mkdir(parents=True)
    sys.exit(0)
if name == "gdbus":
    fcitx5 = state["fcitx5"]
    if fcitx5.get("error"):
        print("Error: GDBus.Error:org.freedesktop.DBus.Error.ServiceUnknown: The name org.fcitx.Fcitx5 was not provided by any .service files", file=sys.stderr)
        sys.exit(1)
    method = arguments[arguments.index("--method") + 1].rsplit(".", 1)[1]
    values = [ast.literal_eval(argument) for argument in arguments[arguments.index("--method") + 2:]]
    if method == "NameHasOwner":
        # A world without a current group is one where Fcitx5 is not running on this bus.
        print("(true,)" if values == ["org.fcitx.Fcitx5"] and "current" in fcitx5 else "(false,)")
    elif method == "AvailableInputMethods":
        # a(ssssssb): unique name, name, native name, icon, label, language, configurable; gdbus spells the boolean in lower case.
        entries = ", ".join(f"({text(im)}, {text(im)}, '', '', '', '', true)" for im in fcitx5["loaded"])
        print(f"([{entries}],)" if entries else "(@a(ssssssb) [],)")
    elif method == "Restart":
        # The new instance loads every installed addon, including the one installed after the old instance started.
        fcitx5["loaded"] = fcitx5["installed"]
        save()
        print("()")
    elif method == "CurrentInputMethodGroup":
        print(text((fcitx5["current"],)))
    elif method == "InputMethodGroupInfo":
        layout, items = fcitx5["groups"][values[0]]
        print(f"({text(layout)}, {listing([tuple(item) for item in items], '(ss)')})")
    elif method == "SetInputMethodGroupInfo":
        group, layout, items = values
        # Like InputMethodManager::setGroup, entries naming an input method Fcitx5 has not loaded are dropped.
        fcitx5["groups"][group] = [layout, [list(item) for item in items if item[0] in fcitx5["loaded"]]]
        save()
        print("()")
    else:
        sys.exit(2)
    sys.exit(0)
if name == "gsettings":
    settings = state["gsettings"]
    if arguments[0] == "list-schemas":
        print("\n".join(settings))
    elif arguments[0] == "get":
        value = settings[arguments[1]][arguments[2]]
        element = "(ss)" if arguments[1] == "org.gnome.desktop.input-sources" else "s"
        print(listing([tuple(item) if isinstance(item, list) else item for item in value], element))
    elif arguments[0] == "set":
        settings[arguments[1]][arguments[2]] = ast.literal_eval(arguments[3])
        save()
    sys.exit(0)
if name == "ibus":
    ibus = state["ibus"]
    if arguments == ["list-engine"]:
        print("language: English")
        print("  xkb:us::eng - English (US)")
        if ibus["known"]:
            print("language: Chinese")
            print("  msime-linux - Metasequoia 水杉输入法")
    elif arguments == ["restart"]:
        # The new daemon reads the component files, including the one installed after the old daemon started.
        ibus["known"] = ibus["installed"]
        save()
    sys.exit(0)
sys.exit(127)
'''

TOOLS = ("pgrep", "systemctl", "gdbus", "gsettings", "ibus")
GNOME = "org.gnome.desktop.input-sources"
IBUS = "org.freedesktop.ibus.general"


def load_setup():
    spec = importlib.util.spec_from_loader(
        "msime_client_setup", importlib.machinery.SourceFileLoader("msime_client_setup", str(SCRIPT))
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def gvariant_round_trip() -> None:
    setup = load_setup()
    # 空数组带类型标注；字符串里的 @ 和引号不能被当成标注或截断。
    assert setup.parse_gvariant("('us', @a(ss) [])\n") == ("us", [])
    assert setup.parse_gvariant("@as []") == []
    assert setup.parse_gvariant("[('xkb', 'us'), ('ibus', 'mozc-jp')]") == [("xkb", "us"), ("ibus", "mozc-jp")]
    assert setup.parse_gvariant("('it\\'s @as x',)") == ("it's @as x",)
    assert setup.parse_gvariant('(["a\\"b"],)') == (['a"b'],)
    value = [("Default", "it's"), ("a\\b", "")]
    assert setup.parse_gvariant(setup.gvariant_text(value)) == value
    assert setup.gvariant_text(("Default",)) == "('Default',)"
    # 布尔值按 GVariant 的小写写法读；字符串里的 true 不动。
    assert setup.parse_gvariant("([('msime', 'true', '', '', '', 'zh_CN', true), ('x', '', '', '', '', '', false)],)") == (
        [("msime", "true", "", "", "", "zh_CN", True), ("x", "", "", "", "", "", False)],
    )
    try:
        setup.parse_gvariant("<not gvariant>")
    except setup.RegistrationFailed:
        pass
    else:
        raise AssertionError("unparseable output must fall back, not crash setup")


class Harness:
    def __init__(self, scratch: Path):
        self.scratch = scratch
        prefix = scratch / "prefix"
        (prefix / "bin").mkdir(parents=True)
        self.setup = prefix / "bin/msime-linux-setup"
        self.setup.write_text(SCRIPT.read_text())
        self.setup.chmod(0o755)
        tools = scratch / "tools"
        tools.mkdir()
        stub = tools / "stub"
        stub.write_text(STUB)
        stub.chmod(0o755)
        for tool in TOOLS:
            (tools / tool).symlink_to(stub)
        (prefix / "bin/msime-linux-prepare").symlink_to(stub)
        resources = scratch / "resources"
        resources.mkdir()
        payload = b"synthetic dictionary"
        (resources / "msime-pinyin.db").write_bytes(payload)
        lock = scratch / "desktop-dictionary.lock.json"
        lock.write_text(json.dumps({"artifacts": [
            {"name": "msime-pinyin.db", "size": len(payload), "sha256": hashlib.sha256(payload).hexdigest(),
             "url": "https://example.invalid/msime-pinyin.db"},
        ]}))
        self.resources = resources
        self.state_file = scratch / "stub-state.json"
        self.log = scratch / "calls.log"
        self.runs = 0
        self.environment = {
            key: value for key, value in os.environ.items()
            if not key.startswith(("MSIME_", "XDG_"))
        }
        self.environment.update(
            PATH=f"{tools}:{os.environ.get('PATH', '/usr/bin:/bin')}",
            HOME=str(scratch / "home"),
            XDG_CONFIG_HOME=str(scratch / "config"),
            XDG_DATA_HOME=str(scratch / "data"),
            MSIME_DICTIONARY_LOCK=str(lock),
            STUB_STATE=str(self.state_file),
            STUB_LOG=str(self.log),
        )

    def world(self, running=(), desktop="", fcitx5=None, gsettings=None, ibus=None, commands=()) -> None:
        # running 是进程名（pgrep -x），commands 是完整命令行（pgrep -f）。
        self.state_file.write_text(json.dumps({
            "running": list(running),
            "commands": list(commands),
            "fcitx5": fcitx5 or {},
            "gsettings": gsettings or {},
            "ibus": ibus or {"known": False, "installed": True},
        }, ensure_ascii=False))
        self.environment["XDG_CURRENT_DESKTOP"] = desktop
        self.log.write_text("")

    def state(self) -> dict:
        return json.loads(self.state_file.read_text())

    def systemctl_changes(self) -> list:
        """systemctl 调用里改动用户级 systemd 的那些；`show-environment` 只是读取（GNOME Wayland 提示据此判断 GTK_IM_MODULE），不算。"""
        return [call for call in self.calls("systemctl") if call != ["--user", "show-environment"]]

    def calls(self, name: str) -> list:
        return [call[1:] for call in map(json.loads, self.log.read_text().splitlines()) if call[0] == name]

    def unregister(self, **overrides: str) -> subprocess.CompletedProcess:
        # The session bus address is set per case: the derivation from XDG_RUNTIME_DIR must not depend on the environment the test runs in.
        environment = {key: value for key, value in self.environment.items() if key != "DBUS_SESSION_BUS_ADDRESS"}
        environment.update(overrides)
        result = subprocess.run(
            [str(self.setup), "--unregister"], env=environment, capture_output=True, text=True, timeout=30,
        )
        assert result.returncode == 0, result
        assert "Traceback" not in result.stderr, result.stderr
        # Only the lists: no dictionary check, no state preparation, no services.
        assert self.calls("msime-linux-prepare") == [] and self.systemctl_changes() == [], self.log.read_text()
        assert not (self.scratch / "config/msime-client").exists()
        assert "词库" not in result.stdout, result.stdout
        return result

    def register(self, *extra: str, **overrides: str) -> subprocess.CompletedProcess:
        environment = {key: value for key, value in self.environment.items() if key != "DBUS_SESSION_BUS_ADDRESS"}
        environment.update(overrides)
        return subprocess.run(
            [str(self.setup), "--register", *extra], env=environment, capture_output=True, text=True, timeout=30,
        )

    def run(self, *extra: str) -> subprocess.CompletedProcess:
        # msime-linux-setup refuses to prepare a directory that exists, so every run gets a fresh one.
        self.runs += 1
        state = self.scratch / f"state-{self.runs}"
        result = subprocess.run(
            [str(self.setup), "--resources", str(self.resources), "--state", str(state), *extra],
            env=self.environment, capture_output=True, text=True, timeout=30,
        )
        assert result.returncode == 0, result
        assert state.is_dir(), result
        assert f"状态目录已就绪：{state}" in result.stdout, result.stdout
        return result


def fcitx5_world(**overrides) -> dict:
    world = {
        "current": "Default",
        "groups": {"Default": ["us", [["keyboard-us", ""], ["pinyin", ""]]], "Other": ["de", []]},
        "loaded": ["keyboard-us", "pinyin", "msime"],
        "installed": ["keyboard-us", "pinyin", "msime"],
    }
    world.update(overrides)
    return world


def unregistering() -> None:
    with tempfile.TemporaryDirectory() as name:
        harness = Harness(Path(name))
        set_group = "org.fcitx.Fcitx.Controller1.SetInputMethodGroupInfo"
        everywhere = {
            GNOME: {"sources": [["xkb", "us"], ["ibus", "msime-linux"], ["ibus", "mozc-jp"]]},
            IBUS: {"preload-engines": ["xkb:us::eng", "msime-linux", "libpinyin"]},
        }
        fcitx5 = fcitx5_world(groups={
            "Default": ["us", [["keyboard-us", ""], ["msime", ""], ["pinyin", ""]]], "Other": ["de", [["msime", ""]]],
        })

        # 卸载：从 Fcitx5 当前组、GNOME 输入源和 IBus 预载引擎三处移除，其余项保持原来的顺序，别的组不动。卸载从用户的 systemd 实例里运行，那里往往没有 XDG_CURRENT_DESKTOP，所以两份 IBus 列表都清理，与当前跑的是哪个宿主无关。
        harness.world(fcitx5=fcitx5, gsettings=everywhere)
        result = harness.unregister()
        assert "已从 Fcitx5 当前输入法组「Default」移除「水杉输入法」" in result.stdout, result
        assert "已从 org.gnome.desktop.input-sources sources 移除「Metasequoia 水杉输入法」" in result.stdout, result
        assert "已从 org.freedesktop.ibus.general preload-engines 移除「Metasequoia 水杉输入法」" in result.stdout, result
        assert result.stderr == "", result.stderr
        state = harness.state()
        assert state["fcitx5"]["groups"] == {
            "Default": ["us", [["keyboard-us", ""], ["pinyin", ""]]], "Other": ["de", [["msime", ""]]],
        }, state["fcitx5"]
        assert state["gsettings"][GNOME]["sources"] == [["xkb", "us"], ["ibus", "mozc-jp"]], state["gsettings"]
        assert state["gsettings"][IBUS]["preload-engines"] == ["xkb:us::eng", "libpinyin"], state["gsettings"]
        writes = [call for call in harness.calls("gdbus") if set_group in call]
        assert writes == [[
            "call", "--session", "--dest", "org.fcitx.Fcitx5", "--object-path", "/controller", "--method",
            set_group, "'Default'", "'us'", "[('keyboard-us', ''), ('pinyin', '')]",
        ]], writes
        # 注销不需要 Fcitx5 或 IBus 重新加载任何东西。
        assert not any("org.fcitx.Fcitx.Controller1.Restart" in call for call in harness.calls("gdbus"))
        assert harness.calls("ibus") == [] and harness.calls("pgrep") == [], harness.log.read_text()

        # 再跑一次：都已不在列表里，只读不写。
        harness.log.write_text("")
        result = harness.unregister()
        assert "「水杉输入法」不在 Fcitx5 当前输入法组「Default」中" in result.stdout, result
        assert result.stdout.count("「Metasequoia 水杉输入法」不在") == 2, result.stdout
        assert not any(set_group in call for call in harness.calls("gdbus")), harness.log.read_text()
        assert not any(call[0] == "set" for call in harness.calls("gsettings")), harness.calls("gsettings")
        assert harness.state() == state

        # 列表里只有水杉：移除后会变空，空列表会让桌面退回一个未必是用户原来的默认值，Fcitx5 空组则没有可切回的键盘布局，所以保持原样。
        alone = {GNOME: {"sources": [["ibus", "msime-linux"]]}, IBUS: {"preload-engines": ["msime-linux"]}}
        harness.world(fcitx5=fcitx5_world(current="Other", groups=fcitx5["groups"]), gsettings=alone)
        before = harness.state()
        result = harness.unregister()
        assert "Fcitx5 当前输入法组「Other」只有「水杉输入法」，移除后会变空，保持不变" in result.stdout, result
        assert "org.gnome.desktop.input-sources sources 只有「Metasequoia 水杉输入法」，移除后会变空，保持不变" in result.stdout, result
        assert "org.freedesktop.ibus.general preload-engines 只有「Metasequoia 水杉输入法」，移除后会变空，保持不变" in result.stdout, result
        assert harness.state() == before
        assert not any(set_group in call for call in harness.calls("gdbus"))
        assert not any(call[0] == "set" for call in harness.calls("gsettings"))

        # Fcitx5 没在这个会话总线上：不去调用它（免得被 D-Bus 激活），IBus 列表照常清理。
        harness.world(gsettings=everywhere)
        result = harness.unregister()
        assert "Fcitx5 没有在运行，其输入法组未改动" in result.stdout, result
        assert [call[-1] for call in harness.calls("gdbus")] == ["'org.fcitx.Fcitx5'"], harness.calls("gdbus")
        assert harness.state()["gsettings"][IBUS]["preload-engines"] == ["xkb:us::eng", "libpinyin"]

        # 会话总线连不上：说明原因，其余列表照常处理，退出码仍为 0。
        harness.world(fcitx5=fcitx5_world(error=True), gsettings=everywhere)
        result = harness.unregister()
        assert "未能从输入法列表移除：gdbus call 失败：" in result.stderr, result
        assert harness.state()["gsettings"][GNOME]["sources"] == [["xkb", "us"], ["ibus", "mozc-jp"]]

        # 没有 gdbus 也没有 gsettings（最小化系统、容器）：逐项说明，不失败。
        python_only = Path(name) / "python-only"
        python_only.mkdir()
        (python_only / "python3").symlink_to(sys.executable)
        harness.world(fcitx5=fcitx5, gsettings=everywhere)
        result = harness.unregister(PATH=str(python_only))
        assert "无法执行 gdbus" in result.stderr and "无法执行 gsettings" in result.stderr, result.stderr
        assert harness.log.read_text() == "", harness.log.read_text()

        # 从用户的 systemd 实例运行时环境里常常没有会话总线地址：按 XDG_RUNTIME_DIR/bus 补上，否则 gsettings 会写进一个用完即弃的后端。没装 GNOME 的 schema 时只处理 IBus 的列表。
        runtime = Path(name) / "runtime"
        runtime.mkdir()
        (runtime / "bus").touch()
        harness.world(fcitx5=fcitx5, gsettings={IBUS: everywhere[IBUS]})
        result = harness.unregister(XDG_RUNTIME_DIR=str(runtime))
        buses = {call[1] for call in harness.calls("bus")}
        assert buses == {f"unix:path={runtime}/bus"}, buses
        assert "org.gnome.desktop.input-sources" not in result.stdout, result.stdout
        assert harness.state()["gsettings"] == {IBUS: {"preload-engines": ["xkb:us::eng", "libpinyin"]}}

        # 已有的地址不被覆盖。
        harness.world(fcitx5=fcitx5, gsettings=everywhere)
        harness.unregister(XDG_RUNTIME_DIR=str(runtime), DBUS_SESSION_BUS_ADDRESS="unix:path=/session/bus")
        assert {call[1] for call in harness.calls("bus")} == {"unix:path=/session/bus"}


def registering_again() -> None:
    """--register：发行版的包替换掉发布页的 msime-linux（或 AUR 的 msime 与 msime-bin 互换）时，被替换的包按卸载处理，停用了用户单元并运行了 --unregister。新包装好后对每个用户运行 --register，把这两样恢复，不碰词库和状态。"""
    with tempfile.TemporaryDirectory() as name:
        harness = Harness(Path(name))
        state = Path(name) / "config/msime-client"
        units = ["msime-linux-online.socket", "msime-linux-voice.socket", "msime-linux-clipboard.service"]

        # 还没配置过的用户（新装的机器上就是所有人）：什么也不做，不启用服务、不碰任何列表。
        harness.world(running=["fcitx5"], fcitx5=fcitx5_world())
        result = harness.register()
        assert result.returncode == 0 and "Traceback" not in result.stderr, result
        assert "还没有完成首次配置" in result.stdout, result.stdout
        assert harness.log.read_text() == "", harness.log.read_text()
        assert not state.exists()

        # 配置过的用户：启用首次配置启用的那几个单元，把输入法加回 Fcitx5 当前组；不检查词库，不运行 msime-linux-prepare，状态目录原样。
        state.mkdir(parents=True)
        (state / "runtime-options.json").write_text('{"resources": "/synthetic"}')
        before = {path.name: path.read_bytes() for path in state.iterdir()}
        fcitx5 = fcitx5_world()
        harness.world(running=["fcitx5"], fcitx5=fcitx5)
        result = harness.register()
        assert result.returncode == 0 and result.stderr == "", result
        assert harness.systemctl_changes() == [["--user", "enable", "--now", *units]], harness.calls("systemctl")
        assert "已把「水杉输入法」加入 Fcitx5 当前输入法组「Default」" in result.stdout, result.stdout
        assert harness.state()["fcitx5"]["groups"]["Default"] == ["us", [["keyboard-us", ""], ["pinyin", ""], ["msime", ""]]]
        assert harness.calls("msime-linux-prepare") == [], harness.log.read_text()
        assert "词库" not in result.stdout, result.stdout
        assert {path.name: path.read_bytes() for path in state.iterdir()} == before

        # 与 --unregister 来回一次：撤下之后再加回来，结果与撤下之前相同；已在列表里时不重复写入。
        everywhere = {GNOME: {"sources": [["xkb", "us"], ["ibus", "mozc-jp"]]}, IBUS: {"preload-engines": ["xkb:us::eng"]}}
        harness.world(running=["ibus-daemon"], desktop="GNOME", gsettings=everywhere, ibus={"known": True, "installed": True})
        harness.register()
        registered = harness.state()["gsettings"]
        assert registered[GNOME]["sources"] == [["xkb", "us"], ["ibus", "mozc-jp"], ["ibus", "msime-linux"]], registered
        # 不用 harness.unregister：它断言状态目录不存在，这里的用户是配置过的。
        result = subprocess.run([str(harness.setup), "--unregister"], env=harness.environment, capture_output=True, text=True, timeout=30)
        assert result.returncode == 0, result
        assert harness.state()["gsettings"][GNOME]["sources"] == [["xkb", "us"], ["ibus", "mozc-jp"]]
        result = harness.register()
        assert harness.state()["gsettings"] == registered, harness.state()["gsettings"]
        harness.log.write_text("")
        result = harness.register()
        assert "「Metasequoia 水杉输入法」已在输入源列表中" in result.stdout, result.stdout
        assert not any(call[0] == "set" for call in harness.calls("gsettings")), harness.calls("gsettings")

        # 包管理器经 systemd-run 在用户的 systemd 实例里运行它，那里往往没有会话总线地址：与 --unregister 一样按 XDG_RUNTIME_DIR/bus 补上。
        runtime = Path(name) / "runtime"
        runtime.mkdir()
        (runtime / "bus").touch()
        harness.world(running=["fcitx5"], fcitx5=fcitx5_world())
        harness.register(XDG_RUNTIME_DIR=str(runtime))
        assert {call[1] for call in harness.calls("bus")} == {f"unix:path={runtime}/bus"}, harness.calls("bus")

        # 两个宿主都没在跑：服务照样启用，列表无处可加，说明之后怎么补。
        harness.world()
        result = harness.register()
        assert result.returncode == 0, result
        assert harness.systemctl_changes() == [["--user", "enable", "--now", *units]], harness.calls("systemctl")
        assert "msime-linux-setup --register" in result.stdout, result.stdout
        assert harness.calls("gdbus") == [] and harness.calls("gsettings") == [] and harness.calls("ibus") == []

        # 两个方向不能同时要。
        result = harness.register("--unregister")
        assert result.returncode == 2 and "不能同时使用" in result.stderr, result


def main() -> int:
    gvariant_round_trip()
    with tempfile.TemporaryDirectory() as name:
        harness = Harness(Path(name))
        fcitx5_desktop = {GNOME: {"sources": [["xkb", "us"]]}, IBUS: {"preload-engines": ["xkb:us::eng"]}}

        # Fcitx5：加到当前组末尾，保留组里原有的项和默认布局，别的组不动；不碰 gsettings 与 ibus。
        harness.world(running=["fcitx5", "ibus-daemon"], desktop="KDE", fcitx5=fcitx5_world(), gsettings=fcitx5_desktop)
        result = harness.run()
        assert "已把「水杉输入法」加入 Fcitx5 当前输入法组「Default」" in result.stdout, result
        assert "下一步" not in result.stdout, result.stdout
        # 托盘入口的提示与是否自动加入无关，照常打印。
        assert "状态栏上的中英文" in result.stdout, result.stdout
        groups = harness.state()["fcitx5"]["groups"]
        assert groups["Default"] == ["us", [["keyboard-us", ""], ["pinyin", ""], ["msime", ""]]], groups
        assert groups["Other"] == ["de", []], groups
        writes = [call for call in harness.calls("gdbus") if "org.fcitx.Fcitx.Controller1.SetInputMethodGroupInfo" in call]
        assert writes == [[
            "call", "--session", "--dest", "org.fcitx.Fcitx5", "--object-path", "/controller", "--method",
            "org.fcitx.Fcitx.Controller1.SetInputMethodGroupInfo",
            "'Default'", "'us'", "[('keyboard-us', ''), ('pinyin', ''), ('msime', '')]",
        ]], writes
        assert harness.calls("gsettings") == [] and harness.calls("ibus") == [], harness.log.read_text()
        assert not any("org.fcitx.Fcitx.Controller1.Restart" in call for call in harness.calls("gdbus")), harness.log.read_text()

        # 再跑一次：已经在组里，只读不写。
        harness.log.write_text("")
        result = harness.run()
        assert "「水杉输入法」已在 Fcitx5 当前输入法组「Default」中" in result.stdout, result
        assert not any("org.fcitx.Fcitx.Controller1.SetInputMethodGroupInfo" in call for call in harness.calls("gdbus"))

        # 空组（gdbus 打印带类型标注的空数组）：组里第一项是非激活状态用的输入法，应当是键盘布局；只写入本输入法会让它无处可切回，所以不写，退回手动步骤。
        harness.world(running=["fcitx5"], fcitx5=fcitx5_world(current="Other"))
        result = harness.run()
        assert "未能自动加入输入法列表：Fcitx5 当前输入法组「Other」为空" in result.stderr, result
        assert "下一步：用 fcitx5-configtool 把「水杉输入法」（英文界面显示为「MSIME」）加入当前输入法组。" in result.stdout, result
        assert harness.state()["fcitx5"]["groups"]["Other"] == ["de", []]
        assert not any("org.fcitx.Fcitx.Controller1.SetInputMethodGroupInfo" in call for call in harness.calls("gdbus"))

        # Fcitx5 在装包之前就已启动、还没加载 msime：先让它重启一次，加载新装的输入法之后再加入当前组。
        harness.world(running=["fcitx5"], fcitx5=fcitx5_world(loaded=["keyboard-us", "pinyin"]))
        result = harness.run()
        assert "已把「水杉输入法」加入 Fcitx5 当前输入法组「Default」" in result.stdout, result
        restarts = [call for call in harness.calls("gdbus") if "org.fcitx.Fcitx.Controller1.Restart" in call]
        assert len(restarts) == 1, harness.log.read_text()
        assert harness.state()["fcitx5"]["groups"]["Default"] == ["us", [["keyboard-us", ""], ["pinyin", ""], ["msime", ""]]]

        # 再跑一次：已经加载，不再重启 Fcitx5，也不重复写入。
        harness.log.write_text("")
        result = harness.run()
        assert "「水杉输入法」已在 Fcitx5 当前输入法组「Default」中" in result.stdout, result
        assert not any("org.fcitx.Fcitx.Controller1.Restart" in call for call in harness.calls("gdbus")), harness.log.read_text()
        assert not any("org.fcitx.Fcitx.Controller1.SetInputMethodGroupInfo" in call for call in harness.calls("gdbus"))

        # 重启之后仍没有加载（比如插件没装到 Fcitx5 找得到的位置）：不写，退回手动步骤，不谎报成功。
        harness.world(running=["fcitx5"], fcitx5=fcitx5_world(loaded=["keyboard-us", "pinyin"], installed=["keyboard-us", "pinyin"]))
        result = harness.run()
        assert "未能自动加入输入法列表：Fcitx5 重启之后仍没有加载 msime" in result.stderr, result
        assert "下一步：用 fcitx5-configtool 把「水杉输入法」（英文界面显示为「MSIME」）加入当前输入法组。" in result.stdout, result
        assert "已把" not in result.stdout, result.stdout
        assert not any("org.fcitx.Fcitx.Controller1.SetInputMethodGroupInfo" in call for call in harness.calls("gdbus"))

        # D-Bus 调用失败：说清楚原因，退回到手动步骤，状态目录照样准备好、退出码仍为 0。
        harness.world(running=["fcitx5"], fcitx5=fcitx5_world(error=True))
        result = harness.run()
        assert "未能自动加入输入法列表：gdbus call 失败：" in result.stderr, result
        assert "ServiceUnknown" in result.stderr, result.stderr
        assert "Traceback" not in result.stderr, result.stderr
        assert "下一步：用 fcitx5-configtool" in result.stdout, result.stdout

        # --no-register：一次 D-Bus 调用都不发，只打印手动步骤。
        harness.world(running=["fcitx5"], fcitx5=fcitx5_world())
        result = harness.run("--no-register")
        assert harness.calls("gdbus") == [], harness.log.read_text()
        assert "下一步：用 fcitx5-configtool" in result.stdout, result.stdout
        assert "未能自动加入" not in result.stderr, result.stderr

        # IBus + GNOME：ibus-daemon 还不认识新装的引擎，先 ibus restart；GNOME Shell 只读自己的输入源，所以写 sources 而不是 preload-engines。
        harness.world(
            running=["ibus-daemon"], desktop="ubuntu:GNOME",
            gsettings={GNOME: {"sources": [["xkb", "us"], ["ibus", "mozc-jp"]]}, IBUS: {"preload-engines": ["xkb:us::eng"]}},
        )
        result = harness.run()
        assert "已把「Metasequoia 水杉输入法」加入输入源列表" in result.stdout, result
        assert harness.calls("ibus").count(["restart"]) == 1, harness.calls("ibus")
        settings = harness.state()["gsettings"]
        assert settings[GNOME]["sources"] == [["xkb", "us"], ["ibus", "mozc-jp"], ["ibus", "msime-linux"]], settings
        assert settings[IBUS]["preload-engines"] == ["xkb:us::eng"], settings
        assert harness.calls("gdbus") == [], harness.log.read_text()

        # 再跑一次：引擎已被列出就不再重启 IBus，也不重复写入。
        harness.log.write_text("")
        result = harness.run()
        assert "「Metasequoia 水杉输入法」已在输入源列表中" in result.stdout, result
        assert ["restart"] not in harness.calls("ibus"), harness.calls("ibus")
        assert not any(call[0] == "set" for call in harness.calls("gsettings")), harness.calls("gsettings")

        # 其他桌面上的 IBus 面板读 preload-engines；引擎已被列出时不重启。GNOME 的 schema 装着也不去写它。
        harness.world(
            running=["ibus-daemon"], desktop="KDE",
            gsettings={GNOME: {"sources": [["xkb", "us"]]}, IBUS: {"preload-engines": ["xkb:us::eng", "libpinyin"]}},
            ibus={"known": True, "installed": True},
        )
        result = harness.run()
        assert "已把「Metasequoia 水杉输入法」加入输入源列表" in result.stdout, result
        assert ["restart"] not in harness.calls("ibus"), harness.calls("ibus")
        settings = harness.state()["gsettings"]
        assert settings[IBUS]["preload-engines"] == ["xkb:us::eng", "libpinyin", "msime-linux"], settings
        assert settings[GNOME]["sources"] == [["xkb", "us"]], settings

        # 经包装脚本启动的 ibus-daemon（Nix 的 wrapProgram）进程名不是 ibus-daemon，按命令行的第一个词认出它；
        # 只是参数里出现 ibus-daemon 的进程不算。
        hyprland = {IBUS: {"preload-engines": ["xkb:us::eng"]}}
        harness.world(
            desktop="Hyprland", gsettings=hyprland, ibus={"known": True, "installed": True},
            commands=["/nix/store/0-ibus-with-plugins-1.5.34/bin/ibus-daemon --cache=refresh --daemonize --xim"],
        )
        result = harness.run()
        assert "已把「Metasequoia 水杉输入法」加入输入源列表" in result.stdout, result
        assert harness.state()["gsettings"][IBUS]["preload-engines"] == ["xkb:us::eng", "msime-linux"]
        harness.world(desktop="Hyprland", gsettings=hyprland, commands=["/usr/bin/python3 /tmp/ibus-daemon"])
        result = harness.run()
        assert "下一步：启动 fcitx5 或 ibus，再在各自的设置里加入水杉输入法。" in result.stdout, result

        # 列表为空说明桌面在用没有写进这一项的默认输入源，只写入本引擎会把它顶掉：不写，退回手动步骤。
        harness.world(
            running=["ibus-daemon"], desktop="GNOME",
            gsettings={GNOME: {"sources": []}, IBUS: {"preload-engines": []}},
            ibus={"known": True, "installed": True},
        )
        result = harness.run()
        assert "未能自动加入输入法列表：org.gnome.desktop.input-sources sources 为空" in result.stderr, result
        assert "下一步：ibus restart，再在输入源里添加「Metasequoia 水杉输入法」。" in result.stdout, result
        assert not any(call[0] == "set" for call in harness.calls("gsettings")), harness.calls("gsettings")

        # 两个宿主都没在跑：无处可加，也不去调用任何一方。
        harness.world(fcitx5=fcitx5_world(), gsettings=fcitx5_desktop)
        result = harness.run()
        assert "下一步：启动 fcitx5 或 ibus，再在各自的设置里加入水杉输入法。" in result.stdout, result
        assert harness.calls("gdbus") == [] and harness.calls("gsettings") == [] and harness.calls("ibus") == []

    unregistering()
    registering_again()
    print("setup registration tests passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())

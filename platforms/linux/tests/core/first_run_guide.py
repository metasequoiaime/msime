#!/usr/bin/env python3
"""选中输入法却还没做首次配置时的引导：IBus 启动器与共享的引导脚本；以及升级后下载的词库落后于当前版本时的通知。

用桩代替 msime-linux-settings、notify-send 和 msime-linux-ibus，只看谁被调用了几次，不联网、不需要词库，也不起任何桌面进程。
"""
import os
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPTS = ROOT / "scripts"


def stub(path: Path, log: Path, name: str) -> None:
    path.write_text(f'#!/bin/sh\nprintf "%s %s\\n" "{name}" "$*" >> "{log}"\n')
    path.chmod(0o755)


def calls(log: Path, name: str) -> list:
    if not log.exists():
        return []
    return [line for line in log.read_text().splitlines() if line.startswith(name + " ")]


def wait_for(predicate, message: str) -> None:
    # The guide detaches both children, so their log lines land shortly after it returns.
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(0.05)
    raise AssertionError(message)


def settle() -> None:
    # Long enough for a detached child that should not exist to have written its line.
    time.sleep(0.5)


def main() -> int:
    with tempfile.TemporaryDirectory() as name:
        scratch = Path(name)
        bin_dir = scratch / "bin"
        bin_dir.mkdir()
        log = scratch / "calls.log"
        for script in ("msime-linux-ibus-launcher", "msime-linux-first-run-guide"):
            target = bin_dir / script
            target.write_text((SCRIPTS / script).read_text())
            target.chmod(0o755)
        stub(bin_dir / "msime-linux-settings", log, "settings")
        stub(bin_dir / "msime-linux-ibus", log, "ibus")
        tools = scratch / "tools"
        tools.mkdir()
        stub(tools / "notify-send", log, "notify")
        runtime = scratch / "runtime"
        runtime.mkdir(mode=0o700)
        config_home = scratch / "config"
        system_options = scratch / "etc/runtime-options.json"

        base = {
            key: value
            for key, value in os.environ.items()
            if not key.startswith("MSIME_") and key not in ("DISPLAY", "WAYLAND_DISPLAY")
        }
        base.update(
            PATH=f"{tools}:{os.environ.get('PATH', '/usr/bin:/bin')}",
            HOME=str(scratch / "home"),
            XDG_CONFIG_HOME=str(config_home),
            XDG_RUNTIME_DIR=str(runtime),
        )
        graphical = dict(base, WAYLAND_DISPLAY="wayland-0")

        def launch(environment: dict) -> subprocess.CompletedProcess:
            return subprocess.run(
                [str(bin_dir / "msime-linux-ibus-launcher"), str(system_options)],
                env=environment, capture_output=True, text=True, timeout=10,
            )

        # 没有图形会话：只留 stderr，不开窗口、不发通知，也不占用冷却期，登录桌面后照样能引导。
        result = launch(base)
        assert result.returncode == 1, result
        assert "MSIME runtime options not found" in result.stderr, result.stderr
        settle()
        assert calls(log, "settings") == [] and calls(log, "notify") == [], log.read_text() if log.exists() else ""
        assert not (runtime / "msime-client/first-run-guide.stamp").exists()

        # 有图形会话：退出码语义不变，设置窗口与通知各一次，窗口不带任何面板参数（由首次配置页接管）。
        result = launch(graphical)
        assert result.returncode == 1, result
        assert "MSIME runtime options not found" in result.stderr, result.stderr
        wait_for(lambda: len(calls(log, "settings")) == 1 and len(calls(log, "notify")) == 1,
                 "settings window and notification were not both launched once")
        assert calls(log, "settings") == ["settings "], calls(log, "settings")
        assert "尚未完成首次配置" in calls(log, "notify")[0], calls(log, "notify")
        # IBus 组件已经退出，恢复步骤是切走再切回，必要时 ibus restart。
        assert "切换到其他输入法" in calls(log, "notify")[0] and "ibus restart" in calls(log, "notify")[0], calls(log, "notify")
        # 切回时要找的名字就是 IBus 输入源列表里显示的 longname：组件 XML 与 msime-linux-ibus 自报的描述都得是这一个。
        longname = re.search(r"<longname>([^<]+)</longname>", (ROOT / "data/msime-linux.xml.in").read_text()).group(1)
        assert longname == "Metasequoia 水杉输入法", longname
        # msime-linux-ibus 自报的显示名是 LinuxEdition.h 里本版本的 MSIME_EDITION_IBUS_LONGNAME，full 的那一个就是这个 longname。
        assert "MSIME_EDITION_IBUS_LONGNAME" in (ROOT / "src/entrypoints/ibus_main.cpp").read_text()
        full_longname = re.search(r'#if defined\(MSIME_EDITION_FULL\).*?#define MSIME_EDITION_IBUS_LONGNAME ("[^"]*")', (ROOT / "src/core/LinuxEdition.h").read_text(), re.S).group(1)
        assert full_longname.encode().decode("unicode_escape").encode("latin-1").decode() == f'"{longname}"', full_longname
        assert f"切回「{longname}」" in calls(log, "notify")[0], calls(log, "notify")
        assert calls(log, "ibus") == []
        # 状态目录必须仍不存在：msime-linux-setup 拒绝准备一个已存在的目录。
        assert not (config_home / "msime-client").exists()

        # ibus-daemon 每次选中都会再起一次启动器；同一登录会话内不再弹窗、不再通知。
        for _ in range(3):
            assert launch(graphical).returncode == 1
        settle()
        assert len(calls(log, "settings")) == 1 and len(calls(log, "notify")) == 1, log.read_text()

        # 会话目录里的记录不按时间过期：用户关掉窗口继续打字，过多久都不会再被打断。
        stamp = runtime / "msime-client/first-run-guide.stamp"
        stamp.write_text(f"{int(time.time()) - 86400} 1\n")
        assert launch(graphical).returncode == 1
        settle()
        assert len(calls(log, "settings")) == 1 and len(calls(log, "notify")) == 1, log.read_text()

        # Fcitx5 下次按键或聚焦就会重读配置，通知不要求重新选择输入法。
        stamp.unlink()
        guide = [str(bin_dir / "msime-linux-first-run-guide"), "--host", "fcitx5"]
        assert subprocess.run(guide, env=graphical, timeout=10).returncode == 0
        wait_for(lambda: len(calls(log, "settings")) == 2 and len(calls(log, "notify")) == 2,
                 "Fcitx5 guidance was not launched")
        assert "即可直接输入" in calls(log, "notify")[-1], calls(log, "notify")
        assert "重新选择" not in calls(log, "notify")[-1] and "ibus" not in calls(log, "notify")[-1], calls(log, "notify")

        # 两个宿主同时调用时，同一会话里只有一个能引导。
        stamp.unlink()
        racers = [subprocess.Popen(guide, env=graphical) for _ in range(8)]
        assert all(racer.wait(timeout=10) == 0 for racer in racers)
        wait_for(lambda: len(calls(log, "settings")) == 3, "no racing caller launched the guidance")
        settle()
        assert len(calls(log, "settings")) == 3 and len(calls(log, "notify")) == 3, log.read_text()

        # 显式指定的配置无效是配置错误，不是首次使用：照旧报错退出，不引导。
        stamp.unlink()
        result = launch(dict(graphical, MSIME_IBUS_OPTIONS=str(scratch / "missing.json")))
        assert result.returncode == 1 and "not found" in result.stderr, result
        settle()
        assert len(calls(log, "settings")) == 3, log.read_text()
        assert not stamp.exists()

        # 用户配置是悬空符号链接同样属于配置损坏。
        (config_home / "msime-client").mkdir(parents=True)
        (config_home / "msime-client/runtime-options.json").symlink_to(scratch / "nowhere.json")
        result = launch(graphical)
        assert result.returncode == 1, result
        settle()
        assert len(calls(log, "settings")) == 3, log.read_text()
        (config_home / "msime-client/runtime-options.json").unlink()
        (config_home / "msime-client").rmdir()

        # 没装设置窗口（只装输入法的最小安装）：仍然通知，改为指向终端命令。
        (bin_dir / "msime-linux-settings").unlink()
        # 引导脚本只在没有 MSIME_CLIENT_SETTINGS_COMMAND 时才去 PATH 里找设置启动器，这里把它指向
        # 一个不存在的文件，所以测试机上装没装 msime-linux 都不影响结果，PATH 保持原样：
        # 此前为避开已装的启动器把它所在的 /usr/bin 和 /bin 整个去掉，脚本连 mkdir、date 都找不到。
        environment = dict(
            graphical,
            MSIME_CLIENT_SETTINGS_COMMAND=str(scratch / "missing-settings"),
        )
        assert launch(environment).returncode == 1
        wait_for(lambda: any("msime-linux-setup" in line for line in calls(log, "notify")), "notification missing without a settings window")
        assert any("msime-linux-setup" in line for line in calls(log, "notify")), calls(log, "notify")
        assert len(calls(log, "settings")) == 3
        stub(bin_dir / "msime-linux-settings", log, "settings")

        # 没有 XDG_RUNTIME_DIR 时记录落在缓存目录，缓存跨会话保留，所以只在冷却期内压住重复引导。
        stamp.unlink()
        no_runtime = {key: value for key, value in graphical.items() if key != "XDG_RUNTIME_DIR"}
        no_runtime["XDG_CACHE_HOME"] = str(scratch / "cache")
        for _ in range(2):
            assert launch(no_runtime).returncode == 1
        wait_for(lambda: len(calls(log, "settings")) == 4, "guidance missing without XDG_RUNTIME_DIR")
        settle()
        assert len(calls(log, "settings")) == 4, log.read_text()
        cache_stamp = scratch / "cache/msime-client/first-run-guide.stamp"
        assert cache_stamp.exists()

        # 冷却期过去之后再引导一次。
        cache_stamp.write_text(f"{int(time.time()) - 301} 1\n")
        assert launch(no_runtime).returncode == 1
        wait_for(lambda: len(calls(log, "settings")) == 5, "guidance did not return after the cooldown")

        # 时钟回拨留下的「未来」时间戳不能把引导永久压住。
        cache_stamp.write_text(f"{int(time.time()) + 3600} 1\n")
        assert launch(no_runtime).returncode == 1
        wait_for(lambda: len(calls(log, "settings")) == 6, "a future stamp suppressed the guidance")

        # 配置已就绪：直接交给 msime-linux-ibus，不引导。
        system_options.parent.mkdir(parents=True)
        system_options.write_text("{}")
        try:
            (runtime / "msime-client/first-run-guide.stamp").unlink()
        except FileNotFoundError:
            pass
        result = launch(graphical)
        assert result.returncode == 0, result
        wait_for(lambda: len(calls(log, "ibus")) == 1, "prepared configuration did not reach the engine")
        assert calls(log, "ibus") == [f"ibus {system_options}"], calls(log, "ibus")
        settle()
        assert len(calls(log, "settings")) == 6, log.read_text()

        # 升级后下载的词库落后于当前版本：宿主以 --reason dictionary-outdated 调用，只发通知、不开设置窗口，通知里给出取回新词库的命令。
        outdated = [str(bin_dir / "msime-linux-first-run-guide"), "--reason", "dictionary-outdated"]
        outdated_stamp = runtime / "msime-client/dictionary-outdated.stamp"
        first_run_stamp = runtime / "msime-client/first-run-guide.stamp"
        notified = len(calls(log, "notify"))

        # 没有图形会话：不通知，也不写标记。
        assert subprocess.run(outdated, env=base, timeout=10).returncode == 0
        settle()
        assert len(calls(log, "notify")) == notified and not outdated_stamp.exists(), log.read_text()

        # 首次配置的标记已在：词库过期的通知用自己的标记，照样发出。
        first_run_stamp.write_text(f"{int(time.time())} 1\n")
        assert subprocess.run(outdated, env=graphical, timeout=10).returncode == 0
        wait_for(lambda: len(calls(log, "notify")) == notified + 1, "outdated dictionary notification missing")
        message = calls(log, "notify")[-1]
        assert "词库需要更新" in message and "msime-linux-setup --update --download" in message, message
        assert "尚未完成首次配置" not in message, message
        assert outdated_stamp.exists()
        settle()
        assert len(calls(log, "settings")) == 6, log.read_text()

        # 宿主参数放在原因之前或之后都一样被接受。每种顺序都先清掉词库过期的标记、留着首次配置的标记：原因若被丢掉，脚本会走首次配置那一支并因标记已在而静默退出，所以必须各自发出一条词库过期的通知。
        orders = ([*outdated, "--host", "ibus"], [outdated[0], "--host", "fcitx5", *outdated[1:]])
        for arguments in orders:
            outdated_stamp.unlink()
            assert first_run_stamp.exists()
            count = len(calls(log, "notify"))
            assert subprocess.run(arguments, env=graphical, timeout=10).returncode == 0, arguments
            wait_for(lambda: len(calls(log, "notify")) == count + 1, f"outdated dictionary notification missing for {arguments}")
            message = calls(log, "notify")[-1]
            assert "msime-linux-setup --update --download" in message and "尚未完成首次配置" not in message, message
        notified += len(orders)
        settle()
        assert len(calls(log, "settings")) == 6, log.read_text()

        # 同一登录会话内不再通知，无论参数顺序。
        for arguments in (outdated, *orders):
            assert subprocess.run(arguments, env=graphical, timeout=10).returncode == 0
        settle()
        assert len(calls(log, "notify")) == notified + 1, log.read_text()

        # 反过来，词库过期的标记也不占用首次配置的引导。
        first_run_stamp.unlink()
        assert subprocess.run(guide, env=graphical, timeout=10).returncode == 0
        wait_for(lambda: len(calls(log, "settings")) == 7 and len(calls(log, "notify")) == notified + 2,
                 "first-run guidance was suppressed by the outdated dictionary stamp")
        assert "尚未完成首次配置" in calls(log, "notify")[-1], calls(log, "notify")

        # 不认识的原因是调用方的错误：不通知、不写标记。
        outdated_stamp.unlink()
        first_run_stamp.unlink()
        result = subprocess.run([outdated[0], "--reason", "unknown"], env=graphical, timeout=10)
        assert result.returncode == 2, result
        settle()
        assert len(calls(log, "notify")) == notified + 2 and not outdated_stamp.exists() and not first_run_stamp.exists()

    # 引导脚本本身不发起任何下载：msime-linux-setup 和 --download 只出现在注释和这几条通知文案里，从不作为命令执行。文案按原样去掉，剩下的非注释行里一处都不许有。
    guide = (SCRIPTS / "msime-linux-first-run-guide").read_text()
    for forbidden in ("curl", "wget"):
        assert forbidden not in guide, forbidden
    for body in (
        '"已安装的词库早于当前版本，输入法暂时继续使用旧词库。在终端运行 msime-linux-setup --update --download 取回新词库并切换，用户词库会一并迁移。"',
        'body="请在终端运行 msime-linux-setup 完成首次配置。$resume"',
    ):
        assert guide.count(body) == 1, body
        guide = guide.replace(body, "")
    for line in guide.splitlines():
        if not line.strip().startswith("#"):
            assert "msime-linux-setup" not in line and "--download" not in line, line

    print("first-run guide tests passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""msime-linux-ibus-launcher 的崩溃守护：宿主崩溃后按退避重启，升级退出立即重启且不影响退避，维护退出、总线断开、配置失效、宿主程序已被删除和停止请求都不重启。

用桩代替 `msime-linux-ibus`，按预设剧本逐次崩溃或退出；记录真实启动器的等待请求和启动参数，按就绪状态发送信号。不需要 IBus、D-Bus 或词库。
"""
import json
import os
import signal
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LAUNCHER = ROOT / "scripts/msime-linux-ibus-launcher"
STOP_STATUS = 77
UPGRADED_STATUS = 78

STUB = r'''#!{python}
import ctypes, json, os, signal, sys, time
from pathlib import Path
scratch = Path({scratch!r})
log = scratch / "runs.log"
runs = log.read_text().splitlines() if log.exists() else []
plan = json.loads((scratch / "plan.json").read_text())
action = plan[len(runs)] if len(runs) < len(plan) else "exit {stop_status}"
if action.startswith("delayed-exit "):
    # 故意模拟宿主迟到，监督器请求的退避不应把启动调度延迟算进去。
    time.sleep(2)
    action = "exit " + action.split()[1]
with log.open("a") as output:
    output.write(json.dumps({{"pid": os.getpid(), "args": sys.argv[1:], "time": time.monotonic()}}) + "\n")
def terminated(signum, frame):
    with (scratch / "signals.log").open("a") as output:
        output.write(f"{{signum}}\n")
    sys.exit(143)
signal.signal(signal.SIGTERM, terminated)
# A SIGSEGV would otherwise dump core, and on CI runners core_pattern pipes to a host handler (apport, systemd-coredump) that the kernel waits for before the process is reaped, adding seconds to the measured backoff. A non-dumpable process skips the dump entirely; RLIMIT_CORE=0 would not, because piped core_pattern ignores it. PR_SET_DUMPABLE is 4.
if ctypes.CDLL(None, use_errno=True).prctl(4, 0, 0, 0, 0) != 0:
    raise OSError(ctypes.get_errno(), "prctl(PR_SET_DUMPABLE, 0) failed")
if action == "segv":
    os.kill(os.getpid(), signal.SIGSEGV)
elif action == "kill":
    os.kill(os.getpid(), signal.SIGKILL)
elif action == "unlink-config":
    Path(sys.argv[-1]).unlink()
    os.kill(os.getpid(), signal.SIGSEGV)
elif action == "unlink-host":
    Path(sys.argv[0]).unlink()
    os.kill(os.getpid(), signal.SIGSEGV)
elif action == "chmod-host":
    Path(sys.argv[0]).chmod(0o644)
    os.kill(os.getpid(), signal.SIGSEGV)
elif action == "hang":
    (scratch / "ready").write_text("ready")
    signal.pause()
elif action.startswith("exit "):
    sys.exit(int(action.split()[1]))
raise AssertionError(action)
'''

SLEEP = r'''#!{python}
import json, signal, sys
from pathlib import Path
scratch = Path({scratch!r})
def terminated(signum, frame):
    with (scratch / "pause-signals.log").open("a") as output:
        output.write(f"{{signum}}\n")
    sys.exit(143)
signal.signal(signal.SIGTERM, terminated)
runs = (scratch / "runs.log").read_text().splitlines()
with (scratch / "sleeps.log").open("a") as output:
    output.write(json.dumps({{"args": sys.argv[1:], "run": len(runs)}}) + "\n")
if (scratch / "block-sleep").exists():
    signal.pause()
'''


class Fixture:
    def __init__(self, scratch: Path):
        self.scratch = scratch
        self.bin_dir = scratch / "bin"
        self.bin_dir.mkdir()
        launcher = self.bin_dir / "msime-linux-ibus-launcher"
        launcher.write_text(LAUNCHER.read_text())
        launcher.chmod(0o755)
        pause = self.bin_dir / "sleep"
        pause.write_text(SLEEP.format(python=sys.executable, scratch=str(scratch)))
        pause.chmod(0o755)
        clock = self.bin_dir / "date"
        # 剧本中的宿主都是短命进程；固定逻辑时钟，避免调度迟到被当成健康运行满 30 秒。
        clock.write_text(
            '#!/bin/sh\n'
            '[ "$#" -eq 1 ] && [ "$1" = +%s ] || exit 2\n'
            "printf '100\\n'\n"
        )
        clock.chmod(0o755)
        self.launchers = []
        self.options = scratch / "options/runtime-options.json"
        self.options.parent.mkdir()
        self.environment = {
            key: value for key, value in os.environ.items() if not key.startswith("MSIME_")
        }
        self.environment.update(HOME=str(scratch / "home"), XDG_CONFIG_HOME=str(scratch / "config"))
        self.environment["PATH"] = (
            str(self.bin_dir) + os.pathsep + self.environment.get("PATH", os.defpath)
        )

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_value, traceback):
        # 断言或超时失败也清理本夹具的进程组，不能留下等待进程污染后续门禁。
        for launcher in self.launchers:
            try:
                os.killpg(launcher.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            launcher.communicate(timeout=10)

    def reset(self, plan: list, *, block_sleep: bool = False) -> None:
        # 每轮重建，因为有的剧本会删宿主或清除执行权限。
        stub = self.bin_dir / "msime-linux-ibus"
        stub.write_text(STUB.format(
            python=sys.executable, scratch=str(self.scratch), stop_status=STOP_STATUS,
        ))
        stub.chmod(0o755)
        for name in ("runs.log", "signals.log", "sleeps.log", "pause-signals.log", "ready", "block-sleep"):
            try:
                (self.scratch / name).unlink()
            except FileNotFoundError:
                pass
        (self.scratch / "plan.json").write_text(json.dumps(plan))
        self.options.write_text("{}")
        if block_sleep:
            (self.scratch / "block-sleep").touch()

    def start(self) -> subprocess.Popen:
        launcher = subprocess.Popen(
            [str(self.bin_dir / "msime-linux-ibus-launcher"), str(self.options)],
            env=self.environment, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True,
            start_new_session=True,
        )
        self.launchers.append(launcher)
        return launcher

    def records(self, name: str) -> list:
        log = self.scratch / name
        # 就绪轮询可能碰到正在追加的一行，完整换行出现后才读取该条记录。
        if not log.exists():
            return []
        return [
            json.loads(line)
            for line in log.read_text().splitlines(keepends=True)
            if line.endswith("\n")
        ]

    def runs(self) -> list:
        return self.records("runs.log")

    def delays(self) -> list:
        return [(row["args"], row["run"]) for row in self.records("sleeps.log")]

    def signals(self) -> list:
        log = self.scratch / "signals.log"
        return [int(line) for line in log.read_text().splitlines()] if log.exists() else []

    def pause_signals(self) -> list:
        log = self.scratch / "pause-signals.log"
        return [int(line) for line in log.read_text().splitlines()] if log.exists() else []


def wait_for(predicate, message: str, timeout: float = 10) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(0.02)
    raise AssertionError(message)


def main() -> int:
    with tempfile.TemporaryDirectory() as name, Fixture(Path(name)) as fixture:
        options = str(fixture.options)

        # 崩溃与非零退出仍请求 2、4、8 秒退避；最后一个宿主故意迟到 2 秒，调度延迟不改变请求值。重启带 `--recovered`，`exit 0` 停止守护。
        fixture.reset(["segv", "kill", "exit 1", "delayed-exit 0"])
        launcher = fixture.start()
        _, errors = launcher.communicate(timeout=30)
        assert launcher.returncode == 0, (launcher.returncode, errors)
        runs = fixture.runs()
        assert [run["args"] for run in runs] == [[options]] + [["--recovered", options]] * 3, runs
        assert fixture.delays() == [(["2"], 1), (["4"], 2), (["8"], 3)], fixture.delays()
        assert errors.count("restarting in") == 3, errors
        assert "restarting in 2s" in errors and "restarting in 8s" in errors, errors

        # 退避期间 ibus-daemon 被 SIGKILL 或崩溃、没来得及停止守护：重启的宿主连不上总线，以 0 退出。守护必须就此结束，不能成为孤儿每 30 秒重试，更不能在之后启动的新 daemon 上重复注册 component。
        fixture.reset(["segv", "exit 0"])
        launcher = fixture.start()
        _, errors = launcher.communicate(timeout=10)
        assert launcher.returncode == 0, (launcher.returncode, errors)
        runs = fixture.runs()
        assert [run["args"] for run in runs] == [[options], ["--recovered", options]], runs
        assert fixture.delays() == [(["2"], 1)], fixture.delays()
        assert errors.count("restarting in") == 1, errors

        # 升级退出：宿主发现自己的程序被升级替换后以 78 退出，守护立即带 --recovered 启动新版本，不等待、不打印退避；退避状态保持原样，所以前后两次崩溃仍按 2、4 秒，既不因升级清零，也不把升级算作第三次崩溃（那样会是 8 秒）。
        fixture.reset(["segv", f"exit {UPGRADED_STATUS}", "segv", "exit 0"])
        launcher = fixture.start()
        _, errors = launcher.communicate(timeout=30)
        assert launcher.returncode == 0, (launcher.returncode, errors)
        runs = fixture.runs()
        assert [run["args"] for run in runs] == [[options]] + [["--recovered", options]] * 3, runs
        assert fixture.delays() == [(["2"], 1), (["4"], 3)], fixture.delays()
        assert errors.count("restarting in") == 2, errors
        assert "restarting in 2s" in errors and "restarting in 4s" in errors, errors
        assert errors.count("replaced by an upgrade") == 1, errors

        # 第一次启动就是升级退出（没有先崩溃）同样立即重启。
        fixture.reset([f"exit {UPGRADED_STATUS}", "exit 0"])
        launcher = fixture.start()
        _, errors = launcher.communicate(timeout=10)
        assert launcher.returncode == 0, (launcher.returncode, errors)
        runs = fixture.runs()
        assert [run["args"] for run in runs] == [[options], ["--recovered", options]], runs
        assert fixture.delays() == [], fixture.delays()
        assert "restarting in" not in errors, errors

        # 维护退出（Ctrl+Shift+Alt+T）是有意停止：原样退出，不重启。
        fixture.reset([f"exit {STOP_STATUS}", "exit 0"])
        launcher = fixture.start()
        _, errors = launcher.communicate(timeout=10)
        assert launcher.returncode == STOP_STATUS, (launcher.returncode, errors)
        assert len(fixture.runs()) == 1, fixture.runs()
        assert fixture.delays() == [], fixture.delays()
        assert "restarting" not in errors, errors

        # 配置在崩溃后读不到了：重启也救不回来，报错退出。
        fixture.reset(["unlink-config", "exit 0"])
        launcher = fixture.start()
        _, errors = launcher.communicate(timeout=10)
        assert launcher.returncode == 1, (launcher.returncode, errors)
        assert "no longer readable" in errors, errors
        assert len(fixture.runs()) == 1, fixture.runs()
        assert fixture.delays() == [(["2"], 1)], fixture.delays()

        # apt remove 之后仍在运行的宿主崩溃了：程序已被删除（或不再可执行），重启只会得到 127/126。守护立即报错退出，不进入退避、不每 30 秒对着缺失的文件重试。
        for action in ("unlink-host", "chmod-host"):
            fixture.reset([action, "exit 0"])
            launcher = fixture.start()
            _, errors = launcher.communicate(timeout=10)
            assert launcher.returncode == 1, (action, launcher.returncode, errors)
            assert "no longer installed" in errors, (action, errors)
            assert "restarting in" not in errors, (action, errors)
            assert fixture.delays() == [], (action, fixture.delays())
            assert len(fixture.runs()) == 1, (action, fixture.runs())

        # ibus-daemon 用 SIGTERM 停组件；SIGINT、SIGHUP 同样当作停止请求。守护把它作为 SIGTERM 转给宿主，宿主退出后不再重启。
        for request in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP):
            fixture.reset(["hang", "exit 0"])
            launcher = fixture.start()
            wait_for(lambda: (fixture.scratch / "ready").exists(), "host signal handler was not ready")
            launcher.send_signal(request)
            _, errors = launcher.communicate(timeout=10)
            assert fixture.signals() == [signal.SIGTERM], (request, fixture.signals())
            assert launcher.returncode == 143, (request, launcher.returncode, errors)
            assert len(fixture.runs()) == 1, (request, fixture.runs())
            assert fixture.delays() == [], (request, fixture.delays())

        # 等待进程保持阻塞直到收到信号；不转发停止请求就无法自行结束，必须由就绪状态触发停止。
        fixture.reset(["segv", "exit 0"], block_sleep=True)
        launcher = fixture.start()
        wait_for(lambda: fixture.delays() == [(["2"], 1)], "backoff signal handler was not ready")
        launcher.terminate()
        launcher.communicate(timeout=10)
        assert fixture.pause_signals() == [signal.SIGTERM], fixture.pause_signals()
        assert len(fixture.runs()) == 1, fixture.runs()

    print("IBus launcher supervisor tests passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())

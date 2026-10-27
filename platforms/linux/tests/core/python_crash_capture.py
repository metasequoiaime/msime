#!/usr/bin/env python3
"""真实 Python 产品子进程的原生故障、未捕获异常、同意变化和独立补报回归。"""

import argparse
import array
import fcntl
import json
import os
from pathlib import Path
import resource
import runpy
import shutil
import signal
import subprocess
import sys
import tempfile
import termios
import time

from auxiliary_crash_capture import Fixture, wait_for


class PythonFixture(Fixture):
    def __init__(self, root, settings, installation):
        super().__init__(root, settings)
        self.installation = installation
        self.runtime = root / "runtime"
        self.runtime.mkdir(mode=0o700)
        self.environment.update(PYTHONDONTWRITEBYTECODE="1", WAYLAND_DISPLAY="synthetic-wayland")
        self.environment.pop("DISPLAY", None)
        self.environment["PATH"] = str(self.runtime) + ":" + os.environ["PATH"]
        self.ready = self.runtime / "ready"
        watcher = self.runtime / "wl-paste"
        watcher.write_text("#!" + sys.executable + "\nfrom pathlib import Path\nimport os, time\n"
                           "Path(%r).write_text(str(os.getpid()))\ntime.sleep(60)\n" % str(self.ready))
        watcher.chmod(0o700)

    def command(self, component):
        if component.startswith("online"):
            script = self.installation / "msime-linux-online-provider"
            arguments = (["--http-request"] if component.endswith("http") else
                         [str(self.runtime / "provider.sock"), "--config-directory", str(self.preferences)])
        elif component.startswith("voice"):
            script = self.installation / "msime-linux-voice-provider"
            arguments = (["--http-request"] if component.endswith("http") else
                         [str(self.runtime / "provider.sock"), "--config", str(self.preferences / "voice-provider.json")])
        else:
            script = self.installation / "msime-linux-clipboard-monitor"
            arguments = [str(self.options)]
            if component == "clipboard-event":
                arguments.append("--capture-event")
        self.environment["MSIME_CRASH_PARENT_PREFERENCES"] = str(self.preferences) if component == "online-http" else ""
        self.environment["MSIME_CRASH_PARENT_OPTIONS"] = ""
        return [sys.executable, str(script), *arguments]

    def check_records(self, component, expected, version, python_exception=False):
        records = self.records()
        assert len(records) == int(expected), (component, len(records), expected)
        if expected:
            raw = records[0].read_text()
            assert component in raw and version in raw, raw
            assert "synthetic-private" not in raw and str(self.root) not in raw, raw
            if python_exception:
                assert "Python unhandled exception" in raw
                assert "msime-linux-clipboard-monitor!main:" in raw, raw
            self.control("collect")
            events = self.events()
            crashes = [event for event in events if event["kind"] == "crash"]
            assert len(crashes) == 1 and crashes[0]["version"] == version
            assert component in crashes[0]["message"]
            assert not [event for event in events if event["kind"] in ("session", "session_crash")]
            assert not self.records()

    def crash(self, component, expected, version, change=None):
        command = self.command(component)
        input_read, input_write = os.pipe()
        process = subprocess.Popen(command, stdin=input_read, stdout=subprocess.DEVNULL,
                                   stderr=subprocess.DEVNULL, env=self.environment, start_new_session=True)
        try:
            if component.endswith("http") or component == "clipboard-event":
                os.write(input_write, b" ")
                def consumed():
                    assert process.poll() is None, (component, process.returncode)
                    available = array.array("i", [0])
                    fcntl.ioctl(input_read, termios.FIONREAD, available, True)
                    return available[0] == 0
                wait_for(consumed, "Python 子进程未读取请求")
            else:
                def ready():
                    assert process.poll() is None, (component, process.returncode)
                    return self.ready.exists() if component == "clipboard-monitor" else (self.runtime / "provider.sock").exists()
                wait_for(ready, "Python 服务未就绪")
            if change:
                change()
                # 与生产观察器的 500 ms 周期留出调度余量；随后实际致命故障验证最终状态。
                time.sleep(1.2)
            process.send_signal(signal.SIGABRT)
            assert process.wait(timeout=15) == -signal.SIGABRT
            self.check_records(component, expected, version)
        finally:
            if process.poll() is None:
                process.kill()
            process.wait(timeout=10)
            # monitor 的假 watcher 是外部工具，采集进程被杀后也必须由夹具回收。
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            if self.ready.exists():
                try:
                    os.killpg(int(self.ready.read_text()), signal.SIGKILL)
                except ProcessLookupError:
                    pass
            os.close(input_read)
            os.close(input_write)

    def python_exception(self, expected, version):
        command = self.command("clipboard-monitor")
        # 让真实 main 在创建监听错误管道处失败，异常含合成私密文本；不修改产品源码。
        injection = """
import runpy, sys, tempfile
def fail(*args, **kwargs):
    raise RuntimeError('synthetic-private-exception-value')
tempfile.TemporaryFile = fail
sys.path.insert(0, sys.argv[1])
sys.argv = sys.argv[2:]
runpy.run_path(sys.argv[0], run_name='__main__')
"""
        process = subprocess.run([sys.executable, "-c", injection, str(self.installation), *command[1:]],
                                 env=self.environment, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                 timeout=15)
        assert process.returncode == 1
        self.check_records("clipboard-monitor", expected, version, python_exception=True)


def main():
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    parser = argparse.ArgumentParser()
    parser.add_argument("--controller", required=True)
    parser.add_argument("--bridge", required=True, type=Path)
    parser.add_argument("--paths", required=True, type=Path)
    parser.add_argument("--host-library", type=Path)
    parser.add_argument("--scripts", required=True, type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--client-directory", default="msime-client")
    parser.add_argument("--telemetry-directory", default="msime")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="msime-python-crash-") as temporary:
        root = Path(temporary).resolve()
        installation = root / "product" / "bin"
        installation.mkdir(parents=True)
        for path in args.scripts.iterdir():
            if path.suffix == ".py" or path.name in ("msime-linux-online-provider", "msime-linux-voice-provider", "msime-linux-clipboard-monitor"):
                shutil.copyfile(path, installation / path.name)
        shutil.copyfile(args.paths, installation / "msime_voice_paths.py")
        paths = runpy.run_path(str(args.paths))
        bridge = installation / paths["CRASH_LIBRARY"]
        bridge.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(args.bridge, bridge)
        host = args.host_library or args.paths.parent / paths["HOST_LIBRARY"]
        shutil.copyfile(host, bridge.parent / Path(paths["HOST_LIBRARY"]).name)
        components = ("online-provider", "voice-provider", "clipboard-monitor", "online-http", "voice-http", "clipboard-event")
        for component in components:
            for enabled in (True, False):
                fixture = PythonFixture(root / (component + str(enabled)), args, installation)
                fixture.control("enable" if enabled else "disable")
                # 剪贴板监听已开，才能观察到真实 wl-paste 就绪；不读或存入任何真实剪贴板。
                preferences = fixture.preferences / "preferences.json"
                snapshot = json.loads(preferences.read_text())
                snapshot["preferences"]["clipboard_history"] = True
                preferences.write_text(json.dumps(snapshot))
                fixture.crash(component, enabled, args.version)
        for enabled in (False, True):
            fixture = PythonFixture(root / ("exception" + str(enabled)), args, installation)
            fixture.control("enable" if enabled else "disable")
            fixture.python_exception(enabled, args.version)
        for mode in ("disable", "enable", "invalid", "redirect"):
            fixture = PythonFixture(root / mode, args, installation)
            fixture.control("disable" if mode == "enable" else "enable")
            def change():
                if mode in ("disable", "enable"):
                    fixture.control(mode)
                elif mode == "invalid":
                    fixture.options.write_text("{synthetic-private-invalid")
                else:
                    other = fixture.root / "disabled-preferences"
                    fixture.control("disable", other)
                    fixture.options.write_text(json.dumps({"preferences_directory": str(other)}))
            fixture.crash("voice-http", mode == "enable", args.version, change)
        fixture = PythonFixture(root / "online-parent-preferences", args, installation)
        fixture.control("enable")
        other = fixture.root / "disabled-preferences"
        fixture.control("disable", other)
        fixture.options.write_text(json.dumps({"preferences_directory": str(other)}))
        fixture.crash("online-http", True, args.version)
        for component in ("online-http", "voice-http"):
            fixture = PythonFixture(root / (component + "-ordinary-error"), args, installation)
            fixture.control("enable")
            result = subprocess.run(fixture.command(component), input=b"{}\n", env=fixture.environment,
                                    stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=10)
            assert result.returncode == 1
            assert not fixture.records(), "普通请求拒绝不能成为崩溃"
        for script in ("msime-linux-online-provider", "msime-linux-voice-provider", "msime-linux-clipboard-monitor"):
            home = root / (script + "-help")
            result = subprocess.run([sys.executable, str(installation / script), "--help"],
                                    env=dict(os.environ, HOME=str(home), XDG_STATE_HOME=str(home / "state")),
                                    stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=10)
            assert result.returncode == 0 and not home.exists()
    print("Linux Python 六个入口：致命信号、异常脱敏、动态同意、原版本补报及无输入会话通过")


if __name__ == "__main__":
    main()

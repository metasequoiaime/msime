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
import select
import shutil
import signal
import socket
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
        self.client_directory = settings.client_directory
        edition = runpy.run_path(str(installation / "msime_voice_paths.py"))["EDITION"]
        self.setup_name = "msime-linux-setup" if edition == "full" else "msime-linux-%s-setup" % edition
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
            assert len(crashes) == 1 and crashes[0]["version"] == version, (component, crashes, version)
            assert component in crashes[0]["message"]
            assert not [event for event in events if event["kind"] in ("session", "session_crash")]
            assert not self.records()

    def crash(self, component, expected, version, change=None):
        command = self.command(component)
        input_read, input_write = os.pipe()
        state, child_state = socket.socketpair()
        injection = """
import runpy, socket, sys, threading
state = socket.socket(fileno=int(sys.argv[1]))
sys.path.insert(0, sys.argv[2])
sys.argv = sys.argv[3:]
def report_state():
    while state.recv(1):
        capture = getattr(sys.modules.get("msime_crash_capture"), "_capture", None)
        armed = capture is not None and capture.library.msime_python_crash_is_armed() != 0
        state.sendall(b"1" if armed else b"0")
threading.Thread(target=report_state, daemon=True).start()
runpy.run_path(sys.argv[0], run_name="__main__")
"""
        process = subprocess.Popen([sys.executable, "-c", injection, str(child_state.fileno()),
                                    str(self.installation), *command[1:]],
                                   stdin=input_read, stdout=subprocess.DEVNULL,
                                   stderr=subprocess.DEVNULL, env=self.environment, start_new_session=True,
                                   pass_fds=(child_state.fileno(),))
        child_state.close()
        state.settimeout(10)
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
                # 直接读取真实记录器的武装状态，保留原有十秒期限。
                def observed():
                    assert process.poll() is None, (component, "exited before consent refresh")
                    state.sendall(b"?")
                    return state.recv(1) == (b"1" if expected else b"0")
                wait_for(observed, "Python 子进程未完成实际同意刷新：%s" % component)
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
            state.close()
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

    def configuration(self, component, expected, version, fault="native", change=None, explicit=False, query=None, provider="online"):
        script = self.installation / (self.setup_name if component == "setup" else "msime-linux-provider-session")
        if component == "setup":
            arguments = ["--update", "--state", str(self.options.parent)] if explicit else ["--register"]
            pause = "read_dictionary_lock" if explicit else "use_runtime_session_bus"
            if query:
                arguments = [query]
                pause = "input_method_status" if query == "--status" else "read_dictionary_lock"
        else:
            arguments, pause = [provider], None
            if provider == "online":
                # 在线服务的启动器和子进程同样使用 XDG 配置目录里的偏好。
                shutil.copyfile(self.preferences / "preferences.json", self.options.parent / "preferences.json")
        # 收集必须使用产品实际读取的偏好目录，不能由另一个目录的当前开关追认故障。
        self.collection_preferences = (self.options.parent if component == "provider-session" and provider == "online"
                                       else Path(json.loads(self.options.read_text())["preferences_directory"]))
        self.environment["XDG_RUNTIME_DIR"] = str(self.runtime)
        injection = """
import pathlib, runpy, sys
sys.path.insert(0, str(pathlib.Path(sys.argv[1]).parent))
sys.argv = [sys.argv[1], *sys.argv[3:]]
namespace = runpy.run_path(sys.argv[0], run_name="synthetic_configuration")
def pause(*args, **kwargs):
    print("ready", flush=True)
    while True:
        command = sys.stdin.buffer.read(1)
        if command == b"?":
            capture = getattr(sys.modules.get("msime_crash_capture"), "_capture", None)
            armed = capture is not None and capture.library.msime_python_crash_is_armed() != 0
            print("armed" if armed else "disarmed", flush=True)
        else:
            raise FAULT("synthetic-private-exception")
"""
        injection = injection.replace("FAULT", "SystemExit" if fault == "handled" else "RuntimeError")
        injection += ("namespace['main'].__globals__[%r] = pause\n" % pause if pause else
                      "pathlib.Path.mkdir = pause\n")
        injection += "namespace['main']()\n"
        process = subprocess.Popen([sys.executable, "-c", injection, str(script), "fixture", *arguments],
                                   stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                                   env=self.environment)
        try:
            # 暂停点在真实 main 完成采集初始化之后、任何注册/下载/exec 操作之前。
            assert select.select([process.stdout], [], [], 10)[0], (component, "not ready")
            assert process.stdout.readline() == b"ready\n", (component, process.poll())
            if change:
                change()
            if fault != "handled":
                def observed():
                    assert process.poll() is None, (component, provider, "exited before consent refresh")
                    process.stdin.write(b"?")
                    process.stdin.flush()
                    assert select.select([process.stdout], [], [], 10)[0], (component, provider, "no state")
                    return process.stdout.readline() == (b"armed\n" if expected else b"disarmed\n")
                wait_for(observed, "配置进程未完成实际同意刷新：%s/%s" % (component, provider))
            if fault == "native":
                process.send_signal(signal.SIGABRT)
                assert process.wait(timeout=15) == -signal.SIGABRT
            else:
                process.stdin.write(b"x")
                process.stdin.flush()
                assert process.wait(timeout=15) == 1
            if expected and fault == "exception":
                raw = self.records()[0].read_text()
                assert "Python unhandled exception" in raw and script.name + "!main:" in raw, raw
            self.check_records(component, expected, version)
        finally:
            if process.poll() is None:
                process.kill()
            process.wait(timeout=10)
            process.stdin.close()
            process.stdout.close()

    def provider_exec(self, expected, version, provider):
        self.environment["XDG_RUNTIME_DIR"] = str(self.runtime)
        if provider == "online":
            shutil.copyfile(self.preferences / "preferences.json", self.options.parent / "preferences.json")
            self.collection_preferences = self.options.parent
        process = subprocess.Popen([sys.executable, str(self.installation / "msime-linux-provider-session"), provider],
                                   env=self.environment, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            def ready():
                assert process.poll() is None, ("provider exec", process.returncode)
                return (self.runtime / self.client_directory / (provider + ".sock")).exists()
            wait_for(ready, "启动器未 exec 到实际 provider")
            process.send_signal(signal.SIGABRT)
            assert process.wait(timeout=15) == -signal.SIGABRT
            self.check_records(provider + "-provider", expected, version)
        finally:
            if process.poll() is None:
                process.kill()
            process.wait(timeout=10)


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
        paths = runpy.run_path(str(args.paths))
        setup_name = "msime-linux-setup" if paths["EDITION"] == "full" else "msime-linux-%s-setup" % paths["EDITION"]
        for path in args.scripts.iterdir():
            if path.suffix == ".py" or path.name in ("msime-linux-online-provider", "msime-linux-voice-provider", "msime-linux-clipboard-monitor", "msime-linux-provider-session", "msime-linux-setup", setup_name):
                name = setup_name if path.name == "msime-linux-setup" else path.name
                shutil.copy2(path, installation / name)
        shutil.copyfile(args.paths, installation / "msime_voice_paths.py")
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
        for component in ("setup", "provider-session"):
            for enabled in (False, True):
                for fault in ("native", "exception", "handled"):
                    fixture = PythonFixture(root / (component + fault + str(enabled)), args, installation)
                    fixture.control("enable" if enabled else "disable")
                    fixture.configuration(component, enabled and fault != "handled", args.version, fault)
            for mode in ("disable", "enable", "invalid"):
                fixture = PythonFixture(root / (component + mode), args, installation)
                fixture.control("disable" if mode == "enable" else "enable")
                def change():
                    if mode == "invalid":
                        target = fixture.options if component == "setup" else fixture.options.parent / "preferences.json"
                        target.write_text("{synthetic-private-invalid")
                    else:
                        fixture.control(mode)
                        if component == "provider-session":
                            shutil.copyfile(fixture.preferences / "preferences.json", fixture.options.parent / "preferences.json")
                fixture.configuration(component, mode == "enable", args.version, change=change)
        fixture = PythonFixture(root / "setup-explicit-state", args, installation)
        fixture.control("enable")
        custom = fixture.root / "custom-state"
        custom.mkdir()
        custom.joinpath("runtime-options.json").write_text(fixture.options.read_text())
        # 默认位置指向关闭的偏好；--state 必须仍跟随实际配置目录并记录故障。
        other = fixture.root / "disabled-preferences"
        fixture.control("disable", other)
        fixture.options.write_text(json.dumps({"preferences_directory": str(other)}))
        fixture.options = custom / "runtime-options.json"
        fixture.configuration("setup", True, args.version, explicit=True)
        for query in ("--status", "--list-downloads"):
            fixture = PythonFixture(root / ("setup-query" + query), args, installation)
            fixture.control("enable")
            fixture.configuration("setup", False, args.version, query=query)
        for provider in ("online", "voice"):
            for enabled in (False, True):
                fixture = PythonFixture(root / (provider + "-exec" + str(enabled)), args, installation)
                fixture.control("enable" if enabled else "disable")
                fixture.provider_exec(enabled, args.version, provider)
                if provider == "voice":
                    fixture = PythonFixture(root / (provider + "-launcher" + str(enabled)), args, installation)
                    fixture.control("enable" if enabled else "disable")
                    # XDG 目录的同意与 options 指向的偏好相反，启动器必须跟随语音子进程。
                    fixture.control("disable" if enabled else "enable", fixture.options.parent)
                    fixture.configuration("provider-session", enabled, args.version, provider=provider)
        for script in ("msime-linux-online-provider", "msime-linux-voice-provider", "msime-linux-clipboard-monitor", "msime-linux-provider-session", setup_name):
            home = root / (script + "-help")
            result = subprocess.run([sys.executable, str(installation / script), "--help"],
                                    env=dict(os.environ, HOME=str(home), XDG_STATE_HOME=str(home / "state")),
                                    stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=10)
            assert result.returncode == 0 and not home.exists()
    print("Linux Python 八个入口：致命信号、异常脱敏、动态同意、原版本补报及无输入会话通过")


if __name__ == "__main__":
    main()

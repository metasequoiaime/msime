#!/usr/bin/env python3
"""对安装包的真实独立程序触发致命信号；控制器只做本地偏好与补报入队，不联网。"""

import argparse
import array
import fcntl
import json
import os
from pathlib import Path
import select
import resource
import signal
import subprocess
import tempfile
import termios
import time


def wait_for(predicate, message):
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(0.02)
    raise AssertionError(message)


class Fixture:
    def __init__(self, root, settings):
        self.root = root
        self.preferences = root / "preferences"
        self.collection_preferences = self.preferences
        self.telemetry = root / "state" / settings.telemetry_directory
        self.options = root / "config" / settings.client_directory / "runtime-options.json"
        self.options.parent.mkdir(parents=True)
        self.options.write_text(json.dumps({"preferences_directory": str(self.preferences)}))
        self.environment = dict(os.environ, HOME=str(root), XDG_CONFIG_HOME=str(root / "config"),
                                XDG_STATE_HOME=str(root / "state"))
        for name in ("MSIME_CLIENT_HOST_OPTIONS", "MSIME_IBUS_OPTIONS", "MSIME_FCITX5_OPTIONS"):
            self.environment.pop(name, None)
        self.controller = settings.controller

    def control(self, mode, preferences=None):
        selected = preferences or (self.collection_preferences if mode == "collect" else self.preferences)
        subprocess.run([self.controller, mode, str(self.telemetry),
                        str(selected), "collector"],
                       env=self.environment, check=True, timeout=15, stdout=subprocess.DEVNULL)

    def records(self):
        return list((self.telemetry / "telemetry-crashes").glob("*.crash"))

    def events(self):
        path = self.telemetry / "telemetry.json"
        return json.loads(path.read_text()) if path.exists() else []

    def crash(self, binary, name, expected, version):
        component = name[len("msime-linux-"):] if name.startswith("msime-linux-") else "voice-local"
        arguments = []
        if component in ("cloud-dictionary", "cloud-clipboard", "online", "translation", "emoji", "handwriting", "voice"):
            arguments = [str(self.root / "provider.sock")]
        elif component == "clipboard":
            arguments = [str(self.root / "history.json"), "add-stdin"]
        elif component == "clipboard-capture":
            arguments = [str(self.preferences)]
        elif component == "voice-local":
            arguments = ["--runtime", str(self.root / "missing-runtime.so")]
        input_read, input_write = os.pipe()
        process = subprocess.Popen([binary, *arguments], stdin=input_read, stdout=subprocess.PIPE,
                                   stderr=subprocess.DEVNULL, env=self.environment)
        try:
            if component in ("voice-local", "clipboard-watch-x11"):
                assert select.select([process.stdout], [], [], 10)[0], "helper did not become ready"
                line = process.stdout.readline()
                if component == "voice-local":
                    assert json.loads(line)["type"] == "hello"
                else:
                    assert line == b"\n"
            else:
                # 所有请求程序都在采集器构造之后读 stdin。确认这一个字节已被消费，避免与武装竞争。
                os.write(input_write, b" ")
                def consumed():
                    assert process.poll() is None, "helper exited before reading its request"
                    available = array.array("i", [0])
                    fcntl.ioctl(input_read, termios.FIONREAD, available, True)
                    return available[0] == 0
                wait_for(consumed, "helper did not read its request")
            process.send_signal(signal.SIGABRT)
            assert process.wait(timeout=15) == -signal.SIGABRT
            records = self.records()
            assert len(records) == int(expected), (component, len(records), expected)
            if expected:
                raw = records[0].read_text()
                assert component in raw and version in raw
                assert "synthetic-private" not in raw and str(self.root) not in raw
                self.control("collect")
                events = self.events()
                crashes = [event for event in events if event["kind"] == "crash"]
                assert len(crashes) == 1 and crashes[0]["version"] == version, (component, crashes, version)
                assert component in crashes[0]["message"]
                assert not self.records(), "queued original was not acknowledged"
                assert not [event for event in events if event["kind"] in ("session", "session_crash")]
        finally:
            if process.poll() is None:
                process.kill()
            process.wait(timeout=10)
            process.stdout.close()
            os.close(input_read)
            os.close(input_write)


def main():
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    parser = argparse.ArgumentParser()
    parser.add_argument("--controller", required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--client-directory", default="msime-client")
    parser.add_argument("--telemetry-directory", default="msime")
    parser.add_argument("--binary", nargs=2, action="append", required=True)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="msime-linux-auxiliary-crash-") as temporary:
        root = Path(temporary).resolve()
        for name, binary in args.binary:
            for enabled in (False, True):
                fixture = Fixture(root / (name + str(enabled)), args)
                fixture.control("enable" if enabled else "disable")
                fixture.crash(binary, name, enabled, args.version)
            help_root = root / (name + "-help")
            subprocess.run([binary, "--help"], env=dict(os.environ, HOME=str(help_root),
                           XDG_CONFIG_HOME=str(help_root / "config"), XDG_STATE_HOME=str(help_root / "state")),
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=10)
            assert not help_root.exists(), "help initialized telemetry"

        candidates = [(name, binary) for name, binary in args.binary if name == "msime-linux-dictionary"]
        if candidates:
            name, binary = candidates[0]
            for mode in ("redirected", "invalid", "relative", "symlink", "conflicting", "explicit"):
                fixture = Fixture(root / mode, args)
                fixture.control("enable")
                expected = mode in ("redirected", "explicit")
                if mode == "redirected":
                    other = fixture.root / "redirected-preferences"
                    fixture.control("disable")
                    fixture.control("enable", other)
                    fixture.collection_preferences = other
                    fixture.options.write_text(json.dumps({"preferences_directory": str(other)}))
                elif mode == "invalid":
                    fixture.options.write_text("{synthetic-private-invalid")
                elif mode == "relative":
                    fixture.environment["MSIME_CLIENT_HOST_OPTIONS"] = "relative.json"
                elif mode == "symlink":
                    actual = fixture.root / "actual.json"
                    fixture.options.rename(actual)
                    fixture.options.symlink_to(actual)
                elif mode == "conflicting":
                    fixture.environment["MSIME_IBUS_OPTIONS"] = str(fixture.options)
                    fixture.environment["MSIME_FCITX5_OPTIONS"] = str(fixture.root / "other.json")
                elif mode == "explicit":
                    fixture.environment["MSIME_CLIENT_HOST_OPTIONS"] = str(fixture.options)
                    fixture.environment["MSIME_IBUS_OPTIONS"] = "invalid"
                fixture.crash(binary, name, expected, args.version)
    print("Linux 独立程序：真实致命信号、关闭不记录、原版本补报、无输入会话和配置边界通过")


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""用真实 Host API 和独立故障进程校验辅助进程采集；不调用遥测网络接口。"""

import json
import os
from pathlib import Path
import platform
import queue
import subprocess
import sys
import tempfile
import threading
import time

BINARY = str(Path(sys.argv[1]).resolve())


class Helper:
    def __init__(self, root, component):
        self.directory = root / "telemetry"
        self.preferences = root / "preferences"
        self.arguments = [BINARY, "serve", str(self.directory), str(self.preferences), component]
        self.environment = dict(os.environ, HOME=str(root), LOCALAPPDATA=str(root), XDG_STATE_HOME=str(root))
        self.process = subprocess.Popen(self.arguments, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                        stderr=subprocess.DEVNULL, text=True, env=self.environment)
        self.lines = queue.Queue()
        threading.Thread(target=self.read_output, daemon=True).start()
        assert self.read() == "off"

    def read_output(self):
        for line in self.process.stdout:
            self.lines.put(line.rstrip("\n"))

    def read(self):
        return self.lines.get(timeout=10)

    def send(self, command):
        self.process.stdin.write(command + "\n")
        self.process.stdin.flush()

    def command(self, command, expected):
        self.send(command)
        assert self.read() == expected, command

    def wait_state(self, expected):
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            self.send("status")
            if self.read() == expected:
                return
            time.sleep(0.05)
        raise AssertionError(f"helper did not become {expected}")

    def records(self):
        return sorted((self.directory / "telemetry-crashes").glob("*.crash"))

    def events(self):
        path = self.directory / "telemetry.json"
        return json.loads(path.read_text()) if path.exists() else []

    def collect(self):
        subprocess.run([BINARY, "collect", *self.arguments[2:]], env=self.environment, check=True, timeout=15)

    def stop(self, command="stop"):
        self.command(command, "stopped")
        assert self.process.wait(timeout=10) == 0

    def die(self, command):
        self.send(command)
        assert self.process.wait(timeout=15) != 0

    def cleanup(self):
        if self.process.poll() is None:
            self.process.kill()
        self.process.wait(timeout=10)
        self.process.stdin.close()
        self.process.stdout.close()


def main():
    with tempfile.TemporaryDirectory(prefix="msime-helper-capture-") as temporary:
        root = Path(temporary).resolve()
        for component in ("watchdog", "prepare"):
            for command in ("fault", "terminate"):
                helper = Helper(root / (component + "-" + command), component)
                try:
                    assert not helper.preferences.exists(), "capture created a fresh state directory"
                    helper.command("enable", "saved")
                    helper.wait_state("armed")
                    assert not [e for e in helper.events() if e["kind"] in ("session", "session_crash")]
                    helper.die(command)
                    records = helper.records()
                    assert len(records) == 1, records
                    contents = records[0].read_text()
                    assert component in contents and "1.2.3" in contents
                    assert "synthetic-private" not in contents
                    helper.collect()
                    events = helper.events()
                    crashes = [event for event in events if event["kind"] == "crash"]
                    assert len(crashes) == 1, events
                    assert crashes[0]["version"] == "1.2.3"
                    assert component in crashes[0]["message"]
                    assert not [e for e in events if e["kind"] in ("session", "session_crash")]
                    assert not helper.records(), "queued record was not acknowledged"
                finally:
                    helper.cleanup()
        helper = Helper(root / "consent", "watchdog")
        try:
            helper.command("disable", "saved")
            helper.wait_state("off")
            assert not helper.records()
            helper.command("enable", "saved")
            helper.wait_state("armed")
            for invalid in ("unknown", "throw"):
                helper.command(invalid, "configured")
                helper.wait_state("off")
                helper.command("valid", "configured")
                helper.wait_state("armed")
            preferences = helper.preferences / "preferences.json"
            valid = preferences.read_bytes()
            preferences.write_bytes(b"{synthetic-invalid")
            helper.wait_state("off")
            preferences.write_bytes(valid)
            helper.wait_state("armed")
            helper.command("disable", "saved")
            helper.wait_state("off")
            assert not helper.events(), "explicit off did not clear pending events"
            helper.die("fault")
            assert not helper.records(), "off helper wrote a crash"
        finally:
            helper.cleanup()
        for command in ("stop", "recover"):
            helper = Helper(root / command, "prepare")
            try:
                helper.command("enable", "saved")
                helper.wait_state("armed")
                helper.stop(command)
                assert not helper.records()
                assert not [e for e in helper.events() if e["kind"] in ("session", "session_crash")]
            finally:
                helper.cleanup()
        if platform.system() == "Windows":
            state = root / "windows-state"
            state.mkdir()
            environment = dict(os.environ, LOCALAPPDATA=str(root))
            def describe():
                raw = subprocess.check_output([BINARY, "describe", str(root), str(state), "watchdog"],
                                              text=True, env=environment, timeout=10)
                return json.loads(raw)
            host = describe()
            assert Path(host["preferences"]) == state and host["capture_only"]
            redirected = root / "redirected"
            (state / "runtime-options.json").write_text(json.dumps({"preferences_directory": str(redirected)}))
            assert Path(describe()["preferences"]) == redirected
            (state / "runtime-options.json").write_text("{synthetic-invalid")
            assert describe() is None
    print("辅助进程：故障落盘、原版本重启入队、无输入会话、同意变化与正常停止通过")


if __name__ == "__main__":
    main()

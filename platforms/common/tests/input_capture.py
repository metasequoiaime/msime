#!/usr/bin/env python3
"""输入会话的实际故障、持续授权和线程收尾；发送由测试二进制拦截。"""
import json
import os
from pathlib import Path
import queue
import subprocess
import sys
import tempfile
import threading
import time

BINARY = str(Path(sys.argv[1]).resolve())


class Input:
    def __init__(self, root, mode="manual"):
        self.root = root
        self.directory = root / "telemetry"
        self.preferences = root / "preferences"
        self.process = subprocess.Popen(
            [BINARY, mode, str(self.directory), str(self.preferences)],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
            text=True, env=dict(os.environ, HOME=str(root), LOCALAPPDATA=str(root), XDG_STATE_HOME=str(root)))
        self.lines = queue.Queue()
        threading.Thread(target=self.read_output, daemon=True).start()
        self.initial = self.read()
        assert self.initial["armed"]

    def read_output(self):
        for line in self.process.stdout:
            self.lines.put(line.strip())

    def read(self):
        line = self.lines.get(timeout=10)
        return json.loads(line) if line.startswith("{") else line

    def command(self, command):
        self.process.stdin.write(command + "\n")
        self.process.stdin.flush()
        return self.read()

    def epoch(self):
        return json.loads((self.preferences / "preferences.json").read_text())["preferences"]["usage_reporting_consent"]["epoch"]

    def rotate(self):
        old = self.epoch()
        assert self.command("disable") == "saved"
        assert self.command("enable") == "saved"
        assert self.epoch() != old
        return old, self.epoch()

    def wait(self, enabled, epoch=None, *, queue_cleared=False):
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            state = self.command("status")
            marker = state["marker"]
            # 解除采集先于清理队列；关闭验收在同一期限内等待两者完成。
            if (state["armed"] == enabled
                    and (epoch is None or marker.get("context", {}).get("consent_epoch") == epoch)
                    and (not queue_cleared or not self.events("active"))):
                return state
            time.sleep(0.05)
        raise AssertionError("input observer did not reach the expected authorization")

    def die(self, command="fault"):
        self.process.stdin.write(command + "\n")
        self.process.stdin.flush()
        assert self.process.wait(timeout=15) != 0

    def collect(self):
        subprocess.run([BINARY, "collect", str(self.directory), str(self.preferences)], check=True, timeout=15)

    def events(self, kind):
        path = self.directory / "telemetry.json"
        try:
            queued = json.loads(path.read_text())
        except FileNotFoundError:
            return []
        return [event for event in queued if event["kind"] == kind]

    def records(self):
        return sorted((self.directory / "telemetry-crashes").glob("*.crash"))

    def cleanup(self):
        if self.process.poll() is None:
            self.process.kill()
        self.process.wait(timeout=10)
        self.process.stdin.close()
        self.process.stdout.close()


def check_crash(host):
    host.collect()
    assert len(host.events("crash")) == 1
    assert len(host.events("session_crash")) == 1
    assert host.events("crash")[0]["version"] == "1.2.3"
    assert not host.records()
    assert host.epoch() not in json.dumps(host.events("crash"))


def main():
    with tempfile.TemporaryDirectory(prefix="msime-input-capture-") as temporary:
        root = Path(temporary).resolve()
        for command in ("fault", "terminate"):
            host = Input(root / command)
            try:
                assert host.command("callback-off")["marker"] == host.initial["marker"], "cached false overrode canonical consent"
                host.die(command)
                assert len(host.records()) == 1
                assert "synthetic-private" not in host.records()[0].read_text()
                check_crash(host)
            finally:
                host.cleanup()

        for ending in ("fault", "normal", "blocked-reopen", "blocked-stop"):
            host = Input(root / ("independent-" + ending), "independent")
            try:
                assert host.initial["marker"] == {}, "independent settings created an input session"
                if ending.startswith("blocked-"):
                    request = host.command("flush-start")
                    assert request["capture_only"] and request["component"] == "settings"
                    assert request["consent_epoch"] == host.epoch()
                    assert host.command("disable") == "saved"
                    host.wait(False)
                    assert host.command("enable") == "saved"
                    host.wait(True)
                    if ending == "blocked-stop":
                        assert not host.command("observer-stop")["armed"]
                    assert host.command("flush-release")["armed"] == (ending != "blocked-stop")
                if ending in ("normal", "blocked-stop"):
                    assert not host.command("stop")["armed"]
                    assert host.process.wait(timeout=10) == 0
                    assert not host.records()
                else:
                    host.die()
                    assert len(host.records()) == 1
                    context = json.loads(host.records()[0].read_text().splitlines()[0].split(" ", 1)[1])
                    assert context["component"] == "settings" and context["consent_epoch"] == host.epoch()
                    host.collect()
                    crashes = host.events("crash")
                    assert len(crashes) == 1 and crashes[0]["version"] == "1.2.3"
                    assert not host.records()
                assert not host.events("session") and not host.events("session_crash")
                assert not (host.directory / "telemetry-session.json").exists()
            finally:
                host.cleanup()

        host = Input(root / "normal", "observe")
        try:
            assert not host.command("stop")["armed"]
            assert host.process.wait(timeout=10) == 0
            assert len(host.events("session")) == 1
            assert not host.events("crash") and not host.events("session_crash")
        finally:
            host.cleanup()

        for unreadable in (False, True):
            host = Input(root / ("unreadable-fault" if unreadable else "disabled-fault"), "observe")
            try:
                if unreadable:
                    (host.preferences / "preferences.json").write_bytes(b"{")
                else:
                    assert host.command("disable") == "saved"
                host.wait(False)
                host.die()
                assert not host.records()
            finally:
                host.cleanup()

        host = Input(root / "missed-withdrawal")
        try:
            _, epoch = host.rotate()
            state = host.command("refresh")
            assert state["armed"] and state["marker"]["id"] != host.initial["marker"]["id"]
            assert state["marker"]["context"]["consent_epoch"] == epoch
            host.die()
            check_crash(host)
        finally:
            host.cleanup()

        host = Input(root / "read-recovery")
        try:
            path = host.preferences / "preferences.json"
            saved = path.read_bytes()
            path.write_bytes(b"{")
            assert not host.command("refresh")["armed"]
            path.write_bytes(saved)
            state = host.command("refresh")
            assert state["armed"] and state["marker"] == host.initial["marker"]
            host.die()
            check_crash(host)
        finally:
            host.cleanup()

        host = Input(root / "late-old")
        try:
            old, epoch = host.rotate()
            host.collect()
            install = json.loads((host.directory / "telemetry-state.json").read_text())["install_id"]
            host.die()
            assert len(host.records()) == 1
            header = json.loads(host.records()[0].read_text().splitlines()[0].split(" ", 1)[1])
            assert header["consent_epoch"] == old and old != epoch
            host.collect()
            assert not host.events("crash") and not host.events("session_crash") and not host.records()
            assert json.loads((host.directory / "telemetry-state.json").read_text())["install_id"] == install
        finally:
            host.cleanup()
        fresh = Input(root / "late-old")
        try:
            fresh.die()
            check_crash(fresh)
        finally:
            fresh.cleanup()

        for reopen in (False, True):
            host = Input(root / ("prepared-cycle" if reopen else "prepared-off"))
            try:
                host.rotate()
                assert host.command("prepare-start") == "prepared"
                assert host.command("disable") == "saved"
                if reopen:
                    assert host.command("enable") == "saved"
                assert not host.command("prepare-release")["armed"], "late begin rearmed withdrawn consent"
                host.die()
                assert not host.records()
            finally:
                host.cleanup()

        for stop in (False, True):
            host = Input(root / ("blocked-stop" if stop else "blocked-reopen"), "observe")
            try:
                request = host.command("flush-start")
                assert request["consent_epoch"] == host.epoch()
                assert request["preferences_directory"] == str(host.preferences) and "enabled" not in request
                assert host.command("disable") == "saved"
                host.wait(False, queue_cleared=True)
                assert not host.events("active")
                assert host.command("enable") == "saved"
                current = host.wait(True, host.epoch())
                assert current["marker"]["id"] != host.initial["marker"]["id"]
                if stop:
                    assert not host.command("observer-stop")["armed"]
                after = host.command("flush-release")
                assert after["armed"] == (not stop), "old flush completion changed the new lifecycle"
                if not stop:
                    assert after["marker"] == current["marker"]
                    host.die()
                    check_crash(host)
                else:
                    assert not host.command("stop")["armed"]
                    assert host.process.wait(timeout=10) == 0
            finally:
                host.cleanup()
        print("input capture: faults, epoch rotation, read recovery, late records and blocked delivery lifecycle passed")


if __name__ == "__main__":
    main()

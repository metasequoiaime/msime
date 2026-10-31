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
    def __init__(self, root, component, initial="off", edition=None):
        self.directory = root / "telemetry"
        self.preferences = root / "preferences"
        self.arguments = [BINARY, "serve", str(self.directory), str(self.preferences), component]
        if edition is not None:
            self.arguments.append(edition)
        self.environment = dict(os.environ, HOME=str(root), LOCALAPPDATA=str(root), XDG_STATE_HOME=str(root))
        self.process = subprocess.Popen(self.arguments, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                        stderr=subprocess.DEVNULL, text=True, env=self.environment)
        self.lines = queue.Queue()
        threading.Thread(target=self.read_output, daemon=True).start()
        assert self.read() == initial

    def read_output(self):
        for line in self.process.stdout:
            self.lines.put(line.rstrip("\n"))
        self.lines.put(None)

    def read(self):
        line = self.lines.get(timeout=10)
        assert line is not None, f"helper exited before response (exit={self.process.poll()})"
        return line

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

    def wait_epoch(self, expected):
        deadline = time.monotonic() + 10
        path = self.directory / "telemetry-state.json"
        while time.monotonic() < deadline:
            try:
                current = json.loads(path.read_text()).get("consent_epoch")
            except (OSError, ValueError):
                current = None
            if current == expected:
                self.wait_state("armed")
                return
            time.sleep(0.05)
        raise AssertionError("helper did not activate the current continuous authorization")

    def wait_cleared(self):
        # 解除故障武装先于磁盘清理；不能把观察到 off 当成清理完成。
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            try:
                if not self.events() and not self.records():
                    return
            except FileNotFoundError:
                # 队列可在存在检查之后被清理；下一拍重新确认两类记录都已消失。
                pass
            time.sleep(0.05)
        raise AssertionError("explicit off did not clear pending events and records")

    def epoch(self):
        return json.loads((self.preferences / "preferences.json").read_text())["preferences"]["usage_reporting_consent"]["epoch"]

    def rotate_while_paused(self):
        old = self.epoch()
        self.command("pause", "paused")
        for command in ("disable", "enable"):
            subprocess.run([BINARY, command, *self.arguments[2:]], env=self.environment, check=True, timeout=15)
        current = self.epoch()
        assert current != old, "withdrawal reused the old continuous authorization"
        return old, current

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
        # 固定首次 load/save 之间出现授权初始化的时序，不能靠多跑几次等待碰撞。
        subprocess.run([BINARY, "enable-after-consent", str(root / "first-save-telemetry"),
                        str(root / "first-save-preferences"), "prepare"], check=True, timeout=15)
        # 准备程序先独占建好空目录，但此时尚未写 edition 标记；采集器不得抢先保存 full 默认值。
        editions = json.loads((Path(__file__).resolve().parents[3] / "shared/contracts/editions.json").read_text())["editions"]
        for edition in editions:
            state = root / ("bootstrap-" + edition["id"])
            (state / "preferences").mkdir(parents=True)
            helper = Helper(state, "prepare", initial="armed", edition=edition["id"])
            try:
                preferences = json.loads((helper.preferences / "preferences.json").read_text())["preferences"]
                assert preferences["scheme"] == edition["default_scheme"], (edition["id"], preferences["scheme"])
                assert preferences.get("last_chinese_scheme") == ("wubi" if edition["id"] == "wubi" else None)
                defaults = edition.get("preference_defaults", {})
                if "wubi_mixed_pinyin" in defaults:
                    assert preferences["wubi_mixed_pinyin"] == defaults["wubi_mixed_pinyin"]
                helper.stop()
                assert not [e for e in helper.events() if e["kind"] in ("session", "session_crash")]
            finally:
                helper.cleanup()
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
        # 用实际偏好保存和停止的观察线程确定性覆盖短暂撤回，不能依赖 500 ms 内抢跑。
        helper = Helper(root / "short-withdrawal", "watchdog")
        try:
            helper.command("enable", "saved")
            helper.wait_state("armed")
            old, current = helper.rotate_while_paused()
            helper.command("resume", "resumed")
            helper.wait_epoch(current)
            helper.die("fault")
            records = helper.records()
            assert len(records) == 1
            header = json.loads(records[0].read_text().splitlines()[0].split(" ", 1)[1])
            assert header["consent_epoch"] == current and header["consent_epoch"] != old
            helper.collect()
            crashes = [event for event in helper.events() if event["kind"] == "crash"]
            assert len(crashes) == 1
            assert current not in json.dumps(helper.events()), "local authorization leaked into events"
        finally:
            helper.cleanup()
        helper = Helper(root / "late-old-writer", "prepare")
        try:
            helper.command("enable", "saved")
            helper.wait_state("armed")
            old, current = helper.rotate_while_paused()
            helper.collect()
            install = json.loads((helper.directory / "telemetry-state.json").read_text())["install_id"]
            helper.die("fault")
            records = helper.records()
            assert len(records) == 1
            header = json.loads(records[0].read_text().splitlines()[0].split(" ", 1)[1])
            assert header["consent_epoch"] == old
            helper.collect()
            assert not [event for event in helper.events() if event["kind"] == "crash"], "old late writer entered the new queue"
            assert not helper.records(), "rejected old native record was not acknowledged"
            assert json.loads((helper.directory / "telemetry-state.json").read_text())["install_id"] == install
            fresh = Helper(root / "late-old-writer", "prepare", initial="armed")
            try:
                fresh.die("fault")
                fresh.collect()
                assert len([event for event in fresh.events() if event["kind"] == "crash"]) == 1
                assert current not in json.dumps(fresh.events())
            finally:
                fresh.cleanup()
        finally:
            helper.cleanup()
        # 真实 begin 回包或同代次恢复之后改变授权，必须在本轮最终武装前拒绝。
        for recovery in (False, True):
            for change in ("disable", "reopen", "unreadable"):
                helper = Helper(root / ("arm-%s-%s" % (recovery, change)), "prepare")
                try:
                    if recovery:
                        helper.command("enable", "saved")
                        helper.wait_state("armed")
                        helper.command("unknown", "configured")
                        helper.wait_state("off")
                    helper.command("arm-block", "blocked")
                    helper.command("valid" if recovery else "enable", "configured" if recovery else "saved")
                    helper.command("arm-wait", "preparing")
                    if change == "unreadable":
                        (helper.preferences / "preferences.json").write_bytes(b"{synthetic-invalid")
                    else:
                        helper.command("disable", "saved")
                        if change == "reopen":
                            helper.command("enable", "saved")
                    helper.command("arm-release", "off")
                    helper.die("fault")
                    assert not helper.records(), (recovery, change, "late publication captured a fault")
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
            helper.wait_cleared()
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

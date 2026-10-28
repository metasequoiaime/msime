#!/usr/bin/env python3
"""真实 macOS 输入法上报适配：致命故障、语言异常、授权与阻塞发送。"""
import json
from pathlib import Path
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "common/tests"))
from input_capture import Input


class MacInput(Input):
    def __init__(self, root):
        super().__init__(root, "observe")
        self.directory = Path(self.initial["directory"])
        assert self.directory.is_relative_to(root)


def check_crash(host, kind):
    host.collect()
    crashes = host.events("crash")
    assert len(crashes) == 1 and len(host.events("session_crash")) == 1
    assert crashes[0]["version"] == "unknown", "collector changed the original binary's version"
    assert host.epoch() not in json.dumps(crashes) and "synthetic-private" not in json.dumps(crashes)
    if kind == "exception":
        assert crashes[0]["message"].splitlines()[0] == "Uncaught SyntheticException"
        assert "main" in crashes[0]["stack"], "the exception's throw site was lost"
    assert not host.records()


def main():
    with tempfile.TemporaryDirectory(prefix="msime-macos-input-") as temporary:
        root = Path(temporary).resolve()
        for kind in ("fault", "exception"):
            host = MacInput(root / kind)
            try:
                host.die(kind)
                assert len(host.records()) == 1
                check_crash(host, kind)
            finally:
                host.cleanup()

        host = MacInput(root / "read-recovery")
        try:
            path = host.preferences / "preferences.json"
            original = path.read_bytes()
            path.write_bytes(b"{")
            host.wait(False)
            path.write_bytes(original)
            assert host.wait(True)["marker"] == host.initial["marker"]
            host.die()
            check_crash(host, "fault")
        finally:
            host.cleanup()

        for stop in (False, True):
            host = MacInput(root / ("blocked-stop" if stop else "blocked-reopen"))
            try:
                request = host.command("flush-start")
                assert request["consent_epoch"] == host.epoch() and "enabled" not in request
                assert host.command("disable") == "saved"
                host.wait(False)
                host.wait_cleared("active")
                assert not host.events("active")
                assert host.command("enable") == "saved"
                current = host.wait(True, host.epoch())
                assert current["marker"]["id"] != host.initial["marker"]["id"]
                if stop:
                    assert not host.command("observer-stop")["armed"]
                after = host.command("flush-release")
                assert after["armed"] == (not stop)
                if stop:
                    assert not host.command("stop")["armed"]
                    assert host.process.wait(timeout=10) == 0
                else:
                    assert after["marker"] == current["marker"]
                    host.die()
                    check_crash(host, "fault")
            finally:
                host.cleanup()
    print("macOS input: real faults, throw-site stack, read recovery and blocked-delivery consent passed")


if __name__ == "__main__":
    main()

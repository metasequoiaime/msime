#!/usr/bin/env python3
"""在合成输入法 bundle 中运行真实语音 helper；故障与补报仅写本地目录。"""

import argparse
import json
import os
from pathlib import Path
import plistlib
import resource
import select
import shutil
import signal
import subprocess
import tempfile
import time


class Fixture:
    def __init__(self, root, args, edition="full"):
        self.root = root
        self.controller = args.controller
        self.preferences = root / "synthetic-private-preferences"
        support = root / "Library" / "Application Support"
        self.telemetry = support / "MSIME"
        if edition != "full":
            self.telemetry /= edition
        self.telemetry /= "telemetry"
        settings = "app.msime.macos" + ("." + edition if edition != "full" else "")
        self.options = support / settings / "runtime-options.json"
        self.options.parent.mkdir(parents=True)
        self.options.write_text(json.dumps({"preferences_directory": str(self.preferences)}))
        self.bundle = root / "SyntheticInput.app"
        self.helper = self.bundle / "Contents" / "MacOS" / "msime-voice-local"
        self.helper.parent.mkdir(parents=True)
        shutil.copy2(args.helper, self.helper)
        (self.helper.parent / "SyntheticInput").touch()
        self.info = {"CFBundleIdentifier": "app.msime.inputmethod.MetasequoiaIME" if edition == "full"
                     else "app.msime.inputmethod." + edition,
                     "CFBundleExecutable": "SyntheticInput", "CFBundlePackageType": "APPL",
                     "CFBundleShortVersionString": "1.2.3", "CFBundleVersion": "1"}
        if edition != "full":
            self.info.update(MSIMEEdition=edition, MSIMESettingsBundleIdentifier=settings)
        self.write_info()
        self.environment = dict(os.environ, HOME=str(root), CFFIXED_USER_HOME=str(root))
        self.environment.pop("MSIME_CLIENT_HOST_OPTIONS", None)

    def write_info(self):
        (self.bundle / "Contents" / "Info.plist").write_bytes(plistlib.dumps(self.info))

    def control(self, mode):
        subprocess.run([self.controller, mode, str(self.telemetry), str(self.preferences), "collector"],
                       env=self.environment, check=True, timeout=15, stdout=subprocess.DEVNULL)

    def crash(self, expected, change=None):
        process = subprocess.Popen([str(self.helper), "--runtime", str(self.root / "missing-runtime.dylib")],
                                   stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                   stderr=subprocess.DEVNULL, env=self.environment)
        try:
            assert select.select([process.stdout], [], [], 10)[0], "helper did not become ready"
            assert json.loads(process.stdout.readline())["type"] == "hello"
            if change:
                change()
                # 观察周期为 500 ms；这里跨过多个周期再触发故障，覆盖常驻进程的真实同意刷新。
                time.sleep(1.5)
            process.send_signal(signal.SIGABRT)
            assert process.wait(timeout=15) == -signal.SIGABRT
            records = list((self.telemetry / "telemetry-crashes").glob("*.crash"))
            assert len(records) == int(expected), (self.root.name, records)
            if expected:
                raw = records[0].read_text()
                assert "voice-local" in raw and "1.2.3" in raw
                assert str(self.root) not in raw and "synthetic-private" not in raw
                self.control("collect")
                events = json.loads((self.telemetry / "telemetry.json").read_text())
                crashes = [event for event in events if event["kind"] == "crash"]
                assert len(crashes) == 1 and crashes[0]["version"] == "1.2.3"
                assert "voice-local" in crashes[0]["message"]
                assert not [event for event in events if event["kind"] in ("session", "session_crash")]
                assert not list((self.telemetry / "telemetry-crashes").glob("*.crash"))
        finally:
            if process.poll() is None:
                process.kill()
            process.wait(timeout=10)
            process.stdin.close()
            process.stdout.close()


def main():
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    parser = argparse.ArgumentParser()
    parser.add_argument("--helper", required=True)
    parser.add_argument("--controller", required=True)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="msime-macos-voice-crash-") as temporary:
        root = Path(temporary).resolve()
        for edition in ("full", "pinyin"):
            for enabled in (False, True):
                fixture = Fixture(root / (edition + str(enabled)), args, edition)
                fixture.control("enable" if enabled else "disable")
                fixture.crash(enabled)
        for mode in ("close", "open", "corrupt", "redirect", "relative", "symlink", "embedded", "explicit",
                     "missing-identity", "invalid-identity", "wrong-bundle", "unbundled", "help"):
            fixture = Fixture(root / mode, args, "pinyin")
            fixture.control("disable" if mode == "open" else "enable")
            expected = mode in ("open", "embedded", "explicit")
            change = None
            if mode in ("close", "open"):
                change = lambda: fixture.control("disable" if mode == "close" else "enable")
            elif mode == "corrupt":
                change = lambda: fixture.options.write_text("{synthetic-private-invalid")
            elif mode == "redirect":
                # 新偏好明确关闭；不能继续沿用旧目录的同意。
                other = fixture.root / "redirected-preferences"
                subprocess.run([fixture.controller, "disable", str(fixture.telemetry), str(other), "collector"],
                               env=fixture.environment, check=True, timeout=15, stdout=subprocess.DEVNULL)
                change = lambda: fixture.options.write_text(json.dumps({"preferences_directory": str(other)}))
            elif mode == "relative":
                fixture.environment["MSIME_CLIENT_HOST_OPTIONS"] = "relative.json"
            elif mode == "symlink":
                actual = fixture.root / "actual.json"
                fixture.options.rename(actual)
                fixture.options.symlink_to(actual)
            elif mode == "embedded":
                resources = fixture.bundle / "Contents" / "Resources"
                resources.mkdir()
                shutil.copy2(fixture.options, resources / "runtime-options.json")
                fixture.options.write_text("invalid")
            elif mode == "explicit":
                actual = fixture.root / "parent-runtime-options.json"
                shutil.copy2(fixture.options, actual)
                fixture.options.write_text("invalid")
                fixture.environment["MSIME_CLIENT_HOST_OPTIONS"] = str(actual)
            elif mode == "missing-identity":
                del fixture.info["MSIMESettingsBundleIdentifier"]
                fixture.write_info()
            elif mode == "invalid-identity":
                fixture.info["MSIMEEdition"] = "../full"
                fixture.write_info()
            elif mode == "wrong-bundle":
                fixture.info["CFBundleIdentifier"] = "app.synthetic.unrelated"
                fixture.write_info()
            elif mode == "unbundled":
                helper = fixture.root / "msime-voice-local"
                shutil.copy2(fixture.helper, helper)
                fixture.helper = helper
            elif mode == "help":
                result = subprocess.run([str(fixture.helper), "--help"], env=fixture.environment,
                                        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=10)
                assert result.returncode == 2
                assert not fixture.telemetry.exists(), "help initialized telemetry"
                continue
            fixture.crash(expected, change)
            if mode != "unbundled":
                assert not (fixture.root / "Library/Application Support/MSIME/telemetry").exists(), "used full consent"
    print("macOS 真实语音 helper：故障、版隔离、同意刷新、配置边界与原版本补报通过")


if __name__ == "__main__":
    main()

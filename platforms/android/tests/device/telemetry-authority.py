#!/usr/bin/env python3
"""专用 API 28/35 AVD：产品 JNI 在真实应用 UID 下读取只读共享授权。"""

import os
from pathlib import Path
import re
import subprocess
import sys


def main():
    sdk = Path(os.environ.get("ANDROID_SDK_ROOT", os.environ.get("ANDROID_HOME", "")))
    serial = sys.argv[1] if len(sys.argv) == 3 else ""
    revision = sys.argv[2] if len(sys.argv) == 3 else ""
    expected = {"emulator-5566": ("msime-client-test-api28", "28"),
                "emulator-5580": ("msime-client-test", "35")}
    if serial not in expected:
        raise RuntimeError("requires dedicated API 28/35 AVD serial")
    if re.fullmatch(r"[0-9a-f]{40}", revision) is None:
        raise RuntimeError("requires exact expected product revision as second argument")

    def run(*args):
        return subprocess.run([str(sdk / "platform-tools/adb"), "-s", serial, *args],
                              capture_output=True, text=True, check=True, timeout=90).stdout.strip()

    avd, api = expected[serial]
    if run("emu", "avd", "name").splitlines()[0] != avd or run("shell", "getprop ro.build.version.sdk") != api:
        raise RuntimeError("requires repository-owned AVD with matching API")
    if any(line.lstrip().startswith("Instrumentation #")
           for line in run("shell", "dumpsys activity processes").splitlines()):
        raise RuntimeError("dedicated AVD is already running instrumentation")
    run("shell", "am force-stop app.msime.android")
    try:
        output = run("shell", "am instrument -w -e phase authority -e expected-revision " + revision + " "
                     "app.msime.android.test/app.msime.android.test.DiagnosticConsentDeviceSmoke")
        if "MSIME_DEVICE_SMOKE_PASSED" not in output or "MSIME_DEVICE_SMOKE_FAILED" in output:
            raise AssertionError("real app UID authority checks failed: " + output)
        print(serial + ": real application UID, read-only authority, recovery and epoch rejection passed")
    finally:
        run("shell", "am force-stop app.msime.android")


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""专用 API 35 AVD：真实系统退出补录及跨重启撤回；预先安装同源原生产品 APK。"""

import json
import os
from pathlib import Path
import subprocess
import sys
import time


def main():
    sdk = Path(os.environ.get("ANDROID_SDK_ROOT", os.environ.get("ANDROID_HOME", "")))
    adb = str(sdk / "platform-tools/adb")
    serial = sys.argv[1] if len(sys.argv) == 2 else "emulator-5580"
    if serial != "emulator-5580":
        raise RuntimeError("requires dedicated emulator-5580")

    def run(*args, data=None, check=True):
        result = subprocess.run([adb, "-s", serial, *args], input=data, capture_output=True, check=check, timeout=60)
        return result.stdout.decode().strip()

    def shell(command, **kwargs):
        return run("shell", command, **kwargs)

    if run("emu", "avd", "name").splitlines()[0] != "msime-client-test":
        raise RuntimeError("requires dedicated msime-client-test AVD")
    if int(shell("getprop ro.build.version.sdk")) < 30:
        raise RuntimeError("system exit history requires API 30+")
    run("root")
    run("wait-for-device")
    base = "/data/user/0/app.msime.android/files"
    telemetry = base + "/telemetry"
    ledger = base + "/telemetry-diagnostic-consent"

    def read(path):
        return shell("cat " + path)

    def write(path, content):
        run("shell", "cat > " + path, data=content.encode())

    def wait(check, label):
        for _ in range(60):
            value = check()
            if value:
                return value
            time.sleep(.5)
        raise AssertionError(label)

    def stop():
        shell("am force-stop app.msime.android")

    def start():
        shell("am start -W -n app.msime.android/app.msime.android.home.HomeActivity")
        wait(lambda: shell("pidof app.msime.android"), "application start")
        time.sleep(3)

    def crash_files():
        return [name for name in shell("ls " + telemetry + "/telemetry-crashes", check=False).splitlines()
                if name.endswith(".crash")]

    def crashes():
        raw = shell("cat " + telemetry + "/telemetry.json", check=False)
        return [event for event in json.loads(raw or "[]") if event.get("kind") == "crash"]

    def crash():
        output = shell("am instrument -w -e phase system-crash "
                       "app.msime.android.test/app.msime.android.test.DiagnosticConsentDeviceSmoke")
        assert "Process crashed" in output, "actual uncaught product-process exception"
        assert not crash_files(), "system-only fixture must not leave a native record"
        time.sleep(1)

    stop()
    uid = shell("stat -c %u " + base)
    if not uid.isdigit():
        raise RuntimeError("install and prepare product APK first")
    rule = "OUTPUT -m owner --uid-owner " + uid + " -j REJECT"
    dialogs = shell("settings get global hide_error_dialogs")
    shell("iptables -I " + rule)
    original = None
    preferences = None
    try:
        # 让系统按用户关闭错误对话框的路径结束进程，避免 force-stop 篡改退出原因。
        shell("settings put global hide_error_dialogs 1")
        options = json.loads(read(base + "/runtime-options.json"))
        preferences = options["preferences_directory"] + "/preferences.json"
        original = read(preferences)
        document = json.loads(original)
        document["preferences"].pop("usage_reporting_consent", None)
        document["preferences"]["usage_reporting"] = True
        write(preferences, json.dumps(document))
        shell("rm -rf " + telemetry)
        shell("rm -f " + ledger)
        start()
        wait(lambda: shell("test -f " + ledger + " && echo ready", check=False), "capture preparation")
        old = read(ledger)
        crash()
        # 夹具只解除现场采集，真实 ActivityManager 历史补录仍走产品入口。
        start()
        wait(lambda: any(event.get("message", "").startswith("Crash Android system reason ")
                         for event in crashes()), "actual system exit collection")
        assert read(ledger) == old, "continuous consent changed at restart"
        print("API 35: actual system exit -> queue; continuous consent survived restart", flush=True)
        first_id = first[0]["id"]
        stop()
        start()
        assert len(crashes()) == 1 and crashes()[0]["id"] == first_id, "system crash duplicated on restart"
        stop()
        # 隔离网络下模拟队列已确认发送；持久收据仍须阻止系统历史再次入队。
        write(telemetry + "/telemetry.json", "[]")
        start()
        assert not crashes() and not crash_files(), "delivered system crash recreated on restart"
        print("API 35: restart and drained-queue restart both reject the same historical crash", flush=True)

        stop()
        document["preferences"].pop("usage_reporting_consent", None)
        document["preferences"]["usage_reporting"] = False
        write(preferences, json.dumps(document))
        output = shell("am instrument -w -e phase revoke "
                       "app.msime.android.test/app.msime.android.test.DiagnosticConsentDeviceSmoke")
        assert "MSIME_DEVICE_SMOKE_PASSED" in output, "product setEnabled callback"
        assert shell("test ! -e " + ledger + " && echo revoked") == "revoked"
        assert not crashes(), "withdrawal did not clear queue"
        stop()
        document["preferences"].pop("usage_reporting_consent", None)
        document["preferences"]["usage_reporting"] = True
        write(preferences, json.dumps(document))
        start()
        assert read(ledger) != old, "re-enable reused revoked epoch"
        assert not crashes(), "old system exit returned after re-enable"
        stop()
        start()
        assert not crashes(), "old system exit returned after another restart"
        print("API 35: withdraw -> re-enable -> two restarts rejects old system history", flush=True)

        crash()
        start()
        wait(lambda: any(event.get("message", "").startswith("Crash Android system reason ")
                         for event in crashes()), "new epoch system exit collection")
        print("API 35: new epoch still collects a new actual system exit", flush=True)
    except AssertionError:
        print("Synthetic diagnostic: crashes=" + json.dumps([
            {"kind": event.get("kind"), "summary": event.get("message", "").splitlines()[:1]}
            for event in crashes()]) + "; pending_records=" + str(len(crash_files())), flush=True)
        raise
    finally:
        stop()
        try:
            if original is not None:
                write(preferences, original)
            shell("rm -rf " + telemetry)
            shell("rm -f " + ledger)
        finally:
            try:
                shell("iptables -D " + rule)
            finally:
                shell("settings delete global hide_error_dialogs" if dialogs == "null"
                      else "settings put global hide_error_dialogs " + str(int(dialogs)))
        print("Synthetic queue removed; preferences and network rule restored", flush=True)


if __name__ == "__main__":
    main()

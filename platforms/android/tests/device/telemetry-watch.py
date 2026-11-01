#!/usr/bin/env python3
"""专用 API 28/35 AVD：真实 :ime 存活期间的外部同意变化与故障落盘。"""

import json
import os
from pathlib import Path
import shlex
import subprocess
import sys
import time


def main():
    sdk = Path(os.environ.get("ANDROID_SDK_ROOT", os.environ.get("ANDROID_HOME", "")))
    adb = str(sdk / "platform-tools/adb")
    serial = sys.argv[1] if len(sys.argv) == 2 else "emulator-5580"
    expected = {"emulator-5580": "msime-client-test", "emulator-5566": "msime-client-test-api28"}
    if serial not in expected:
        raise RuntimeError("requires dedicated API 28/35 AVD")

    def run(*args, data=None, check=True):
        result = subprocess.run([adb, "-s", serial, *args], input=data, capture_output=True, check=check, timeout=60)
        return result.stdout.decode().strip()

    def shell(command, **kwargs):
        return run("shell", command, **kwargs)

    if run("emu", "avd", "name").splitlines()[0] != expected[serial]:
        raise RuntimeError("requires repository-owned AVD")
    run("root")
    run("wait-for-device")
    base = "/data/user/0/app.msime.android/files"
    fixture = base + "/synthetic-telemetry-watch"
    telemetry = base + "/telemetry"
    ledger = base + "/telemetry-diagnostic-consent"
    options_path = base + "/runtime-options.json"
    process = None

    def read(path, check=True):
        return shell("cat " + shlex.quote(path), check=check)

    def write(path, content):
        # 原子替换模拟产品的持久化保存，避免测试自己的半份 JSON 触发停采。
        run("shell", "cat > " + shlex.quote(path + ".watch-tmp"), data=content.encode())
        shell("mv " + shlex.quote(path + ".watch-tmp") + " " + shlex.quote(path))

    def wait(check, label):
        for _ in range(80):
            value = check()
            if value:
                return value
            time.sleep(.25)
        raise AssertionError(label)

    def status():
        try:
            return json.loads(read(fixture + "/status.json", check=False))
        except json.JSONDecodeError:
            return {}

    def active():
        current = status()
        return current if current.get("enabled") and current.get("capture") else None

    def inactive():
        current = status()
        return current if current and not current.get("enabled") else None

    def stop():
        nonlocal process
        shell("am force-stop app.msime.android")
        if process is not None:
            process.communicate(timeout=15)
            process = None

    def start():
        nonlocal process
        stop()
        shell("rm -rf " + fixture)
        process = subprocess.Popen([adb, "-s", serial, "shell", "am instrument -w "
            "app.msime.android.test/app.msime.android.test.TelemetryWatchDeviceSmoke"],
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        return wait(lambda: status(), "instrumentation state")

    def command(value):
        write(fixture + "/command", value)

    def records():
        return [name for name in shell("ls " + telemetry + "/telemetry-crashes", check=False).splitlines()
                if name.endswith(".crash")]

    def events():
        return json.loads(read(telemetry + "/telemetry.json", check=False) or "[]")

    def consent(value):
        current = json.loads(read(preferences))
        if not value or not current["preferences"].get("usage_reporting", True):
            current["preferences"].pop("usage_reporting_consent", None)
        current["preferences"]["usage_reporting"] = value
        document.clear()
        document.update(current)
        write(preferences, json.dumps(document))

    def same_process(original):
        assert status().get("pid") == original["pid"], "fixture restarted instead of observing consent"
        assert shell("pidof app.msime.android:ime") == str(original["pid"]), "actual :ime process"

    def fault(kind, enabled):
        original = wait(active if enabled else inactive, "fault consent preparation")
        same_process(original)
        before = set(records())
        command(kind)
        wait(lambda: not shell("pidof app.msime.android:ime", check=False), "actual fatal process exit")
        if enabled:
            wait(lambda: set(records()) - before, "enabled fault record")
        else:
            assert set(records()) == before, "disabled fault wrote a record"
        stop()

    stop()
    uid = shell("stat -c %u " + base)
    if not uid.isdigit():
        raise RuntimeError("install and prepare native product APK first")
    rule = "OUTPUT -m owner --uid-owner " + uid + " -j REJECT"
    dialogs = shell("settings get global hide_error_dialogs")
    original_options = read(options_path)
    preferences = json.loads(original_options)["preferences_directory"] + "/preferences.json"
    original = read(preferences)
    document = json.loads(original)
    shell("iptables -I " + rule)
    try:
        shell("settings put global hide_error_dialogs 1")
        shell("rm -rf " + telemetry)
        shell("rm -f " + ledger)
        consent(True)
        start()
        ready = wait(active, "initial capture")
        assert ready["session_id"] and ready["watching"], "input session with independent observer"
        for _ in range(8):
            time.sleep(.25)
            same_process(ready)
            assert status()["session_id"] == ready["session_id"], "stable consent created another session"
        consent(False)
        wait(inactive, "other process disables capture")
        same_process(ready)
        assert not read(ledger, check=False), "withdrawal retained diagnostic epoch"
        assert not events(), "withdrawal retained queue"
        fault("java", False)
        print(serial + ": live :ime observes withdrawal; Java fatal writes nothing", flush=True)

        consent(True)
        start()
        ready = wait(active, "restart preparation")
        consent(False)
        wait(inactive, "native withdrawal")
        same_process(ready)
        fault("native", False)
        print(serial + ": live :ime observes withdrawal; native SIGABRT writes nothing", flush=True)

        # 临时不可读不算明确撤回；恢复同一持久身份时不重复开始输入会话。
        consent(True)
        start()
        ready = wait(active, "recovery preparation")
        document = json.loads(read(preferences))
        write(options_path, "{")
        wait(inactive, "bad options suspend capture")
        same_process(ready)
        write(options_path, original_options)
        restored = wait(active, "restored options resume capture")
        assert restored["session_id"] == ready["session_id"] and restored["epoch"] == ready["epoch"]
        shell("rm " + options_path)
        wait(inactive, "removed options suspend capture")
        same_process(ready)
        write(options_path, original_options)
        restored = wait(active, "restored removed options resume capture")
        assert restored["session_id"] == ready["session_id"] and restored["epoch"] == ready["epoch"]
        write(preferences, "{")
        wait(inactive, "bad preferences suspend capture")
        write(preferences, json.dumps(document))
        restored = wait(active, "restored preferences resume capture")
        assert restored["session_id"] == ready["session_id"] and restored["epoch"] == ready["epoch"]
        print(serial + ": unreadable options/preferences suspend capture and resume the same session", flush=True)

        consent(False)
        wait(inactive, "withdrawal before re-enable")
        consent(True)
        reopened = wait(active, "live re-enable")
        same_process(ready)
        assert reopened["session_id"] != ready["session_id"] and reopened["epoch"] != ready["epoch"]
        # 阻止观察器看到中间关闭，用真实 JNI 保存两次，不能靠篡改 Java 镜像模拟共享代次。
        command("cycle")
        changed = wait(lambda: s if (s := active()) and s["completed"] == "cycle"
                       and s["epoch"] != reopened["epoch"] else None, "changed shared epoch")
        assert changed["session_id"] != reopened["session_id"], "missed off/on reused old identity"
        shared = json.loads(read(preferences))["preferences"]["usage_reporting_consent"]
        assert changed["epoch"] == shared["epoch"], "capture did not bind shared consent"
        assert read(ledger).split("|")[0] == shared["epoch"], "Java mirror generated its own epoch"
        same_process(ready)
        before = events()
        command("stale-close")
        wait(lambda: status().get("completed") == "stale-close", "stale close callback")
        assert status()["enabled"] and status()["session_id"] == changed["session_id"]
        assert events() == before, "stale close cleared re-enabled queue"
        # 模拟旧原生快照在新队列激活后才写完；重启只能接收当前授权故障。
        write(reopened["capture"], reopened["header"] + "Synthetic revoked late native fault\nsynthetic+0x41\n")
        fault("java", True)
        start()
        wait(active, "restart ingests Java crash")
        wait(lambda: any(event.get("kind") == "crash" for event in events()), "Java crash queue")
        assert not records(), "ingested crash original remains"
        assert not any("Synthetic revoked late native fault" in event.get("message", "") for event in events()), "revoked late write was accepted"
        print(serial + ": live re-enable and changed epoch use fresh identities; Java fault -> restart -> queue", flush=True)

        ready = wait(active, "end preparation")
        command("end")
        wait(lambda: s if (s := inactive()) and not s.get("watching") else None, "normal end")
        time.sleep(1.5)
        assert not status()["enabled"] and not status()["watching"], "end rearmed capture"
        same_process(ready)
        command("begin")
        restarted = wait(active, "new input session")
        assert restarted["session_id"] != ready["session_id"], "new session reused ended identity"
        time.sleep(1)
        assert status()["session_id"] == restarted["session_id"] and status()["enabled"], "stale end stopped new session"
        fault("native", True)
        start()
        wait(active, "restart ingests native crash")
        wait(lambda: sum(event.get("kind") == "crash" for event in events()) >= 2, "native crash queue")
        print(serial + ": end stays disarmed; a new session survives; native fault -> restart -> queue", flush=True)

        consent(False)
        start()
        wait(inactive, "initial disabled")
        time.sleep(1)
        assert not status()["enabled"] and not events() and not records(), "initial off captures or queues"
        consent(True)
        wait(active, "initial off -> live enable")
        command("end")
        wait(lambda: s if (s := inactive()) and not s.get("watching") else None, "end before application capture")
        consent(False)
        consent(True)
        # 清空独立采集验证前的合成输入会话；产品 start 路径自身不创建输入会话。
        shell("rm -rf " + telemetry)
        command("application")
        app = wait(active, "capture-only start")
        assert app["session_id"] == "", "capture-only started input session"
        capture = app["capture"]
        command("application")
        time.sleep(1)
        assert status()["capture"] == capture, "reopening capture-only app reallocated capture"
        assert not any(event.get("kind") in ("session", "session_crash") for event in events())
        command("finish")
        output = process.communicate(timeout=20)[0].decode()
        process = None
        assert "MSIME_DEVICE_SMOKE_PASSED" in output, "instrumentation completion"
        print(serial + ": initial off can enable; capture-only creates no input-session events", flush=True)

        # 实际 HomeActivity 的应用进程也必须响应外部关闭和重开，不能只验 IME 里的静态方法。
        stop()
        shell("rm -rf " + telemetry)
        shell("am start -W -n app.msime.android/app.msime.android.home.HomeActivity")
        app_pid = wait(lambda: shell("pidof app.msime.android"), "actual application process")
        time.sleep(2)
        consent(False)
        # 前台设置窗口每秒核对一次授权，留出一轮半的余量再触发故障。
        time.sleep(2.5)
        assert shell("pidof app.msime.android") == app_pid, "application restarted at withdrawal"
        shell("kill -ABRT " + app_pid)
        # Android 会自动重启前台 Activity；核对故障 PID 消失，不能要求新进程也不存在。
        wait(lambda: app_pid not in shell("pidof app.msime.android", check=False).split(), "disabled application fatal exit")
        assert not records() and not events(), "disabled application retained native capture"
        stop()
        shell("am start -W -n app.msime.android/app.msime.android.home.HomeActivity")
        app_pid = wait(lambda: shell("pidof app.msime.android"), "disabled application start")
        consent(True)
        time.sleep(2)
        assert shell("pidof app.msime.android") == app_pid, "application restarted at re-enable"
        shell("kill -ABRT " + app_pid)
        wait(lambda: app_pid not in shell("pidof app.msime.android", check=False).split(), "enabled application fatal exit")
        wait(lambda: records() or any(event.get("kind") == "crash" for event in events()),
             "re-enabled application native record or automatic restart ingestion")
        stop()
        shell("am start -W -n app.msime.android/app.msime.android.home.HomeActivity")
        wait(lambda: any(event.get("kind") == "crash" for event in events()), "application restart queue")
        assert not any(event.get("kind") in ("session", "session_crash") for event in events())
        print(serial + ": actual App observes off/on while alive; native fault -> restart -> queue without input sessions", flush=True)
    finally:
        stop()
        try:
            write(options_path, original_options)
            write(preferences, original)
            shell("rm -rf " + telemetry + " " + fixture)
            shell("rm -f " + ledger)
        finally:
            try:
                shell("iptables -D " + rule)
            finally:
                shell("settings delete global hide_error_dialogs" if dialogs == "null"
                      else "settings put global hide_error_dialogs " + str(int(dialogs)))
        print("Synthetic queue/fixture removed; preferences, options and network rule restored", flush=True)


if __name__ == "__main__":
    main()

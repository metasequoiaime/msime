#!/usr/bin/env python3
"""专用 API 35 AVD：系统真实输入超时 ANR、重启补录、去重与关闭抑制。"""

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
    if serial != "emulator-5580":
        raise RuntimeError("requires dedicated emulator-5580")

    def run(*args, data=None, check=True):
        result = subprocess.run([adb, "-s", serial, *args], input=data, capture_output=True, check=check, timeout=60)
        return result.stdout.decode().strip()

    def shell(command, **kwargs):
        return run("shell", command, **kwargs)

    if run("emu", "avd", "name").splitlines()[0] != "msime-client-test":
        raise RuntimeError("requires repository-owned msime-client-test AVD")
    if int(shell("getprop ro.build.version.sdk")) < 30:
        raise RuntimeError("system exit history requires API 30+")
    run("root")
    run("wait-for-device")
    base = "/data/user/0/app.msime.android/files"
    telemetry = base + "/telemetry"
    ledger = base + "/telemetry-diagnostic-consent"
    stopped_pid = None

    def read(path, check=True):
        return shell("cat " + shlex.quote(path), check=check)

    def write(path, content):
        run("shell", "cat > " + shlex.quote(path + ".anr-tmp"), data=content.encode())
        shell("mv " + shlex.quote(path + ".anr-tmp") + " " + shlex.quote(path))

    def wait(check, label):
        for _ in range(120):
            value = check()
            if value:
                return value
            time.sleep(.5)
        raise AssertionError(label)

    def stop():
        nonlocal stopped_pid
        # 失败清理先恢复仍存活的原 App，不能留下 SIGSTOP 进程妨碍后续任务。
        if stopped_pid and stopped_pid in shell("pidof app.msime.android", check=False).split():
            shell("kill -CONT " + stopped_pid, check=False)
        stopped_pid = None
        shell("am force-stop app.msime.android")

    def start():
        shell("am start -W -n app.msime.android/app.msime.android.home.HomeActivity")
        pid = wait(lambda: shell("pidof app.msime.android"), "actual application start")
        time.sleep(3)
        return pid

    def events():
        return json.loads(read(telemetry + "/telemetry.json", check=False) or "[]")

    def hangs():
        return [event for event in events() if event.get("message", "").startswith("Hang Android ANR")]

    def records():
        return [name for name in shell("ls " + telemetry + "/telemetry-crashes", check=False).splitlines()
                if name.endswith(".crash")]

    def consent(value):
        document["preferences"].pop("usage_reporting_consent", None)
        document["preferences"]["usage_reporting"] = value
        write(preferences, json.dumps(document))

    def anr(pid):
        nonlocal stopped_pid
        assert pid in shell("pidof app.msime.android").split(), "original App PID"
        stopped_pid = pid
        # 冻结真实前台进程，再送入触摸事件；让 InputDispatcher 和 ActivityManager 判定/结束 ANR。
        # 不伪造 ApplicationExitInfo，也不用 force-stop 或主动 SIGKILL 代替系统处置。
        shell("kill -STOP " + pid)
        shell("input tap 400 500")
        wait(lambda: pid not in shell("pidof app.msime.android", check=False).split(), "actual system ANR exit")
        stopped_pid = None

    stop()
    uid = shell("stat -c %u " + base)
    if not uid.isdigit():
        raise RuntimeError("install and prepare native product APK first")
    original_options = read(base + "/runtime-options.json")
    preferences = json.loads(original_options)["preferences_directory"] + "/preferences.json"
    original = read(preferences)
    document = json.loads(original)
    dialogs = shell("settings get global hide_error_dialogs")
    rule = "OUTPUT -m owner --uid-owner " + uid + " -j REJECT"
    shell("iptables -I " + rule)
    try:
        shell("settings put global hide_error_dialogs 1")
        shell("rm -rf " + telemetry)
        shell("rm -f " + ledger)
        consent(True)
        pid = start()
        wait(lambda: read(ledger, check=False), "diagnostic capture preparation")
        epoch = read(ledger)
        anr(pid)
        start()
        first = wait(lambda: hangs(), "actual ANR -> product restart -> queue")
        assert len(first) == 1, "ANR must create one diagnostic"
        assert not records(), "ANR fabricated a crash-site record"
        assert not any(event.get("kind") == "session_crash" for event in events()), "ANR polluted input crash rate"
        assert not any(event.get("message", "").startswith("Crash Android system reason ") for event in events())
        identity = first[0]["id"]
        assert read(ledger) == epoch, "continuous consent changed at restart"
        stop()
        start()
        assert len(hangs()) == 1 and hangs()[0]["id"] == identity, "historical ANR duplicated on restart"
        print("API 35: actual input-dispatch ANR -> system exit -> restart -> one Hang diagnostic; next restart deduplicates", flush=True)
        stop()
        # 本用例阻断网络；清空合成队列模拟发送完成，不能借旧队列的相同 id 掩盖重复补录。
        write(telemetry + "/telemetry.json", "[]")
        start()
        assert not hangs() and not records(), "delivered ANR recreated on restart"
        print("API 35: drained-queue restart does not requeue the historical ANR", flush=True)

        # 第二次真实 ANR 发生前，另一个进程关闭偏好；活跃 App 应清除系统摘要和持久 epoch。
        consent(False)
        wait(lambda: not read(ledger, check=False), "live App withdrawal")
        assert not events(), "withdrawal retained queue"
        pid = shell("pidof app.msime.android")
        anr(pid)
        start()
        assert not hangs() and not records() and not events(), "disabled ANR captured"
        stop()
        consent(True)
        start()
        assert read(ledger) != epoch, "re-enable reused withdrawn epoch"
        assert not hangs() and not records(), "re-enable resurrected prior ANRs"
        stop()
        start()
        assert not hangs() and not records(), "second restart resurrected prior ANRs"
        print("API 35: live withdrawal -> actual ANR records nothing; re-enable and two restarts reject prior ANRs", flush=True)
    finally:
        stop()
        try:
            write(preferences, original)
            shell("rm -rf " + telemetry)
            shell("rm -f " + ledger)
        finally:
            try:
                shell("iptables -D " + rule)
            finally:
                shell("settings delete global hide_error_dialogs" if dialogs == "null"
                      else "settings put global hide_error_dialogs " + str(int(dialogs)))
        print("Synthetic queue removed; preferences, system dialogs and network rule restored", flush=True)


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""核对 macOS 发布包的并行公证：package-release.sh 的 notary_submit 只提交不等待，notary_result 等到最终状态为止。

用合成的 `xcrun` 和 `sleep` 运行脚本里的这两个函数，不访问 Apple 服务、不读取签名凭据。真实的提交、等待和装订只有带 secrets 的 Release macOS 运行能验证；这里守住的是那次运行覆盖不到的分支：处理中提前返回、被拒、状态取不到，以及密码不进日志。

只用标准库；没有 bash 或打包脚本时打印 skipped 并以 0 退出。
"""
import os
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PACKAGE = ROOT / "platforms/macos/package-release.sh"
PASSWORD = "synthetic-app-specific-password"

# 合成的 xcrun：submit 打印提交 id；wait 什么也不做；info 依次读 $MSIME_TEST_INFO_PLAN（逗号分隔）里的状态，`error` 表示这一次取状态失败。每次调用记进 $MSIME_TEST_CALLS。
FAKES = r"""
sleep() { echo "sleep $1" >> "$MSIME_TEST_CALLS"; }
xcrun() {
  echo "$*" | sed "s/$APPLE_APP_SPECIFIC_PASSWORD/<password>/g" | cut -d' ' -f1-3 >> "$MSIME_TEST_CALLS"
  case "$2" in
    submit) echo "Conducting pre-submission checks"; printf '{"id":"00000000-synthetic","path":"%s","message":"Successfully uploaded file"}\n' "$3" ;;
    wait) echo "Current status: synthetic" ;;
    info)
      local index status
      index="$(grep -c ' info ' "$MSIME_TEST_CALLS")"
      status="$(echo "$MSIME_TEST_INFO_PLAN" | cut -d, -f"$index")"
      [ "$status" != error ] || { echo "synthetic network error" >&2; return 1; }
      printf '{"status":"%s","id":"%s","message":"Successfully received submission info"}\n' "${status:-In Progress}" "$3"
      ;;
    *) return 90 ;;
  esac
}
"""


def function(source: str, name: str) -> str:
    match = re.search(rf"^{name}\(\) \{{\n.*?^\}}\n", source, re.MULTILINE | re.DOTALL)
    if match is None:
        raise SystemExit(f"FAIL: package-release.sh has no {name}()")
    return match.group()


def run(bash: str, helpers: str, body: str, plan: str = "") -> tuple:
    with tempfile.TemporaryDirectory(prefix="msime-notary-") as directory:
        calls = Path(directory) / "calls"
        calls.touch()
        script = "set -Eeuo pipefail\nnotary_ids=()\n" + FAKES + helpers + body
        # 使用当前解释器解析合成 JSON；不假定系统目录中有 python3。
        script = f'python3() {{ {shlex.quote(sys.executable)} -I -X utf8 "$@"; }}\n' + script
        environment = {
            # Windows 的 Python 启动器仍需通过 PATH 查找解释器动态库。
            "PATH": os.environ["PATH"] if os.name == "nt" else "/usr/bin:/bin",
            "APPLE_ID": "synthetic@example.invalid",
            "APPLE_TEAM_ID": "SYNTHETIC",
            "APPLE_APP_SPECIFIC_PASSWORD": PASSWORD,
            "MSIME_TEST_CALLS": str(calls),
            "MSIME_TEST_INFO_PLAN": plan,
        }
        if os.name == "nt":
            for key in ("SystemRoot", "TEMP", "TMP"):
                if key in os.environ:
                    environment[key] = os.environ[key]
        # Windows 要反复启动 MSYS 工具和隔离 Python；预算不改变合成重试次数或任何结果断言。
        timeout = 60 if os.name == "nt" else 20
        result = subprocess.run([bash, "-c", script], env=environment, capture_output=True, text=True, timeout=timeout)
        return result, calls.read_text().splitlines()


def main() -> int:
    bash = shutil.which("bash")
    if bash is None or not PACKAGE.is_file() or shutil.which("python3") is None:
        print("skipped: notarization flow needs bash, python3 and platforms/macos/package-release.sh")
        return 0
    source = PACKAGE.read_text(encoding="utf-8")
    helpers = function(source, "notary_field") + function(source, "notary_submit") + function(source, "notary_result")
    failures = []

    def check(condition: bool, message: str) -> None:
        if not condition:
            failures.append(message)

    # 不再逐个 --wait：提交之后接着打下一个版本。
    live = [line for line in source.splitlines() if not line.lstrip().startswith("#")]
    check(not any("notarytool submit" in line and "--wait" in line for line in live), "package-release.sh still waits on each notarization submission")
    check(any('notary_submit "$dmg"' in line for line in live), "package_edition does not submit its DMG with notary_submit")
    check(sum("staple-ticket.sh" in line for line in live) >= 1, "package-release.sh does not staple through staple-ticket.sh")

    result, calls = run(bash, helpers, 'notary_submit "/合成 目录/msime.dmg"\necho "ids=${notary_ids[*]}"\n')
    check(result.returncode == 0 and "ids=00000000-synthetic" in result.stdout, f"submit: {result.returncode} {result.stdout!r} {result.stderr!r}")
    check(calls == ["notarytool submit /合成"], f"submit calls: {calls}")

    for plan, expected, waits, sleeps in (
        ("Accepted", "Accepted", 1, 0),
        ("In Progress,In Progress,Accepted", "Accepted", 3, 2),
        ("error,Accepted", "Accepted", 2, 1),
        ("Invalid", "Invalid", 1, 0),
        ("Rejected", "Rejected", 1, 0),
        ("In Progress,In Progress,In Progress,In Progress,In Progress", "In Progress", 5, 5),
        ("error,error,error,error,error", "unknown", 5, 5),
    ):
        result, calls = run(bash, helpers, 'status="$(notary_result 00000000-synthetic)"\necho "status=$status"\n', plan)
        check(result.returncode == 0 and result.stdout.strip() == f"status={expected}", f"{plan}: expected {expected}, got {result.returncode} {result.stdout!r} {result.stderr!r}")
        check(sum(call.startswith("notarytool wait") for call in calls) == waits, f"{plan}: expected {waits} waits, calls {calls}")
        check(sum(call.startswith("sleep") for call in calls) == sleeps, f"{plan}: expected {sleeps} sleeps, calls {calls}")
        check(PASSWORD not in result.stdout + result.stderr, f"{plan}: the password reached the output")

    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        return 1
    print("macOS notarization flow: submit without waiting, wait for a final status, password kept out of the log")
    return 0


if __name__ == "__main__":
    sys.exit(main())

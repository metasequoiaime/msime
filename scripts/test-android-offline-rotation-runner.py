#!/usr/bin/env python3
"""确保离线语音旋转设备 runner 覆盖 instrumentation 退出后的方向恢复。"""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
RUNNER = ROOT / "platforms/android/tests/device/offline-voice-rotation.sh"
assert RUNNER.exists(), "缺少离线语音旋转设备 runner"
source = RUNNER.read_text()

for required in (
    "settings get system accelerometer_rotation",
    "settings get system user_rotation",
    "scenario offline-rotation",
    "settings put system accelerometer_rotation",
    "settings put system user_rotation",
):
    assert required in source, f"runner 缺少方向恢复步骤：{required}"

run_start = source.index("run_rotation_case()")
check = source.index("actual_user", run_start)
invoke = source.rindex("\nrun_rotation_case\n")
assert check < invoke, "必须先核对 instrumentation 退出后的方向，再结束 runner"
assert "trap restore EXIT" in source, "runner 必须在失败时恢复设备方向设置"
print("Android offline voice rotation runner: post-instrumentation restoration is guarded")

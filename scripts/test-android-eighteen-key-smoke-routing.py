#!/usr/bin/env python3
"""防止十八键全拼与双拼的设备验收被合并成同一次调用。"""

from pathlib import Path
import re
import shlex


ROOT = Path(__file__).resolve().parent.parent


def main() -> None:
    smoke = (ROOT / "platforms/android/tests/device/smoke.sh").read_text(encoding="utf-8")
    invocations = re.findall(
        r"^result=\$\(run_instrumentation "
        r"app\.msime\.android\.test/app\.msime\.android\.test\.FourteenKeyDeviceSmoke (.*?)\)$",
        smoke,
        re.M,
    )
    eighteen_options = []
    for invocation in invocations:
        args = shlex.split(invocation)
        options = {}
        for index in range(0, len(args), 3):
            assert args[index] == "-e" and len(args[index:index + 3]) == 3, \
                "字母组设备验收参数必须是完整的 -e 键值对"
            options[args[index + 1]] = args[index + 2]
        if any(key.startswith("eighteen_") and value == "true"
               for key, value in options.items()):
            eighteen_options.append(options)
    assert len(eighteen_options) == 2, "标准冒烟必须分别安排十八键全拼和双拼"
    assert eighteen_options[0] != eighteen_options[1], \
        "十八键全拼和双拼使用了相同参数，实际只执行了一种方案"

    runner = (ROOT / "platforms/android/tests/device/keyboard/FourteenKeyDeviceSmoke.java").read_text(encoding="utf-8")
    for options in eighteen_options:
        for key in options:
            assert f'getString("{key}")' in runner, f"设备入口没有读取参数 {key}"
    assert "FourteenKeyShuangpinDeviceChecks.runEighteen(this)" in runner, \
        "十八键双拼检查没有接到 instrumentation 入口"
    print("Android eighteen-key smoke routing contracts: passed")


if __name__ == "__main__":
    main()

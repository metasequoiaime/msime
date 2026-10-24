#!/usr/bin/env python3
"""用合成能力表验证新旧专用 AVD 的触屏选择边界，不发送输入事件。"""
from pathlib import Path
import shutil
import subprocess
import sys


def device(index, name, axes=("SLOT", "TRACKING_ID", "POSITION_X", "POSITION_Y")):
    return f'add device {index}: /dev/input/event{index}\n  name: "{name}"\n' + "".join(
        f"    ABS_MT_{axis} : value 0, min 0, max 32767, fuzz 0, flat 0\n"
        for axis in axes
    )


def main():
    awk = shutil.which("awk")
    if not awk:
        print("skipped: awk is unavailable")
        return 0
    selector = Path(__file__).resolve().parents[1] / "platforms/android/tests/device/handwriting-touch-device.awk"
    legacy = device(2, "synthetic legacy touchscreen")
    primary = device(3, "virtio_input_multi_touch_1")
    auxiliary = device(4, "virtio_input_multi_touch_2")
    keyboard = device(1, "synthetic keyboard", ())
    incomplete = device(5, "synthetic absolute pointer", ("POSITION_X", "POSITION_Y"))
    cases = [
        ("legacy touchscreen among non-touch devices", keyboard + legacy + incomplete, "/dev/input/event2"),
        ("primary among auxiliary displays", auxiliary + legacy + primary, "/dev/input/event3"),
        ("CRLF capability output", (keyboard + primary).replace("\n", "\r\n"), "/dev/input/event3"),
        ("ambiguous legacy devices", legacy + device(6, "synthetic second touchscreen"), None),
        ("auxiliary display alone", auxiliary, None),
        ("missing tracking and slot", keyboard + incomplete, None),
        ("primary name without capabilities", device(3, "virtio_input_multi_touch_1", ()), None),
        ("empty capability output", "", None),
    ]
    for name, capabilities, expected in cases:
        result = subprocess.run([awk, "-f", str(selector)], input=capabilities, text=True,
                                capture_output=True, timeout=10)
        if expected is None:
            assert result.returncode != 0 and not result.stdout.strip(), name
        else:
            assert result.returncode == 0 and result.stdout.strip() == expected, name
    print(f"Android handwriting touch device: {len(cases)} capability fixtures passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())

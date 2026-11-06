#!/usr/bin/env python3
"""macOS Caps Lock 的 HID 管理器只能往主 run loop 调度一次。

同一个 IOHIDManager 第二次调用 IOHIDManagerScheduleWithRunLoop（哪怕换一个模式）会在 IOKit 里以「Schedule failed queue: runLoop:」断言中止进程，0.58.0 的输入法因此在外接键盘按 Caps Lock 时整个崩掉（#10999）。单测把 MSIMECapsLockHIDStart 换成了计数桩，打不开真实键盘，走不到这段代码，所以在源码层面钉住：MSIMEStartCapsLockHIDTracking 里调度和反调度各只出现一次。
"""

import re
import sys
from pathlib import Path

SOURCE = Path(__file__).resolve().parents[1] / "platforms/macos/src/input/InputController.mm"


def main() -> int:
    if not SOURCE.is_file():
        print(f"skipped: {SOURCE} is missing")
        return 0
    source = SOURCE.read_text(encoding="utf-8")
    start = source.index("static void MSIMEStartCapsLockHIDTracking() {")
    end = source.index("\n}\n", start)
    body = "\n".join(line for line in source[start:end].splitlines() if not line.lstrip().startswith("//"))
    schedules = re.findall(r"IOHIDManagerScheduleWithRunLoop\(", body)
    unschedules = re.findall(r"IOHIDManagerUnscheduleFromRunLoop\(", body)
    failures = []
    if len(schedules) != 1:
        failures.append(f"MSIMEStartCapsLockHIDTracking schedules the HID manager {len(schedules)} times; a second IOHIDManagerScheduleWithRunLoop aborts the input method (#10999)")
    if len(unschedules) != 1:
        failures.append(f"MSIMEStartCapsLockHIDTracking unschedules the HID manager {len(unschedules)} times; unschedule exactly the one mode it was scheduled in")
    for failure in failures:
        print(f"FAIL: {failure}", file=sys.stderr)
    if failures:
        return 1
    print("macOS Caps Lock HID manager: scheduled once, unscheduled once")
    return 0


if __name__ == "__main__":
    sys.exit(main())

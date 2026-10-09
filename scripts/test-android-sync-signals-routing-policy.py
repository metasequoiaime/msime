#!/usr/bin/env python3
"""同步信号直接复用共享账号会话路由策略。"""

from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/account/SyncSignals.java"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    errors = []
    if "private static boolean ownsStore(" in source:
        errors.append(f"{SOURCE}: 不应保留 ownsStore 转发方法")
    if re.search(r"(?<![.\w])ownsStore\(", source):
        errors.append(f"{SOURCE}: 不应调用未限定的 ownsStore 方法")
    if source.count("AccountSessionRoutingPolicy.ownsSession(") < 2:
        errors.append(f"{SOURCE}: 同步信号应直接复用 AccountSessionRoutingPolicy.ownsSession")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android sync signals use the shared account-session routing policy")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

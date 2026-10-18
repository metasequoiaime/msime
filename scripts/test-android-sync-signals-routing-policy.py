#!/usr/bin/env python3
"""同步信号：记改动、看开关直接调 Rust（两个进程都一样），账号绑定复用共享的会话路由策略。"""

from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/account/SyncSignals.java"
PROVIDER = ROOT / "platforms/android/java/app/msime/android/account/AccountSessionProvider.java"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    provider = PROVIDER.read_text(encoding="utf-8")
    errors = []
    if "private static boolean ownsStore(" in source:
        errors.append(f"{SOURCE}: 不应保留 ownsStore 转发方法")
    if re.search(r"(?<![.\w])ownsStore\(", source):
        errors.append(f"{SOURCE}: 不应调用未限定的 ownsStore 方法")
    if "AccountSessionRoutingPolicy.ownsSession(" not in source:
        errors.append(f"{SOURCE}: 读账号绑定应直接复用 AccountSessionRoutingPolicy.ownsSession")
    # #8201 迁移计划 5d：`:ime` 记改动、看开关都直接调 Rust 的 mark_dirty 和 peek，不再经 provider 让主进程代写；主进程里只剩 Rust 一个写同步状态的地方。
    for operation in ("mark_dirty", "peek"):
        if f'.put("operation", "{operation}")' not in source:
            errors.append(f"{SOURCE}: 应调 Rust 的 {operation}")
    if "METHOD_SYNC_DIRTY" in source or "METHOD_SYNC_DIRTY" in provider or "SyncSwitch.markDirty" in provider:
        errors.append("记改动不应再经 AccountSessionProvider 绕回主进程")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android sync signals call Rust directly and share the account-session routing policy")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

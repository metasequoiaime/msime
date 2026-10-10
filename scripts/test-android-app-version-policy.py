#!/usr/bin/env python3
"""Android 应用版本名查询复用公共策略。"""

from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
JAVA = ROOT / "platforms/android/java/app/msime/android"
POLICY = JAVA / "AppVersionPolicy.java"
DEVICE_SOURCES = ROOT / "platforms/android/tests/device/editor-sources.txt"
SITES = {
    JAVA / "account/BackendAccount.java": 'AppVersionPolicy.versionName(context, "")',
    JAVA / "core/Telemetry.java": "AppVersionPolicy.versionName(app)",
    JAVA / "home/AccountFragment.java": 'AppVersionPolicy.versionName(requireContext(), "—")',
    JAVA / "home/DeveloperPage.java": 'AppVersionPolicy.versionName(context, "")',
    JAVA / "home/DeviceInfo.java": "AppVersionPolicy.current(context)",
    JAVA / "home/FeedbackPage.java": "AppVersionPolicy.current(context)",
    JAVA / "home/UpdateJobService.java": 'AppVersionPolicy.versionName(context, "0")',
}


def main() -> int:
    errors = []
    policy = POLICY.read_text(encoding="utf-8") if POLICY.exists() else ""
    if "record Version(String name, long code)" not in policy:
        errors.append("AppVersionPolicy 缺少版本名与版本码快照")
    if "Version current(Context context)" not in policy:
        errors.append("AppVersionPolicy 缺少当前应用版本快照查询")
    if "String versionName(Context context)" not in policy:
        errors.append("AppVersionPolicy 缺少保留异常语义的版本名查询")
    if "String versionName(Context context, String fallback)" not in policy:
        errors.append("AppVersionPolicy 缺少带回退值的版本名查询")
    device_sources = DEVICE_SOURCES.read_text(encoding="utf-8")
    if "platforms/android/java/app/msime/android/AppVersionPolicy.java" not in device_sources:
        errors.append("Android 设备套件源码清单缺少 AppVersionPolicy")

    for path, expected in SITES.items():
        source = path.read_text(encoding="utf-8")
        if expected not in source:
            errors.append(f"{path}: 应用版本名查询未复用 AppVersionPolicy")
        if re.search(r"\.getPackageManager\(\)\s*\.getPackageInfo\(", source):
            errors.append(f"{path}: 仍在调用点重复查询应用版本名")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android app version-name reads use the shared policy")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Android 输入视图直接复用共享的严格整数解析策略。"""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
POLICY = ROOT / "platforms/android/java/app/msime/android/core/InputViewValuePolicy.java"
SMOKE = ROOT / "platforms/android/tests/core/InputViewValuePolicySmoke.java"


def main() -> int:
    policy = POLICY.read_text(encoding="utf-8")
    smoke = SMOKE.read_text(encoding="utf-8")
    errors = []
    if "public static int integer(Object raw" in policy:
        errors.append(f"{POLICY}: 不应保留原始值整数解析转发方法")
    if "schemeValue(" in policy:
        errors.append(f"{POLICY}: 不应保留方案整数解析转发方法")
    if "return NumberPolicy.strictInt(object == null ? null : object.opt(key), fallback);" not in policy:
        errors.append(f"{POLICY}: JSONObject 字段读取应直接调用共享整数策略")
    if "import app.msime.android.NumberPolicy;" not in smoke:
        errors.append(f"{SMOKE}: 应直接导入 NumberPolicy")
    if "InputViewValuePolicy.schemeValue(" in smoke:
        errors.append(f"{SMOKE}: 不应通过方案整数转发方法检查")
    if "InputViewValuePolicy.integer(46" in smoke:
        errors.append(f"{SMOKE}: 不应通过原始值整数转发方法检查")
    if smoke.count("NumberPolicy.strictInt(") != 6:
        errors.append(f"{SMOKE}: 应直接检查 NumberPolicy.strictInt")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android input view uses the shared strict integer policy directly")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

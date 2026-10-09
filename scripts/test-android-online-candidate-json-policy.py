#!/usr/bin/env python3
"""在线候选直接复用共享 JSON 字符串与空值策略。"""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
POLICY = ROOT / "platforms/android/java/app/msime/android/candidate/OnlineCandidatePolicy.java"
SERVICE = ROOT / "platforms/android/java/app/msime/android/core/MSIMEInputService.java"
SMOKE = ROOT / "platforms/android/tests/candidate/OnlineCandidatePolicySmoke.java"


def main() -> int:
    policy = POLICY.read_text(encoding="utf-8")
    service = SERVICE.read_text(encoding="utf-8")
    smoke = SMOKE.read_text(encoding="utf-8")
    if "strictText(Object value)" in policy:
        print(f"{POLICY}: 不应保留 strictText 转发方法", file=sys.stderr)
        return 1
    if "private static String text(String value)" in policy:
        print(f"{POLICY}: 不应保留 text 空值转发方法", file=sys.stderr)
        return 1
    if "value = text(value);" in policy:
        print(f"{POLICY}: 签名字段不应通过 text 转发空值", file=sys.stderr)
        return 1
    if "value = TextPolicy.emptyIfNull(value);" not in policy:
        print(f"{POLICY}: 签名字段应直接复用共享空值策略", file=sys.stderr)
        return 1
    for path, source in ((SERVICE, service), (SMOKE, smoke)):
        if "OnlineCandidatePolicy.strictText" in source:
            print(f"{path}: 不应继续通过在线候选策略读取 JSON 字符串", file=sys.stderr)
            return 1
    for expression in (
        'JsonPolicy.strictString(message.opt("content"))',
        'JsonPolicy.strictString(entry.opt("text"))',
    ):
        if expression not in service:
            print(f"{SERVICE}: 缺少 {expression} 直接调用", file=sys.stderr)
            return 1
    if "import app.msime.android.JsonPolicy;" not in smoke:
        print(f"{SMOKE}: 应直接导入 JsonPolicy", file=sys.stderr)
        return 1
    if smoke.count("JsonPolicy.strictString") < 3:
        print(f"{SMOKE}: 缺少共享 JSON 字符串策略合同检查", file=sys.stderr)
        return 1
    print("Android 在线候选已复用共享 JSON 字符串与空值策略")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

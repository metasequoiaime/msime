#!/usr/bin/env python3
"""Android 反馈校验与页面直接复用共享文本长度策略。"""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
API = ROOT / "platforms/android/java/app/msime/android/account/FeedbackApi.java"
PAGE = ROOT / "platforms/android/java/app/msime/android/home/FeedbackPage.java"
SMOKE = ROOT / "platforms/android/tests/core/FeedbackApiSmoke.java"


def main() -> int:
    api = API.read_text(encoding="utf-8")
    page = PAGE.read_text(encoding="utf-8")
    smoke = SMOKE.read_text(encoding="utf-8")
    errors = []
    if "public static int length(" in api:
        errors.append(f"{API}: 不应保留文本长度转发方法")
    if "TextPolicy.codePointLength(text) > MAX_TEXT" not in api:
        errors.append(f"{API}: 反馈校验应直接调用共享文本长度策略")
    if "import app.msime.android.TextPolicy;" not in page:
        errors.append(f"{PAGE}: 应直接导入 TextPolicy")
    if "FeedbackApi.length(" in page or page.count("TextPolicy.codePointLength(") != 1:
        errors.append(f"{PAGE}: 字数计数应直接调用共享文本长度策略")
    if "import app.msime.android.TextPolicy;" not in smoke:
        errors.append(f"{SMOKE}: 应直接导入 TextPolicy")
    if "FeedbackApi.length(" in smoke or smoke.count("TextPolicy.codePointLength(") != 1:
        errors.append(f"{SMOKE}: 应直接检查共享文本长度策略")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android feedback uses the shared text length policy directly")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

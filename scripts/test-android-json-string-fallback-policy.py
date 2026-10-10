#!/usr/bin/env python3
"""验证 JSON 字符串指定回退值的策略由公共层统一提供。"""

from pathlib import Path
import re


ROOT = Path(__file__).resolve().parents[1]
JSON_POLICY = ROOT / "platforms/android/java/app/msime/android/JsonPolicy.java"
INPUT_VIEW = ROOT / "platforms/android/java/app/msime/android/core/InputViewValuePolicy.java"
KEYBOARD_SKIN = ROOT / "platforms/android/java/app/msime/android/keyboard/KeyboardSkin.java"


def main() -> None:
    policy = JSON_POLICY.read_text(encoding="utf-8")
    input_view = INPUT_VIEW.read_text(encoding="utf-8")
    skin = KEYBOARD_SKIN.read_text(encoding="utf-8")
    errors = []
    if "strictString(Object value, String fallback)" not in policy:
        errors.append(f"{JSON_POLICY}: 缺少带指定回退值的严格字符串策略")
    if "JsonPolicy.strictString(raw, fallback)" not in input_view:
        errors.append(f"{INPUT_VIEW}: 字符串回退仍未复用 JsonPolicy")
    for method in ("text", "textOr"):
        if f"private static String {method}(JSONObject" in skin:
            errors.append(f"{KEYBOARD_SKIN}: 仍保留 {method} JSON 字符串转发方法")
    if re.search(r"(?<![.\w])(text|textOr)\(", skin):
        errors.append(f"{KEYBOARD_SKIN}: 皮肤解析仍通过本地字符串转发方法")
    if skin.count("JsonPolicy.strictString(") < 4:
        errors.append(f"{KEYBOARD_SKIN}: 皮肤字段没有直接复用严格字符串策略")
    if errors:
        for error in errors:
            print(error)
        raise SystemExit(1)
    print("Android JSON string fallbacks use the shared policy")


if __name__ == "__main__":
    main()

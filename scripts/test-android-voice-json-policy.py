#!/usr/bin/env python3
"""验证语音配置和润色请求直接复用共享 JSON 字符串策略。"""

from pathlib import Path
import re


ROOT = Path(__file__).resolve().parents[1]
CONFIG = ROOT / "platforms/android/java/app/msime/android/voice/VoiceConfiguration.java"
SMOKE = ROOT / "platforms/android/tests/voice/VoiceConfigurationSmoke.java"
POLISH = ROOT / "platforms/android/java/app/msime/android/voice/VoicePolishPolicy.java"
POLISH_SMOKE = ROOT / "platforms/android/tests/voice/VoicePolishPolicySmoke.java"


def main() -> None:
    config = CONFIG.read_text()
    smoke = SMOKE.read_text()
    polish = POLISH.read_text()
    polish_smoke = POLISH_SMOKE.read_text()
    errors = []
    if "private static String text(Object value)" in config:
        errors.append(f"{CONFIG}: 仍保留 text 转发方法")
    if "static String strictString(Object value)" in config:
        errors.append(f"{CONFIG}: 仍保留 JSON 字符串转发方法")
    if re.search(r"(?<![.\w])text\(", config):
        errors.append(f"{CONFIG}: 语音字段仍通过 text 转发")
    if config.count("JsonPolicy.strictStringOrEmpty(") < 15:
        errors.append(f"{CONFIG}: 没有统一使用共享 JSON 字符串回退策略")
    if 'getDeclaredMethod("strictString"' in smoke:
        errors.append(f"{SMOKE}: 仍通过语音配置转发方法验证字符串策略")
    if smoke.count("JsonPolicy.strictStringOrEmpty") < 2:
        errors.append(f"{SMOKE}: 没有直接验证共享 JSON 字符串回退策略")
    if "public static String json(String value)" in polish:
        errors.append(f"{POLISH}: 仍保留 json 转发方法")
    if re.search(r"(?<![.\w])json\(", polish):
        errors.append(f"{POLISH}: 润色请求仍通过 json 转发")
    if polish.count("JsonPolicy.quote(") < 3:
        errors.append(f"{POLISH}: 没有统一使用共享 JSON 字符串引用策略")
    if "VoicePolishPolicy.json(" in polish_smoke:
        errors.append(f"{POLISH_SMOKE}: 仍通过润色策略转发方法验证 JSON 引用")
    if polish_smoke.count("JsonPolicy.quote(") < 5:
        errors.append(f"{POLISH_SMOKE}: 没有直接验证共享 JSON 字符串引用策略")
    if errors:
        for error in errors:
            print(error)
        raise SystemExit(1)
    print("Android 语音配置和润色请求已复用共享 JSON 字符串策略")


if __name__ == "__main__":
    main()

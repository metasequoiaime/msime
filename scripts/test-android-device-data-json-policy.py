#!/usr/bin/env python3
"""设备数据响应直接复用共享 JSON 字符串策略。"""

from pathlib import Path
import re


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/account/DeviceDataApi.java"


def main() -> None:
    source = SOURCE.read_text()
    errors = []
    if "private static String string(JSONObject object, String key)" in source:
        errors.append(f"{SOURCE}: 仍保留 string 转发方法")
    if re.search(r"(?<![.\w])string\(", source):
        errors.append(f"{SOURCE}: 设备数据字段仍通过 string 转发")
    if source.count("JsonPolicy.strictStringOrEmpty(") < 14:
        errors.append(f"{SOURCE}: 没有统一使用共享 JSON 字符串回退策略")
    if errors:
        for error in errors:
            print(error)
        raise SystemExit(1)
    print("Android 设备数据响应已复用共享 JSON 字符串回退策略")


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Keep the AI skin prompt bound to methods that exist on the shared Android text policy."""
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PAGE = ROOT / "platforms/android/java/app/msime/android/home/AiSkinPage.java"
POLICY = ROOT / "platforms/android/java/app/msime/android/TextPolicy.java"


def main() -> None:
    page = PAGE.read_text(encoding="utf-8")
    policy = POLICY.read_text(encoding="utf-8")
    if "TextPolicy.hasText(" in page:
        raise AssertionError("AiSkinPage calls a method that is not part of TextPolicy")
    if "!TextPolicy.blank(input.getText().toString())" not in page:
        raise AssertionError("AiSkinPage must use the shared blank-text policy")
    if "public static boolean blank(String value)" not in policy:
        raise AssertionError("TextPolicy.blank is missing")
    print("Android AI skin prompt uses the shared text policy")


if __name__ == "__main__":
    main()

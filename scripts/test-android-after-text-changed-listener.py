#!/usr/bin/env python3
"""检查 Android 文本变化回调复用共享监听器适配方法。"""

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
HOME = ROOT / "platforms/android/java/app/msime/android/home"
POLICY = ROOT / "platforms/android/java/app/msime/android/ViewPolicy.java"
CONSUMERS = {
    "SearchPill.java": 1,
    "InputDialog.java": 1,
    "AiSkinPage.java": 2,
    "FeedbackPage.java": 1,
    "KeyboardTryoutActivity.java": 1,
}


def main() -> int:
    errors = []
    policy = POLICY.read_text(encoding="utf-8")
    required = (
        "public static void afterTextChanged(TextView view, Consumer<Editable> listener)",
        "view.addTextChangedListener(new TextWatcher()",
        "@Override public void beforeTextChanged(CharSequence text, int start, int count, int after) {}",
        "@Override public void onTextChanged(CharSequence text, int start, int before, int count) {}",
        "@Override public void afterTextChanged(Editable text) { listener.accept(text); }",
    )
    for snippet in required:
        if snippet not in policy:
            errors.append(f"{POLICY}: 文本监听器适配方法缺少：{snippet}")

    for name, expected in CONSUMERS.items():
        path = HOME / name
        source = path.read_text(encoding="utf-8")
        actual = source.count("ViewPolicy.afterTextChanged(")
        if actual != expected:
            errors.append(f"{path}: 应有 {expected} 处 ViewPolicy.afterTextChanged，实际为 {actual}")
        if "new TextWatcher()" in source or "import android.text.TextWatcher;" in source:
            errors.append(f"{path}: 仍在直接实现 TextWatcher")

    for path in HOME.glob("*.java"):
        if "new TextWatcher()" in path.read_text(encoding="utf-8"):
            errors.append(f"{path}: 应复用 ViewPolicy.afterTextChanged")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android text change callbacks use the shared after-change listener")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

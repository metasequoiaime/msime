#!/usr/bin/env python3
"""Guard the debounced multi-stroke OCR contract from Apple HandwritingTests."""

from pathlib import Path


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    view = (root / "platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets").read_text()
    feedback = (root / "platforms/harmony/entry/src/main/ets/keyboard/KeyboardFeedback.ts").read_text()
    start = view.index("  private handwritingTouch(")
    touch = view[start : view.index("  private cancelHandwritingTimer()", start)]
    required = {
        "touches remain writable during OCR": "this.handwritingBusy" not in touch,
        # 防抖时长现在取用户设置的「识别等待时间」，由 `KeyboardFeedback` 限制在 200-1500 ms（默认 600），不再是源码里固定的 550。
        "source debounce is retained": "KeyboardFeedback.handwritingDelay(" in view
        and "HANDWRITING_DELAY_MIN_MS: number = 200" in feedback
        and "this.scheduleHandwritingRecognition()" in touch,
        "lift point respects the cap":
            "this.handwritingCurrent.length < HANDWRITING_MAX_POINTS" in touch,
        "changed ink invalidates old OCR": "this.handwritingRecognition.changed()" in view
        and "this.handwritingRecognition.accepts(ticket)" in view,
        "latest canvas is queued": "this.handwritingRecognition.request()" in view
        and "this.handwritingRecognition.finish()" in view,
    }
    problems = [name for name, present in required.items() if not present]
    if problems:
        for problem in problems:
            print(f"missing handwriting scheduling guard: {problem}")
        return 1
    print("harmony handwriting scheduling: multi-stroke ink stays writable and OCR is debounced")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

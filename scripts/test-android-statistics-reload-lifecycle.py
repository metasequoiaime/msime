#!/usr/bin/env python3
"""统计页回到前台时必须重试失败的跨进程读取。

唯一的例外是暂停时认证还开着（Android 9、10 的锁屏凭据确认是独立 Activity）：那次可见是否重新认证交给 AuthenticationPauseTracker 按认证结果决定，认证成功的回调自己会读，被系统取消时仍按重新进入处理。除此之外不能有别的条件把失败读取留在空状态。
"""

from pathlib import Path


SOURCE = Path(__file__).resolve().parents[1] / "platforms/android/java/app/msime/android/home/StatisticsFragment.java"


def method(source: str, header: str) -> str:
    start = source.index(header)
    indent = header[: len(header) - len(header.lstrip())]
    closing = "\n" + indent + "}"
    end = source.index(closing + "\n", start) + len(closing)
    return source[start:end].strip()


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    expected = {
        "    @Override protected void onBecameVisible()":
            "@Override protected void onBecameVisible() {\n        if (pause.reentersOnVisible(authentication != null)) reenter();\n    }",
        "    private void reenter()":
            "private void reenter() {\n        authorized = false;\n        requestAuthentication();\n    }",
        "    private void authenticationEnded(boolean systemCanceled)":
            "private void authenticationEnded(boolean systemCanceled) {\n        if (pause.reentersOnEnd(systemCanceled, isResumed() && !isHidden() && getView() != null)) reenter();\n    }",
        "    @Override public void onPause()":
            "@Override public void onPause() {\n        super.onPause();\n        pause.paused(authentication != null);\n    }",
    }
    for header, body in expected.items():
        if method(source, header) != body:
            raise SystemExit("统计页回到前台必须重试 reload()，只有跨越暂停的认证由 AuthenticationPauseTracker 按结果决定，不能把失败读取永久留在空状态")
    success = method(source, "                @Override public void onAuthenticationSucceeded(")
    if "if (getView() != null) reload();" not in success or "authenticationEnded(false);" not in success:
        raise SystemExit("统计页认证成功后必须自己读取统计，跨越暂停的那次可见才可以不重新认证")
    print("Android statistics tab retries reads whenever it becomes visible")


if __name__ == "__main__":
    main()

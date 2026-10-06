#!/usr/bin/env python3
"""检查更换 macOS 偏好目录时是否释放旧设置窗口。"""

from pathlib import Path
import sys


def main() -> int:
    source = Path(sys.argv[1]) if len(sys.argv) == 2 else Path("platforms/macos/src/settings/AppearancePreferences.mm")
    text = source.read_text(encoding="utf-8")
    marker = "- (void)setTranslationPreferencesDirectory:(NSString *)directory"
    start = text.index(marker)
    end = text.index("\n}", start) + 2
    setter = text[start:end]
    required = (
        "[translationWindow close]",
        "[translationWindow invalidatePendingCallbacks]",
        "[_aiWindow close]; _aiWindow = nil;",
    )
    missing = [item for item in required if item not in setter]
    if missing:
        raise AssertionError(f"macOS settings directory switch misses lifecycle guard(s): {missing}")
    print("macOS 设置目录切换会失效翻译回调并释放 AI 窗口")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

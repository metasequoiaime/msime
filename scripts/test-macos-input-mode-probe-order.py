#!/usr/bin/env python3
"""确保 macOS 独立设置窗口也能读到输入法模式探针。"""

from pathlib import Path


SOURCE = Path("platforms/macos/src/input/input_method_main.mm")


def main() -> None:
    source = SOURCE.read_text(encoding="utf-8")
    probe = source.index("MSIMEInputModeEnabledProbe = MSIMEInputSourceIsEnabled;")
    standalone = source.index("if (MSIMEShouldShowPreferences(argc, argv))")
    assert probe < standalone, (
        "输入法模式探针必须在独立设置窗口分支之前初始化，"
        "否则 --preferences 永远无法显示缺失入口提示"
    )


if __name__ == "__main__":
    main()

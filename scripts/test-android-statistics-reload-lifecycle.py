#!/usr/bin/env python3
"""统计页回到前台时必须重试失败的跨进程读取。"""

from pathlib import Path


SOURCE = Path(__file__).resolve().parents[1] / "platforms/android/java/app/msime/android/home/StatisticsFragment.java"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    start = source.index("    @Override protected void onBecameVisible()")
    end = source.index("    private void reload()", start)
    body = source[start:end]
    if body.strip() != "@Override protected void onBecameVisible() {\n        authorized = false;\n        requestAuthentication();\n    }":
        raise SystemExit("统计页回到前台必须无条件重试 reload()，不能把失败读取永久留在空状态")
    print("Android statistics tab retries reads whenever it becomes visible")


if __name__ == "__main__":
    main()

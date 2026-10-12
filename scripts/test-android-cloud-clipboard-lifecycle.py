#!/usr/bin/env python3
"""确保云剪贴板写操作进行时回到页面不会抢跑一次旧的读取。"""
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/home/CloudClipboardPage.java"
source = SOURCE.read_text(encoding="utf-8")

match = re.search(
    r"protected boolean shouldReloadWhenVisible\(\)\s*\{(?P<body>.*?)\n\s*\}",
    source,
    re.S,
)
if match is None:
    raise SystemExit(
        "CloudClipboardPage must suppress visibility reloads while a mutation is in flight; "
        "otherwise the read can finish before the write and leave stale content visible"
    )
if "!busy" not in match.group("body"):
    raise SystemExit("visibility reload must be skipped while CloudClipboardPage.busy is true")
print("android cloud clipboard lifecycle: visibility reload waits for in-flight mutation")

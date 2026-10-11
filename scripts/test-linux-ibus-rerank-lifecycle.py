#!/usr/bin/env python3
"""IBus settled rerank 定时器必须持有 Engine 引用直到回调销毁。"""

import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/linux/src/core/ClientEngine.cpp"
text = SOURCE.read_text(encoding="utf-8")

match = re.search(
    r"void settled_rerank_schedule\(IBusEngine \*engine\)\s*\{(?P<body>.*?)\n\}\n\nvoid translation_schedule",
    text,
    re.DOTALL,
)
if not match:
    sys.exit(f"missing settled_rerank_schedule in {SOURCE}")

body = match.group("body")
if "g_timeout_add_full" not in body:
    sys.exit("settled_rerank_schedule no longer registers a GLib timeout")

expected = "g_object_ref(engine), [](gpointer data) { g_object_unref(data); });"
if expected not in body:
    sys.exit(
        "settled rerank timeout passes a borrowed Engine pointer; its destroy notify "
        "must unref the reference held by the source"
    )

if re.search(r"\n\s*engine,\s*nullptr\);", body):
    sys.exit("settled rerank timeout still uses raw Engine data without a destroy notify")

print("linux IBus settled rerank lifecycle: timeout owns Engine until callback removal")

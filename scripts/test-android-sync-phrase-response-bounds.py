#!/usr/bin/env python3
"""Android phrase sync responses must enforce the shared phrase count cap."""
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/account/SyncApi.java"
text = SOURCE.read_text(encoding="utf-8")
start = text.index("static Phrases parsePhrases(")
end = text.index("    // ---- 词库快照 ----", start)
body = text[start:end]
assert "SyncMergePolicy.MAX_PHRASES" in body, (
    "phrase sync response parsing must enforce MAX_PHRASES"
)
assert "raw.length() > SyncMergePolicy.MAX_PHRASES" in body, (
    "oversized phrase sync responses must be rejected before allocation"
)
print("Android phrase sync responses enforce the shared count bound")

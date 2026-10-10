#!/usr/bin/env python3
"""Android diagnostics responses must bound access history while parsing."""
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/account/DiagnosticsApi.java"
text = SOURCE.read_text(encoding="utf-8")
start = text.index("JSONArray list = root.optJSONArray(\"accesses\");")
end = text.index("return new State(snapshot", start)
body = text[start:end]
assert "BoundsPolicy.atMost(accessCount, MAX_EVENTS)" in body, (
    "diagnostics access parsing must cap list allocation through the shared policy"
)
assert "BoundsPolicy.nonNegative(accessCount - MAX_EVENTS)" in body, (
    "diagnostics access parsing must retain only the newest bounded entries"
)
print("Android diagnostics access responses are bounded before list construction")

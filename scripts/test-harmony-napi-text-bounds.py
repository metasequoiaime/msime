#!/usr/bin/env python3
"""Harmony N-API text arguments must be bounded before native allocation."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/harmony/native/client_napi.cpp"
text = SOURCE.read_text(encoding="utf-8")

if "constexpr size_t kMaxTextArgumentBytes = 17 * 1024 * 1024;" not in text:
    raise SystemExit("Harmony N-API text argument limit is missing")

start = text.find("static bool argumentText(")
end = text.find("\n}\n", start)
if start < 0 or end < 0:
    raise SystemExit("Harmony N-API argumentText helper is missing")
body = text[start:end]
if "length > kMaxTextArgumentBytes" not in body:
    raise SystemExit("argumentText must reject oversized strings before assign")
if "out.assign(length, '\\0')" not in body:
    raise SystemExit("argumentText allocation marker is missing")
if body.index("length > kMaxTextArgumentBytes") > body.index("out.assign(length, '\\0')"):
    raise SystemExit("argumentText must check length before native allocation")

print("Harmony N-API text arguments are bounded before native allocation")

#!/usr/bin/env python3
"""Installing a downloaded community skin must not block the Harmony UI thread."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
NATIVE = (ROOT / "platforms/harmony/native/client_napi.cpp").read_text(encoding="utf-8")
TYPES = (ROOT / "platforms/harmony/entry/src/main/cpp/types/libmsimeclient/index.d.ts").read_text(encoding="utf-8")
SETTINGS = (ROOT / "platforms/harmony/entry/src/main/ets/pages/Settings.ets").read_text(encoding="utf-8")

if "TEXT_ENTRY(CommunitySkinInstall," in NATIVE:
    raise SystemExit("community skin installation must not run synchronously in N-API")
if "static napi_value CommunitySkinInstall(" not in NATIVE:
    raise SystemExit("community skin installation worker entry is missing")
if "communitySkinInstall: (request: string) => Promise<string>;" not in TYPES:
    raise SystemExit("community skin installation must be typed as a Promise")
if "await client.communitySkinInstall(JSON.stringify({" not in SETTINGS:
    raise SystemExit("Harmony settings must await community skin installation")

print("harmony community skin installation runs off the UI thread")

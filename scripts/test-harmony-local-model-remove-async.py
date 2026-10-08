#!/usr/bin/env python3
"""Deleting a local voice model must not block the Harmony UI thread."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
NATIVE = (ROOT / "platforms/harmony/native/client_napi.cpp").read_text(encoding="utf-8")
TYPES = (ROOT / "platforms/harmony/entry/src/main/cpp/types/libmsimeclient/index.d.ts").read_text(encoding="utf-8")
SETTINGS = (ROOT / "platforms/harmony/entry/src/main/ets/pages/Settings.ets").read_text(encoding="utf-8")

if "TEXT_ENTRY(VoiceLocalModelRemove," in NATIVE:
    raise SystemExit("local voice model removal must not run synchronously in N-API")
if "static napi_value VoiceLocalModelRemove(" not in NATIVE:
    raise SystemExit("local voice model removal worker entry is missing")
if "voiceLocalModelRemove: (request: string) => Promise<string>;" not in TYPES:
    raise SystemExit("local voice model removal must be typed as a Promise")
if "await client.voiceLocalModelRemove(JSON.stringify(request))" not in SETTINGS:
    raise SystemExit("Harmony settings must await local voice model removal")

print("harmony local voice model removal runs off the UI thread")

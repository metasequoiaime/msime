#!/usr/bin/env python3
"""Picked Harmony skin folders must be validated and copied off the UI thread."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
NATIVE = (ROOT / "platforms/harmony/native/client_napi.cpp").read_text(encoding="utf-8")
TYPES = (ROOT / "platforms/harmony/entry/src/main/cpp/types/libmsimeclient/index.d.ts").read_text(encoding="utf-8")
SETTINGS = (ROOT / "platforms/harmony/entry/src/main/ets/pages/Settings.ets").read_text(encoding="utf-8")

if "TEXT_ENTRY(SkinImport," in NATIVE:
    raise SystemExit("skin import must not run synchronously in N-API")
if "queueRequest(env, info, msime_client_skin_import" not in NATIVE:
    raise SystemExit("skin import worker entry is missing")
if "skinImport: (request: string) => Promise<string>;" not in TYPES:
    raise SystemExit("skin import must be typed as a Promise")
if "await client.skinImport(JSON.stringify({" not in SETTINGS:
    raise SystemExit("settings must await the validated skin import worker")
if "fs.copyDirSync(picked[0], skinsDirectory, 1)" in SETTINGS:
    raise SystemExit("settings must not copy an unbounded skin folder on the UI thread")

print("harmony skin folder import runs off the UI thread")

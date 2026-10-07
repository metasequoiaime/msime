#!/usr/bin/env python3
"""Large custom skin library reads and writes must stay off the Harmony UI thread."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
NATIVE = (ROOT / "platforms/harmony/native/client_napi.cpp").read_text(encoding="utf-8")
TYPES = (ROOT / "platforms/harmony/entry/src/main/cpp/types/libmsimeclient/index.d.ts").read_text(encoding="utf-8")
SETTINGS = (ROOT / "platforms/harmony/entry/src/main/ets/pages/Settings.ets").read_text(encoding="utf-8")
PAGE = (ROOT / "apps/harmony/src/main.tsx").read_text(encoding="utf-8")

if "TEXT_ENTRY(CustomSkinLibrary," in NATIVE:
    raise SystemExit("custom skin library must not run synchronously in N-API")
if "queueRequest(env, info, msime_client_custom_skin_library" not in NATIVE:
    raise SystemExit("custom skin library worker entry is missing")
if "customSkinLibrary: (request: string) => Promise<string>;" not in TYPES:
    raise SystemExit("custom skin library must be typed as a Promise")
if "this.customSkinLibrary(payload).then(deliver).catch(failed)" not in SETTINGS:
    raise SystemExit("settings request channel must deliver custom skin library replies")
if "await client.customSkinLibrary(request)" not in SETTINGS:
    raise SystemExit("settings bridge must await the custom skin library worker")
if 'bridgeRequest(native, "custom_skin_library"' not in PAGE:
    raise SystemExit("shared settings page must use the asynchronous request channel")

print("harmony custom skin library runs off the UI thread")

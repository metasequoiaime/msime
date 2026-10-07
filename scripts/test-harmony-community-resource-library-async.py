#!/usr/bin/env python3
"""Community reply template storage must not block the Harmony UI thread."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
NATIVE = (ROOT / "platforms/harmony/native/client_napi.cpp").read_text(encoding="utf-8")
TYPES = (ROOT / "platforms/harmony/entry/src/main/cpp/types/libmsimeclient/index.d.ts").read_text(encoding="utf-8")
SETTINGS = (ROOT / "platforms/harmony/entry/src/main/ets/pages/Settings.ets").read_text(encoding="utf-8")

if "TEXT_ENTRY(CommunityResourceLibrary," in NATIVE:
    raise SystemExit("community resource library must not run synchronously in N-API")
if "queueRequest(env, info, msime_client_community_resource_library" not in NATIVE:
    raise SystemExit("community resource library worker entry is missing")
if "communityResourceLibrary: (request: string) => Promise<string>;" not in TYPES:
    raise SystemExit("community resource library must be typed as a Promise")
if "await client.communityResourceLibrary(`{\"file\":" not in SETTINGS:
    raise SystemExit("community resource saves must await the worker")
if "await client.communityResourceLibrary(JSON.stringify(remove))" not in SETTINGS:
    raise SystemExit("community resource removals must await the worker")

print("harmony community resource library runs off the UI thread")

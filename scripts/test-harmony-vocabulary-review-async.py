#!/usr/bin/env python3
"""Vocabulary imports and review storage must not block the Harmony UI thread."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
NATIVE = (ROOT / "platforms/harmony/native/client_napi.cpp").read_text(encoding="utf-8")
TYPES = (ROOT / "platforms/harmony/entry/src/main/cpp/types/libmsimeclient/index.d.ts").read_text(encoding="utf-8")
SETTINGS = (ROOT / "platforms/harmony/entry/src/main/ets/pages/Settings.ets").read_text(encoding="utf-8")
PAGE = (ROOT / "apps/harmony/src/main.tsx").read_text(encoding="utf-8")

if "TEXT_ENTRY(VocabularyReview," in NATIVE:
    raise SystemExit("vocabulary review must not run synchronously in N-API")
if "queueRequest(env, info, msime_client_vocabulary_review" not in NATIVE:
    raise SystemExit("vocabulary review worker entry is missing")
if "vocabularyReview: (request: string) => Promise<string>;" not in TYPES:
    raise SystemExit("vocabulary review must be typed as a Promise")
if "this.vocabularyReview(payload).then(deliver).catch(failed)" not in SETTINGS:
    raise SystemExit("settings request channel must deliver vocabulary review replies")
if 'bridgeRequest(native, "vocabulary_review"' not in PAGE:
    raise SystemExit("shared settings page must use the asynchronous request channel")

print("harmony vocabulary review runs off the UI thread")

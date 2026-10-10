#!/usr/bin/env python3
"""Named dictionaries reach the Harmony settings page off the UI thread, and the keyboard keeps draining them."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
NATIVE = (ROOT / "platforms/harmony/native/client_napi.cpp").read_text(encoding="utf-8")
TYPES = (ROOT / "platforms/harmony/entry/src/main/cpp/types/libmsimeclient/index.d.ts").read_text(encoding="utf-8")
SETTINGS = (ROOT / "platforms/harmony/entry/src/main/ets/pages/Settings.ets").read_text(encoding="utf-8")
SESSION = (ROOT / "platforms/harmony/entry/src/main/ets/keyboard/KeyboardSession.ets").read_text(encoding="utf-8")
PAGE = (ROOT / "apps/harmony/src/main.tsx").read_text(encoding="utf-8")

checks = {
    # An import parses up to 16 MiB, so the settings page's entry is a worker; the keyboard's one-batch flush stays synchronous.
    "settings entry runs on a worker": "queueRequest(env, info, msime_client_dictionary_collections" in NATIVE,
    "both entries are registered": 'ENTRY("dictionaryCollections", DictionaryCollections)' in NATIVE
    and 'ENTRY("dictionaryCollectionsAsync", DictionaryCollectionsAsync)' in NATIVE,
    "the worker entry is typed as a Promise": "dictionaryCollectionsAsync: (request: string) => Promise<string>;" in TYPES,
    "the synchronous entry is typed": "dictionaryCollections: (request: string) => string;" in TYPES,
    "settings delivers collection replies": "this.dictionaryCollections(payload).then(deliver)" in SETTINGS,
    "the session awaits the worker": "await client.dictionaryCollectionsAsync(" in SESSION,
    # Android's keyboard sends the next batch after each personal-dictionary drain (`flushSent`); without it a large import stalls at 128 words.
    "the keyboard flushes after each drain": "this.flushDictionaryCollections(options)" in SESSION
    and '"action":{"operation":"flush"}' in SESSION,
    "the page uses the asynchronous request channel": 'bridgeRequest(native, "dictionary_collections"' in PAGE,
    "the page client is attached": "    dictionaryCollections,\n" in PAGE,
}
problems = [name for name, ok in checks.items() if not ok]
if problems:
    for problem in problems:
        print(f"harmony dictionary collections: {problem}")
    raise SystemExit(1)

print("harmony dictionary collections run off the UI thread and drain from the keyboard")

#!/usr/bin/env python3
"""The Harmony settings page can repair a preferences document an older build cannot read, instead of offering only 重新读取."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
HEADER = (ROOT / "crates/host-api/include/msime_client.h").read_text(encoding="utf-8")
NATIVE = (ROOT / "platforms/harmony/native/client_napi.cpp").read_text(encoding="utf-8")
TYPES = (ROOT / "platforms/harmony/entry/src/main/cpp/types/libmsimeclient/index.d.ts").read_text(encoding="utf-8")
SETTINGS = (ROOT / "platforms/harmony/entry/src/main/ets/pages/Settings.ets").read_text(encoding="utf-8")
PAGE = (ROOT / "apps/harmony/src/main.tsx").read_text(encoding="utf-8")

checks = {
    "the explicit repair is in the C ABI": "char *msime_client_repair_preferences(const uint8_t *directory, size_t length);" in HEADER,
    # It copies the file and rewrites it under the writer lock, so it must not run on the ArkTS thread.
    "the repair runs on a worker": "queueRequest(env, info, msime_client_repair_preferences" in NATIVE
    and 'ENTRY("repairPreferences", RepairPreferences)' in NATIVE,
    "the repair is typed as a Promise": "repairPreferences: (directory: string) => Promise<string>;" in TYPES,
    "settings delivers the repair": "this.repairPreferences().then(deliver)" in SETTINGS
    and "await client.repairPreferences(this.stateDirectory)" in SETTINGS,
    # Without `recoverPreferences` the shared page shows no 修复配置文件… button at all.
    "the page offers the repair": "recoverPreferences: async ()" in PAGE
    and 'bridgeRequest(native, "repair_preferences"' in PAGE,
}
problems = [name for name, ok in checks.items() if not ok]
if problems:
    for problem in problems:
        print(f"harmony preferences repair: {problem}")
    raise SystemExit(1)

print("harmony preferences repair: an unreadable document can be repaired from the settings page")

#!/usr/bin/env python3
"""「我的词条」经 WebDAV 网盘同步在鸿蒙上的接线：设置页经 ArkTS 桥和工作线程上的 NAPI 调 `msime_client_dictionary_sync`，在隐私政策的授权之后，状态放在自己的子目录里；键盘扩展不调它。

键盘扩展在自己的沙箱里，这个入口又没有「只用已有身份」的开关（云剪贴板的 `create_identity: false`），键盘一调就会在网盘上登记成同一台手机的第二台设备（#10029、#10032 修的正是这一类问题），所以这里守着调用点只在设置应用一侧。
"""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
ETS = ROOT / "platforms/harmony/entry/src/main/ets"
NATIVE = (ROOT / "platforms/harmony/native/client_napi.cpp").read_text(encoding="utf-8")
TYPES = (ROOT / "platforms/harmony/entry/src/main/cpp/types/libmsimeclient/index.d.ts").read_text(encoding="utf-8")
SETTINGS = (ETS / "pages/Settings.ets").read_text(encoding="utf-8")
BRIDGE = (ETS / "account/HarmonyAccountCloudBridge.ets").read_text(encoding="utf-8")
REQUESTS = (ETS / "account/HarmonyAccountRequests.ts").read_text(encoding="utf-8")
CONSENT = (ETS / "privacy/PrivacyConsentPolicy.ts").read_text(encoding="utf-8")
PAGE = (ROOT / "apps/harmony/src/main.tsx").read_text(encoding="utf-8")

start = SETTINGS.index("startRequest(kind: string, id: number, payload: string): string {")
wall = SETTINGS.index("PrivacyConsentPolicy.refuses(kind, payload", start)
dispatch = SETTINGS.find("kind === 'dictionary_sync'", start)

callers = sorted(
    str(path.relative_to(ROOT))
    for path in ETS.rglob("*")
    if path.suffix in {".ets", ".ts"} and "client.dictionarySync(" in path.read_text(encoding="utf-8")
)
keyboard_mentions = sorted(
    str(path.relative_to(ROOT))
    for path in (ETS / "keyboard").rglob("*")
    if path.suffix in {".ets", ".ts"}
    and ("dictionarySync" in path.read_text(encoding="utf-8") or "dictionary_sync" in path.read_text(encoding="utf-8"))
)

checks = {
    # 除 status 外都会阻塞网络，整次同步还持有一把文件锁。
    "the entry runs on a worker": "queueRequest(env, info, msime_client_dictionary_sync" in NATIVE,
    "the entry is registered": 'ENTRY("dictionarySync", DictionarySync)' in NATIVE,
    "the entry is typed as a Promise": "export const dictionarySync: (request: string) => Promise<string>;" in TYPES,
    "settings dispatches the request after the consent wall": dispatch > wall,
    "settings delivers the reply": "this.dictionarySync(payload).then(deliver).catch(failed)" in SETTINGS,
    "the bridge awaits the worker": "await client.dictionarySync(plan.request)" in BRIDGE,
    "only the settings bridge calls the entry": callers
    == ["platforms/harmony/entry/src/main/ets/account/HarmonyAccountCloudBridge.ets"],
    "the keyboard never reaches dictionary sync": keyboard_mentions == [],
    # files/state 里已有云剪贴板中继同名的密钥串和网盘缓存。
    "the state has its own directory": "`${stateDirectory}/dictionary-sync`" in REQUESTS,
    "every action but status needs consent": '"dictionary_sync",\n' in CONSENT
    and 'dictionary_sync: ["status"],' in CONSENT,
    "the page asks only with consent": "dictionarySync: networkAllowed" in PAGE
    and '"dictionary_sync",' in PAGE,
}
problems = [name for name, ok in checks.items() if not ok]
if problems:
    for problem in problems:
        print(f"harmony dictionary sync: {problem}")
    if callers:
        print(f"  callers of client.dictionarySync: {callers}")
    if keyboard_mentions:
        print(f"  keyboard files mentioning dictionary sync: {keyboard_mentions}")
    raise SystemExit(1)

print("harmony dictionary sync runs from the settings app only, off the UI thread and behind consent")

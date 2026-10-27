#!/usr/bin/env python3
"""钉住鸿蒙「我的词条」交换的接线：规则在 Rust 的 msime_client_user_words_mirror，时机在 UserWordsMirrorPolicy；这里守着 ArkTS 编不进 node 测试的那几处调用点。"""
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
ETS = ROOT / "platforms/harmony/entry/src/main/ets"
SESSION = ETS / "keyboard/KeyboardSession.ets"
SETTINGS = ETS / "pages/Settings.ets"
NAPI = ROOT / "platforms/harmony/native/client_napi.cpp"
TYPES = ROOT / "platforms/harmony/entry/src/main/cpp/types/libmsimeclient/index.d.ts"
HEADER = ROOT / "crates/host-api/include/msime_client.h"


def body(text: str, signature: str) -> str:
    start = text.index(signature)
    end = text.index("\n  }\n", start)
    return text[start:end]


def main() -> int:
    session = SESSION.read_text(encoding="utf-8")
    settings = SETTINGS.read_text(encoding="utf-8")
    napi = NAPI.read_text(encoding="utf-8")
    types = TYPES.read_text(encoding="utf-8")
    header = HEADER.read_text(encoding="utf-8")
    start = session.index("  start(resources:")
    mirrored = session.index("this.syncUserWords(options)", start)
    create = session.index("client.create(options)", start)
    keyboard_sync = body(session, "  private syncUserWords(options: string): boolean {")
    keyboard_publish = body(session, "  private publishUserWords(): void {")
    detach = body(session, "  detach(): void {")
    shared = body(session, "  private syncSharedState(): void {")
    settings_sync = body(settings, "  private syncUserWords(): void {")
    required = {
        # 写词库要 Engine 的独占维护权，只有建会话之前拿得到。
        "keyboard syncs before the Engine session": start < mirrored < create,
        "keyboard writes from the session thread only before a session":
            "UserWordsMirror.run(" in keyboard_sync and "'sync', 'keyboard'" in keyboard_sync,
        # 基础访问模式下键盘对共享目录只读。
        "keyboard publishes only in full mode":
            "this.sharedFullMode() ? mirror.userWordsFile('keyboard') : null" in keyboard_sync,
        "keyboard publish on blur runs on a worker without writing the dictionary":
            "UserWordsMirror.runAsync(" in keyboard_publish
            and "'publish', 'keyboard'" in keyboard_publish
            and "this.publishUserWords();" in detach,
        "a new settings publication schedules an idle rebuild":
            "this.schedulePersonalDictionaryDrain();" in shared
            and "userWordsFile('settings')" in shared,
        "settings exchanges on a worker after draining its own queue":
            "this.drainDictionaryQueues(options)" in settings_sync
            and "UserWordsMirror.runAsync(request)" in settings_sync
            and "'sync', 'settings'" in settings_sync,
        "settings exchanges on foreground and after each change":
            settings.count("this.syncUserWords();") >= 3,
        # 一台手机只有一个词库同步的设备身份：键盘从不调网盘同步。
        "keyboard never calls dictionary sync": "dictionarySync" not in session,
        "native entry registered": 'ENTRY("userWordsMirror", UserWordsMirror)' in napi
        and 'ENTRY("userWordsMirrorAsync", UserWordsMirrorAsync)' in napi
        and 'ENTRY("personalDictionarySyncAsync", PersonalDictionarySyncAsync)' in napi,
        "native entry typed": "export const userWordsMirror:" in types
        and "export const userWordsMirrorAsync:" in types
        and "export const personalDictionarySyncAsync:" in types,
        "C entry declared": "char *msime_client_user_words_mirror(" in header,
    }
    missing = [name for name, present in required.items() if not present]
    if missing:
        print("harmony user words mirror: missing " + ", ".join(missing), file=sys.stderr)
        return 1
    print("harmony user words mirror: keyboard syncs before its session, publishes only in full mode, settings drains and exchanges on a worker")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

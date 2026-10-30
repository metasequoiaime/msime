#!/usr/bin/env python3
"""鸿蒙输入法存活时，个人词库队列按有界批次继续排空。"""
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SESSION = ROOT / "platforms/harmony/entry/src/main/ets/keyboard/KeyboardSession.ets"
STORE = ROOT / "crates/client-services/src/dictionary/personal.rs"
SETTINGS = ROOT / "platforms/harmony/entry/src/main/ets/pages/Settings.ets"
POLICY = ROOT / "platforms/harmony/entry/src/main/ets/keyboard/DictionaryMaintenancePolicy.ts"
HOST = ROOT / "crates/host-api/src/dictionary.rs"
RETRY_POLICY = SESSION.parent / "IdleDictionaryRetryPolicy.ts"


def main() -> int:
    session = SESSION.read_text(encoding="utf-8")
    store = STORE.read_text(encoding="utf-8")
    settings = SETTINGS.read_text(encoding="utf-8")
    policy = POLICY.read_text(encoding="utf-8")
    host = HOST.read_text(encoding="utf-8")
    retry_policy = RETRY_POLICY.read_text(encoding="utf-8") if RETRY_POLICY.exists() else ""
    start = session.index("  start(resources:")
    drain = session.index("= this.drainPersonalDictionary(options)", start)
    create = session.index("client.create(options)", drain)
    scheduler = session[
        session.index("  private schedulePersonalDictionaryDrain()") : session.index(
            "\n  /** Whether the Engine's output",
            session.index("  private schedulePersonalDictionaryDrain()"),
        )
    ]
    queued = session[
        session.index("      if (decision.queued)") : session.index(
            "    } catch (error)", session.index("      if (decision.queued)")
        )
    ]
    required = {
        "bounded native batch": ".synchronize_batch_reporting_present_with_source(" in host
        and "self.synchronize_batch(MAX_ACTIVE_REQUESTS, apply, source, page)" in store
        and "self.synchronize_batch(MAX_ACTIVE_REQUESTS, apply, || None, page)" in store
        and ".take(limit)" in store,
        "idle sync before Engine session": start < drain < create,
        "drain result adopted": "this.personalDictionarySettled = this.drainPersonalDictionary(options)" in session
        or "const drained: PersonalDictionaryDrain = this.drainPersonalDictionary(options)" in session
        and "this.personalDictionarySettled = drained === 'settled' && mirrored" in session
        and "const mirrored: boolean = this.syncUserWords(options)" in session,
        "two-second yield": "IDLE_DICTIONARY_RETRY_MS: number = 2000" in session
        or "IDLE_DICTIONARY_RETRY_MS: number = 2000" in retry_policy
        and "IdleDictionaryRetryPolicy.delay(this.personalDictionaryStalls)" in scheduler,
        # 组合输入和本地引擎模式都不能排空队列；直接检查实际状态，让 ArkTS 编译器也能发现已移除的方法调用。
        "composition and local-mode guard": "this.composing() || this.localMode !== 'none'" in scheduler,
        "Engine session release": "this.restartIdleSession('personal dictionary queue')" in scheduler,
        "mode restoration": "client.setEnglishMode" in scheduler
        and "client.setKeyGrid" in scheduler,
        "remaining work rescheduled": "this.schedulePersonalDictionaryDrain();" in scheduler,
        "new queue work noticed": "this.personalDictionarySettled = false" in queued
        and "this.schedulePersonalDictionaryDrain();" in queued,
        "shutdown cancels timer": "clearTimeout(this.personalDictionaryTimer)" in session,
        "cloud download names queue operation": "operation: 'queue_edit'" in settings,
        "cloud download policy is queued": '"queue_edit"' in policy,
        "cloud download reaches native queue": "Operation::QueueEdit" in host
        and "| Operation::QueueEdit" in host,
    }
    missing = [name for name, present in required.items() if not present]
    if missing:
        print("harmony personal dictionary: missing " + ", ".join(missing), file=sys.stderr)
        return 1
    print("harmony personal dictionary: bounded batches resume while idle")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

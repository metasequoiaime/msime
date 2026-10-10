#!/usr/bin/env python3
"""Keep Harmony's bounded personal-dictionary queue draining while the IME stays alive."""
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SESSION = ROOT / "platforms/harmony/entry/src/main/ets/keyboard/KeyboardSession.ets"
STORE = ROOT / "crates/client-core/src/dictionary/personal.rs"
SETTINGS = ROOT / "platforms/harmony/entry/src/main/ets/pages/Settings.ets"
POLICY = ROOT / "platforms/harmony/entry/src/main/ets/keyboard/DictionaryMaintenancePolicy.ts"
HOST = ROOT / "crates/host-api/src/dictionary.rs"


def main() -> int:
    session = SESSION.read_text(encoding="utf-8")
    store = STORE.read_text(encoding="utf-8")
    settings = SETTINGS.read_text(encoding="utf-8")
    policy = POLICY.read_text(encoding="utf-8")
    host = HOST.read_text(encoding="utf-8")
    start = session.index("  start(resources:")
    drain = session.index("this.personalDictionarySettled = this.drainPersonalDictionary(options)")
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
        "four-entry native batch": ".take(4)" in store,
        "idle sync before Engine session": start < drain < create,
        "two-second yield": "IDLE_DICTIONARY_RETRY_MS: number = 2000" in session,
        # Keep the queue out of both an active composition and a local Engine mode. The old
        # helper name was removed from KeyboardSession; checking the actual state also lets the
        # ArkTS compiler catch stale method calls instead of preserving one for this guard.
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

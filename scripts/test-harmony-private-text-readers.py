#!/usr/bin/env python3
"""Harmony private text readers must validate and read through one descriptor."""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
ACCOUNT = ROOT / "platforms/harmony/entry/src/main/ets/account/HarmonyAccountTransport.ets"
SESSION = ROOT / "platforms/harmony/entry/src/main/ets/keyboard/KeyboardSession.ets"
STAGED = ROOT / "platforms/harmony/entry/src/main/ets/keyboard/StagedResources.ets"
TELEMETRY = ROOT / "platforms/harmony/entry/src/main/ets/telemetry/Telemetry.ets"


def main() -> int:
    account = ACCOUNT.read_text(encoding="utf-8")
    account_region = account[account.index("  load(): string | null {") : account.index("\n  save(value: string): void", account.index("  load(): string | null {"))]
    session = SESSION.read_text(encoding="utf-8")
    staged = STAGED.read_text(encoding="utf-8")
    staged_region = staged[staged.index("  private static readMarker(") : staged.index("\n  /** Remove a staged tree", staged.index("  private static readMarker("))]
    telemetry = TELEMETRY.read_text(encoding="utf-8")
    telemetry_region = telemetry[telemetry.index("  private static readRememberedPid(") : telemetry.index("\n  private static rememberPid", telemetry.index("  private static readRememberedPid("))]
    helper_marker = "  private static readPrivateText("
    helper_start = session.find(helper_marker)
    helper_end = session.find("\n  /** Generates bounded reply candidates", helper_start)
    helper = session[helper_start:helper_end] if helper_start >= 0 and helper_end >= 0 else ""
    required_helper = (
        "safePrivateFile(path, maximumBytes)",
        "fs.openSync",
        "fs.statSync(handle.fd)",
        "stat.ino !== expected.ino",
        "fs.readSync",
        "fs.closeSync",
        "isFile",
    )
    missing = [token for token in required_helper if token not in helper]
    if not helper:
        missing.append("readPrivateText helper")
    if "fs.readTextSync(this.file)" in account_region:
        missing.append("account descriptor read")
    for label, region in (("staged marker", staged_region), ("telemetry marker", telemetry_region)):
        for token in ("fs.openSync", "fs.statSync(handle.fd)", "fs.readSync", "fs.closeSync"):
            if token not in region:
                missing.append(f"{label}: {token}")
        for token in ("fs.lstatSync", "stat.ino !== expected.ino"):
            if token not in region:
                missing.append(f"{label}: {token}")
    for token in ("fs.lstatSync(this.file)", "stat.ino !== expected.ino"):
        if token not in account_region:
            missing.append(f"account reader: {token}")
    exclusive_region = account[account.index("  async exclusive<T>") : account.index("\n  load(): string | null", account.index("  async exclusive<T>"))]
    for token in ("fs.statSync(lock.fd)", "opened.ino !== pathStat.ino"):
        if token not in exclusive_region:
            missing.append(f"account lock: {token}")
    if "fs.readTextSync(marker)" in staged_region:
        missing.append("staged marker path read")
    if "fs.readTextSync(path)" in telemetry_region:
        missing.append("telemetry marker path read")
    for call in (
        "return KeyboardFeedback.parse(fs.readTextSync(path));",
        "return KeyboardFeedback.parse(fs.readTextSync(this.feedbackFile()));",
        "return EmojiCatalogModel.parseRecents(fs.readTextSync(file));",
        "return CommunityReplyLibraryPolicy.parse(fs.readTextSync(file));",
    ):
        if call in session:
            missing.append(call)
    if missing:
        print("Harmony private text readers missing " + ", ".join(missing), file=sys.stderr)
        return 1
    print("Harmony private text readers use descriptor validation")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

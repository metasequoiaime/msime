#!/usr/bin/env python3
"""Check that no HarmonyOS host static method is reachable only from its own tests.

`test-harmony-unwired-policies.py` asks the same question one level up: *does anything but a test
reach this file*. That check is green for a file the application imports heavily — and a file can be
imported heavily while one of the methods on it has never been called by anything but the suite.

That is not hypothetical. `JapaneseNineKeyLayout.digitKeys()` and `digitBrackets()` were written to
the source's shape, unit-tested, and never called: the digit layer handed over to the twenty-six key
symbol rows instead, which is the opposite of what the Apple host asserts. `JapaneseNineKeyLayout.ts`
is imported by the view, so the file-level check passed the whole time. Widening it to symbols turned
up twenty more, among them every label for the shortcut bar, which meant four of its seven buttons
announced nothing at all to a screen reader.

The rule is the same one the file-level check uses, so read the two together: a ported policy with
tests and no caller is not shipped behaviour, whatever the assertion count says.

Two things this deliberately does not do.

It matches `Owner.method(` rather than `.method(`, because a bare name collides: `CandidateGlossPolicy
.isCurrent` looked wired for as long as `HarmonyDoubaoRecognizer` had a private method of the same
name. The first version of this scan reported fourteen where there were twenty-one.

It attributes each method to the class whose body encloses it rather than to the first class in the
file, because several files export more than one — `ClipboardHistoryStore.ts` exports six — and
getting that wrong reported six methods of `ClipboardHistoryError` that do not exist.

There are two lists, and the difference between them is the whole point.

`ALLOWED` is for a symbol this platform will never call: the question it answers is one HarmonyOS
does not ask. Those are permanent, and each carries the difference that makes it so.

`PENDING` is for a symbol that should be called and is not yet, named with what it actually is. It
is not a place to put something rather than look at it — an entry only goes in after the gap behind
it has been diagnosed, and the diagnosis is the text. Writing "not done yet" is not an entry.

Both are ratchets. A name on either list that becomes reachable fails the check, so neither can rot
into a list of lies, and a symbol that appears in neither fails immediately. `PENDING` is expected to
reach zero; `ALLOWED` is not.

It is empty as of 2026-09-21. Of the six it started with, three were wired — the Japanese digit
layer's brackets, and the pair that lets a scheme the settings page turned off be left — and three
turned out to be platform differences once the question was answered rather than deferred, which is
why they are in `ALLOWED` with what settled them. An empty `PENDING` says this one class is closed:
no host static is reachable only from its own tests. It does not say the port is complete, and the
axes in docs/harmony-parity.md are where that question lives.
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SOURCES = ROOT / "platforms/harmony/entry/src/main/ets"
TESTS = ROOT / "platforms/harmony/tests/keyboard-logic.test.ts"

CLASS = re.compile(r"^\s*(?:export\s+)?(?:abstract\s+)?class\s+([A-Za-z_][A-Za-z0-9_]*)")
STATIC = re.compile(r"^\s{2}static\s+(?:readonly\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*\(")

# Symbol -> why this host has no call site for it. Only a platform difference belongs here.
ALLOWED: dict[str, str] = {
    "CandidateManagementAction.fromMenuItemId":
        "maps an OS menu item id back to an action; this host draws the menu itself and passes the "
        "action object, so no integer ever comes back",
    "CandidateManagementAction.fixedPosition":
        "answers the slot for FIX_FIRST, which belongs to the four-item menu; this host shows the "
        "five-slot menu, whose entries carry their own position",
    "InputDiagnosticPolicy.visible":
        "normalize(value).length > 0, and the view holds an already-normalized string, so the "
        "condition it would replace is the same expression",
    "KeyboardGeometry.halfGapPixels":
        "the Android candidate separator is drawn at half the gap in pixels; this host lays out in "
        "vp and draws no separator",
    "LocalInputMode.fromTrigger":
        "Android intercepts the trigger letter itself; here the Engine consumes it and reports the "
        "mode it entered, which is what the host reads",
    "KeyboardScheme.fromHostSelection":
        "reads a selection in the host-API shape; this host reads the preference document directly",
    "ReturnKeyAction.shouldPerformEditorAction":
        "Android splits Return into performEditorAction and a committed newline, and this is that "
        "split; HarmonyOS takes the enter key type in sendKeyFunction and handles every value "
        "itself, ENTER_KEY_TYPE_NEW_LINE included. Measured on the emulator: wiring the split in "
        "replaced a working framework newline with an insertText that the editor ignored",
    "CandidateGlossPolicy.token":
        "a three-field staleness token, session/generation/epoch. This host threads (handle, epoch) "
        "and mints the epoch from TranslationPolicy.signature, which itself carries the composition "
        "generation — so the third field is already inside the second by construction, and the host "
        "guard additionally checks handle != 0, which the policy cannot see. Threading the token "
        "would compare the generation twice",
    "CandidateGlossPolicy.isCurrent":
        "the reply side of that token, unused for the same reason",
    "FloatingToolbarLayout.allComponents":
        "every component on. It is not this host's default and wiring it would be wrong: the "
        "prepared toolbar record ships screen_keyboard false, because HarmonyOS has a screen "
        "keyboard of its own, so turning this on by default would add a button nobody asked for",
    "VoiceCaptureDevicePolicy.match":
        "matches against already-built VoiceCaptureDevice values; the platform hands this host raw "
        "AudioDeviceDescriptors, which HarmonyVoiceCaptureDevices matches by stableId instead",
}

# Symbol -> what the gap behind it actually is. Expected to reach zero.
PENDING: dict[str, str] = {
    "AppEdition.of":
        "parses an edition a build declares; HarmonyOS has a single product today (build-profile.json5 "
        "`default`, the full edition), so AppEdition.current() returns FULL without parsing anything. "
        "It is called once a per-edition product passes its id, schemes and default scheme as build "
        "parameters for current() to read; see the 产品版本 section of platforms/harmony/README.md",
}


def statics(text: str) -> list[tuple[str, str]]:
    """Every static method, attributed to the class whose body encloses it."""
    found: list[tuple[str, str]] = []
    owner: str | None = None
    depth = 0
    for line in text.splitlines():
        opened = CLASS.match(line)
        if opened is not None and depth == 0:
            owner = opened.group(1)
        declared = STATIC.match(line)
        if declared is not None and owner is not None:
            found.append((owner, declared.group(1)))
        depth = max(0, depth + line.count("{") - line.count("}"))
    return found


def main() -> int:
    sources = {
        path: path.read_text(encoding="utf-8")
        for path in sorted(list(SOURCES.rglob("*.ts")) + list(SOURCES.rglob("*.ets")))
    }
    if not sources:
        print(f"no HarmonyOS host sources under {SOURCES}", file=sys.stderr)
        return 1
    tests = TESTS.read_text(encoding="utf-8")

    unwired: list[tuple[str, str]] = []
    total = 0
    for path, text in sources.items():
        for owner, method in statics(text):
            total += 1
            name = f"{owner}.{method}"
            call = re.compile(rf"\b{re.escape(owner)}\s*\.\s*{re.escape(method)}\s*\(")
            if any(other != path and call.search(body) for other, body in sources.items()):
                listed = "ALLOWED" if name in ALLOWED else "PENDING" if name in PENDING else None
                if listed is not None:
                    print(
                        f"{name} is listed in {listed} but something now calls it; "
                        f"remove the entry",
                        file=sys.stderr,
                    )
                    return 1
                continue
            # Inside its own class a static is reached as `this.method(` as often as by name. That
            # form only type-checks from a static context of the same class, so looking for it in
            # the declaring file is enough to tell a real call from a coincidence.
            inner = re.compile(rf"\bthis\s*\.\s*{re.escape(method)}\s*\(")
            if call.search(text) or inner.search(text):
                continue
            if name in ALLOWED or name in PENDING:
                continue
            label = name if call.search(tests) else f"{name} (not even tested)"
            unwired.append((label, path.relative_to(SOURCES).as_posix()))

    if unwired:
        print("HarmonyOS host statics that nothing but the tests can reach:", file=sys.stderr)
        for name, path in unwired:
            print(f"  {name}  [{path}]", file=sys.stderr)
        print(
            "\nCall it from the host, or record it: ALLOWED with the platform difference that means "
            "it is never called here, PENDING with what the gap behind it actually is.",
            file=sys.stderr,
        )
        return 1

    print(
        f"harmony unwired symbols: {total} host statics, "
        f"{len(ALLOWED)} explained by the platform, {len(PENDING)} diagnosed and pending"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Check HarmonyOS host sources against the ArkTS rules `tsc` does not enforce.

`platforms/harmony/tests/run.sh` type-checks the ported logic with `tsc`, and `tsc` accepts the
whole TypeScript language. The HarmonyOS application is not compiled by `tsc` — `hvigorw
assembleHap` runs the ArkTS compiler, which accepts a deliberately smaller language. Code can
therefore type-check, pass every unit test, build the settings bundle, pass `verify-local.sh
--quick`, merge, and still not produce a HAP.

That is not hypothetical. Six merged pull requests left `develop` in exactly that state: thirteen
ArkTS errors across three `.ets` files, none of which any gate looked at, found only when a HAP was
finally assembled for a device.

Assembling a HAP here is not an option: it needs DevEco Studio or the command-line SDK, the
OpenHarmony NDK, cross-compiled natives and a 180 MB dictionary release. This checks the three
rules those thirteen errors fell under, which are the ones ordinary TypeScript habits walk into:

  arkts-no-any-unknown       `any` and `unknown` are not types here.
  arkts-no-aliases-by-index  `T["member"]` is not a type.

The third rule those errors fell under, arkts-no-untyped-obj-literals, is deliberately **not**
checked. Whether a literal needs a named type depends on the type of what receives it — ArkTS
accepts `JSON.stringify({ ok: false, error: x })` and rejects the same call with a literal nested
inside it — so a text-only rule either misses the real cases or reports two hundred that the
compiler accepts. Both were tried. That rule needs the real compiler, and this file says so rather
than pretending otherwise.

Only `.ets` files. That is not a simplification: the same build that rejected thirteen `.ets`
constructions compiled `AccountCloudBridge.ts` unchanged, and that file is full of `unknown`. The
strict subset applies to the ArkTS sources, and the plain TypeScript beside them - the ported
policies, which are also what `platforms/harmony/tests` compiles under tsc - is checked as
TypeScript. Widening this to `.ts` produced hundreds of findings the real compiler does not make.

It is a subset of the real compiler and does not replace it. It is the part that can run on any
machine, and it would have caught all thirteen.

It happened again: ten more errors from four later pull requests, in five `.ets` files, found the
same way. Seven of them fell under three rules that are readable from the text alone, in the narrow
forms checked here, with no finding on code the compiler accepts:

  arkts-no-spread                 `{ ...other }`: an object literal that starts by spreading.
  arkts-limited-throw             `throw error;` inside `catch (error)`: the binding has no type,
                                  so it must be rethrown `as Error`.
  arkts-no-obj-literals-as-types  `{ fd: number }` as a parameter or return type.

Of the other three, two were a missing import and one an untyped object literal, which only the
compiler can see.
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
# Only the application's own sources. `tests/` is compiled by tsc on purpose and is not shipped.
SOURCES = ROOT / "platforms/harmony/entry/src/main/ets"


def strip_noise(text: str) -> str:
    """Blank out comments and string bodies so their contents cannot look like code."""
    out = []
    index = 0
    length = len(text)
    while index < length:
        pair = text[index : index + 2]
        if pair == "//":
            end = text.find("\n", index)
            end = length if end < 0 else end
            out.append(" " * (end - index))
            index = end
        elif pair == "/*":
            end = text.find("*/", index + 2)
            end = length if end < 0 else end + 2
            # Keep newlines so reported line numbers stay right.
            out.append("".join(c if c == "\n" else " " for c in text[index:end]))
            index = end
        elif text[index] in "\"'`":
            quote = text[index]
            end = index + 1
            while end < length and text[end] != quote:
                if text[end] == "\\":
                    end += 1
                end += 1
            end = min(end + 1, length)
            out.append("".join(c if c == "\n" else " " for c in text[index:end]))
            index = end
        else:
            out.append(text[index])
            index += 1
    return "".join(out)


# `unknown`/`any` in a type position: after a colon, an `as`, inside a generic, or in a union.
# Requiring the prefix is what keeps a variable named `any` from being reported as a type — the
# first version of this did exactly that, on code the real compiler had just accepted.
BANNED_TYPE = re.compile(r"(?:[:<|&,]|\bas\b)\s*(unknown|any)(?![A-Za-z0-9_$])")
# `Something["member"]` in a type position: after `:`, `as`, `<`, or an arrow return.
INDEXED_TYPE = re.compile(r"(?::\s*|\bas\s+|<)\s*[A-Z][A-Za-z0-9_]*\s*\[\s*['\"]")
# An object literal whose first member is a spread. A spread later in the literal looks the same as
# one in an argument list, so only the leading form is read; it is the one copying code writes.
OBJECT_SPREAD = re.compile(r"\{\s*\.\.\.")
CATCH = re.compile(r"\bcatch\s*\(\s*([A-Za-z_$][A-Za-z0-9_$]*)\s*\)\s*\{")
# An object literal type on a parameter or a return. `key: {` elsewhere is just as often a value
# inside an object literal, which ArkTS accepts, so those positions are left to the compiler.
LITERAL_PARAMETER_TYPE = re.compile(r"\(\s*[A-Za-z_$][A-Za-z0-9_$]*\s*\??\s*:\s*\{\s*[A-Za-z_$][A-Za-z0-9_$]*\s*\??\s*:")
LITERAL_RETURN_TYPE = re.compile(r"\)\s*:\s*\{\s*[A-Za-z_$][A-Za-z0-9_$]*\s*\??\s*:")


def block_end(text: str, open_brace: int) -> int:
    """Index just past the brace that closes the one at `open_brace`."""
    depth = 0
    for index in range(open_brace, len(text)):
        if text[index] == "{":
            depth += 1
        elif text[index] == "}":
            depth -= 1
            if depth == 0:
                return index + 1
    return len(text)


def line_of(text: str, index: int) -> int:
    return text.count("\n", 0, index) + 1


def findings(path: pathlib.Path) -> list[tuple[int, str]]:
    text = strip_noise(path.read_text(encoding="utf-8"))
    found: list[tuple[int, str]] = []
    for number, line in enumerate(text.splitlines(), start=1):
        for match in BANNED_TYPE.finditer(line):
            found.append((number, f'"{match.group(1)}" is not a type in ArkTS (arkts-no-any-unknown)'))
        if INDEXED_TYPE.search(line):
            found.append((number, "indexed access type (arkts-no-aliases-by-index)"))
    for match in OBJECT_SPREAD.finditer(text):
        found.append((line_of(text, match.start()), "object spread (arkts-no-spread)"))
    for pattern in (LITERAL_PARAMETER_TYPE, LITERAL_RETURN_TYPE):
        for match in pattern.finditer(text):
            found.append((
                line_of(text, match.start()),
                "object literal used as a type (arkts-no-obj-literals-as-types)",
            ))
    for match in CATCH.finditer(text):
        name = re.escape(match.group(1))
        start = match.end() - 1
        body = text[start : block_end(text, start)]
        for rethrow in re.finditer(rf"\bthrow\s+{name}\s*;", body):
            found.append((
                line_of(text, start + rethrow.start()),
                f"untyped catch binding rethrown; write `throw {match.group(1)} as Error` (arkts-limited-throw)",
            ))
    return sorted(found)


def main() -> int:
    if not SOURCES.is_dir():
        print(f"harmony ArkTS subset: {SOURCES} is missing", file=sys.stderr)
        return 1
    files = sorted(SOURCES.rglob("*.ets"))
    if not files:
        print("harmony ArkTS subset: no sources found; the extraction is wrong", file=sys.stderr)
        return 1
    problems = 0
    for path in files:
        for number, reason in findings(path):
            print(f"{path.relative_to(ROOT)}:{number}: {reason}", file=sys.stderr)
            problems += 1
    if problems:
        print(
            f"harmony ArkTS subset: {problems} construction(s) the ArkTS compiler rejects; "
            "tsc accepts them and the HAP will not build",
            file=sys.stderr,
        )
        return 1
    print(f"harmony ArkTS subset: {len(files)} host sources use only constructions ArkTS accepts")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

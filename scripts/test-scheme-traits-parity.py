#!/usr/bin/env python3
"""Scheme traits agree across the engine, the macOS, Windows and Linux hosts and the settings page.

The engine's `SchemeType` const fns (crates/engine/src/types.rs) are the one source of truth for what differs between input schemes, and input-runtime and host-api call them directly. Two places cannot: the macOS controller, the Windows Server and TIP, and the Linux IBus and Fcitx5 hosts decide from a view's `scheme` number in C++, so each platform's `InputSchemeTraits.h` copies the predicates the view does not publish, and the settings page is TypeScript, so it keeps its own lists of which schemes are Chinese. Each copy compiles and passes its own tests on its own values, and a scheme added or moved on one side alone shows up only as a key that behaves like the wrong language.

The Android input service decides from the same number in Java, so `InputSchemeTraits.java` copies the predicates it needs the same way and is checked the same way as the header.

The HarmonyOS keyboard decides from the number in ArkTS, so `SchemeTraits.ts` copies them too, each predicate written as the list of schemes it is true for, and its `NAMES` are the engine's wire names by ordinal.

This reads all of them and checks:

- each header's, the Android copy's and the Harmony copy's scheme constants are the engine ordinals;
- every header function that mirrors a `SchemeType` predicate, either by being named after it in CamelCase or by a comment starting with that predicate's name in backticks, answers the same as the engine for every scheme, and false for a number the engine does not know;
- every Harmony predicate is a list of scheme constants, and the one named after a `SchemeType` predicate in camelCase answers the same as the engine, and false for an unknown number;
- the Harmony `NAMES` are the engine's wire names in ordinal order;
- each header's host-only `OpensCandidateList` is the engine's `has_openable_candidate_list`, which is the same list under a host name;
- the page's `chineseInputSchemeOptions` and `nonChineseSchemes` split the engine's schemes by `is_chinese`, in engine order, and `knownInputSchemes` names every scheme;
- the page's `InputScheme` and `ChineseScheme` types, and client-core's `InputScheme` and `ChineseScheme` enums they mirror, name the same schemes in engine order.
"""

from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
ENGINE = ROOT / "crates/engine/src/types.rs"
HEADERS = (
    ROOT / "platforms/macos/src/input/InputSchemeTraits.h",
    ROOT / "platforms/windows/common/InputSchemeTraits.h",
    ROOT / "platforms/linux/src/core/InputSchemeTraits.h",
)
ANDROID = ROOT / "platforms/android/java/app/msime/android/policy/InputSchemeTraits.java"
HARMONY = ROOT / "platforms/harmony/entry/src/main/ets/keyboard/SchemeTraits.ts"
OPTIONS = ROOT / "packages/ui/src/settings/input-scheme-options.ts"
UI_TYPES = ROOT / "packages/ui/src/index.tsx"
PREFERENCES = ROOT / "crates/client-core/src/preferences.rs"

# Host-only header functions that are an engine predicate under another name.
HOST_ALIASES = {"OpensCandidateList": "has_openable_candidate_list"}

# Scheme numbers no build knows, which every header trait must answer false for.
UNKNOWN_SCHEMES = (-1, 10, 255)


def rel(path: pathlib.Path) -> str:
    return str(path.relative_to(ROOT))


def snake_case(name: str) -> str:
    return re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower()


def block(text: str, header: str) -> str | None:
    """The body of the first `{ ... }` after `header`, braces balanced."""
    start = text.find(header)
    if start < 0:
        return None
    open_at = text.find("{", start)
    depth = 0
    for index in range(open_at, len(text)):
        if text[index] == "{":
            depth += 1
        elif text[index] == "}":
            depth -= 1
            if depth == 0:
                return text[open_at + 1 : index]
    return None


class Engine:
    def __init__(self, text: str, errors: list[str]) -> None:
        body = block(text, "pub enum SchemeType")
        # Variant -> ordinal, in declaration order.
        self.ordinals: dict[str, int] = {}
        for match in re.finditer(r"^\s*(\w+)\s*=\s*(\d+)\s*,", body or "", re.M):
            self.ordinals[match.group(1)] = int(match.group(2))
        if not self.ordinals:
            errors.append(f"{rel(ENGINE)}: no `pub enum SchemeType` with explicit ordinals")
            self.names: dict[str, str] = {}
            self.predicates: dict[str, set[str]] = {}
            return
        impl = block(text, "impl SchemeType")
        name_body = block(impl or "", "pub fn name(self)") or ""
        self.names = dict(re.findall(r"Self::(\w+)\s*=>\s*\"(\w+)\"", name_body))
        if set(self.names) != set(self.ordinals):
            errors.append(f"{rel(ENGINE)}: `SchemeType::name` does not name every variant")
        # Predicate -> the variants it is true for.
        self.predicates = {}
        for match in re.finditer(r"pub const fn (\w+)\(self\) -> bool", impl or ""):
            predicate = match.group(1)
            arms = block(impl[match.start() :], match.group(0)) or ""
            true_for: set[str] = set()
            seen: list[str] = []
            for arm in re.finditer(
                r"(?P<variants>Self::\w+(?:[ \t\r\n]*\|[ \t\r\n]*Self::\w+)*)"
                r"[ \t\r\n]*=>[ \t\r\n]*\{?[ \t\r\n]*(?P<result>true|false)",
                arms,
            ):
                variants = re.findall(r"Self::(\w+)", arm.group(1))
                seen.extend(variants)
                if arm.group("result") == "true":
                    true_for.update(variants)
            if sorted(seen) != sorted(self.ordinals):
                errors.append(f"{rel(ENGINE)}: `{predicate}` does not decide every variant exactly once in a readable match")
                continue
            self.predicates[predicate] = true_for
        if not self.predicates:
            errors.append(f"{rel(ENGINE)}: no `pub const fn <trait>(self) -> bool` predicates on SchemeType")

    def by_ordinal(self) -> list[str]:
        return sorted(self.ordinals, key=self.ordinals.__getitem__)

    def wire_names(self, keep=lambda variant: True) -> list[str]:
        return [self.names[variant] for variant in self.by_ordinal() if keep(variant)]


class Header:
    constant_pattern = r"constexpr int (\w+) = (\d+);"
    function_pattern = r"((?:^//[^\n]*\n)*)^constexpr bool (\w+)\(int scheme\)\s*\{\s*return\s+(.*?);\s*\}"
    missing = "no `constexpr bool <Trait>(int scheme)` functions"

    def __init__(self, path: pathlib.Path, errors: list[str]) -> None:
        self.path = path
        text = path.read_text(encoding="utf-8")
        self.constants = {name: int(value) for name, value in re.findall(self.constant_pattern, text)}
        # Function -> (the engine predicate its comment names or None, its boolean expression).
        self.functions: dict[str, tuple[str | None, str]] = {}
        for match in re.compile(self.function_pattern, re.M | re.S).finditer(text):
            comment = match.group(1).strip()
            mirrored = re.match(r"//\s*`(\w+)`:", comment.splitlines()[-1].strip()) if comment else None
            self.functions[match.group(2)] = (mirrored.group(1) if mirrored else None, " ".join(match.group(3).split()))
        if not self.functions:
            errors.append(f"{rel(self.path)}: {self.missing}")

    def scheme_constants(self) -> dict[str, int]:
        """The scheme constants under the engine's CamelCase variant names."""
        return self.constants

    def evaluate(self, function: str, scheme: int, depth: int = 0) -> bool:
        expression = self.functions[function][1]
        if not re.fullmatch(r"[\w\s()=|&!]+", expression) or depth > len(self.functions):
            raise ValueError(f"`{function}` is not a plain boolean expression over `scheme`: {expression}")

        def call(match: re.Match[str]) -> str:
            if match.group(1) not in self.functions:
                raise ValueError(f"`{function}` calls `{match.group(1)}`, which is not a trait in this header")
            return str(self.evaluate(match.group(1), scheme, depth + 1))

        python = re.sub(r"\b(\w+)\(scheme\)", call, expression)
        python = python.replace("||", " or ").replace("&&", " and ")
        python = re.sub(r"!(?!=)", " not ", python)
        names = {**self.constants, "scheme": scheme, "True": True, "False": False}
        for word in re.findall(r"[A-Za-z_]\w*", python):
            if word not in names and word not in ("or", "and", "not"):
                raise ValueError(f"`{function}` uses `{word}`, which is neither `scheme` nor a scheme constant")
        return bool(eval(python, {"__builtins__": {}}, names))


class AndroidTraits(Header):
    """The Android copy: `static final int QUANPIN = 0;` constants and `static boolean isChinese(int scheme) { return ...; }` traits, each optionally preceded by `// `predicate`: ...` comment lines."""

    constant_pattern = r"static final int (\w+) = (\d+);"
    function_pattern = r"((?:^[ \t]*//[^\n]*\n)*)^[ \t]*public static boolean (\w+)\(int scheme\)\s*\{\s*return\s+(.*?);\s*\}"
    missing = "no `public static boolean <trait>(int scheme)` functions"

    def scheme_constants(self) -> dict[str, int]:
        return {"".join(part.capitalize() for part in name.split("_")): value for name, value in self.constants.items()}


class HarmonyTraits(AndroidTraits):
    """The Harmony copy: `static readonly QUANPIN: number = 0;` constants and `static isChinese(scheme: number): boolean { return [SchemeTraits.QUANPIN, ...].includes(scheme); }` predicates, read as the equivalent `scheme == QUANPIN || ...` expression. Like the Android copy it has no host aliases and its constants are the variant names in upper snake case."""

    constant_pattern = r"static readonly (\w+): number = (\d+);"
    function_pattern = r"^[ \t]*static (\w+)\(scheme: number\): boolean \{\s*return \[(.*?)\]\.includes\(scheme\);\s*\}"
    missing = "no `static <trait>(scheme: number): boolean` predicates"

    def __init__(self, path: pathlib.Path, errors: list[str]) -> None:
        self.path = path
        text = path.read_text(encoding="utf-8")
        self.constants = {name: int(value) for name, value in re.findall(self.constant_pattern, text)}
        self.functions = {}
        for match in re.compile(self.function_pattern, re.M | re.S).finditer(text):
            elements = [element.strip() for element in match.group(2).split(",") if element.strip()]
            # An element that is not a scheme constant is kept as written, so evaluating it reports it rather than skipping it.
            terms = [f"scheme == {element[len('SchemeTraits.') :]}" if re.fullmatch(r"SchemeTraits\.\w+", element) else element for element in elements]
            self.functions[match.group(1)] = (None, " || ".join(terms) or "False")
        declared = re.findall(r"^[ \t]*static (\w+)\(scheme: number\): boolean", text, re.M)
        for function in declared:
            if function not in self.functions:
                errors.append(f"{rel(self.path)}: `{function}` is not written as `return [SchemeTraits.X, ...].includes(scheme);`, so it cannot be checked")
        if not self.functions:
            errors.append(f"{rel(self.path)}: {self.missing}")
        names = re.search(r"static readonly NAMES: string\[\] = \[(.*?)\];", text, re.S)
        self.names = re.findall(r"\"([a-z_]+)\"", names.group(1)) if names else None


def ts_string_list(text: str, name: str) -> list[str] | None:
    """An exported array's string elements, or its option objects' `value`s."""
    match = re.search(rf"export const {name}\b[^=]*=\s*\[(.*?)\]", text, re.S)
    if not match:
        return None
    values = re.findall(r"value:\s*\"([a-z_]+)\"", match.group(1))
    return values or re.findall(r"\"([a-z_]+)\"", match.group(1))


def ts_union(text: str, name: str) -> list[str] | None:
    match = re.search(rf"export type {name}\s*=\s*([^;]*);", text)
    return re.findall(r"\"([a-z_]+)\"", match.group(1)) if match else None


def rust_enum_names(text: str, name: str) -> list[str] | None:
    body = block(text, f"pub enum {name} {{")
    if body is None:
        return None
    body = re.sub(r"//[^\n]*", "", body)
    return [snake_case(variant) for variant in re.findall(r"^\s*(\w+)\s*,", body, re.M)]


def check_header(engine: Engine, header: Header, errors: list[str]) -> int:
    for variant, ordinal in engine.ordinals.items():
        constant = "Japanese" if variant == "JapaneseRomaji" else variant
        if header.scheme_constants().get(constant) != ordinal:
            errors.append(f"{rel(header.path)}: `{constant}` should be {ordinal}, the engine's `SchemeType::{variant}`, found {header.scheme_constants().get(constant)}")
    extra = set(header.scheme_constants()) - {("Japanese" if v == "JapaneseRomaji" else v) for v in engine.ordinals}
    for constant in sorted(extra):
        errors.append(f"{rel(header.path)}: scheme constant `{constant}` has no engine `SchemeType` variant")

    compared = 0
    compared_functions: set[str] = set()
    for function, (mirrored, _) in header.functions.items():
        # A function named after an engine predicate is compared whatever its comment says, so dropping or rewording the comment cannot turn the comparison off.
        named = snake_case(function) if snake_case(function) in engine.predicates else None
        predicate = mirrored or named or HOST_ALIASES.get(function)
        if predicate is None:
            continue
        if predicate not in engine.predicates:
            errors.append(f"{rel(header.path)}: `{function}` mirrors `{predicate}`, which is not a SchemeType predicate")
            continue
        if mirrored and snake_case(function) != predicate:
            errors.append(f"{rel(header.path)}: `{function}` mirrors `{predicate}`, so it should be named after it")
        compared += 1
        compared_functions.add(function)
        try:
            for variant, ordinal in engine.ordinals.items():
                want = variant in engine.predicates[predicate]
                if header.evaluate(function, ordinal) != want:
                    errors.append(f"{rel(header.path)}: `{function}({ordinal})` is {not want}, but the engine's `SchemeType::{variant}.{predicate}()` is {want}")
            for unknown in UNKNOWN_SCHEMES:
                if header.evaluate(function, unknown):
                    errors.append(f"{rel(header.path)}: `{function}({unknown})` is true for a scheme no build knows")
        except ValueError as error:
            errors.append(f"{rel(header.path)}: {error}")
    # The aliases are the macOS header's names; the Android and Harmony copies name every trait after its predicate.
    for function in () if isinstance(header, AndroidTraits) else HOST_ALIASES:
        if function not in header.functions:
            errors.append(f"{rel(header.path)}: `{function}` is gone; drop it from HOST_ALIASES here if the host no longer needs it")
    if compared == 0:
        errors.append(f"{rel(header.path)}: no function is documented as mirroring a SchemeType predicate")
    # Host-only traits still have to answer false for an unknown number.
    for function in header.functions:
        if function in compared_functions:
            continue
        try:
            for unknown in UNKNOWN_SCHEMES:
                if header.evaluate(function, unknown):
                    errors.append(f"{rel(header.path)}: `{function}({unknown})` is true for a scheme no build knows")
        except ValueError as error:
            errors.append(f"{rel(header.path)}: {error}")
    return compared


def compare(errors: list[str], where: str, found: list[str] | None, want: list[str]) -> None:
    if found is None:
        errors.append(f"{where}: not found")
    elif found != want:
        errors.append(f"{where}: {found}, expected {want} (the engine's schemes in ordinal order)")


def main() -> int:
    inputs = (ENGINE, *HEADERS, OPTIONS, UI_TYPES, PREFERENCES)
    missing = [rel(path) for path in inputs if not path.is_file()]
    if missing:
        print(f"skipped: {', '.join(missing)} missing")
        return 0

    errors: list[str] = []
    engine = Engine(ENGINE.read_text(encoding="utf-8"), errors)
    if errors:
        for error in errors:
            print(f"FAIL {error}", file=sys.stderr)
        return 1
    if "is_chinese" not in engine.predicates:
        print(f"FAIL {rel(ENGINE)}: SchemeType has no `is_chinese` predicate to split the page's lists by", file=sys.stderr)
        return 1

    compared = {rel(path): check_header(engine, Header(path, errors), errors) for path in HEADERS}
    if ANDROID.is_file():
        compared[rel(ANDROID)] = check_header(engine, AndroidTraits(ANDROID, errors), errors)
    harmony_traits: HarmonyTraits | None = None
    if HARMONY.is_file():
        harmony_traits = HarmonyTraits(HARMONY, errors)
        compared[rel(HARMONY)] = check_header(engine, harmony_traits, errors)

    chinese = engine.predicates["is_chinese"]
    all_names = engine.wire_names()
    chinese_names = engine.wire_names(lambda variant: variant in chinese)
    other_names = engine.wire_names(lambda variant: variant not in chinese)

    if harmony_traits is not None:
        compare(errors, f"{rel(HARMONY)} NAMES", harmony_traits.names, all_names)

    options = OPTIONS.read_text(encoding="utf-8")
    compare(errors, f"{rel(OPTIONS)} chineseInputSchemeOptions", ts_string_list(options, "chineseInputSchemeOptions"), chinese_names)
    compare(errors, f"{rel(OPTIONS)} nonChineseSchemes", ts_string_list(options, "nonChineseSchemes"), other_names)
    base = ts_string_list(options, "baseInputSchemes")
    known = re.search(r"const knownInputSchemes\b[^=]*=\s*\[(.*?)\]", options, re.S)
    if base is None or known is None:
        errors.append(f"{rel(OPTIONS)}: baseInputSchemes or knownInputSchemes not found")
    else:
        spread = "...baseInputSchemes" in known.group(1)
        listed = (base if spread else []) + re.findall(r"\"([a-z_]+)\"", known.group(1))
        if sorted(listed) != sorted(all_names) or len(set(listed)) != len(listed):
            errors.append(f"{rel(OPTIONS)} knownInputSchemes: {listed}, expected every engine scheme once: {all_names}")

    ui_types = UI_TYPES.read_text(encoding="utf-8")
    compare(errors, f"{rel(UI_TYPES)} type InputScheme", ts_union(ui_types, "InputScheme"), all_names)
    compare(errors, f"{rel(UI_TYPES)} type ChineseScheme", ts_union(ui_types, "ChineseScheme"), chinese_names)

    preferences = PREFERENCES.read_text(encoding="utf-8")
    compare(errors, f"{rel(PREFERENCES)} enum InputScheme", rust_enum_names(preferences, "InputScheme"), all_names)
    compare(errors, f"{rel(PREFERENCES)} enum ChineseScheme", rust_enum_names(preferences, "ChineseScheme"), chinese_names)

    if errors:
        for error in errors:
            print(f"FAIL {error}", file=sys.stderr)
        return 1
    print(
        f"scheme traits: {len(engine.ordinals)} schemes; "
        + "; ".join(f"{count} traits in {path}" for path, count in compared.items())
        + " match the engine predicates they mirror;"
        f" the settings page and client-core split them {len(chinese_names)} Chinese / {len(other_names)} other as `is_chinese` does"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Keep the settings page's Preferences shape and the Rust document in step.

设置页能写、Rust 偏好结构里却没有字段的键，是一个永远不生效的开关。偏好结构以前带 `deny_unknown_fields`，这样的键会让整次保存失败，三个智能标点子开关就因此让设置窗口什么都存不下来；现在偏好文档容忍并保留本版本不认识的键（见 `.agents/notes/implemented/architecture/2026-10-15-preferences-keep-unknown-fields.md`），保存不再失败，键原样留在文档里，但没有任何代码读它。失败变得悄无声息，所以这道检查仍然必要。

A field only Rust has is the milder direction: a setting nobody can reach from
the settings page. Those exist on purpose sometimes, so they are listed here
with the reason rather than being reported.

Types are paired two ways. Same-named types are compared as before, and every Rust struct reachable from `Preferences` is also paired with whatever the page declares for the field that holds it - a named type, possibly from another file under `packages/ui/src`, or an inline object such as `diagnostic_log?: { server?: boolean; tsf?: boolean }`. Pairing by name alone left seven reachable structs uncompared because the page spells them inline, and a reachable struct with no page counterpart at all now fails instead of being skipped. Struct-level `rename_all = "camelCase"` is applied to the Rust side, since that is the spelling the page has to write.
"""

from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
UI = ROOT / "packages/ui/src/index.tsx"
RUST = ROOT / "crates/client-core/src/preferences.rs"

# Rust fields with no control on the settings page, and why.
RUST_ONLY: dict[str, set[str]] = {
    "Preferences": {
        # 智能标点默认开的一次性迁移标记，不是设置：读入时由 client-core 设上，保存时只增不减，设置页不显示也不写它。
        "smart_punctuation_migrated",
        # 快捷符号当前只有 iOS 原生设置页提供编辑，其他宿主保留共享字段，不在 React 设置页重复增加入口。
        "touch_quick_punctuation_symbols",
        # 大屏竖屏分离只在 Android 原生键盘设置页提供，其他宿主保留共享字段，React 设置页不显示。
        "touch_keyboard_split_portrait",
        # 日语九键连点切换只在 Android 原生输入设置页提供，其他宿主保留共享字段，React 设置页不显示。
        "touch_japanese_toggle_input",
        # 文本编辑触控板仅由 Android 原生设置页呈现，其他宿主保留文档字段，不在 React 页提供无效入口。
        "android_text_trackpad",
        # 上屏后联想先在 Android 原生键盘验收，其他宿主保留字段，尚无共享设置入口。
        "next_word_prediction",

    },
    "KeybindingPreferences": set(),
    "TouchToolbarPreferences": {
        # 撤销与重做仅在 Android 原生工具栏设置页提供入口，其他宿主保留共享字段，React 设置页不显示。
        "undo",
        "redo",
    },
    "SentenceAssociationPreferences": {
        # octagram 搭配调参系数：与 Windows `[association]` 同名七键一致，两边都不进设置页，只在 preferences.json 里手调。
        "sentence_collocation_weight",
        "sentence_collocation_max_length",
        "sentence_collocation_min_length",
        "sentence_collocation_penalty",
        "sentence_collocation_non_penalty",
        "sentence_collocation_weak_penalty",
        "sentence_collocation_rear_penalty",
    },
}


MEMBER = re.compile(r"\s*(\w+)\??\s*:")
TS_OBJECT_DECLARATION = re.compile(r"export type (\w+) = ((?:\w+\s*&\s*)*)\{")


def strip_comments(source: str) -> str:
    return re.sub(r"/\*.*?\*/|//[^\n]*", "", source, flags=re.S)


def ts_members(body: str) -> dict[str, str]:
    """An object type body's own members -> their type text, skipping nested objects' members."""
    members: dict[str, str] = {}
    body = strip_comments(body)
    index, depth = 0, 0
    while index < len(body):
        char = body[index]
        if char in "{([<":
            depth += 1
        elif char in "})]>":
            depth -= 1
        elif depth == 0:
            # A member starts at the beginning of the body or after a `;` or a newline.
            match = MEMBER.match(body, index) if index == 0 or body[index - 1] in ";\n" else None
            if match:
                start, level = match.end(), 0
                end = start
                while end < len(body):
                    c = body[end]
                    if c in "{([<":
                        level += 1
                    elif c in "})]>":
                        if level == 0:
                            break
                        level -= 1
                    elif level == 0 and c == ";":
                        break
                    elif level == 0 and c == "\n" and re.match(r"\s*(?:\w+\??\s*:|$)", body[end + 1 :]):
                        break
                    end += 1
                members[match.group(1)] = body[start:end].strip()
                index = end
                continue
        index += 1
    return members


def ts_union_members(source: str) -> dict[str, list[str]]:
    """`export type Name = "a" | "b";` -> its string-literal members."""
    out: dict[str, list[str]] = {}
    for name, body in re.findall(r"export type (\w+) =([^;{]*);", source):
        literals = re.findall(r'"([^"]+)"', body)
        if literals:
            out[name] = literals
    return out


def ts_record_bodies(source: str) -> dict[str, str]:
    """`export type Name = Record<Union, V>` -> an object body over that union.

    The mapped spelling has no braces to match, so it has to be written out as the
    object it stands for: each string member of the union becomes a key. Without
    this, a field the page declares that way (`local_modes?: LocalModePreferences`
    is one) reads as a type this check cannot see, and a reachable struct goes
    uncompared.
    """
    unions = ts_union_members(source)
    out: dict[str, str] = {}
    for match in re.finditer(r"export type (\w+) = Record<\s*(\w+)\s*,\s*([^>;]+?)\s*>", source):
        members = unions.get(match.group(2))
        if members:
            value = match.group(3)
            out[match.group(1)] = "\n".join(f"  {member}: {value};" for member in members)
    return out


def ts_object_bodies(source: str) -> dict[str, str]:
    """读取对象声明自身的成员；命名交叉部分在模块类型收集完毕后展开。"""
    out: dict[str, str] = {}
    for match in TS_OBJECT_DECLARATION.finditer(source):
        start = match.end() - 1
        depth = 0
        for index in range(start, len(source)):
            if source[index] == "{":
                depth += 1
            elif source[index] == "}":
                depth -= 1
                if depth == 0:
                    out[match.group(1)] = source[start + 1 : index]
                    break
    out.update(ts_record_bodies(source))
    return out


def ts_types(source: str) -> dict[str, set[str]]:
    """Type name -> its own top-level keys.

    Brace matching rather than a line-anchored pattern: these declarations come
    both as one line and as many, and a regex that assumes the multi-line shape
    runs past the closing brace and collects whatever follows. The bodies come from
    `ts_object_bodies`, so the mapped spelling is read here too.
    """
    out: dict[str, set[str]] = {}
    for name, body in ts_object_bodies(source).items():
        # Only this type's own keys; a nested inline object is reached through
        # whichever named type owns it, not counted twice here.
        keys: set[str] = set()
        depth = 0
        for piece in re.split(r"([{}])", body):
            if piece == "{":
                depth += 1
                continue
            if piece == "}":
                depth -= 1
                continue
            if depth == 0:
                keys.update(re.findall(r"(?:^|[;\n])\s*(\w+)\??:", piece))
        out[name] = keys
    return out


def rust_structs(source: str) -> dict[str, set[str]]:
    out: dict[str, set[str]] = {}
    pattern = r"((?:#\[[^\n]*\]\s*\n)*)pub struct (\w+)\s*\{(.*?)\n\}\n"
    for attributes, name, body in re.findall(pattern, source, re.S):
        rename_all = 'rename_all = "camelCase"' in attributes
        fields: set[str] = set()
        field_pattern = r"((?:#\[[^\n]*\]\s*\n)*)\s*pub (\w+):"
        for field_attributes, field in re.findall(field_pattern, body):
            explicit = re.search(r'rename\s*=\s*"([^"]+)"', field_attributes)
            if explicit:
                fields.add(explicit.group(1))
            elif rename_all:
                fields.add(camel(field))
            else:
                fields.add(field)
        out[name] = fields
    return out


def camel(name: str) -> str:
    head, *rest = name.split("_")
    return head + "".join(part[:1].upper() + part[1:] for part in rest)


def rust_field_types(source: str) -> dict[str, dict[str, str]]:
    """Struct -> {serialized field name -> declared type}, with struct-level camelCase renaming applied."""
    out: dict[str, dict[str, str]] = {}
    pattern = r"((?:#\[[^\n]*\]\s*\n)*)pub struct (\w+)\s*\{(.*?)\n\}\n"
    for attributes, name, body in re.findall(pattern, source, re.S):
        rename = camel if 'rename_all = "camelCase"' in attributes else (lambda field: field)
        out[name] = {
            rename(field): kind.strip()
            for field, kind in re.findall(r"^\s*pub (\w+):\s*([^\n]+?),?\s*$", body, re.M)
        }
    return out


def reachable_pairs(
    rust: dict[str, dict[str, str]], ts: dict[str, str]
) -> tuple[list[tuple[str, str, str]], list[str]]:
    """Walk `Preferences` field by field. Returns (Rust struct, page type label, page body) pairs for every struct reached, and the reachable structs the page has no object type for."""
    pairs: list[tuple[str, str, str]] = []
    missing: list[str] = []
    seen: set[tuple[str, str]] = set()
    queue = [("Preferences", "Preferences", ts["Preferences"])]
    while queue:
        struct, label, body = queue.pop(0)
        if (struct, label) in seen:
            continue
        seen.add((struct, label))
        pairs.append((struct, label, body))
        members = ts_members(body)
        for field, kind in sorted(rust[struct].items()):
            nested = [word for word in re.findall(r"\w+", kind) if word in rust]
            if not nested or field not in members:
                # A missing member is reported by the key comparison itself.
                continue
            page_kind = members[field]
            where = f"{struct}.{field}"
            if page_kind.startswith("{"):
                queue.append((nested[-1], f"{label}.{field} (inline)", page_kind[1:-1]))
                continue
            named = [word for word in re.findall(r"\w+", page_kind) if word in ts]
            if named:
                queue.append((nested[-1], named[0], ts[named[0]]))
            else:
                missing.append(
                    f"{where} holds {nested[-1]}, but the page declares it as `{page_kind}`, "
                    "which is neither an inline object nor a type this check can read"
                )
    return pairs, missing


def page_object_bodies() -> dict[str, str]:
    """收集入口及其他模块中唯一命名的对象，递归展开命名交叉类型。"""
    source = UI.read_text(encoding="utf-8")
    bodies = ts_object_bodies(source)
    bases = {match[1]: re.findall(r"\w+", match[2]) for match in TS_OBJECT_DECLARATION.finditer(source)}
    elsewhere: dict[str, list[tuple[str, list[str]]]] = {}
    for path in sorted(UI.parent.rglob("*")):
        if path == UI or path.suffix not in {".ts", ".tsx"} or not path.is_file():
            continue
        source = path.read_text(encoding="utf-8")
        inherited = {match[1]: re.findall(r"\w+", match[2]) for match in TS_OBJECT_DECLARATION.finditer(source)}
        for name, body in ts_object_bodies(source).items():
            elsewhere.setdefault(name, []).append((body, inherited.get(name, [])))
    for name, found in elsewhere.items():
        if name not in bodies and len(found) == 1:
            bodies[name], bases[name] = found[0]

    resolved: dict[str, str] = {}

    def expand(name: str, visiting: set[str]) -> str:
        if name in resolved:
            return resolved[name]
        if name in visiting or name not in bodies:
            raise ValueError(f"Cannot resolve intersection object type: {name}")
        inherited = [expand(base, visiting | {name}) for base in bases.get(name, [])]
        resolved[name] = ";\n".join([*inherited, bodies[name]])
        return resolved[name]

    # 只展开偏好可达的组合；无关 UI 组件可能继承这里不解析的接口。
    pending = ["Preferences"] if "Preferences" in bodies else []
    visited: set[str] = set()
    while pending:
        name = pending.pop()
        if name in visited:
            continue
        visited.add(name)
        body = expand(name, set())
        for kind in ts_members(body).values():
            pending.extend(word for word in re.findall(r"\w+", kind) if word in bodies)
    return {name: body for name, body in bodies.items() if not bases.get(name)} | resolved


def main() -> int:
    try:
        page_bodies = page_object_bodies()
    except ValueError as error:
        print(str(error), file=sys.stderr)
        return 1
    # Preferences imports several object types from sibling files under
    # packages/ui/src. Build the same type index used for reachable pairs so
    # those imported declarations are available for the top-level comparison.
    ts = {name: set(ts_members(body)) for name, body in page_bodies.items()}
    rust_source = RUST.read_text(encoding="utf-8")
    rust = rust_structs(rust_source)
    if "Preferences" not in ts or "Preferences" not in rust:
        print("Preferences type not found; the parser needs updating", file=sys.stderr)
        return 1

    failures = []
    checked = 0

    def compare(name: str, label: str, page: set[str], fields: set[str]) -> None:
        unmapped = sorted(page - fields)
        if unmapped:
            failures.append(
                f"{label}: the settings page can write {unmapped}, which "
                f"client_core::preferences::{name} has no field for. "
                "The key is kept in the document as unknown and never takes effect."
            )
        unreachable = sorted(fields - page - RUST_ONLY.get(name, set()))
        if unreachable:
            failures.append(
                f"{label}: {unreachable} exist in client_core::preferences::{name} with no "
                "control on the settings page. Add one, or record the reason in RUST_ONLY."
            )

    compared: set[tuple[str, str]] = set()
    for name in sorted(set(ts) & set(rust)):
        checked += 1
        compared.add((name, name))
        compare(name, name, ts[name], rust[name])

    # Structs reached through a Preferences field, paired with what the page declares for that field. Same-named pairs were compared above.
    typed = rust_field_types(rust_source)
    pairs, missing = reachable_pairs(typed, page_bodies)
    for name, label, body in pairs:
        if (name, label) in compared:
            continue
        compared.add((name, label))
        checked += 1
        compare(name, label if label == name else f"{name} (page: {label})", set(ts_members(body)), set(typed[name]))
    failures.extend(missing)

    for failure in failures:
        print(f"FAIL {failure}", file=sys.stderr)
    if failures:
        return 1
    print(f"Preferences field parity: {checked} shared types agree")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

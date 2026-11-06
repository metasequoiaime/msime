#!/usr/bin/env python3
"""候选字重 `candidate_font_weight`（#10748）的取值、OpenType 数值和预编辑行下限以 client-core 的 `CandidateFontWeight` 为准；macOS、Windows、Linux 三个候选窗各自抄了一份字重表，这里核对三份都与 Rust 一致（见 .agents/notes/implemented/feature/2026-11-03-candidate-font-weight.md）。"""

from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
RUST = ROOT / "crates/client-core/src/preferences.rs"
WINDOWS = ROOT / "platforms/windows/src/candidate/CandidateFontWeight.h"
MACOS = ROOT / "platforms/macos/src/candidate/CandidateFontWeight.h"
LINUX = ROOT / "platforms/linux/src/candidates/CandidateFontPolicy.h"
# AppKit 的字重常量对应的 OpenType 数值。
NS_FONT_WEIGHTS = {"Regular": 400, "Medium": 500, "Semibold": 600, "Bold": 700}


def snake_case(name: str) -> str:
    return re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower()


def one(pattern: str, text: str, what: str, flags: int = re.S) -> re.Match:
    matches = list(re.finditer(pattern, text, flags))
    if len(matches) != 1:
        raise ValueError(f"找不到唯一的 {what}（{len(matches)} 处）")
    return matches[0]


def rust_rules() -> tuple[dict[str, int], str, int]:
    text = RUST.read_text(encoding="utf-8")
    body = one(r"pub enum CandidateFontWeight \{(.*?)\n\}", text, "CandidateFontWeight 枚举").group(1)
    variants = re.findall(r"^\s*(?:#\[default\]\s*)?([A-Z]\w*),", body, re.M)
    default = one(r"#\[default\]\s*([A-Z]\w*),", body, "#[default] 取值").group(1)
    impl = one(r"impl CandidateFontWeight \{(.*?)\n\}", text, "impl CandidateFontWeight").group(1)
    values = {name: int(value) for name, value in re.findall(r"Self::(\w+) => (\d+),", impl)}
    if set(values) != set(variants):
        raise ValueError(f"value() 覆盖的取值 {sorted(values)} 与枚举 {variants} 不同")
    floor = one(r"if value > (\d+) \{\s*value\s*\} else \{\s*(\d+)\s*\}", impl, "preedit_value 的下限")
    if floor.group(1) != floor.group(2):
        raise ValueError("preedit_value 的比较值和下限不同")
    return {snake_case(name): values[name] for name in variants}, snake_case(default), int(floor.group(1))


def windows_rules() -> tuple[dict[str, int], int]:
    text = WINDOWS.read_text(encoding="utf-8")
    table = one(r"candidate_font_weights\{\{(.*?)\}\};", text, "candidate_font_weights 表").group(1)
    weights = {name: int(value) for name, value in re.findall(r'\{"(\w+)", (\d+), L"[^"]*"\}', table)}
    floor = one(r"return weight > (\d+) \? weight : (\d+);", text, "candidate_preedit_font_weight")
    if floor.group(1) != floor.group(2):
        raise ValueError("Windows 预编辑下限的比较值和下限不同")
    regular = int(one(r"candidate_regular_font_weight = (\d+);", text, "candidate_regular_font_weight").group(1))
    return weights | {"<default>": regular}, int(floor.group(1))


def macos_rules() -> tuple[dict[str, int], int]:
    text = MACOS.read_text(encoding="utf-8")
    ids = re.findall(r'@"(\w+)"', one(r"MSIMECandidateFontWeightIds\(\) \{(.*?)\n\}", text, "MSIMECandidateFontWeightIds").group(1))
    body = one(r"NSFontWeight MSIMECandidateFontWeightValue\(NSString \*weight\) \{(.*?)\n\}", text, "MSIMECandidateFontWeightValue").group(1)
    mapped = {name: NS_FONT_WEIGHTS[constant] for name, constant in re.findall(r'isEqual:@"(\w+)"\]\) return NSFontWeight(\w+);', body)}
    default = NS_FONT_WEIGHTS[one(r"\n\s*return NSFontWeight(\w+);", body, "MSIMECandidateFontWeightValue 的缺省").group(1)]
    weights = {name: mapped.get(name, default) for name in ids}
    if set(mapped) - set(ids):
        raise ValueError(f"MSIMECandidateFontWeightValue 认的取值 {sorted(mapped)} 不在 MSIMECandidateFontWeightIds 里")
    floor = NS_FONT_WEIGHTS[one(r"MAX\(NSFontWeight(\w+), MSIMECandidateFontWeightValue\(weight\)\)", text, "MSIMECandidatePreeditFontWeightValue").group(1)]
    return weights | {"<default>": default}, floor


def linux_rules() -> tuple[dict[str, int], int, set[int]]:
    text = LINUX.read_text(encoding="utf-8")
    body = one(r"int candidate_font_weight_value\(std::string_view id\) \{(.*?)\n\}", text, "candidate_font_weight_value").group(1)
    mapped = {name: int(value) for name, value in re.findall(r'if \(id == "(\w+)"\) return (\d+);', body)}
    default = int(one(r"\n\s*return (\d+);", body, "candidate_font_weight_value 的缺省").group(1))
    floor = one(r"candidate_preedit_font_weight\(int weight\) \{ return weight > (\d+) \? weight : (\d+); \}", text, "candidate_preedit_font_weight")
    if floor.group(1) != floor.group(2):
        raise ValueError("Linux 预编辑下限的比较值和下限不同")
    words = one(r"candidate_font_weight_word\(int weight\) \{(.*?)\n\}", text, "candidate_font_weight_word").group(1)
    worded = {int(value) for value in re.findall(r"case (\d+): return \"\w[\w-]*\";", words)}
    return mapped | {"<default>": default}, int(floor.group(1)), worded


def main() -> int:
    errors: list[str] = []
    try:
        weights, default, floor = rust_rules()
        expected = {name: value for name, value in weights.items() if name != default} | {"<default>": weights[default]}
        full = weights | {"<default>": weights[default]}
        windows, windows_floor = windows_rules()
        macos, macos_floor = macos_rules()
        linux, linux_floor, linux_words = linux_rules()
    except (ValueError, KeyError) as error:
        print(f"候选字重的字重表读不出来: {error}", file=sys.stderr)
        return 1
    # Windows 和 macOS 列全部取值（含常规），Linux 只列非常规的取值、其余按缺省。
    for name, table, wanted in (("Windows", windows, full), ("macOS", macos, full), ("Linux", linux, expected)):
        if table != wanted:
            errors.append(f"{name} 的字重表 {table} 与 client-core 的 CandidateFontWeight {wanted} 不同")
    for name, value in (("Windows", windows_floor), ("macOS", macos_floor), ("Linux", linux_floor)):
        if value != floor:
            errors.append(f"{name} 的预编辑行下限 {value} 与 CandidateFontWeight::preedit_value 的 {floor} 不同")
    if linux_words != set(expected.values()) - {weights[default]}:
        errors.append(f"Linux 的 Pango 样式词只覆盖 {sorted(linux_words)}，非常规字重是 {sorted(set(expected.values()) - {weights[default]})}")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(f"Candidate font weights match client-core on macOS, Windows and Linux ({', '.join(f'{k}={v}' for k, v in weights.items())}; preedit floor {floor})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

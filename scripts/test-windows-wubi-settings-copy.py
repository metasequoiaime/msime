#!/usr/bin/env python3
"""检查 Windows 两个原生设置窗口的五笔开关与共享设置页一一对应、逐字相同。

共享页 `wubi-section.tsx` 的 `WubiSection` 列出五笔的全部开关；Windows 发布的设置窗口是原生 WinUI（`main.cpp` 的 `if (scheme == L"wubi")` 分组），Windows 7 用 Win32 表单（`win7/SchemeForm.h`），三份文案各写一份。#10696 的报告人在 Windows 上把「第五码顶字」当成了「四码唯一自动上屏」失效，因为 Windows 两个窗口都漏了「第五码将首选上屏」和「z 键通配」两行，共享页和 macOS 早就有。这里逐条比较：WinUI 的 bool_row 要与共享页的开关同序、同标题、同说明、同偏好键、同出厂缺省（`wubiControlState`）；Win7 表单的每个开关要有同标题的复选框，并在 valid / refresh / 点击三处绑定同一个偏好键。
"""

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def section(path: str, start: str, end: str) -> str:
    source = (ROOT / path).read_text(encoding="utf-8")
    begin = source.index(start)
    return source[begin : source.index(end, begin)]


# ---- 共享页 ----

tsx = section(
    "packages/ui/src/settings/wubi-section.tsx",
    "export function WubiSection(",
    "\n}\n",
)
shared = []
for block in tsx.split("<SwitchRow")[1:]:
    title = re.search(r'title="([^"]*)"', block)
    description = re.search(r'description="([^"]*)"', block)
    key = re.search(r"onChange\(\{ (\w+): checked \}\)", block)
    assert title and description and key, f"共享页 WubiSection 有一个 SwitchRow 读不出标题、说明或偏好键：{block[:120]}"
    shared.append((title.group(1), description.group(1), key.group(1)))
assert len(shared) >= 5, f"共享页 WubiSection 应至少有 5 个开关，找到 {len(shared)} 个"

state = section(
    "packages/ui/src/settings/wubi-preferences.ts",
    "export function wubiControlState(",
    "\n}\n",
)
defaults = {key: value == "true" for key, value in re.findall(r"preferences\.(\w+) \?\? (true|false)", state)}

# ---- WinUI ----

cpp = section(
    "platforms/windows/settings/main.cpp",
    '    if (scheme == L"wubi") {',
    "\n    }\n",
)
rows = []
for block in cpp.split("bool_row(group, ")[1:]:
    literals = re.findall(r'L"([^"\\\n]*)"', block[: block.index(");")])
    fallback = block[: block.index(");")].rsplit(",", 1)[1].strip()
    assert len(literals) == 3, f"main.cpp 五笔分组有一个 bool_row 不是「标题、说明、偏好键」三个字面量：{block[:120]}"
    rows.append((literals[0], literals[1], literals[2], fallback))

assert [row[:3] for row in rows] == shared, (
    "Windows 原生设置的五笔分组与共享页 wubi-section.tsx 的开关不一致（顺序、标题、说明或偏好键），改共享页时要同步改 main.cpp 的 `if (scheme == L\"wubi\")` 分组：\n"
    + "\n".join(f"  共享页  {a}\n  Windows {b[:3]}" for a, b in zip(shared, rows) if a != b[:3])
    + ("" if len(shared) == len(rows) else f"\n  共享页 {len(shared)} 个开关，Windows {len(rows)} 个：缺 {[s[2] for s in shared if s[2] not in {r[2] for r in rows}]}")
)
for title, _, key, fallback in rows:
    if key == "wubi_mixed_pinyin":
        # 混输的缺省跟随版本（共享页的 mixedPinyinDefault），WinUI 读版本宏。
        assert fallback == "MSIME_EDITION_WUBI_MIXED_PINYIN_DEFAULT != 0", f"main.cpp 的「{title}」缺省应跟随版本宏，现在是 {fallback}"
        continue
    assert key in defaults, f"wubiControlState 没有 {key} 的缺省"
    expected = "true" if defaults[key] else "false"
    assert fallback == expected, f"main.cpp 的「{title}」（{key}）缺省是 {fallback}，共享页的出厂缺省是 {expected}"
assert "build_wubi_candidate_selection(group);" in cpp, "main.cpp 五笔分组不再画「五笔快捷选词」"

# ---- Win7 ----

win7_path = "platforms/windows/settings/win7/SchemeForm.h"
win7 = (ROOT / win7_path).read_text(encoding="utf-8")
checks = {title: (member, control) for member, control, title in re.findall(r'(\w+) = check\((\w+), L"([^"]*)"\)', win7)}
valid = section(win7_path, "  static bool valid(const Json& preferences) {", "\n  }\n")
refresh = section(win7_path, "  void refresh(const Json& preferences) {", "\n  }\n")
clicked = section(win7_path, "    } else if (notification == BN_CLICKED) {", "\n    }\n    return result;")
enable = section(win7_path, "  void enable(bool enabled) {", "\n  }\n")
for title, description, key in shared:
    assert title in checks, f"Win7 设置的五笔表单没有「{title}」复选框（{key}）"
    member, control = checks[title]
    assert f'"{key}"' in valid, f"Win7 SchemeForm::valid 没有校验 {key}"
    assert f'{{{member}, "{key}"}}' in refresh, f"Win7 SchemeForm::refresh 没有把 {key} 读进「{title}」"
    assert f'{{{control}, {member}, "{key}"}}' in clicked, f"Win7 SchemeForm::command 没有把「{title}」写回 {key}"
    assert re.search(rf"[{{ ]{member}[,}}]", enable), f"Win7 SchemeForm::enable 没有随方案启停「{title}」"

# 这两个开关单看标题猜不出边界（z 通配会停用 z 反查，顶字关掉只在混输开启时生效），Win7 表单在复选框下面照抄共享页的说明。
for title, description, key in shared:
    if key in ("wubi_z_wildcard", "wubi_fifth_code_top_commit"):
        assert f'L"{description}"' in win7, f"Win7 五笔表单没有照抄「{title}」的说明"

print("windows wubi settings: WinUI rows and Win7 checkboxes match the shared WubiSection switches")

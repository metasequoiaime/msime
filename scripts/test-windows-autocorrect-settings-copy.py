#!/usr/bin/env python3
"""检查 Windows 原生设置的全拼纠错 / 双拼纠错组与共享设置页逐字相同。

文案有三份：共享页 `input-page.tsx`、WinUI 窗口 `main.cpp` 的 `build_autocorrect_group`，以及 `settings_ui.ps1` 的 `Assert-PinyinAutocorrect` 用来在真实窗口里找行的那几句。UI Automation 只能把窗口和 ps1 绑在一起，所以这里把 tsx 和 cpp 按出现顺序逐条比较（组名、两行标题、两句说明都是「双拼 ? … : 全拼 …」的写法，顺序一致才说明每句说明落在同一行），再要求 ps1 里的每一句都出现在 cpp 里，偏好字段名三处也一致。
"""

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def section(path: str, start: str, end: str) -> str:
    source = (ROOT / path).read_text(encoding="utf-8")
    begin = source.index(start)
    return source[begin : source.index(end, begin)]


def chinese(literals: list[str]) -> list[str]:
    # 只比较带中文的字面量：组名、标题和说明；方案名、字段名和英文报错另外核对或不比较。
    return [text for text in literals if any(ord(c) > 0x7F for c in text)]


tsx = section(
    "packages/ui/src/settings/pages/input-page.tsx",
    '(draft.scheme === "quanpin" || draft.scheme === "shuangpin") && (',
    "</GroupList>",
)
cpp = section(
    "platforms/windows/settings/main.cpp",
    "  void build_autocorrect_group(StackPanel const &page) {",
    "\n  }\n",
)
ps1 = section(
    "platforms/windows/tests/tools/settings_ui.ps1",
    "function Assert-PinyinAutocorrect(",
    "\n}\n",
)
ps1 = "\n".join(line for line in ps1.splitlines() if not line.lstrip().startswith("#"))

shared = chinese(re.findall(r'"([^"\\\n]*)"', tsx))
windows = chinese(re.findall(r'L"([^"\\\n]*)"', cpp))
# 只取 $titles、$descriptions 两张表；之后切标签时等的「启用模糊音」「中文方案」不属于这一组。
probes = chinese(re.findall(r"'([^'\n]*)'", ps1[ps1.index("$titles = @{") : ps1.index("Select-PageTab")]))

assert len(shared) == 10, f"共享页的全拼纠错 / 双拼纠错组应有组名、两行标题和说明共 10 句，找到 {len(shared)} 句"
assert windows == shared, (
    "Windows 原生设置的全拼纠错 / 双拼纠错组与共享页 input-page.tsx 不一致，改共享页文案时要同步改 main.cpp 的 build_autocorrect_group：\n"
    + "\n".join(f"  共享页 {a}\n  Windows {b}" for a, b in zip(shared, windows) if a != b)
)
assert probes, "settings_ui.ps1 的 Assert-PinyinAutocorrect 没有可核对的文案"
for text in probes:
    assert text in windows, f"settings_ui.ps1 的 Assert-PinyinAutocorrect 找的「{text}」不在 main.cpp 的 build_autocorrect_group 里"

for field in ("autocorrect_neighbor", "autocorrect_transposition"):
    assert f"quanpin?.{field}" in tsx, f"共享页的纠错组不再读 quanpin.{field}"
    assert f'L"quanpin.{field}", true' in cpp, f"Windows 纠错组没有以默认开写 quanpin.{field}"
    assert f"'{field}'" in ps1, f"settings_ui.ps1 没有核对 quanpin.{field}"

print("windows autocorrect settings: native group titles and descriptions match the shared input page and the UI test")

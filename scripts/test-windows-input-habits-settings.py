#!/usr/bin/env python3
"""检查 Windows 原生输入习惯页没有遗漏共享偏好支持的候选项。"""

from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
source = (ROOT / "platforms/windows/settings/main.cpp").read_text(encoding="utf-8")
start = source.index("  void build_input_habits(")
end = source.index("\n  // 选择日语、韩语、越南语或藏语", start)
page = source[start:end]

expected = {
    "日文混输（实验）": 'L"mixed_input.japanese"',
    "只出单字": 'L"single_character_only"',
    "本地整句联想": 'L"sentence_association.word_lattice"',
}
for title, key in expected.items():
    assert title in page, f"Windows 输入习惯页缺少「{title}」"
    assert key in page, f"Windows 输入习惯页没有接入偏好键 {key}"

print("windows input habits: Japanese mixed input, single-character and sentence rows are present")

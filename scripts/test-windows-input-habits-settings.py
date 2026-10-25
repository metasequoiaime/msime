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

assert "build_neural_desktop_settings(candidates);" in page
assert "build_sentence_model_settings(candidates);" in page
model = source[source.index("  void build_neural_desktop_settings("):]
for key in (
    'L"sentence_association.neural_desktop"',
    'L"sentence_association.show_next_on_duplicate"',
    'L"sentence_association.grammar_model"',
    "msime_client_resource_packs",
    "msime_client_resource_pack_install",
    "msime_client_resource_pack_remove",
    # 安装包不带落定重排模型，开了「桌面神经联想」必须能在这里下载它。
    'sentence_pack_row(group, L"settled-model"',
):
    assert key in model, f"Windows 整句模型设置缺少 {key}"
assert "grammar-model-baishuang" not in model

print("windows input habits: sentence association switches, settled model download and locked grammar model management are present")

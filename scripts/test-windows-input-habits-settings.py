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
# 「整句联想」的「标准 / 增强」写共享的 `neural_keyboard`，缺键按默认的开读；只列在带键盘模型的版本里，排在「桌面神经联想」前面。
assert page.index("build_sentence_level_settings(candidates);") < page.index("build_neural_desktop_settings(candidates);")
level = source[source.index("  void build_sentence_level_settings("):source.index("  void build_neural_desktop_settings(")]
for key in (
    "if constexpr (MSIME_EDITION_NEURAL_KEYBOARD != 0)",
    'document_.Boolean(L"sentence_association.neural_keyboard", true)',
    'doc.SetBoolean(L"sentence_association.neural_keyboard", value == L"enhanced")',
    '{{L"standard", L"标准"}, {L"enhanced", L"增强"}}',
    'add_row(group, 0xE8C1, L"整句联想", L"更准确，但更耗电", selector)',
    'L"sentence-association-level"',
):
    assert key in level, f"Windows「整句联想」选择缺少 {key}"
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
# 神经联想模型和语法模型也能不联网安装（#10622）：列出要用浏览器下载的文件链接，「从文件导入」走和语音模型同一个打开对话框，选中的文件以 `files` 交给资源包安装，失败按导入的说法提示。
packs = source[source.index("  void sentence_pack_row("):source.index("  void cancel_sentence_pack(")]
for key in (
    'L"import_files"',
    "append_link(offline,",
    'L"sentence-model-import-" + pack',
    "pick_model_files(failure)",
    "vs::local_model_picker_error_message(",
    'request.SetNamedValue(L"files", paths)',
    "vs::local_model_import_error_message(code)",
    "正在导入和校验模型…",
):
    assert key in packs, f"Windows 整句模型缺少从文件导入的 {key}"

print("windows input habits: sentence association switches, the 标准/增强 level, settled model download, offline import and locked grammar model management are present")

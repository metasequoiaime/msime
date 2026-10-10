#pragma once

namespace Global
{
// 候选列表开着时，哪些键要带 PipeMetadata::CandidateActive 发给 Server。modifiers 是线上的修饰键位（Shift 1、Ctrl 2、Alt 4），已经按 CharacterModifiers 去掉了 AltGr 打字。
//
// 普通组字的候选列表是 CANDIDATE_INCREMENTAL，以前只有 CANDIDATE_ORIGINAL（通配转换）才带这一位，于是 Server 的释义列快捷键（gloss_column_key）、Ctrl+Enter 和它打开的释义页（commit_candidate_translation、translation_page_key）在普通组字里都收不到这一位，什么也不做。这里列的正是这几条路认的键：空格和数字 1–9（含小键盘）在预选释义列或释义页上上屏释义，Tab 和 Shift+Tab 预选释义列，翻页、Home、End、上下方向键在释义页上翻看，只按 Alt 或只按 Ctrl 的主键盘数字直接上屏某一列释义，Ctrl+Enter 上屏或打开释义。
//
// 其余的键不带：Server 用没有这一位来认出其他路，裸回车按组字原文上屏（带了就改成选中高亮候选）、'[' ']' '-' '=' 以词定字、Ctrl+Backspace 恢复上一段，都只在不带这一位时生效。
inline bool CandidateKeyReportsActiveList(unsigned virtualKey, unsigned modifiers)
{
    const unsigned chord = modifiers & 0b111u;
    const bool mainDigit = virtualKey >= 0x31u && virtualKey <= 0x39u;
    const bool numpadDigit = virtualKey >= 0x61u && virtualKey <= 0x69u;
    if (chord == 0)
    {
        switch (virtualKey)
        {
        case 0x09: // Tab
        case 0x20: // 空格
        case 0x21: // 上翻页
        case 0x22: // 下翻页
        case 0x23: // End
        case 0x24: // Home
        case 0x26: // 上方向键
        case 0x28: // 下方向键
            return true;
        default:
            return mainDigit || numpadDigit;
        }
    }
    if (chord == 0b001u)
        return virtualKey == 0x09u;
    if (chord == 0b010u && virtualKey == 0x0Du)
        return true;
    return (chord == 0b010u || chord == 0b100u) && mainDigit;
}
} // namespace Global

#pragma once

#include <cstddef>
#include <cstdint>
#include <string>
#include <string_view>

namespace msime::linux_host {

// 候选行文字的写法，IBus 与 Fcitx5 共用一份，和 macOS 候选窗（InputController.mm 的 MSIMEParenthesizedAnnotation、CandidateDisplay）逐字一致：注释（辅助码、五笔编码提示、/ 和 @ 模式的命令标题或省市）包在半角括号里、紧贴候选字，画成「你好(wqvb)」；云候选和 AI 候选在其后隔一个空格加 ☁️、🤖 角标。两个前端的面板都只能画一行纯文本，这里只决定文字本身，颜色和字形属性由各前端自己加。

// 注释去掉首尾空白后包上半角括号；已经带半角或全角括号的原样保留，空串不画。
inline std::string candidate_annotation_text(std::string_view annotation) {
  const auto blank = [](char value) { return value == ' ' || value == '\t' || value == '\n' || value == '\r'; };
  while (!annotation.empty() && blank(annotation.front())) annotation.remove_prefix(1);
  while (!annotation.empty() && blank(annotation.back())) annotation.remove_suffix(1);
  if (annotation.empty()) return {};
  constexpr std::string_view full_open = "\xEF\xBC\x88", full_close = "\xEF\xBC\x89";
  const bool half = annotation.front() == '(' && annotation.back() == ')';
  const bool full = annotation.size() >= full_open.size() + full_close.size() &&
                    annotation.substr(0, full_open.size()) == full_open &&
                    annotation.substr(annotation.size() - full_close.size()) == full_close;
  if (half || full) return std::string(annotation);
  return "(" + std::string(annotation) + ")";
}

// Engine 的候选来源：2 是云候选，3 是 AI 候选。角标只供显示，选词仍按候选身份。
inline std::string_view candidate_source_badge(std::uint64_t source) {
  if (source == 2) return " \xE2\x98\x81\xEF\xB8\x8F";
  if (source == 3) return " \xF0\x9F\xA4\x96";
  return {};
}

// 固定了排位的候选行的文字颜色，macOS 与 Windows 都画 #379AD3（不随主题变）。Linux 只在未选中的行上画它：Adwaita 的选中底色是 #3584E4，同色系的字压在上面读不出来，选中行照旧用选中文字色。
inline constexpr std::uint32_t kCandidateFixedPositionColor = 0x379AD3u;

// 第 `slot` 个候选（从 0 起）的序号，与选它的数字键一致：前九个是 1–9，第十个由 0 键选，标 0。
inline std::string candidate_index_label(std::size_t slot) {
  return slot == 9 ? std::string("0") : std::to_string(slot + 1);
}

}  // namespace msime::linux_host

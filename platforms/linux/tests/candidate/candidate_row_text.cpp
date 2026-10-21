#include "../src/candidates/CandidateRowText.h"

#include <cassert>
#include <string>

int main() {
  namespace host = msime::linux_host;
  // 注释包上半角括号紧贴候选字，与 macOS 的 MSIMEParenthesizedAnnotation 相同；首尾空白去掉，空串不画。
  assert(host::candidate_annotation_text("wqvb") == "(wqvb)");
  assert(host::candidate_annotation_text("  广东 广州 \n") == "(广东 广州)");
  assert(host::candidate_annotation_text("").empty() && host::candidate_annotation_text(" \t").empty());
  // 已经带括号的原样保留，半角和全角都算。
  assert(host::candidate_annotation_text("(ab)") == "(ab)");
  assert(host::candidate_annotation_text("（拼音）") == "（拼音）");
  // 只在一头有括号的不算括好了。
  assert(host::candidate_annotation_text("(ab") == "((ab)");
  assert(host::candidate_annotation_text("（") == "(（)");
  // 云候选和 AI 候选的角标隔一个空格，其他来源没有。
  assert(host::candidate_source_badge(2) == " ☁️");
  assert(host::candidate_source_badge(3) == " 🤖");
  for (const std::uint64_t source : {0u, 1u, 4u, 12u, 15u}) assert(host::candidate_source_badge(source).empty());
  // 序号与选它的数字键一致，第十个标 0。
  assert(host::candidate_index_label(0) == "1" && host::candidate_index_label(8) == "9");
  assert(host::candidate_index_label(9) == "0");
  assert(host::kCandidateFixedPositionColor == 0x379AD3u);
  // 画不出单行颜色的面板用「  固定N」代替固定色，只认 1–5 位。
  assert(host::candidate_fixed_position_marker(1) == "  固定1" && host::candidate_fixed_position_marker(5) == "  固定5");
  assert(host::candidate_fixed_position_marker(0).empty() && host::candidate_fixed_position_marker(6).empty());
  return 0;
}

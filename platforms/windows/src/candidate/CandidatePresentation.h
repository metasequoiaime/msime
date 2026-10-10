#pragma once
#include "CandidateActionAvailability.h"
#include "CandidateGlossReadings.h"
#include "ChineseTextConversion.h"
#include "FocusGate.h"
#include "GlossColumnPolicy.h"
#include "InputSchemeTraits.h"
#include "PipeMetadata.h"
#include "ReplyComposer.h"
#include "ShuangpinKeymapLayout.h"
#include "TranslationDisplay.h"
#include "WubiCodeHintPolicy.h"
#include <algorithm>

namespace msime::windows {
// TSF can report the candidate show event before it has a usable text extent.
// Keep this sentinel aligned with the native Windows host contract.
inline constexpr int invalid_candidate_anchor_y = -100000;

struct PresentationCandidate {
  uint64_t session;
  uint64_t generation;
  size_t index;
  std::string text;
  bool highlighted;
  // Engine-corrected spellings are marked for the user, while `text` remains
  // the exact value selected by the candidate identity.
  bool corrected = false;
  std::string annotation;
  std::string badge;
  uint8_t fixed_position = 0;
  std::string translation;
  bool actions_available = true;
  // The Wubi code left after the typed prefix; shown only when the `wubi_code_hint` preference is on, see with_wubi_code_hints.
  std::string wubi_code_hint{};
  // A Korean Hanja's 훈음 (나라 이름 한), which the Engine sends as the row's annotation. It is drawn on the smaller secondary line whatever the translation preferences say, above the translation when there is one, and it is display only: nothing commits it, and `translation` keeps only what the Engine applied as a translation.
  std::string gloss{};
  // 释义每一行的读音，按 "\n" 分行，和 translation 的各行（U+2028 分行）逐行对应；只用于显示，从不上屏。由翻译工作线程算好、CandidateMailbox 按候选文字和释义挂上来，见 CandidateReading。
  std::string pronunciation{};
  // 整句候选的逐词拆解（「我 I · 喜欢 to like · 你 you」），画在释义下面单独一行，同样只用于显示。
  std::string breakdown{};
};
// Whether this view's candidates are a Korean Hanja list: the Korean scheme under its own rules, outside the dedicated English mode and every local mode, where the Engine lists candidates only after MSIME_CONVERT_HANJA. ReplyComposer::korean_hanja reads the same three fields.
inline bool korean_hanja_view(const nlohmann::json &view) {
  return view.value("scheme", 0u) == candidate_scheme_korean &&
         !view.value("dedicated_english", false) &&
         view.value("local_mode", std::string("none")) == "none";
}
// 这个 view 的候选会不会带释义，和 host-api msime_client_translation_query 的判据一样：方案显示释义（scheme::ShowsGlosses，日文、粤语、注音、笔画、越南文、藏文都不显示），且不在临时日文组字和网址模式里。
inline bool candidate_view_shows_glosses(const nlohmann::json &view) {
  const auto mode = view.value("local_mode", std::string("none"));
  return scheme::ShowsGlosses(static_cast<int>(view.value("scheme", 0u))) && mode != "temporary_japanese" &&
         mode != "url";
}
// Move a Hanja row's 훈음 out of the annotation run, which follows the Hanja at full size, into `gloss`, so the main text is the Hanja alone.
inline void move_korean_hanja_gloss(PresentationCandidate &candidate) {
  candidate.gloss = std::move(candidate.annotation);
  candidate.annotation.clear();
}
// 释义部分画出来的样子：每种目标语言一行，行后跟读音，整句的逐词拆解在最后一行（candidate_gloss_display）。with_readings 为 false 时只有释义各行，不带读音和拆解。
inline std::string candidate_translation_display(const PresentationCandidate &candidate,
                                                 bool with_readings = true) {
  std::vector<std::string> lines;
  if (!candidate.translation.empty())
    lines = translation_lines(candidate.translation);
  if (!with_readings)
    return candidate_gloss_display(lines, {}, {});
  std::vector<std::string> readings;
  for (size_t start = 0; !candidate.pronunciation.empty();) {
    const auto cut = candidate.pronunciation.find('\n', start);
    readings.push_back(candidate.pronunciation.substr(start, cut == std::string::npos ? std::string::npos : cut - start));
    if (cut == std::string::npos)
      break;
    start = cut + 1;
  }
  return candidate_gloss_display(lines, readings, candidate.breakdown);
}
// 一行候选下面那段较小的次要文字：只有 훈음、只有释义，或 훈음 在上、释义在下一行。释义部分是 candidate_translation_display：每种目标语言一行并跟着读音，最后是逐词拆解。候选窗只画不超过 4096 字节的文字（CandidateWindow 的 wide()），读音和拆解加上去超过时就不带它们，免得一条很长的释义让整个候选窗画不出来。
inline std::string candidate_secondary_text(const PresentationCandidate &candidate) {
  const auto compose = [&](bool with_readings) {
    const auto translation = candidate_translation_display(candidate, with_readings);
    if (candidate.gloss.empty())
      return translation;
    if (translation.empty())
      return candidate.gloss;
    return candidate.gloss + "\n" + translation;
  };
  auto text = compose(true);
  if (text.size() > 4096 && (!candidate.pronunciation.empty() || !candidate.breakdown.empty()))
    text = compose(false);
  return text;
}
// 把翻译工作线程算好的读音和拆解挂到候选上，没有的清空：读音只在候选眼下的释义与算它时的释义相同才挂，拆解按候选文字挂。
inline void attach_candidate_readings(std::vector<PresentationCandidate> &candidates,
                                      const CandidateReadings &readings) {
  for (auto &candidate : candidates) {
    candidate.pronunciation.clear();
    candidate.breakdown.clear();
    const auto found = readings.find(candidate.text);
    if (found == readings.end())
      continue;
    if (!candidate.translation.empty() && found->second.translation == candidate.translation)
      candidate.pronunciation = found->second.pronunciation;
    candidate.breakdown = found->second.breakdown;
  }
}
// The correction marker is display-only: keep it out of the text used for
// selection, dictionary actions and candidate identity.
inline std::string candidate_primary_text(const PresentationCandidate &candidate) {
  return candidate.text + (candidate.corrected ? "*" : "") + candidate.badge;
}
// 鼠标停在一行候选上时的提示：候选全文，有释义时下一行是释义，和 macOS 候选按钮的 toolTip 一样。长候选和长释义在卡片里会折行，提示里是完整的一段。
inline std::string candidate_tooltip_text(const PresentationCandidate &candidate) {
  auto text = candidate_primary_text(candidate);
  const auto secondary = candidate_secondary_text(candidate);
  if (!secondary.empty())
    text += "\n" + secondary;
  return text;
}
// candidate_secondary_text 在折行之前有几行：훈음、每种目标语言一行和逐词拆解。Engine 不收含控制字符的释义，훈음 表里也没有，所以每个换行都是 candidate_secondary_text 自己放进去的。
inline size_t candidate_secondary_lines(const PresentationCandidate &candidate) {
  const auto text = candidate_secondary_text(candidate);
  return 1 + static_cast<size_t>(std::count(text.begin(), text.end(), '\n'));
}
struct CandidatePresentation {
  FocusLease lease;
  uint64_t session;
  uint64_t generation;
  // Monotonic identity for the complete rendered snapshot, including async
  // provider updates that keep the same Engine generation.
  uint64_t render_serial = 0;
  bool visible = false;
  int x = 0;
  int y = 0;
  std::string preedit;
  // Byte offset of the caret within `preedit`, or npos when it cannot be
  // placed. The Engine reports the caret against editing_text, which is not
  // always the same string as the preedit being drawn; putting the caret at
  // the wrong character is worse than not drawing one, so it is only carried
  // when the two agree.
  size_t preedit_caret = std::string::npos;
  std::vector<PresentationCandidate> candidates;
  bool traditional_output = false;
  // The 0-based page on show and how many pages the Engine has so far, for the pager in the preedit row. Both zero draw no pager. The count grows as the user pages, because the Engine fetches candidates lazily.
  size_t page = 0;
  size_t page_count = 0;
  // Whether the mouse may pick a row, page the list or open a row's menu. False for a list driven from the keyboard only (scheme::KeyboardOnlyCandidateList).
  bool pointer_input = true;
  // 这一页的候选会不会有释义（candidate_view_shows_glosses）。为 false 时翻译查询为空，横排候选窗不为释义预留高度。
  bool shows_glosses = true;
  // 游戏会话（包上带 PipeMetadata::GameHost）：宿主给的锚点可能不可信，候选窗允许在游戏客户区里兜底定位。
  bool game_host = false;
  // 双拼组字时候选窗旁的键位提示（方案名和要高亮的键），按 view 判断，不是双拼组字时为空；开关由 Server 主循环另外看。
  std::optional<ShuangpinKeymapHint> shuangpin_keymap;
  // Tab 预选的释义列（1 或 2），候选窗给高亮候选的那一行释义画下划线；0 表示没有预选。见 GlossColumnPolicy.h。
  int armed_gloss_column = 0;
};
// 高亮候选确实有第 column 列释义时才算预选，否则为 0：释义刷新、云候选插入后高亮候选可能已经没有那一列，ReplyComposer 上屏时按同一条规则判断。
inline int candidate_armed_gloss_column(const std::vector<PresentationCandidate> &candidates,
                                        int column) {
  if (column <= 0)
    return 0;
  for (const auto &candidate : candidates)
    if (candidate.highlighted)
      return candidate.gloss.empty() && !gloss_column_text(candidate.translation, column).empty()
                 ? column
                 : 0;
  return 0;
}
// 预选的那一列释义在 candidate_secondary_text 里的位置（UTF-8 字节的起点和长度），候选窗据此画下划线。候选带 훈음 时那一段另起一行在上面，这种候选（韩文）不接释义列，不画。
inline std::optional<std::pair<size_t, size_t>>
candidate_gloss_column_range(const PresentationCandidate &candidate, int column) {
  if (column <= 0 || !candidate.gloss.empty())
    return std::nullopt;
  const auto gloss = gloss_column_text(candidate.translation, column);
  if (gloss.empty())
    return std::nullopt;
  const auto text = candidate_secondary_text(candidate);
  size_t start = 0;
  for (int line = 1; line < column; ++line) {
    const auto cut = text.find('\n', start);
    if (cut == std::string::npos)
      return std::nullopt;
    start = cut + 1;
  }
  if (text.compare(start, gloss.size(), gloss) != 0)
    return std::nullopt;
  return std::make_pair(start, gloss.size());
}
// Copy the view's page position into `output`, dropping one that is not a page of the count rather than drawing "4 / 3".
inline void candidate_presentation_page(CandidatePresentation &output,
                                        const nlohmann::json &view) {
  const auto page = view.value("page", size_t{0});
  const auto count = view.value("page_count", size_t{0});
  if (page < count) {
    output.page = page;
    output.page_count = count;
  }
}
inline CandidatePresentation
candidate_presentation_from_view(const FocusLease &lease,
                                 const nlohmann::json &view, int x, int y,
                                 const std::string &prefix,
                                 bool traditional_output = false) {
  // Named rather than positional: this is an aggregate, so inserting a field
  // above silently shifts every following initializer onto the wrong member.
  CandidatePresentation output{};
  output.lease = lease;
  output.session = view.at("session").get<uint64_t>();
  output.generation = view.at("generation").get<uint64_t>();
  output.visible = false;
  output.x = x;
  output.y = y;
  if (!view.at("focused").get<bool>() ||
      view.at("editing_text").get<std::string>().empty())
    return output;
  const auto text = view.at("preedit").get<std::string>();
  if (prefix.size() > 4096 ||
      text.size() > 4096 - prefix.size() || view.at("candidates").size() > 9)
    throw std::invalid_argument("Oversized candidate presentation");
  output.traditional_output = traditional_output;
  output.preedit = prefix + text;
  // caret_position is a byte offset into the Engine's ASCII editing_text. It
  // transfers to the drawn preedit only when the two are the same string; the
  // prefix this host prepends shifts it by its own length.
  const auto editing = view.at("editing_text").get<std::string>();
  if (editing == text) {
    const auto caret = view.value("caret_position", text.size());
    if (caret <= text.size())
      output.preedit_caret = prefix.size() + caret;
  }
  size_t highlighted = 0;
  const bool hanja = korean_hanja_view(view);
  for (const auto &candidate : view.at("candidates")) {
    const auto &id = candidate.at("id");
    const auto candidate_source = candidate.value("source", uint8_t{});
    PresentationCandidate item{
        id.at("session").get<uint64_t>(), id.at("generation").get<uint64_t>(),
        id.at("index").get<size_t>(),
        simplified_to_traditional(candidate.at("text").get<std::string>(),
                                  traditional_output),
        candidate.at("highlighted").get<bool>(),
        candidate.value("corrected", false),
        candidate.value("annotation", std::string{}),
        candidate_source == 2 ? " ☁️" : candidate_source == 3 ? " 🤖" : "",
        candidate.value("fixed_position", uint8_t{}),
        candidate.value("translation", std::string{}),
        candidate_actions_available(view.value("scheme", 0u), candidate_source)};
    if (hanja)
      move_korean_hanja_gloss(item);
    if (item.session != output.session ||
        item.generation != output.generation || item.text.size() > 4096 ||
        item.annotation.size() > 4096 || item.gloss.size() > 4096 ||
        item.badge.size() > 4096 ||
        item.translation.size() > 4096)
      throw std::invalid_argument("Invalid presented candidate");
    item.wubi_code_hint = wubi_code_hint(view, candidate);
    highlighted += item.highlighted;
    output.candidates.push_back(std::move(item));
  }
  if (!output.candidates.empty() && highlighted != 1)
    throw std::invalid_argument("Invalid candidate highlight");
  candidate_presentation_page(output, view);
  output.pointer_input = !scheme::KeyboardOnlyCandidateList(
      static_cast<int>(view.value("scheme", 0u)));
  output.shows_glosses = candidate_view_shows_glosses(view);
  output.shuangpin_keymap = shuangpin_keymap_hint(view);
  output.armed_gloss_column =
      candidate_armed_gloss_column(output.candidates, view.value("armed_gloss_column", 0));
  output.visible = true;
  return output;
}
// Value-only projection. Call only from the confirmed-delivery callback.
// A future UI consumer must revalidate focus before displaying or clicking.
inline CandidatePresentation
candidate_presentation(const FocusLease &lease, const PendingReply &reply,
                       const FanyImeNamedpipeData &packet) {
  const auto &source = reply.source;
  if (!lease.epoch || source.client_id != lease.transport.client ||
      source.activation_epoch != lease.epoch ||
      packet.client_id != source.client_id ||
      packet.request_id != source.request_id ||
      packet.event_type != FanyImePipeEventType::KeyEvent)
    throw std::invalid_argument("Invalid candidate presentation identity");
  const auto &view = source.transition.at("view");
  CandidatePresentation output{};
  output.lease = lease;
  output.session = view.at("session").get<uint64_t>();
  output.generation = view.at("generation").get<uint64_t>();
  output.x = packet.point[0];
  output.y = packet.point[1];
  output.game_host = (packet.modifiers_down & PipeMetadata::GameHost) != 0;
  if (!view.at("focused").get<bool>() ||
      (packet.modifiers_down & FanyImePipeFlags::UiLess) ||
      view.at("editing_text").get<std::string>().empty())
    return output;
  const auto text = view.at("preedit").get<std::string>();
  if (reply.next_prefix.size() > 4096 ||
      text.size() > 4096 - reply.next_prefix.size() ||
      view.at("candidates").size() > 9)
    throw std::invalid_argument("Oversized candidate presentation");
  output.traditional_output = reply.traditional_output;
  output.preedit = reply.next_prefix + text;
  size_t highlighted = 0;
  const bool hanja = korean_hanja_view(view);
  for (const auto &candidate : view.at("candidates")) {
    const auto &id = candidate.at("id");
    const auto candidate_source = candidate.value("source", uint8_t{});
    PresentationCandidate item{
        id.at("session").get<uint64_t>(), id.at("generation").get<uint64_t>(),
        id.at("index").get<size_t>(),
        simplified_to_traditional(candidate.at("text").get<std::string>(),
                                  reply.traditional_output),
        candidate.at("highlighted").get<bool>(),
        candidate.value("corrected", false),
        candidate.value("annotation", std::string{}),
        candidate_source == 2 ? " ☁️" : candidate_source == 3 ? " 🤖" : "",
        candidate.value("fixed_position", uint8_t{}),
        candidate.value("translation", std::string{}),
        candidate_actions_available(view.value("scheme", 0u), candidate_source)};
    if (hanja)
      move_korean_hanja_gloss(item);
    if (item.session != output.session ||
        item.generation != output.generation || item.text.size() > 4096 ||
        item.annotation.size() > 4096 || item.gloss.size() > 4096 ||
        item.badge.size() > 4096 ||
        item.translation.size() > 4096)
      throw std::invalid_argument("Invalid presented candidate");
    item.wubi_code_hint = wubi_code_hint(view, candidate);
    highlighted += item.highlighted;
    output.candidates.push_back(std::move(item));
  }
  if (!output.candidates.empty() && highlighted != 1)
    throw std::invalid_argument("Invalid candidate highlight");
  candidate_presentation_page(output, view);
  output.pointer_input = !scheme::KeyboardOnlyCandidateList(
      static_cast<int>(view.value("scheme", 0u)));
  output.shows_glosses = candidate_view_shows_glosses(view);
  output.shuangpin_keymap = shuangpin_keymap_hint(view);
  output.armed_gloss_column =
      candidate_armed_gloss_column(output.candidates, view.value("armed_gloss_column", 0));
  output.visible = true;
  return output;
}
} // namespace msime::windows

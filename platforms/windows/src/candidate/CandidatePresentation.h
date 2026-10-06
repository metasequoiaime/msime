#pragma once
#include "CandidateActionAvailability.h"
#include "ChineseTextConversion.h"
#include "FocusGate.h"
#include "InputSchemeTraits.h"
#include "ReplyComposer.h"
#include "WubiCodeHintPolicy.h"

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
};
// Whether this view's candidates are a Korean Hanja list: the Korean scheme under its own rules, outside the dedicated English mode and every local mode, where the Engine lists candidates only after MSIME_CONVERT_HANJA. ReplyComposer::korean_hanja reads the same three fields.
inline bool korean_hanja_view(const nlohmann::json &view) {
  return view.value("scheme", 0u) == candidate_scheme_korean &&
         !view.value("dedicated_english", false) &&
         view.value("local_mode", std::string("none")) == "none";
}
// Move a Hanja row's 훈음 out of the annotation run, which follows the Hanja at full size, into `gloss`, so the main text is the Hanja alone.
inline void move_korean_hanja_gloss(PresentationCandidate &candidate) {
  candidate.gloss = std::move(candidate.annotation);
  candidate.annotation.clear();
}
// The smaller secondary run of a row: the 훈음 alone, the translation alone, or the 훈음 with the translation on the line under it.
inline std::string candidate_secondary_text(const PresentationCandidate &candidate) {
  if (candidate.gloss.empty())
    return candidate.translation;
  if (candidate.translation.empty())
    return candidate.gloss;
  return candidate.gloss + "\n" + candidate.translation;
}
// The correction marker is display-only: keep it out of the text used for
// selection, dictionary actions and candidate identity.
inline std::string candidate_primary_text(const PresentationCandidate &candidate) {
  return candidate.text + (candidate.corrected ? "*" : "") + candidate.badge;
}
// How many lines candidate_secondary_text starts with before any wrapping. The Engine refuses control characters in a translation, and the 훈음 table has none, so the only line break is the one joining them.
inline size_t candidate_secondary_lines(const PresentationCandidate &candidate) {
  return !candidate.gloss.empty() && !candidate.translation.empty() ? 2 : 1;
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
};
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
  output.visible = true;
  return output;
}
} // namespace msime::windows

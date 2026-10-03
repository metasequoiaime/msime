#include "ReplyComposer.h"
#include "CandidateTranslationPolicy.h"
#include "ChineseTextConversion.h"
#include "InputSchemeTraits.h"
#include "KoreanHanjaKey.h"
#include "PunctuationPolicy.h"
#include <algorithm>
#include <limits>
#include <stdexcept>

namespace msime::windows {
namespace {
std::optional<NavigationReply> navigation_for(ReplyPath path) {
  switch (path) {
  case ReplyPath::IgnoredNavigation:
    return NavigationReply::Ignored;
  case ReplyPath::PreviousCandidate:
    return NavigationReply::PreviousCandidate;
  case ReplyPath::NextCandidate:
    return NavigationReply::NextCandidate;
  case ReplyPath::PreviousPage:
    return NavigationReply::PreviousPage;
  case ReplyPath::NextPage:
    return NavigationReply::NextPage;
  default:
    return std::nullopt;
  }
}
// The UI-less reply for a composition: its display text and the candidate page, with the highlight the view marks.
EncodedReply uiless_composition(uint64_t request, const std::string &display,
                                const nlohmann::json &view) {
  std::vector<std::string> candidates;
  const auto &view_candidates = view.at("candidates");
  candidates.reserve(view_candidates.size());
  size_t highlighted = 0;
  bool found_highlight = false;
  for (const auto &candidate : view_candidates) {
    if (candidate.at("highlighted").get<bool>()) {
      if (found_highlight)
        throw std::logic_error("Ambiguous candidate highlight");
      highlighted = candidates.size();
      found_highlight = true;
    }
    candidates.push_back(candidate.at("text").get<std::string>());
  }
  if (!candidates.empty() && !found_highlight)
    throw std::logic_error("Missing candidate highlight");
  return uiless_reply(request, display, candidates, highlighted);
}
// The reference's CandidateTextForOutput: the Japanese scheme's kana and kanji never go through the simplified-to-traditional table, whatever the character-set toggle says. Korean Hangul and Vietnamese are not Chinese text either, Cantonese and Zhuyin are Traditional already, and Stroke writes its characters as stored (scheme::ScriptConversionApplies). The toggle itself is untouched, so leaving any of them restores traditional output.
bool traditional_projection(const ServerSession &session) {
  if (!session.traditional_output())
    return false;
  return scheme::ScriptConversionApplies(static_cast<int>(session.view().value("scheme", 0u)));
}
// The view's scheme, as the scheme traits take it.
int view_scheme(const nlohmann::json &view) { return static_cast<int>(view.value("scheme", 0u)); }
} // namespace
ReplyComposer::ReplyComposer(uint64_t client, uint64_t epoch)
    : client_(client), epoch_(epoch) {
  if (!client || !epoch)
    throw std::invalid_argument("Invalid reply activation");
}
const PendingReply &
ReplyComposer::stage(const KeyResult &result, ReplyPath path, bool uiless,
                     std::optional<std::string> local_text) {
  if (pending_)
    throw std::logic_error(
        "Resolve the pending reply before dispatching another key");
  if (result.client_id != client_ || result.activation_epoch != epoch_)
    throw std::logic_error("Expired reply route");
  const auto &view = result.transition.at("view");
  const auto session = view.at("session").get<uint64_t>();
  if (!session || (session_ && session != session_))
    throw std::logic_error("Reply changed host session");
  const auto raw = view.at("editing_text").get<std::string>();
  const auto display = view.at("preedit").get<std::string>();
  const auto &commit = result.transition.at("commit");
  const auto delta =
      commit.is_null() ? std::string{} : commit.get<std::string>();
  const auto output_delta =
      simplified_to_traditional(delta, traditional_output_);
  // By name, not by position: PendingReply has gained fields twice, and a
  // positional list silently shifts every value past the new one rather than
  // failing to compile.
  PendingReply next;
  next.source = result;
  next.next_prefix = prefix_;
  next.traditional_output = traditional_output_;
  const auto invalid = [&] {
    next.encoded = EncodedReply{ReplyError::InvalidFields, {}};
  };
  switch (path) {
  case ReplyPath::LocalCancel:
    if (!delta.empty() || !raw.empty()) {
      invalid();
      break;
    }
    next.next_prefix.clear();
    break;
  case ReplyPath::NoReply:
    if (!delta.empty()) {
      invalid();
      break;
    }
    break;
  case ReplyPath::LocalCommit:
    // Legacy Enter inserts the prefix plus its local raw buffer. This is a
    // completion acknowledgement, not a second text insertion frame.
    if (!raw.empty() || !local_text || *local_text != prefix_ + delta)
      next.encoded = EncodedReply{ReplyError::InvalidFields, {}};
    else {
      next.next_prefix.clear();
      // Inserted by the TSF itself; the observation was validated against the
      // prefix and Engine state above, so it is in the document either way.
      next.committed_text = *local_text;
    }
    break;
  case ReplyPath::Selection:
    if (!result.reply_expected) {
      invalid();
      break;
    }
    if (delta.empty())
      next.encoded = ignored_reply(result.request_id);
    else {
      const auto total = prefix_ + output_delta;
      if (!raw.empty()) {
        next.encoded =
            partial_selection(result.request_id, raw, total, total + display);
        next.next_prefix = total;
      } else {
        next.encoded = candidate_commit(result.request_id, total);
        next.next_prefix.clear();
        next.committed_text = total;
      }
    }
    break;
  case ReplyPath::Punctuation:
    if (!result.reply_expected) {
      invalid();
      break;
    }
    if (delta.empty())
      next.encoded = ignored_reply(result.request_id);
    else {
      if (!raw.empty()) {
        invalid();
        break;
      }
      next.encoded = exact_commit(result.request_id, prefix_ + output_delta);
      next.next_prefix.clear();
      next.committed_text = prefix_ + output_delta;
    }
    break;
  case ReplyPath::CandidatePunctuationFallback:
    if (!result.reply_expected || !raw.empty()) {
      invalid();
      break;
    }
    next.encoded = candidate_commit(result.request_id,
                                    prefix_ + output_delta);
    next.next_prefix.clear();
    next.committed_text = prefix_ + output_delta;
    break;
  case ReplyPath::IgnoredNavigation:
  case ReplyPath::PreviousCandidate:
  case ReplyPath::NextCandidate:
  case ReplyPath::PreviousPage:
  case ReplyPath::NextPage:
  case ReplyPath::Composition:
    if (!delta.empty() || !result.reply_expected) {
      invalid();
      break;
    }
    if (!uiless) {
      const auto navigation = navigation_for(path);
      next.encoded = navigation
                         ? navigation_reply(result.request_id, *navigation)
                         : preedit_reply(result.request_id, prefix_ + display);
    } else {
      next.encoded = uiless_composition(result.request_id, prefix_ + display, view);
    }
    break;
  case ReplyPath::SyllableCommit:
    // No selected prefix exists in Korean, and the commit is already in the document by the time this reply is read: the TIP's host session produced the same syllable for the same key. Only the composition after it is encoded.
    if (!prefix_.empty()) {
      invalid();
      break;
    }
    if (result.reply_expected) {
      if (raw.empty())
        next.encoded = ignored_reply(result.request_id);
      else
        next.encoded = uiless ? uiless_composition(result.request_id, display, view)
                              : preedit_reply(result.request_id, display);
    }
    next.next_prefix.clear();
    if (!output_delta.empty())
      next.committed_text = output_delta;
    break;
  case ReplyPath::AutoCommitAndContinue: {
    // Two Wubi commits take this path: the fourth letter of a unique code, which leaves nothing to compose, and a letter typed after a complete code (顶字), which commits the first candidate and leaves that letter composing. The worker frame tells the TIP to consume the four letters of the committed code from its own buffer and keep whatever follows, so the key reply only has to show the composition the Engine now holds.
    const auto &context = result.transition.at("commit_context");
    if (delta.empty() || context.is_null() ||
        context.value("scheme", 255u) != 2u) {
      invalid();
      break;
    }
    const auto total = prefix_ + output_delta;
    next.worker = commit_candidate_and_continue_bytes(4, total);
    if (!next.worker) {
      invalid();
      break;
    }
    if (result.reply_expected)
      next.encoded = uiless ? uiless_composition(result.request_id, display, view)
                            : preedit_reply(result.request_id, display);
    next.next_prefix.clear();
    next.committed_text = total;
    break;
  }
  }
  session_ = session;
  pending_ = std::move(next);
  return *pending_;
}
const PendingReply &ReplyComposer::pending() const {
  if (!pending_)
    throw std::logic_error("No pending reply");
  return *pending_;
}
const PendingReply &ReplyComposer::dispatch(
    ServerSession &session, const FanyImeNamedpipeData &packet, uint64_t epoch,
    ReplyPath path, bool uiless, std::optional<std::string> local_text) {
  if (pending_ || packet.client_id != client_ || epoch != epoch_)
    throw std::logic_error("Pending or expired Windows reply route");
  traditional_output_ = traditional_projection(session);
  if (session_ && session.view().at("session").get<uint64_t>() != session_)
    throw std::logic_error("Reply changed host session");
  if (path == ReplyPath::LocalCommit && session.input_enabled()) {
    const auto action = translate_key(packet);
    const auto raw = session.view().at("editing_text").get<std::string>();
    // The TSF has already inserted its local text. Validate that observation
    // before MSIME_COMMIT_RAW can clear Engine state. The postflight check in
    // stage() remains necessary; never manufacture proof from Engine's result.
    if (action.kind != KeyKind::Command || action.value != MSIME_COMMIT_RAW ||
        !local_text || *local_text != prefix_ + raw)
      throw std::invalid_argument("Invalid local commit observation");
  }
  const auto action = translate_key(packet);
  const bool candidate_enter =
      path == ReplyPath::Selection && action.kind == KeyKind::Command &&
      action.value == MSIME_COMMIT_RAW;
  std::string selected_raw_before;
  KeyResult result;
  if (path == ReplyPath::Punctuation) {
    result = session.punctuation(packet, epoch);
  } else if (candidate_enter) {
    if (packet.event_type != FanyImePipeEventType::KeyEvent ||
        !packet.request_id || packet.request_id == FANY_IME_NO_REQUEST_ID ||
        packet.pinyin_length < 0 || packet.pinyin_length >= 128)
      throw std::invalid_argument("Invalid Windows candidate Enter request");
    const auto current = session.view();
    selected_raw_before = current.at("editing_text").get<std::string>();
    std::optional<std::pair<uint64_t, size_t>> highlighted;
    for (const auto &candidate : current.at("candidates")) {
      if (!candidate.at("highlighted").get<bool>())
        continue;
      if (highlighted)
        throw std::logic_error("Ambiguous candidate highlight");
      const auto &id = candidate.at("id");
      highlighted = std::pair<uint64_t, size_t>{
          id.at("generation").get<uint64_t>(), id.at("index").get<size_t>()};
    }
    if (!highlighted)
      throw std::logic_error("Missing candidate highlight");
    auto transition = session.select(epoch, highlighted->first, highlighted->second);
    result = {client_, epoch, packet.request_id, true, std::move(transition)};
  } else {
    result = session.key(packet, epoch);
  }
  if (!session.input_enabled())
    path = result.reply_expected ? ReplyPath::IgnoredNavigation
                                 : ReplyPath::NoReply;
  const auto &pending = stage(result, path, uiless, std::move(local_text));
  if (candidate_enter && !selected_raw_before.empty()) {
    const auto after = result.transition.at("view").at("editing_text").get<std::string>();
    if (after.size() < selected_raw_before.size() &&
        selected_raw_before.compare(selected_raw_before.size() - after.size(),
                                    after.size(), after) == 0) {
      pending_->segment_restore = PendingReply::SegmentRestore{
          selected_raw_before.substr(0, selected_raw_before.size() - after.size()),
          prefix_};
    }
  }
  return pending;
}
std::optional<PendingReply> ReplyComposer::basic_key(
    ServerSession &session, const FanyImeNamedpipeData &packet, uint64_t epoch,
    TsfPreeditStyle style, std::optional<std::string> local_text) {
  if (pending_ || packet.client_id != client_ || epoch != epoch_)
    throw std::logic_error("Pending or expired Windows reply route");
  traditional_output_ = traditional_projection(session);
  if (style != TsfPreeditStyle::Local && style != TsfPreeditStyle::Pinyin &&
      style != TsfPreeditStyle::Empty)
    throw std::invalid_argument("Invalid TSF preedit style");
  if (auto restored = restore_segment(session, packet, epoch))
    return restored;
  if (translation_page_active_) {
    if (auto translation = translation_page_key(session, packet, epoch))
      return translation;
  }
  // Ahead of the reset route, which would treat Escape as discarding the syllable when with the list open it only closes the list.
  if (auto hanja = korean_hanja(session, packet, epoch))
    return hanja;
  const auto action = translate_key(packet);
  const bool uiless = (packet.modifiers_down & FanyImePipeFlags::UiLess) != 0;
  if (action.kind == KeyKind::LocalReset) {
    // The first Escape on a Vietnamese word shows its raw keys again and the word keeps composing, so there is no cleared composition to report; the TIP stays composing with the same keys.
    const auto current = session.view();
    const bool restores_raw = packet.keycode == kVirtualKeyEscape && session.input_enabled() &&
                              scheme::CancelRestoresRaw(view_scheme(current)) &&
                              !current.at("editing_text").get<std::string>().empty();
    return dispatch(session, packet, epoch, restores_raw ? ReplyPath::NoReply : ReplyPath::LocalCancel, uiless);
  }
  if (action.kind == KeyKind::Ignore)
    return dispatch(session, packet, epoch, ReplyPath::NoReply, uiless);
  // Ctrl+Shift+E reaches the Server as an ordinary key whose composition the TSF has already cancelled without reading a reply, the same contract as Escape. Toggle the mode here, as the reference does unconditionally, instead of letting the generic modifier fallback drop it.
  if (is_english_mode_toggle_key(packet.keycode,
                                 PipeMetadata::key_modifiers(packet.modifiers_down))) {
    const bool toggle = session.input_enabled();
    KeyResult result{client_, epoch_, packet.request_id, false,
                     nlohmann::json{{"commit", nullptr},
                                    {"view", toggle ? session.toggle_dedicated_english(epoch)
                                                    : session.view()}}};
    return stage(result, toggle ? ReplyPath::LocalCancel : ReplyPath::NoReply, uiless);
  }
  if (auto korean = korean_syllable_end(session, packet, epoch))
    return korean;
  // Control+Enter is a candidate-only translation action. It must be checked
  // before the generic modifier fallback, which intentionally forwards other
  // Control combinations to the host application.
  if (packet.keycode == 0x0D &&
      PipeMetadata::key_modifiers(packet.modifiers_down) == 2u &&
      (packet.modifiers_down & PipeMetadata::CandidateActive) != 0) {
    if (auto translation =
            commit_candidate_translation(session, packet, epoch))
      return translation;
  }
  if (action.kind == KeyKind::CancelAndForward)
    return std::nullopt; // Configuration-specific shortcuts are not generic
                         // cancel.
  if (action.kind == KeyKind::Command && action.value == MSIME_COMMIT_RAW) {
    const auto current = session.view();
    const bool candidate_active =
        (packet.modifiers_down & PipeMetadata::CandidateActive) != 0;
    const bool has_candidates = candidate_active && !current.at("candidates").empty();
    const auto path = has_candidates ? ReplyPath::Selection : ReplyPath::LocalCommit;
    return dispatch(session, packet, epoch, path, uiless,
                    std::move(local_text));
  }
  if (auto edited = edit(session, packet, epoch, style))
    return edited;
  const auto view = session.view();
  const auto mode = view.at("local_mode").get<std::string>();
  if (mode == "unknown")
    return std::nullopt;
  const auto key = normalize_digit_key(packet.keycode);
  const auto modifiers = PipeMetadata::key_modifiers(packet.modifiers_down);
  const bool digit = key >= '1' && key <= '9';
  if ((key == 0x20 && modifiers == 0) ||
      (digit && digit_selects_candidate(
                    mode, view.value("spelling_symbols", std::string{}),
                    static_cast<uint32_t>(packet.wch), modifiers)))
    return dispatch(session, packet, epoch, ReplyPath::Selection, uiless);
  return std::nullopt;
}
std::optional<PendingReply> ReplyComposer::toggle_character_set(
    ServerSession &session, const FanyImeNamedpipeData &packet,
    uint64_t epoch, bool enabled,
    const std::function<bool(bool)> &persist) {
  if (pending_ || packet.client_id != client_ || epoch != epoch_ ||
      packet.event_type != FanyImePipeEventType::KeyEvent ||
      !packet.request_id || packet.request_id == FANY_IME_NO_REQUEST_ID)
    throw std::logic_error("Invalid Windows character-set shortcut route");
  const bool desired = !session.traditional_output();
  const bool apply = enabled && session.input_enabled() &&
                     (!persist || persist(desired));
  if (apply)
    (void)session.toggle_traditional_output(epoch);
  traditional_output_ = traditional_projection(session);
  KeyResult result{client_, epoch_, packet.request_id, false,
                   nlohmann::json{{"commit", nullptr}, {"view", session.view()}}};
  return stage(result, ReplyPath::NoReply,
               (packet.modifiers_down & FanyImePipeFlags::UiLess) != 0);
}
std::optional<PendingReply>
ReplyComposer::edit(ServerSession &session, const FanyImeNamedpipeData &packet,
                    uint64_t epoch, TsfPreeditStyle style) {
  if (pending_ || packet.client_id != client_ || epoch != epoch_)
    throw std::logic_error("Pending or expired Windows reply route");
  traditional_output_ = traditional_projection(session);
  if (style != TsfPreeditStyle::Local && style != TsfPreeditStyle::Pinyin &&
      style != TsfPreeditStyle::Empty)
    throw std::invalid_argument("Invalid TSF preedit style");
  const auto before = session.view();
  if (session_ && before.at("session").get<uint64_t>() != session_)
    throw std::logic_error("Reply changed host session");
  if (!session.input_enabled())
    return std::nullopt;
  const auto kind =
      edit_kind(packet, before.at("local_mode").get<std::string>(),
                !before.at("editing_text").get<std::string>().empty(),
                before.value("microsoft_shuangpin", false),
                before.at("editing_text").get<std::string>(),
                before.at("caret_position").get<size_t>(),
                before.value("scheme", 0u) == 3u,
                before.value("spelling_symbols", std::string{}),
                scheme::ApostropheIsPunctuationWhileComposing(view_scheme(before)));
  if (kind == EditKind::None)
    return std::nullopt;
  auto result = session.key(packet, epoch);
  const bool uiless = (packet.modifiers_down & FanyImePipeFlags::UiLess) != 0;
  const bool erased_all =
      kind == EditKind::Erase && result.transition.at("view")
                                     .at("editing_text")
                                     .get<std::string>()
                                     .empty();
  const auto path = uiless || (style != TsfPreeditStyle::Local &&
                               kind != EditKind::Caret && !erased_all)
                        ? ReplyPath::Composition
                        : ReplyPath::NoReply;
  result.reply_expected = path != ReplyPath::NoReply;
  const bool auto_wubi_commit =
      !result.transition.at("commit").is_null() &&
      !result.transition.at("commit_context").is_null() &&
      result.transition.at("commit_context").value("scheme", 255u) == 2u;
  // A Korean letter that starts a new syllable carries the finished one as its commit, a Vietnamese key typed after a finished word commits it, and a Zhuyin syllable past the conversion's limit commits its first word. The TIP writes each of them from its own host session (scheme::AlwaysInlinePreedit), so here they are only counted.
  const bool tip_commit =
      !result.transition.at("commit").is_null() &&
      scheme::AlwaysInlinePreedit(view_scheme(result.transition.at("view")));
  return stage(result,
               auto_wubi_commit ? ReplyPath::AutoCommitAndContinue
               : tip_commit     ? ReplyPath::SyllableCommit
                                : path,
               uiless);
}
std::optional<PendingReply>
ReplyComposer::navigate(ServerSession &session,
                        const FanyImeNamedpipeData &packet, uint64_t epoch,
                        const NavigationBindings &bindings) {
  if (pending_ || packet.client_id != client_ || epoch != epoch_)
    throw std::logic_error("Pending or expired Windows reply route");
  traditional_output_ = traditional_projection(session);
  if (session_ && session.view().at("session").get<uint64_t>() != session_)
    throw std::logic_error("Reply changed host session");
  auto result = session.navigate(packet, epoch, bindings);
  if (!result)
    return std::nullopt;
  ReplyPath path;
  switch (result->direction) {
  case NavigationReply::Ignored:
    path = ReplyPath::IgnoredNavigation;
    break;
  case NavigationReply::PreviousCandidate:
    path = ReplyPath::PreviousCandidate;
    break;
  case NavigationReply::NextCandidate:
    path = ReplyPath::NextCandidate;
    break;
  case NavigationReply::PreviousPage:
    path = ReplyPath::PreviousPage;
    break;
  case NavigationReply::NextPage:
    path = ReplyPath::NextPage;
    break;
  default:
    throw std::logic_error("Invalid shared navigation result");
  }
  return stage(result->key, path,
               (packet.modifiers_down & FanyImePipeFlags::UiLess) != 0);
}
void ReplyComposer::confirm_delivery(uint64_t client, uint64_t epoch,
                                     uint64_t request) {
  const auto &current = pending();
  if (current.ui_selection || client != client_ || epoch != epoch_ ||
      request != current.source.request_id)
    throw std::logic_error("Expired reply delivery acknowledgement");
  if (current.encoded && !*current.encoded)
    throw std::logic_error("Unencodable reply cannot be acknowledged");
  prefix_ = current.next_prefix;
  if (current.segment_restore) {
    if (current.restoring_segment && !segment_restore_history_.empty() &&
        segment_restore_history_.back().raw == current.segment_restore->raw &&
        segment_restore_history_.back().previous_prefix ==
            current.segment_restore->previous_prefix)
      segment_restore_history_.pop_back();
    else if (!current.restoring_segment)
      segment_restore_history_.push_back(*current.segment_restore);
  }
  pending_.reset();
}
std::optional<PendingReply> ReplyComposer::select_candidate(ServerSession &session,
    uint64_t expected_session, uint64_t generation, size_t index) {
  if (pending_ || !session.input_enabled()) return std::nullopt;
  traditional_output_ = traditional_projection(session);
  const auto view = session.view();
  if (!expected_session || view.at("session") != expected_session ||
      (session_ && session_ != expected_session) ||
      view.at("generation") != generation || !view.at("focused").get<bool>())
    return std::nullopt;
  // A pick made here would leave the TIP's own host session behind (scheme::KeyboardOnlyCandidateList).
  if (scheme::KeyboardOnlyCandidateList(view_scheme(view)))
    return std::nullopt;
  const auto raw_before = view.at("editing_text").get<std::string>();
  if (translation_page_active_) {
    if (index >= translation_page_items_.size())
      return std::nullopt;
    const auto text = simplified_to_traditional(translation_page_items_[index],
                                                traditional_output_);
    session.cancel_composition(epoch_);
    auto transition = session.view();
    transition["commit"] = nullptr;
    PendingReply next;
    next.source = {client_, epoch_, 0, true, std::move(transition)};
    next.next_prefix.clear();
    next.traditional_output = traditional_output_;
    next.ui_selection = ui_complete_selection(prefix_ + text);
    next.committed_text = prefix_ + text;
    if (!next.ui_selection)
      throw std::runtime_error("Unencodable translation selection");
    translation_page_active_ = false;
    translation_page_items_.clear();
    session_ = expected_session;
    pending_ = std::move(next);
    return pending_;
  }
  bool found = false;
  for (const auto &candidate : view.at("candidates")) {
    const auto &id = candidate.at("id");
    if (id.at("session") == expected_session &&
        id.at("generation") == generation && id.at("index") == index)
      found = true;
  }
  if (!found)
    return std::nullopt;
  auto transition = session.select(epoch_, generation, index);
  const auto &commit = transition.at("commit");
  const auto delta =
      commit.is_null() ? std::string{} : commit.get<std::string>();
  const auto output_delta =
      simplified_to_traditional(delta, traditional_output_);
  const auto &next_view = transition.at("view");
  const auto raw = next_view.at("editing_text").get<std::string>();
  PendingReply next;
  next.source = {client_, epoch_, 0, true, std::move(transition)};
  next.next_prefix = prefix_;
  next.traditional_output = traditional_output_;
  if (delta.empty())
    next.ui_selection = ui_rejected_selection();
  else if (raw.empty()) {
    next.ui_selection = ui_complete_selection(prefix_ + output_delta);
    next.next_prefix.clear();
    next.committed_text = prefix_ + output_delta;
  } else {
    next.next_prefix = prefix_ + output_delta;
    const auto &raw_after = next.source.transition.at("view").at("editing_text").get<std::string>();
    if (raw_after.size() < raw_before.size() &&
        raw_before.compare(raw_before.size() - raw_after.size(),
                          raw_after.size(), raw_after) == 0) {
      next.segment_restore = PendingReply::SegmentRestore{
          raw_before.substr(0, raw_before.size() - raw_after.size()), prefix_};
    }
    next.ui_selection = ui_partial_selection(
        raw, next.next_prefix,
        next.next_prefix +
            next.source.transition.at("view").at("preedit").get<std::string>());
  }
  if (!next.ui_selection)
    throw std::runtime_error(
        "Unencodable UI selection; disconnect without replay");
  session_ = expected_session;
  pending_ = std::move(next);
  return pending_;
}

std::optional<PendingReply> ReplyComposer::commit_candidate_translation(
    ServerSession &session, const FanyImeNamedpipeData &packet,
    uint64_t epoch) {
  if (pending_ || packet.client_id != client_ || epoch != epoch_ ||
      !session.input_enabled())
    throw std::logic_error("Invalid candidate translation commit route");
  if (packet.keycode != 0x0D ||
      PipeMetadata::key_modifiers(packet.modifiers_down) != 2u ||
      (packet.modifiers_down & PipeMetadata::CandidateActive) == 0)
    return std::nullopt;
  traditional_output_ = traditional_projection(session);
  const auto view = session.view();
  if (!view.at("focused").get<bool>() || view.at("candidates").empty())
    return std::nullopt;
  // The TIP never sends this for Korean, Zhuyin or Vietnamese: it composes them in its own host session, which a commit made here would leave behind. A Hanja row's translation is display only, like its 훈음, which never reaches the view's translation at all.
  if (scheme::AlwaysInlinePreedit(view_scheme(view)))
    return std::nullopt;
  const auto generation = view.at("generation").get<uint64_t>();
  const auto expected_session = view.at("session").get<uint64_t>();
  std::string translation;
  bool found = false;
  for (const auto &candidate : view.at("candidates")) {
    if (!candidate.value("highlighted", false)) {
      continue;
    }
    translation = candidate.value("translation", std::string{});
    found = true;
    break;
  }
  if (!found || translation.empty())
    return std::nullopt;
  const auto senses = translation_senses(translation);
  if (senses.size() > 1) {
    nlohmann::json page = {{"commit", nullptr}, {"view", view}};
    page["view"]["candidates"] = nlohmann::json::array();
    const auto count = std::min<size_t>(senses.size(), 9);
    for (size_t item = 0; item < count; ++item) {
      page["view"]["candidates"].push_back({
          {"id", {{"session", expected_session}, {"generation", generation},
                   {"index", item}}},
          {"text", senses[item]}, {"highlighted", item == 0},
          {"annotation", ""}, {"source", 0}, {"fixed_position", false},
          {"translation", ""}});
    }
    translation_page_items_.assign(senses.begin(), senses.begin() + count);
    translation_page_view_ = page;
    translation_page_active_ = true;
    return translation_page_reply(packet, epoch, page);
  }
  // Like the reference HandleTranslationCommitKey, a single sense is committed as exact text and the composition is dropped: the gloss is not a candidate the user picked, so it must not go through selection and teach the Engine.
  const auto output = simplified_to_traditional(
      first_translation_sense(translation), traditional_output_);
  session.cancel_composition(epoch);
  auto transition = session.view();
  transition["commit"] = nullptr;
  PendingReply next;
  next.source = {client_, epoch_, packet.request_id, true, transition};
  next.encoded = exact_commit(packet.request_id, prefix_ + output);
  next.next_prefix.clear();
  next.committed_text = prefix_ + output;
  next.traditional_output = traditional_output_;
  session_ = expected_session;
  pending_ = std::move(next);
  return pending_;
}

PendingReply ReplyComposer::translation_page_reply(
    const FanyImeNamedpipeData &packet, uint64_t epoch,
    const nlohmann::json &transition) {
  PendingReply next;
  next.source = {client_, epoch, packet.request_id, true, transition};
  next.encoded = navigation_reply(packet.request_id, NavigationReply::Ignored);
  next.next_prefix = prefix_;
  next.traditional_output = traditional_output_;
  session_ = transition.at("view").at("session").get<uint64_t>();
  pending_ = std::move(next);
  return *pending_;
}

std::optional<PendingReply> ReplyComposer::restore_segment(
    ServerSession &session, const FanyImeNamedpipeData &packet,
    uint64_t epoch) {
  if (packet.keycode != 0x08 ||
      PipeMetadata::key_modifiers(packet.modifiers_down) != 2u ||
      (packet.modifiers_down & PipeMetadata::CandidateActive) != 0 ||
      prefix_.empty() || segment_restore_history_.empty())
    return std::nullopt;
  const auto current = session.view();
  if (!current.at("focused").get<bool>() ||
      !current.at("editing_text").get<std::string>().empty())
    return std::nullopt;
  const auto &entry = segment_restore_history_.back();
  auto result = session.restore_raw(epoch, packet.request_id, entry.raw);
  const auto &view = result.transition.at("view");
  const auto raw = view.at("editing_text").get<std::string>();
  const auto display = view.at("preedit").get<std::string>();
  PendingReply next;
  next.source = std::move(result);
  next.next_prefix = entry.previous_prefix;
  next.traditional_output = traditional_output_;
  next.segment_restore = entry;
  next.restoring_segment = true;
  if (!raw.empty() && !entry.previous_prefix.empty())
    next.encoded = partial_selection(packet.request_id, raw,
                                     entry.previous_prefix,
                                     entry.previous_prefix + display);
  else if (!raw.empty())
    next.encoded = preedit_reply(packet.request_id, display);
  else
    next.encoded = ignored_reply(packet.request_id);
  if (!next.encoded || !*next.encoded)
    throw std::runtime_error("Unencodable segment restoration");
  pending_ = std::move(next);
  return pending_;
}

std::optional<PendingReply> ReplyComposer::korean_hanja(
    ServerSession &session, const FanyImeNamedpipeData &packet,
    uint64_t epoch) {
  if (!session.input_enabled() ||
      packet.event_type != FanyImePipeEventType::KeyEvent ||
      (PipeMetadata::key_modifiers(packet.modifiers_down) & ~1u) != 0)
    return std::nullopt;
  const auto key = korean_hanja_key(packet.keycode, static_cast<uint32_t>(packet.wch));
  if (key.kind == KoreanHanjaKeyKind::None)
    return std::nullopt;
  const auto current = session.view();
  const int scheme = view_scheme(current);
  // The scheme's rules hold only outside the dedicated English mode and every local mode, which keep their own rules in that scheme.
  const bool rules = scheme::OpensCandidateList(scheme) &&
                     !current.value("dedicated_english", false) &&
                     current.at("local_mode").get<std::string>() == "none";
  const bool composing = rules && !current.at("editing_text").get<std::string>().empty();
  // Under these rules the Engine lists candidates only once the list is opened, so a composing view with candidates is the open list.
  const bool list_open = composing && !current.at("candidates").empty();
  const bool trigger = composing && opens_candidate_list(scheme, packet.keycode, list_open);
  const bool hanja_key = packet.keycode == kVirtualKeyHanja;
  if (!trigger && !hanja_key && !list_open)
    return std::nullopt;
  const bool uiless = (packet.modifiers_down & FanyImePipeFlags::UiLess) != 0;
  nlohmann::json transition;
  if (!trigger && (hanja_key || !list_open)) {
    // The TIP eats the Hanja key only while a Korean syllable composes. One that arrives after the syllable ended, or after the scheme changed, is spent here too rather than left with no reply; in every other scheme it is no key of a list and is spent the same way.
    transition = {{"handled", false}, {"commit", nullptr}, {"view", current}};
  } else if (trigger) {
    transition = session.command(epoch, MSIME_OPEN_CANDIDATE_LIST);
  } else if (key.kind == KoreanHanjaKeyKind::Select) {
    // A digit past the visible page chooses nothing.
    const auto &page = current.at("candidates");
    if (key.value < page.size()) {
      const auto &id = page.at(key.value).at("id");
      transition = session.select(epoch, id.at("generation").get<uint64_t>(), id.at("index").get<size_t>());
    } else {
      transition = {{"handled", true}, {"commit", nullptr}, {"view", current}};
    }
  } else {
    transition = session.command(epoch, key.value);
  }
  // The TIP has already written a chosen Hanja from its own session, so a commit is only counted, as for a syllable. A Zhuyin choice commits nothing: it fixes that span of the conversion, which keeps composing.
  const auto path = transition.at("commit").is_null() ? ReplyPath::NoReply : ReplyPath::SyllableCommit;
  return stage({client_, epoch_, packet.request_id, false, std::move(transition)}, path, uiless);
}

std::optional<PendingReply> ReplyComposer::korean_syllable_end(
    ServerSession &session, const FanyImeNamedpipeData &packet,
    uint64_t epoch) {
  if (!session.input_enabled() ||
      packet.event_type != FanyImePipeEventType::KeyEvent ||
      (PipeMetadata::key_modifiers(packet.modifiers_down) & ~1u) != 0)
    return std::nullopt;
  switch (packet.keycode) {
  case 0x09: // Tab
  case 0x0D: // Enter
  case 0x21: // Page Up
  case 0x22: // Page Down
  case 0x23: // End
  case 0x24: // Home
  case 0x25: // Left
  case 0x26: // Up
  case 0x27: // Right
  case 0x28: // Down
  case 0x2D: // Insert
  case 0x2E: // Delete
    break;
  default:
    return std::nullopt;
  }
  const auto current = session.view();
  // Korean, Zhuyin and Vietnamese compose in the TIP's own host session and write the composition out when a key leaves it (scheme::CommitsOnBlur); an open Zhuyin list took its keys in korean_hanja already.
  if (!scheme::CommitsOnBlur(view_scheme(current)) ||
      current.at("local_mode").get<std::string>() != "none" ||
      current.at("editing_text").get<std::string>().empty())
    return std::nullopt;
  // The TIP committed the composition and the key goes on to the application, directly or replayed after a queued commit (Zhuyin's Enter only commits), so nothing is sent back.
  return stage({client_, epoch_, packet.request_id, false,
                session.finish_composition(epoch)},
               ReplyPath::SyllableCommit,
               (packet.modifiers_down & FanyImePipeFlags::UiLess) != 0);
}

std::optional<PendingReply> ReplyComposer::translation_page_key(
    ServerSession &session, const FanyImeNamedpipeData &packet,
    uint64_t epoch) {
  if (!translation_page_active_)
    return std::nullopt;
  if (packet.event_type != FanyImePipeEventType::KeyEvent ||
      (packet.modifiers_down & PipeMetadata::CandidateActive) == 0 ||
      PipeMetadata::key_modifiers(packet.modifiers_down) != 0)
    return std::nullopt;
  const auto key = normalize_digit_key(packet.keycode);
  const bool navigation = packet.keycode == 0x21 || packet.keycode == 0x22 ||
                          packet.keycode == 0x23 || packet.keycode == 0x24 ||
                          packet.keycode == 0x26 || packet.keycode == 0x28;
  if (navigation) {
    if (PipeMetadata::key_modifiers(packet.modifiers_down) == 0) {
      return translation_page_reply(packet, epoch, translation_page_view_);
    }
    translation_page_active_ = false;
    translation_page_items_.clear();
    translation_page_view_ = {};
    return std::nullopt;
  }
  size_t index = std::numeric_limits<size_t>::max();
  if (packet.keycode == 0x20)
    index = 0;
  else {
    if (key >= '1' && key <= '9')
      index = static_cast<size_t>(key - '1');
  }
  if (index == std::numeric_limits<size_t>::max()) {
    translation_page_active_ = false;
    translation_page_items_.clear();
    return std::nullopt;
  }
  if (index >= translation_page_items_.size())
    return translation_page_reply(packet, epoch, translation_page_view_);
  const auto text = simplified_to_traditional(translation_page_items_[index],
                                              traditional_output_);
  session.cancel_composition(epoch);
  auto transition = session.view();
  transition["commit"] = nullptr;
  PendingReply next;
  next.source = {client_, epoch, packet.request_id, true, transition};
  next.encoded = exact_commit(packet.request_id, prefix_ + text);
  next.next_prefix.clear();
  next.committed_text = prefix_ + text;
  next.traditional_output = traditional_output_;
  translation_page_active_ = false;
  translation_page_items_.clear();
  translation_page_view_ = {};
  pending_ = std::move(next);
  return *pending_;
}

void ReplyComposer::confirm_ui_delivery(uint64_t client, uint64_t epoch,
                                        uint64_t generation) {
  const auto &current = pending();
  if (!current.ui_selection || client != client_ || epoch != epoch_ ||
      current.source.transition.at("view").at("generation") != generation)
    throw std::logic_error("Expired UI delivery acknowledgement");
  prefix_ = current.next_prefix;
  if (current.segment_restore) {
    if (current.restoring_segment && !segment_restore_history_.empty() &&
        segment_restore_history_.back().raw == current.segment_restore->raw &&
        segment_restore_history_.back().previous_prefix ==
            current.segment_restore->previous_prefix)
      segment_restore_history_.pop_back();
    else if (!current.restoring_segment)
      segment_restore_history_.push_back(*current.segment_restore);
  }
  pending_.reset();
}
void ReplyComposer::cancel() {
  pending_.reset();
  prefix_.clear();
  segment_restore_history_.clear();
  translation_page_active_ = false;
  translation_page_items_.clear();
  translation_page_view_ = {};
}
std::optional<PendingReply> ReplyComposer::configured_key(
    ServerSession &session, const FanyImeNamedpipeData &packet, uint64_t epoch,
    TsfPreeditStyle style, const NavigationBindings &bindings,
    std::optional<std::string> local_text, WordCharacterBinding word_binding) {
  if (pending_ || packet.client_id != client_ || epoch != epoch_)
    throw std::logic_error("Pending or expired Windows reply route");
  traditional_output_ = traditional_projection(session);
  if (style != TsfPreeditStyle::Local && style != TsfPreeditStyle::Pinyin &&
      style != TsfPreeditStyle::Empty)
    throw std::invalid_argument("Invalid TSF preedit style");
  if (auto word = session.word_character(packet, epoch, word_binding))
    return stage(word->key, word->exact
                                ? ReplyPath::Punctuation
                                : ReplyPath::CandidatePunctuationFallback);
  if (auto basic =
          basic_key(session, packet, epoch, style, std::move(local_text)))
    return basic;
  // basic_key checked pending/route/style and left unsupported keys untouched.
  const auto current = session.view();
  if (!session.input_enabled() || current.at("local_mode") == "unknown")
    return std::nullopt;
  const bool composing =
      !current.at("editing_text").get<std::string>().empty();
  // Checked first: the numpad decimal is also on the candidate punctuation list, where it would be translated to '。'.
  if ((composing && literal_candidate_punctuation(packet)) ||
      candidate_punctuation(packet, bindings, view_scheme(current) == scheme::Japanese,
                            !scheme::UsesChinesePunctuation(view_scheme(current)) ||
                                scheme::OpensCandidateList(view_scheme(current))))
    return dispatch(session, packet, epoch, ReplyPath::Punctuation,
                    (packet.modifiers_down & FanyImePipeFlags::UiLess) != 0);
  if (!composing)
    return std::nullopt;
  return navigate(session, packet, epoch, bindings);
}
} // namespace msime::windows

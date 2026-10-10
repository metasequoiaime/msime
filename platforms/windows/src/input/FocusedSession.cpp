#include "FocusedSession.h"
#include "../../../common/HostApiString.h"
#include "FloatingToolbarVisibilityPolicy.h"
#include "InputSchemeTraits.h"
#include "KeySoundPolicy.h"
#include "TypingEffectPolicy.h"
#include "TypingEffectSignal.h"
#include <ctime>
#include <memory>
#include <stdexcept>
#include <thread>
#include <utility>
#include "FullscreenForeground.h"

namespace msime::windows {
namespace {
// Effect sounds and music stay quiet while a full-screen application is in front: a game, a video or a presentation the user did not ask to hear typing in.
bool sound_allowed() { return !foreground_is_fullscreen(GetForegroundWindow()); }
} // namespace
std::string
FocusedSession::typing_statistics_directory(const std::string &options) {
  try {
    return nlohmann::json::parse(options).value("preferences_directory",
                                                std::string{});
  } catch (...) {
    // Statistics are optional. A configuration this session already accepted
    // must not be re-litigated here.
    return {};
  }
}
namespace {
// Off the calling thread: the shared store takes a file lock, and neither a commit nor a pipe listener may wait on statistics. Detached like the other hosts do; the request is a self-contained copy, so nothing here outlives it.
void submit_typing_statistics_request(std::string request) {
  try {
    std::thread([payload = std::move(request)] {
      try {
        if (auto *raw = msime_client_typing_statistics(
                reinterpret_cast<const uint8_t *>(payload.data()),
                payload.size()))
          msime::host_api::discard_string(raw);
      } catch (...) {
        // Best effort; text commitment has already happened.
      }
    }).detach();
  } catch (...) {
    // Thread exhaustion drops the record rather than the keystroke.
  }
}
} // namespace
void record_typing_statistics_async(const std::string &directory,
                                    const std::string &text,
                                    TypingSource source, bool quiet) {
  if (directory.empty())
    return;
  const auto local = local_time_parts(std::time(nullptr));
  if (!local)
    return;
  auto request = typing_statistics_record_request(
      directory, text, source, local->day, local->hour, quiet);
  if (request.empty())
    return;
  submit_typing_statistics_request(std::move(request));
}
void record_typing_keys_async(const std::string &directory,
                              const std::string &day,
                              const std::map<std::string, uint64_t> &keys) {
  auto request = typing_statistics_record_keys_request(directory, day, keys);
  if (request.empty())
    return;
  submit_typing_statistics_request(std::move(request));
}
std::optional<FocusedSession::Commit> FocusedSession::pending_commit() const {
  if (!composer_ || !composer_->has_pending())
    return std::nullopt;
  const auto &reply = composer_->pending();
  if (!reply.committed_text || reply.committed_text->empty())
    return std::nullopt;
  return Commit{*reply.committed_text,
                resolve_typing_source_from_transition(reply.source.transition),
                transition_counts_as_typing(reply.source.transition)};
}
void FocusedSession::record_commit(const std::optional<Commit> &delivered) {
  if (!delivered)
    return;
  // Sampled once, at commit time: the achievement jingle a milestone plays follows the same full-screen rule as the commit sound.
  const bool allowed = sound_allowed();
  if (allowed)
    (void)session_.commit_sound();
  // The commit flash, on the session's current combo: a commit counts nothing and only reports the state.
  if (session_.input_enabled()) {
    publish_typing_effect_settings();
    TypingEffectSignal::instance().publish(
        typing_effect_mark_commit(session_.typing_effect(typing_effect_commit(allowed))));
  }
  if (!delivered->typing)
    return;
  if (statistics_) {
    statistics_(delivered->text, delivered->source);
    return;
  }
  record_typing_statistics_async(statistics_directory_, delivered->text,
                                 delivered->source, !allowed);
}
void FocusedSession::check_thread() const {
  if (std::this_thread::get_id() != thread_)
    throw std::logic_error("Wrong focused session thread");
}
bool FocusedSession::prepared(const FocusLease &lease) const {
  return lease_ && composer_ && lease_->epoch == lease.epoch &&
         lease_->token == lease.token &&
         same_ticket(lease_->transport, lease.transport);
}
void FocusedSession::attach_online_query(
    const FocusLease &lease, std::optional<PendingReply> &reply) {
  if (reply) {
    reply->online_query = session_.online_query(lease.epoch);
    if (reply->online_query) {
      try {
        reply->ai_request =
            session_.ai_request(lease.epoch, *reply->online_query);
      } catch (...) {
        // AI configuration is optional; keep the ordinary reply path intact.
        reply->ai_request.reset();
      }
    }
    reply->translation_query = session_.translation_query(lease.epoch);
  }
}
std::optional<nlohmann::json>
FocusedSession::dedicated_english(const FocusLease &lease, bool exit) {
  check_thread();
  if (!prepared(lease) || composer_->has_pending())
    return std::nullopt;
  std::optional<nlohmann::json> result;
  gate_.with_active(lease, [&] {
    result = session_.dedicated_english(lease.epoch, exit);
    if (exit) composer_->cancel();
  });
  return result;
}
std::optional<nlohmann::json>
FocusedSession::set_dedicated_english(const FocusLease &lease, bool enabled) {
  check_thread();
  if (!prepared(lease) || composer_->has_pending())
    return std::nullopt;
  std::optional<nlohmann::json> result;
  gate_.with_active(lease, [&] {
    const auto current = session_.view();
    // 托盘点击不经过 TIP：组字时切换会让 Engine 丢掉组字，而 TIP 手里的组字还留在编辑器里，两边从此对不上。所以只在没有组字、没有候选时切换，组字中的点击什么也不做。
    if (!current.at("editing_text").get<std::string>().empty() ||
        !current.at("candidates").empty())
      return;
    const bool changed = current.at("dedicated_english").get<bool>() != enabled;
    result = session_.set_dedicated_english(lease.epoch, enabled);
    // 切换后回复合成器记下的前缀和译文页属于旧模式，一并作废。
    if (changed) composer_->cancel();
  });
  return result;
}
bool FocusedSession::prepare(const FocusLease &lease) {
  check_thread();
  if (lease.transport.client != client_)
    return false;
  try {
    return gate_.with_pending(lease, [&] {
      if (prepared(lease))
        return;
      if (composer_)
        composer_->cancel();
      preferences_retry_.reset();
      continuation_hide_.clear();
      session_.activate(lease.epoch);
      composer_.emplace(client_, lease.epoch);
      lease_ = lease;
      session_.set_music_active(sound_allowed());
    });
  } catch (...) {
    gate_.deactivate(lease);
    composer_.reset();
    lease_.reset();
    throw;
  }
}
std::optional<PendingReply>
FocusedSession::key(const FocusLease &lease, const FanyImeNamedpipeData &packet,
                    ReplyPath path, bool uiless,
                    std::optional<std::string> local_text) {
  check_thread();
  if (!prepared(lease))
    return std::nullopt;
  std::optional<PendingReply> result;
  gate_.with_active(lease, [&] {
    result = composer_->dispatch(session_, packet, lease.epoch, path, uiless,
                                 std::move(local_text));
    attach_online_query(lease, result);
  });
  return result;
}
std::optional<PendingReply> FocusedSession::edit(const FocusLease &lease,
    const FanyImeNamedpipeData &packet, TsfPreeditStyle style) {
  check_thread();
  if (!prepared(lease)) return std::nullopt;
  std::optional<PendingReply> result;
  gate_.with_active(lease, [&] {
    result = composer_->edit(session_, packet, lease.epoch, style);
    attach_online_query(lease, result);
  });
  return result;
}
std::optional<PendingReply> FocusedSession::basic_key(
    const FocusLease &lease, const FanyImeNamedpipeData &packet,
    TsfPreeditStyle style, std::optional<std::string> local_text) {
  check_thread();
  if (!prepared(lease))
    return std::nullopt;
  std::optional<PendingReply> result;
  gate_.with_active(lease, [&] {
    result = composer_->basic_key(session_, packet, lease.epoch, style,
                                  std::move(local_text));
    attach_online_query(lease, result);
  });
  return result;
}
std::optional<PendingReply> FocusedSession::toggle_character_set(
    const FocusLease &lease, const FanyImeNamedpipeData &packet, bool enabled,
    const std::function<bool(bool)> &persist) {
  check_thread();
  if (!prepared(lease))
    return std::nullopt;
  std::optional<PendingReply> result;
  gate_.with_active(lease, [&] {
    result = composer_->toggle_character_set(session_, packet, lease.epoch,
                                              enabled, persist);
    attach_online_query(lease, result);
  });
  return result;
}
std::optional<PendingReply>
FocusedSession::navigate(const FocusLease &lease,
                         const FanyImeNamedpipeData &packet,
                         const NavigationBindings &bindings) {
  check_thread();
  if (!prepared(lease))
    return std::nullopt;
  std::optional<PendingReply> result;
  gate_.with_active(lease, [&] {
    result = composer_->navigate(session_, packet, lease.epoch, bindings);
    attach_online_query(lease, result);
  });
  return result;
}
bool FocusedSession::confirm(const FocusLease &lease, uint64_t request) {
  check_thread();
  if (!prepared(lease))
    return false;
  // Read before the acknowledgement, which clears the pending reply, and
  // recorded only if it went through: a throw leaves the commit unconfirmed,
  // and an unconfirmed commit is not in the document.
  const auto delivered = pending_commit();
  const bool auto_commit = composer_->pending().worker.has_value();
  const bool confirmed = gate_.with_active(lease, [&] {
    composer_->confirm_delivery(client_, lease.epoch, request);
    if (preferences_retry_) {
      session_.update_preferences(lease.epoch, preferences_retry_->dump());
      preferences_retry_.reset();
    }
  });
  if (confirmed) {
    if (auto_commit)
      continuation_hide_.arm(ContinuationHide::Clock::now());
    record_commit(delivered);
  }
  return confirmed;
}
std::optional<PendingReply>
FocusedSession::select_candidate(const FocusLease &lease, uint64_t session,
                                 uint64_t generation, size_t index) {
  check_thread();
  if (!prepared(lease))
    return std::nullopt;
  std::optional<PendingReply> result;
  gate_.with_active(lease, [&] {
    result = composer_->select_candidate(session_, session, generation, index);
    attach_online_query(lease, result);
  });
  return result;
}
std::optional<nlohmann::json>
FocusedSession::candidate_action(const FocusLease &lease, uint64_t session,
                                 uint64_t generation, size_t index,
                                 CandidateAction action, uint8_t position) {
  check_thread();
  if (!prepared(lease))
    return std::nullopt;
  std::optional<nlohmann::json> result;
  gate_.with_active(lease, [&] {
    if (composer_->has_pending())
      return;
    const auto current = session_.view();
    if (current.at("session").get<uint64_t>() != session ||
        current.at("generation").get<uint64_t>() != generation ||
        !current.at("focused").get<bool>())
      return;
    bool found = false;
    for (const auto &candidate : current.at("candidates")) {
      const auto &id = candidate.at("id");
      if (id.at("session").get<uint64_t>() == session &&
          id.at("generation").get<uint64_t>() == generation &&
          id.at("index").get<size_t>() == index) {
        found = true;
        break;
      }
    }
    if (found)
      result = session_.candidate_action(lease.epoch, generation, index,
                                         action, position);
  });
  return result;
}
std::optional<nlohmann::json>
FocusedSession::page_candidate(const FocusLease &lease, uint64_t session,
                               uint64_t generation, bool previous,
                               unsigned steps) {
  check_thread();
  if (!prepared(lease))
    return std::nullopt;
  std::optional<nlohmann::json> result;
  gate_.with_active(lease, [&] {
    if (composer_->has_pending())
      return;
    const auto current = session_.view();
    if (!current.at("focused").get<bool>() ||
        current.at("session").get<uint64_t>() != session ||
        current.at("generation").get<uint64_t>() != generation)
      return;
    // A page turned here would leave the list the TIP drives from its own host session on another page.
    if (scheme::KeyboardOnlyCandidateList(static_cast<int>(current.value("scheme", 0u))))
      return;
    result = session_.page_candidate(lease.epoch, session, generation,
                                     previous, steps);
  });
  return result;
}
bool FocusedSession::confirm_ui(const FocusLease &lease, uint64_t generation) {
  check_thread();
  if (!prepared(lease))
    return false;
  const auto delivered = pending_commit();
  const bool confirmed = gate_.with_active(lease, [&] {
    composer_->confirm_ui_delivery(client_, lease.epoch, generation);
    if (preferences_retry_) {
      session_.update_preferences(lease.epoch, preferences_retry_->dump());
      preferences_retry_.reset();
    }
  });
  if (confirmed)
    record_commit(delivered);
  return confirmed;
}
std::optional<PendingReply> FocusedSession::pending(const FocusLease &lease) {
  check_thread();
  if (!prepared(lease))
    return std::nullopt;
  std::optional<PendingReply> result;
  gate_.with_active(lease, [&] {
    if (composer_->has_pending())
      result = composer_->pending();
  });
  return result;
}
bool FocusedSession::set_input_enabled(const FocusLease &lease, bool enabled) {
  check_thread();
  if (!prepared(lease))
    return false;
  return gate_.with_active(lease, [&] {
    if (composer_->has_pending())
      throw std::logic_error("Input mode changed before reply delivery");
    session_.set_input_enabled(lease.epoch, enabled);
    if (!enabled)
      composer_->cancel();
  });
}
bool FocusedSession::set_chinese_punctuation(const FocusLease &lease, bool enabled) {
  check_thread();
  if (!prepared(lease))
    return false;
  return gate_.with_active(lease, [&] {
    if (composer_->has_pending())
      throw std::logic_error("Punctuation mode changed before reply delivery");
    session_.set_chinese_punctuation(lease.epoch, enabled);
  });
}
bool FocusedSession::balance_paired_punctuation(const FocusLease &lease,
                                                uint8_t opening) {
  check_thread();
  if (!prepared(lease))
    return false;
  return gate_.with_active(lease, [&] {
    session_.balance_paired_punctuation(lease.epoch, opening);
  });
}
bool FocusedSession::cancel_composition(const FocusLease &lease) {
  check_thread();
  if (!prepared(lease))
    return false;
  return gate_.with_active(lease, [&] {
    if (composer_->has_pending())
      throw std::logic_error("Composition cancelled before reply delivery");
    session_.cancel_composition(lease.epoch);
    composer_->cancel();
    continuation_hide_.clear();
  });
}
HideCandidateDisposition
FocusedSession::hide_candidate(const FocusLease &lease) {
  check_thread();
  if (!prepared(lease))
    return HideCandidateDisposition::Rejected;
  HideCandidateDisposition disposition = HideCandidateDisposition::Rejected;
  gate_.with_active(lease, [&] {
    if (composer_->has_pending())
      throw std::logic_error("Composition hidden before reply delivery");
    if (continuation_hide_.consume(ContinuationHide::Clock::now()) &&
        !session_.view().at("editing_text").get<std::string>().empty()) {
      disposition = HideCandidateDisposition::Suppressed;
      return;
    }
    session_.cancel_composition(lease.epoch);
    composer_->cancel();
    disposition = HideCandidateDisposition::Cancelled;
  });
  return disposition;
}
bool FocusedSession::reset_cache() {
  check_thread();
  try {
    session_.reset_cache();
    return true;
  } catch (...) {
    return false;
  }
}
bool FocusedSession::cancel(const FocusLease &lease) {
  check_thread();
  if (!prepared(lease))
    return false;
  // This can be cleanup for a previous owner; don't invalidate a new lease.
  gate_.deactivate(lease);
  composer_->cancel();
  preferences_retry_.reset();
  continuation_hide_.clear();
  session_.set_music_active(false);
  session_.deactivate(lease.epoch);
  composer_.reset();
  lease_.reset();
  return true;
}
bool FocusedSession::cancel_focus_token(uint64_t token) {
  check_thread();
  if (!token)
    return false;
  // Not focused, or focused under a later token: the session the DLL named is
  // already gone, so there is nothing left to tear down.
  if (!lease_ || lease_->token != token)
    return true;
  return cancel(*lease_);
}
std::optional<nlohmann::json>
FocusedSession::rerank_settled(const FocusLease &lease) {
  check_thread();
  if (!prepared(lease))
    return std::nullopt;
  std::optional<nlohmann::json> result;
  gate_.with_active(lease, [&] { result = session_.rerank_settled(lease.epoch); });
  return result;
}
std::optional<nlohmann::json>
FocusedSession::apply_cloud_response(const FocusLease &lease,
                                     const std::string &query,
                                     const std::string &body) {
  check_thread();
  if (!prepared(lease))
    return std::nullopt;
  std::optional<nlohmann::json> result;
  gate_.with_active(lease, [&] {
    result = session_.apply_cloud_response(lease.epoch, query, body);
  });
  return result;
}
std::optional<nlohmann::json>
FocusedSession::apply_ai_candidates(const FocusLease &lease,
                                    const std::string &query,
                                    const std::string &candidates) {
  check_thread();
  if (!prepared(lease))
    return std::nullopt;
  std::optional<nlohmann::json> result;
  gate_.with_active(lease, [&] {
    result = session_.apply_ai_candidates(lease.epoch, query, candidates);
  });
  return result;
}
std::optional<std::string>
FocusedSession::translation_query(const FocusLease &lease) {
  check_thread();
  if (!prepared(lease))
    return std::nullopt;
  std::optional<std::string> result;
  gate_.with_active(lease, [&] { result = session_.translation_query(lease.epoch); });
  return result;
}
std::optional<nlohmann::json>
FocusedSession::apply_translations(const FocusLease &lease, uint64_t generation,
                                   const std::string &translations) {
  check_thread();
  if (!prepared(lease))
    return std::nullopt;
  std::optional<nlohmann::json> result;
  gate_.with_active(lease, [&] {
    result = session_.apply_translations(lease.epoch, generation, translations);
  });
  return result;
}
std::optional<nlohmann::json>
FocusedSession::update_preferences(const FocusLease &lease,
                                   const std::string &snapshot) {
  check_thread();
  if (!prepared(lease))
    return std::nullopt;
  std::optional<nlohmann::json> result;
  gate_.with_active(lease, [&] {
    if (!composer_->has_pending())
      result = session_.update_preferences(lease.epoch, snapshot);
  });
  return result;
}
bool FocusedSession::queue_current_preferences(const std::string &snapshot) {
  check_thread();
  return lease_ && queue_preferences(*lease_, snapshot);
}
bool FocusedSession::queue_preferences(const FocusLease &lease,
                                       const std::string &snapshot) {
  check_thread();
  if (!prepared(lease))
    return false;
  return gate_.with_active(lease, [&] {
    if (snapshot.empty() || snapshot.size() > 16384)
      throw std::invalid_argument("Invalid preferences snapshot size");
    auto document = nlohmann::json::parse(snapshot);
    if (!document.is_object() || !document.contains("revision") ||
        !document.at("revision").is_number_unsigned() ||
        document.value("format_version", 0) != 1 ||
        !document.contains("preferences") ||
        !document.at("preferences").is_object())
      throw std::invalid_argument("Invalid preferences snapshot envelope");
    if (preferences_retry_) {
      const auto revision = document.at("revision").get<uint64_t>();
      const auto previous = preferences_retry_->at("revision").get<uint64_t>();
      if (revision < previous ||
          (revision == previous && document != *preferences_retry_))
        throw std::invalid_argument("Stale or conflicting queued preferences");
    }
    if (composer_->has_pending())
      preferences_retry_ = std::move(document);
    else {
      session_.update_preferences(lease.epoch, snapshot);
      preferences_retry_.reset();
    }
  });
}
std::optional<PendingReply> FocusedSession::configured_key(
    const FocusLease &lease, const FanyImeNamedpipeData &packet,
    TsfPreeditStyle style, const NavigationBindings &bindings,
    std::optional<std::string> local_text, WordCharacterBinding word_binding) {
  check_thread();
  if (!prepared(lease))
    return std::nullopt;
  std::optional<PendingReply> result;
  gate_.with_active(lease, [&] {
    // 不论输入法收不收这个键（英文模式也算），都是用户在打字：空闲隐藏的工具栏据此回来。
    ServerKeyActivity::instance().note();
    result = composer_->configured_key(session_, packet, lease.epoch, style,
                                       bindings, std::move(local_text),
                                       word_binding);
    // After the Engine, so only a key the input method took sounds (with input off, in English mode, the Server answers keys without taking them), and before the online queries are built, so they add no delay to it.
    if (result && session_.input_enabled()) {
      if (const auto key_class = key_sound_class(packet))
        sound_key(*key_class, (packet.modifiers_down & PipeMetadata::AutoRepeat) != 0);
    }
    attach_online_query(lease, result);
  });
  return result;
}
void FocusedSession::sound_key(uint32_t key_class, bool auto_repeat) {
  const bool allowed = sound_allowed();
  if (allowed)
    (void)session_.key_sound(key_class);
  // The same keys drive the typing effect and its combo, which keep counting in a full-screen application but stay silent there. The candidate window draws it on the UI thread; this only posts the packed value.
  publish_typing_effect_settings();
  TypingEffectSignal::instance().publish(
      session_.typing_effect(typing_effect_key_event(key_class, allowed, auto_repeat)));
}
void FocusedSession::publish_typing_effect_settings() {
  TypingEffectSignal::instance().publish_settings(pack_typing_effect_settings(session_.typing_effect_settings()));
  TypingEffectSignal::instance().publish_palette(session_.typing_effect_palette());
}
bool FocusedSession::passthrough_key(uint64_t token, uint32_t key_class) {
  check_thread();
  // 只认此刻持有焦点的那次激活：令牌对不上的是已经离开的会话，或者别的线程。
  if (!token || !lease_ || lease_->token != token || !prepared(*lease_))
    return false;
  bool sounded = false;
  gate_.with_active(*lease_, [&] {
    ServerKeyActivity::instance().note();
    // 英文模式不出声，和 Server 处理的键一样。交给应用的键没有自动重复：TIP 已经把它们滤掉了。
    if (!session_.input_enabled())
      return;
    sound_key(key_class, false);
    sounded = true;
  });
  return sounded;
}
} // namespace msime::windows

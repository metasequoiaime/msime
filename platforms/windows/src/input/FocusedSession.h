#pragma once
#include "ContinuationHide.h"
#include "FocusGate.h"
#include "ReplyComposer.h"
#include "TypingStatistics.h"
#include <functional>
#include <map>
#include <string>

namespace msime::windows {
// Receives the text of a commit that has been confirmed as delivered, together
// with the mode that produced it. Injected so the queue can be exercised
// without touching the shared store; the default writes it there.
using TypingStatisticsSink =
    std::function<void(const std::string &, TypingSource)>;
// Hands one record to the shared store on a detached thread. An empty directory or text records nothing; so does a store whose statistics are switched off, because the shared entry point checks that itself. `quiet` keeps a milestone's achievement jingle silent (see `typing_statistics_record_request`).
void record_typing_statistics_async(const std::string &directory,
                                    const std::string &text,
                                    TypingSource source, bool quiet = false);
// Hands one day's per-key press counts to the shared store on a detached thread. Same contract as above: nothing to record or a store with statistics off writes nothing.
void record_typing_keys_async(const std::string &directory,
                              const std::string &day,
                              const std::map<std::string, uint64_t> &keys);
enum class HideCandidateDisposition { Rejected, Cancelled, Suppressed };

// One registered client's queue-owned adapter. No pipe I/O runs here; the
// controller prepares on the input queue, writes the focus fence on an I/O
// worker, then enqueues keys. It must cancel the previous client's session too.
class FocusedSession final {
public:
  FocusedSession(FocusGate &gate, uint64_t client, const std::string &options)
      : gate_(gate), client_(client), session_(client, options),
        statistics_directory_(typing_statistics_directory(options)) {}
  // Replaces the default shared-store writer. The queue thread calls it
  // synchronously, so an injected sink must not block.
  void set_typing_statistics_sink(TypingStatisticsSink sink) {
    statistics_ = std::move(sink);
  }
  bool prepare(const FocusLease &lease);
  // No operation may invalidate a key reply awaiting transport confirmation.
  std::optional<nlohmann::json> dedicated_english(const FocusLease &lease, bool exit);
  // 把 Engine 的英文模式设成 `enabled`（托盘「英文候选模式」）。有等待送达确认的按键回复、正在组字或列着候选时都不动，返回空。
  std::optional<nlohmann::json> set_dedicated_english(const FocusLease &lease, bool enabled);
  std::optional<PendingReply>
  key(const FocusLease &lease, const FanyImeNamedpipeData &packet,
      ReplyPath path, bool uiless = false,
      std::optional<std::string> local_text = std::nullopt);
  // Only queue this after successful I/O or verified local-only completion.
  // A false return means the receipt is obsolete; never replay the key.
  bool confirm(const FocusLease &lease, uint64_t request);
  std::optional<PendingReply> select_candidate(const FocusLease &lease,
      uint64_t session, uint64_t generation, size_t index);
  std::optional<nlohmann::json>
  candidate_action(const FocusLease &lease, uint64_t session,
                   uint64_t generation, size_t index, CandidateAction action,
                   uint8_t position = 0);
  std::optional<nlohmann::json>
  page_candidate(const FocusLease &lease, uint64_t session,
                 uint64_t generation, bool previous, unsigned steps);
  bool confirm_ui(const FocusLease &lease, uint64_t generation);
  std::optional<PendingReply> configured_key(
      const FocusLease &lease, const FanyImeNamedpipeData &packet,
      TsfPreeditStyle style, const NavigationBindings &bindings,
      std::optional<std::string> local_text = std::nullopt,
      WordCharacterBinding word_binding = WordCharacterBinding::Disabled);
  std::optional<PendingReply> basic_key(const FocusLease &lease,
      const FanyImeNamedpipeData &packet, TsfPreeditStyle style,
      std::optional<std::string> local_text = std::nullopt);
  std::optional<PendingReply> toggle_character_set(
      const FocusLease &lease, const FanyImeNamedpipeData &packet,
      bool enabled, const std::function<bool(bool)> &persist);
  std::optional<PendingReply> edit(const FocusLease &lease,
      const FanyImeNamedpipeData &packet, TsfPreeditStyle style);
  std::optional<PendingReply> navigate(const FocusLease &lease,
                                       const FanyImeNamedpipeData &packet,
                                       const NavigationBindings &bindings);
  /// Re-rank with the settled model once the host reports typing has stopped.
  std::optional<nlohmann::json> rerank_settled(const FocusLease &lease);
  std::optional<nlohmann::json>
  apply_cloud_response(const FocusLease &lease, const std::string &query,
                       const std::string &body);
  std::optional<nlohmann::json> apply_ai_candidates(const FocusLease &lease,
                                                    const std::string &query,
                                                    const std::string &candidates);
  std::optional<std::string> translation_query(const FocusLease &lease);
  std::optional<nlohmann::json>
  apply_translations(const FocusLease &lease, uint64_t generation,
                     const std::string &translations);
  // Recover the staged result without rerunning Engine. This does NOT permit
  // blindly resending a frame whose previous delivery is uncertain.
  std::optional<PendingReply> pending(const FocusLease &lease);
  // Retry the latest snapshot after a pending reply is confirmed. Do not let
  // configuration mutate Engine/view state while an earlier result is unsent.
  std::optional<nlohmann::json> update_preferences(const FocusLease &lease,
                                                   const std::string &snapshot);
  bool cancel(const FocusLease &lease);
  // Deactivate at the DLL's request, which names a focus token rather than a
  // lease it has no way to construct. True also when this client is already
  // not focused on that token: the state the caller asked for holds either
  // way, and it is the state - not the act - that is being reported.
  bool cancel_focus_token(uint64_t token);
  // TIP 交给应用的一次按下（Aux 管道的 KeySound）：出按键音、计入打字特效的连击，和 Server 处理的键一样。只在这个客户端正以 `token` 持有焦点、而且不在英文模式时才出声；返回是否出了声。
  bool passthrough_key(uint64_t token, uint32_t key_class);
  // Explicit host composition termination, preserving the active focus lease.
  bool cancel_composition(const FocusLease &lease);
  HideCandidateDisposition hide_candidate(const FocusLease &lease);
  // Queue-owned maintenance operation; it is available without a focus lease.
  bool reset_cache();
  bool set_input_enabled(const FocusLease &lease, bool enabled);
  bool set_chinese_punctuation(const FocusLease &lease, bool enabled);
  // Leaves the composition and any pending reply alone: the nesting count is not part of either.
  bool balance_paired_punctuation(const FocusLease &lease, uint8_t opening);
  // Retain at most one latest snapshot while a reply is pending. True means
  // accepted for delivery, not necessarily applied to an active composition.
  bool queue_preferences(const FocusLease &lease, const std::string &snapshot);
  // Queue-owned settings broadcast, not an external focus authorization API.
  bool queue_current_preferences(const std::string &snapshot);
  nlohmann::json view() const { return session_.view(); }

private:
  void check_thread() const;
  bool prepared(const FocusLease &lease) const;
  void attach_online_query(const FocusLease &lease,
                           std::optional<PendingReply> &reply);
  // One commit's text and the mode that produced it, read off the pending
  // reply before a confirmation clears it. Only the string is copied: the
  // input queue runs this on every key, and copying the whole reply would deep
  // copy its candidate list for keys that commit nothing.
  struct Commit {
    std::string text;
    TypingSource source = TypingSource::Unknown;
    // False for the text the V, "/" and "@" modes generate; see transition_counts_as_typing.
    bool typing = true;
  };
  std::optional<Commit> pending_commit() const;
  // Records a commit that has been confirmed as delivered, and plays its commit sound.
  void record_commit(const std::optional<Commit> &delivered);
  // 一个出声的键：按键音（全屏应用在前台时不出）和打字特效。
  void sound_key(uint32_t key_class, bool auto_repeat);
  // 把会话解析好的特效设置和特效包的颜色、粒子数交给界面线程，在发布特效之前调用。
  void publish_typing_effect_settings();
  static std::string typing_statistics_directory(const std::string &options);
  FocusGate &gate_;
  uint64_t client_;
  const std::thread::id thread_ = std::this_thread::get_id();
  ServerSession session_;
  std::string statistics_directory_;
  TypingStatisticsSink statistics_;
  std::optional<FocusLease> lease_;
  std::optional<ReplyComposer> composer_;
  std::optional<nlohmann::json> preferences_retry_;
  ContinuationHide continuation_hide_;
};
} // namespace msime::windows

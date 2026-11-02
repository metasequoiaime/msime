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
// PredictionKept：会话停在上屏后联想、TIP 还没为这一代联想发过 Show，这个隐藏是上屏结束组字顺带发出的，联想留着，候选窗先藏起来等 Show。
enum class HideCandidateDisposition { Rejected, Cancelled, Suppressed, PredictionKept };
// 卷轴展开取到的完整候选列表（msime_client_all_candidates 的结果），以及取它时引擎放出扣着的尾部、代次前进后重新读到的 view（代次没变时为空）。
struct CandidateExpansionFetch {
  nlohmann::json snapshot;
  std::optional<nlohmann::json> view;
};

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
  // 换掉「全屏应用在前台」的判断（默认看 GetForegroundWindow）。测试用它让 FULLSCREEN 在两次调用之间变化，核对属性先于按键、先于激活报给核心。
  void set_fullscreen_check(std::function<bool()> check) { fullscreen_check_ = std::move(check); }
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
  // expanded_text 有值时是卷轴展开网格里点的候选：index 是完整候选列表里的下标，expanded_text 是网格上画的文字，见 ReplyComposer::select_candidate。
  std::optional<PendingReply> select_candidate(const FocusLease &lease,
      uint64_t session, uint64_t generation, size_t index,
      uint64_t translation_page,
      const std::optional<std::string> &expanded_text = std::nullopt);
  // 卷轴展开：为眼下这一页取完整候选列表。有等待送达确认的回复、释义页开着、没有候选或候选只能用键盘选时不取，返回空。
  std::optional<CandidateExpansionFetch> expanded_candidates(const FocusLease &lease);
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
  std::optional<PendingReply> toggle_japanese(
      const FocusLease &lease, const FanyImeNamedpipeData &packet,
      bool enabled, const std::function<bool()> &persist);
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
  std::optional<nlohmann::json> resolve_ai_candidates(const FocusLease &lease,
                                                      const std::string &query);
  std::optional<std::string> translation_query(const FocusLease &lease);
  // Rebuild the AI descriptor on the input queue immediately before dispatch;
  // settings may have changed after the key reply was composed.
  std::optional<std::string> ai_request(const FocusLease &lease,
                                        const std::string &query);
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
  // TIP 交给应用的一次按下（Aux 管道的 KeySound）：带 MSIME_KEY_PASSTHROUGH 报给生命周期核心，出按键音、计入打字特效的连击，和 Server 处理的键一样，但不开新的一轮。只在这个客户端正以 `token` 持有焦点、而且不在英文模式时才报；返回是否报了。
  bool passthrough_key(uint64_t token, uint32_t key_class);
  // TIP 报来的字段属性（Main 事件 FieldTraits，FanyImeFieldTraits 的位）。属于这个客户端而不是哪一次焦点，所以不要租约：立刻连同全屏前台一起交给核心，之后的激活和每个键都按它算。旧 TIP 从不报，属性保持未知。
  void set_field_traits(uint32_t traits);
  // Explicit host composition termination, preserving the active focus lease.
  bool cancel_composition(const FocusLease &lease);
  HideCandidateDisposition hide_candidate(const FocusLease &lease);
  // TIP 的 ShowCandidateWnd：会话正停在上屏后联想时，记下 TIP 已经把这一代联想显示在光标处。此后的隐藏是关闭联想（Esc、光标移走、别的键），要清掉会话里的联想；在这之前的隐藏只是上屏顺带的，联想留着。不是联想时什么也不做。
  void show_candidate(const FocusLease &lease);
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
  // 记下一次确认送达的上屏，只管统计：上屏音和上屏特效在派发时已经由生命周期核心按回复的 feedback 出过了。
  void record_commit(const std::optional<Commit> &delivered);
  // 每个键之前比较一次字段属性（TIP 报的加上此刻的全屏前台），变了先报给核心。
  void refresh_field_traits();
  // 把一个键或一次上屏的打字特效交给候选窗，先发布会话的特效设置。
  void publish_typing_effect(uint32_t packed);
  // 回复里带着核心算好的上屏特效时（commit_feedback）画出来；Server 合成的上屏没有。
  void publish_commit_feedback(const std::optional<PendingReply> &reply);
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
  // TIP 最近一次为之发过 Show 的联想代次（view 的 generation），0 表示还没有。见 show_candidate。
  uint64_t shown_prediction_generation_ = 0;
  // TIP 最近一次报的字段属性；旧 TIP 没协商到 FieldTraits，一直为空。
  std::optional<uint32_t> tip_field_traits_;
  // 空时用 foreground_is_fullscreen(GetForegroundWindow())。
  std::function<bool()> fullscreen_check_;
};
} // namespace msime::windows

#pragma once
#include "CandidateAction.h"
#include "CandidateClickWorker.h"
#include "ModeMailbox.h"
#include "CandidateMailbox.h"
#include "CloudCandidateWorker.h"
#include "AiCandidateWorker.h"
#include "TranslationWorker.h"
#include "PreferenceMonitor.h"
#include "RegistrationInbox.h"
#include "SessionWorkers.h"
#include "UiLessCandidateSync.h"
#include "HostCandidateRelay.h"
#include <atomic>
#include <chrono>
#include <optional>
#include <string_view>

namespace msime::windows {
enum class ModeRequestResult { Rejected, Sent, WriteFailed };
enum class SelectionRequestResult { Rejected, Busy, Sent, Failed };
enum class CandidateActionRequestResult { Rejected, Busy, Sent, Failed };
enum class CandidatePageRequestResult { Rejected, Busy, Sent, Failed };
enum class VoiceCompositionResult { Rejected, Busy, Sent, Failed };
enum class ControllerFailure {
  None,
  Service,
  InputQueue,
  SessionWorkers,
  Control,
  Preferences
};
// Owns the queue, connection workers and their external supervision thread.
// Service and mailbox are constructed first and must outlive this controller.
// stop_service runs once on the control thread and must close ALL registered
// endpoints (including unconsumed inbox tickets) and join service workers.
class SessionController final {
public:
  SessionController(
      MainTransport &transport, RegistrationInbox &inbox, size_t clients,
      size_t input_capacity, std::string options, SessionPump::KeyHandler key,
      SessionPump::EventHandler event, std::function<bool()> healthy,
      std::function<void()> stop_service,
      std::chrono::milliseconds interval = std::chrono::milliseconds(100),
      std::string preferences_directory = {},
      SessionPump::Presentation presentation = {},
      PreferenceMonitor::Published published = {});
  ~SessionController();
  SessionController(const SessionController &) = delete;
  SessionController &operator=(const SessionController &) = delete;
  void request_stop(); // Callback-safe: only signals/closes the mailbox.
  void
  stop(); // External thread, waits for ordered service/worker/queue shutdown.
  ControllerFailure failure() const { return failure_.load(); }
  // External thread only; finite transport write may block. Sent means bytes
  // delivered, not that TSF applied the mode. Never call from input/event callbacks.
  ModeRequestResult request_mode(const FocusLease &lease, WorkerMode mode);
  bool send_caps_lock(const FocusLease &lease, bool enabled);
  // External maintenance callback. The Engine operation remains queue-owned.
  bool reset_cache();
  // 外部偏好写入完成后唤醒监视器，让当前会话尽快收到新快照。
  void wake_preferences();
  // Push the TSF-local settings to every registered TIP. Returns true only
  // when every snapshot ticket accepted the complete frame set.
  bool send_tsf_config(const TsfLocalConfig &config);
  // External/UI thread, value copy only. Empty means hide. Re-read on paint;
  // selection still requires an independently validated candidate command.
  std::optional<CandidatePresentation> candidate_view();
  // Non-blocking, any thread: is `lease` still the focus on file. candidate_view() returns nothing for contention as well, so the candidate window asks this to tell a busy read from a focus that moved on (another client, another app) before it lets an exit animation keep drawing the old candidates.
  bool candidate_lease_current(const FocusLease &lease) const {
    return !stopping_ && focus_.still_current(lease);
  }
  void wait_candidate_render_for_key(const FocusLease &lease,
                                     const FanyImeNamedpipeData &packet);
  // Called after the native candidate window has successfully presented a
  // frame. The receipt is generation- and lease-bound.
  void candidate_rendered(const FocusLease &lease, uint64_t generation) {
    candidates_.rendered(lease, generation);
  }
  // 候选界面的绘制请求（#9519）：按键和点选的渲染屏障要等回执时调用它，叫醒候选窗立即读快照绘制，不等它下一次轮询。任何线程都可能调用；回调只能发信号（如 SetEvent），不得回调本控制器或候选信箱。候选窗销毁前传空函数撤掉，返回后旧回调不会再被调用。
  void set_candidate_render_request(std::function<void()> request) {
    candidates_.set_render_request(std::move(request));
  }
  std::optional<ModePresentation> mode_view();
  // Definitive: is a focused client's mode on file at all. mode_view() returns
  // nothing for contention as well, so a UI needs this to tell busy from gone.
  bool mode_active();
  // External worker only: reads Engine state on its owning input queue.
  // Empty means stale, busy or unavailable; never a guessed mode.
  std::optional<bool> dedicated_english_state(const FocusLease &lease);
  // Exit the Engine's dedicated-English mode for this exact focus lease.
  bool exit_dedicated_english(const FocusLease &lease);
  // 托盘「英文候选模式」：把这个焦点租约的 Engine 英文模式设成 `enabled`。和退出一样只在外部工作线程上调用，最多等 2 秒；成功才返回 true。
  bool set_dedicated_english(const FocusLease &lease, bool enabled);
  // Control-thread lease validation, not a best-effort UI snapshot. Waits for
  // an existing focus transaction instead of treating a busy gate as loss.
  bool focus_current(const FocusLease &lease);
  // External I/O thread only, never a window/input callback. Busy is a dropped
  // request, not queued/replayed. Sent means delivery confirmed, not TSF applied.
  // translation_page 是点击所在那一帧的 CandidatePresentation::translation_page，和 session、generation 一起认定点的是哪一张表。
  // expanded_text 有值时是卷轴展开网格里的候选（CandidateClick::expanded_text）：index 是完整候选列表里的下标，画出来的那一帧挂着的完整列表里必须有这一项、文字相同。
  SelectionRequestResult request_selection(const FocusLease &lease,
      uint64_t session, uint64_t generation, size_t index,
      uint64_t translation_page,
      const std::optional<std::string> &expanded_text = std::nullopt);
  // 卷轴展开：候选窗点开「∨」（open 为 true）或收起、隐藏（false）。界面线程调用，不等待：打开时排一个输入队列任务去取完整候选列表，取到后随候选快照交给候选窗。之后每发布一页新的代次都会再取一次，直到收起。
  void set_candidate_expansion(const FocusLease &lease, bool open);
  // 释义页上的行（translation_page 不为 0）没有候选动作，一律拒绝。
  CandidateActionRequestResult request_candidate_action(
      const FocusLease &lease, uint64_t session, uint64_t generation,
      size_t index, uint64_t translation_page, CandidateAction action,
      uint8_t position = 0);
  CandidatePageRequestResult request_page(const CandidatePage &page);
  // Send a bounded voice snapshot through the authenticated worker endpoint.
  // The caller owns recording/ASR; this method only validates the focus lease
  // and performs the ordered frame delivery.
  VoiceCompositionResult send_voice_composition(
      const FocusLease &lease, uint32_t message, std::wstring_view text,
      wchar_t generation);
  // The Aux pipe's TerminalDeactivation fallback. Called on the Aux listener
  // thread, never from an input or window callback. Returns true only once the
  // named client really is not focused under that token, because the DLL
  // writes its "OK" on the strength of this answer.
  bool deactivate_terminal(uint64_t client, uint64_t token);
  // Aux 管道的 KeySound：把 TIP 交给应用的一次按下排进输入队列，不等它执行。按键音晚了就没有意义，队列满或正在停止时直接丢掉，这不是输入，不需要让焦点失效。
  void passthrough_key(uint64_t client, uint64_t token, uint32_t key_class);
  // Release every Engine session so another process can take the exclusive
  // dictionary lock, and rebuild them afterwards. Called on the Aux listener
  // thread. A quiesce that is never resumed would leave the IME dead, so it
  // carries its own deadline and the control loop resumes without being
  // asked once it passes.
  bool quiesce_dictionaries();
  bool resume_dictionaries();
  std::optional<PreferenceMonitorStatus> preferences_status() const {
    return preferences_
               ? std::optional<PreferenceMonitorStatus>(preferences_->status())
               : std::nullopt;
  }

private:
  void run();
  // 展开开着、而手里的完整候选列表不是眼下这一代时，排一个任务去取（CandidateMailbox::expansion_due）。同一时刻最多排一个；取的时候引擎代次前进了，就先发布新的 view，再交出列表。只在发布了新一页之后调用，不阻塞。
  void refresh_expansion(const FocusLease &lease);
  std::atomic<bool> expansion_queued_{false};
  /// The composition that has been standing still, and since when.
  ///
  /// Windows has no run loop here to hang a timer on, so the settle pass rides
  /// the existing service tick: the lease is recorded when a reply is
  /// delivered, and the pass runs on the first tick after the delay has
  /// elapsed. Cleared once it has run, so a composition is ranked once rather
  /// than on every tick that follows.
  std::optional<FocusLease> settled_pending_;
  std::chrono::steady_clock::time_point settled_since_{};
  void run_settled_rerank();
  void push_host_candidates(const FocusLease &, unsigned, const std::string &, const nlohmann::json &);
  HostCandidateRelay host_candidates_;
  uint16_t host_candidates_transaction_ = 0;
  void push_uiless_candidates(const FocusLease &lease,
                              const nlohmann::json &view);
  RegistrationInbox &inbox_;
  MainTransport &transport_;
  std::function<bool()> healthy_;
  std::function<void()> stop_service_;
  std::chrono::milliseconds interval_;
  FocusGate focus_;
  CandidateMailbox candidates_;
  UiLessCandidateSync uiless_candidates_;
  ModeMailbox modes_;
  std::shared_ptr<std::mutex> transactions_ = std::make_shared<std::mutex>();
  SessionPump::Presentation presentation_;
  SessionPump::EventHandler event_;
  InputQueue input_;
  CloudCandidateWorker cloud_;
  AiCandidateWorker ai_;
  TranslationWorker translations_;
  SessionWorkers workers_;
  std::unique_ptr<PreferenceMonitor> preferences_;
  std::atomic<bool> stopping_{false};
  // Monotonic deadline for an outstanding quiesce; zero when not quiesced.
  std::atomic<std::chrono::steady_clock::rep> quiesce_deadline_{0};
  std::atomic<ControllerFailure> failure_{ControllerFailure::None};
  std::mutex stop_mutex_;
  std::thread control_;
};
} // namespace msime::windows

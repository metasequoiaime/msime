#pragma once
#include "FocusRouter.h"
#include "FocusedSession.h"
#include "PreferenceSnapshot.h"
#include <chrono>
#include <condition_variable>
#include <deque>
#include <functional>
#include <future>
#include <memory>

namespace msime::windows {
// Owned exclusively by InputQueue's worker, including construction/destruction.
// References to this object or its sessions must never escape a task.
class InputState final {
public:
  InputState(FocusGate &gate, size_t clients, std::string options);
  ~InputState();
  InputState(const InputState &) = delete;
  InputState &operator=(const InputState &) = delete;
  FocusRoute connected(const PipeTicket &ticket);
  FocusRoute disconnected(const PipeTicket &ticket);
  // Applies old-session cleanup and new-session preparation before returning.
  // The caller then schedules a fence on an I/O worker, never in this task.
  FocusRoute dispatch(const PipeTicket &ticket,
                      const FanyImeNamedpipeData &packet);
  bool confirmed(const FocusLease &lease);
  // Reset every live Engine session, including clients without focus.
  bool reset_cache();
  // The DLL's Aux-pipe fallback: deactivate a client it names by id and focus
  // token, having failed to write the deactivate on the Main pipe. True means
  // that client is not focused under that token - which is what the DLL is
  // waiting to hear, and is already so when it never was.
  bool deactivate_terminal(uint64_t client, uint64_t token);
  // TIP 经 Aux 管道报来的一次交给应用的按下：交给那个客户端的会话出按键音、计入连击。客户端不在或没有以 `token` 持有焦点时什么都不做。
  bool passthrough_key(uint64_t client, uint64_t token, uint32_t key_class);
  // Dictionary maintenance runs in another process and needs the exclusive
  // file lock that every Engine session holds a share of. Dropping the
  // sessions is what releases it: the Engine has the dictionary files open,
  // so leaving it alive while they are swapped underneath would have it
  // reading a tree that no longer exists. Transport registrations are kept,
  // so the clients stay connected and get a session back on resume.
  size_t quiesce_dictionaries();
  size_t resume_dictionaries();
  bool quiesced() const {
    check_thread();
    return quiesced_;
  }
  FocusRoute failed(const FocusLease &lease);
  std::optional<PendingReply>
  key(const FocusLease &lease, const FanyImeNamedpipeData &packet,
      ReplyPath path, bool uiless = false,
      std::optional<std::string> local_text = std::nullopt);
  bool delivered(const FocusLease &lease, uint64_t request);
  bool cancel_composition(const FocusLease &lease);
  HideCandidateDisposition hide_candidate(const FocusLease &lease);
  std::optional<nlohmann::json> dedicated_english(const FocusLease &lease,
                                                  bool exit);
  std::optional<nlohmann::json> set_dedicated_english(const FocusLease &lease,
                                                      bool enabled);
  std::optional<nlohmann::json>
  apply_ai_candidates(const FocusLease &lease, const std::string &query,
                      const std::string &candidates);
  /// Re-rank with the settled model; nothing when the order held or no model
  /// is installed, which is the answer on every one-model installation.
  std::optional<nlohmann::json> rerank_settled(const FocusLease &lease);
  std::optional<nlohmann::json> apply_cloud_response(const FocusLease &lease,
                                                     const std::string &query,
                                                     const std::string &body);
  std::optional<nlohmann::json> apply_ai_response(const FocusLease &lease,
                                                  const std::string &query,
                                                  const std::string &body);
  std::optional<std::string> translation_query(const FocusLease &lease);
  std::optional<std::pair<FocusLease, std::string>>
  current_translation_request();
  std::optional<nlohmann::json>
  apply_translations(const FocusLease &lease, uint64_t generation,
                     const std::string &translations);
  std::optional<PendingReply> select_candidate(const FocusLease &lease,
                                               uint64_t session,
                                               uint64_t generation,
                                               size_t index);
  std::optional<nlohmann::json>
  candidate_action(const FocusLease &lease, uint64_t session,
                   uint64_t generation, size_t index, CandidateAction action,
                   uint8_t position = 0);
  std::optional<nlohmann::json> page_candidate(const FocusLease &lease,
                                               uint64_t session,
                                               uint64_t generation,
                                               bool previous, unsigned steps,
                                               bool from_wheel);
  bool ui_delivered(const FocusLease &lease, uint64_t generation);
  std::optional<PendingReply> configured_key(
      const FocusLease &lease, const FanyImeNamedpipeData &packet,
      TsfPreeditStyle style, const NavigationBindings &bindings,
      std::optional<std::string> local_text = std::nullopt,
      WordCharacterBinding word_binding = WordCharacterBinding::Disabled);
  std::optional<PendingReply>
  toggle_character_set(const FocusLease &lease,
                       const FanyImeNamedpipeData &packet, bool enabled,
                       const std::function<bool(bool)> &persist = {});
  std::optional<PendingReply>
  basic_key(const FocusLease &lease, const FanyImeNamedpipeData &packet,
            TsfPreeditStyle style,
            std::optional<std::string> local_text = std::nullopt);
  std::optional<PendingReply> edit(const FocusLease &lease,
                                   const FanyImeNamedpipeData &packet,
                                   TsfPreeditStyle style);
  bool synchronize_input_mode(const FocusLease &lease,
                              const FanyImeNamedpipeData &packet);
  bool balance_paired_punctuation(const FocusLease &lease,
                                  const FanyImeNamedpipeData &packet);
  bool queue_preferences(const FocusLease &lease, const std::string &snapshot);
  // Retain latest validated global settings for current and future focus.
  // Snapshot loading happens outside the input queue. Older/conflicting values
  // are rejected; identical publications are safe retries.
  void publish_preferences(const PreferenceSnapshot &snapshot);
  NavigationBindings navigation_bindings() const {
    check_thread();
    return navigation_;
  }
  WordCharacterBinding word_character_binding() const {
    check_thread();
    return word_character_;
  }
  TsfPreeditStyle tsf_preedit_style() const {
    check_thread();
    return tsf_preedit_style_;
  }
  bool character_set_shortcut_enabled() const {
    check_thread();
    return character_set_shortcut_enabled_;
  }
  std::optional<PendingReply> navigate(const FocusLease &lease,
                                       const FanyImeNamedpipeData &packet,
                                       const NavigationBindings &bindings);
  std::optional<nlohmann::json> update_preferences(const FocusLease &lease,
                                                   const std::string &snapshot);

private:
  friend class InputQueue;
  void shutdown() noexcept;
  struct Client {
    PipeTicket ticket;
    std::unique_ptr<FocusedSession> session;
  };
  void check_thread() const;
  void cleanup(const FocusRoute &route);
  FocusedSession *session(const PipeTicket &ticket);
  const std::thread::id thread_ = std::this_thread::get_id();
  FocusGate &gate_;
  FocusRouter router_;
  std::string options_;
  NavigationBindings navigation_;
  WordCharacterBinding word_character_ = WordCharacterBinding::Disabled;
  TsfPreeditStyle tsf_preedit_style_ = TsfPreeditStyle::Local;
  bool character_set_shortcut_enabled_ = true;
  std::optional<PreferenceSnapshot> preferences_;
  bool quiesced_ = false;
  std::unordered_map<uint64_t, Client> clients_;
};

enum class InputTaskStatus { Completed, Failed, Cancelled };
struct InputQueueStats {
  size_t queued = 0;
  bool active = false;
  bool accepting = false;
  uint64_t completed = 0;
  uint64_t failed = 0;
  uint64_t cancelled = 0;
};
class InputQueue final {
public:
  using Task = std::function<void(InputState &)>;
  // Capacity bounds queued task count, not captured payload bytes. Controller
  // tasks must contain bounded copies, never borrowed pipe buffers or sessions.
  // Gate must outlive stop(). Tasks must not block on I/O or queue futures.
  InputQueue(FocusGate &gate, size_t clients, size_t capacity,
             std::string options);
  ~InputQueue();
  InputQueue(const InputQueue &) = delete;
  InputQueue &operator=(const InputQueue &) = delete;
  // Nonblocking admission. Full/stopped/empty task returns null. Every accepted
  // task settles its future, including cancellation. No silent input dropping:
  // caller must invalidate affected transport/focus on failed admission.
  std::optional<std::future<InputTaskStatus>> submit(Task task);
  void request_stop(); // May be called from a task; does not join.
  void stop();         // External control thread only; idempotent, joins.
  InputQueueStats stats() const;
  bool on_worker_thread() const noexcept;
  // Valid only from a task running on this queue's worker. The value measures
  // time spent waiting after admission, before the task began executing.
  std::chrono::milliseconds current_task_wait() const noexcept;

private:
  struct Job {
    Task task;
    std::promise<InputTaskStatus> completion;
    std::chrono::steady_clock::time_point enqueued_at;
  };
  void run(FocusGate &gate, size_t clients, std::string options);
  size_t capacity_;
  mutable std::mutex mutex_;
  std::mutex stop_mutex_;
  std::condition_variable ready_;
  bool started_ = false;
  bool startup_failed_ = false;
  bool stopping_ = false;
  std::deque<Job> jobs_;
  InputQueueStats stats_;
  std::chrono::milliseconds active_task_wait_{0};
  std::thread worker_;
};
} // namespace msime::windows

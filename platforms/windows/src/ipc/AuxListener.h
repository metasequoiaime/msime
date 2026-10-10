#pragma once
#include "AuxMessage.h"
#include <atomic>
#include <functional>
#include <memory>
#include <mutex>
#include <string>
#include <thread>
#include <windows.h>

namespace msime::windows {
class PipeListener;

// Counters only. The Aux message may describe where the user clicked, so the
// listener never records its contents (AGENTS.md forbids input in logs).
struct AuxStats {
  uint64_t accepted = 0;
  uint64_t malformed = 0;
  uint64_t unknown_verb = 0;
  uint64_t dispatched = 0;
  // A well-formed verb from a peer not allowed to send it. Kept apart from
  // unknown_verb so a forged request is not mistaken for a protocol mismatch.
  uint64_t rejected = 0;
  uint64_t callback_failures = 0;
};

// A fourth, session-less pipe endpoint. The TSF DLL writes one message and
// closes; there is no handshake, no registry entry and no client id, so this
// listener deliberately holds no reference to the input path: its outputs are
// a tray anchor and optional validated host-action messages.
class AuxListener final {
public:
  using Sink = std::function<void(const TrayMenuAnchor &)>;
  using MessageSink = std::function<void(const std::wstring &)>;
  // The IME activation edges reported by the TSF DLL.
  using ActivationSink = std::function<void(AuxActivation)>;
  // Return true once the client really has been deactivated; only then is the
  // "OK" the DLL is waiting on written back.
  using TerminalSink = std::function<bool(const AuxTerminalDeactivation &)>;
  // Return true once the sessions really have been released, or rebuilt. Only
  // then is the "OK" the settings process is waiting on written back: it takes
  // that as permission to open the dictionaries exclusively.
  using MaintenanceSink = std::function<bool(AuxDictionaryMaintenance)>;
  // Keys the TIP let through to the application, batched. Return true once the batch has been handed to the store; the "OK" that follows tells the DLL to keep sending, and its absence makes it back off.
  using StatisticsSink = std::function<bool(const AuxTypingStatistics &)>;
  // Per-key press counts for the key heatmap, or a probe with no counts. Return true only while statistics are on and the counts, if any, have been handed to the store; the DLL buffers nothing until a probe is answered "OK" and stops again at the first unanswered batch.
  using KeysSink = std::function<bool(const AuxTypingKeys &)>;
  // TIP 交给应用的一次按下（KeySound），发送方的进程号已经核对过。返回 true 表示按键音或打字特效开着，之后写回的 "OK" 让 TIP 继续发；没有 "OK" 时 TIP 停一阵。只能入队、不能等输入线程。
  using KeySoundSink = std::function<bool(const AuxKeySound &)>;
  static std::unique_ptr<AuxListener> create(const std::wstring &name,
                                             Sink sink, DWORD &error,
                                             MessageSink message_sink = {},
                                             ActivationSink activation = {},
                                             TerminalSink terminal = {},
                                             MaintenanceSink maintenance = {},
                                             StatisticsSink statistics = {},
                                             KeysSink keys = {},
                                             KeySoundSink key_sound = {});
  ~AuxListener();
  AuxListener(const AuxListener &) = delete;
  AuxListener &operator=(const AuxListener &) = delete;
  // Safe from inside the sink: signals only, never joins.
  void request_stop();
  void stop();
  DWORD failure() const { return failure_.load(); }
  AuxStats stats() const;

private:
  AuxListener() = default;
  void run();
  void write_ok(HANDLE connection);
  void callback_failed() noexcept;
  std::unique_ptr<PipeListener> listener_;
  Sink sink_;
  MessageSink message_sink_;
  ActivationSink activation_;
  TerminalSink terminal_;
  MaintenanceSink maintenance_;
  StatisticsSink statistics_;
  KeysSink keys_;
  KeySoundSink key_sound_;
  HANDLE cancel_ = nullptr;
  std::thread worker_;
  std::mutex stop_mutex_;
  mutable std::mutex stats_mutex_;
  AuxStats stats_;
  std::atomic<DWORD> failure_{ERROR_SUCCESS};
};
} // namespace msime::windows

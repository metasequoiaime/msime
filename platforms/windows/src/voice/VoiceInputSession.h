#pragma once
#include "VoiceReviewResult.h"
#include "VoiceCaptureSelection.h"
#include "VoiceSessionEpoch.h"

#include "FocusGate.h"
#include "SessionController.h"
#include "WaveOverlay.h"
#include "DoubaoAsrClient.h"
#include "CuePlayer.h"
#include <atomic>
#include <chrono>
#include <functional>
#include <future>
#include <memory>
#include <mutex>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

namespace msime::windows {
class AudioCapture;
class OnDeviceAsrStream;

struct VoiceInputConfig {
  VoiceCaptureSelection capture;
  bool enabled = true;
  bool start_sound = true;
  bool end_sound = true;
  bool sound_enabled = true;
  bool mute_system_audio = false;
  bool hotkey_ralt = true;
  bool hotkey_ctrl_f9 = true;
  bool hotkey_ctrl_win = false;
  bool hotkey_rctrl_ralt = false;
  bool hotkey_hold_space_lock = true;
  bool stream_inline_preedit = true;
  std::string commit_mode = "tsf";
  // 原生语音上屏前是否转成繁体：traditional_chinese_output 打开，而且正在运行的方案做简繁转换（scheme::ScriptConversionApplies，日文等方案原样上屏），与候选和上屏的 traditional_projection 同一规则。
  bool traditional_output = false;
  std::string asr_provider = "doubao";
  // Provider "local" only: the installed catalog model directory (voice_input.asr_model_path).
  std::string asr_model_path;
  // The Server's HostOptions JSON, for reading the user's dictionary words as local-recognition hotwords. Shared so that copying the config for each dictation does not copy the document.
  std::shared_ptr<const std::string> host_options;
  std::string endpoint;
  std::string model;
  std::string token;
  std::string app_key;
  std::string doubao_auth_mode;
  std::string resource_id;
  bool enable_itn = true;
  bool enable_punc = true;
  bool enable_ddc = false;
  std::string boosting_table_id;
  std::string language = "zh-cn";
  bool polish_enabled = false;
  bool polish_text = false;
  std::string polish_provider;
  std::string polish_token;
  std::string polish_endpoint;
  std::string polish_model;
  std::string polish_prompt_id = "cleanup";
  std::string polish_prompt_custom_1;
  std::string polish_prompt_custom_2;
  std::string polish_prompt_custom_3;
};

// Owns microphone capture and the asynchronous batch recognizer. Native UI
// uses WaveOverlay; review captures expose a bounded VoiceReviewResult instead.
// No capture callback calls stop or touches the window procedure directly.
class VoiceInputSession final {
public:
  using LeaseProvider = std::function<std::optional<FocusLease>()>;
  using Sender = std::function<VoiceCompositionResult(
      const FocusLease &, uint32_t, std::wstring_view, wchar_t)>;
  using ConfigProvider = std::function<VoiceInputConfig()>;
  // 控制线程调用：租约是否仍是当前获得焦点的 TSF 客户端。maintain() 用它在焦点离开时取消原生语音。
  using FocusValidator = std::function<bool(const FocusLease &)>;
  // 原生语音成功上屏后调用一次，参数是上屏的 UTF-8 文本，由 Server 记入打字统计（来源 voice）。在识别工作线程上调用，不得阻塞。
  using CommitRecorder = std::function<void(const std::string &)>;

  VoiceInputSession(WaveOverlay &overlay, LeaseProvider lease_provider,
                    Sender sender, ConfigProvider config_provider,
                    FocusValidator focus_validator,
                    CommitRecorder commit_recorder);
  ~VoiceInputSession();
  VoiceInputSession(const VoiceInputSession &) = delete;
  VoiceInputSession &operator=(const VoiceInputSession &) = delete;

  bool toggle();
  // Control-thread only, like stop/cancel. Null means busy/unavailable. The
  // dispatcher must authenticate the controller and retain its focus lease;
  // this result object grants no authority to stop a different session.
  std::shared_ptr<VoiceReviewResult> start_review(std::string_view language);
  bool stop_review(const std::shared_ptr<VoiceReviewResult> &expected);
  bool cancel_review(const std::shared_ptr<VoiceReviewResult> &expected);
  void stop();
  void cancel();
  // 控制线程调用。识别或润色进行中点浮层的 ✓：只收起浮层，结果照常在后台上屏，对应 macOS 的 dismissProcessing。之后的状态更新不再弹出浮层，失败提示照常显示。
  void dismiss_processing();
  void lock();
  // Control-thread only; the Server loop calls it on every pass. Ends a recording whose capture stopped delivering (with a message) or whose batch buffer is full (submitting what it holds). The capture callback cannot do either itself.
  void maintain();
  bool init_cues(const std::wstring &start_path, const std::wstring &end_path);
  bool recording() const { return recording_.load(); }
  bool locked() const { return locked_.load(); }

private:
  bool start(std::shared_ptr<VoiceReviewResult> review = {},
             std::string_view language = {});
  void finish(std::vector<float> samples, FocusLease lease,
              VoiceInputConfig config, uint64_t session,
              std::shared_ptr<DoubaoAsrClient> doubao,
              std::shared_ptr<OnDeviceAsrStream> local_stream,
              std::shared_ptr<std::atomic_bool> cancelled,
              std::shared_ptr<VoiceReviewResult> review, HWND start_window);
  void clear_overlay();
  void cancel_session(bool failed);
  // Stamps the time a local model was last used, for release_idle_local_model(). Any thread.
  void note_local_model_use();
  // On-device recognition of a finished batch recording, reached only when asr_model_path is not an installed model directory (those stream through LocalAsrStream); shared/voice refuses such a path and the person is told the model is unusable (voice_local_failure). Runs on the recognition worker.
  std::string recognize_local(const std::vector<float> &samples,
                              const VoiceInputConfig &config,
                              const std::shared_ptr<std::atomic_bool> &cancelled);
  // Control-thread only, from maintain(). Hands a model left idle for long enough to a worker to unload; see local_model_used_.
  void release_idle_local_model();
  // 控制线程在录音中调用：当前这次录音的本机识别器（系统识别或本地模型）已经出错停下时，返回给人看的那句话。
  std::optional<std::string> local_stream_failure();
  // Shows `message` on the overlay for a few seconds without blocking the caller. Callable from any thread; `session` is the epoch the message belongs to, and a later session takes the overlay over.
  void report_failure(std::string_view message, uint64_t session);

  WaveOverlay &overlay_;
  LeaseProvider lease_provider_;
  Sender sender_;
  ConfigProvider config_provider_;
  FocusValidator focus_validator_;
  CommitRecorder commit_recorder_;
  AudioCapture *capture_ = nullptr;
  std::unique_ptr<AudioCapture> capture_owner_;
  CuePlayer cue_player_;
  std::atomic<bool> recording_{false};
  std::atomic<bool> starting_{false};
  std::atomic<bool> locked_{false};
  std::atomic<bool> cancel_requested_{false};
  VoiceSessionEpoch session_;
  // 原生录音停止后、finish() 还在识别或润色时等于该次会话的代次，finish() 结束时清零。maintain() 据此在识别期间也检查焦点，dismiss_processing() 据此判断是否有结果在路上。
  std::atomic<uint64_t> finishing_session_{0};
  // dismiss_processing() 收起浮层的那次会话代次；finish() 不再为它重新显示浮层。
  std::atomic<uint64_t> dismissed_session_{0};
  std::mutex samples_mutex_;
  std::vector<float> samples_;
  std::size_t captured_frames_ = 0;
  std::atomic<bool> capture_full_{false};
  std::optional<FocusLease> lease_;
  // 开始录音时的前台窗口，控制线程写入并随识别任务带进 finish()；SendInput 和 Ctrl+V 上屏时前台仍是它也算目标在前台。
  HWND start_window_ = nullptr;
  std::shared_ptr<VoiceReviewResult> review_; // control-thread owned
  std::mutex config_mutex_;
  std::optional<VoiceInputConfig> active_config_;
  std::mutex doubao_mutex_;
  std::shared_ptr<DoubaoAsrClient> doubao_;
  // 与 doubao_ 对应的本机识别器：采集回调往里推音频，识别任务从录音开始就运行。本地模型和 Windows 系统识别都放在这里。
  std::mutex local_stream_mutex_;
  std::shared_ptr<OnDeviceAsrStream> local_stream_;
  // Set by the capture callback when the streaming local recognizer cannot
  // retain another audio chunk. The control thread turns it into a visible
  // failed recording; audio is never silently discarded.
  std::atomic<bool> local_stream_overflow_{false};
  std::atomic<bool> muted_system_audio_{false};
  // 开始提示音退回系统声音时推迟静音的到点时刻（steady_clock 计数），0 表示没有推迟中的静音。控制线程读写，maintain() 到点后静音并清零，stop() 和 cancel_session() 清零。
  std::atomic<std::chrono::steady_clock::rep> pending_mute_at_{0};
  std::mutex request_mutex_;
  std::vector<std::shared_ptr<std::atomic_bool>> request_cancellations_;
  std::mutex tasks_mutex_;
  std::vector<std::future<void>> tasks_;
  std::atomic<uint64_t> failure_displays_{0};
  std::mutex notices_mutex_;
  std::vector<std::future<void>> notices_;
  // A loaded local model holds hundreds of megabytes to over a gigabyte. The recognizer keeps it for the next dictation, and maintain() unloads it after a while without one. The unload takes the recognizer's cache lock, which a model load holds for seconds, so it runs on idle_release_ rather than the control thread. local_model_used_ is a steady_clock tick count written before local_model_loaded_.
  std::atomic<bool> local_model_loaded_{false};
  std::atomic<std::chrono::steady_clock::rep> local_model_used_{0};
  std::future<void> idle_release_; // control-thread owned
};
} // namespace msime::windows

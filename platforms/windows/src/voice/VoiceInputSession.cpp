#include "VoiceInputSession.h"
#include "AudioCapture.h"
#include "ChineseTextConversion.h"
#include "PolishPrompt.h"
#include "VoiceHotwordTexts.h"
#include "VoiceRequestCancellation.h"

#include "LocalAsr.h"
#include "LocalAsrAudioQueue.h"
#include "OnDeviceAsrStream.h"
#include "SystemAsrPolicy.h"
#include "SystemAsrStream.h"
#include "ReplyCodec.h"
#include "SystemAudioMuter.h"
#include "../../tsf/IPC/PassthroughStatistics.h"
#include "../../../../shared/voice/VoiceProviders.h"
#include "VoiceCommitPolicy.h"
#include "VoiceSessionPolicy.h"
#include "msime_client.h"
#include "../../../common/HostApiString.h"

#include <algorithm>
#include <chrono>
#include <cmath>
#include <condition_variable>
#include <cstring>
#include <exception>
#include <nlohmann/json.hpp>
#include <type_traits>

namespace msime::windows {
namespace {
constexpr std::size_t kSampleRate = 16000;
// MSIME-Windows keeps its message boxes until they are dismissed. The overlay cannot take focus, so it holds a failure long enough to read a provider's sentence and then steps aside.
constexpr DWORD kFailureDisplayMs = 4000;
constexpr DWORD kFailurePollMs = 100;
// How long a loaded local model is kept without a dictation: the same two minutes the macOS and Linux recognizer process waits (shared/voice/LocalAsrHelper.cpp).
constexpr auto kLocalModelIdle = std::chrono::seconds(120);
// 开始提示音退回系统声音 MessageBeep 时，静音其他声音推迟这么久：MessageBeep 走系统声音会话，立刻静音会把它一起静掉。它不报告何时放完，所以按系统提示音的长度等一段固定时间，对应 macOS 等开始提示音放完才静音（VoiceAudioMuter deferredMute）。
constexpr auto kSystemBeepMuteDelay = std::chrono::milliseconds(1000);

bool is_local_asr_provider(std::string_view provider) {
  return normalize_voice_provider(provider) == "local";
}

// 「Windows 系统识别」：SAPI 进程内听写，对应 macOS 的「macOS 系统识别」。
bool is_system_asr_provider(std::string_view provider) {
  return normalize_voice_provider(provider) == "system";
}

// The value of a host-api response, or nothing when the call failed. Frees the returned string.
std::optional<nlohmann::json> host_value(char *raw) {
  const auto owned = msime::host_api::own_string(raw);
  if (!owned)
    return std::nullopt;
  auto document = nlohmann::json::parse(owned.get(), nullptr, false);
  if (!document.is_object() || !document.value("ok", false) ||
      !document.contains("value"))
    return std::nullopt;
  return std::move(document.at("value"));
}

// The user's own pinyin dictionary words, heaviest first, as [{text,pinyin}]. Recognition without them is still recognition, so a dictionary that cannot be read right now (maintenance holds it, the store is missing) gives none.
nlohmann::json local_hotwords(const VoiceInputConfig &config) {
  auto none = nlohmann::json::array();
  if (!config.host_options || config.host_options->empty())
    return none;
  // host_options is already a serialized JSON object, so it is spliced in rather than parsed and dumped again for every dictation.
  const auto request = "{\"options\":" + *config.host_options + "}";
  const auto value = host_value(msime_client_voice_hotwords(
      reinterpret_cast<const uint8_t *>(request.data()), request.size()));
  if (!value || !value->is_object())
    return none;
  const auto hotwords = value->find("hotwords");
  if (hotwords == value->end() || !hotwords->is_array())
    return none;
  return *hotwords;
}

// The text with near-miss spellings of the user's words replaced. Best-effort: any failure keeps what the recognizer produced.
std::string correct_with_hotwords(const std::string &text,
                                  const nlohmann::json &hotwords) {
  const auto request =
      nlohmann::json{{"text", text}, {"hotwords", hotwords}}.dump(
          -1, ' ', false, nlohmann::json::error_handler_t::replace);
  const auto value = host_value(msime_client_voice_hotword_correct(
      reinterpret_cast<const uint8_t *>(request.data()), request.size()));
  if (!value || !value->is_object())
    return text;
  const auto corrected = value->find("text");
  if (corrected == value->end() || !corrected->is_string() ||
      corrected->get_ref<const std::string &>().empty())
    return text;
  return corrected->get<std::string>();
}

// 本机识别器在录音中就出错时给人看的那句话，与 finish() 捕获同一失败时的文案相同。
std::string on_device_failure_message(const VoiceInputConfig &config,
                                      const std::exception_ptr &error) {
  if (is_system_asr_provider(config.asr_provider)) {
    try {
      std::rethrow_exception(error);
    } catch (const std::exception &failure) {
      return system_asr_failure(failure);
    } catch (...) {
      return system_asr_failure(std::runtime_error("unknown"));
    }
  }
  return std::string(voice_local_failure(
      local_asr_available(), msime::voice::is_local_model_dir(config.asr_model_path)));
}

std::wstring wide(std::string_view text) {
  if (text.empty())
    return {};
  const int length = MultiByteToWideChar(
      CP_UTF8, MB_ERR_INVALID_CHARS, text.data(), static_cast<int>(text.size()),
      nullptr, 0);
  if (length <= 0)
    return {};
  std::wstring result(static_cast<size_t>(length), L'\0');
  if (MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(),
                          static_cast<int>(text.size()), result.data(), length) !=
      length)
    return {};
  return result;
}

bool should_polish(const VoiceInputConfig &config, std::string_view text) {
  const auto endpoint = config.polish_endpoint.empty()
                            ? default_polish_endpoint(config.polish_provider)
                            : config.polish_endpoint;
  const auto model = config.polish_model.empty()
                         ? default_polish_model(config.polish_provider)
                         : config.polish_model;
  return (config.polish_enabled || config.polish_text) && !text.empty() &&
         !config.polish_token.empty() && secure_voice_endpoint(endpoint, false) &&
         !model.empty();
}

std::string polish_prompt(const VoiceInputConfig &config) {
  PolishPromptSlots slots;
  slots.id = config.polish_prompt_id;
  slots.custom_1 = config.polish_prompt_custom_1;
  slots.custom_2 = config.polish_prompt_custom_2;
  slots.custom_3 = config.polish_prompt_custom_3;
  return polish_prompt_for(slots);
}

// 一次录音的投递目标：焦点租约所属的 TSF 客户端进程，开始录音时的前台窗口（前台是 Server 自己的窗口时为空），以及核对开始录音时的焦点租约仍是当前焦点的回调（为空时不核对）。
struct CommitTarget {
  uint32_t process = 0;
  HWND window = nullptr;
  std::function<bool()> lease_current;
};

// 开始录音时的前台窗口。从托盘菜单或浮动工具栏开始时前台可能是 Server 自己，这时不记窗口，只按进程核对。
HWND recording_start_window() {
  const HWND window = GetForegroundWindow();
  if (!window)
    return nullptr;
  DWORD process = 0;
  (void)GetWindowThreadProcessId(window, &process);
  return process != 0 && process != GetCurrentProcessId() ? window : nullptr;
}

// wide() 的反方向，打字统计只记实际送出的那一段时用。
std::string utf8(std::wstring_view text) {
  if (text.empty())
    return {};
  const int length = WideCharToMultiByte(CP_UTF8, 0, text.data(), static_cast<int>(text.size()),
                                         nullptr, 0, nullptr, nullptr);
  if (length <= 0)
    return {};
  std::string result(static_cast<size_t>(length), '\0');
  if (WideCharToMultiByte(CP_UTF8, 0, text.data(), static_cast<int>(text.size()), result.data(),
                          length, nullptr, nullptr) != length)
    return {};
  return result;
}

// 前台进程是 TSF 客户端进程，或前台窗口仍是开始录音时那一个。只看前台，不验焦点租约。
bool target_foreground_matches(const CommitTarget &target) {
  DWORD foreground = 0;
  const HWND window = GetForegroundWindow();
  if (window)
    (void)GetWindowThreadProcessId(window, &foreground);
  return voice_target_in_foreground(target.process, foreground, GetCurrentProcessId(),
                                    target.window && window == target.window);
}

// 目标此刻是否仍然有效：前台对得上（target_foreground_matches），而且焦点租约仍是当前焦点（同一窗口里换了输入框时前台核对看不出来）。先看前台，前台不对就不去拿焦点闸门的锁。
bool target_in_foreground(const CommitTarget &target) {
  const bool foreground_matches = target_foreground_matches(target);
  return voice_delivery_target_current(
      foreground_matches,
      foreground_matches && (!target.lease_current || target.lease_current()));
}

// 逐个 UTF-16 单元模拟输入，每 16 个单元前确认一次目标仍在前台，前台换了就停下，和 macOS 按块投递时反复检查 current() 一样。返回实际送出的 UTF-16 单元数，停下时不会落在代理对中间；送出了一部分时调用方不能再走别的路线重复上屏，打字统计也只记送出的那一段。
std::size_t send_text_via_send_input(std::wstring_view text, const CommitTarget &target) {
  constexpr std::size_t kCheckUnits = 16;
  std::size_t since_check = kCheckUnits;
  std::size_t sent = 0;
  for (const wchar_t ch : text) {
    // 不在代理对中间停下，低位代理总跟着它的高位代理发出。
    const bool low_surrogate = ch >= 0xDC00 && ch <= 0xDFFF;
    if (since_check >= kCheckUnits && !low_surrogate) {
      if (!target_in_foreground(target))
        return sent;
      since_check = 0;
    }
    INPUT input[2]{};
    input[0].type = INPUT_KEYBOARD;
    input[0].ki.wScan = static_cast<WORD>(ch);
    input[0].ki.dwFlags = KEYEVENTF_UNICODE;
    // 这段文字由 Server 记作 voice，带上面板注入标记，tip 就不会再把它当直通字符记一次。
    input[0].ki.dwExtraInfo = static_cast<ULONG_PTR>(PanelTextSendInputExtraInfo);
    input[1] = input[0];
    input[1].ki.dwFlags |= KEYEVENTF_KEYUP;
    (void)SendInput(2, input, sizeof(INPUT));
    ++sent;
    ++since_check;
  }
  return sent;
}

// 把一个 DWORD 值以注册格式放进已打开的剪贴板。
bool set_clipboard_marker(const VoiceClipboardMarker &marker) {
  const std::wstring name(marker.format);
  const UINT format = RegisterClipboardFormatW(name.c_str());
  if (!format)
    return false;
  HGLOBAL memory = GlobalAlloc(GMEM_MOVEABLE, sizeof(DWORD));
  if (!memory)
    return false;
  auto *value = static_cast<DWORD *>(GlobalLock(memory));
  if (!value) {
    GlobalFree(memory);
    return false;
  }
  *value = marker.value;
  GlobalUnlock(memory);
  if (!SetClipboardData(format, memory)) {
    GlobalFree(memory);
    return false;
  }
  return true;
}

bool copy_text_to_clipboard(std::wstring_view text) {
  if (!OpenClipboard(nullptr))
    return false;
  const auto bytes = (text.size() + 1) * sizeof(wchar_t);
  HGLOBAL memory = GlobalAlloc(GMEM_MOVEABLE, bytes);
  if (!memory) {
    CloseClipboard();
    return false;
  }
  auto *destination = GlobalLock(memory);
  if (!destination) {
    GlobalFree(memory);
    CloseClipboard();
    return false;
  }
  std::memcpy(destination, text.data(), text.size() * sizeof(wchar_t));
  static_cast<wchar_t *>(destination)[text.size()] = L'\0';
  GlobalUnlock(memory);
  EmptyClipboard();
  // 识别文本不能留在任何剪贴板历史里，包括水杉自己的。标记放不上就整段不走剪贴板，调用方改用 SendInput。
  for (const auto &marker : voice_clipboard_markers)
    if (!set_clipboard_marker(marker)) {
      GlobalFree(memory);
      EmptyClipboard();
      CloseClipboard();
      return false;
    }
  if (!SetClipboardData(CF_UNICODETEXT, memory)) {
    GlobalFree(memory);
    EmptyClipboard();
    CloseClipboard();
    return false;
  }
  CloseClipboard();
  return true;
}

// 返回送出的 UTF-16 单元数，粘贴出去就是整段。写剪贴板前后各确认一次目标仍在前台；写入后前台换了就不粘贴，文字只留在已标记为不进历史的剪贴板里。
std::size_t send_text_via_ctrl_v(std::wstring_view text, const CommitTarget &target) {
  if (!target_in_foreground(target))
    return 0;
  if (!copy_text_to_clipboard(text))
    return send_text_via_send_input(text, target);
  Sleep(30);
  if (!target_in_foreground(target))
    return 0;
  INPUT input[4]{};
  input[0].type = INPUT_KEYBOARD;
  input[0].ki.wVk = VK_CONTROL;
  input[1].type = INPUT_KEYBOARD;
  input[1].ki.wVk = 'V';
  input[2] = input[1];
  input[2].ki.dwFlags = KEYEVENTF_KEYUP;
  input[3] = input[0];
  input[3].ki.dwFlags = KEYEVENTF_KEYUP;
  (void)SendInput(4, input, sizeof(INPUT));
  return text.size();
}

// The epoch gate prevents generation changes during each visible effect. A new session or a later failure takes the overlay over, so the wait ends early and leaves the overlay to it. This sleeps, so it runs on a worker: the overlay's window belongs to the control thread, and a sleeping control thread would paint nothing and then hide it at once.
void show_voice_failure(WaveOverlay &overlay, VoiceSessionEpoch &session,
                        uint64_t expected, std::atomic<uint64_t> &displays,
                        const std::wstring &message) {
  const uint64_t display = displays.fetch_add(1) + 1;
  if (!session.with_current(expected, [&] {
    overlay.set_listening(false);
    overlay.set_input_level(0.0f);
    overlay.set_show_transcript(true);
    overlay.set_compact_status(WaveOverlay::CompactStatus::None);
    overlay.set_actions_visible(false);
    overlay.set_transcript(message);
    overlay.show();
  }))
    return;
  for (DWORD waited = 0; waited < kFailureDisplayMs; waited += kFailurePollMs) {
    Sleep(kFailurePollMs);
    if (session.load() != expected || displays.load() != display)
      return;
  }
  session.with_current(expected, [&] {
    if (displays.load() != display)
      return;
    overlay.set_transcript(L"");
    overlay.hide();
  });
}
static_assert(std::is_invocable_v<decltype(show_voice_failure), WaveOverlay &,
                                  VoiceSessionEpoch &, uint64_t,
                                  std::atomic<uint64_t> &, const std::wstring &>);
static_assert(!std::is_invocable_v<decltype(show_voice_failure), WaveOverlay &,
                                   uint64_t, uint64_t, std::atomic<uint64_t> &,
                                   const std::wstring &>);
// The epoch must arrive as the live object, never a temporary. That used to be
// asserted through is_invocable with an rvalue argument, but MSVC answers true
// there even though the call itself does not compile - a temporary cannot bind
// to a non-const lvalue reference. Assert the signature instead: it is the
// signature that carries the guarantee, and unlike the trait it reads the same
// on every compiler.
template <typename Signature> struct epoch_parameter;
template <typename Result, typename First, typename Second, typename... Rest>
struct epoch_parameter<Result(First, Second, Rest...)> {
  using type = Second;
};
static_assert(
    std::is_same_v<epoch_parameter<decltype(show_voice_failure)>::type,
                   VoiceSessionEpoch &>);
} // namespace

// One on-device dictation fed while it is recorded, so its text appears as the person speaks, the way Doubao's does. The capture thread appends to a bounded queue: loading a model takes seconds and decoding a finished speech segment can take longer than a capture buffer, neither of which the audio callback may wait on. run() does both on a recognition task and finish() collects the transcript from it.
class LocalAsrStream final : public OnDeviceAsrStream {
public:
  using Partial = msime::voice::LocalAsrSession::PartialCallback;

  // Capture thread. Audio arriving after the end, a cancellation or a failed load is dropped rather than queued for a worker that will never read it. An over-budget queue is reported to the session so it can fail visibly.
  bool push(const float *samples, std::size_t count) override {
    if (!samples || count == 0)
      return true;
    const auto result = queue_.push(samples, count);
    if (result == LocalAsrAudioQueue::PushResult::overflowed) {
      cancelled_->store(true);
      return false;
    }
    return true;
  }

  // Recognition task of the finished recording: no more audio is coming. Waits for run() to decode what is left and returns the transcript, hotword-corrected; rethrows what the recognizer threw.
  std::string finish() override {
    queue_.finish();
    std::unique_lock lock(mutex_);
    done_wake_.wait(lock, [this] { return done_; });
    if (error_)
      std::rethrow_exception(error_);
    return result_;
  }

  // Any thread, any number of times, also after finish(). The recognizer stops at its next check and run() returns without a transcript.
  void cancel() override {
    cancelled_->store(true);
    queue_.cancel();
  }

  // 控制线程在录音中调用：模型加载或识别已经出错结束时返回那个异常。取消和溢出不算。
  std::exception_ptr failure() override {
    if (cancelled_->load())
      return nullptr;
    std::lock_guard lock(mutex_);
    return done_ ? error_ : nullptr;
  }

  // The recognition task started with the recording. Reads the hotwords itself, because listing the dictionary reads the store, which the control thread must not wait on.
  void run(const VoiceInputConfig &config, const Partial &on_partial) {
    std::string text;
    std::exception_ptr error;
    try {
      const auto hotwords = local_hotwords(config);
      msime::voice::LocalAsrOptions options;
      options.model_dir = config.asr_model_path;
      options.language = config.language;
      options.hotwords = hotword_texts(hotwords);
      msime::voice::LocalAsrSession session(options, on_partial, cancelled_);
      std::vector<float> batch;
      for (;;) {
        bool last = false;
        batch = queue_.wait_and_take(last);
        if (cancelled_->load())
          break;
        // Everything queued while the model loaded arrives here in one call; the recognizer slices it.
        if (!batch.empty())
          session.accept(batch.data(), batch.size());
        batch.clear();
        if (last) {
          // The queue closes before finish() waits, so no later capture batch
          // can be added after this one.
          text = session.finish();
          break;
        }
      }
      if (!cancelled_->load() && !text.empty() && !hotwords.empty() &&
          msime::voice::local_model_uses_pinyin_hotwords(config.asr_model_path))
        text = correct_with_hotwords(text, hotwords);
    } catch (...) {
      error = std::current_exception();
    }
    // 出错后不再读队列：之后采集推来的音频直接丢掉，不在队列里攒到溢出、被误报成录音中断。
    if (error)
      queue_.cancel();
    {
      std::lock_guard lock(mutex_);
      result_ = std::move(text);
      error_ = error;
      done_ = true;
    }
    done_wake_.notify_all();
  }

private:
  std::shared_ptr<std::atomic_bool> cancelled_ =
      std::make_shared<std::atomic_bool>(false);
  LocalAsrAudioQueue queue_;
  std::mutex mutex_;
  std::condition_variable done_wake_;
  bool done_ = false;
  std::string result_;
  std::exception_ptr error_;
};

VoiceInputSession::VoiceInputSession(WaveOverlay &overlay,
                                     LeaseProvider lease_provider,
                                     Sender sender,
                                     ConfigProvider config_provider,
                                     FocusValidator focus_validator,
                                     CommitRecorder commit_recorder)
    : overlay_(overlay), lease_provider_(std::move(lease_provider)),
      sender_(std::move(sender)), config_provider_(std::move(config_provider)),
      focus_validator_(std::move(focus_validator)),
      commit_recorder_(std::move(commit_recorder)),
      capture_owner_(std::make_unique<AudioCapture>()) {
  capture_ = capture_owner_.get();
}

VoiceInputSession::~VoiceInputSession() {
  cancel();
  if (capture_)
    capture_->stop();
  {
    std::lock_guard lock(tasks_mutex_);
    for (auto &task : tasks_)
      if (task.valid())
        task.wait();
  }
  if (idle_release_.valid())
    idle_release_.wait();
  // After the recognizers: a finishing one may still report a failure. cancel() moved the epoch on, so each display ends within one poll.
  std::lock_guard lock(notices_mutex_);
  for (auto &notice : notices_)
    if (notice.valid())
      notice.wait();
}

bool VoiceInputSession::init_cues(const std::wstring &start_path,
                                  const std::wstring &end_path) {
  return cue_player_.init(start_path, end_path);
}

bool VoiceInputSession::toggle() {
  if (recording() || starting_.load()) {
    stop();
    return true;
  }
  return start();
}

std::shared_ptr<VoiceReviewResult>
VoiceInputSession::start_review(std::string_view language) {
  if (language.empty() ||
      language.size() > FanyImeVoiceController::MaxLanguageBytes)
    return {};
  for (const unsigned char ch : language)
    if (!((ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z') ||
          (ch >= '0' && ch <= '9') || ch == '-'))
      return {};
  // A panel must not steal a native recording or its in-flight completion.
  if (recording_.load() || starting_.load())
    return {};
  {
    std::lock_guard lock(tasks_mutex_);
    for (auto &task : tasks_)
      if (task.valid() &&
          task.wait_for(std::chrono::seconds(0)) != std::future_status::ready)
        return {};
  }
  auto review = std::make_shared<VoiceReviewResult>();
  if (!start(review, language)) {
    review->fail();
    return {};
  }
  return review;
}

bool VoiceInputSession::stop_review(
    const std::shared_ptr<VoiceReviewResult> &expected) {
  if (!expected || expected != review_)
    return false;
  stop();
  return true;
}

bool VoiceInputSession::cancel_review(
    const std::shared_ptr<VoiceReviewResult> &expected) {
  if (!expected || expected != review_)
    return false;
  cancel();
  return true;
}

bool VoiceInputSession::start(std::shared_ptr<VoiceReviewResult> review,
                              std::string_view language) {
  if (review_ && review_->active())
    return false;
  if (!capture_ || !lease_provider_ || !sender_ || !config_provider_)
    return false;
  VoiceInputConfig config = config_provider_();
  if (review)
    config.language = std::string(language);
  const bool doubao = is_doubao_asr_provider(config.asr_provider);
  // An endpoint whose transport disagrees with the provider is configuration
  // left behind by an earlier choice, so fall back to this provider's own
  // default rather than posting its token to the previous provider's host.
  // Doubao speaks websocket and the others speak HTTPS, so the scheme is a
  // sufficient test, and existing installs with a stale endpoint are repaired
  // here rather than only for users who re-pick the provider in settings.
  const auto endpoint = resolved_asr_endpoint(config.asr_provider, config.endpoint);
  const auto model = config.model.empty()
                         ? default_asr_model(config.asr_provider)
                         : config.model;
  const bool local = is_local_asr_provider(config.asr_provider);
  // An installed catalog model decodes as the audio arrives and reports partial text like Doubao. Any other path, such as a Whisper model file the setting held before the catalog, goes through the batch recognizer, which refuses it after the recording.
  const bool local_stream =
      local && msime::voice::is_local_model_dir(config.asr_model_path);
  // 系统识别总是边录边识别，中间结果和本地模型一样走内联预编辑或浮层。
  const bool system = is_system_asr_provider(config.asr_provider);
  const bool stream_inline = voice_inline_allowed(
      review, config.stream_inline_preedit, doubao || local_stream || system,
      config.commit_mode);
  // Without a focused input context there is nothing to dictate into, and MSIME-Windows says nothing either.
  const auto lease = lease_provider_();
  if (!lease || !lease->epoch || !lease->token)
    return false;
  // A review capture reports through its own result object; only native recordings are told why they did not start.
  const auto refuse = [&](std::string_view message) {
    if (!review)
      report_failure(message, session_.load());
    return false;
  };
  const auto verdict = voice_start_verdict(
      {config.enabled, doubao, config.token, endpoint, model, config.resource_id,
       local, config.asr_model_path, system});
  if (verdict.check == VoiceStartCheck::Disabled)
    return false;
  if (verdict.check == VoiceStartCheck::Rejected)
    return refuse(verdict.message);
  // 没装这种语言的语音识别时在开始录音前就说清楚，而不是让人说完一整段才看到失败。
  if (system)
    if (const auto problem = system_asr_start_problem(config.language))
      return refuse(*problem);
  // 这台主机打不开的设备选择（别的平台写下的后端或设备 id）就是所选麦克风不可用。
  if (!config.capture.supported())
    return refuse(voice_microphone_device_message);
  bool expected = false;
  if (!starting_.compare_exchange_strong(expected, true))
    return false;
  const uint64_t session = session_.fetch_add(1) + 1;
  review_ = review;
  {
    std::lock_guard lock(config_mutex_);
    active_config_ = config;
  }
  cancel_requested_.store(false);
  locked_.store(false);
  capture_full_.store(false);
  local_stream_overflow_.store(false);
  {
    std::lock_guard lock(samples_mutex_);
    samples_.clear();
    captured_frames_ = 0;
  }
  lease_ = *lease;
  start_window_ = recording_start_window();
  const auto generation = static_cast<wchar_t>((session % 0xfffeu) + 1u);
  // Streaming recognizers report the whole transcript so far, from their own thread: inline as the composition when that is allowed, else on the overlay.
  const auto show_partial = [this, lease = *lease, generation, session,
                             stream_inline, review](const std::string &text) {
    if (review)
      return; // panel receives the final bounded result only
    if (session_.load() != session || cancel_requested_.load())
      return;
    session_.with_current(session, [&] {
      const auto converted = wide(text);
      if (stream_inline) {
        (void)sender_(lease, FanyImeWorkerReplyType::UpdateVoiceComposition,
                      converted, generation);
      } else {
        overlay_.set_transcript(converted);
      }
    });
  };
  // Replaced even when this recording does not stream: a stream still finishing an earlier recording belongs to that recording's task alone, and stop() must not hand it to this one.
  const auto local_recognizer =
      local_stream ? std::make_shared<LocalAsrStream>() : nullptr;
  const auto system_recognizer =
      system ? std::make_shared<SystemAsrStream>() : nullptr;
  const std::shared_ptr<OnDeviceAsrStream> stream =
      local_recognizer ? std::shared_ptr<OnDeviceAsrStream>(local_recognizer)
                       : std::shared_ptr<OnDeviceAsrStream>(system_recognizer);
  {
    std::lock_guard lock(local_stream_mutex_);
    local_stream_ = stream;
  }
  // Before the recognition task starts, the stream holds only the queue, so a failed start just retires it.
  const auto drop_stream = [&] {
    if (!stream)
      return;
    stream->cancel();
    std::lock_guard lock(local_stream_mutex_);
    if (local_stream_ == stream)
      local_stream_.reset();
  };
  if (doubao) {
    auto client = std::make_shared<DoubaoAsrClient>(
        endpoint, config.doubao_auth_mode, config.app_key, config.token,
        config.resource_id, config.enable_itn, config.enable_punc,
        config.enable_ddc, config.boosting_table_id, show_partial);
    {
      std::lock_guard lock(doubao_mutex_);
      doubao_ = client;
    }
    if (!client->Start()) {
      std::lock_guard lock(doubao_mutex_);
      if (doubao_ == client)
        doubao_.reset();
      lease_.reset();
      starting_.store(false);
      return refuse(voice_doubao_start_message);
    }
  }
  // 阿里云百炼（chat_audio）一次能上传的录音短得多（约 218 秒），录到它就结束并提交，和 macOS 一样，不让录满后整段被拒。
  const std::size_t batch_limit = batch_capture_sample_limit_for(config.asr_provider);
  const bool started = capture_->start([this, review, batch_limit](const float *samples,
                                                                   std::size_t frames) {
    if (!samples || (!recording_.load() && !starting_.load()))
      return;
    double sum = 0.0;
    for (std::size_t i = 0; i < frames; ++i)
      sum += static_cast<double>(samples[i]) * samples[i];
    const float rms = frames ? static_cast<float>(std::sqrt(sum / frames)) : 0.0f;
    const float normalized = std::min(1.0f, std::max(0.0f, rms - 0.004f) * 14.0f);
    const float level = std::pow(normalized, 0.55f);
    if (review)
      review->level(level);
    else
      overlay_.set_input_level(level);
    std::shared_ptr<DoubaoAsrClient> client;
    {
      std::lock_guard doubao_lock(doubao_mutex_);
      client = doubao_;
    }
    std::shared_ptr<OnDeviceAsrStream> local_feed;
    if (!client) {
      std::lock_guard stream_lock(local_stream_mutex_);
      local_feed = local_stream_;
    }
    // Streaming bypasses the batch buffer. Its own bounded queue reports an
    // overflow to maintain(), which cancels the recording instead of silently
    // dropping the second half of a hands-free dictation.
    if (client)
      client->PushFloatSamples(samples, frames);
    else if (local_feed && !local_feed->push(samples, frames))
      local_stream_overflow_.store(true);
    std::lock_guard lock(samples_mutex_);
    if (client || local_feed) {
      captured_frames_ += frames;
      return;
    }
    // A batch recording keeps what one upload can carry. When that is reached the recording finishes and submits it, as it does on macOS; maintain() calls stop(), which this capture thread must not.
    const auto capture = voice_batch_capture(captured_frames_, frames, batch_limit);
    samples_.insert(samples_.end(), samples, samples + capture.keep);
    captured_frames_ += capture.keep;
    if (capture.full)
      capture_full_.store(true);
  }, config.capture.device_id);
  if (!started) {
    std::shared_ptr<DoubaoAsrClient> client;
    {
      std::lock_guard lock(doubao_mutex_);
      client = std::move(doubao_);
    }
    if (client)
      client->Cancel();
    drop_stream();
    lease_.reset();
    starting_.store(false);
    return refuse(voice_capture_start_message(capture_->last_failure()));
  }
  if (cancel_requested_.load()) {
    capture_->stop();
    std::shared_ptr<DoubaoAsrClient> client;
    {
      std::lock_guard lock(doubao_mutex_);
      client = std::move(doubao_);
    }
    if (client)
      client->Cancel();
    drop_stream();
    lease_.reset();
    starting_.store(false);
    return false;
  }
  recording_.store(true);
  starting_.store(false);
  if (!review) {
    overlay_.set_input_level(0.0f);
    overlay_.set_listening(true);
    overlay_.set_show_transcript(!stream_inline);
    overlay_.set_compact_status(WaveOverlay::CompactStatus::None);
    overlay_.set_actions_visible(false);
    overlay_.set_transcript(L"");
    overlay_.show();
  }
  // 先放开始提示音再静音其他声音。自己的提示音文件走本进程的音频会话，不受静音影响；退回系统声音时它走系统声音会话，静音推迟到 maintain() 里它放完之后。
  const bool system_beep =
      config.sound_enabled && config.start_sound && !cue_player_.play_start();
  pending_mute_at_.store(0);
  if (config.mute_system_audio) {
    if (system_beep) {
      pending_mute_at_.store(
          (std::chrono::steady_clock::now() + kSystemBeepMuteDelay).time_since_epoch().count());
    } else {
      mute_other_system_audio();
      muted_system_audio_.store(true);
    }
  }
  if (stream) {
    // The model loads while the person starts speaking; what the capture queues meanwhile is decoded once it is ready. finish() or cancel_session() ends the task, and the destructor waits for it with the others.
    std::lock_guard lock(tasks_mutex_);
    tasks_.erase(std::remove_if(tasks_.begin(), tasks_.end(),
                                [](auto &task) {
                                  return task.wait_for(std::chrono::seconds(0)) ==
                                         std::future_status::ready;
                                }),
                  tasks_.end());
    tasks_.emplace_back(std::async(
        std::launch::async,
        [this, local_recognizer, system_recognizer, config, show_partial] {
          if (system_recognizer) {
            system_recognizer->run(config.language, show_partial);
            return;
          }
          local_recognizer->run(config, show_partial);
          note_local_model_use();
        }));
  }
  (void)session;
  return true;
}

void VoiceInputSession::stop() {
  if (!recording_.exchange(false))
    return;
  if (capture_)
    capture_->stop();
  if (local_stream_overflow_.exchange(false)) {
    const bool native = !review_;
    cancel_session(true);
    if (native)
      report_failure(voice_capture_interrupted_message, session_.load());
    return;
  }
  // A callback that threw stopped delivering audio part-way, so what was captured is not the recording the person made. MSIME-Windows StopRecording discards it and says so.
  if (capture_ && capture_->callback_failed()) {
    const bool native = !review_;
    cancel_session(true);
    if (native)
      report_failure(voice_capture_interrupted_message, session_.load());
    return;
  }
  locked_.store(false);
  const auto review = review_;
  if (review)
    review->recognizing();
  overlay_.set_listening(false);
  overlay_.set_input_level(0.0f);
  VoiceInputConfig config;
  {
    std::lock_guard lock(config_mutex_);
    config = active_config_ ? *active_config_ : config_provider_();
  }
  pending_mute_at_.store(0);
  if (muted_system_audio_.exchange(false))
    restore_other_system_audio();
  if (config.sound_enabled && config.end_sound)
    cue_player_.play_end();
  const auto lease = lease_;
  // Keep the target on the control thread so cancel during recognition can
  // retire its inline composition without relying on a stale worker callback.
  std::shared_ptr<DoubaoAsrClient> doubao;
  {
    std::lock_guard lock(doubao_mutex_);
    doubao = doubao_;
  }
  std::shared_ptr<OnDeviceAsrStream> local_stream;
  {
    std::lock_guard lock(local_stream_mutex_);
    local_stream = local_stream_;
  }
  const auto drop_local_stream = [&] {
    if (!local_stream)
      return;
    local_stream->cancel();
    std::lock_guard lock(local_stream_mutex_);
    if (local_stream_ == local_stream)
      local_stream_.reset();
  };
  const bool streaming = doubao || local_stream;
  const auto cancel_inline = [&] {
    if (!voice_inline_allowed(review, config.stream_inline_preedit, streaming,
                              config.commit_mode) ||
        !lease)
      return;
    const auto generation = static_cast<wchar_t>((session_.load() % 0xfffeu) + 1u);
    const auto encoded = voice_composition_bytes(
        FanyImeWorkerReplyType::CancelVoiceComposition, L"", generation);
    if (encoded)
      (void)sender_(*lease, FanyImeWorkerReplyType::CancelVoiceComposition,
                    L"", generation);
  };
  if (!lease) {
    if (review)
      review->fail();
    if (doubao)
      doubao->Cancel();
    drop_local_stream();
    cancel_inline();
    clear_overlay();
    return;
  }
  std::vector<float> samples;
  std::size_t captured_frames = 0;
  {
    std::lock_guard lock(samples_mutex_);
    samples.swap(samples_);
    captured_frames = captured_frames_;
  }
  if (captured_frames < kSampleRate / 4) {
    if (review)
      review->fail();
    if (doubao)
      doubao->Cancel();
    drop_local_stream();
    cancel_inline();
    clear_overlay();
    return;
  }
  const uint64_t session = session_.load();
  const bool stream_inline = voice_inline_allowed(
      review, config.stream_inline_preedit, streaming, config.commit_mode);
  if (!review) {
    finishing_session_.store(session);
    overlay_.set_compact_status(WaveOverlay::CompactStatus::Recognizing);
    overlay_.set_actions_visible(true);
    overlay_.set_show_transcript(!stream_inline);
    overlay_.show();
  }
  std::lock_guard lock(tasks_mutex_);
  tasks_.erase(std::remove_if(tasks_.begin(), tasks_.end(),
                              [](auto &task) {
                                return task.wait_for(std::chrono::seconds(0)) ==
                                       std::future_status::ready;
                              }),
                tasks_.end());
  auto cancelled = std::make_shared<std::atomic_bool>(false);
  {
    std::lock_guard request_lock(request_mutex_);
    prune_completed_voice_requests(request_cancellations_);
    request_cancellations_.push_back(cancelled);
  }
  tasks_.emplace_back(std::async(
      std::launch::async,
      [this, samples = std::move(samples), lease = *lease, config, session,
       doubao, local_stream, review, start_window = start_window_,
       cancelled = std::move(cancelled)]() mutable {
        finish(std::move(samples), lease, config, session, std::move(doubao),
               std::move(local_stream), std::move(cancelled), review,
               start_window);
      }));
}

void VoiceInputSession::finish(std::vector<float> samples, FocusLease lease,
                               VoiceInputConfig config, uint64_t session,
                               std::shared_ptr<DoubaoAsrClient> doubao,
                               std::shared_ptr<OnDeviceAsrStream> local_stream,
                               std::shared_ptr<std::atomic_bool> cancelled,
                               std::shared_ptr<VoiceReviewResult> review,
                               HWND start_window) {
  // However this ends, the stream's recognition task must too: a return before local_stream->finish() (a newer recording took over, the text came back empty) would otherwise leave it waiting for audio that never comes, and the destructor waiting for it.
  struct StreamRelease {
    VoiceInputSession &owner;
    const std::shared_ptr<OnDeviceAsrStream> &stream;
    ~StreamRelease() {
      if (!stream)
        return;
      stream->cancel();
      std::lock_guard lock(owner.local_stream_mutex_);
      if (owner.local_stream_ == stream)
        owner.local_stream_.reset();
    }
  } stream_release{*this, local_stream};
  // 无论从哪里返回，这次会话都不再处于识别中；新会话已经接手时不动它的标记。
  struct FinishingRelease {
    std::atomic<uint64_t> &finishing;
    uint64_t session;
    ~FinishingRelease() {
      auto expected = session;
      finishing.compare_exchange_strong(expected, 0);
    }
  } finishing_release{finishing_session_, session};
  const auto clear_current_overlay = [&] {
    if (review)
      review->fail(); // no-op after completion/cancellation
    session_.with_current(session, [&] { clear_overlay(); });
  };
  const auto release_doubao = [&] {
    if (!doubao)
      return;
    std::lock_guard lock(doubao_mutex_);
    if (doubao_ == doubao)
      doubao_.reset();
  };
  const bool stream_inline = voice_inline_allowed(
      review, config.stream_inline_preedit, doubao || local_stream,
      config.commit_mode);
  const auto cancel_inline = [&] {
    if (!stream_inline)
      return;
    const auto generation = static_cast<wchar_t>((session % 0xfffeu) + 1u);
    const auto encoded = voice_composition_bytes(
        FanyImeWorkerReplyType::CancelVoiceComposition, L"", generation);
    if (encoded)
      session_.with_current(session, [&] {
        (void)sender_(lease, FanyImeWorkerReplyType::CancelVoiceComposition, L"",
                      generation);
      });
  };
  if (session_.load() != session || cancel_requested_.load()) {
    clear_current_overlay();
    release_doubao();
    return;
  }
  std::string text;
  const bool local = !doubao && is_local_asr_provider(config.asr_provider);
  const bool system = !doubao && is_system_asr_provider(config.asr_provider);
  try {
    if (local_stream) {
      text = local_stream->finish();
    } else if (local) {
      text = recognize_local(samples, config, cancelled);
    } else if (doubao) {
      text = doubao->Finish();
      const auto error = doubao->LastError();
      if (text.empty() && !error.empty()) {
        cancel_inline();
        clear_current_overlay();
        release_doubao();
        if (!review && !cancel_requested_.load())
          report_failure(error, session);
        return;
      }
    } else if (system) {
      // 系统识别承诺音频不出设备：没有本机识别器接手的录音只报失败，绝不落到下面的云端上传分支。
      throw SystemAsrError("语音识别失败");
    } else {
      const auto endpoint = resolved_asr_endpoint(config.asr_provider, config.endpoint);
      const auto model = config.model.empty()
                             ? default_asr_model(config.asr_provider)
                             : config.model;
      text = recognize_cloud_asr(samples, config.asr_provider, endpoint, model,
                                 config.token, config.language, cancelled);
    }
  } catch (const std::exception &error) {
    cancel_inline();
    clear_current_overlay();
    release_doubao();
    if (!review && !cancel_requested_.load())
      report_failure(
          system ? system_asr_failure(error)
          : local ? std::string(voice_local_failure(
                      local_asr_available(),
                      msime::voice::is_local_model_dir(config.asr_model_path)))
                : voice_recognition_failure(error),
          session);
    return;
  }
  if (session_.load() != session || cancel_requested_.load() || text.empty()) {
    cancel_inline();
    clear_current_overlay();
    release_doubao();
    // 识别正常结束却没有文字：告诉人没听到，而不是让浮层一声不响地消失。取消或新会话接手时不提示。
    if (text.empty() && !review && session_.load() == session &&
        !cancel_requested_.load())
      report_failure(voice_no_speech_message, session);
    return;
  }
  if (!review)
    session_.with_current(session,
                          [&] { overlay_.set_transcript(wide(text)); });
  std::string final_text = text;
  if (should_polish(config, text)) {
    if (review)
      review->processing();
    else
      session_.with_current(session, [&] {
        overlay_.set_compact_status(WaveOverlay::CompactStatus::Processing);
        overlay_.set_actions_visible(true);
        // 识别中已经点 ✓ 收起的浮层不再为润色弹出来。
        if (dismissed_session_.load() != session)
          overlay_.show();
      });
    const auto endpoint = config.polish_endpoint.empty()
                              ? default_polish_endpoint(config.polish_provider)
                              : config.polish_endpoint;
    const auto model = config.polish_model.empty()
                           ? default_polish_model(config.polish_provider)
                           : config.polish_model;
    // Polishing is best-effort, as it is upstream: a transport error, a non-2xx
    // status or a missing body must not cost the user a transcript the ASR has
    // already produced. The exception used to escape into the std::async future
    // - which is only wait()ed, never get() - so the text vanished silently.
    try {
      auto polished =
          polish_cloud_text(text, config.polish_provider, endpoint, model,
                            config.polish_token, polish_prompt(config),
                            cancelled, voice_polish_timeout_ms);
      if (!polished.empty())
        final_text = std::move(polished);
    } catch (const std::exception &) {
      final_text = text;
    }
  }
  if (session_.load() != session || cancel_requested_.load() ||
      final_text.empty()) {
    cancel_inline();
    clear_current_overlay();
    release_doubao();
    return;
  }
  // 模拟按键和粘贴只能投给前台窗口，目标是焦点租约所属的 TSF 客户端进程或开始录音时的前台窗口；前台已经换成别处、或者焦点租约已经失效（换了输入框）时文字直接丢弃，和 macOS 的 stale 一样不写进别的应用或别的输入框。每次核对都重新验租约，不等控制循环下一轮的 maintain()。
  const CommitTarget target{client_pid(lease.transport), start_window, [this, &lease] {
                              return !focus_validator_ || focus_validator_(lease);
                            }};
  // 繁体输出打开、当前方案做简繁转换时，原生语音三条路线上屏的都是转换后的繁体，与 macOS applyVoiceResult（模拟按键和粘贴）及 TSF 路线经 displayTransition 转换一致；打字统计也记转换后的文字。开关在投递这一刻读，录音中途切换按最新状态。面板的审阅录音把原文交给面板，由面板决定怎样上屏。
  if (!review && config_provider_().traditional_output)
    final_text = simplified_to_traditional(final_text, true);
  // 实际送出的 UTF-16 单元数；SendInput 中途因前台换了而停下时只是前面一段。
  std::size_t delivered = 0;
  std::wstring converted;
  session_.with_current(session, [&] {
    deliver_voice_result(review, final_text, [&] {
      overlay_.set_compact_status(WaveOverlay::CompactStatus::None);
      overlay_.set_transcript(wide(final_text));
      converted = wide(final_text);
      const auto generation = static_cast<wchar_t>((session % 0xfffeu) + 1u);
      switch (voice_commit_route(config.commit_mode)) {
      case VoiceCommitRoute::SendInput:
        delivered = send_text_via_send_input(converted, target);
        break;
      case VoiceCommitRoute::CtrlV:
        delivered = send_text_via_ctrl_v(converted, target);
        break;
      case VoiceCommitRoute::Tsf: {
        const auto encoded = voice_composition_bytes(
            FanyImeWorkerReplyType::CommitVoiceComposition, converted,
            generation);
        const auto result =
            encoded ? sender_(lease, FanyImeWorkerReplyType::CommitVoiceComposition,
                              converted, generation)
                    : VoiceCompositionResult::Rejected;
        if (result == VoiceCompositionResult::Sent) {
          delivered = converted.size();
          break;
        }
        if (stream_inline)
          (void)sender_(lease, FanyImeWorkerReplyType::CancelVoiceComposition,
                        L"", generation);
        // 租约失效说明焦点已经去了别的输入框或窗口，整段文字丢弃。事务锁忙或编码失败时，录音目标仍在前台、焦点租约也仍是当前焦点才退回 SendInput；锁忙时 TSF 路线没验租约就返回了，这里的租约核对（会等正在进行的焦点事务结束）拦住点击别的输入框造成的那一种。管道写失败时 Server 已经自己作废租约、停下控制器，租约核对必然失败，而写之前它刚核对过租约是当前焦点，所以只要前台仍是目标就退回，之后的 SendInput 也只核对前台，免得丢掉整段录音。
        const bool focus_rejected = encoded && result == VoiceCompositionResult::Rejected;
        const bool write_failed = result == VoiceCompositionResult::Failed;
        const bool foreground = !focus_rejected && target_foreground_matches(target);
        const bool lease_current = foreground && !write_failed &&
                                   (!target.lease_current || target.lease_current());
        if (voice_tsf_refusal_falls_back(focus_rejected, write_failed, foreground,
                                         lease_current)) {
          CommitTarget fallback = target;
          if (write_failed)
            fallback.lease_current = nullptr;
          delivered = send_text_via_send_input(converted, fallback);
        }
        break;
      }
      }
      clear_overlay();
    });
  });
  // 三条路线送出的语音文字都记作 voice，与 macOS 投递路线一致。只记实际送出的那一段：SendInput 中途因前台换了而停下时，后面没进编辑器的文字不算。SendInput 注入的按键带着面板注入标记，tip 的直通统计会跳过它们，不会重复计成 unknown；Ctrl+V 本身带 Ctrl，也不进直通统计。
  if (delivered > 0 && !review && commit_recorder_)
    commit_recorder_(delivered >= converted.size()
                         ? final_text
                         : utf8(std::wstring_view(converted).substr(0, delivered)));
  release_doubao();
  {
    std::lock_guard lock(config_mutex_);
    if (active_config_ && session_.load() == session)
      active_config_.reset();
  }
}

std::string VoiceInputSession::recognize_local(
    const std::vector<float> &samples, const VoiceInputConfig &config,
    const std::shared_ptr<std::atomic_bool> &cancelled) {
  // Read here, on the recognition worker, rather than when the recording starts: listing the dictionary reads the store, which the control thread must not wait on.
  const auto hotwords = local_hotwords(config);
  // Stamped whether or not recognition succeeds: a model can load and then fail to decode, and it still has to be unloaded later.
  struct UseStamp {
    VoiceInputSession &session;
    ~UseStamp() { session.note_local_model_use(); }
  } stamp{*this};
  auto text = recognize_local_asr(samples, config.asr_model_path,
                                  config.language, cancelled,
                                  hotword_texts(hotwords));
  if (!text.empty() && !hotwords.empty() &&
      msime::voice::local_model_uses_pinyin_hotwords(config.asr_model_path))
    text = correct_with_hotwords(text, hotwords);
  return text;
}

void VoiceInputSession::note_local_model_use() {
  // The time is written before the flag; release_idle_local_model() relies on that order.
  local_model_used_.store(
      std::chrono::steady_clock::now().time_since_epoch().count());
  local_model_loaded_.store(true);
}

void VoiceInputSession::release_idle_local_model() {
  if (!local_model_loaded_.load())
    return;
  const auto idle_for = [this] {
    return std::chrono::steady_clock::now().time_since_epoch() -
           std::chrono::steady_clock::duration(local_model_used_.load());
  };
  if (idle_for() < kLocalModelIdle)
    return;
  if (idle_release_.valid() &&
      idle_release_.wait_for(std::chrono::seconds(0)) != std::future_status::ready)
    return;
  local_model_loaded_.store(false);
  // A dictation that finished between the check above and the store has already written a fresh time, so read it again rather than lose its flag.
  if (idle_for() < kLocalModelIdle) {
    local_model_loaded_.store(true);
    return;
  }
  // The recognizer keeps a model a live dictation is using, whatever its age.
  idle_release_ = std::async(std::launch::async, [] {
    (void)msime::voice::release_idle_local_models(kLocalModelIdle);
  });
}

void VoiceInputSession::cancel() { cancel_session(false); }

void VoiceInputSession::cancel_session(bool failed) {
  const auto review = review_;
  if (review) {
    if (failed)
      review->fail();
    else
      review->cancel();
  }
  const bool was_recording = recording_.exchange(false);
  cancel_requested_.store(true);
  const auto session = session_.fetch_add(1);
  locked_.store(false);
  VoiceInputConfig config;
  {
    std::lock_guard lock(config_mutex_);
    config = active_config_ ? *active_config_ : config_provider_();
  }
  {
    std::lock_guard request_lock(request_mutex_);
    prune_completed_voice_requests(request_cancellations_);
    for (const auto &request : request_cancellations_)
      request->store(true);
  }
  const auto lease = lease_;
  lease_.reset();
  if ((was_recording || starting_.load()) && capture_)
    capture_->stop();
  std::shared_ptr<DoubaoAsrClient> doubao;
  {
    std::lock_guard lock(doubao_mutex_);
    doubao = doubao_;
  }
  if (doubao)
    doubao->Cancel();
  std::shared_ptr<OnDeviceAsrStream> local_stream;
  {
    std::lock_guard lock(local_stream_mutex_);
    local_stream = std::move(local_stream_);
  }
  if (local_stream)
    local_stream->cancel();
  pending_mute_at_.store(0);
  if (muted_system_audio_.exchange(false))
    restore_other_system_audio();
  if (was_recording && config.sound_enabled && config.end_sound)
    cue_player_.play_end();
  if (lease && !review) {
    const auto generation = static_cast<wchar_t>((session % 0xfffeu) + 1u);
    const auto encoded = voice_composition_bytes(
        FanyImeWorkerReplyType::CancelVoiceComposition, L"", generation);
    if (encoded)
      (void)sender_(*lease, FanyImeWorkerReplyType::CancelVoiceComposition,
                    L"", generation);
  }
  clear_overlay();
  {
    std::lock_guard lock(config_mutex_);
    active_config_.reset();
  }
}

void VoiceInputSession::dismiss_processing() {
  if (recording_.load() || review_)
    return;
  const uint64_t session = session_.load();
  if (finishing_session_.load() != session)
    return;
  dismissed_session_.store(session);
  session_.with_current(session, [&] { clear_overlay(); });
}

void VoiceInputSession::lock() {
  if (!recording_.load())
    return;
  locked_.store(true);
  if (voice_lock_shows_actions(true, !!review_)) {
    overlay_.set_actions_visible(true);
    overlay_.show();
  }
}

void VoiceInputSession::maintain() {
  release_idle_local_model();
  // 焦点离开开始录音时的那个输入框（换了输入框、换了窗口或 TIP 断开）就取消，录音和识别、润色阶段都一样，识别结果不会写进别的地方。
  const bool in_progress =
      recording_.load() ||
      (finishing_session_.load() != 0 &&
       finishing_session_.load() == session_.load());
  // 只在原生语音进行中才去验租约：它要拿焦点闸门的锁，空闲时不必每轮都碰。
  if (!review_ && in_progress && lease_ && focus_validator_ &&
      voice_focus_loss_cancels(true, true, focus_validator_(*lease_))) {
    cancel_session(false);
    return;
  }
  if (muted_system_audio_.load())
    follow_default_system_audio_output();
  if (!recording_.load())
    return;
  // 开始提示音退回系统声音时推迟的静音，到点后在录音中补上。start、stop、cancel 和这里都在控制线程上，读和清零之间不会被别处改动。
  const auto mute_at = pending_mute_at_.load();
  if (mute_at != 0 && std::chrono::steady_clock::now().time_since_epoch().count() >= mute_at) {
    pending_mute_at_.store(0);
    mute_other_system_audio();
    muted_system_audio_.store(true);
  }
  if (local_stream_overflow_.exchange(false)) {
    const bool native = !review_;
    cancel_session(true);
    if (native)
      report_failure(voice_capture_interrupted_message, session_.load());
    return;
  }
  // 本机识别器（系统识别、本地模型）在录音中就已失败时立即结束这次录音并报出原因，而不是让人说完一整段、停下后才看到失败。识别器停下后采集推来的音频直接丢掉，不会走到上面的溢出判定。
  if (const auto failure = local_stream_failure()) {
    const bool native = !review_;
    cancel_session(true);
    if (native)
      report_failure(*failure, session_.load());
    return;
  }
  if (capture_ && capture_->callback_failed()) {
    // Detected while still recording rather than when the key comes up: a locked recording could otherwise sit on a dead microphone indefinitely.
    const bool native = !review_;
    cancel_session(true);
    if (native)
      report_failure(voice_capture_interrupted_message, session_.load());
    return;
  }
  if (capture_full_.load())
    stop();
}

std::optional<std::string> VoiceInputSession::local_stream_failure() {
  std::shared_ptr<OnDeviceAsrStream> stream;
  {
    std::lock_guard lock(local_stream_mutex_);
    stream = local_stream_;
  }
  if (!stream)
    return std::nullopt;
  const auto error = stream->failure();
  if (!error)
    return std::nullopt;
  VoiceInputConfig config;
  {
    std::lock_guard lock(config_mutex_);
    config = active_config_ ? *active_config_ : config_provider_();
  }
  return on_device_failure_message(config, error);
}

void VoiceInputSession::report_failure(std::string_view message,
                                       uint64_t session) {
  auto text = wide(message);
  if (text.empty())
    return;
  std::lock_guard lock(notices_mutex_);
  notices_.erase(std::remove_if(notices_.begin(), notices_.end(),
                                [](auto &notice) {
                                  return notice.wait_for(std::chrono::seconds(0)) ==
                                         std::future_status::ready;
                                }),
                 notices_.end());
  notices_.emplace_back(std::async(
      std::launch::async, [this, text = std::move(text), session] {
        show_voice_failure(overlay_, session_, session, failure_displays_, text);
      }));
}

void VoiceInputSession::clear_overlay() {
  overlay_.set_listening(false);
  overlay_.set_input_level(0.0f);
  overlay_.set_show_transcript(true);
  overlay_.set_compact_status(WaveOverlay::CompactStatus::None);
  overlay_.set_actions_visible(false);
  overlay_.set_transcript(L"");
  overlay_.hide();
}
} // namespace msime::windows

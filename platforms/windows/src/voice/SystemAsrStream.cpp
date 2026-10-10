#include "SystemAsrStream.h"
#include "SystemAsrPolicy.h"

#include <windows.h>
#include <mmsystem.h>
#include <sapi.h>
#include <wrl/client.h>

#include <algorithm>
#include <atomic>
#include <chrono>
#include <cstdint>
#include <cstring>
#include <utility>
#include <vector>

namespace msime::windows {
namespace {
using Microsoft::WRL::ComPtr;

// SAPI 的类和接口 id 在这里直接写出，不依赖 sapi.lib / libsapi.a 是否带着它们（MinGW 的 libsapi.a 没有 SPDFID_WaveFormatEx）。数值与 sapi.h 的 DEFINE_GUID 相同。
constexpr GUID kClsidSpInprocRecognizer{0x41b89b6b, 0x9399, 0x11d2, {0x96, 0x23, 0x00, 0xc0, 0x4f, 0x8e, 0xe6, 0x28}};
constexpr GUID kClsidSpObjectTokenCategory{0xa910187f, 0x0c7a, 0x45ac, {0x92, 0xcc, 0x59, 0xed, 0xaf, 0xb7, 0x7b, 0x53}};
constexpr GUID kIidSpRecognizer{0xc2b5f241, 0xdaa0, 0x4507, {0x9e, 0x16, 0x5a, 0x1e, 0xaa, 0x2b, 0x7a, 0x5c}};
constexpr GUID kIidSpObjectTokenCategory{0x2d3d3845, 0x39af, 0x4850, {0xbb, 0xf9, 0x40, 0xb4, 0x97, 0x80, 0x01, 0x1d}};
constexpr GUID kIidSpStreamFormat{0xbed530be, 0x2606, 0x4f4d, {0xa1, 0xc0, 0x54, 0xc5, 0xcd, 0xa5, 0x56, 0x6f}};
constexpr GUID kWaveFormatExFormatId{0xc31adbae, 0x527f, 0x4ff5, {0xa2, 0x30, 0xf6, 0x2b, 0xb6, 0x1f, 0xf7, 0x0c}};

constexpr DWORD kEventPollMs = 100;
constexpr ULONG kWholePhrase = static_cast<ULONG>(-1);
constexpr ULONGLONG event_bit(int id) { return 1ULL << id; }
// SAPI 要求兴趣掩码带上两个保留位（SDK 里 SPFEI 宏加的 SPFEI_FLAGCHECK），MinGW 的 sapi.h 没有这个宏，所以在这里算。
constexpr ULONGLONG kEventInterest =
    event_bit(SPEI_RESERVED1) | event_bit(SPEI_RESERVED2) | event_bit(SPEI_RECOGNITION) |
    event_bit(SPEI_HYPOTHESIS) | event_bit(SPEI_FALSE_RECOGNITION) | event_bit(SPEI_END_SR_STREAM);

// 本线程的 COM 初始化。线程已经是单线程套间时照样能创建进程内对象，只是不由这里收尾。
struct ComApartment {
  HRESULT result = CoInitializeEx(nullptr, COINIT_MULTITHREADED);
  ComApartment() = default;
  ComApartment(const ComApartment &) = delete;
  ComApartment &operator=(const ComApartment &) = delete;
  ~ComApartment() {
    if (SUCCEEDED(result))
      CoUninitialize();
  }
  bool usable() const { return SUCCEEDED(result) || result == RPC_E_CHANGED_MODE; }
};

std::string narrow(const wchar_t *text) {
  if (!text || !*text)
    return {};
  const int length = WideCharToMultiByte(CP_UTF8, 0, text, -1, nullptr, 0, nullptr, nullptr);
  if (length <= 1)
    return {};
  std::string result(static_cast<std::size_t>(length), '\0');
  if (WideCharToMultiByte(CP_UTF8, 0, text, -1, result.data(), length, nullptr, nullptr) != length)
    return {};
  result.resize(static_cast<std::size_t>(length - 1));
  return result;
}

// 释放事件带着的参数，对应 SDK sphelper.h 的 SpClearEvent。
void clear_event(SPEVENT &event) {
  switch (static_cast<int>(event.elParamType)) {
  case SPET_LPARAM_IS_POINTER:
  case SPET_LPARAM_IS_STRING:
    CoTaskMemFree(reinterpret_cast<void *>(event.lParam));
    break;
  case SPET_LPARAM_IS_TOKEN:
  case SPET_LPARAM_IS_OBJECT:
    if (event.lParam)
      reinterpret_cast<IUnknown *>(event.lParam)->Release();
    break;
  default:
    break;
  }
  event = SPEVENT{};
}

// 一条识别结果或中间假设的整句文字，用识别器的文本替换规则（数字、标点等的书写形式）。
std::string phrase_text(const SPEVENT &event) {
  auto *result = reinterpret_cast<ISpRecoResult *>(event.lParam);
  if (!result)
    return {};
  wchar_t *text = nullptr;
  if (FAILED(result->GetText(kWholePhrase, kWholePhrase, TRUE, &text, nullptr)))
    return {};
  auto converted = narrow(text);
  CoTaskMemFree(text);
  return converted;
}

// 交给识别器的音频输入：16 kHz 单声道 16 位 PCM，从这次录音的有界队列里读。识别器在自己的线程上调用 Read，凑不够请求的字节数就一直等，直到队列关闭；读到的比请求的少就是流结束，与 System.Speech 把实时音频流交给 SAPI 的做法相同。不支持定位，Seek 只报告当前位置。
class QueueAudioStream final : public ISpStreamFormat {
public:
  explicit QueueAudioStream(std::shared_ptr<LocalAsrAudioQueue> queue)
      : queue_(std::move(queue)) {}

  HRESULT STDMETHODCALLTYPE QueryInterface(REFIID id, void **object) override {
    if (!object)
      return E_POINTER;
    if (id == __uuidof(IUnknown) || id == __uuidof(ISequentialStream) ||
        id == __uuidof(IStream) || id == kIidSpStreamFormat) {
      *object = static_cast<ISpStreamFormat *>(this);
      AddRef();
      return S_OK;
    }
    *object = nullptr;
    return E_NOINTERFACE;
  }
  ULONG STDMETHODCALLTYPE AddRef() override { return ++references_; }
  ULONG STDMETHODCALLTYPE Release() override {
    const ULONG remaining = --references_;
    if (remaining == 0)
      delete this;
    return remaining;
  }

  HRESULT STDMETHODCALLTYPE Read(void *buffer, ULONG bytes, ULONG *read) override {
    if (!buffer && bytes)
      return STG_E_INVALIDPOINTER;
    auto *out = static_cast<unsigned char *>(buffer);
    ULONG filled = 0;
    while (filled < bytes) {
      if (offset_ >= pending_.size()) {
        if (ended_)
          break;
        bool last = false;
        const auto batch = queue_->wait_and_take(last);
        pending_.resize(batch.size() * sizeof(std::int16_t));
        for (std::size_t i = 0; i < batch.size(); ++i) {
          const std::int16_t sample = system_asr_pcm16(batch[i]);
          std::memcpy(pending_.data() + i * sizeof(sample), &sample, sizeof(sample));
        }
        offset_ = 0;
        ended_ = last;
        continue;
      }
      const auto take = std::min<std::size_t>(bytes - filled, pending_.size() - offset_);
      std::memcpy(out + filled, pending_.data() + offset_, take);
      offset_ += take;
      filled += static_cast<ULONG>(take);
    }
    position_ += filled;
    if (read)
      *read = filled;
    return S_OK;
  }
  HRESULT STDMETHODCALLTYPE Write(const void *, ULONG, ULONG *) override {
    return STG_E_ACCESSDENIED;
  }
  HRESULT STDMETHODCALLTYPE Seek(LARGE_INTEGER, DWORD, ULARGE_INTEGER *position) override {
    if (position)
      position->QuadPart = position_;
    return S_OK;
  }
  HRESULT STDMETHODCALLTYPE SetSize(ULARGE_INTEGER) override { return E_NOTIMPL; }
  HRESULT STDMETHODCALLTYPE CopyTo(IStream *, ULARGE_INTEGER, ULARGE_INTEGER *,
                                   ULARGE_INTEGER *) override {
    return E_NOTIMPL;
  }
  HRESULT STDMETHODCALLTYPE Commit(DWORD) override { return E_NOTIMPL; }
  HRESULT STDMETHODCALLTYPE Revert() override { return E_NOTIMPL; }
  HRESULT STDMETHODCALLTYPE LockRegion(ULARGE_INTEGER, ULARGE_INTEGER, DWORD) override {
    return E_NOTIMPL;
  }
  HRESULT STDMETHODCALLTYPE UnlockRegion(ULARGE_INTEGER, ULARGE_INTEGER, DWORD) override {
    return E_NOTIMPL;
  }
  // 实时流没有长度，报最大值，和 System.Speech 包装长度为 -1 的流时一样。
  HRESULT STDMETHODCALLTYPE Stat(STATSTG *stat, DWORD) override {
    if (!stat)
      return STG_E_INVALIDPOINTER;
    *stat = STATSTG{};
    stat->type = STGTY_STREAM;
    stat->cbSize.QuadPart = ~0ULL;
    return S_OK;
  }
  HRESULT STDMETHODCALLTYPE Clone(IStream **) override { return E_NOTIMPL; }

  HRESULT STDMETHODCALLTYPE GetFormat(GUID *format_id, WAVEFORMATEX **format) override {
    if (!format_id || !format)
      return E_POINTER;
    auto *wave = static_cast<WAVEFORMATEX *>(CoTaskMemAlloc(sizeof(WAVEFORMATEX)));
    if (!wave)
      return E_OUTOFMEMORY;
    *wave = WAVEFORMATEX{};
    wave->wFormatTag = WAVE_FORMAT_PCM;
    wave->nChannels = 1;
    wave->nSamplesPerSec = 16000;
    wave->wBitsPerSample = 16;
    wave->nBlockAlign = 2;
    wave->nAvgBytesPerSec = 32000;
    *format_id = kWaveFormatExFormatId;
    *format = wave;
    return S_OK;
  }

private:
  ~QueueAudioStream() = default;

  std::atomic<ULONG> references_{1};
  std::shared_ptr<LocalAsrAudioQueue> queue_;
  std::vector<unsigned char> pending_;
  std::size_t offset_ = 0;
  bool ended_ = false;
  ULONGLONG position_ = 0;
};

// 按识别语言找一个已安装的 SAPI 识别器令牌，同语言有多个时优先微软自带的。找不到时返回给人看的那句话。
std::optional<std::string> find_recognizer(std::string_view language, ISpObjectToken **token) {
  const auto language_id = system_asr_language_id(language);
  if (!language_id)
    return system_asr_unsupported_language_message(language);
  ComPtr<ISpObjectTokenCategory> category;
  if (FAILED(CoCreateInstance(kClsidSpObjectTokenCategory, nullptr, CLSCTX_INPROC_SERVER,
                              kIidSpObjectTokenCategory,
                              reinterpret_cast<void **>(category.GetAddressOf()))) ||
      !category)
    return std::string(system_asr_unavailable_message);
  // 没有任何识别器时 Recognizers 键本身就不存在，这和缺少这种语言是同一件事。
  if (FAILED(category->SetId(SPCAT_RECOGNIZERS, FALSE)))
    return system_asr_missing_language_message(language);
  const std::wstring required =
      L"Language=" + std::wstring(language_id->begin(), language_id->end());
  ComPtr<IEnumSpObjectTokens> tokens;
  ComPtr<ISpObjectToken> first;
  ULONG fetched = 0;
  if (FAILED(category->EnumTokens(required.c_str(), L"Vendor=Microsoft", tokens.GetAddressOf())) ||
      !tokens || tokens->Next(1, first.GetAddressOf(), &fetched) != S_OK || fetched != 1 ||
      !first)
    return system_asr_missing_language_message(language);
  if (token)
    *token = first.Detach();
  return std::nullopt;
}

// 一次听写用到的 SAPI 对象。析构时先关掉音频队列：识别器线程可能正阻塞在 Read 上，让它先读到流结束，停用识别器时两边才不会互相等待。成员按声明的逆序释放，语法先于识别器。
struct Dictation {
  std::shared_ptr<LocalAsrAudioQueue> queue;
  ComPtr<ISpRecognizer> recognizer;
  ComPtr<ISpRecoContext> context;
  ComPtr<ISpRecoGrammar> grammar;

  explicit Dictation(std::shared_ptr<LocalAsrAudioQueue> audio) : queue(std::move(audio)) {}
  Dictation(const Dictation &) = delete;
  Dictation &operator=(const Dictation &) = delete;
  ~Dictation() {
    queue->cancel();
    if (grammar)
      (void)grammar->SetDictationState(SPRS_INACTIVE);
    if (recognizer)
      (void)recognizer->SetRecoState(SPRST_INACTIVE_WITH_PURGE);
  }
};

void require(HRESULT result) {
  if (FAILED(result))
    throw SystemAsrError("语音识别失败");
}

std::string recognize_dictation(const std::shared_ptr<LocalAsrAudioQueue> &queue,
                                const std::string &language,
                                const SystemAsrStream::Partial &on_partial,
                                const std::atomic_bool &cancelled) {
  const ComApartment com;
  if (!com.usable())
    throw SystemAsrError(std::string(system_asr_unavailable_message));
  ComPtr<ISpObjectToken> token;
  if (const auto problem = find_recognizer(language, token.GetAddressOf()))
    throw SystemAsrError(*problem);
  Dictation dictation(queue);
  if (FAILED(CoCreateInstance(kClsidSpInprocRecognizer, nullptr, CLSCTX_INPROC_SERVER,
                              kIidSpRecognizer,
                              reinterpret_cast<void **>(dictation.recognizer.GetAddressOf()))) ||
      !dictation.recognizer)
    throw SystemAsrError(std::string(system_asr_unavailable_message));
  require(dictation.recognizer->SetRecognizer(token.Get()));
  ComPtr<QueueAudioStream> input;
  input.Attach(new QueueAudioStream(queue));
  // 允许识别器在引擎要求别的格式时自己插入格式转换。
  require(dictation.recognizer->SetInput(static_cast<ISpStreamFormat *>(input.Get()), TRUE));
  require(dictation.recognizer->CreateRecoContext(dictation.context.GetAddressOf()));
  require(dictation.context->SetNotifyWin32Event());
  require(dictation.context->SetInterest(kEventInterest, kEventInterest));
  require(dictation.context->CreateGrammar(0, dictation.grammar.GetAddressOf()));
  // 听写语法加载失败多半是该语言的识别器不带听写模型，说法与没装语音识别相同。
  if (FAILED(dictation.grammar->LoadDictation(nullptr, SPLO_STATIC)))
    throw SystemAsrError(system_asr_missing_language_message(language));
  require(dictation.grammar->SetDictationState(SPRS_ACTIVE));
  // 识别器没进入活动状态就不会读音频，录音只会在队列里攒到溢出、被误报成录音中断，所以这里失败要抛出来，由控制线程在录音中经 failure() 立即报出。
  require(dictation.recognizer->SetRecoState(SPRST_ACTIVE));

  std::string committed;
  std::string hypothesis;
  bool ended = false;
  bool drained = false;
  std::chrono::steady_clock::time_point deadline{};
  while (!ended && !cancelled.load()) {
    if (!drained && queue->closed()) {
      drained = true;
      deadline = std::chrono::steady_clock::now() + system_asr_drain_timeout;
    }
    if (drained && std::chrono::steady_clock::now() >= deadline) {
      committed = system_asr_join(committed, hypothesis);
      break;
    }
    if (dictation.context->WaitForNotifyEvent(kEventPollMs) != S_OK)
      continue;
    SPEVENT event{};
    ULONG fetched = 0;
    while (!ended && SUCCEEDED(dictation.context->GetEvents(1, &event, &fetched)) && fetched == 1) {
      switch (static_cast<int>(event.eEventId)) {
      case SPEI_HYPOTHESIS:
        hypothesis = phrase_text(event);
        if (on_partial)
          on_partial(system_asr_join(committed, hypothesis));
        break;
      case SPEI_RECOGNITION:
        committed = system_asr_join(committed, phrase_text(event));
        hypothesis.clear();
        if (on_partial)
          on_partial(committed);
        break;
      case SPEI_FALSE_RECOGNITION:
        if (!hypothesis.empty() && on_partial)
          on_partial(committed);
        hypothesis.clear();
        break;
      case SPEI_END_SR_STREAM:
        ended = true;
        break;
      default:
        break;
      }
      clear_event(event);
      fetched = 0;
    }
  }
  return cancelled.load() ? std::string{} : committed;
}
} // namespace

SystemAsrStream::SystemAsrStream()
    : queue_(std::make_shared<LocalAsrAudioQueue>()) {}

bool SystemAsrStream::push(const float *samples, std::size_t count) {
  if (!samples || count == 0)
    return true;
  if (queue_->push(samples, count) == LocalAsrAudioQueue::PushResult::overflowed) {
    cancelled_->store(true);
    return false;
  }
  return true;
}

std::string SystemAsrStream::finish() {
  queue_->finish();
  std::unique_lock lock(mutex_);
  done_wake_.wait(lock, [this] { return done_; });
  if (error_)
    std::rethrow_exception(error_);
  return result_;
}

void SystemAsrStream::cancel() {
  cancelled_->store(true);
  queue_->cancel();
}

std::exception_ptr SystemAsrStream::failure() {
  if (cancelled_->load())
    return nullptr;
  std::lock_guard lock(mutex_);
  return done_ ? error_ : nullptr;
}

void SystemAsrStream::run(const std::string &language, const Partial &on_partial) {
  std::string text;
  std::exception_ptr error;
  try {
    text = recognize_dictation(queue_, language, on_partial, *cancelled_);
  } catch (...) {
    error = std::current_exception();
  }
  // 识别器已经停下：之后采集推来的音频直接丢掉，不在队列里攒到溢出。出错时控制线程在录音中经 failure() 看到它，立即结束录音并报出原因。
  queue_->cancel();
  {
    std::lock_guard lock(mutex_);
    result_ = std::move(text);
    error_ = error;
    done_ = true;
  }
  done_wake_.notify_all();
}

std::optional<std::string> system_asr_start_problem(std::string_view language) {
  const ComApartment com;
  if (!com.usable())
    return std::string(system_asr_unavailable_message);
  return find_recognizer(language, nullptr);
}
} // namespace msime::windows

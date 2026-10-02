#include "SystemAudioMuter.h"
#include "AudioMuteState.h"

#include <audiopolicy.h>
#include <mmdeviceapi.h>
#include <windows.h>

#include <atomic>
#include <filesystem>
#include <sstream>
#include <mutex>
#include <string>
#include <utility>
#include <vector>

namespace msime::windows {
namespace {
struct MutedSession {
  ISimpleAudioVolume *volume = nullptr;
  std::wstring id;
};
class SessionNotification;
std::mutex mutex;
std::vector<MutedSession> muted;
IAudioSessionManager2 *manager = nullptr;
SessionNotification *notification = nullptr;
bool active = false;
bool com_owned = false;
std::wstring state_path;

std::wstring session_id(IAudioSessionControl *session) {
  IAudioSessionControl2 *control = nullptr;
  if (FAILED(session->QueryInterface(__uuidof(IAudioSessionControl2),
                                     reinterpret_cast<void **>(&control))) ||
      !control)
    return {};
  LPWSTR value = nullptr;
  std::wstring result;
  if (SUCCEEDED(control->GetSessionInstanceIdentifier(&value)) && value) {
    result = value;
    CoTaskMemFree(value);
  }
  control->Release();
  return result;
}

bool is_own_session(IAudioSessionControl *session) {
  IAudioSessionControl2 *control = nullptr;
  if (FAILED(session->QueryInterface(__uuidof(IAudioSessionControl2),
                                     reinterpret_cast<void **>(&control))) ||
      !control)
    return false;
  DWORD process = 0;
  const HRESULT result = control->GetProcessId(&process);
  control->Release();
  return SUCCEEDED(result) && process == GetCurrentProcessId();
}

bool known_locked(const std::wstring &id) {
  for (const auto &item : muted)
    if (!id.empty() && item.id == id)
      return true;
  return false;
}

// The reader decodes these ids with MultiByteToWideChar(CP_UTF8), so the writer
// has to encode with the matching conversion. Narrowing each wchar_t to char
// truncated every non-ASCII endpoint id and could not round-trip.
std::string utf8_from_wide(const std::wstring &value) {
  if (value.empty())
    return {};
  const int length =
      WideCharToMultiByte(CP_UTF8, 0, value.data(), static_cast<int>(value.size()),
                          nullptr, 0, nullptr, nullptr);
  if (length <= 0)
    return {};
  std::string text(static_cast<size_t>(length), '\0');
  if (WideCharToMultiByte(CP_UTF8, 0, value.data(), static_cast<int>(value.size()),
                          text.data(), length, nullptr, nullptr) != length)
    return {};
  return text;
}

void persist_locked() {
  if (state_path.empty())
    return;
  std::ostringstream output;
  for (const auto &item : muted) {
    if (!item.id.empty())
      output << "0\t" << utf8_from_wide(item.id) << '\n';
  }
  (void)write_audio_mute_state(std::filesystem::path(state_path), output.str());
}

void clear_state() {
  if (!state_path.empty())
    DeleteFileW(state_path.c_str());
}

void mute_session(IAudioSessionControl *session) {
  if (!session || is_own_session(session))
    return;
  ISimpleAudioVolume *volume = nullptr;
  if (FAILED(session->QueryInterface(__uuidof(ISimpleAudioVolume),
                                     reinterpret_cast<void **>(&volume))) ||
      !volume)
    return;
  BOOL is_muted = FALSE;
  if (FAILED(volume->GetMute(&is_muted)) || is_muted) {
    volume->Release();
    return;
  }
  const auto id = session_id(session);
  std::lock_guard lock(mutex);
  if (!active || known_locked(id) || FAILED(volume->SetMute(TRUE, nullptr))) {
    volume->Release();
    return;
  }
  muted.push_back({volume, id});
  persist_locked();
}

void mute_existing(IAudioSessionManager2 *audio_manager) {
  IAudioSessionEnumerator *enumerator = nullptr;
  if (FAILED(audio_manager->GetSessionEnumerator(&enumerator)) || !enumerator)
    return;
  int count = 0;
  if (SUCCEEDED(enumerator->GetCount(&count))) {
    for (int i = 0; i < count; ++i) {
      IAudioSessionControl *session = nullptr;
      if (SUCCEEDED(enumerator->GetSession(i, &session)) && session) {
        mute_session(session);
        session->Release();
      }
    }
  }
  enumerator->Release();
}

IAudioSessionManager2 *create_manager() {
  IMMDeviceEnumerator *enumerator = nullptr;
  if (FAILED(CoCreateInstance(__uuidof(MMDeviceEnumerator), nullptr,
                              CLSCTX_ALL, __uuidof(IMMDeviceEnumerator),
                              reinterpret_cast<void **>(&enumerator))) ||
      !enumerator)
    return nullptr;
  IMMDevice *device = nullptr;
  const HRESULT device_result =
      enumerator->GetDefaultAudioEndpoint(eRender, eMultimedia, &device);
  enumerator->Release();
  if (FAILED(device_result) || !device)
    return nullptr;
  IAudioSessionManager2 *result = nullptr;
  const HRESULT activate = device->Activate(
      __uuidof(IAudioSessionManager2), CLSCTX_ALL, nullptr,
      reinterpret_cast<void **>(&result));
  device->Release();
  return SUCCEEDED(activate) ? result : nullptr;
}

void restore_from_disk() {
  if (state_path.empty())
    return;
  std::string contents;
  if (!read_audio_mute_state(std::filesystem::path(state_path), contents))
    return;
  std::istringstream input(contents);
  std::vector<std::wstring> ids;
  std::string line;
  while (std::getline(input, line)) {
    const size_t tab = line.find('\t');
    if (tab == std::string::npos || tab + 1 >= line.size())
      continue;
    const char *text = line.data() + tab + 1;
    const int length = MultiByteToWideChar(
        CP_UTF8, 0, text, static_cast<int>(line.size() - tab - 1), nullptr, 0);
    if (length <= 0)
      continue;
    std::wstring id(static_cast<size_t>(length), L'\0');
    if (MultiByteToWideChar(CP_UTF8, 0, text,
                            static_cast<int>(line.size() - tab - 1), id.data(),
                            length) == length)
      ids.push_back(std::move(id));
  }
  if (ids.empty()) {
    clear_state();
    return;
  }
  IAudioSessionManager2 *audio_manager = create_manager();
  if (!audio_manager)
    return;
  IAudioSessionEnumerator *enumerator = nullptr;
  if (SUCCEEDED(audio_manager->GetSessionEnumerator(&enumerator)) &&
      enumerator) {
    int count = 0;
    enumerator->GetCount(&count);
    for (int i = 0; i < count; ++i) {
      IAudioSessionControl *session = nullptr;
      if (FAILED(enumerator->GetSession(i, &session)) || !session)
        continue;
      const auto id = session_id(session);
      bool recorded = false;
      for (const auto &saved : ids)
        if (saved == id) {
          recorded = true;
          break;
        }
      if (recorded) {
        ISimpleAudioVolume *volume = nullptr;
        if (SUCCEEDED(session->QueryInterface(
                __uuidof(ISimpleAudioVolume),
                reinterpret_cast<void **>(&volume))) &&
            volume) {
          volume->SetMute(FALSE, nullptr);
          volume->Release();
        }
      }
      session->Release();
    }
    enumerator->Release();
  }
  audio_manager->Release();
  clear_state();
}

void release_muted_locked() {
  for (auto &item : muted) {
    if (item.volume) {
      item.volume->SetMute(FALSE, nullptr);
      item.volume->Release();
      item.volume = nullptr;
    }
  }
  muted.clear();
}

class SessionNotification final : public IAudioSessionNotification {
public:
  HRESULT STDMETHODCALLTYPE QueryInterface(REFIID id, void **object) override {
    if (!object)
      return E_POINTER;
    if (id == IID_IUnknown || id == __uuidof(IAudioSessionNotification)) {
      *object = static_cast<IAudioSessionNotification *>(this);
      AddRef();
      return S_OK;
    }
    *object = nullptr;
    return E_NOINTERFACE;
  }
  ULONG STDMETHODCALLTYPE AddRef() override { return refs.fetch_add(1) + 1; }
  ULONG STDMETHODCALLTYPE Release() override {
    const ULONG left = refs.fetch_sub(1) - 1;
    if (!left)
      delete this;
    return left;
  }
  HRESULT STDMETHODCALLTYPE OnSessionCreated(IAudioSessionControl *session) override {
    mute_session(session);
    return S_OK;
  }
private:
  ~SessionNotification() = default;
  std::atomic<ULONG> refs{1};
};

void teardown() {
  IAudioSessionManager2 *old_manager = nullptr;
  SessionNotification *old_notification = nullptr;
  bool owned = false;
  {
    std::lock_guard lock(mutex);
    if (!active)
      return;
    active = false;
    old_manager = manager;
    manager = nullptr;
    old_notification = notification;
    notification = nullptr;
    owned = com_owned;
    com_owned = false;
    release_muted_locked();
  }
  clear_state();
  if (old_manager && old_notification)
    old_manager->UnregisterSessionNotification(old_notification);
  if (old_notification)
    old_notification->Release();
  if (old_manager)
    old_manager->Release();
  if (owned)
    CoUninitialize();
}
} // namespace

void configure_audio_mute_state_path(std::wstring path) {
  std::lock_guard lock(mutex);
  state_path = std::move(path);
}

void mute_other_system_audio() {
  restore_other_system_audio();
  const HRESULT init = CoInitializeEx(nullptr, COINIT_MULTITHREADED);
  const bool owned = init == S_OK;
  if (FAILED(init) && init != RPC_E_CHANGED_MODE && init != S_FALSE)
    return;
  IAudioSessionManager2 *audio_manager = create_manager();
  if (!audio_manager) {
    if (owned)
      CoUninitialize();
    return;
  }
  auto *session_notification = new SessionNotification();
  {
    std::lock_guard lock(mutex);
    active = true;
    manager = audio_manager;
    notification = session_notification;
    com_owned = owned;
  }
  audio_manager->RegisterSessionNotification(session_notification);
  mute_existing(audio_manager);
}

void restore_other_system_audio() {
  teardown();
  const HRESULT init = CoInitializeEx(nullptr, COINIT_MULTITHREADED);
  const bool owned = init == S_OK;
  if (FAILED(init) && init != RPC_E_CHANGED_MODE && init != S_FALSE)
    return;
  restore_from_disk();
  if (owned)
    CoUninitialize();
}
} // namespace msime::windows

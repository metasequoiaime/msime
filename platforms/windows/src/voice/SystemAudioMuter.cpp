#include "SystemAudioMuter.h"
#include "AudioMuteState.h"

#include <audiopolicy.h>
#include <mmdeviceapi.h>
#include <windows.h>

#include <algorithm>
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
class EndpointNotification;
// 一个被静音并监听新会话的输出设备。录音中默认输出换过几次就有几个。
struct DeviceWatch {
  IAudioSessionManager2 *manager = nullptr;
  SessionNotification *notification = nullptr;
  std::wstring device;
};
std::mutex mutex;
std::vector<MutedSession> muted;
// watches、device_enumerator 和 endpoint_notification 只在控制线程上增减（静音、跟随、恢复都由 VoiceInputSession 在控制线程调用），mutex 保护的是它们与音频服务回调线程共享的 active 和 muted。
std::vector<DeviceWatch> watches;
IMMDeviceEnumerator *device_enumerator = nullptr;
EndpointNotification *endpoint_notification = nullptr;
// 音频服务线程在默认输出设备变化时置位，控制线程在 follow_default_system_audio_output() 里取走。回调里不做任何阻塞工作。
std::atomic<bool> default_output_changed{false};
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
    (void)remove_private_file(std::filesystem::path(state_path));
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

IMMDeviceEnumerator *create_enumerator() {
  IMMDeviceEnumerator *enumerator = nullptr;
  if (FAILED(CoCreateInstance(__uuidof(MMDeviceEnumerator), nullptr,
                              CLSCTX_ALL, __uuidof(IMMDeviceEnumerator),
                              reinterpret_cast<void **>(&enumerator))) ||
      !enumerator)
    return nullptr;
  return enumerator;
}

std::wstring endpoint_id(IMMDevice *device) {
  LPWSTR value = nullptr;
  std::wstring result;
  if (SUCCEEDED(device->GetId(&value)) && value) {
    result = value;
    CoTaskMemFree(value);
  }
  return result;
}

IAudioSessionManager2 *activate_manager(IMMDevice *device) {
  IAudioSessionManager2 *result = nullptr;
  const HRESULT activate = device->Activate(
      __uuidof(IAudioSessionManager2), CLSCTX_ALL, nullptr,
      reinterpret_cast<void **>(&result));
  return SUCCEEDED(activate) ? result : nullptr;
}

// 把一个输出设备上记录过的会话取消静音。
void restore_sessions(IAudioSessionManager2 *audio_manager,
                      const std::vector<std::wstring> &ids) {
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
}

void restore_from_disk() {
  if (state_path.empty())
    return;
  std::string contents;
  if (!read_audio_mute_state(std::filesystem::path(state_path), contents))
    return;
  std::istringstream input(contents);
  std::vector<std::wstring> ids;
  ids.reserve(static_cast<std::size_t>(std::count(contents.begin(), contents.end(), '\n')) +
              (!contents.empty() && contents.back() != '\n'));
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
  // 静音会跟着默认输出设备换到别的设备上，所以记下的会话可能分布在任何一个输出设备上，逐个查找当前可用的输出设备。
  IMMDeviceEnumerator *device_list = create_enumerator();
  if (!device_list)
    return;
  IMMDeviceCollection *devices = nullptr;
  if (FAILED(device_list->EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE, &devices)) ||
      !devices) {
    device_list->Release();
    return;
  }
  UINT device_count = 0;
  if (FAILED(devices->GetCount(&device_count)))
    device_count = 0;
  for (UINT index = 0; index < device_count; ++index) {
    IMMDevice *device = nullptr;
    if (FAILED(devices->Item(index, &device)) || !device)
      continue;
    IAudioSessionManager2 *audio_manager = activate_manager(device);
    device->Release();
    if (!audio_manager)
      continue;
    restore_sessions(audio_manager, ids);
    audio_manager->Release();
  }
  devices->Release();
  device_list->Release();
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

// 只关心默认输出设备的变化，在音频服务的线程上调用，只置一个标记。
class EndpointNotification final : public IMMNotificationClient {
public:
  HRESULT STDMETHODCALLTYPE QueryInterface(REFIID id, void **object) override {
    if (!object)
      return E_POINTER;
    if (id == IID_IUnknown || id == __uuidof(IMMNotificationClient)) {
      *object = static_cast<IMMNotificationClient *>(this);
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
  // 与静音时选设备用的 eRender / eMultimedia 是同一个角色。
  HRESULT STDMETHODCALLTYPE OnDefaultDeviceChanged(EDataFlow flow, ERole role,
                                                   LPCWSTR) override {
    if (flow == eRender && role == eMultimedia)
      default_output_changed.store(true);
    return S_OK;
  }
  HRESULT STDMETHODCALLTYPE OnDeviceStateChanged(LPCWSTR, DWORD) override {
    return S_OK;
  }
  HRESULT STDMETHODCALLTYPE OnDeviceAdded(LPCWSTR) override { return S_OK; }
  HRESULT STDMETHODCALLTYPE OnDeviceRemoved(LPCWSTR) override { return S_OK; }
  HRESULT STDMETHODCALLTYPE OnPropertyValueChanged(LPCWSTR,
                                                   const PROPERTYKEY) override {
    return S_OK;
  }

private:
  ~EndpointNotification() = default;
  std::atomic<ULONG> refs{1};
};

// 静音当前默认输出设备上的其他应用并监听它新建的会话。已经在监听的设备不重复处理；之前设备上静音的会话留在 muted 里，restore 时一并恢复。控制线程调用。
void watch_default_output(IMMDeviceEnumerator *enumerator) {
  IMMDevice *device = nullptr;
  if (FAILED(enumerator->GetDefaultAudioEndpoint(eRender, eMultimedia, &device)) ||
      !device)
    return;
  const auto id = endpoint_id(device);
  for (const auto &watch : watches)
    if (!id.empty() && watch.device == id) {
      device->Release();
      return;
    }
  IAudioSessionManager2 *audio_manager = activate_manager(device);
  device->Release();
  if (!audio_manager)
    return;
  {
    std::lock_guard lock(mutex);
    if (!active) {
      audio_manager->Release();
      return;
    }
  }
  auto *session_notification = new SessionNotification();
  watches.push_back({audio_manager, session_notification, id});
  audio_manager->RegisterSessionNotification(session_notification);
  mute_existing(audio_manager);
}

void teardown() {
  std::vector<DeviceWatch> old_watches;
  IMMDeviceEnumerator *old_enumerator = nullptr;
  EndpointNotification *old_endpoints = nullptr;
  bool owned = false;
  {
    std::lock_guard lock(mutex);
    if (!active)
      return;
    active = false;
    old_watches.swap(watches);
    old_enumerator = device_enumerator;
    device_enumerator = nullptr;
    old_endpoints = endpoint_notification;
    endpoint_notification = nullptr;
    owned = com_owned;
    com_owned = false;
    release_muted_locked();
  }
  default_output_changed.store(false);
  clear_state();
  if (old_enumerator && old_endpoints)
    old_enumerator->UnregisterEndpointNotificationCallback(old_endpoints);
  if (old_endpoints)
    old_endpoints->Release();
  for (auto &watch : old_watches) {
    if (watch.manager && watch.notification)
      watch.manager->UnregisterSessionNotification(watch.notification);
    if (watch.notification)
      watch.notification->Release();
    if (watch.manager)
      watch.manager->Release();
  }
  if (old_enumerator)
    old_enumerator->Release();
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
  IMMDeviceEnumerator *enumerator = create_enumerator();
  if (!enumerator) {
    if (owned)
      CoUninitialize();
    return;
  }
  auto *endpoints = new EndpointNotification();
  {
    std::lock_guard lock(mutex);
    active = true;
    device_enumerator = enumerator;
    endpoint_notification = endpoints;
    com_owned = owned;
  }
  default_output_changed.store(false);
  // 先注册再静音：两步之间默认设备若刚好变了，下一轮 follow 会补上新设备。
  enumerator->RegisterEndpointNotificationCallback(endpoints);
  watch_default_output(enumerator);
}

void follow_default_system_audio_output() {
  if (!default_output_changed.exchange(false))
    return;
  IMMDeviceEnumerator *enumerator = nullptr;
  {
    std::lock_guard lock(mutex);
    if (!active || !device_enumerator)
      return;
    enumerator = device_enumerator;
  }
  // 静音时初始化的 COM 一直保持到 restore，调用线程就是同一个控制线程。
  watch_default_output(enumerator);
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

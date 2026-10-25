#include "UpdateNotifier.h"
#include "../../../common/HostApiString.h"
#include "../ipc/ServerResources.h"
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#include <shellapi.h>
#include <chrono>
#include <condition_variable>
#include <cstddef>
#include <exception>
#include <mutex>
#include <optional>
#include <stdexcept>
#include <system_error>
#include <thread>
#include <utility>

namespace msime::windows {

namespace {
constexpr wchar_t window_class[] = L"MsimeUpdateNotifier";
// 工作线程告诉窗口「有一个要提醒的构建」。
constexpr UINT ready_message = WM_APP + 0x31;
// 通知图标的回调消息；用 NOTIFYICON_VERSION_4，事件在 lParam 的低字。
constexpr UINT icon_message = WM_APP + 0x32;
constexpr UINT icon_id = 1;
// 系统没有发来超时或关闭（例如气泡被勿扰模式压下）时，图标最多留这么久。辅助功能里「通知显示时间」最长五分钟，再多留一分钟。
constexpr UINT_PTR removal_timer = 1;
constexpr UINT removal_delay_ms = 6 * 60 * 1000;
// 停下时等工作线程多久：它在联网时可能要十几秒才回来，不能让 Server 退出卡在这里。
constexpr std::chrono::seconds stop_wait{2};

// 定长字段放不下时截断，总留一个结尾的 0。
template <std::size_t N> void copy_text(wchar_t (&target)[N], const std::wstring &text) {
  const std::size_t length = text.size() < N - 1 ? text.size() : N - 1;
  text.copy(target, length);
  target[length] = L'\0';
}
} // namespace

struct UpdateNotifier::State {
  Options options;
  std::mutex mutex;
  std::condition_variable wake;
  std::condition_variable done;
  bool stopping = false;
  bool finished = false;
  // 工作线程交过来、还没显示的构建。
  std::optional<UpdateNotice> pending;
  // 下面这些只在消息循环线程上读写（window 例外：工作线程在锁里读它来投递消息）。
  HWND window = nullptr;
  HICON small_icon = nullptr;
  HICON large_icon = nullptr;
  bool icon_shown = false;
  // 正在显示的那条提醒，点了以后打开它。
  std::optional<UpdateNotice> shown;
  std::thread worker;
};

namespace {
void remove_icon(UpdateNotifier::State &state) {
  if (!state.icon_shown)
    return;
  NOTIFYICONDATAW data{};
  data.cbSize = sizeof(data);
  data.hWnd = state.window;
  data.uID = icon_id;
  Shell_NotifyIconW(NIM_DELETE, &data);
  KillTimer(state.window, removal_timer);
  state.icon_shown = false;
  state.shown.reset();
}

// 加图标（已经有一个时改它的内容）并弹出气泡。先不带气泡加上、设好 NOTIFYICON_VERSION_4，再改出气泡，保证回调从第一条起就是新格式。
void show_notice(UpdateNotifier::State &state, const UpdateNotice &notice) {
  const auto text = update_notification_text(notice);
  NOTIFYICONDATAW data{};
  data.cbSize = sizeof(data);
  data.hWnd = state.window;
  data.uID = icon_id;
  if (!state.icon_shown) {
    data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP;
    data.uCallbackMessage = icon_message;
    data.hIcon = state.small_icon;
    copy_text(data.szTip, text.title);
    if (!Shell_NotifyIconW(NIM_ADD, &data)) {
      if (state.options.log)
        state.options.log("Update notification icon unavailable");
      return;
    }
    state.icon_shown = true;
    data.uVersion = NOTIFYICON_VERSION_4;
    Shell_NotifyIconW(NIM_SETVERSION, &data);
  }
  data.uFlags = NIF_INFO | NIF_TIP | NIF_SHOWTIP;
  copy_text(data.szTip, text.title);
  copy_text(data.szInfoTitle, text.title);
  copy_text(data.szInfo, text.body);
  data.dwInfoFlags = NIIF_INFO;
  if (state.large_icon) {
    data.dwInfoFlags = NIIF_USER | NIIF_LARGE_ICON;
    data.hBalloonIcon = state.large_icon;
  }
  if (!Shell_NotifyIconW(NIM_MODIFY, &data)) {
    remove_icon(state);
    if (state.options.log)
      state.options.log("Update notification unavailable");
    return;
  }
  state.shown = notice;
  SetTimer(state.window, removal_timer, removal_delay_ms, nullptr);
}

void open_shown(UpdateNotifier::State &state) {
  const auto notice = state.shown;
  remove_icon(state);
  if (state.options.open_about && state.options.open_about())
    return;
  if (notice && !notice->release_url.empty())
    ShellExecuteW(nullptr, L"open", notice->release_url.c_str(), nullptr, nullptr, SW_SHOWNORMAL);
}

LRESULT CALLBACK window_procedure(HWND window, UINT message, WPARAM wparam, LPARAM lparam) {
  auto *state = reinterpret_cast<UpdateNotifier::State *>(GetWindowLongPtrW(window, GWLP_USERDATA));
  if (!state)
    return DefWindowProcW(window, message, wparam, lparam);
  switch (message) {
  case ready_message: {
    std::optional<UpdateNotice> notice;
    {
      std::lock_guard<std::mutex> lock(state->mutex);
      notice = std::move(state->pending);
      state->pending.reset();
    }
    if (notice)
      show_notice(*state, *notice);
    return 0;
  }
  case icon_message: {
    const UINT event = LOWORD(lparam);
    // 点了气泡（Windows 10/11 的通知）或者点了图标本身：打开「关于」页。
    if (event == NIN_BALLOONUSERCLICK || event == NIN_SELECT || event == NIN_KEYSELECT)
      open_shown(*state);
    // 气泡超时、被关掉、或者在图标上点了右键：不打开任何东西，只收起图标。
    else if (event == NIN_BALLOONTIMEOUT || event == NIN_BALLOONHIDE || event == WM_CONTEXTMENU)
      remove_icon(*state);
    return 0;
  }
  case WM_TIMER:
    if (wparam == removal_timer) {
      remove_icon(*state);
      return 0;
    }
    break;
  default:
    break;
  }
  return DefWindowProcW(window, message, wparam, lparam);
}

HICON load_logo(int size_metric) {
  const int size = GetSystemMetrics(size_metric);
  return static_cast<HICON>(LoadImageW(GetModuleHandleW(nullptr), MAKEINTRESOURCEW(IDI_MSIME_LOGO),
                                       IMAGE_ICON, size, size, LR_DEFAULTCOLOR));
}

void log_line(const UpdateNotifier::State &state, const std::string &line) {
  if (state.options.log)
    state.options.log(line);
}

// 工作线程：到点或者通道变了就调用一次共享层，有要提醒的构建就交给窗口。共享层回答「关掉了」时结束（开发构建、MSIME_DISABLE_UPDATE_CHECK 在本进程里不会再变）。
void run_worker(const std::shared_ptr<UpdateNotifier::State> &state) {
  using clock = std::chrono::steady_clock;
  auto next = clock::now() + update_first_check_delay;
  std::string last_channel;
  bool failure_logged = false;
  std::unique_lock<std::mutex> lock(state->mutex);
  while (!state->stopping) {
    state->wake.wait_for(lock, update_channel_poll, [&] { return state->stopping; });
    if (state->stopping)
      break;
    lock.unlock();
    const std::string channel = state->options.channel ? state->options.channel() : std::string("stable");
    const auto now = clock::now();
    if (now < next && (last_channel.empty() || channel == last_channel)) {
      lock.lock();
      continue;
    }
    last_channel = channel;
    next = now + update_check_interval;
    const auto body = update_background_request(state->options.directory, state->options.windows7,
                                                state->options.version, state->options.edition, channel)
                          .dump(-1, ' ', false, nlohmann::json::error_handler_t::replace);
    std::string response;
    if (auto owned = host_api::own_string(msime_client_update_background_check(
            reinterpret_cast<const uint8_t *>(body.data()), body.size())))
      response = owned.get();
    const auto result = parse_update_background_response(response);
    if (result.status == UpdateCheckStatus::disabled) {
      log_line(*state, "Background update check disabled in this build or environment");
      lock.lock();
      break;
    }
    if (result.status == UpdateCheckStatus::failed && !failure_logged) {
      failure_logged = true;
      log_line(*state, "Background update check failed; retrying hourly");
    }
    if (result.notice)
      log_line(*state, "Update available on channel " + channel + "; notifying");
    lock.lock();
    if (result.notice && !state->stopping) {
      state->pending = result.notice;
      if (state->window)
        PostMessageW(state->window, ready_message, 0, 0);
    }
  }
  state->finished = true;
  lock.unlock();
  state->done.notify_all();
}
} // namespace

UpdateNotifier::UpdateNotifier(Options options) : state_(std::make_shared<State>()) {
  state_->options = std::move(options);
  const HINSTANCE instance = GetModuleHandleW(nullptr);
  WNDCLASSEXW window_type{};
  window_type.cbSize = sizeof(window_type);
  window_type.lpfnWndProc = window_procedure;
  window_type.hInstance = instance;
  window_type.lpszClassName = window_class;
  if (!RegisterClassExW(&window_type) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS)
    throw std::runtime_error("Update notifier window class unavailable");
  // 普通的顶层窗口而不是只收消息的窗口：通知图标的回调要发给一个能收到 Explorer 消息的窗口。从不显示，WS_EX_TOOLWINDOW 让它也不进任务栏和 Alt+Tab。
  state_->window = CreateWindowExW(WS_EX_TOOLWINDOW, window_class, L"", WS_POPUP, 0, 0, 0, 0, nullptr,
                                   nullptr, instance, nullptr);
  if (!state_->window)
    throw std::runtime_error("Update notifier window unavailable");
  SetWindowLongPtrW(state_->window, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(state_.get()));
  state_->small_icon = load_logo(SM_CXSMICON);
  state_->large_icon = load_logo(SM_CXICON);
  auto shared = state_;
  try {
    state_->worker = std::thread([shared] {
      try {
        run_worker(shared);
      } catch (const std::exception &) {
        // 线程边界：后台检查出了意外只结束这个线程，不能让 std::terminate 带走输入法。
        std::lock_guard<std::mutex> lock(shared->mutex);
        shared->finished = true;
        shared->done.notify_all();
      }
    });
  } catch (const std::system_error &) {
    // 起不了线程时窗口不能留着指向随异常释放的状态。
    SetWindowLongPtrW(state_->window, GWLP_USERDATA, 0);
    DestroyWindow(state_->window);
    if (state_->small_icon)
      DestroyIcon(state_->small_icon);
    if (state_->large_icon)
      DestroyIcon(state_->large_icon);
    throw;
  }
}

UpdateNotifier::~UpdateNotifier() { stop(); }

void UpdateNotifier::stop() {
  if (!state_)
    return;
  HWND window = nullptr;
  {
    std::lock_guard<std::mutex> lock(state_->mutex);
    state_->stopping = true;
    window = state_->window;
  }
  state_->wake.notify_all();
  bool finished = false;
  {
    std::unique_lock<std::mutex> lock(state_->mutex);
    finished = state_->done.wait_for(lock, stop_wait, [&] { return state_->finished; });
  }
  if (state_->worker.joinable()) {
    if (finished)
      state_->worker.join();
    else
      state_->worker.detach();
  }
  remove_icon(*state_);
  if (window) {
    SetWindowLongPtrW(window, GWLP_USERDATA, 0);
    {
      std::lock_guard<std::mutex> lock(state_->mutex);
      state_->window = nullptr;
    }
    DestroyWindow(window);
  }
  if (state_->small_icon)
    DestroyIcon(state_->small_icon);
  if (state_->large_icon)
    DestroyIcon(state_->large_icon);
  state_->small_icon = state_->large_icon = nullptr;
  state_.reset();
}

} // namespace msime::windows

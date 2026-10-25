#include "UpdateNotifier.h"
#include "../../../common/HostApiString.h"
#include "../ipc/ServerResources.h"
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#include <shellapi.h>
#include <algorithm>
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
// 用户既没点也没右键时，图标最多留一天。气泡超时、被关掉都不删图标：共享层已经把这个构建记成提醒过，Windows 10/11 删掉图标会连操作中心里的那条通知一起删掉，被勿扰或专注助手压下的通知也只在操作中心里；一天后共享层早已不会再为它提醒，「关于」页仍显示它，图标不必在通知区域里一直挂着。
constexpr UINT_PTR removal_timer = 1;
constexpr UINT removal_delay_ms = 24 * 60 * 60 * 1000;
// 系统不接受通知时，每 `update_notice_retry` 再看一次能不能弹出。
constexpr UINT_PTR defer_timer = 2;
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
  // 系统不接受通知时留下、还没弹出的构建，从哪一刻开始等，以及等待的计时器是否在走。
  std::optional<UpdateNotice> deferred;
  std::chrono::steady_clock::time_point deferred_since{};
  bool deferring = false;
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
  const auto text = update_notification_text(notice, state.options.product_name);
  NOTIFYICONDATAW data{};
  data.cbSize = sizeof(data);
  data.hWnd = state.window;
  data.uID = icon_id;
  if (!state.icon_shown) {
    data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP;
    data.uCallbackMessage = icon_message;
    data.hIcon = state.small_icon;
    copy_text(data.szTip, text.tip);
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
  copy_text(data.szTip, text.tip);
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

// 系统现在是否接受通知。全屏游戏或演示（QUNS_BUSY、QUNS_RUNNING_D3D_FULL_SCREEN、QUNS_PRESENTATION_MODE）、锁屏或屏保（QUNS_NOT_PRESENT）、新用户登录后的安静时间（QUNS_QUIET_TIME）、全屏的商店应用都不算；查询失败时按接受处理，不因此不提醒。
bool system_accepts_notifications() {
  QUERY_USER_NOTIFICATION_STATE notification_state{};
  if (FAILED(SHQueryUserNotificationState(&notification_state)))
    return true;
  return notification_state == QUNS_ACCEPTS_NOTIFICATIONS;
}

// 把留下的构建弹出来：系统接受通知或者已经等满 `update_notice_max_defer` 时弹出，否则开着计时器，过一会儿再来。
void show_deferred(UpdateNotifier::State &state) {
  if (!state.deferred)
    return;
  const auto waited = std::chrono::duration_cast<std::chrono::minutes>(std::chrono::steady_clock::now() -
                                                                       state.deferred_since);
  if (!update_notice_may_show(system_accepts_notifications(), waited)) {
    if (!state.deferring) {
      state.deferring = true;
      SetTimer(state.window, defer_timer,
               static_cast<UINT>(std::chrono::duration_cast<std::chrono::milliseconds>(update_notice_retry).count()),
               nullptr);
      if (state.options.log)
        state.options.log("Update notification deferred until the system accepts notifications");
    }
    return;
  }
  if (state.deferring) {
    KillTimer(state.window, defer_timer);
    state.deferring = false;
  }
  const UpdateNotice notice = std::move(*state.deferred);
  state.deferred.reset();
  show_notice(state, notice);
}

// 工作线程交来一个要提醒的构建。已经有一个在等时换成新的，但从最早那个开始计时，等待的上限不因此往后推。
void present(UpdateNotifier::State &state, UpdateNotice notice) {
  if (!state.deferred)
    state.deferred_since = std::chrono::steady_clock::now();
  state.deferred = std::move(notice);
  show_deferred(state);
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
      present(*state, std::move(*notice));
    return 0;
  }
  case icon_message: {
    const UINT event = LOWORD(lparam);
    // 点了气泡（Windows 10/11 的通知）或者点了图标本身：打开「关于」页。
    if (event == NIN_BALLOONUSERCLICK || event == NIN_SELECT || event == NIN_KEYSELECT)
      open_shown(*state);
    // 在图标上点了右键：不打开任何东西，只收起图标。气泡超时（NIN_BALLOONTIMEOUT）、被关掉或收起（NIN_BALLOONHIDE）时图标留着，见 removal_timer。
    else if (event == WM_CONTEXTMENU)
      remove_icon(*state);
    return 0;
  }
  case WM_TIMER:
    if (wparam == removal_timer) {
      remove_icon(*state);
      return 0;
    }
    if (wparam == defer_timer) {
      show_deferred(*state);
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

std::string current_channel(const UpdateNotifier::State &state) {
  return state.options.channel ? state.options.channel() : std::string("stable");
}

// 工作线程：到点就调用一次共享层，有要提醒的构建就交给窗口。通道变了以后把下一次调用推到 `update_channel_settle` 之后（已经更晚的不提前），到时共享层见到新通道会立刻重查。共享层回答「关掉了」时结束（开发构建、MSIME_DISABLE_UPDATE_CHECK 在本进程里不会再变）。
void run_worker(const std::shared_ptr<UpdateNotifier::State> &state) {
  using clock = std::chrono::steady_clock;
  auto next = clock::now() + update_first_check_delay;
  std::string last_channel = current_channel(*state);
  bool failure_logged = false;
  std::unique_lock<std::mutex> lock(state->mutex);
  while (!state->stopping) {
    state->wake.wait_for(lock, update_channel_poll, [&] { return state->stopping; });
    if (state->stopping)
      break;
    lock.unlock();
    const std::string channel = current_channel(*state);
    const auto now = clock::now();
    if (channel != last_channel) {
      last_channel = channel;
      next = (std::max)(next, now + std::chrono::duration_cast<clock::duration>(update_channel_settle));
    }
    if (now < next) {
      lock.lock();
      continue;
    }
    next = now + update_check_interval;
    const auto body = update_background_request(state->options.directory, state->options.windows7,
                                                state->options.version, state->options.edition, channel,
                                                state->options.architecture)
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

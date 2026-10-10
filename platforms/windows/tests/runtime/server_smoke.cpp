#include "CandidateCardSize.h"
#include "CandidateClickWorker.h"
#include "CandidateFlyoutWindow.h"
#include "CandidateWindow.h"
#include "FloatingToolbarWindow.h"
#include "PreviewDispatcher.h"
#include "StateRootLease.h"
#include "../core/MaterializedSymlink.h"
#include "../core/TestHostOptions.h"
#include "TrayMenuWindow.h"
#include "WaveOverlay.h"
#include "WindowsServer.h"
#include <cstring>
#include <filesystem>
#include <cstdio>
#include <iostream>
#include <utility>
#include <vector>

using namespace msime::windows;
namespace {
void require_at(bool value, int line) {
  if (!value)
    throw std::runtime_error("Native Windows server fixture failed at line " +
                             std::to_string(line));
}
#define require(...) require_at((__VA_ARGS__), __LINE__)
// Click the middle of the first candidate row, measured with the same geometry
// the card draws and hit-tests with, so the fixture cannot drift from it.
LPARAM first_candidate_point(HWND window, size_t count) {
  RECT client{};
  require(GetClientRect(window, &client));
  const double scale = GetDpiForWindow(window) / 96.0;
  // The card sits inside transparent shadow margins, and the window hit-tests
  // against the card rather than against itself: rows are laid out in the
  // card's width, and a point in card space has to be moved back out by the
  // same insets to become the client point a mouse message carries. Measuring
  // against the window's own width and sending the result unshifted lands above
  // and left of the row - which is what this helper used to do, and why nothing
  // it produced ever registered as a click.
  const CandidateShadowInsets insets;
  const double card_width = client.right / scale - insets.left - insets.right;
  const auto row = candidate_row_bounds(
      0, count, card_width, candidate_card_metrics(16.0, 16.0, true), false);
  return MAKELPARAM(
      static_cast<int>((insets.left + (row.left + row.right) / 2.0) * scale),
      static_cast<int>((insets.top + (row.top + row.bottom) / 2.0) * scale));
}
template <class T> std::vector<uint8_t> fixture_bytes(const T &value) {
  std::vector<uint8_t> bytes(sizeof(T));
  std::memcpy(bytes.data(), &value, sizeof(T));
  return bytes;
}
struct ClientPipe {
  HANDLE handle = INVALID_HANDLE_VALUE;
  explicit ClientPipe(const std::wstring &name) {
    handle = CreateFileW(name.c_str(), GENERIC_READ | GENERIC_WRITE, 0, nullptr,
                         OPEN_EXISTING, FILE_FLAG_OVERLAPPED, nullptr);
    require(handle != INVALID_HANDLE_VALUE);
    DWORD mode = PIPE_READMODE_MESSAGE;
    if (!SetNamedPipeHandleState(handle, &mode, nullptr, nullptr)) {
      CloseHandle(handle);
      handle = INVALID_HANDLE_VALUE;
      require(false);
    }
  }
  ~ClientPipe() {
    if (handle != INVALID_HANDLE_VALUE)
      CloseHandle(handle);
  }
  ClientPipe(const ClientPipe &) = delete;
  ClientPipe &operator=(const ClientPipe &) = delete;
};
} // namespace
namespace {
// Wine honours SetThreadDpiAwarenessContext(PER_MONITOR_AWARE_V2) for the
// thread and then reports windows created on it as v1. Ask ntdll whether this
// is Wine rather than inferring it from the failure: treating "the assertion
// did not hold" as "this must be Wine" would let a real Windows regression
// through as well.
bool running_under_wine() {
  const auto ntdll = GetModuleHandleW(L"ntdll.dll");
  return ntdll && GetProcAddress(ntdll, "wine_get_version") != nullptr;
}
// Exactly what the thread asked for on Windows; at least per-monitor v1 on
// Wine, which does not carry v2 through to the window. The thread's own context
// is still asserted strictly, so the product is still on the hook for setting it.
bool window_awareness_is_expected(HWND window) {
  const auto context = GetWindowDpiAwarenessContext(window);
  if (AreDpiAwarenessContextsEqual(context,
                                   DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2))
    return true;
  return running_under_wine() &&
         AreDpiAwarenessContextsEqual(context,
                                      DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE);
}
} // namespace
int main() {
  try {
    {
      std::optional<CandidatePresentation> value;
      const auto original_dpi = GetThreadDpiAwarenessContext();
      CandidateWindow window([&] { return value; });
      require(AreDpiAwarenessContextsEqual(original_dpi,
                                           GetThreadDpiAwarenessContext()));
      require(window_awareness_is_expected(window.handle()));
      require(!IsWindowVisible(window.handle()));
      require((GetWindowLongPtrW(window.handle(), GWL_EXSTYLE) &
               WS_EX_NOACTIVATE) != 0);
      require(SendMessageW(window.handle(), WM_MOUSEACTIVATE, 0, 0) ==
              MA_NOACTIVATEANDEAT);
      CandidatePresentation frame{};
      frame.lease = {{42, {1, 2, 3}}, 1, 1};
      frame.session = 1;
      frame.generation = 1;
      frame.visible = true;
      frame.preedit = "U4e2d";
      frame.candidates.push_back({1, 1, 0, "中", true, false, {}, {}, false, {}});
      value = frame;
      value->y = invalid_candidate_anchor_y;
      window.refresh();
      require(!IsWindowVisible(window.handle()) && !window.failed());
      value->y = frame.y;
      const auto foreground = GetForegroundWindow();
      window.refresh();
      UpdateWindow(window.handle());
      require(IsWindowVisible(window.handle()) && !window.failed());
      require(GetForegroundWindow() == foreground);
      value->y = invalid_candidate_anchor_y;
      window.refresh();
      require(!IsWindowVisible(window.handle()) && !window.failed());
      value->y = frame.y;
      window.refresh();
      require(IsWindowVisible(window.handle()) && !window.failed());
      // Re-showing invalidates, so flush that paint before asking whether a
      // second refresh dirties anything. Without this the check below reads the
      // re-show's own update region and says nothing about refresh idempotence.
      UpdateWindow(window.handle());
      window.refresh();
      require(!GetUpdateRect(window.handle(), nullptr, FALSE));
      require(AreDpiAwarenessContextsEqual(original_dpi,
                                           GetThreadDpiAwarenessContext()));
      SendMessageW(window.handle(), WM_DPICHANGED, MAKELONG(192, 192), 0);
      window.refresh();
      require(GetUpdateRect(window.handle(), nullptr, FALSE));
      UpdateWindow(window.handle());
      // Display-device lifecycle changes must bypass the unchanged-frame fast
      // path. In production they also discard the cached composition target,
      // so a stale DPI or device is never presented after the next refresh.
      for (const auto &[message, wparam] : {
               std::pair<UINT, WPARAM>{WM_DISPLAYCHANGE, 0},
               std::pair<UINT, WPARAM>{WM_DWMCOMPOSITIONCHANGED, 0},
               std::pair<UINT, WPARAM>{WM_POWERBROADCAST,
                                       PBT_APMRESUMEAUTOMATIC},
           }) {
        SendMessageW(window.handle(), message, wparam, 0);
        window.refresh();
        require(GetUpdateRect(window.handle(), nullptr, FALSE));
        UpdateWindow(window.handle());
        require(!window.failed());
      }
      value.reset();
      InvalidateRect(window.handle(), nullptr, FALSE);
      UpdateWindow(window.handle());
      require(!IsWindowVisible(window.handle())); // Paint rechecks the source.
      value = frame;
      value->visible = false;
      window.refresh();
      require(!IsWindowVisible(window.handle()));
      value = frame;
      value->preedit = std::string(1, static_cast<char>(0xff));
      window.refresh();
      UpdateWindow(window.handle());
      require(window.failed() && !IsWindowVisible(window.handle()));
      value = frame;
      size_t clicks = 0;
      CandidateWindow clickable([&] { return value; },
                                [&](const CandidateClick &click) {
                                  require(click.index == 0 &&
                                          click.session == 1 &&
                                          click.generation == 1);
                                  ++clicks;
                                });
      clickable.refresh();
      UpdateWindow(clickable.handle());
      require(SendMessageW(clickable.handle(), WM_MOUSEACTIVATE, 0, 0) ==
              MA_NOACTIVATE);
      const auto point = first_candidate_point(clickable.handle(), 1);
      SendMessageW(clickable.handle(), WM_LBUTTONDOWN, MK_LBUTTON, point);
      SendMessageW(clickable.handle(), WM_LBUTTONUP, 0, point);
      // Split so a failure names the cause. A dead click with failed()==false
      // means the card never painted, so hit() has no page to test against.
      require(!clickable.failed());
      require(clicks == 1);
      // Cancellation must reject the release even with an unchanged frame.
      for (const UINT cancellation : {WM_MOUSELEAVE, WM_CANCELMODE,
                                       WM_CAPTURECHANGED}) {
        SendMessageW(clickable.handle(), WM_LBUTTONDOWN, MK_LBUTTON, point);
        SendMessageW(clickable.handle(), cancellation, 0, 0);
        SendMessageW(clickable.handle(), WM_LBUTTONUP, 0, point);
        require(clicks == 1 && !clickable.failed());
      }
      SendMessageW(clickable.handle(), WM_LBUTTONDOWN, MK_LBUTTON, point);
      SendMessageW(clickable.handle(), WM_LBUTTONUP, 0, point);
      require(clicks == 2 && !clickable.failed());
      SendMessageW(clickable.handle(), WM_LBUTTONDOWN, MK_LBUTTON, point);
      ++value->generation;
      ++value->candidates[0].generation;
      clickable.refresh();
      UpdateWindow(clickable.handle());
      SendMessageW(clickable.handle(), WM_LBUTTONUP, 0, point);
      require(clicks == 2 && !clickable.failed());
      // 策略隐藏一个可见快照时照样发渲染回执，否则选词键要白等回执超时；同一个 render_serial 只发一次。
      std::vector<uint64_t> receipts;
      value = frame;
      value->render_serial = 7;
      value->y = invalid_candidate_anchor_y;
      CandidateWindow receipted(
          [&] { return value; }, {}, 16, 16, "Segoe UI", {}, std::nullopt,
          false, true, {},
          [&](const CandidatePresentation &shown) {
            receipts.push_back(shown.render_serial);
          });
      receipted.refresh();
      require(!IsWindowVisible(receipted.handle()) && !receipted.failed());
      require(receipts == std::vector<uint64_t>{7});
      receipted.refresh();
      require(receipts.size() == 1);
      // 游戏会话：前台属于客户端进程时，INVALID_Y 兜底到游戏客户区左下部并显示出来。
      HWND game = CreateWindowExW(0, L"STATIC", L"game", WS_POPUP, 100, 100,
                                  800, 600, nullptr, nullptr,
                                  GetModuleHandleW(nullptr), nullptr);
      require(game != nullptr);
      value->game_host = true;
      value->lease.transport.client =
          (static_cast<uint64_t>(GetCurrentProcessId()) << 32) |
          GetCurrentThreadId();
      value->render_serial = 8;
      receipted.set_foreground(game, ForegroundPresentation::Windowed);
      receipted.refresh();
      UpdateWindow(receipted.handle());
      require(IsWindowVisible(receipted.handle()) && !receipted.failed());
      require(receipts.back() == 8);
      require(receipted.take_suppression_changes().empty());
      // 独占全屏下游戏会话策略隐藏，同样发回执，并报出一次抑制变化。
      value->render_serial = 9;
      receipted.set_foreground(game, ForegroundPresentation::ExclusiveFullscreen);
      receipted.refresh();
      require(!IsWindowVisible(receipted.handle()) && receipts.back() == 9);
      auto changes = receipted.take_suppression_changes();
      require(changes.size() == 1 &&
              changes[0].reason == CandidateSuppression::ExclusiveFullscreen &&
              changes[0].active && changes[0].pid == GetCurrentProcessId());
      // 前台不属于客户端进程时既不抑制也不兜底：INVALID_Y 照旧隐藏，回执不重发。
      const auto receipted_count = receipts.size();
      receipted.set_foreground(nullptr,
                               ForegroundPresentation::ExclusiveFullscreen);
      receipted.refresh();
      require(!IsWindowVisible(receipted.handle()) &&
              receipts.size() == receipted_count);
      changes = receipted.take_suppression_changes();
      require(changes.size() == 1 && !changes[0].active);
      // 反应式锁存：在几何全屏前台上第一次弹出后盯住游戏窗口，矩形一变就锁存并策略隐藏。
      value->render_serial = 10;
      receipted.set_foreground(game, ForegroundPresentation::Fullscreen);
      receipted.refresh();
      require(IsWindowVisible(receipted.handle()) && !receipted.failed());
      require(SetWindowPos(game, nullptr, 100, 100, 640, 480,
                           SWP_NOACTIVATE | SWP_NOZORDER));
      // 呈现方式按 HWND 缓存 300ms，触发这一轮拿到的仍是全屏。
      receipted.set_foreground(game, ForegroundPresentation::Fullscreen);
      changes = receipted.take_suppression_changes();
      require(changes.size() == 1 &&
              changes[0].reason == CandidateSuppression::Latched &&
              changes[0].active &&
              std::strcmp(changes[0].cause, "rect") == 0);
      receipted.refresh();
      require(!IsWindowVisible(receipted.handle()));
      // 游戏被这次弹出挤成窗口化，锁存不能因此解除。
      receipted.set_foreground(game, ForegroundPresentation::Windowed);
      receipted.refresh();
      require(!IsWindowVisible(receipted.handle()) &&
              receipted.take_suppression_changes().empty());
      // 回到全屏后用户再切成窗口化，才解除。
      receipted.set_foreground(game, ForegroundPresentation::Fullscreen);
      receipted.refresh();
      require(!IsWindowVisible(receipted.handle()) &&
              receipted.take_suppression_changes().empty());
      receipted.set_foreground(game, ForegroundPresentation::Windowed);
      changes = receipted.take_suppression_changes();
      require(changes.size() == 1 &&
              changes[0].reason == CandidateSuppression::Latched &&
              !changes[0].active &&
              std::strcmp(changes[0].cause, "windowed") == 0);
      // 解除后重新允许盯梢：回到全屏的下一次弹出照常显示，并再盯 2 秒。
      value->render_serial = 11;
      receipted.set_foreground(game, ForegroundPresentation::Fullscreen);
      receipted.refresh();
      require(IsWindowVisible(receipted.handle()) && !receipted.failed());
      require(SetWindowPos(game, nullptr, 100, 100, 800, 600,
                           SWP_NOACTIVATE | SWP_NOZORDER));
      receipted.set_foreground(game, ForegroundPresentation::Fullscreen);
      changes = receipted.take_suppression_changes();
      require(changes.size() == 1 && changes[0].active &&
              std::strcmp(changes[0].cause, "rect") == 0);
      DestroyWindow(game);
    }
    {
      std::optional<ModePresentation> value =
          ModePresentation{{{43, {4, 5, 6}}, 1, 1}, true, true, false};
      FloatingToolbarWindow toolbar([&] { return value; },
                                    [](const ModeClick &) {});
      const auto foreground = GetForegroundWindow();
      toolbar.refresh(true);
      UpdateWindow(toolbar.handle());
      require(IsWindowVisible(toolbar.handle()) && !toolbar.failed());
      require(GetForegroundWindow() == foreground);
      toolbar.refresh(true);
      require(!GetUpdateRect(toolbar.handle(), nullptr, FALSE));
      for (const auto &[message, wparam] : {
               std::pair<UINT, WPARAM>{WM_DPICHANGED, 0},
               std::pair<UINT, WPARAM>{WM_DISPLAYCHANGE, 0},
               std::pair<UINT, WPARAM>{WM_DWMCOMPOSITIONCHANGED, 0},
               std::pair<UINT, WPARAM>{WM_POWERBROADCAST,
                                       PBT_APMRESUMEAUTOMATIC},
           }) {
        const auto result = SendMessageW(toolbar.handle(), message, wparam, 0);
        if (message == WM_POWERBROADCAST)
          require(result == TRUE);
        require(GetUpdateRect(toolbar.handle(), nullptr, FALSE));
        UpdateWindow(toolbar.handle());
        require(IsWindowVisible(toolbar.handle()) && !toolbar.failed());
      }
    }
    {
      TrayMenuWindow tray(
          {}, [](TrayMenuCommand) { return true; },
          [] { return TrayMenuState{}; });
      for (const auto &[message, wparam] : {
               std::pair<UINT, WPARAM>{WM_DPICHANGED, 0},
               std::pair<UINT, WPARAM>{WM_DISPLAYCHANGE, 0},
               std::pair<UINT, WPARAM>{WM_DWMCOMPOSITIONCHANGED, 0},
               std::pair<UINT, WPARAM>{WM_POWERBROADCAST,
                                       PBT_APMRESUMEAUTOMATIC},
           }) {
        require(tray.open(100, 100));
        UpdateWindow(tray.handle());
        require(tray.visible() && !tray.failed());
        const auto result = SendMessageW(tray.handle(), message, wparam, 0);
        if (message == WM_POWERBROADCAST)
          require(result == TRUE);
        require(!tray.visible() && !tray.failed());
      }
    }
    {
      WaveOverlay overlay;
      require(
          overlay.init(GetModuleHandleW(nullptr), [](WaveOverlay::Action) {}));
      const auto foreground = GetForegroundWindow();
      overlay.show();
      MSG message{};
      for (size_t i = 0; i < 16 && !IsWindowVisible(overlay.handle()); ++i) {
        if (PeekMessageW(&message, overlay.handle(), 0, 0, PM_REMOVE)) {
          TranslateMessage(&message);
          DispatchMessageW(&message);
        }
      }
      require(IsWindowVisible(overlay.handle()));
      require(GetForegroundWindow() == foreground);
      UpdateWindow(overlay.handle());
      for (const auto &[system_message, wparam] : {
               std::pair<UINT, WPARAM>{WM_DPICHANGED, 0},
               std::pair<UINT, WPARAM>{WM_DISPLAYCHANGE, 0},
               std::pair<UINT, WPARAM>{WM_DWMCOMPOSITIONCHANGED, 0},
               std::pair<UINT, WPARAM>{WM_SETTINGCHANGE, 0},
               std::pair<UINT, WPARAM>{WM_POWERBROADCAST,
                                       PBT_APMRESUMEAUTOMATIC},
           }) {
        const auto result =
            SendMessageW(overlay.handle(), system_message, wparam, 0);
        if (system_message == WM_POWERBROADCAST)
          require(result == TRUE);
        require(GetUpdateRect(overlay.handle(), nullptr, FALSE));
        UpdateWindow(overlay.handle());
        require(IsWindowVisible(overlay.handle()));
      }
    }
    {
      CandidateFlyoutWindow flyout([](const CandidateMenuChoice &) {});
      for (const auto &[message, wparam] : {
               std::pair<UINT, WPARAM>{WM_DPICHANGED, 0},
               std::pair<UINT, WPARAM>{WM_DISPLAYCHANGE, 0},
               std::pair<UINT, WPARAM>{WM_DWMCOMPOSITIONCHANGED, 0},
               std::pair<UINT, WPARAM>{WM_SETTINGCHANGE, 0},
               std::pair<UINT, WPARAM>{WM_POWERBROADCAST,
                                       PBT_APMRESUMEAUTOMATIC},
           }) {
        // A writable two-character candidate that is not pinned: this case is
        // about the device-edge messages below, so every menu row is live.
        require(flyout.open(100, 100, 2, true, 0));
        UpdateWindow(flyout.handle());
        require(flyout.visible());
        require(GetCapture() == flyout.handle());
        const auto result = SendMessageW(flyout.handle(), message, wparam, 0);
        if (message == WM_POWERBROADCAST)
          require(result == TRUE);
        require(!flyout.visible());
        require(GetCapture() != flyout.handle());
      }
    }
    const auto suffix = std::to_wstring(GetCurrentProcessId()) + L"-" +
                        std::to_wstring(GetTickCount64());
    const auto root = std::filesystem::temp_directory_path() /
                      (L"msime-server-fixture-" + suffix);
    require(std::filesystem::create_directory(root));
    struct Cleanup {
      std::filesystem::path path;
      ~Cleanup() {
        std::error_code error;
        std::filesystem::remove_all(path, error);
      }
    } cleanup{root};
    {
      StateRootLease lease(root);
      bool rejected = false;
      try {
        StateRootLease second(root);
      } catch (...) {
        rejected = true;
      }
      require(rejected);
    }
    {
      StateRootLease reacquired(root);
    }
    require(std::filesystem::exists(root / L".msime-client-server.lock"));
    // 与 Rust 的 `is_symlink` 一致：符号链接、目录联接拒绝，OneDrive 云文件和 App 执行别名这类非名称代理重解析点放行。
    require(is_name_surrogate_reparse_point(FILE_ATTRIBUTE_REPARSE_POINT, IO_REPARSE_TAG_SYMLINK));
    require(is_name_surrogate_reparse_point(FILE_ATTRIBUTE_REPARSE_POINT, IO_REPARSE_TAG_MOUNT_POINT));
    require(!is_name_surrogate_reparse_point(FILE_ATTRIBUTE_REPARSE_POINT, IO_REPARSE_TAG_CLOUD));
    require(!is_name_surrogate_reparse_point(FILE_ATTRIBUTE_REPARSE_POINT, IO_REPARSE_TAG_APPEXECLINK));
    require(!is_name_surrogate_reparse_point(FILE_ATTRIBUTE_DIRECTORY, IO_REPARSE_TAG_SYMLINK));
    {
      // 最后一级：普通文件可信，指向它的文件符号链接不可信（创建符号链接需要开发者模式或权限，失败时跳过）。
      const auto open_leaf = [](const std::filesystem::path &path) {
        return CreateFileW(path.c_str(), FILE_READ_ATTRIBUTES,
                           FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, nullptr,
                           OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
                           nullptr);
      };
      const auto lock_file = root / L".msime-client-server.lock";
      HANDLE plain = open_leaf(lock_file);
      require(plain != INVALID_HANDLE_VALUE);
      require(handle_is_trusted_file(plain));
      CloseHandle(plain);
      const auto leaf_link = root / L"leaf-link.lock";
      if (msime::windows::tests::create_materialized_symlink(
              leaf_link, lock_file, 0)) {
        HANDLE linked_leaf = open_leaf(leaf_link);
        require(linked_leaf != INVALID_HANDLE_VALUE);
        require(!handle_is_trusted_file(linked_leaf));
        CloseHandle(linked_leaf);
        std::filesystem::remove(leaf_link);
      }
      HANDLE directory = CreateFileW(root.c_str(), FILE_READ_ATTRIBUTES,
                                     FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, nullptr,
                                     OPEN_EXISTING, FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS,
                                     nullptr);
      require(directory != INVALID_HANDLE_VALUE);
      require(!handle_is_trusted_file(directory));
      CloseHandle(directory);
    }
    const auto outside = root.parent_path() / (root.filename().wstring() + L"-outside");
    require(std::filesystem::create_directory(outside));
    const auto linked = root.parent_path() / (root.filename().wstring() + L"-linked");
    if (msime::windows::tests::create_materialized_symlink(
            linked, outside, SYMBOLIC_LINK_FLAG_DIRECTORY)) {
      bool rejected = false;
      try {
        reject_reparse_ancestors(linked / L"nested");
      } catch (...) {
        rejected = true;
      }
      require(rejected);
      std::filesystem::remove(linked);
    }
    std::filesystem::remove_all(outside);
    auto host = test_host_options(root);
    WindowsServerOptions options;
    options.pipes.max_clients = 2;
    options.pipes.capabilities = FanyImeProtocol::RequiredCapabilities;
    options.pipes.handshake_timeout = 2000;
    options.write_timeout = 2000;
    for (size_t role = 0; role < 3; ++role)
      options.pipes.names[role] = L"\\\\.\\pipe\\msime-server-fixture-" +
                                  suffix + L"-" + std::to_wstring(role);
    const nlohmann::json launch{{"format_version", 1},
                                {"resources", host.at("resources")},
                                {"state_root", root.u8string()},
                                {"pipe_namespace", "server-fixture"},
                                {"preedit_style", "pinyin"}};
    // Same key-handler factory and background click path as the executable.
    WindowsServer server(
        options, host.dump(),
        preview_key_handler(PreviewConfig::parse(launch.dump())),
        [](const FocusRoute &, const FanyImeNamedpipeData &) { return true; });
    require(!server.candidate_view());
    const uint64_t client =
        (static_cast<uint64_t>(GetCurrentProcessId()) << 32) | 42u;
    ClientPipe replies(options.pipes.names[1]);
    ClientPipe worker(options.pipes.names[2]);
    FanyImePipeHello reverse{};
    reverse.client_id = client;
    reverse.pipe_role = FanyImePipeRole::ToTsf;
    require(
        write_frame(replies.handle, fixture_bytes(reverse), 2000).complete());
    require(read_frame(replies.handle, sizeof(FanyImeNamedpipeDataToTsf), 2000)
                .complete());
    reverse.pipe_role = FanyImePipeRole::ToTsfWorkerThread;
    require(
        write_frame(worker.handle, fixture_bytes(reverse), 2000).complete());
    require(read_frame(worker.handle,
                       sizeof(FanyImeNamedpipeDataToTsfWorkerThread), 2000)
                .complete());
    ClientPipe main(options.pipes.names[0]);
    require(write_frame(main.handle,
                        fixture_bytes(FanyImeProtocol::Hello(client, 1)), 2000)
                .complete());
    auto ready =
        read_frame(replies.handle, sizeof(FanyImeNamedpipeDataToTsf), 2000);
    require(ready.complete() &&
            ready.frame[0] == FanyImeReplyType::ProtocolReady);
    FanyImeNamedpipeData packet{};
    packet.client_id = client;
    packet.event_type = FanyImePipeEventType::ClientActivated;
    packet.request_id = 77;
    require(write_frame(main.handle, fixture_bytes(packet), 2000).complete());
      const auto fence = *focus_ready_bytes(7, 8, 77);
    auto activation =
        read_frame(worker.handle, static_cast<DWORD>(fence.size()), 2000);
    require(activation.complete() && activation.frame == fence);
    packet.event_type = FanyImePipeEventType::KeyEvent;
    packet.request_id = 2;
    // Keep keyboard commit coverage, then compose again for window selection.
    for (char c : std::string("U4e2d U4e2d")) {
      packet.keycode =
          static_cast<uint32_t>(c >= 'a' && c <= 'z' ? c - 'a' + 'A' : c);
      packet.wch = static_cast<FanyImeWireChar>(c == ' ' ? 0 : c);
      packet.modifiers_down = c == 'U' ? 1 : 0;
      require(write_frame(main.handle, fixture_bytes(packet), 2000).complete());
      auto marker =
          read_frame(worker.handle, static_cast<DWORD>(fence.size()), 2000);
      require(marker.complete() && marker.frame == fence);
      auto reply =
          read_frame(replies.handle, sizeof(FanyImeNamedpipeDataToTsf), 2000);
      require(reply.complete());
      if (c == ' ') {
        auto expected = *wire_bytes(candidate_commit(packet.request_id, "中"));
        require(reply.frame ==
                std::vector<uint8_t>(expected.begin(), expected.end()));
      }
      ++packet.request_id;
    }
    std::atomic<SelectionRequestResult> selected{
        SelectionRequestResult::Rejected};
    CandidateClickWorker clicks([&](const CandidateClick &click) {
      selected = server.request_selection(click.lease, click.session,
                                          click.generation, click.index);
    });
    struct ClickShutdown {
      WindowsServer &server;
      CandidateClickWorker &clicks;
      ~ClickShutdown() {
        clicks.request_stop();
        server.request_stop();
        clicks.stop();
      }
    } click_shutdown{server, clicks};
    CandidateWindow candidates(
        [&] { return server.candidate_view(); },
        [&](const CandidateClick &click) { require(clicks.submit(click)); });
    // Pipe receipt precedes queue confirmation. Wait for the confirmed value,
    // not a guessed delay or an independently fabricated window snapshot.
    const auto deadline =
        std::chrono::steady_clock::now() + std::chrono::seconds(2);
    bool painted = false;
    while (std::chrono::steady_clock::now() < deadline) {
      const auto value = server.candidate_view();
      if (value && value->visible && !value->candidates.empty() &&
          value->candidates[0].text == "中") {
        candidates.refresh();
        UpdateWindow(candidates.handle());
        painted = IsWindowVisible(candidates.handle()) && !candidates.failed();
        if (painted)
          break;
      }
      std::this_thread::sleep_for(std::chrono::milliseconds(1));
    }
    require(painted);
    const auto foreground = GetForegroundWindow();
    const auto point = first_candidate_point(candidates.handle(), 1);
    SendMessageW(candidates.handle(), WM_LBUTTONDOWN, MK_LBUTTON, point);
    SendMessageW(candidates.handle(), WM_LBUTTONUP, 0, point);
    const auto committed = read_frame(
        worker.handle, sizeof(FanyImeNamedpipeDataToTsfWorkerThread), 2000);
    require(committed.complete() &&
            committed.frame == ui_complete_selection("中")->worker);
    // Wait for controller confirmation before stopping; the received frame
    // alone is not proof that the background selection transaction completed.
    const auto confirmed_deadline =
        std::chrono::steady_clock::now() + std::chrono::seconds(2);
    while (selected.load() == SelectionRequestResult::Rejected &&
           std::chrono::steady_clock::now() < confirmed_deadline)
      std::this_thread::sleep_for(std::chrono::milliseconds(1));
    require(selected.load() == SelectionRequestResult::Sent &&
            !clicks.failed());
    candidates.refresh();
    require(!IsWindowVisible(candidates.handle()) && !candidates.failed() &&
            GetForegroundWindow() == foreground);
    server.stop(); // Must cancel the now-idle Main reader before joining it.
    clicks.stop();
    server.stop();
    require(!server.candidate_view());
    require(server.failure() == ControllerFailure::None);
    std::cout << "Native isolated Windows server pipeline passed\n";
  } catch (const std::exception &error) {
    // The message names the failing assertion only; never a packet or text.
    std::cerr << "Native Windows server pipeline failed: " << error.what()
              << "\n";
    return 1;
  } catch (...) {
    std::cerr << "Native Windows server pipeline failed\n";
    return 1;
  }
}

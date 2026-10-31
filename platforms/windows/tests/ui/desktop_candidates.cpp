#include "DesktopCandidates.h"
#include "ClientDesktop.h"
#include "TypingEffectSignal.h"
#include "msime_edition.h"
#include <atomic>
#include <cassert>
#include <cstdio>
#include <mutex>
#include <thread>

using namespace msime::windows;
namespace {
constexpr wchar_t candidate_class[] = L"MSIME.Client.Preview.Candidates" MSIME_EDITION_NAME_SUFFIX;
struct Probe { unsigned count = 0, total = 0; HWND candidate = nullptr; };
// x86 的窗口枚举回调使用 stdcall，不能交给默认调用约定的 lambda。
BOOL CALLBACK CountCandidateWindows(HWND handle, LPARAM data) {
  wchar_t name[128]{};
  if (GetClassNameW(handle, name, 128) && !lstrcmpW(name, candidate_class)) {
    auto &value = *reinterpret_cast<Probe *>(data);
    ++value.total;
    if (GetWindowLongPtrW(handle, GWL_STYLE) & WS_VISIBLE) {
      ++value.count;
      value.candidate = handle;
    }
  }
  return TRUE;
}
struct Client {
  HDESK desktop;
  std::atomic<DWORD> tid{0};
  std::atomic<bool> stopping{false}, wheel{false}, paint_requested{false};
  std::atomic<bool> desktop_visible{false};
  std::atomic<unsigned> visible{0}, windows{0};
  std::thread thread;
  explicit Client(const std::wstring &name) {
    desktop = CreateDesktopW(name.c_str(), nullptr, nullptr, 0,
        DESKTOP_CREATEWINDOW | DESKTOP_CREATEMENU | DESKTOP_READOBJECTS |
            DESKTOP_WRITEOBJECTS | DESKTOP_ENUMERATE, nullptr);
    assert(desktop);
    thread = std::thread([this] {
      assert(SetThreadDesktop(desktop));
      desktop_visible.store(IsWindowVisible(GetDesktopWindow()) != FALSE);
      HWND window = CreateWindowExW(0, L"STATIC", L"", WS_POPUP, 0, 0, 1, 1,
                                    nullptr, nullptr, GetModuleHandleW(nullptr), nullptr);
      assert(window);
      tid.store(GetCurrentThreadId());
      while (!stopping.load()) {
        Probe probe;
        assert(EnumDesktopWindows(desktop, CountCandidateWindows, reinterpret_cast<LPARAM>(&probe)));
        windows.store(probe.total);
        visible.store(probe.count);
        // 未切换为输入桌面的私有桌面不会自动绘制；显式发绘制消息只验证线程归属与绘制资源。
        if (probe.candidate && paint_requested.exchange(false))
          PostMessageW(probe.candidate, WM_PAINT, 0, 0);
        if (wheel.exchange(false) && probe.candidate)
          PostMessageW(probe.candidate, WM_MOUSEWHEEL, MAKEWPARAM(0, static_cast<WORD>(-WHEEL_DELTA)), 0);
        MSG message{};
        while (PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE)) {
          TranslateMessage(&message);
          DispatchMessageW(&message);
        }
        Sleep(10);
      }
      assert(DestroyWindow(window));
    });
  }
  ~Client() {
    stopping.store(true);
    thread.join();
    assert(CloseDesktop(desktop));
  }
};
template <class Check> void wait_for(Check check, DesktopCandidates &router, const char *stage,
    const std::function<void()> &diagnose = {}) {
  const auto deadline = GetTickCount64() + 15000;
  while (!check() && GetTickCount64() < deadline && !router.failed()) Sleep(10);
  for (const auto &line : router.take_diagnostics()) std::fprintf(stderr, "%s\n", line.c_str());
  if (router.failed() || !check()) {
    std::fprintf(stderr, "desktop UI wait failed: %s\n", stage);
    if (diagnose) diagnose();
  }
  assert(!router.failed() && check());
}
CandidatePresentation frame(const Client &client, uint64_t epoch) {
  CandidatePresentation result{};
  result.lease = {{(uint64_t{GetCurrentProcessId()} << 32) | client.tid.load(), {1, 2, 3}, 0}, epoch, 1};
  result.session = 1;
  result.generation = epoch;
  result.render_serial = epoch;
  result.visible = true;
  result.x = 100;
  result.y = 100;
  result.preedit = "synthetic";
  result.page_count = 3;
  result.page_size = 1;
  PresentationCandidate candidate{};
  candidate.session = 1;
  candidate.generation = epoch;
  candidate.text = "synthetic";
  candidate.highlighted = true;
  result.candidates.push_back(std::move(candidate));
  return result;
}
void signal_owner() {
  auto &signal = TypingEffectSignal::instance();
  const auto make_window = [] {
    return CreateWindowExW(0, L"STATIC", L"", 0, 0, 0, 0, 0, HWND_MESSAGE,
                           nullptr, GetModuleHandleW(nullptr), nullptr);
  };
  const auto first = make_window(), second = make_window();
  assert(first && second);
  signal.attach(first);
  signal.publish(7);
  assert(!signal.take(second));
  assert(signal.take(first) == 7);
  signal.attach(second);
  signal.detach(first);
  signal.publish(9);
  assert(!signal.take(first));
  assert(signal.take(second) == 9);
  signal.detach(second);
  assert(DestroyWindow(first));
  assert(DestroyWindow(second));
}
} // namespace
int main() {
  signal_owner();
  const auto token = L"MSIME9439UI" + std::to_wstring(GetCurrentProcessId()) + L"x" + std::to_wstring(GetTickCount64());
  Client first(token + L"A"), second(token + L"B");
  std::atomic<uint64_t> current{0}, rendered{0}, paged{0};
  std::atomic<bool> busy{false}, wrong_desktop{false};
  std::mutex mutex;
  std::optional<CandidatePresentation> source;
  DesktopCandidates router({
      [&]() -> std::optional<CandidatePresentation> {
        if (busy.load()) return std::nullopt;
        std::lock_guard lock(mutex);
        return source;
      }, {},
      [&](const CandidatePage &page) { paged.store(page.lease.epoch); },
      [&](const CandidatePresentation &value) {
        const auto expected = static_cast<DWORD>(value.lease.transport.client) == first.tid.load()
            ? first.desktop : second.desktop;
        if (!same_desktop_object(GetThreadDesktop(GetCurrentThreadId()), expected)) wrong_desktop.store(true);
        rendered.store(value.render_serial);
      }, {},
      [&](const FocusLease &lease) { return current.load() == lease.epoch; }});
  DesktopCandidateSettings settings;
  settings.mouse_wheel = true;
  settings.palette = candidate_native_palette(false);
  router.start(settings);
  wait_for([&] { return first.tid.load() && second.tid.load(); }, router, "client readiness");
  auto show = [&](Client &client, uint64_t epoch) {
    std::lock_guard lock(mutex);
    source = frame(client, epoch);
    client.paint_requested.store(true);
    current.store(epoch);
  };
  show(first, 1);
  wait_for([&] { return first.visible.load() == 1 && rendered.load() == 1; }, router, "first desktop rendering", [&] {
    std::fprintf(stderr, "visible=%u,%u rendered=%llu paged=%llu wrong=%d\n",
        first.visible.load(), second.visible.load(),
        static_cast<unsigned long long>(rendered.load()),
        static_cast<unsigned long long>(paged.load()), wrong_desktop.load());
  });
  show(second, 2);
  wait_for([&] { return !first.visible.load() && second.visible.load() == 1 && rendered.load() == 2; }, router, "second desktop rendering");
  if (second.desktop_visible.load()) {
    second.wheel.store(true);
    wait_for([&] { return paged.load() == 2; }, router, "second desktop paging");
  } else {
    std::puts("desktop candidates: pointer paging skipped on inactive private desktop");
  }
  // 读锁争用保留当前帧；失焦时即使一直争用也必须收起，不能把旧帧留在桌面上。
  busy.store(true);
  Sleep(120);
  assert(second.visible.load() == 1);
  current.store(0);
  wait_for([&] { return !first.visible.load() && !second.visible.load(); }, router, "focus loss");
  busy.store(false);
  show(first, 3);
  wait_for([&] { return first.visible.load() == 1 && !second.visible.load() && rendered.load() == 3; }, router, "return to first desktop");
  // 无法认证的新客户端不能沿用原桌面的帧。
  {
    std::lock_guard lock(mutex);
    source = frame(first, 4);
    source->lease.transport.client = uint64_t{GetCurrentProcessId()} << 32;
    current.store(4);
  }
  wait_for([&] { return !first.visible.load() && !second.visible.load(); }, router, "unresolved owner");
  show(first, 5);
  wait_for([&] { return first.visible.load() == 1 && rendered.load() == 5; }, router, "return after unresolved owner");
  router.stop();
  wait_for([&] { return !first.windows.load() && !second.windows.load(); }, router, "teardown");
  assert(!wrong_desktop.load());
  std::puts("desktop candidates: routed paint, show requests, contention, focus loss and teardown passed");
  return 0;
}

#include "DesktopCandidates.h"
#include "ClientDesktop.h"
#include "ShuangpinKeymapWindow.h"
#include <atomic>
#include <mutex>
#include <stdexcept>
#include <thread>

namespace msime::windows {
namespace {
struct Apartment {
  Apartment() {
    const auto result = CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
    if (FAILED(result)) throw std::runtime_error("Candidate desktop apartment unavailable");
  }
  ~Apartment() { CoUninitialize(); }
};
bool same_lease(const FocusLease &first, const FocusLease &second) {
  return first.epoch == second.epoch && first.token == second.token &&
         same_ticket(first.transport, second.transport);
}
std::string suppression_line(const CandidateSuppressionChange &change) {
  std::string line = change.reason == CandidateSuppression::ExclusiveFullscreen
      ? "Candidate exclusive-fullscreen suppression " : "Candidate reactive latch ";
  line += change.active ? "on" : "off";
  line += " pid=" + std::to_string(change.pid);
  if (change.cause) line += std::string(" cause=") + change.cause;
  return line;
}
std::optional<DWORD> window_band(HWND window) {
  using Query = BOOL(WINAPI *)(HWND, DWORD *);
  const auto query = reinterpret_cast<Query>(reinterpret_cast<void *>(
      GetProcAddress(GetModuleHandleW(L"user32.dll"), "GetWindowBand")));
  DWORD band = 0;
  return query && query(window, &band) ? std::optional<DWORD>{band} : std::nullopt;
}
void apply_appearance(CandidateWindow &window, ShuangpinKeymapWindow *keymap,
                      const DesktopCandidateSettings &settings) {
  window.set_theme_palette(settings.palette);
  window.set_skin_min_width(settings.min_width);
  window.set_skin_decoration(settings.decoration);
  window.set_skin_background(settings.background);
  window.set_skin_corner_radius(settings.skin_radius);
  window.set_skin_layout(settings.skin_layout);
  window.set_skin_font_family(settings.skin_font);
  window.set_skin_animation(settings.animation);
  window.set_skin_canvas(settings.canvas);
  if (keymap) keymap->set_palette(settings.keymap_palette);
}
} // namespace

struct DesktopCandidates::State {
  struct Input {
    std::optional<CandidatePresentation> view;
    std::optional<FocusLease> lease;
    bool routed = false;
  };
  struct Surface {
    State &owner;
    std::shared_ptr<ClientDesktop> desktop;
    std::shared_ptr<const Input> input = std::make_shared<const Input>();
    std::atomic<bool> stopping{false};
    // 协调线程发布新快照后叫醒本线程，不等 16 毫秒的轮询（#9519）。自动复位。
    HANDLE wake_event = nullptr;
    std::thread thread;
    // 失焦时关闭候选菜单；句柄登记和销毁在同一把锁下，不向失效句柄发消息。
    std::mutex window_mutex;
    HWND window = nullptr;
    Surface(State &state, std::shared_ptr<ClientDesktop> target)
        : owner(state), desktop(std::move(target)) {
      wake_event = CreateEventW(nullptr, FALSE, FALSE, nullptr);
      if (!wake_event) throw std::runtime_error("Candidate desktop wake event unavailable");
      thread = std::thread([this] { run(); });
    }
    ~Surface() {
      stopping.store(true, std::memory_order_release);
      cancel_menu();
      wake();
      if (thread.joinable()) thread.join();
      CloseHandle(wake_event);
    }
    void wake() { SetEvent(wake_event); }
    void cancel_menu() {
      std::lock_guard lock(window_mutex);
      if (window) PostMessageW(window, WM_CANCELMODE, 0, 0);
    }
    bool lease_current(const FocusLease &lease) const {
      const auto value = std::atomic_load(&input);
      return value->routed && value->lease && same_lease(*value->lease, lease) &&
             owner.callbacks.lease_current(lease);
    }
    void publish(std::shared_ptr<const Input> next) {
      const auto previous = std::atomic_exchange(&input, std::move(next));
      const auto value = std::atomic_load(&input);
      if (previous->routed && (!value->routed || !previous->lease || !value->lease ||
                              !same_lease(*previous->lease, *value->lease) ||
                              (value->view && !value->view->visible &&
                               (!previous->view || previous->view->visible))))
        cancel_menu();
    }
    void run() noexcept {
      try {
        // COM、Direct2D、窗口类和所有关联浮层都在绑定之后初始化。
        if (!SetThreadDesktop(desktop->handle())) {
          owner.failed.store(true, std::memory_order_release);
          owner.diagnose("Candidate desktop binding failed, error=" + std::to_string(GetLastError()));
          return;
        }
        Apartment apartment;
        auto settings = std::atomic_load(&owner.published_settings);
        std::optional<ShuangpinKeymapFrame> keymap_frame;
        // 浮层比候选窗先构造、后销毁，候选窗的回调始终引用活着的同线程对象。
        std::optional<TypingEffectOverlay> overlay;
        std::optional<ShuangpinKeymapWindow> keymap;
        try { overlay.emplace(); }
        catch (...) { owner.diagnose("Typing effect overlay unavailable; continuing without it"); }
        try { keymap.emplace(); }
        catch (...) { owner.diagnose("Shuangpin keymap unavailable; continuing without it"); }
        CandidateWindow candidates(
            [&] {
              const auto value = std::atomic_load(&input);
              keymap_frame.reset();
              if (!value->routed || !value->view || !value->lease ||
                  !owner.callbacks.lease_current(*value->lease))
                return std::optional<CandidatePresentation>{};
              auto view = with_wubi_code_hints(*value->view, settings->layout.wubi_code_hint);
              if (view.visible && view.shuangpin_keymap)
                keymap_frame = ShuangpinKeymapFrame{*view.shuangpin_keymap, view.x, view.y};
              return std::optional<CandidatePresentation>{std::move(view)};
            }, owner.callbacks.click, settings->fonts.size, settings->fonts.preedit_size,
            settings->fonts.family, settings->fonts.fallback, std::nullopt,
            settings->layout.horizontal, settings->layout.show_preedit,
            owner.callbacks.page, owner.callbacks.rendered, settings->mouse_wheel, false);
        // 必须先撤掉登记，再由 candidates 的析构销毁 HWND。
        struct WindowRegistration {
          Surface &surface;
          ~WindowRegistration() {
            std::lock_guard lock(surface.window_mutex);
            surface.window = nullptr;
          }
        } registration{*this};
        {
          std::lock_guard lock(window_mutex);
          window = candidates.handle();
        }
        candidates.set_expansion_handler(owner.callbacks.expansion);
        candidates.set_lease_check([this](const FocusLease &lease) { return lease_current(lease); });
        if (overlay)
          candidates.set_typing_effect_presenter([&](const TypingEffectPresentation &value) {
            return overlay->present(value);
          });
        bool first = true, overlay_reported = false, keymap_reported = false, band_reported = false;
        std::shared_ptr<const DesktopCandidateSettings> applied;
        while (!stopping.load(std::memory_order_acquire)) {
          settings = std::atomic_load(&owner.published_settings);
          const auto value = std::atomic_load(&input);
          const bool active = value->routed && value->lease &&
                              owner.callbacks.lease_current(*value->lease);
          candidates.set_typing_effect_active(active);
          if (!active && overlay) overlay->settle();
          if (first || !(settings->fonts == applied->fonts)) candidates.set_fonts(settings->fonts);
          if (first || settings->style != applied->style) candidates.set_style(settings->style);
          candidates.set_layout(settings->layout);
          candidates.set_follow_cursor(settings->follow_cursor);
          candidates.set_mouse_wheel(settings->mouse_wheel);
          if (first || settings->appearance_revision != applied->appearance_revision)
            apply_appearance(candidates, keymap ? &*keymap : nullptr, *settings);
          if (first || settings->image_revision != applied->image_revision)
            candidates.invalidate_skin_images();
          applied = settings;
          first = false;
          MSG message{};
          for (size_t count = 0; count < 64 && PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE); ++count) {
            if (message.message == WM_QUIT) { stopping.store(true); break; }
            TranslateMessage(&message);
            DispatchMessageW(&message);
          }
          if (stopping.load()) break;
          // 前台与全屏缓存属于该桌面的 UI 线程，不能沿用 Server 主桌面的 HWND。
          const auto foreground = GetForegroundWindow();
          const auto presentation = foreground_presentation(foreground, GetTickCount64());
          candidates.set_foreground(foreground, presentation);
          candidates.refresh();
          candidates.keep_on_top();
          if (keymap) {
            keymap->update(settings->keymap_enabled ? keymap_frame : std::nullopt, candidates);
            if (keymap->failed() && !keymap_reported) {
              keymap_reported = true;
              owner.diagnose(component_failure("Shuangpin keymap", keymap->failure_site()));
            }
          }
          if (overlay && overlay->failed() && !overlay_reported) {
            overlay_reported = true;
            owner.diagnose(component_failure("Typing effect overlay", overlay->failure_site()));
          }
          for (const auto &change : candidates.take_suppression_changes())
            owner.diagnose(suppression_line(change));
          if (!band_reported && presentation != ForegroundPresentation::Windowed &&
              IsWindowVisible(candidates.handle())) {
            band_reported = true;
            const auto band = window_band(candidates.handle());
            owner.diagnose("Candidate window band over fullscreen: " +
                           (band ? std::to_string(*band) : std::string("unavailable")));
          }
          if (candidates.failed()) {
            owner.failed.store(true, std::memory_order_release);
            owner.diagnose(component_failure("Candidate window", candidates.failure_site()));
            break;
          }
          if (MsgWaitForMultipleObjectsEx(1, &wake_event, 16, QS_ALLINPUT, MWMO_INPUTAVAILABLE) == WAIT_FAILED)
            throw std::runtime_error("Candidate desktop message wait failed");
        }
        candidates.set_typing_effect_active(false);
        candidates.hide();
        if (keymap) keymap->hide();
        if (overlay) overlay->settle();
      } catch (...) {
        owner.failed.store(true, std::memory_order_release);
        owner.diagnose("Candidate desktop UI failed, error=" + std::to_string(GetLastError()));
      }
    }
  };
  explicit State(DesktopCandidateCallbacks value) : callbacks(std::move(value)) {
    stop_event = CreateEventW(nullptr, TRUE, FALSE, nullptr);
    if (!stop_event) throw std::runtime_error("Candidate desktop stop event unavailable");
    wake_event = CreateEventW(nullptr, FALSE, FALSE, nullptr);
    if (!wake_event) {
      CloseHandle(stop_event);
      throw std::runtime_error("Candidate desktop wake event unavailable");
    }
  }
  ~State() { stop(); CloseHandle(wake_event); CloseHandle(stop_event); }
  void diagnose(std::string line) {
    std::lock_guard lock(diagnostic_mutex);
    if (diagnostics.size() < 64) diagnostics.push_back(std::move(line));
  }
  void stop() {
    SetEvent(stop_event);
    if (coordinator.joinable()) coordinator.join();
  }
  void run() noexcept {
    try {
      std::vector<std::unique_ptr<Surface>> surfaces;
      std::shared_ptr<ClientDesktop> resolved;
      std::optional<PipeTicket> transport;
      uint64_t retry_at = 0, diagnosed_at = 0;
      // 上一轮等待是被 wake() 叫醒的：这一轮读到的快照立即交给各桌面线程，并叫醒它们绘制。
      bool woken = false;
      while (!failed.load(std::memory_order_acquire) &&
             WaitForSingleObject(stop_event, 0) == WAIT_TIMEOUT) {
        auto view = callbacks.read();
        Surface *target = nullptr;
        if (view && view->lease.epoch && callbacks.lease_current(view->lease)) {
          if (!transport || !same_ticket(*transport, view->lease.transport)) {
            resolved.reset();
            transport = view->lease.transport;
            retry_at = 0;
          }
          if (resolved && !resolved->current()) resolved.reset();
          if (!resolved && GetTickCount64() >= retry_at) {
            retry_at = GetTickCount64() + 500;
            if (auto desktop = ClientDesktop::resolve(view->lease.transport))
              resolved = std::make_shared<ClientDesktop>(std::move(*desktop));
          }
          if (resolved) {
            for (const auto &surface : surfaces)
              if (same_desktop_object(surface->desktop->handle(), resolved->handle())) {
                target = surface.get();
                break;
              }
            // Sysinternals Desktops 最多四个桌面；限制对象数，拒绝由输入客户端无限制造 UI 线程。
            if (!target && surfaces.size() < 8) {
              surfaces.push_back(std::make_unique<Surface>(*this, resolved));
              target = surfaces.back().get();
            }
          }
          if (!target && GetTickCount64() >= diagnosed_at) {
            diagnosed_at = GetTickCount64() + 5000;
            diagnose("Candidate desktop routing unavailable");
          }
        }
        for (const auto &surface : surfaces) {
          auto input = std::make_shared<Input>();
          if (view) {
            input->routed = surface.get() == target;
            if (input->routed) { input->view = *view; input->lease = view->lease; }
          } else {
            // 快照锁争用时保留归属，仅把读取结果传为空；CandidateWindow 仍按原租约规则保帧。
            const auto previous = std::atomic_load(&surface->input);
            input->lease = previous->lease;
            input->routed = previous->routed && input->lease &&
                            callbacks.lease_current(*input->lease) && resolved && resolved->current();
          }
          surface->publish(std::move(input));
          if (woken) surface->wake();
        }
        const HANDLE waits[] = {stop_event, wake_event};
        woken = WaitForMultipleObjects(2, waits, FALSE, 16) == WAIT_OBJECT_0 + 1;
      }
      // 所有窗口先在所属线程销毁，之后 Server 才能停止候选回调依赖的服务。
      for (const auto &surface : surfaces) { surface->stopping.store(true); surface->cancel_menu(); }
      surfaces.clear();
    } catch (...) {
      failed.store(true, std::memory_order_release);
      diagnose("Candidate desktop coordinator failed");
    }
  }
  DesktopCandidateCallbacks callbacks;
  std::shared_ptr<const DesktopCandidateSettings> published_settings;
  HANDLE stop_event = nullptr;
  HANDLE wake_event = nullptr;
  std::thread coordinator;
  std::atomic<bool> failed{false};
  std::mutex diagnostic_mutex;
  std::vector<std::string> diagnostics;
};

DesktopCandidates::DesktopCandidates(DesktopCandidateCallbacks callbacks)
    : state_(std::make_unique<State>(std::move(callbacks))) {}
DesktopCandidates::~DesktopCandidates() = default;
void DesktopCandidates::start(DesktopCandidateSettings settings) {
  publish(std::move(settings));
  state_->coordinator = std::thread([this] { state_->run(); });
}
void DesktopCandidates::publish(DesktopCandidateSettings settings) {
  std::atomic_store(&state_->published_settings,
                    std::make_shared<const DesktopCandidateSettings>(std::move(settings)));
}
void DesktopCandidates::stop() { state_->stop(); }
void DesktopCandidates::wake() { SetEvent(state_->wake_event); }
bool DesktopCandidates::failed() const { return state_->failed.load(std::memory_order_acquire); }
std::vector<std::string> DesktopCandidates::take_diagnostics() {
  std::lock_guard lock(state_->diagnostic_mutex);
  return std::exchange(state_->diagnostics, {});
}
} // namespace msime::windows

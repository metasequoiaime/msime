#!/usr/bin/env python3
"""用合成宿主执行游戏候选同步、Caps Lock 时序与基岩版定位函数。"""
from __future__ import annotations
import pathlib
import shutil
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent


def function(source: str, signature: str) -> str:
    start = source.index(signature)
    begin = source.index("{", start)
    depth = 1
    end = begin + 1
    while depth:
        if source[end] == "{":
            depth += 1
        elif source[end] == "}":
            depth -= 1
        end += 1
    return source[start:end]


HARNESS = r'''
#include <atomic>
#include <cassert>
#include <cwchar>
#include <string>
#include <optional>
#include <vector>
#include <stdexcept>
#include "uiless_page.h"
#define _In_
using HWND = void*;
struct POINT { long x, y; };
struct RECT { long left, top, right, bottom; };
struct MONITORINFO { unsigned long cbSize; RECT rcMonitor{}, rcWork{}; };
constexpr int GA_ROOT = 1, MONITOR_DEFAULTTONEAREST = 2, VK_CAPITAL = 20, WM_ApplyCapsLockEdge = 33;
bool postOk = true, posted = false, toggle = false;
int conversions = 0;
bool PostMessage(HWND, int message, int, int) { assert(message == WM_ApplyCapsLockEdge); posted = true; return postOk; }
int GetKeyState(int key) { assert(key == VK_CAPITAL); return toggle ? 1 : 0; }
HWND GetAncestor(HWND window, int) { return window; }
int MonitorFromWindow(HWND, int) { return 1; }
bool GetMonitorInfoW(int, MONITORINFO* info) { info->rcWork = {-3840, -500, 0, 1588}; return true; }
namespace msime::windows::compat {
bool physical_point(HWND, POINT* point) { ++conversions; point->x *= 2; point->y *= 2; return true; }
}
bool SUCCEEDED(int result) { return result >= 0; }
struct ITfContextView { int GetWnd(HWND* window) { *window = reinterpret_cast<HWND>(1); return 0; } };
namespace Global {
std::atomic<bool> CapsLockEnabled{false};
std::wstring current_process_name;
bool IsCandidatePinnedTopLeftProcess(const std::wstring& name) { return name == L"Minecraft.Windows.exe"; }
}
class CMetasequoiaIME {
public:
 HWND _msgWndHandle = reinterpret_cast<HWND>(1);
 bool armedBefore = false;
 int refreshes = 0;
 void _ArmCapsLockTap(bool before) { armedBefore = before; }
 void _RequestLanguageBarCapsIconRefresh() { ++refreshes; }
 void _ApplyCapsLockEdge(bool enabled);
 void _QueueCapsLockEdge();
};
class CCandidateListUIPresenter {
public:
 struct Host { bool ready = false; bool valid() const { return ready; } };
 struct Engine { Host host; Host* GetHostEngineAdapter() { return &host; } };
 struct Service { Engine engine; Engine* GetCompositionProcessorEngine() { return &engine; } };
 Service* _pTextService = nullptr;
 struct State { int clears = 0; void Clear() { ++clears; } } _candidateState;
 std::wstring _lastUiLessCandidatePage;
 std::uint64_t _lastUiLessGeneration = 0;
 std::wstring page;
 int selected = 0;
 void _ApplyUiLessCandidatePage(const std::wstring& text, int selection) { page = text; selected = selection; }
 bool _ApplyUiLessPayload(const FanyImeUiLessPage::Payload& payload);
 void _ClearList();
};
namespace nlohmann { struct json {}; }
struct FocusLease { int transport = 1; };
enum class KeyEventSendResult { Sent, Failed };
namespace FanyImePipeRole { constexpr int ToTsfWorkerThread = 3; }
class SessionController {
public:
 struct Sync {
  std::optional<std::vector<unsigned char>> frame = std::vector<unsigned char>{1};
  auto updated(const FocusLease&, const nlohmann::json&) { return frame; }
 } uiless_candidates_;
 struct Gate {
  bool active = true;
  int invalidated = 0;
  template <class Action> void with_active(const FocusLease&, Action action) { if (active) action(); }
  void invalidate(int) { ++invalidated; }
 } focus_;
 struct Transport {
  bool valid = true, throws = false;
  int sent = 0, closed = 0;
  KeyEventSendResult result = KeyEventSendResult::Sent;
  bool current(int) { return valid; }
  KeyEventSendResult send(int, int, const std::vector<unsigned char>&) { ++sent; if (throws) throw std::runtime_error("合成传输异常"); return result; }
  void close(int) { ++closed; }
 } transport_;
 bool stopping_ = false;
 void push_uiless_candidates(const FocusLease&, const nlohmann::json&);
};
@FUNCTIONS@
int main() {
 for (int scenario = 0; scenario < 9; ++scenario) {
  SessionController controller;
  if (scenario == 1 || scenario == 2) controller.uiless_candidates_.frame = std::vector<unsigned char>{};
  if (scenario == 2) controller.focus_.active = false;
  if (scenario == 3) controller.transport_.result = KeyEventSendResult::Failed;
  if (scenario == 4) controller.transport_.throws = true;
  if (scenario == 5) controller.stopping_ = true;
  if (scenario == 6) controller.uiless_candidates_.frame.reset();
  if (scenario == 7) controller.transport_.valid = false;
  if (scenario == 8) controller.focus_.active = false;
  controller.push_uiless_candidates(FocusLease{}, nlohmann::json{});
  const bool shouldClose = scenario == 1 || scenario == 3 || scenario == 4;
  assert(controller.transport_.closed == (shouldClose ? 1 : 0));
  assert(controller.focus_.invalidated == (shouldClose ? 1 : 0));
  assert(controller.transport_.sent == ((scenario == 0 || scenario == 3 || scenario == 4) ? 1 : 0));
 }
 CCandidateListUIPresenter presenter;
 assert(presenter._ApplyUiLessPayload(FanyImeUiLessPage::Parse(L"ni\t你,拟\t0\t42")));
 assert(presenter.page == L"你,拟" && presenter._lastUiLessGeneration == 42);
 assert(!presenter._ApplyUiLessPayload(FanyImeUiLessPage::Parse(L"ni\t旧页\t0\t41")));
 assert(presenter.page == L"你,拟");
 assert(presenter._ApplyUiLessPayload(FanyImeUiLessPage::Parse(L"ni\t你,拟\t1\t42")));
 assert(presenter.selected == 1);
 assert(!presenter._ApplyUiLessPayload(FanyImeUiLessPage::Parse(L"ni\t错误\t0\t-1")));
 assert(presenter.page == L"你,拟");
 assert(presenter._ApplyUiLessPayload(FanyImeUiLessPage::Parse(L"ni\t旧协议\t0")));
 assert(presenter.page == L"旧协议" && presenter._lastUiLessGeneration == 42);
 presenter._lastUiLessCandidatePage = L"旧协议";
 presenter._ClearList();
 assert(presenter._candidateState.clears == 1 && presenter._lastUiLessCandidatePage.empty());
 assert(presenter._lastUiLessGeneration == 42);
 assert(!presenter._ApplyUiLessPayload(FanyImeUiLessPage::Parse(L"ni\t旧页\t0\t41")));
 CCandidateListUIPresenter::Service service;
 presenter._pTextService = &service;
 service.engine.host.ready = true;
 assert(!presenter._ApplyUiLessPayload(FanyImeUiLessPage::Parse(L"ni\tServer页\t0\t43")));
 assert(presenter.page == L"旧协议" && presenter._lastUiLessGeneration == 42);
 service.engine.host.ready = false;
 assert(presenter._ApplyUiLessPayload(FanyImeUiLessPage::Parse(L"ni\tServer页\t0\t43")));
 CMetasequoiaIME ime;
 // 原生 TSF 在切换前询问，IMM 在切换后询问且不调用 OnKeyDown；两者最终状态相同。
 for (bool imm : {false, true}) {
  for (bool before : {false, true}) {
   toggle = imm ? !before : before;
   posted = false;
   const int refreshes = ime.refreshes;
   ime._QueueCapsLockEdge();
   assert(posted && ime.refreshes == refreshes);
   toggle = !before;
   ime._ApplyCapsLockEdge((GetKeyState(VK_CAPITAL) & 1) != 0);
   assert(Global::CapsLockEnabled.load() == !before);
   assert(ime.armedBefore == before && ime.refreshes == refreshes + 1);
  }
 }
 // 没有消息窗口时仍保留原来的预测回退。
 ime._msgWndHandle = 0;
 toggle = false;
 ime._QueueCapsLockEdge();
 assert(Global::CapsLockEnabled.load());
 ime._msgWndHandle = reinterpret_cast<HWND>(1);
 postOk = false;
 toggle = true;
 ime._QueueCapsLockEdge();
 assert(!Global::CapsLockEnabled.load() && ime.armedBefore);
 ITfContextView context;
 Global::current_process_name = L"Minecraft.Windows.exe";
 // 首键右下角和组字后系统组合窗的位置都归一到同一显示器工作区，不二次缩放。
 for (RECT extent : {RECT{3839, 2080, 3840, 2087}, RECT{13, 20, 30, 37}}) {
  const auto anchor = GetPhysicalTextAnchor(&context, extent);
  assert(anchor.x == -3840 && anchor.y == -500 && conversions == 0);
 }
 Global::current_process_name = L"javaw.exe";
 const auto normal = GetPhysicalTextAnchor(&context, RECT{13, 20, 30, 37});
 assert(normal.x == 26 && normal.y == 74 && conversions == 1);
}
'''


def main() -> int:
    compiler = shutil.which("c++")
    if not compiler:
        print("skipped: no host C++ compiler")
        return 0
    sink = (ROOT / "platforms/windows/tsf/Key/KeyEventSink.cpp").read_text()
    layout = (ROOT / "platforms/windows/tsf/Tf/TfTextLayoutSink.cpp").read_text()
    ime = (ROOT / "platforms/windows/tsf/IME/MetasequoiaIME.cpp").read_text()
    test_down = function(sink, "STDAPI CMetasequoiaIME::OnTestKeyDown(")
    assert "_QueueCapsLockEdge();" in test_down
    assert "Global::CapsLockEnabled.store(!lockWasOn" not in test_down
    assert "caseWM_ApplyCapsLockEdge:pIME->_ApplyCapsLockEdge((GetKeyState(VK_CAPITAL)&0x0001)!=0);" in "".join(ime.split())
    methods = [function(sink, f"void CMetasequoiaIME::{name}(")
               for name in ("_ApplyCapsLockEdge", "_QueueCapsLockEdge")]
    methods.append(function(layout, "POINT GetPhysicalTextAnchor("))
    presenter = (ROOT / "platforms/windows/tsf/UI/CandidateListUIPresenter.cpp").read_text()
    methods.append(function(presenter, "bool CCandidateListUIPresenter::_ApplyUiLessPayload("))
    methods.append(function(presenter, "void CCandidateListUIPresenter::_ClearList("))
    controller = (ROOT / "platforms/windows/src/ipc/SessionController.cpp").read_text()
    methods.append(function(controller, "void SessionController::push_uiless_candidates("))
    paging = ime[ime.index("case WM_AsyncServerCandidateKey: {", ime.index("LRESULT CALLBACK CMetasequoiaIME_WindowProc")):]
    assert "UiLessComposition" in paging and "_ApplyUiLessPayload" in paging

    with tempfile.TemporaryDirectory(prefix="msime-game-compat-") as directory:
        source = pathlib.Path(directory) / "test.cpp"
        binary = pathlib.Path(directory) / "test"
        source.write_text(HARNESS.replace("@FUNCTIONS@", "\n".join(methods)))
        subprocess.run([compiler, "-std=c++17", "-Wall", "-Wextra", "-Werror", "-I" + str(ROOT / "shared/contracts"), str(source), "-o", str(binary)], check=True)
        subprocess.run([str(binary)], check=True)
    print("Windows game compatibility: TSF/IMM Caps Lock timing, Bedrock physical anchors and candidate ordering and focus-bound sends passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

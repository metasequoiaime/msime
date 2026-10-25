#include "../../panels/win7/KeyboardWindow.h"
#include "../../settings/win7/PreferencesStore.h"
#include <cassert>
#include <cstdio>
#include <fstream>

namespace {
using namespace msime::windows::win7;
using namespace msime::settings::win7;
struct TemporaryState {
  std::filesystem::path path;
  TemporaryState() { wchar_t root[MAX_PATH]{}, file[MAX_PATH]{}; assert(GetTempPathW(MAX_PATH,root) && GetTempFileNameW(root,L"m7k",0,file)); assert(DeleteFileW(file) && CreateDirectoryW(file,nullptr)); path = file; }
  ~TemporaryState() { std::error_code error; std::filesystem::remove_all(path,error); }
};
struct Environment {
  const wchar_t* name; std::wstring previous; bool present = false;
  Environment(const wchar_t* key, const std::filesystem::path& value) : name(key) {
    const auto size = GetEnvironmentVariableW(name,nullptr,0); present = size != 0;
    if (present) { previous.resize(size); previous.resize(GetEnvironmentVariableW(name,previous.data(),size)); }
    assert(SetEnvironmentVariableW(name,value.c_str()));
  }
  ~Environment() { SetEnvironmentVariableW(name,present ? previous.c_str() : nullptr); }
};
void pump(const std::function<bool()>& condition) {
  const auto deadline = GetTickCount64() + 10000;
  for (;;) {
    MSG message{}; while (PeekMessageW(&message,nullptr,0,0,PM_REMOVE)) { assert(message.message != WM_QUIT); TranslateMessage(&message); DispatchMessageW(&message); }
    if (condition()) return; assert(GetTickCount64() < deadline); MsgWaitForMultipleObjects(0,nullptr,FALSE,20,QS_ALLINPUT);
  }
}
}
int main() {
  TemporaryState temporary;
  Environment state(L"MSIME_CLIENT_STATE_DIR",temporary.path), options(L"MSIME_CLIENT_HOST_OPTIONS",temporary.path / L"runtime-options.json");
  PreferencesStore store(temporary.path.u8string()); auto initial = store.load(); assert(initial.snapshot);
  // 默认不显示语音入口；合成文档显式开启后验证入口，不改变产品出厂值。
  initial = store.save(*initial.snapshot,{{{"touch_voice_shortcut"},true}}); assert(initial.snapshot);
  const auto instance = GetModuleHandleW(nullptr);
  const auto editor = CreateWindowExW(0,L"EDIT",L"synthetic-editor",WS_POPUP,0,0,100,100,nullptr,nullptr,instance,nullptr);
  const auto second = CreateWindowExW(0,L"EDIT",L"synthetic-second",WS_POPUP,0,0,100,100,nullptr,nullptr,instance,nullptr); assert(editor && second);
  HWND foreground = editor, delivered = nullptr; unsigned probes = 0, sends = 0, opens = 0; bool drift = false, partial = false;
  std::vector<INPUT> received; std::string opened;
  KeyboardWindow window([&] { ++probes; return drift && probes % 2 == 0 ? second : foreground; },
    [&](HWND destination, const std::vector<INPUT>& inputs) { ++sends; delivered = destination; received = inputs; return partial ? UINT{1} : static_cast<UINT>(inputs.size()); },
    [&](std::string_view route) { ++opens; opened = route; return false; });
  assert(window.create(instance)); const auto host = window.handle();
  assert(GetWindowLongPtrW(host,GWL_EXSTYLE) & WS_EX_NOACTIVATE);
  assert(SendMessageW(host,WM_MOUSEACTIVATE,0,0) == MA_NOACTIVATE);
  const auto control = [&](int id) { auto child = GetDlgItem(host,id); assert(child); return child; };
  const auto wait = [&] { pump([&] { return IsWindowEnabled(control(keyboard_panel_refresh)) != FALSE; }); }; wait();
  const auto click = [&](int id) { SendMessageW(control(id),BM_CLICK,0,0); };
  const auto find = [&](const wchar_t* label) {
    for (int id = keyboard_panel_first_key; id < keyboard_panel_first_key + 200; ++id) { const auto child = GetDlgItem(host,id); if (!child) break; if (control_text::wide(control_text::read(child).value_or("")) == label) return id; }
    assert(false); return 0;
  };
  auto a = find(L"a"), shift = find(L"Shift"), caps = find(L"Caps Lock"), ctrl = find(L"Ctrl");
  for (const auto* title : {L"F12",L"Num 0",L"Num /",L"PrtSc",L"Pause",L"↑",L"Menu"}) assert(find(title));
  click(a); assert(sends == 1 && received.size() == 2 && received[0].ki.wVk == 'A' && delivered == editor);
  click(shift); a = find(L"A"); click(a); assert(received.size() == 4 && received[0].ki.wVk == VK_SHIFT && received[3].ki.dwFlags & KEYEVENTF_KEYUP); a = find(L"a");
  click(caps); a = find(L"A"); click(shift); a = find(L"a"); click(a); assert(received.size() == 2); a = find(L"A"); click(caps); a = find(L"a");
  click(ctrl); click(a); assert(received.size() == 4 && received[0].ki.wVk == VK_CONTROL && received[3].ki.wVk == VK_CONTROL);
  click(find(L"1")); assert(received.size() == 2 && received[0].ki.wVk == '1'); click(a); assert(received.size() == 4); click(ctrl);
  const auto calls_before = sends;
  // 鼠标按下不发键；拖到键外松开和捕获被取消不提交，长按期间不连发。
  const auto button = control(a);
  SendMessageW(button,WM_LBUTTONDOWN,MK_LBUTTON,MAKELPARAM(3,3)); assert(sends == calls_before);
  SendMessageW(button,WM_MOUSEMOVE,MK_LBUTTON,MAKELPARAM(-10,-10));
  SendMessageW(button,WM_LBUTTONUP,0,MAKELPARAM(-10,-10)); assert(sends == calls_before);
  SendMessageW(button,WM_LBUTTONDOWN,MK_LBUTTON,MAKELPARAM(3,3)); SendMessageW(button,WM_CANCELMODE,0,0);
  SendMessageW(button,WM_LBUTTONUP,0,MAKELPARAM(3,3)); assert(sends == calls_before);
  SendMessageW(button,WM_LBUTTONDOWN,MK_LBUTTON,MAKELPARAM(3,3));
  SendMessageW(button,WM_LBUTTONUP,0,MAKELPARAM(3,3)); assert(sends == calls_before + 1);
  SendMessageW(button,WM_LBUTTONDBLCLK,MK_LBUTTON,MAKELPARAM(3,3)); assert(sends == calls_before + 1);
  SendMessageW(button,WM_LBUTTONUP,0,MAKELPARAM(3,3)); assert(sends == calls_before + 2);
  // 每次点击重新捕获当前编辑器；前台在发送前变化、面板自己或无前台均拒绝发送。
  foreground = second; click(a); assert(delivered == second);
  auto before = sends; foreground = editor; drift = true; probes = 0; click(a); assert(sends == before); drift = false;
  foreground = host; click(a); assert(sends == before); foreground = button; click(a); assert(sends == before);
  foreground = nullptr; click(a); assert(sends == before); foreground = editor;
  SendMessageW(host,WM_COMMAND,MAKEWPARAM(a,BN_CLICKED),reinterpret_cast<LPARAM>(control(shift))); assert(sends == before);
  partial = true; click(a); assert(sends == before + 1 && control_text::read(control(keyboard_panel_status)).value().find("未重试") != std::string::npos); partial = false;
  click(keyboard_panel_voice); assert(opens == 1 && opened == "voice"); click(keyboard_panel_settings); assert(opens == 2 && opened == "settings:screen-keyboard");
  click(keyboard_panel_layout); assert(!GetDlgItem(host,keyboard_panel_first_key + 13)); click(find(L"2")); assert(received[0].ki.wVk == '2');
  assert(store.load().snapshot->revision() == initial.snapshot->revision());
  // 已应用方案变更经共享 C ABI 生成双拼提示；刷新不修改偏好，坏文件撤销按键权限。
  auto changed = store.save(*initial.snapshot,{{{"touch_keyboard_layout"},"nine_key_shuangpin"}}); assert(changed.snapshot);
  const auto hints = keyboard::shuangpin_hints(changed.snapshot->preferences()); assert(hints && !hints->empty());
  click(keyboard_panel_refresh); wait(); const auto first_title = control_text::read(control(keyboard_panel_first_key)).value(); assert(first_title.find('\n') != std::string::npos);
  assert(store.load().snapshot->revision() == changed.snapshot->revision());
  // 破坏后不自动重建默认配置；读取失败期间即使伪造点击也不发键。
  { std::ofstream output(temporary.path / L"preferences.json"); output << "synthetic-invalid"; }
  click(keyboard_panel_refresh); wait(); before = sends;
  assert(!IsWindowEnabled(control(keyboard_panel_first_key)));
  click(keyboard_panel_first_key); assert(sends == before);
  DestroyWindow(host); DestroyWindow(editor); DestroyWindow(second);
  std::puts("Windows 7 keyboard: real HWND release/cancel, modifiers, target drift, serial delivery, shared nine-key hints and read-only preferences passed");
}

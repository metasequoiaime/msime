#include "../../settings/win7/SettingsWindow.h"
#include "../../settings/win7/KeyboardForm.h"
#include <cassert>
#include <cstdio>

namespace {
using namespace msime::settings::win7;
using Form = KeyboardForm;
void pump(const std::function<bool()>& ready) {
  const auto deadline = GetTickCount64() + 10000;
  for (;;) {
    MSG message{}; while (PeekMessageW(&message,nullptr,0,0,PM_REMOVE)) { assert(message.message != WM_QUIT); TranslateMessage(&message); DispatchMessageW(&message); }
    if (ready()) return; assert(GetTickCount64() < deadline); MsgWaitForMultipleObjects(0,nullptr,FALSE,10,QS_ALLINPUT);
  }
}
}
int main() {
  wchar_t temp[MAX_PATH]{}, unique[MAX_PATH]{}; assert(GetTempPathW(MAX_PATH,temp) && GetTempFileNameW(temp,L"m7k",0,unique));
  assert(DeleteFileW(unique) && CreateDirectoryW(unique,nullptr)); const std::filesystem::path directory(unique);
  const auto size = GetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR",nullptr,0); std::wstring previous(size,L'\0');
  if (size) previous.resize(GetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR",previous.data(),size));
  assert(SetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR",directory.c_str()));
  {
    PreferencesStore store(directory.u8string()); const auto initial = store.load(); assert(initial.snapshot);
    bool approve = false; unsigned confirmations = 0, opens = 0; std::string opened;
    SettingsWindow window({},[&](HWND,const wchar_t*,const wchar_t*) { ++confirmations; return approve; },{},{},{},{},{},{},{},{},{},
      [&](std::string_view route) { ++opens; opened = route; return false; });
    assert(window.open_route("settings:screen-keyboard") && window.create(GetModuleHandleW(nullptr)));
    const auto host = window.handle(), body = GetDlgItem(host,page_body), selector = GetDlgItem(host,page_selector); assert(body && selector);
    const auto control = [&](int id) { auto child = GetDlgItem(body,id); assert(child); return child; };
    const auto wait = [&] { pump([&] { return IsWindowEnabled(GetDlgItem(body,Form::save)) != FALSE; }); }; wait();
    assert(SendMessageW(selector,CB_GETCURSEL,0,0) == 20 && store.load().snapshot->revision() == initial.snapshot->revision());
    const auto click = [&](int id) { SendMessageW(control(id),BM_CLICK,0,0); wait(); };
    click(Form::save); assert(store.load().snapshot->revision() == initial.snapshot->revision());
    const auto slide = [&](int id, int position) { SendMessageW(control(id),TBM_SETPOS,TRUE,position); SendMessageW(body,WM_HSCROLL,MAKEWPARAM(TB_THUMBTRACK,position),reinterpret_cast<LPARAM>(control(id))); };
    slide(Form::height,23); slide(Form::key_gap,35); slide(Form::row_gap,96); click(Form::voice);
    assert(store.load().snapshot->revision() == initial.snapshot->revision()); click(Form::save);
    auto saved = store.load(); assert(saved.snapshot); const auto& preferences = saved.snapshot->preferences();
    assert(preferences.at("touch_keyboard_height_adjustment") == 23 && preferences.at("touch_key_spacing_tenths") == 35 && preferences.at("touch_row_spacing_tenths") == 96 && preferences.at("touch_voice_shortcut") == true);
    assert(preferences.at("voice_input") == initial.snapshot->preferences().at("voice_input") && preferences.at("touch_keyboard_layout") == initial.snapshot->preferences().at("touch_keyboard_layout"));
    // 可拖动预览只改本地表单；小于阈值不改，拖动后保存仍走真实共享存储。
    const auto preview = control(Form::preview); const auto revision = saved.snapshot->revision();
    SendMessageW(preview,WM_LBUTTONDOWN,MK_LBUTTON,MAKELPARAM(20,20));
    SendMessageW(preview,WM_MOUSEMOVE,MK_LBUTTON,MAKELPARAM(21,21));
    assert(SendMessageW(control(Form::key_gap),TBM_GETPOS,0,0) == 35);
    SendMessageW(preview,WM_MOUSEMOVE,MK_LBUTTON,MAKELPARAM(38,20));
    SendMessageW(preview,WM_LBUTTONUP,0,MAKELPARAM(38,20));
    assert(SendMessageW(control(Form::key_gap),TBM_GETPOS,0,0) > 35 && store.load().snapshot->revision() == revision); click(Form::save);
    click(Form::open); assert(opens == 1 && opened == "keyboard");
    if constexpr (MSIME_EDITION_HANDWRITING != 0) { click(Form::handwriting); assert(opened == "handwriting"); }
    else assert(!GetDlgItem(body,Form::handwriting));
    // 取消恢复不写入；确认后只重置五项，其他偏好保持不变。
    saved = store.load(); const auto before = saved.snapshot->preferences(); click(Form::reset); assert(confirmations == 1 && store.load().snapshot->revision() == saved.snapshot->revision());
    approve = true; click(Form::reset); saved = store.load(); assert(saved.snapshot && confirmations == 2);
    const auto defaults = theme_data::value(theme_data::response(msime_client_default_preferences())); assert(defaults);
    for (const auto* field : {"touch_keyboard_height_adjustment","touch_key_spacing_tenths","touch_row_spacing_tenths","touch_voice_shortcut","touch_toolbar"}) assert(saved.snapshot->preferences().at(field) == defaults->at(field));
    assert(saved.snapshot->preferences().at("voice_input") == before.at("voice_input"));
    assert(store.save(*saved.snapshot,{{{"voice_input"},nullptr,true}}).outcome == Outcome::invalid_change);
    // 并发保存冲突不能覆盖外部修改，失败后重读真实状态。
    auto external = store.save(*saved.snapshot,{{{"touch_keyboard_height_adjustment"},17}}); assert(external.snapshot);
    slide(Form::key_gap,31); click(Form::save); auto actual = store.load(); assert(actual.snapshot->revision() == external.snapshot->revision() && actual.snapshot->preferences().at("touch_keyboard_height_adjustment") == 17);
    click(Form::themes); pump([&] { return IsWindowEnabled(GetDlgItem(body,theme_mode)) != FALSE; }); assert(SendMessageW(selector,CB_GETCURSEL,0,0) == 8);
    DestroyWindow(host);
  }
  SetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR",size ? previous.c_str() : nullptr);
  std::error_code error; std::filesystem::remove_all(directory,error);
  std::puts("Windows 7 keyboard settings: real trackbars, draggable preview, explicit save/reset, cancellation, conflict preservation and routes passed");
}

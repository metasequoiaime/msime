#include "../../settings/win7/KeyboardSkinPage.h"
#include "../../settings/win7/KeyboardSkin.h"
#include <cassert>
#include <cstdio>
#include <fstream>
#include <thread>
#include <atomic>

using namespace msime::settings::win7;
namespace {
void pump(const std::function<bool()>& ready) {
  const auto deadline = GetTickCount64() + 15000;
  for (;;) {
    MSG message{}; while (PeekMessageW(&message,nullptr,0,0,PM_REMOVE)) { TranslateMessage(&message); DispatchMessageW(&message); }
    if (ready()) return; assert(GetTickCount64() < deadline); MsgWaitForMultipleObjects(0,nullptr,FALSE,20,QS_ALLINPUT);
  }
}
uint64_t pixels(const keyboard_skin::Look& look) {
  assert(keyboard_skin::start()); Gdiplus::Bitmap image(200,100,PixelFormat32bppARGB);
  { Gdiplus::Graphics graphics(&image); keyboard_skin::background(graphics,{0,0,200,100},look,1); keyboard_skin::key(graphics,{20,20,160,60},look,false,false,1); }
  uint64_t hash = 1469598103934665603ULL;
  for (int y = 0; y < 100; ++y) for (int x = 0; x < 200; ++x) { Gdiplus::Color color; assert(image.GetPixel(x,y,&color) == Gdiplus::Ok); hash = (hash ^ color.GetValue()) * 1099511628211ULL; }
  return hash;
}
}
int main() {
  using Page = KeyboardSkinPage;
  wchar_t temporary[MAX_PATH]{}; assert(GetTempPathW(MAX_PATH,temporary));
  const auto directory = std::filesystem::path(temporary) / (L"msime-synthetic-keyboard-skin-" + std::to_wstring(GetCurrentProcessId()));
  std::filesystem::create_directories(directory);
  const auto store = PreferencesStore(directory.u8string()); const auto initial = store.load(); assert(initial.snapshot);
  const auto library = [&] { const auto value = theme_data::value(theme_data::call(msime_client_custom_skin_library,{{"directory",directory.u8string()}})); assert(value); return *value; };
  const auto thread = GetCurrentThreadId(); bool approve = true;
  std::optional<std::filesystem::path> chosen_photo;
  Page::Services services;
  services.confirm = [&](HWND,const wchar_t*,const wchar_t*) { assert(GetCurrentThreadId() == thread); return approve; };
  services.pick_photo = [&](HWND) { return chosen_photo; };
  services.pick_color = [&](HWND,COLORREF) { return std::optional<COLORREF>(RGB(18,52,86)); };
  Json generated_design;
  std::atomic<bool> hold_generation{false}, cancelled{false}, generating{false};
  services.generate = [&](const std::filesystem::path& path,const std::string& request,const std::string& prompt,const std::function<void(uint32_t)>& progress) {
    assert(GetCurrentThreadId() != thread && path == directory && !request.empty() && prompt == "synthetic style");
    generating = true; progress(1);
    while (hold_generation && !cancelled) std::this_thread::sleep_for(std::chrono::milliseconds(5));
    generating = false;
    return cancelled ? Json{{"ok",false},{"error","ai_skin_cancelled"}}.dump() : Json{{"ok",true},{"value",Json::array({{{"name","合成 AI"},{"design",generated_design}}})}}.dump();
  };
  services.cancel = [&](const std::string& request) { assert(GetCurrentThreadId() == thread && !request.empty()); cancelled = true; };
  const auto instance = GetModuleHandleW(nullptr);
  const auto parent = CreateWindowExW(0,L"STATIC",L"合成皮肤验收",WS_OVERLAPPEDWINDOW,0,0,800,600,nullptr,nullptr,instance,nullptr); assert(parent);
  {
    Page page([&] { assert(GetCurrentThreadId() != thread); return directory; },services);
    assert(page.create(parent,instance,static_cast<HFONT>(GetStockObject(DEFAULT_GUI_FONT)),96));
    const auto root = page.handle(); const auto control = [&](int id) { const auto child = GetDlgItem(root,id); assert(child); return child; };
    const auto wait = [&] { pump([&] { return IsWindowEnabled(control(Page::refresh)) != FALSE; }); };
    const auto click = [&](int id) { assert(IsWindowEnabled(control(id))); SendMessageW(control(id),BM_CLICK,0,0); wait(); };
    wait(); assert(library().empty() && store.load().snapshot->revision() == initial.snapshot->revision());
    SetWindowTextW(control(Page::name),L"合成晨光"); click(Page::create_skin); assert(library().size() == 1);
    const auto original = library()[0].at("design"); assert(store.load().snapshot->revision() == initial.snapshot->revision());
    generated_design = original; generated_design["cornerRadius"] = 11;
    assert(SendMessageW(control(Page::templates),CB_GETCOUNT,0,0) == 15 && SendMessageW(control(Page::backgrounds),CB_GETCOUNT,0,0) == 9);
    click(Page::use_template); click(Page::save_skin); const auto preset = library()[0].at("design"); assert(preset != original);
    click(Page::undo); click(Page::save_skin); assert(library()[0].at("design") == original);
    click(Page::redo); click(Page::save_skin); assert(library()[0].at("design") == preset);
    click(Page::reset_design); click(Page::save_skin); assert(library()[0].at("design") == original);
    SendMessageW(control(Page::backgrounds),CB_SETCURSEL,2,0); click(Page::use_background); click(Page::optimize_contrast); click(Page::save_skin);
    auto optimized = library()[0].at("design"); assert(optimized.at("background") == 0x171717 && optimized.at("keyForeground") == 0xFFFFFF);
    click(Page::reset_design);
    SetWindowTextW(control(Page::ai_prompt),L"synthetic style"); click(Page::ai_generate);
    assert(SendMessageW(control(Page::ai_proposals),CB_GETCOUNT,0,0) == 1); click(Page::ai_use); click(Page::save_skin);
    assert(library()[0].at("design").at("cornerRadius") == 11 && store.load().snapshot->revision() == initial.snapshot->revision());
    hold_generation = true; SendMessageW(control(Page::ai_generate),BM_CLICK,0,0); pump([&] { return generating.load(); });
    SendMessageW(control(Page::ai_cancel),BM_CLICK,0,0); wait(); assert(cancelled && library()[0].at("design").at("cornerRadius") == 11);
    click(Page::reset_design); click(Page::save_skin);
    // 所有颜色通过原生选色边界进入真实窗口事件，保存仍走共享命名库。
    for (int index = 0; index < 7; ++index) click(Page::first_color + index);
    SetWindowTextW(control(Page::first_number),L"17"); SetWindowTextW(control(Page::first_number + 1),L"1.5");
    SetWindowTextW(control(Page::first_number + 2),L"0.3"); SetWindowTextW(control(Page::first_number + 3),L"0.65");
    SendMessageW(control(Page::shape),CB_SETCURSEL,3,0); SendMessageW(root,WM_COMMAND,MAKEWPARAM(Page::shape,CBN_SELCHANGE),reinterpret_cast<LPARAM>(control(Page::shape)));
    SendMessageW(control(Page::material),CB_SETCURSEL,2,0); SendMessageW(control(Page::pattern),CB_SETCURSEL,2,0);
    click(Page::monospaced); click(Page::horizontal); click(Page::preview_change);
    assert(library()[0].at("design") == original); click(Page::save_skin);
    auto design = library()[0].at("design"); assert(design.at("keyBackground") == 0x123456 && design.at("background") == 0x123456);
    assert(design.at("cornerRadius") == 17 && design.at("borderWidth") == 1.5 && design.at("keyOpacity") == .65);
    assert(design.at("keyShape") == "pebble" && design.at("keyMaterial") == "glass" && design.at("monospaced") == true && design.at("gradientHorizontal") == true);
    // 存库不改主题；CAS 冲突先拒绝，明确再次应用才可保存，其他主题字段原样保留。
    auto external = store.save(*initial.snapshot,{{{"theme"},"dark"}}); assert(external.snapshot);
    click(Page::apply_skin); assert(store.load().snapshot->revision() == external.snapshot->revision());
    click(Page::apply_skin); auto applied = store.load(); assert(applied.snapshot->preferences().at("global_theme") == "custom");
    assert(applied.snapshot->preferences().at("theme") == "dark" && applied.snapshot->preferences().at("custom_theme").at("keyboard") == design);
    auto remaining = applied.snapshot->preferences(); remaining["global_theme"] = external.snapshot->preferences().at("global_theme"); remaining["custom_theme"] = external.snapshot->preferences().at("custom_theme"); assert(remaining == external.snapshot->preferences());
    const auto valid = directory / "synthetic-photo.png";
    const unsigned char png[]{137,80,78,71,13,10,26,10,0,0,0,13,73,72,68,82,0,0,0,2,0,0,0,2,8,2,0,0,0,253,212,154,115,0,0,0,16,73,68,65,84,120,156,99,184,35,103,4,68,12,16,10,0,35,134,4,177,102,242,130,90,0,0,0,0,73,69,78,68,174,66,96,130};
    { std::ofstream file(valid,std::ios::binary); file.write(reinterpret_cast<const char*>(png),sizeof(png)); }
    chosen_photo = valid; click(Page::photo); click(Page::save_skin); design = library()[0].at("design");
    auto photo_look = keyboard_skin::Look::custom(design); assert(!photo_look.image_failed && photo_look.photo.width == 2);
    const auto photo_hash = pixels(photo_look); auto no_photo = design; no_photo["photo"] = nullptr; assert(photo_hash != pixels(keyboard_skin::Look::custom(no_photo)));
    const auto bad = directory / "synthetic-bad.png"; { std::ofstream file(bad); file << "synthetic-invalid-image"; }
    chosen_photo = bad; click(Page::photo); assert(library()[0].at("design") == design);
    click(Page::remove_photo); click(Page::save_skin); assert(library()[0].at("design").at("photo").is_null());
    // 形状和材质确实改变栅格结果，避免只检查 JSON 能落盘。
    auto sample = original; sample["keyBackground"] = 0xDDE8EE; sample["cornerRadius"] = 15; sample["shadow"] = .3; sample["keyOpacity"] = .7;
    const auto baseline = pixels(keyboard_skin::Look::custom(sample));
    for (const char* shape : {"capsule","ticket","pebble"}) { auto next = sample; next["keyShape"] = shape; assert(pixels(keyboard_skin::Look::custom(next)) != baseline); }
    for (const char* material : {"raised","glass","paper"}) { auto next = sample; next["keyMaterial"] = material; assert(pixels(keyboard_skin::Look::custom(next)) != baseline); }
    SetWindowTextW(control(Page::first_number),L"999"); click(Page::save_skin); assert(library()[0].at("design").at("cornerRadius") == 17);
    click(Page::refresh); SetWindowTextW(control(Page::name),L"合成暮色"); click(Page::rename_skin); assert(library()[0].at("name") == "合成暮色");
    approve = false; click(Page::delete_skin); assert(library().size() == 1);
    approve = true; click(Page::delete_skin); assert(library().empty()); assert(store.load().snapshot->revision() == applied.snapshot->revision());
    // 关闭仍在生成的页面必须先取消再等待线程，不能留下回调引用已销毁的窗口。
    cancelled = false; hold_generation = true;
    SendMessageW(control(Page::ai_generate),BM_CLICK,0,0); pump([&] { return generating.load(); });
  }
  assert(cancelled && !generating);
  DestroyWindow(parent); std::error_code error; std::filesystem::remove_all(directory,error);
  std::puts("Windows 7 keyboard skin: native controls, shared presets, undo/redo, AI progress/cancellation, explicit apply, CAS, photos and pixels passed");
}

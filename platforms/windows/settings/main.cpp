// windows.h comes first so shellapi.h sees its types; the two macros it defines that collide with WinRT member names are dropped before any WinRT header is read.
#include <windows.h>

#include <shellapi.h>
#undef GetCurrentTime
#undef GetObject

#include <algorithm>
#include <array>
#include <chrono>
#include <cmath>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <cwctype>
#include <filesystem>
#include <functional>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include "CandidatePalette.h"
#include "SettingsNavigation.h"
#include "ShellLauncher.h"
#include "msime_client.h"

#include <winrt/base.h>

#include <winrt/Microsoft.UI.Dispatching.h>
#include <winrt/Microsoft.UI.Input.h>
#include <winrt/Microsoft.UI.Windowing.h>
#include <winrt/Microsoft.UI.Xaml.Automation.Peers.h>
#include <winrt/Microsoft.UI.Xaml.Automation.h>
#include <winrt/Microsoft.UI.Xaml.Controls.Primitives.h>
#include <winrt/Microsoft.UI.Xaml.Controls.h>
#include <winrt/Microsoft.UI.Xaml.Input.h>
#include <winrt/Microsoft.UI.Xaml.Markup.h>
#include <winrt/Microsoft.UI.Xaml.Media.Imaging.h>
#include <winrt/Microsoft.UI.Xaml.Media.h>
#include <winrt/Microsoft.UI.Xaml.XamlTypeInfo.h>
#include <winrt/Microsoft.UI.Xaml.h>
#include <winrt/Microsoft.UI.h>
#include <winrt/Windows.ApplicationModel.DataTransfer.h>
#include <winrt/Windows.Data.Json.h>
#include <winrt/Windows.Foundation.Collections.h>
#include <winrt/Windows.Foundation.h>
#include <winrt/Windows.Graphics.h>
#include <winrt/Windows.Storage.Streams.h>
#include <winrt/Windows.UI.Text.h>
#include <winrt/Windows.UI.ViewManagement.h>
#include <winrt/Windows.UI.Xaml.Interop.h>
#include <winrt/Windows.UI.h>

using namespace winrt;
using namespace Microsoft::UI::Xaml;
using namespace Microsoft::UI::Xaml::Controls;
using namespace Microsoft::UI::Xaml::Media;
using namespace Windows::Data::Json;

namespace {

// windows.h may declare the COM ::IInspectable as well; this alias always means the WinRT projection.
using Inspectable = winrt::Windows::Foundation::IInspectable;
using Color = winrt::Windows::UI::Color;
using A11y = Microsoft::UI::Xaml::Automation::AutomationProperties;
namespace nav = msime::settings;

struct Response {
  std::string text;
  bool ok = false;
};

std::string utf8(hstring value) { return to_string(value); }

hstring text(std::string_view value) { return to_hstring(std::string(value)); }

Response take_response(char *raw) {
  if (!raw) {
    return {"{\"ok\":false,\"error\":\"empty host response\"}", false};
  }
  std::string value(raw);
  msime_client_string_free(raw);
  try {
    const auto object = JsonObject::Parse(text(value));
    return {std::move(value), object.GetNamedBoolean(L"ok", false)};
  } catch (...) {
    return {std::move(value), false};
  }
}

// An environment variable, empty when it is unset. GetEnvironmentVariableW rather than the CRT's _wgetenv, which MSVC deprecates (C4996) and /WX would turn into an error.
std::wstring environment(const wchar_t *name) {
  std::wstring value(64, L'\0');
  for (;;) {
    const DWORD length = GetEnvironmentVariableW(
        name, value.data(), static_cast<DWORD>(value.size()));
    if (length == 0)
      return {};
    if (length < value.size()) {
      value.resize(length);
      return value;
    }
    value.resize(length);
  }
}

std::filesystem::path state_directory() {
  if (const auto raw = environment(L"MSIME_CLIENT_STATE_DIR"); !raw.empty()) {
    std::filesystem::path path(raw);
    if (path.is_absolute()) {
      return path;
    }
  }
  if (const auto local = environment(L"LOCALAPPDATA"); !local.empty()) {
    return std::filesystem::path(local) / L"MSIME-Client";
  }
  return {};
}

std::string path_utf8(const std::filesystem::path &path) {
  const auto value = path.wstring();
  if (value.empty())
    return {};
  const int length = WideCharToMultiByte(
      CP_UTF8, WC_ERR_INVALID_CHARS, value.data(),
      static_cast<int>(value.size()), nullptr, 0, nullptr, nullptr);
  if (length <= 0)
    return {};
  std::string result(static_cast<size_t>(length), '\0');
  WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, value.data(),
                      static_cast<int>(value.size()), result.data(), length,
                      nullptr, nullptr);
  return result;
}

// ---- Default input method ----

// Whether this input method is in the user's keyboard list, and whether it is the default one. Unknown when input.dll cannot answer, in which case no banner is shown rather than a wrong one.
enum class InputMethodState { unknown, missing, not_default, ready };

// The text service and its language profile, kept in sync with platforms/windows/tsf/Global/Globals.cpp (MetasequoiaIMECLSID, MetasequoiaIMEGuidProfile) and src/system/Watchdog.cpp.
constexpr GUID input_method_clsid = {
    0xe3062e9a, 0xd834, 0x4637, {0x89, 0x58, 0xed, 0x8c, 0xfa, 0x42, 0x7d, 0x01}};
constexpr GUID input_method_profile = {
    0x4d59b1b4, 0xd503, 0x44ae, {0x92, 0x59, 0xba, 0xd9, 0xbb, 0x27, 0x78, 0xab}};
// The same profile in the <LangID>:<CLSID><profile> form InstallLayoutOrTip and SetDefaultLayoutOrTip take.
constexpr const wchar_t *input_method_id =
    L"0x0804:{E3062E9A-D834-4637-8958-ED8CFA427D01}"
    L"{4D59B1B4-D503-44AE-9259-BAD9BB2778AB}";

// input.dll's "Install Layout or Tip" functions are documented but ship without a header or an import library, so the structure and flags are declared here as the documentation gives them (LAYOUTORTIPPROFILE, LOT_DEFAULT, LOT_DISABLED).
struct LayoutOrTipProfile {
  DWORD profile_type;
  LANGID language;
  GUID clsid;
  GUID profile;
  GUID category;
  DWORD substitute_layout;
  DWORD flags;
  WCHAR id[MAX_PATH];
};
constexpr DWORD layout_or_tip_default = 0x0001;
constexpr DWORD layout_or_tip_disabled = 0x0002;

template <typename Function> Function input_dll_function(const char *name) {
  // A system DLL, loaded from System32 only and kept until the process exits.
  static const HMODULE module =
      LoadLibraryExW(L"input.dll", nullptr, LOAD_LIBRARY_SEARCH_SYSTEM32);
  if (!module)
    return nullptr;
  // Through void*: GetProcAddress returns a generic FARPROC, and casting that straight to the real signature is what -Wcast-function-type rejects.
  return reinterpret_cast<Function>(
      reinterpret_cast<void *>(GetProcAddress(module, name)));
}

bool same_guid(GUID const &a, GUID const &b) {
  return std::memcmp(&a, &b, sizeof(GUID)) == 0;
}

InputMethodState input_method_state() {
  using Enumerate = UINT(WINAPI *)(LPCWSTR, LPCWSTR, LPCWSTR,
                                   LayoutOrTipProfile *, UINT);
  const auto enumerate = input_dll_function<Enumerate>("EnumEnabledLayoutOrTip");
  if (!enumerate)
    return InputMethodState::unknown;
  // With a null buffer the function returns how many items the user has enabled; the buffer length is in items.
  const UINT count = enumerate(nullptr, nullptr, nullptr, nullptr, 0);
  if (count == 0)
    return InputMethodState::unknown;
  std::vector<LayoutOrTipProfile> profiles(count);
  const UINT copied =
      std::min(enumerate(nullptr, nullptr, nullptr, profiles.data(), count), count);
  if (copied == 0)
    return InputMethodState::unknown;
  for (UINT i = 0; i < copied; ++i) {
    const auto &item = profiles[i];
    if (!same_guid(item.clsid, input_method_clsid) ||
        !same_guid(item.profile, input_method_profile))
      continue;
    if (item.flags & layout_or_tip_disabled)
      return InputMethodState::missing;
    return (item.flags & layout_or_tip_default) != 0
               ? InputMethodState::ready
               : InputMethodState::not_default;
  }
  return InputMethodState::missing;
}

// Adds the input method to the current user's keyboard list.
bool add_input_method() {
  using Install = BOOL(WINAPI *)(LPCWSTR, DWORD);
  const auto install = input_dll_function<Install>("InstallLayoutOrTip");
  return install && install(input_method_id, 0) != FALSE;
}

// Makes the input method the current user's default, for this session too.
bool make_input_method_default() {
  using SetDefault = BOOL(WINAPI *)(LPCWSTR, DWORD);
  const auto set_default = input_dll_function<SetDefault>("SetDefaultLayoutOrTip");
  return set_default && set_default(input_method_id, 0) != FALSE;
}

// The page this window opens on: `--route=settings:<id>` from the Server and the other launchers, or the legacy MSIME_CLIENT_SETTINGS_PAGE variable. Either a settings category of the shared route vocabulary or a page id of this window; anything else opens the default page.
std::string route_page() {
  int argc = 0;
  auto *argv = CommandLineToArgvW(GetCommandLineW(), &argc);
  std::optional<std::string> page;
  if (argv) {
    for (int i = 1; i < argc; ++i) {
      constexpr std::wstring_view prefix = L"--route=settings";
      const std::wstring_view argument(argv[i]);
      if (argument == prefix) {
        page = std::string();
        break;
      }
      if (argument.starts_with(prefix) && argument.size() > prefix.size() &&
          argument[prefix.size()] == L':') {
        page = utf8(hstring(argument.substr(prefix.size() + 1)));
        break;
      }
    }
    LocalFree(argv);
  }
  if (!page) {
    if (const auto raw = environment(L"MSIME_CLIENT_SETTINGS_PAGE"); !raw.empty())
      page = utf8(hstring(raw));
  }
  return std::string(nav::page_for_route(page.value_or(std::string())));
}

// The runtime options file the Server hands this window, or the one in the state directory when started from the Start menu. None before the input method is set up.
std::optional<std::filesystem::path> runtime_options_file() {
  std::error_code error;
  if (const auto raw = environment(L"MSIME_CLIENT_HOST_OPTIONS"); !raw.empty()) {
    std::filesystem::path path(raw);
    if (path.is_absolute() && std::filesystem::is_regular_file(path, error))
      return path;
  }
  const auto directory = state_directory();
  if (directory.empty())
    return std::nullopt;
  auto path = directory / L"runtime-options.json";
  if (!std::filesystem::is_regular_file(path, error))
    return std::nullopt;
  return path;
}

// The runtime options msime-mcp is pointed at, as the UTF-8 path the host API takes.
std::optional<std::string> runtime_options_path() {
  const auto path = runtime_options_file();
  if (!path)
    return std::nullopt;
  if (auto value = path_utf8(*path); !value.empty())
    return value;
  return std::nullopt;
}

JsonObject mcp_request(std::optional<std::string> const &options) {
  JsonObject request;
  request.SetNamedValue(L"options",
                        options ? JsonValue::CreateStringValue(text(*options))
                                : JsonValue::CreateNullValue());
  return request;
}

Response call_mcp(char *(*entry)(const uint8_t *, size_t),
                  JsonObject const &request) {
  const auto body = utf8(request.Stringify());
  return take_response(
      entry(reinterpret_cast<const uint8_t *>(body.data()), body.size()));
}

std::wstring response_error(Response const &response) {
  try {
    return std::wstring(JsonObject::Parse(text(response.text))
                            .GetNamedString(L"error", L"")
                            .c_str());
  } catch (...) {
    return {};
  }
}

std::wstring mcp_client_name(std::wstring_view id) {
  if (id == L"claude_desktop")
    return L"Claude Desktop";
  if (id == L"cursor")
    return L"Cursor";
  return std::wstring(id);
}

// Same wording as the shared settings page (packages/ui/src/settings/mcp-connect.tsx).
std::wstring mcp_failure(std::wstring const &code, std::wstring const &name) {
  if (code == L"mcp_client_missing")
    return L"没有找到 " + name + L" 的配置目录。请先安装并打开一次 " + name +
           L"。";
  if (code == L"mcp_config_invalid")
    return name + L" 的配置文件不是有效的 JSON，已保持原样。请先修正该文件。";
  if (code == L"mcp_server_missing")
    return L"没有找到 msime-mcp，请重新安装输入法。";
  if (code == L"mcp_options_missing")
    return L"输入法尚未完成初始化，请先完成设置向导。";
  return L"无法写入 " + name + L" 的配置文件。";
}

// The stored value at a key, or null when it is absent. A key is either a top-level preference or `parent.leaf` one level down.
IJsonValue lookup(JsonObject const &root, std::wstring_view key) {
  if (!root)
    return nullptr;
  JsonObject object = root;
  std::wstring_view leaf = key;
  if (const auto separator = key.find(L'.');
      separator != std::wstring_view::npos) {
    const hstring parent(key.substr(0, separator));
    if (!root.HasKey(parent))
      return nullptr;
    const auto nested = root.Lookup(parent);
    if (nested.ValueType() != JsonValueType::Object)
      return nullptr;
    object = nested.GetObject();
    leaf = key.substr(separator + 1);
  }
  const hstring name(leaf);
  return object.HasKey(name) ? object.Lookup(name) : nullptr;
}

class PreferencesDocument {
public:
  PreferencesDocument() { reset_defaults(); }

  bool Load(std::wstring &error) {
    const auto directory = state_directory();
    if (directory.empty()) {
      error = L"找不到水杉输入法的数据目录。请先完成输入法安装。";
      return false;
    }
    const auto path = path_utf8(directory);
    auto response = take_response(msime_client_load_preferences(
        reinterpret_cast<const uint8_t *>(path.data()), path.size()));
    if (!response.ok) {
      error = L"读取设置失败，请稍后重试。";
      reset_defaults();
      return false;
    }
    try {
      document_ =
          JsonObject::Parse(text(response.text)).GetNamedObject(L"value");
      preferences_ = document_.GetNamedObject(L"preferences");
      revision_ =
          static_cast<uint64_t>(document_.GetNamedNumber(L"revision", 0));
      return true;
    } catch (...) {
      error = L"设置文件格式无效。";
      return false;
    }
  }

  bool Save(std::wstring &error) {
    const auto directory = state_directory();
    const auto path = path_utf8(directory);
    document_.SetNamedValue(L"preferences", preferences_);
    const auto snapshot = utf8(document_.Stringify());
    auto response = take_response(msime_client_save_preferences(
        reinterpret_cast<const uint8_t *>(path.data()), path.size(), revision_,
        reinterpret_cast<const uint8_t *>(snapshot.data()), snapshot.size()));
    if (!response.ok) {
      error = L"保存失败：设置可能已在其他窗口中更新，请重新读取后再保存。";
      return false;
    }
    try {
      document_ =
          JsonObject::Parse(text(response.text)).GetNamedObject(L"value");
      preferences_ = document_.GetNamedObject(L"preferences");
      revision_ = static_cast<uint64_t>(
          document_.GetNamedNumber(L"revision", static_cast<double>(revision_)));
      return true;
    } catch (...) {
      error = L"设置已写入，但返回内容无法读取。";
      return false;
    }
  }

  uint64_t revision() const { return revision_; }

  // The stored value, else the shipped default, else null. A stored null (an unset optional field) is returned as it is.
  IJsonValue Value(std::wstring_view key) const {
    if (auto value = lookup(preferences_, key))
      return value;
    return lookup(defaults_, key);
  }

  // The shipped default for a key, for the rows that restore defaults.
  IJsonValue Default(std::wstring_view key) const {
    return lookup(defaults_, key);
  }

  bool Boolean(std::wstring_view key, bool fallback) const {
    const auto value = Value(key);
    return value && value.ValueType() == JsonValueType::Boolean
               ? value.GetBoolean()
               : fallback;
  }

  std::wstring String(std::wstring_view key, std::wstring_view fallback) const {
    const auto value = Value(key);
    return value && value.ValueType() == JsonValueType::String
               ? std::wstring(value.GetString().c_str())
               : std::wstring(fallback);
  }

  double Number(std::wstring_view key, double fallback) const {
    const auto value = Value(key);
    return value && value.ValueType() == JsonValueType::Number
               ? value.GetNumber()
               : fallback;
  }

  std::vector<std::wstring> Strings(std::wstring_view key) const {
    std::vector<std::wstring> result;
    const auto value = Value(key);
    if (!value || value.ValueType() != JsonValueType::Array)
      return result;
    for (auto const &item : value.GetArray())
      if (item.ValueType() == JsonValueType::String)
        result.emplace_back(item.GetString().c_str());
    return result;
  }

  void SetBoolean(std::wstring_view key, bool value) {
    SetValue(key, JsonValue::CreateBooleanValue(value));
  }

  void SetString(std::wstring_view key, std::wstring_view value) {
    SetValue(key, JsonValue::CreateStringValue(hstring(value)));
  }

  void SetNumber(std::wstring_view key, double value) {
    SetValue(key, JsonValue::CreateNumberValue(value));
  }

  void SetStrings(std::wstring_view key, std::vector<std::wstring> values) {
    std::sort(values.begin(), values.end());
    values.erase(std::unique(values.begin(), values.end()), values.end());
    JsonArray array;
    for (auto const &value : values)
      array.Append(JsonValue::CreateStringValue(hstring(value)));
    SetValue(key, array);
  }

  // Replaces a value with a copy of its shipped default. A key without a default is left as it is.
  void Restore(std::wstring_view key) {
    const auto value = Default(key);
    if (value)
      SetValue(key, JsonValue::Parse(value.Stringify()));
  }

  void SetValue(std::wstring_view key, IJsonValue const &value) {
    if (!preferences_)
      return;
    const auto [object, leaf] = object_for_key(key, true);
    if (object)
      object.SetNamedValue(leaf, value);
  }

private:
  void reset_defaults() {
    auto defaults = take_response(msime_client_default_preferences());
    if (!defaults.ok) {
      document_ = JsonObject();
      preferences_ = JsonObject();
      defaults_ = JsonObject();
      revision_ = 0;
      return;
    }
    try {
      defaults_ =
          JsonObject::Parse(text(defaults.text)).GetNamedObject(L"value");
      preferences_ = JsonObject::Parse(defaults_.Stringify());
      document_ = JsonObject();
      document_.SetNamedValue(L"format_version",
                              JsonValue::CreateNumberValue(1));
      document_.SetNamedValue(L"revision", JsonValue::CreateNumberValue(0));
      document_.SetNamedValue(L"preferences", preferences_);
      revision_ = 0;
    } catch (...) {
      document_ = JsonObject();
      preferences_ = JsonObject();
      defaults_ = JsonObject();
      revision_ = 0;
    }
  }

  std::pair<JsonObject, hstring> object_for_key(std::wstring_view key,
                                                bool create = false) const {
    const auto separator = key.find(L'.');
    if (separator == std::wstring_view::npos) {
      return {preferences_, hstring(key)};
    }
    const auto parent = hstring(key.substr(0, separator));
    const auto leaf = hstring(key.substr(separator + 1));
    const auto existing = preferences_.HasKey(parent)
                              ? preferences_.Lookup(parent)
                              : IJsonValue(nullptr);
    if (existing && existing.ValueType() == JsonValueType::Object)
      return {existing.GetObject(), leaf};
    if (!create)
      return {JsonObject(nullptr), leaf};
    // The store accepts a nested object only whole, so an absent one starts from its shipped default rather than from the single field being set.
    JsonObject nested;
    if (const auto fallback = lookup(defaults_, key.substr(0, separator));
        fallback && fallback.ValueType() == JsonValueType::Object)
      nested = JsonObject::Parse(fallback.Stringify());
    preferences_.SetNamedValue(parent, nested);
    return {nested, leaf};
  }

  JsonObject document_{nullptr};
  JsonObject preferences_{nullptr};
  JsonObject defaults_{nullptr};
  uint64_t revision_ = 0;
};

// ---- Palette ----

Color rgba(int red, int green, int blue, double alpha) {
  return Color{static_cast<uint8_t>(std::lround(alpha * 255)),
               static_cast<uint8_t>(red), static_cast<uint8_t>(green),
               static_cast<uint8_t>(blue)};
}

Color transparent_color() { return Color{0, 0, 0, 0}; }

// The design's Windows palette (`w` in the prototype), which spells out the Fluent tokens. The accent is the design's fixed Fluent blue, #60CDFF on dark and #005FB8 on light, the same accent the candidate window's native palette draws (CandidatePalette.h), rather than the user's Windows accent colour.
struct Palette {
  bool dark = false;
  Color text, sub, faint, subtle, subtle2, card, stroke, stroke2, divider,
      accent, on_accent, seg_well, preview_bg, window_bg, content_bg, edge;
};

Palette make_palette(bool dark) {
  Palette p;
  p.dark = dark;
  p.accent = dark ? rgba(0x60, 0xCD, 0xFF, 1) : rgba(0x00, 0x5F, 0xB8, 1);
  p.on_accent = dark ? rgba(0, 0, 0, 1) : rgba(255, 255, 255, 1);
  if (dark) {
    p.text = rgba(255, 255, 255, 1);
    p.sub = rgba(255, 255, 255, .786);
    p.faint = rgba(255, 255, 255, .6);
    p.subtle = rgba(255, 255, 255, .0605);
    p.subtle2 = rgba(255, 255, 255, .0837);
    p.card = rgba(255, 255, 255, .0512);
    p.stroke = rgba(255, 255, 255, .07);
    p.stroke2 = rgba(255, 255, 255, .093);
    p.divider = rgba(255, 255, 255, .0837);
    p.seg_well = rgba(0, 0, 0, .12);
    p.preview_bg = rgba(0x2C, 0x2C, 0x2C, 1);
    p.window_bg = rgba(0x20, 0x20, 0x20, 1);
    p.content_bg = rgba(0x1C, 0x1C, 0x1C, 1);
    p.edge = rgba(0, 0, 0, .1);
  } else {
    p.text = rgba(0, 0, 0, .894);
    p.sub = rgba(0, 0, 0, .606);
    p.faint = rgba(0, 0, 0, .45);
    p.subtle = rgba(0, 0, 0, .0373);
    p.subtle2 = rgba(0, 0, 0, .0578);
    p.card = rgba(255, 255, 255, .7);
    p.stroke = rgba(0, 0, 0, .0578);
    p.stroke2 = rgba(0, 0, 0, .0803);
    p.divider = rgba(0, 0, 0, .0803);
    p.seg_well = rgba(0, 0, 0, .04);
    p.preview_bg = rgba(255, 255, 255, 1);
    p.window_bg = rgba(0xF3, 0xF3, 0xF3, 1);
    p.content_bg = rgba(255, 255, 255, .5);
    p.edge = rgba(0, 0, 0, .06);
  }
  return p;
}

// A colour of the candidate palette (straight alpha in [0,1]) as a XAML colour.
Color to_color(msime::windows::CandidateColor const &color) {
  auto channel = [](float value) {
    return static_cast<uint8_t>(
        std::lround(std::clamp(value, 0.0f, 1.0f) * 255.0f));
  };
  return Color{channel(color.a), channel(color.r), channel(color.g),
               channel(color.b)};
}

SolidColorBrush brush(Color const &color) { return SolidColorBrush(color); }

// "#RRGGBB" from the theme catalog.
std::optional<Color> parse_hex(std::wstring_view value) {
  if (value.size() != 7 || value[0] != L'#')
    return std::nullopt;
  uint32_t rgb = 0;
  for (size_t i = 1; i < value.size(); ++i) {
    const wchar_t c = value[i];
    uint32_t digit = 0;
    if (c >= L'0' && c <= L'9')
      digit = static_cast<uint32_t>(c - L'0');
    else if (c >= L'a' && c <= L'f')
      digit = static_cast<uint32_t>(c - L'a' + 10);
    else if (c >= L'A' && c <= L'F')
      digit = static_cast<uint32_t>(c - L'A' + 10);
    else
      return std::nullopt;
    rgb = rgb * 16 + digit;
  }
  return Color{255, static_cast<uint8_t>((rgb >> 16) & 0xFF),
               static_cast<uint8_t>((rgb >> 8) & 0xFF),
               static_cast<uint8_t>(rgb & 0xFF)};
}

// ---- Page labels ----

// The sidebar's labels and Segoe Fluent Icons glyphs, keyed by the page ids of SettingsNavigation.h, in the design's wording.
struct PageLabel {
  std::string_view id;
  const wchar_t *title;
  wchar_t glyph;
};

constexpr std::array<PageLabel, 18> page_labels{{
    {"themes", L"主题", 0xE790},
    {"candidate", L"候选窗口", 0xE8FD},
    {"toolbar", L"悬浮工具栏", 0xE7F4},
    {"typing", L"输入", 0xE765},
    {"expression", L"表达", 0xE8C1},
    {"shortcuts", L"快捷键", 0xEDA7},
    {"lexicon", L"词库", 0xE82D},
    {"osk", L"屏幕键盘", 0xE92E},
    {"voice", L"语音输入", 0xE720},
    {"hand", L"手写输入", 0xE929},
    {"account", L"账户与同步", 0xE77B},
    {"clip", L"云剪贴板", 0xE77F},
    {"stats", L"统计", 0xE9D2},
    {"community", L"社区", 0xE716},
    {"download", L"其他平台下载", 0xE896},
    {"dev", L"开发者选项", 0xE90F},
    {"feedback", L"反馈", 0xED15},
    {"about", L"关于", 0xE946},
}};
static_assert(page_labels.size() == nav::pages.size());

const PageLabel &page_label(std::string_view id) {
  for (const auto &label : page_labels)
    if (label.id == id)
      return label;
  return page_labels[3];
}

// What each page of the shared app is for, on the card that opens it.
const wchar_t *shell_page_description(std::string_view id) {
  if (id == "account")
    return L"登录水杉输入法账户，管理设置和词库在设备之间的同步。";
  if (id == "clip")
    return L"查看和管理在各台设备之间同步的剪贴板内容。";
  if (id == "stats")
    return L"查看打字字数、速度和常用输入方式的统计。统计只保存在本机。";
  if (id == "community")
    return L"浏览社区分享的皮肤和内容。";
  return L"";
}

// The line the search list shows when nothing matches. It is a list item, so it can be highlighted and submitted like a result; the search box treats it as neither.
constexpr std::wstring_view no_search_results = L"没有找到相关设置";

constexpr const wchar_t *download_url = L"https://msime.app/download/";
constexpr const wchar_t *privacy_url = L"https://msime.app/privacy/";
constexpr const wchar_t *license_url =
    L"https://github.com/metasequoiaime/msime/blob/develop/LICENSE";
constexpr const wchar_t *releases_url =
    L"https://github.com/metasequoiaime/msime/releases";

std::wstring lowercase(std::wstring_view value) {
  std::wstring result(value);
  for (auto &c : result)
    c = static_cast<wchar_t>(std::towlower(c));
  return result;
}

std::wstring number_text(double value) {
  return std::to_wstring(static_cast<long long>(std::llround(value)));
}

hstring glyph_text(wchar_t glyph) { return hstring(std::wstring(1, glyph)); }

// The largest PNG frame of the product icon (RCDATA 102 is msime.ico), for the title bar and the about card. Empty when the resource is missing or malformed: those places then show no picture.
std::vector<uint8_t> const &logo_png() {
  static const std::vector<uint8_t> bytes = [] {
    std::vector<uint8_t> result;
    const HMODULE instance = GetModuleHandleW(nullptr);
    const HRSRC resource = FindResourceW(instance, MAKEINTRESOURCEW(102),
                                         MAKEINTRESOURCEW(10) /* RT_RCDATA */);
    if (!resource)
      return result;
    const HGLOBAL handle = LoadResource(instance, resource);
    const DWORD size = SizeofResource(instance, resource);
    const auto *data =
        handle ? static_cast<const uint8_t *>(LockResource(handle)) : nullptr;
    if (!data || size < 6)
      return result;
    auto u16 = [data](size_t at) {
      return static_cast<uint32_t>(data[at] | (data[at + 1] << 8));
    };
    auto u32 = [data](size_t at) {
      return static_cast<uint32_t>(data[at]) |
             (static_cast<uint32_t>(data[at + 1]) << 8) |
             (static_cast<uint32_t>(data[at + 2]) << 16) |
             (static_cast<uint32_t>(data[at + 3]) << 24);
    };
    if (u16(0) != 0 || u16(2) != 1)
      return result;
    const uint32_t count = u16(4);
    uint32_t best_width = 0;
    uint32_t best_offset = 0;
    uint32_t best_size = 0;
    for (uint32_t i = 0; i < count; ++i) {
      const size_t entry = 6 + static_cast<size_t>(i) * 16;
      if (entry + 16 > size)
        break;
      const uint32_t width =
          data[entry] == 0 ? 256u : static_cast<uint32_t>(data[entry]);
      const uint32_t length = u32(entry + 8);
      const uint32_t offset = u32(entry + 12);
      if (offset > size || length > size - offset || length < 8)
        continue;
      static constexpr uint8_t png[] = {0x89, 'P', 'N', 'G'};
      if (!std::equal(std::begin(png), std::end(png), data + offset))
        continue;
      if (width > best_width) {
        best_width = width;
        best_offset = offset;
        best_size = length;
      }
    }
    if (best_width != 0)
      result.assign(data + best_offset, data + best_offset + best_size);
    return result;
  }();
  return bytes;
}

fire_and_forget show_logo(Image image, int32_t pixels) {
  const auto &bytes = logo_png();
  if (bytes.empty())
    co_return;
  // Created on the UI thread before the first suspension; awaiting a WinRT operation resumes on this apartment.
  Imaging::BitmapImage bitmap;
  bitmap.DecodePixelWidth(pixels);
  try {
    Windows::Storage::Streams::InMemoryRandomAccessStream stream;
    Windows::Storage::Streams::DataWriter writer(stream);
    writer.WriteBytes(
        array_view<uint8_t const>(bytes.data(), bytes.data() + bytes.size()));
    co_await writer.StoreAsync();
    writer.DetachStream();
    stream.Seek(0);
    co_await bitmap.SetSourceAsync(stream);
    image.Source(bitmap);
  } catch (hresult_error const &) {
    // A logo that cannot be decoded leaves the space empty; nothing else depends on it.
  }
}

struct Option {
  std::wstring value;
  std::wstring label;
};

struct Check {
  std::wstring label;
  bool on = false;
  std::function<void(bool)> apply;
  bool enabled = true;
};

struct SearchEntry {
  std::wstring title;
  std::wstring detail;
  std::string page;
  std::wstring display;
};

struct ThemePreview {
  Color background, panel, accent, text;
};

struct ThemeEntry {
  std::wstring id;
  std::wstring title;
  std::optional<ThemePreview> preview;
};

// The sample the candidate preview draws. It illustrates the layout, page size and font size; it is not a dictionary.
constexpr std::array<std::pair<const wchar_t *, const wchar_t *>, 9>
    preview_words{{{L"候选项", L"candidate"},
                   {L"后选项", L"choice"},
                   {L"侯选项", L"option"},
                   {L"候選項", L"candidate"},
                   {L"厚选项", L"item"},
                   {L"候选", L"candidate"},
                   {L"后选", L"choose"},
                   {L"候", L"wait"},
                   {L"后", L"after"}}};

struct MainWindow : WindowT<MainWindow> {
  explicit MainWindow(std::string page) : current_page_(std::move(page)) {
    Title(L"水杉输入法设置");
    ExtendsContentIntoTitleBar(true);
    reload_document();
    load_catalog();
    system_dark_ = system_dark();
    input_method_ = input_method_state();
    palette_ = make_palette(resolve_dark());
    // The window opens at the design's size in the system's scale; the user may resize it from there.
    const double scale = static_cast<double>(GetDpiForSystem()) / 96.0;
    AppWindow().Resize(
        {static_cast<int32_t>(std::lround(1100 * scale)),
         static_cast<int32_t>(std::lround(760 * scale))});
    build_shell(0.0);
    Activated({this, &MainWindow::on_activated});
    Closed([this](Inspectable const &, WindowEventArgs const &) {
      // A save refused while closing has no window left to report in; the stored settings stay as they were.
      closed_ = true;
      flush_pending();
    });
  }

private:
  // ---- Document and palette ----

  void reload_document() {
    PreferencesDocument fresh;
    std::wstring error;
    if (fresh.Load(error)) {
      document_ = std::move(fresh);
      loaded_ = true;
      load_error_.clear();
    } else if (!loaded_) {
      document_ = std::move(fresh);
      load_error_ = error;
    }
  }

  void load_catalog() {
    themes_.clear();
    auto response = take_response(msime_client_theme_catalog());
    if (!response.ok)
      return;
    try {
      const auto value =
          JsonObject::Parse(text(response.text)).GetNamedObject(L"value");
      for (auto const &item : value.GetNamedArray(L"themes")) {
        if (item.ValueType() != JsonValueType::Object)
          continue;
        const auto theme = item.GetObject();
        ThemeEntry entry;
        entry.id = theme.GetNamedString(L"id", L"").c_str();
        entry.title = theme.GetNamedString(L"title", L"").c_str();
        if (entry.id.empty() || entry.title.empty())
          continue;
        const auto preview =
            theme.GetNamedValue(L"preview", JsonValue::CreateNullValue());
        if (preview.ValueType() == JsonValueType::Object) {
          const auto colors = preview.GetObject();
          auto color = [&colors](const wchar_t *key) {
            return parse_hex(colors.GetNamedString(key, L"").c_str());
          };
          const auto background = color(L"background");
          const auto panel = color(L"panel");
          const auto accent = color(L"accent");
          const auto foreground = color(L"text");
          if (background && panel && accent && foreground)
            entry.preview =
                ThemePreview{*background, *panel, *accent, *foreground};
        }
        themes_.push_back(std::move(entry));
      }
    } catch (hresult_error const &) {
      themes_.clear();
    }
  }

  bool system_dark() {
    const auto background = ui_settings_.GetColorValue(
        Windows::UI::ViewManagement::UIColorType::Background);
    return background.R < 128;
  }

  // 设置界面主题 overrides 主题模式 for this window only; 跟随系统 follows Windows.
  bool resolve_dark() {
    const auto settings = document_.String(L"settings_theme", L"follow");
    if (settings == L"dark")
      return true;
    if (settings == L"light")
      return false;
    const auto theme = document_.String(L"theme", L"system");
    if (theme == L"dark")
      return true;
    if (theme == L"light")
      return false;
    return system_dark_;
  }

  // The candidate window's own light/dark mode, decided as the Server decides it (candidate_theme_dark in src/candidate/CandidateThemeSettings.h): the 候选窗口主题 override, else 主题模式.
  bool candidate_dark() const {
    const auto surface = document_.String(L"candidate_theme", L"follow");
    if (surface == L"dark" || surface == L"light")
      return surface == L"dark";
    const auto theme = document_.String(L"theme", L"system");
    return theme == L"system" ? system_dark_ : theme != L"light";
  }

  // The candidate palette the Server draws for a global theme. The request is the one the Server sends (candidate_theme_request): the stored custom theme without its keyboard design, the candidate window's mode and layout, and the skin root, so a custom theme's candidate_colors and skin package reach the preview exactly as they reach the real window. A request the shared layer refuses, or a response this reader does not recognise, draws the native tokens, as the Server does. The last answer is kept for its request, since the preview redraws on every slider step.
  msime::windows::CandidatePalette candidate_theme(std::wstring const &id) {
    const bool dark = candidate_dark();
    JsonObject request;
    request.SetNamedValue(L"global_theme",
                          JsonValue::CreateStringValue(hstring(id)));
    if (const auto custom = document_.Value(L"custom_theme");
        custom && custom.ValueType() == JsonValueType::Object) {
      auto copy = JsonObject::Parse(custom.Stringify());
      if (copy.HasKey(L"keyboard"))
        copy.Remove(L"keyboard");
      request.SetNamedValue(L"custom_theme", copy);
    }
    request.SetNamedValue(L"dark", JsonValue::CreateBooleanValue(dark));
    request.SetNamedValue(
        L"layout",
        JsonValue::CreateStringValue(
            document_.String(L"candidate_layout", L"vertical") == L"horizontal"
                ? L"horizontal"
                : L"vertical"));
    if (const auto state = state_directory(); state.is_absolute())
      if (const auto skins = path_utf8(state / L"skins"); !skins.empty())
        request.SetNamedValue(L"skins_directory",
                              JsonValue::CreateStringValue(text(skins)));
    auto body = utf8(request.Stringify());
    if (resolved_theme_ && body == resolved_request_)
      return *resolved_theme_;

    auto palette = msime::windows::candidate_native_palette(dark);
    auto response = take_response(msime_client_resolve_theme(
        reinterpret_cast<const uint8_t *>(body.data()), body.size()));
    if (response.ok) {
      try {
        const auto value =
            JsonObject::Parse(text(response.text)).GetNamedObject(L"value");
        std::optional<bool> theme_dark;
        const auto appearance =
            value.GetNamedValue(L"appearance", JsonValue::CreateNullValue());
        if (appearance.ValueType() == JsonValueType::String) {
          if (appearance.GetString() == L"dark")
            theme_dark = true;
          else if (appearance.GetString() == L"light")
            theme_dark = false;
        }
        msime::windows::CandidatePaletteOverrides slots;
        const auto candidate =
            value.GetNamedValue(L"candidate", JsonValue::CreateNullValue());
        if (candidate.ValueType() == JsonValueType::Object) {
          const auto colors = candidate.GetObject();
          auto slot = [&colors](const wchar_t *key,
                                std::optional<std::string> &target) {
            const auto item =
                colors.GetNamedValue(key, JsonValue::CreateNullValue());
            if (item.ValueType() == JsonValueType::String)
              target = utf8(item.GetString());
          };
          slot(L"surface", slots.surface);
          slot(L"border", slots.border);
          slot(L"text", slots.text);
          slot(L"number", slots.number);
          slot(L"accent", slots.accent);
          slot(L"selected", slots.selected);
          slot(L"selected_text", slots.selected_text);
          slot(L"selected_number", slots.selected_number);
          slot(L"hover", slots.hover);
          slot(L"secondary", slots.secondary);
          const auto bar = colors.GetNamedValue(L"show_selected_bar",
                                                JsonValue::CreateNullValue());
          if (bar.ValueType() == JsonValueType::Boolean)
            slots.show_selected_bar = bar.GetBoolean();
        }
        palette = msime::windows::candidate_palette(
            slots, msime::windows::candidate_native_palette(
                       theme_dark.value_or(dark)));
      } catch (hresult_error const &) {
        palette = msime::windows::candidate_native_palette(dark);
      }
    }
    resolved_request_ = std::move(body);
    resolved_theme_ = palette;
    return palette;
  }

  // Re-reads the palette and redraws: the whole window when the palette changed, otherwise the current page.
  void refresh() {
    const bool dark = resolve_dark();
    if (dark != palette_.dark) {
      palette_ = make_palette(dark);
      rebuild_shell();
      return;
    }
    render_page(true);
  }

  // Rebuilds the window in a new palette and carries over where the user was: the scroll position, the text in the title bar search and the sidebar filter. Setting the text does not open the suggestion list, which only answers typing.
  void rebuild_shell() {
    const double offset = scroll_ ? scroll_.VerticalOffset() : 0.0;
    const hstring query = search_ ? search_.Text() : hstring();
    const hstring filter = filter_ ? filter_.Text() : hstring();
    build_shell(offset);
    search_.Text(query);
    filter_.Text(filter);
    filter_navigation(std::wstring(filter.c_str()));
  }

  void on_activated(Inspectable const &, WindowActivatedEventArgs const &args) {
    if (args.WindowActivationState() == WindowActivationState::Deactivated) {
      flush_pending();
      return;
    }
    // Another window (the tray, the shared app, another device through sync) may have changed the settings or Windows may have switched between light and dark while this window was in the background.
    bool changed = false;
    PreferencesDocument fresh;
    std::wstring error;
    if (fresh.Load(error) &&
        (!loaded_ || fresh.revision() != document_.revision())) {
      document_ = std::move(fresh);
      loaded_ = true;
      load_error_.clear();
      changed = true;
    }
    if (const bool dark = system_dark(); dark != system_dark_) {
      system_dark_ = dark;
      changed = true;
    }
    // The keyboard list is edited in Windows Settings, which is where the banner's fallback sends the user.
    if (const auto state = input_method_state(); state != input_method_) {
      input_method_ = state;
      changed = true;
    }
    // A skin package may have been edited in the shared app; its colours are read again on the next preview.
    resolved_theme_.reset();
    if (!changed || !root_)
      return;
    // A click that activates the window is still on its way to a control: the press and release arrive after this event, and a redraw between them would replace the control under the pointer and drop the click. That redraw waits for the release. Every other activation redraws once this event has returned.
    if (args.WindowActivationState() == WindowActivationState::PointerActivated) {
      defer_refresh_until_pointer_up();
      return;
    }
    queue_refresh();
  }

  void defer_refresh_until_pointer_up() {
    refresh_after_pointer_ = true;
    // A click on the title bar's drag region or caption buttons reaches no XAML element, so no release would ever arrive: the timer redraws once it is clear that no press followed in the window's content.
    if (!pointer_timer_) {
      pointer_timer_ = DispatcherQueue().CreateTimer();
      pointer_timer_.Interval(std::chrono::milliseconds(400));
      pointer_timer_.IsRepeating(false);
      pointer_timer_.Tick([this](Microsoft::UI::Dispatching::DispatcherQueueTimer const &,
                                 Inspectable const &) {
        if (!pointer_down_)
          release_deferred_refresh();
      });
    }
    pointer_timer_.Stop();
    pointer_timer_.Start();
  }

  void release_deferred_refresh() {
    if (!refresh_after_pointer_)
      return;
    refresh_after_pointer_ = false;
    if (pointer_timer_)
      pointer_timer_.Stop();
    queue_refresh();
  }

  // Tracks the pointer over the whole window, including presses and releases a control has already handled (a button handles its own), so a deferred redraw runs only after the control has seen its click.
  void track_pointer(UIElement const &root) {
    using Microsoft::UI::Xaml::Input::PointerEventHandler;
    using Microsoft::UI::Xaml::Input::PointerRoutedEventArgs;
    root.AddHandler(UIElement::PointerPressedEvent(),
                    box_value(PointerEventHandler(
                        [this](Inspectable const &, PointerRoutedEventArgs const &) {
                          pointer_down_ = true;
                        })),
                    true);
    const auto released = box_value(PointerEventHandler(
        [this](Inspectable const &, PointerRoutedEventArgs const &) {
          pointer_down_ = false;
          release_deferred_refresh();
        }));
    root.AddHandler(UIElement::PointerReleasedEvent(), released, true);
    root.AddHandler(UIElement::PointerCanceledEvent(), released, true);
    root.AddHandler(UIElement::PointerCaptureLostEvent(), released, true);
  }

  // ---- Saving ----

  // Every control writes through at once. A refused write (a newer revision elsewhere, or a combination the store rejects) reloads the stored settings and says so.
  bool commit() {
    std::wstring error;
    if (document_.Save(error))
      return true;
    notice_ = L"无法保存这项设置：设置可能已在其他窗口中更新，或与其他设置冲突。"
              L"已重新读取当前设置。";
    notice_severity_ = InfoBarSeverity::Error;
    // The refused edit is still in memory; the stored settings replace it, and if they cannot be read the controls are disabled rather than showing a value that was never saved.
    PreferencesDocument fresh;
    std::wstring reload_error;
    const bool reloaded = fresh.Load(reload_error);
    document_ = std::move(fresh);
    loaded_ = reloaded;
    load_error_ = reloaded ? std::wstring() : reload_error;
    queue_refresh();
    return false;
  }

  void change(std::function<void(PreferencesDocument &)> const &edit,
              bool redraw) {
    flush_pending();
    edit(document_);
    if (commit() && redraw)
      queue_refresh();
  }

  // Redraws after the current event handler returns, so a control is never torn down inside its own event.
  void queue_refresh() {
    if (refresh_queued_ || closed_)
      return;
    refresh_queued_ = true;
    DispatcherQueue().TryEnqueue([weak = get_weak()] {
      if (auto self = weak.get()) {
        self->refresh_queued_ = false;
        self->refresh();
      }
    });
  }

  // Sliders write the document as they move and save once they rest.
  void schedule_save() {
    pending_save_ = true;
    if (!save_timer_) {
      save_timer_ = DispatcherQueue().CreateTimer();
      save_timer_.Interval(std::chrono::milliseconds(350));
      save_timer_.IsRepeating(false);
      save_timer_.Tick([this](Microsoft::UI::Dispatching::DispatcherQueueTimer const &,
                              Inspectable const &) { flush_pending(); });
    }
    save_timer_.Stop();
    save_timer_.Start();
  }

  void flush_pending() {
    if (save_timer_)
      save_timer_.Stop();
    if (!pending_save_)
      return;
    pending_save_ = false;
    commit();
  }

  // ---- Window chrome ----

  void build_shell(double offset) {
    Grid root;
    root.RequestedTheme(palette_.dark ? ElementTheme::Dark : ElementTheme::Light);
    root.Background(brush(palette_.window_bg));
    RowDefinition title_row;
    title_row.Height(GridLength{48, GridUnitType::Pixel});
    RowDefinition body_row;
    body_row.Height(GridLength{1, GridUnitType::Star});
    root.RowDefinitions().Append(title_row);
    root.RowDefinitions().Append(body_row);

    titlebar_ = make_titlebar();
    Grid::SetRow(titlebar_, 0);
    root.Children().Append(titlebar_);

    navigation_ = make_navigation();
    Grid::SetRow(navigation_, 1);
    root.Children().Append(navigation_);

    track_pointer(root);
    root_ = root;
    Content(root);
    SetTitleBar(titlebar_);
    style_caption_buttons();
    select_navigation_item();
    render_page_at(offset);
  }

  void style_caption_buttons() {
    auto bar = AppWindow().TitleBar();
    bar.PreferredHeightOption(Microsoft::UI::Windowing::TitleBarHeightOption::Tall);
    bar.ButtonBackgroundColor(transparent_color());
    bar.ButtonInactiveBackgroundColor(transparent_color());
    bar.ButtonForegroundColor(palette_.text);
    bar.ButtonInactiveForegroundColor(palette_.faint);
    bar.ButtonHoverBackgroundColor(palette_.subtle);
    bar.ButtonHoverForegroundColor(palette_.text);
    bar.ButtonPressedBackgroundColor(palette_.subtle2);
    bar.ButtonPressedForegroundColor(palette_.text);
  }

  Grid make_titlebar() {
    Grid bar;
    bar.Height(48);
    ColumnDefinition lead;
    lead.Width(GridLength{1, GridUnitType::Auto});
    ColumnDefinition middle;
    middle.Width(GridLength{1, GridUnitType::Star});
    ColumnDefinition caption;
    caption.Width(GridLength{140, GridUnitType::Pixel});
    bar.ColumnDefinitions().Append(lead);
    bar.ColumnDefinitions().Append(middle);
    bar.ColumnDefinitions().Append(caption);

    StackPanel identity;
    identity.Orientation(Orientation::Horizontal);
    identity.Padding(Thickness{12, 0, 0, 0});
    identity.VerticalAlignment(VerticalAlignment::Center);

    back_ = Button();
    back_.Width(40);
    back_.Height(36);
    back_.Padding(Thickness{0, 0, 0, 0});
    back_.CornerRadius(CornerRadius{4, 4, 4, 4});
    back_.Background(brush(transparent_color()));
    back_.BorderThickness(Thickness{0, 0, 0, 0});
    back_.Content(make_icon(0xE72B, 12, palette_.text));
    back_.IsEnabled(!history_.empty());
    A11y::SetName(back_, L"返回");
    back_.Click([this](Inspectable const &, RoutedEventArgs const &) {
      go_back();
    });
    identity.Children().Append(back_);

    Image logo;
    logo.Width(18);
    logo.Height(18);
    logo.Margin(Thickness{8, 0, 12, 0});
    logo.VerticalAlignment(VerticalAlignment::Center);
    show_logo(logo, 36);
    identity.Children().Append(logo);

    auto product = make_text(L"水杉输入法", 12, palette_.text);
    product.VerticalAlignment(VerticalAlignment::Center);
    identity.Children().Append(product);
    auto section = make_text(L"设置", 12, palette_.sub);
    section.Margin(Thickness{6, 0, 0, 0});
    section.VerticalAlignment(VerticalAlignment::Center);
    identity.Children().Append(section);
    Grid::SetColumn(identity, 0);
    bar.Children().Append(identity);

    search_ = AutoSuggestBox();
    search_.PlaceholderText(L"搜索");
    search_.QueryIcon(SymbolIcon(Symbol::Find));
    search_.MaxWidth(360);
    search_.Height(32);
    search_.Margin(Thickness{16, 0, 16, 0});
    search_.HorizontalAlignment(HorizontalAlignment::Stretch);
    search_.VerticalAlignment(VerticalAlignment::Center);
    A11y::SetName(search_, L"搜索设置");
    search_.TextChanged([this](AutoSuggestBox const &sender,
                               AutoSuggestBoxTextChangedEventArgs const &args) {
      const auto reason = args.Reason();
      if (reason == AutoSuggestionBoxTextChangeReason::UserInput) {
        search_query_ = sender.Text().c_str();
        update_suggestions(search_query_);
      } else if (reason == AutoSuggestionBoxTextChangeReason::SuggestionChosen &&
                 std::wstring_view(sender.Text()) == no_search_results) {
        // Moving onto the no-results line would put it in the box in place of what was typed.
        sender.Text(hstring(search_query_));
      }
    });
    search_.QuerySubmitted(
        [this](AutoSuggestBox const &,
               AutoSuggestBoxQuerySubmittedEventArgs const &args) {
          std::wstring chosen;
          if (const auto item = args.ChosenSuggestion())
            chosen = unbox_value_or<hstring>(item, L"").c_str();
          if (std::wstring_view(chosen) == no_search_results)
            return;
          open_search_result(chosen, std::wstring(args.QueryText().c_str()));
        });
    Grid::SetColumn(search_, 1);
    bar.Children().Append(search_);

    bar.Loaded([this](Inspectable const &, RoutedEventArgs const &) {
      update_passthrough();
    });
    bar.SizeChanged([this](Inspectable const &, SizeChangedEventArgs const &) {
      update_passthrough();
    });
    return bar;
  }

  // The title bar is a drag region; the back button and the search box take their own clicks.
  void update_passthrough() {
    if (!titlebar_ || !titlebar_.XamlRoot())
      return;
    const double scale = titlebar_.XamlRoot().RasterizationScale();
    std::vector<Windows::Graphics::RectInt32> rects;
    auto add = [&rects, scale](FrameworkElement const &element) {
      if (!element || element.ActualWidth() <= 0)
        return;
      const auto bounds = element.TransformToVisual(nullptr).TransformBounds(
          Windows::Foundation::Rect{0, 0,
                                    static_cast<float>(element.ActualWidth()),
                                    static_cast<float>(element.ActualHeight())});
      rects.push_back(Windows::Graphics::RectInt32{
          static_cast<int32_t>(std::lround(bounds.X * scale)),
          static_cast<int32_t>(std::lround(bounds.Y * scale)),
          static_cast<int32_t>(std::lround(bounds.Width * scale)),
          static_cast<int32_t>(std::lround(bounds.Height * scale))});
    };
    add(back_);
    add(search_);
    Microsoft::UI::Input::InputNonClientPointerSource::GetForWindowId(
        AppWindow().Id())
        .SetRegionRects(Microsoft::UI::Input::NonClientRegionKind::Passthrough,
                        rects);
  }

  NavigationView make_navigation() {
    NavigationView view;
    view.PaneDisplayMode(NavigationViewPaneDisplayMode::Left);
    view.OpenPaneLength(280);
    view.IsPaneToggleButtonVisible(false);
    view.IsBackButtonVisible(NavigationViewBackButtonVisible::Collapsed);
    view.IsSettingsVisible(false);
    view.IsTitleBarAutoPaddingEnabled(false);
    view.AlwaysShowHeader(false);

    // Lightweight styling: the template reads these brushes, so the sidebar and the content surface take the design's palette.
    auto resources = view.Resources();
    auto put = [&resources](const wchar_t *key, Color const &color) {
      resources.Insert(box_value(hstring(key)), brush(color));
    };
    put(L"NavigationViewContentBackground", palette_.content_bg);
    put(L"NavigationViewContentGridBorderBrush", palette_.edge);
    put(L"NavigationViewExpandedPaneBackground", transparent_color());
    put(L"NavigationViewDefaultPaneBackground", transparent_color());
    put(L"NavigationViewSelectionIndicatorForeground", palette_.accent);
    put(L"NavigationViewItemBackgroundSelected", palette_.subtle);
    put(L"NavigationViewItemBackgroundSelectedPointerOver", palette_.subtle2);
    put(L"NavigationViewItemBackgroundPointerOver", palette_.subtle);
    put(L"NavigationViewItemForegroundSelected", palette_.text);
    put(L"NavigationViewItemSeparatorForeground", palette_.divider);

    filter_ = AutoSuggestBox();
    filter_.PlaceholderText(L"搜索");
    filter_.QueryIcon(SymbolIcon(Symbol::Find));
    A11y::SetName(filter_, L"筛选页面");
    filter_.TextChanged([this](AutoSuggestBox const &sender,
                               AutoSuggestBoxTextChangedEventArgs const &) {
      filter_navigation(std::wstring(sender.Text().c_str()));
    });
    filter_.QuerySubmitted([this](AutoSuggestBox const &,
                                  AutoSuggestBoxQuerySubmittedEventArgs const &) {
      for (auto const &value : navigation_.MenuItems()) {
        const auto item = value.try_as<NavigationViewItem>();
        if (item && item.Visibility() == Visibility::Visible) {
          navigate(utf8(unbox_value<hstring>(item.Tag())), true);
          return;
        }
      }
    });
    view.AutoSuggestBox(filter_);

    std::size_t group = 0;
    for (const auto &page : nav::pages) {
      if (page.group != group) {
        group = page.group;
        view.MenuItems().Append(NavigationViewItemSeparator());
      }
      const auto &label = page_label(page.id);
      NavigationViewItem item;
      item.Content(box_value(hstring(label.title)));
      item.Icon(make_icon(label.glyph, 16, palette_.text));
      item.Tag(box_value(text(page.id)));
      view.MenuItems().Append(item);
    }
    view.SelectionChanged(
        [this](NavigationView const &sender,
               NavigationViewSelectionChangedEventArgs const &) {
          if (selecting_)
            return;
          const auto item = sender.SelectedItem().try_as<NavigationViewItem>();
          if (item)
            navigate(utf8(unbox_value<hstring>(item.Tag())), true);
        });

    scroll_ = ScrollViewer();
    scroll_.VerticalScrollBarVisibility(ScrollBarVisibility::Auto);
    scroll_.HorizontalScrollBarVisibility(ScrollBarVisibility::Disabled);
    view.Content(scroll_);
    return view;
  }

  void filter_navigation(std::wstring const &query) {
    const auto needle = lowercase(query);
    for (auto const &value : navigation_.MenuItems()) {
      if (const auto item = value.try_as<NavigationViewItem>()) {
        const auto label = lowercase(
            unbox_value_or<hstring>(item.Content(), L"").c_str());
        item.Visibility(needle.empty() || label.find(needle) != std::wstring::npos
                            ? Visibility::Visible
                            : Visibility::Collapsed);
      } else if (const auto separator =
                     value.try_as<NavigationViewItemSeparator>()) {
        separator.Visibility(needle.empty() ? Visibility::Visible
                                            : Visibility::Collapsed);
      }
    }
  }

  void select_navigation_item() {
    for (auto const &value : navigation_.MenuItems()) {
      const auto item = value.try_as<NavigationViewItem>();
      if (item && utf8(unbox_value<hstring>(item.Tag())) == current_page_) {
        selecting_ = true;
        navigation_.SelectedItem(item);
        selecting_ = false;
        return;
      }
    }
  }

  void navigate(std::string const &page, bool remember) {
    const auto *target = nav::find_page(page);
    const std::string id(target ? target->id : nav::default_page);
    flush_pending();
    if (id == current_page_ && focus_row_.empty())
      return;
    if (remember && id != current_page_)
      history_.push_back(current_page_);
    current_page_ = id;
    select_navigation_item();
    render_page(false);
  }

  void go_back() {
    if (history_.empty())
      return;
    const auto page = history_.back();
    history_.pop_back();
    navigate(page, false);
  }

  // ---- Search ----

  void rebuild_search_index() {
    search_index_.clear();
    indexing_ = true;
    const auto saved_page = current_page_;
    for (const auto &page : nav::pages) {
      current_page_ = std::string(page.id);
      StackPanel scratch;
      build_page(page.id, scratch);
    }
    current_page_ = saved_page;
    indexing_ = false;
    index_revision_ = document_.revision();
    index_valid_ = true;
  }

  void record(std::wstring const &title, std::wstring const &detail) {
    if (!indexing_ || title.empty())
      return;
    SearchEntry entry;
    entry.title = title;
    entry.detail = detail;
    entry.page = current_page_;
    entry.display = title + L" · " + page_label(current_page_).title;
    search_index_.push_back(std::move(entry));
  }

  // The element a search result lands on: the first one drawn under the chosen title. Every element that records a search entry marks itself here, so a check box, a theme card or a section is scrolled to and focused like a settings card.
  void mark_search_target(std::wstring const &title,
                          FrameworkElement const &element) {
    if (!indexing_ && !focus_row_.empty() && title == focus_row_ &&
        !focus_target_)
      focus_target_ = element;
  }

  std::vector<SearchEntry const *> search(std::wstring const &query) {
    if (!index_valid_ || index_revision_ != document_.revision())
      rebuild_search_index();
    std::vector<SearchEntry const *> matches;
    const auto needle = lowercase(query);
    if (needle.empty())
      return matches;
    for (const auto &entry : search_index_) {
      const bool hit =
          lowercase(entry.title).find(needle) != std::wstring::npos ||
          lowercase(entry.detail).find(needle) != std::wstring::npos ||
          lowercase(page_label(entry.page).title).find(needle) !=
              std::wstring::npos;
      if (hit)
        matches.push_back(&entry);
      if (matches.size() == 12)
        break;
    }
    return matches;
  }

  void update_suggestions(std::wstring const &query) {
    std::vector<Inspectable> items;
    for (const auto *entry : search(query))
      items.push_back(box_value(hstring(entry->display)));
    if (items.empty() && !query.empty())
      items.push_back(box_value(hstring(no_search_results)));
    search_.ItemsSource(single_threaded_vector<Inspectable>(std::move(items)));
  }

  void open_search_result(std::wstring const &chosen, std::wstring const &query) {
    const auto matches = search(query.empty() ? chosen : query);
    SearchEntry const *target = nullptr;
    for (const auto *entry : matches)
      if (entry->display == chosen)
        target = entry;
    if (!target && !chosen.empty())
      for (const auto &entry : search_index_)
        if (entry.display == chosen)
          target = &entry;
    if (!target && !matches.empty())
      target = matches.front();
    if (!target)
      return;
    focus_row_ = target->title;
    const auto page = target->page;
    if (page == current_page_) {
      render_page(true);
      return;
    }
    navigate(page, true);
  }

  // ---- Page scaffolding ----

  void render_page(bool keep_offset) {
    if (!scroll_)
      return;
    render_page_at(keep_offset ? scroll_.VerticalOffset() : 0.0);
  }

  // Draws the current page and scrolls to `offset`, or to the element a search result named.
  void render_page_at(double offset) {
    if (!scroll_)
      return;
    back_.IsEnabled(!history_.empty());
    preview_host_ = nullptr;
    focus_target_ = nullptr;

    StackPanel page;
    page.Padding(Thickness{56, 36, 56, 48});
    page.Spacing(4);
    if (input_method_ == InputMethodState::missing ||
        input_method_ == InputMethodState::not_default)
      page.Children().Append(make_input_method_banner());
    page.Children().Append(make_page_title());
    if (!loaded_)
      page.Children().Append(make_load_failure());
    if (!notice_.empty())
      page.Children().Append(make_notice());
    build_page(current_page_, page);
    scroll_.Content(page);

    // Weak references: the page owns this handler, so a strong one would keep the page alive through a cycle.
    const bool focus = static_cast<bool>(focus_target_);
    weak_ref<FrameworkElement> weak_target;
    if (focus)
      weak_target = make_weak(focus_target_);
    focus_row_.clear();
    page.Loaded([weak_scroll = make_weak(scroll_), offset, focus,
                 weak_target](Inspectable const &, RoutedEventArgs const &) {
      if (focus) {
        if (auto target = weak_target.get()) {
          target.StartBringIntoView();
          // A control (a check box, a theme card) also takes keyboard focus, so its focus rectangle shows which one the result named; a card is only scrolled to.
          if (const auto control = target.try_as<Control>())
            control.Focus(FocusState::Keyboard);
        }
        return;
      }
      if (auto scroll = weak_scroll.get()) {
        scroll.UpdateLayout();
        scroll.ChangeView(nullptr, offset, nullptr, true);
      }
    });
  }

  FrameworkElement make_page_title() {
    Grid row;
    row.MinHeight(40);
    row.Margin(Thickness{0, 0, 0, 24});
    ColumnDefinition title_column;
    title_column.Width(GridLength{1, GridUnitType::Star});
    ColumnDefinition tools_column;
    tools_column.Width(GridLength{1, GridUnitType::Auto});
    row.ColumnDefinitions().Append(title_column);
    row.ColumnDefinitions().Append(tools_column);

    auto title = make_text(page_label(current_page_).title, 28, palette_.text);
    title.FontWeight(Windows::UI::Text::FontWeight{600});
    title.VerticalAlignment(VerticalAlignment::Center);
    Grid::SetColumn(title, 0);
    row.Children().Append(title);

    if (has_preview(current_page_)) {
      StackPanel tools;
      tools.Orientation(Orientation::Horizontal);
      tools.Spacing(20);
      tools.VerticalAlignment(VerticalAlignment::Center);
      StackPanel switcher;
      switcher.Orientation(Orientation::Horizontal);
      switcher.Spacing(12);
      auto caption = make_text(L"界面预览", 14, palette_.sub);
      caption.VerticalAlignment(VerticalAlignment::Center);
      switcher.Children().Append(caption);
      ToggleSwitch toggle;
      toggle.OnContent(box_value(L""));
      toggle.OffContent(box_value(L""));
      toggle.MinWidth(0);
      toggle.IsOn(preview_visible_);
      A11y::SetName(toggle, L"界面预览");
      toggle.Toggled([this](Inspectable const &sender, RoutedEventArgs const &) {
        preview_visible_ = sender.as<ToggleSwitch>().IsOn();
        queue_refresh();
      });
      switcher.Children().Append(toggle);
      tools.Children().Append(switcher);
      if (current_page_ != "themes") {
        Button edit;
        edit.Height(32);
        edit.Padding(Thickness{11, 0, 11, 0});
        edit.Background(brush(transparent_color()));
        edit.BorderThickness(Thickness{0, 0, 0, 0});
        StackPanel label;
        label.Orientation(Orientation::Horizontal);
        label.Spacing(8);
        label.Children().Append(make_icon(0xE790, 14, palette_.text));
        label.Children().Append(make_text(L"编辑主题", 14, palette_.text));
        edit.Content(label);
        A11y::SetName(edit, L"编辑主题");
        edit.Click([this](Inspectable const &, RoutedEventArgs const &) {
          navigate("themes", true);
        });
        tools.Children().Append(edit);
      }
      Grid::SetColumn(tools, 1);
      row.Children().Append(tools);
    }
    return row;
  }

  static bool has_preview(std::string_view page) {
    return page == "typing" || page == "expression" || page == "candidate" ||
           page == "themes";
  }

  FrameworkElement make_load_failure() {
    InfoBar bar;
    bar.IsOpen(true);
    bar.IsClosable(false);
    bar.Severity(InfoBarSeverity::Error);
    bar.Title(L"无法读取设置");
    bar.Message(hstring(load_error_.empty() ? L"读取设置失败，请稍后重试。"
                                            : load_error_));
    bar.Margin(Thickness{0, 0, 0, 12});
    Button retry;
    retry.Content(box_value(L"重试"));
    retry.Click([this](Inspectable const &, RoutedEventArgs const &) {
      reload_document();
      queue_refresh();
    });
    bar.ActionButton(retry);
    return bar;
  }

  // The design's deskStrip: an orange notice above the page title while the input method is not in the keyboard list, or is not the default one, with the button that fixes it.
  FrameworkElement make_input_method_banner() {
    const bool missing = input_method_ == InputMethodState::missing;
    const std::wstring message = missing ? L"水杉输入法尚未在「语言和区域」中添加"
                                         : L"水杉输入法还不是默认输入法";
    const std::wstring action_label = missing ? L"去添加" : L"设为默认";

    Border strip;
    strip.Padding(Thickness{16, 10, 16, 10});
    strip.Margin(Thickness{0, 0, 0, 16});
    strip.CornerRadius(CornerRadius{4, 4, 4, 4});
    strip.BorderThickness(Thickness{1, 1, 1, 1});
    strip.Background(brush(palette_.dark ? rgba(255, 149, 0, .12)
                                         : rgba(0xFF, 0xF4, 0xE5, 1)));
    strip.BorderBrush(brush(palette_.dark ? rgba(255, 149, 0, .3)
                                          : rgba(0xFF, 0xD8, 0xA8, 1)));

    Grid row;
    row.ColumnSpacing(12);
    ColumnDefinition mark_column;
    mark_column.Width(GridLength{1, GridUnitType::Auto});
    ColumnDefinition text_column;
    text_column.Width(GridLength{1, GridUnitType::Star});
    ColumnDefinition action_column;
    action_column.Width(GridLength{1, GridUnitType::Auto});
    row.ColumnDefinitions().Append(mark_column);
    row.ColumnDefinitions().Append(text_column);
    row.ColumnDefinitions().Append(action_column);

    Border mark;
    mark.Width(18);
    mark.Height(18);
    mark.CornerRadius(CornerRadius{9, 9, 9, 9});
    mark.Background(brush(rgba(0xFF, 0x95, 0x00, 1)));
    mark.VerticalAlignment(VerticalAlignment::Center);
    auto bang = make_text(L"!", 12, rgba(255, 255, 255, 1));
    bang.FontWeight(Windows::UI::Text::FontWeight{700});
    bang.HorizontalAlignment(HorizontalAlignment::Center);
    bang.VerticalAlignment(VerticalAlignment::Center);
    mark.Child(bang);
    A11y::SetAccessibilityView(mark,
                               Microsoft::UI::Xaml::Automation::Peers::AccessibilityView::Raw);
    Grid::SetColumn(mark, 0);
    row.Children().Append(mark);

    auto text_block = make_text(message, 14, palette_.text);
    text_block.VerticalAlignment(VerticalAlignment::Center);
    Grid::SetColumn(text_block, 1);
    row.Children().Append(text_block);

    // The fixed accent fill of the design, kept through hover and press rather than the system accent the default button styles use.
    Button action;
    action.Content(box_value(hstring(action_label)));
    action.FontSize(13);
    action.Padding(Thickness{12, 4, 12, 4});
    action.CornerRadius(CornerRadius{4, 4, 4, 4});
    action.BorderThickness(Thickness{0, 0, 0, 0});
    action.VerticalAlignment(VerticalAlignment::Center);
    auto resources = action.Resources();
    auto put = [&resources](const wchar_t *key, Color const &color) {
      resources.Insert(box_value(hstring(key)), brush(color));
    };
    const auto accent = palette_.accent;
    auto faded = [accent](double alpha) {
      return Color{static_cast<uint8_t>(std::lround(alpha * 255)), accent.R,
                   accent.G, accent.B};
    };
    put(L"ButtonBackground", accent);
    put(L"ButtonBackgroundPointerOver", faded(.9));
    put(L"ButtonBackgroundPressed", faded(.8));
    put(L"ButtonForeground", palette_.on_accent);
    put(L"ButtonForegroundPointerOver", palette_.on_accent);
    put(L"ButtonForegroundPressed", palette_.on_accent);
    A11y::SetName(action, hstring(action_label));
    A11y::SetHelpText(action, hstring(message));
    action.Click([this, missing](Inspectable const &, RoutedEventArgs const &) {
      if (missing) {
        if (!add_input_method()) {
          // Windows refused (a policy, or a profile that is not registered): the user adds it by hand in 语言和区域.
          const auto result = reinterpret_cast<INT_PTR>(
              ShellExecuteW(nullptr, L"open", L"ms-settings:regionlanguage",
                            nullptr, nullptr, SW_SHOWNORMAL));
          if (result <= 32)
            show_notice(L"无法添加水杉输入法。请在「设置 → 时间和语言 → 语言和区域」中为中文添加水杉输入法。");
        }
      } else if (!make_input_method_default()) {
        show_notice(L"无法设为默认输入法。请在「设置 → 时间和语言 → 输入 → 高级键盘设置」中选择水杉输入法。");
      }
      input_method_ = input_method_state();
      queue_refresh();
    });
    Grid::SetColumn(action, 2);
    row.Children().Append(action);

    strip.Child(row);
    A11y::SetName(strip, hstring(message));
    return strip;
  }

  FrameworkElement make_notice() {
    InfoBar bar;
    bar.IsOpen(true);
    bar.IsClosable(true);
    bar.Severity(notice_severity_);
    bar.Message(hstring(notice_));
    bar.Margin(Thickness{0, 0, 0, 12});
    bar.Closed([this](InfoBar const &, InfoBarClosedEventArgs const &) {
      notice_.clear();
      notice_severity_ = InfoBarSeverity::Error;
    });
    return bar;
  }

  void show_notice(std::wstring message,
                   InfoBarSeverity severity = InfoBarSeverity::Error) {
    notice_ = std::move(message);
    notice_severity_ = severity;
    queue_refresh();
  }

  StackPanel add_group(StackPanel const &page, std::wstring const &title) {
    StackPanel group;
    group.Spacing(4);
    if (!title.empty()) {
      auto heading = make_text(title, 20, palette_.text);
      heading.FontWeight(Windows::UI::Text::FontWeight{600});
      heading.Padding(Thickness{4, 28, 4, 8});
      group.Children().Append(heading);
    }
    page.Children().Append(group);
    return group;
  }

  Border make_card() {
    Border card;
    card.Background(brush(palette_.card));
    card.BorderBrush(brush(palette_.stroke));
    card.BorderThickness(Thickness{1, 1, 1, 1});
    card.CornerRadius(CornerRadius{3, 3, 3, 3});
    return card;
  }

  // One Fluent settings card: a 20px icon, the title and its explanation, and the control on the right. `below` holds the check lists that open under the header.
  Border add_row(StackPanel const &group, wchar_t glyph,
                 std::wstring const &title, std::wstring const &subtitle,
                 FrameworkElement const &control,
                 FrameworkElement const &below = nullptr) {
    record(title, subtitle);
    Grid header;
    header.MinHeight(62);
    header.Padding(Thickness{25, 10, 25, 10});
    header.ColumnSpacing(16);
    ColumnDefinition icon_column;
    icon_column.Width(GridLength{20, GridUnitType::Pixel});
    ColumnDefinition text_column;
    text_column.Width(GridLength{1, GridUnitType::Star});
    ColumnDefinition control_column;
    control_column.Width(GridLength{1, GridUnitType::Auto});
    header.ColumnDefinitions().Append(icon_column);
    header.ColumnDefinitions().Append(text_column);
    header.ColumnDefinitions().Append(control_column);

    auto icon = make_icon(glyph, 20, palette_.text);
    icon.VerticalAlignment(VerticalAlignment::Center);
    Grid::SetColumn(icon, 0);
    header.Children().Append(icon);

    StackPanel texts;
    texts.Spacing(1);
    texts.VerticalAlignment(VerticalAlignment::Center);
    texts.Children().Append(make_text(title, 14, palette_.text));
    if (!subtitle.empty())
      texts.Children().Append(make_text(subtitle, 12, palette_.sub));
    Grid::SetColumn(texts, 1);
    header.Children().Append(texts);

    if (control) {
      control.VerticalAlignment(VerticalAlignment::Center);
      Grid::SetColumn(control, 2);
      header.Children().Append(control);
    }

    StackPanel body;
    body.Children().Append(header);
    if (below)
      body.Children().Append(below);
    auto card = make_card();
    card.Child(body);
    group.Children().Append(card);
    mark_search_target(title, card);
    return card;
  }

  // ---- Controls ----

  TextBlock make_text(std::wstring_view value, double size, Color const &color) {
    TextBlock block;
    block.Text(hstring(value));
    block.FontSize(size);
    block.Foreground(brush(color));
    block.TextWrapping(TextWrapping::Wrap);
    return block;
  }

  FontIcon make_icon(wchar_t glyph, double size, Color const &color) {
    FontIcon icon;
    icon.FontFamily(Media::FontFamily(L"Segoe Fluent Icons,Segoe MDL2 Assets"));
    icon.Glyph(glyph_text(glyph));
    icon.FontSize(size);
    icon.Foreground(brush(color));
    return icon;
  }

  FrameworkElement toggle_control(std::wstring const &name, bool on,
                                  std::function<void(bool)> apply,
                                  bool enabled = true) {
    StackPanel box;
    box.Orientation(Orientation::Horizontal);
    box.Spacing(12);
    auto state = make_text(on ? L"开" : L"关", 14, palette_.text);
    state.Width(24);
    state.TextAlignment(TextAlignment::Right);
    state.VerticalAlignment(VerticalAlignment::Center);
    box.Children().Append(state);
    ToggleSwitch toggle;
    toggle.OnContent(box_value(L""));
    toggle.OffContent(box_value(L""));
    toggle.MinWidth(0);
    toggle.IsOn(on);
    toggle.IsEnabled(enabled && loaded_);
    A11y::SetName(toggle, hstring(name));
    toggle.Toggled([weak_state = make_weak(state), on_toggle = std::move(apply)](
                       Inspectable const &sender, RoutedEventArgs const &) {
      const bool value = sender.as<ToggleSwitch>().IsOn();
      if (auto label = weak_state.get())
        label.Text(value ? L"开" : L"关");
      on_toggle(value);
    });
    box.Children().Append(toggle);
    return box;
  }

  FrameworkElement select_control(std::wstring const &name,
                                  std::vector<Option> options,
                                  std::wstring const &current,
                                  std::function<void(std::wstring const &)> apply,
                                  bool enabled = true) {
    ComboBox combo;
    combo.Width(156);
    int32_t selected = -1;
    for (size_t i = 0; i < options.size(); ++i) {
      combo.Items().Append(box_value(hstring(options[i].label)));
      if (options[i].value == current)
        selected = static_cast<int32_t>(i);
    }
    combo.SelectedIndex(selected);
    combo.IsEnabled(enabled && loaded_);
    A11y::SetName(combo, hstring(name));
    combo.SelectionChanged(
        [choices = std::move(options), on_select = std::move(apply)](
            Inspectable const &sender, SelectionChangedEventArgs const &) {
          const auto index = sender.as<ComboBox>().SelectedIndex();
          if (index < 0 || static_cast<size_t>(index) >= choices.size())
            return;
          on_select(choices[static_cast<size_t>(index)].value);
        });
    return combo;
  }

  // The design's segmented control: a well of buttons; the chosen one is raised and carries a 16x3 accent underline.
  FrameworkElement segmented_control(std::wstring const &name,
                                     std::vector<Option> const &options,
                                     std::wstring const &current,
                                     std::function<void(std::wstring const &)> apply,
                                     bool enabled = true) {
    Border well;
    well.Background(brush(palette_.seg_well));
    well.BorderBrush(brush(palette_.stroke));
    well.BorderThickness(Thickness{1, 1, 1, 1});
    well.CornerRadius(CornerRadius{5, 5, 5, 5});
    well.Padding(Thickness{2, 2, 2, 2});
    StackPanel items;
    items.Orientation(Orientation::Horizontal);
    items.Spacing(2);
    for (const auto &option : options) {
      const bool on = option.value == current;
      Button button;
      button.Height(30);
      button.MinWidth(0);
      button.Padding(Thickness{14, 0, 14, 0});
      button.CornerRadius(CornerRadius{4, 4, 4, 4});
      button.BorderThickness(Thickness{1, 1, 1, 1});
      button.Background(brush(on ? palette_.subtle2 : transparent_color()));
      button.BorderBrush(brush(on ? palette_.stroke : transparent_color()));
      button.HorizontalContentAlignment(HorizontalAlignment::Stretch);
      button.VerticalContentAlignment(VerticalAlignment::Stretch);
      Grid content;
      auto label = make_text(option.label, 14, on ? palette_.text : palette_.sub);
      label.TextWrapping(TextWrapping::NoWrap);
      label.HorizontalAlignment(HorizontalAlignment::Center);
      label.VerticalAlignment(VerticalAlignment::Center);
      content.Children().Append(label);
      if (on) {
        Border underline;
        underline.Width(16);
        underline.Height(3);
        underline.CornerRadius(CornerRadius{1.5, 1.5, 1.5, 1.5});
        underline.Background(brush(palette_.accent));
        underline.HorizontalAlignment(HorizontalAlignment::Center);
        underline.VerticalAlignment(VerticalAlignment::Bottom);
        underline.Margin(Thickness{0, 0, 0, 1});
        content.Children().Append(underline);
      }
      button.Content(content);
      A11y::SetName(button, hstring(name + L" " + option.label));
      if (on)
        A11y::SetItemStatus(button, L"已选择");
      button.IsEnabled(enabled && loaded_);
      button.Click([apply, value = option.value, on](Inspectable const &,
                                                     RoutedEventArgs const &) {
        if (!on)
          apply(value);
      });
      items.Children().Append(button);
    }
    A11y::SetName(well, hstring(name));
    well.Child(items);
    return well;
  }

  FrameworkElement slider_control(std::wstring const &name, double minimum,
                                  double maximum, double value,
                                  std::function<void(double)> apply,
                                  bool enabled = true) {
    StackPanel box;
    box.Orientation(Orientation::Horizontal);
    box.Spacing(16);
    Slider slider;
    slider.Width(180);
    slider.Minimum(minimum);
    slider.Maximum(maximum);
    slider.StepFrequency(1);
    slider.SmallChange(1);
    slider.LargeChange(1);
    slider.Value(std::clamp(value, minimum, maximum));
    slider.VerticalAlignment(VerticalAlignment::Center);
    slider.IsEnabled(enabled && loaded_);
    A11y::SetName(slider, hstring(name));
    auto label = make_text(number_text(std::clamp(value, minimum, maximum)), 14,
                           palette_.text);
    label.Width(60);
    label.TextAlignment(TextAlignment::Right);
    label.VerticalAlignment(VerticalAlignment::Center);
    slider.ValueChanged(
        [weak_label = make_weak(label), on_slide = std::move(apply)](
            Inspectable const &,
            Controls::Primitives::RangeBaseValueChangedEventArgs const &args) {
          const double rounded = std::round(args.NewValue());
          if (auto block = weak_label.get())
            block.Text(number_text(rounded));
          on_slide(rounded);
        });
    box.Children().Append(slider);
    box.Children().Append(label);
    return box;
  }

  FrameworkElement button_control(std::wstring const &label,
                                  std::function<void()> click,
                                  bool enabled = true) {
    Button button;
    button.Content(box_value(hstring(label)));
    button.Height(32);
    button.Padding(Thickness{16, 0, 16, 0});
    button.IsEnabled(enabled);
    button.Click([on_click = std::move(click)](Inspectable const &,
                                               RoutedEventArgs const &) { on_click(); });
    return button;
  }

  FrameworkElement checks_panel(std::vector<Check> checks,
                                double item_width = 174) {
    VariableSizedWrapGrid grid;
    grid.Orientation(Orientation::Horizontal);
    grid.ItemWidth(item_width + 24);
    grid.ItemHeight(36);
    grid.Margin(Thickness{61, 0, 25, 18});
    for (auto &check : checks) {
      record(check.label, L"");
      CheckBox box;
      box.Content(box_value(hstring(check.label)));
      box.IsChecked(check.on);
      box.MinWidth(0);
      box.IsEnabled(check.enabled && loaded_);
      box.Click([apply = std::move(check.apply)](Inspectable const &sender,
                                                 RoutedEventArgs const &) {
        const auto state = sender.as<CheckBox>().IsChecked();
        apply(state && state.Value());
      });
      mark_search_target(check.label, box);
      grid.Children().Append(box);
    }
    return grid;
  }

  // ---- Preference rows ----

  void bool_row(StackPanel const &group, wchar_t glyph, std::wstring const &title,
                std::wstring const &subtitle, std::wstring key, bool fallback,
                bool redraw = false, bool enabled = true) {
    const bool on = document_.Boolean(key, fallback);
    add_row(group, glyph, title, subtitle,
            toggle_control(title, on,
                           [this, key, redraw](bool value) {
                             change([&](PreferencesDocument &doc) {
                               doc.SetBoolean(key, value);
                             }, redraw);
                           },
                           enabled));
  }

  void select_row(StackPanel const &group, wchar_t glyph,
                  std::wstring const &title, std::wstring const &subtitle,
                  std::wstring key, std::vector<Option> options,
                  std::wstring const &fallback, bool redraw = false,
                  bool enabled = true) {
    const auto current = document_.String(key, fallback);
    add_row(group, glyph, title, subtitle,
            select_control(title, std::move(options), current,
                           [this, key, redraw](std::wstring const &value) {
                             change([&](PreferencesDocument &doc) {
                               doc.SetString(key, value);
                             }, redraw);
                           },
                           enabled));
  }

  void segment_row(StackPanel const &group, wchar_t glyph,
                   std::wstring const &title, std::wstring const &subtitle,
                   std::wstring key, std::vector<Option> const &options,
                   std::wstring const &fallback) {
    const auto current = document_.String(key, fallback);
    add_row(group, glyph, title, subtitle,
            segmented_control(title, options, current,
                              [this, key](std::wstring const &value) {
                                change([&](PreferencesDocument &doc) {
                                  doc.SetString(key, value);
                                }, true);
                              }));
  }

  // A numeric preference offered as a list; a stored value outside the list is kept as its own entry rather than silently shown as another.
  void number_select_row(StackPanel const &group, wchar_t glyph,
                         std::wstring const &title, std::wstring const &subtitle,
                         std::wstring key, std::vector<int> values,
                         double fallback, std::wstring const &suffix) {
    const auto current = static_cast<int>(std::lround(document_.Number(key, fallback)));
    if (std::find(values.begin(), values.end(), current) == values.end()) {
      values.push_back(current);
      std::sort(values.begin(), values.end());
    }
    std::vector<Option> options;
    for (const int value : values)
      options.push_back({std::to_wstring(value), std::to_wstring(value) + suffix});
    add_row(group, glyph, title, subtitle,
            select_control(title, std::move(options), std::to_wstring(current),
                           [this, key](std::wstring const &value) {
                             change([&](PreferencesDocument &doc) {
                               doc.SetNumber(key, std::stod(value));
                             }, false);
                           }));
  }

  void slider_row(StackPanel const &group, wchar_t glyph,
                  std::wstring const &title, std::wstring const &subtitle,
                  std::wstring key, double minimum, double maximum,
                  double fallback) {
    add_row(group, glyph, title, subtitle,
            slider_control(title, minimum, maximum,
                           document_.Number(key, fallback),
                           [this, key](double value) {
                             document_.SetNumber(key, value);
                             schedule_save();
                             update_preview();
                           }));
  }

  // A surface of the shared desktop app this window does not draw itself.
  void shell_row(StackPanel const &group, wchar_t glyph, std::wstring const &title,
                 std::wstring const &subtitle, std::wstring const &action,
                 nav::ShellTarget target) {
    add_row(group, glyph, title, subtitle,
            button_control(action, [this, target] { open_shell(target); }));
  }

  void url_row(StackPanel const &group, wchar_t glyph, std::wstring const &title,
               std::wstring const &subtitle, std::wstring const &action,
               const wchar_t *url) {
    add_row(group, glyph, title, subtitle,
            button_control(action, [this, url] { open_url(url); }));
  }

  // ---- Launching ----

  // The shared desktop app (MSIME.exe) is installed beside this executable.
  static std::filesystem::path shell_executable() {
    std::wstring buffer(MAX_PATH, L'\0');
    for (;;) {
      const DWORD length = GetModuleFileNameW(
          nullptr, buffer.data(), static_cast<DWORD>(buffer.size()));
      if (length == 0)
        return {};
      if (length < buffer.size()) {
        buffer.resize(length);
        break;
      }
      buffer.resize(buffer.size() * 2);
    }
    return std::filesystem::path(buffer).parent_path() / L"MSIME.exe";
  }

  // The shared app must read the same store as this window; the context is passed only when both paths are known.
  static std::optional<msime::windows::ShellLaunchContext> launch_context() {
    const auto state_root = state_directory();
    const auto options = runtime_options_file();
    if (state_root.empty() || !state_root.is_absolute() || !options ||
        !options->is_absolute())
      return std::nullopt;
    return msime::windows::ShellLaunchContext{state_root, *options};
  }

  // Starting the shared app waits for it to become idle, so it runs off the UI thread; only a failure comes back.
  fire_and_forget open_shell(nav::ShellTarget target) {
    auto weak = get_weak();
    auto queue = DispatcherQueue();
    const msime::windows::ShellSurfaceRequest request{
        std::string(target.panel), std::string(target.page)};
    const auto executable = shell_executable();
    const auto context = launch_context();
    co_await resume_background();
    std::error_code error;
    const bool started =
        !executable.empty() &&
        std::filesystem::is_regular_file(executable, error) &&
        (context ? msime::windows::launch_shell_surface(executable, request,
                                                        *context)
                 : msime::windows::launch_shell_surface(executable, request));
    if (started)
      co_return;
    queue.TryEnqueue([weak] {
      if (auto self = weak.get())
        self->show_notice(
            L"无法打开水杉输入法应用（MSIME.exe）。请重新安装输入法后再试。");
    });
  }

  void open_url(const wchar_t *url) {
    const auto result = reinterpret_cast<INT_PTR>(
        ShellExecuteW(nullptr, L"open", url, nullptr, nullptr, SW_SHOWNORMAL));
    if (result <= 32)
      show_notice(std::wstring(L"无法打开浏览器。地址：") + url);
  }

  void open_folder(std::filesystem::path const &folder) {
    std::error_code error;
    if (folder.empty() || !std::filesystem::is_directory(folder, error)) {
      show_notice(L"数据目录还不存在。请先完成输入法安装。");
      return;
    }
    const auto result = reinterpret_cast<INT_PTR>(ShellExecuteW(
        nullptr, L"open", folder.c_str(), nullptr, nullptr, SW_SHOWNORMAL));
    if (result <= 32)
      show_notice(L"无法打开数据目录：" + folder.wstring());
  }

  static void copy_text(std::wstring const &value) {
    Windows::ApplicationModel::DataTransfer::DataPackage package;
    package.SetText(hstring(value));
    Windows::ApplicationModel::DataTransfer::Clipboard::SetContent(package);
  }

  // ---- Pages ----

  void build_page(std::string_view id, StackPanel const &page) {
    const auto *model = nav::find_page(id);
    if (!model)
      return;
    if (model->host == nav::PageHost::Shell) {
      build_shell_page(*model, page);
      return;
    }
    if (model->host == nav::PageHost::Download) {
      build_download_page(page);
      return;
    }
    if (has_preview(id) && preview_visible_ && !indexing_)
      page.Children().Append(make_preview());
    if (id == "themes")
      build_themes_page(page);
    else if (id == "candidate")
      build_candidate_page(page);
    else if (id == "toolbar")
      build_toolbar_page(page);
    else if (id == "typing")
      build_typing_page(page);
    else if (id == "expression")
      build_expression_page(page);
    else if (id == "shortcuts")
      build_shortcuts_page(page);
    else if (id == "lexicon")
      build_lexicon_page(page);
    else if (id == "osk")
      build_osk_page(page);
    else if (id == "voice")
      build_voice_page(page);
    else if (id == "hand")
      build_hand_page(page);
    else if (id == "dev")
      build_dev_page(page);
    else if (id == "feedback")
      build_feedback_page(page);
    else if (id == "about")
      build_about_page(page);
  }

  // 账户与同步、云剪贴板、统计、社区 are pages of the shared app on every platform; this window opens them there.
  void build_shell_page(nav::Page const &model, StackPanel const &page) {
    const auto &label = page_label(model.id);
    auto group = add_group(page, L"");
    add_row(group, label.glyph, label.title, shell_page_description(model.id),
            button_control(L"打开", [this, target = model.shell] {
              open_shell(target);
            }));
    auto note = make_text(L"这一页由水杉输入法应用提供，点击「打开」后在应用中显示。",
                          12, palette_.faint);
    note.Margin(Thickness{4, 8, 4, 0});
    page.Children().Append(note);
  }

  void build_download_page(StackPanel const &page) {
    auto group = add_group(page, L"");
    url_row(group, 0xE896, L"下载页",
            L"在 macOS、Linux、iOS、Android 和鸿蒙设备上安装水杉输入法。",
            L"打开下载页", download_url);
    add_row(group, 0xE8C8, L"复制下载链接", download_url,
            button_control(L"复制链接", [this] {
              copy_text(download_url);
              show_notice(L"已复制下载链接。", InfoBarSeverity::Success);
            }));
  }

  // ---- 主题 ----

  ThemePreview preview_for(std::wstring const &id) {
    auto builtin = [this](std::wstring const &theme) -> std::optional<ThemePreview> {
      for (const auto &entry : themes_)
        if (entry.id == theme)
          return entry.preview;
      return std::nullopt;
    };
    // 跟随系统 is drawn in the window's own colours; the catalog gives it no fixed palette.
    if (id == L"system")
      return ThemePreview{palette_.window_bg, palette_.preview_bg,
                          palette_.accent, palette_.text};
    // The custom theme is what the user made of it: the candidate card as the shared layer resolves it (base, candidate_colors and skin package), over the base theme's backdrop. The catalog's own entry for it only shows the default custom colours.
    if (id == L"custom") {
      const auto candidate = candidate_theme(id);
      const auto base = builtin(document_.String(L"custom_theme.base", L"system"));
      return ThemePreview{base ? base->background : palette_.window_bg,
                          to_color(candidate.surface), to_color(candidate.accent),
                          to_color(candidate.text)};
    }
    if (auto preview = builtin(id))
      return *preview;
    return ThemePreview{palette_.window_bg, palette_.preview_bg, palette_.accent,
                        palette_.text};
  }

  FrameworkElement make_theme_card(ThemeEntry const &theme, bool on) {
    const auto preview = preview_for(theme.id);
    Button card;
    card.Width(184);
    card.Padding(Thickness{10, 10, 10, 10});
    card.CornerRadius(CornerRadius{6, 6, 6, 6});
    card.Background(brush(palette_.card));
    card.BorderBrush(brush(on ? palette_.accent : palette_.stroke));
    card.BorderThickness(on ? Thickness{2, 2, 2, 2} : Thickness{1, 1, 1, 1});
    card.HorizontalContentAlignment(HorizontalAlignment::Stretch);

    StackPanel body;
    body.Spacing(8);
    Border surface;
    surface.Height(84);
    surface.CornerRadius(CornerRadius{4, 4, 4, 4});
    surface.Background(brush(preview.background));
    Border panel;
    panel.CornerRadius(CornerRadius{4, 4, 4, 4});
    panel.Padding(Thickness{10, 6, 10, 6});
    panel.Background(brush(preview.panel));
    panel.HorizontalAlignment(HorizontalAlignment::Center);
    panel.VerticalAlignment(VerticalAlignment::Center);
    StackPanel words;
    words.Orientation(Orientation::Horizontal);
    words.Spacing(10);
    words.Children().Append(make_text(L"1 候选", 13, preview.accent));
    words.Children().Append(make_text(L"2 候选", 13, preview.text));
    panel.Child(words);
    surface.Child(panel);
    body.Children().Append(surface);

    Grid footer;
    footer.Padding(Thickness{2, 0, 2, 0});
    auto name = make_text(theme.title, 14, palette_.text);
    footer.Children().Append(name);
    if (on) {
      auto badge = make_text(L"使用中", 12, palette_.accent);
      badge.HorizontalAlignment(HorizontalAlignment::Right);
      badge.VerticalAlignment(VerticalAlignment::Center);
      footer.Children().Append(badge);
    }
    body.Children().Append(footer);
    card.Content(body);
    A11y::SetName(card, hstring(L"主题 " + theme.title));
    if (on)
      A11y::SetItemStatus(card, L"使用中");
    card.IsEnabled(loaded_);
    card.Click([this, id = theme.id, on](Inspectable const &,
                                         RoutedEventArgs const &) {
      if (on)
        return;
      change([&](PreferencesDocument &doc) { doc.SetString(L"global_theme", id); },
             true);
    });
    return card;
  }

  void build_themes_page(StackPanel const &page) {
    const auto current = document_.String(L"global_theme", L"system");
    if (themes_.empty()) {
      auto group = add_group(page, L"");
      add_row(group, 0xE790, L"全局主题", L"无法读取主题列表。请重新安装输入法后再试。",
              nullptr);
    } else {
      record(L"全局主题", L"候选窗口、悬浮工具栏、菜单和屏幕键盘的配色");
      VariableSizedWrapGrid cards;
      cards.Orientation(Orientation::Horizontal);
      cards.ItemWidth(196);
      cards.ItemHeight(148);
      mark_search_target(L"全局主题", cards);
      for (const auto &theme : themes_) {
        record(theme.title, L"主题");
        if (!indexing_) {
          const auto card = make_theme_card(theme, theme.id == current);
          mark_search_target(theme.title, card);
          cards.Children().Append(card);
        }
      }
      page.Children().Append(cards);
    }

    auto look = add_group(page, L"外观");
    segment_row(look, 0xE706, L"主题模式", L"深色、浅色或跟随 Windows 的应用模式",
                L"theme",
                {{L"system", L"跟随系统"}, {L"light", L"浅色"}, {L"dark", L"深色"}},
                L"system");
    select_row(look, 0xE713, L"设置界面主题", L"覆盖主题模式，仅影响当前设置窗口",
               L"settings_theme", surface_theme_options(), L"follow", true);
    slider_row(look, 0xE8E9, L"候选窗字号", L"候选词的字号（12–32）",
               L"candidate_font_size", 12, 32, 18);
    const auto family = document_.String(L"candidate_font_family", L"");
    shell_row(look, 0xE8D2, L"候选字体",
              family.empty() ? L"跟随系统默认字体" : family, L"更改",
              nav::shell_links::appearance);
    shell_row(look, 0xE70F, L"自定义主题",
              L"取色器、皮肤包与键盘样式在水杉输入法应用中编辑", L"编辑",
              nav::shell_links::skin);

    auto surfaces = add_group(page, L"界面主题");
    select_row(surfaces, 0xE8FD, L"候选窗口主题", L"预览跟随主题模式",
               L"candidate_theme", surface_theme_options(), L"follow");
    select_row(surfaces, 0xE7F4, L"悬浮工具栏主题", L"", L"toolbar_theme",
               surface_theme_options(), L"follow");
    select_row(surfaces, 0xE700, L"菜单主题",
               L"覆盖托盘菜单与候选右键菜单的明暗外观", L"menu_theme",
               surface_theme_options(), L"follow");
    select_row(surfaces, 0xE76E, L"表情面板主题",
               L"覆盖 Emoji、颜文字和符号面板的明暗外观", L"emoji_theme",
               surface_theme_options(), L"follow");
    select_row(surfaces, 0xE929, L"手写识别板主题", L"覆盖手写识别板的明暗外观",
               L"handwriting_theme", surface_theme_options(), L"follow");
    select_row(surfaces, 0xE720, L"语音输入弹出条主题",
               L"覆盖语音输入面板的明暗外观", L"voice_theme",
               surface_theme_options(), L"follow");
    select_row(surfaces, 0xE92E, L"屏幕键盘主题", L"", L"screen_keyboard_theme",
               surface_theme_options(), L"follow");
  }

  static std::vector<Option> surface_theme_options() {
    return {{L"follow", L"跟随全局"}, {L"dark", L"深色"}, {L"light", L"浅色"}};
  }

  // ---- 界面预览 ----

  FrameworkElement make_preview() {
    Grid holder;
    holder.Padding(Thickness{0, 8, 0, 40});
    preview_host_ = Border();
    preview_host_.HorizontalAlignment(HorizontalAlignment::Center);
    holder.Children().Append(preview_host_);
    update_preview();
    return holder;
  }

  void update_preview() {
    if (!preview_host_)
      return;
    // Drawn from the palette the candidate window itself draws (CandidateWindow.cpp), so the preview shows the resolved theme rather than this window's chrome.
    const auto theme = candidate_theme(document_.String(L"global_theme", L"system"));
    const bool vertical =
        document_.String(L"candidate_layout", L"vertical") == L"vertical";
    const auto count = std::clamp<int>(
        static_cast<int>(std::lround(document_.Number(L"candidate_page_size", 6))),
        1, 9);
    const double font = std::clamp(document_.Number(L"candidate_font_size", 18),
                                   12.0, 32.0);
    const bool preedit =
        document_.String(L"candidate_preedit_style", L"pinyin") != L"empty";
    const bool translations = document_.Boolean(L"candidate_translations", false);

    preview_host_.Background(brush(to_color(theme.surface)));
    preview_host_.BorderBrush(brush(to_color(theme.border)));
    preview_host_.BorderThickness(Thickness{1, 1, 1, 1});
    preview_host_.CornerRadius(CornerRadius{8, 8, 8, 8});
    preview_host_.Padding(Thickness{12, 8, 12, 10});
    A11y::SetName(preview_host_, L"候选窗口预览");

    StackPanel body;
    body.Spacing(6);
    StackPanel head;
    head.Orientation(Orientation::Horizontal);
    head.Spacing(12);
    head.Padding(Thickness{4, 0, 4, 0});
    auto mode = make_text(L"中", 13, to_color(theme.accent));
    mode.FontWeight(Windows::UI::Text::FontWeight{600});
    head.Children().Append(mode);
    // The candidate window draws the preedit in the accent colour at semibold.
    if (preedit) {
      auto pre = make_text(L"hou’xuan’xiang", 15, to_color(theme.accent));
      pre.FontWeight(Windows::UI::Text::FontWeight{600});
      head.Children().Append(pre);
    }
    body.Children().Append(head);

    StackPanel list;
    list.Orientation(vertical ? Orientation::Vertical : Orientation::Horizontal);
    list.Spacing(4);
    for (int i = 0; i < count; ++i) {
      const auto &[word, gloss] = preview_words[static_cast<size_t>(i)];
      const bool highlighted = i == 0;
      Grid item;
      item.Padding(Thickness{6, 4, 10, 4});
      item.CornerRadius(CornerRadius{4, 4, 4, 4});
      if (highlighted)
        item.Background(brush(to_color(theme.selected)));
      if (highlighted && theme.show_selected_bar) {
        Border bar;
        bar.Width(3);
        bar.Height(16);
        bar.CornerRadius(CornerRadius{1.5, 1.5, 1.5, 1.5});
        bar.Background(brush(to_color(theme.accent)));
        bar.HorizontalAlignment(HorizontalAlignment::Left);
        bar.VerticalAlignment(VerticalAlignment::Center);
        bar.Margin(Thickness{-6, 0, 0, 0});
        item.Children().Append(bar);
      }
      StackPanel cell;
      cell.Orientation(Orientation::Horizontal);
      cell.Spacing(8);
      // Alpha 0 in the selected slots means "keep the unselected colour", as the renderer reads it. The translation draws in the package's translation colour, or else the number colour, as the renderer draws it.
      const auto number_color =
          msime::windows::candidate_row_number_color(theme, highlighted);
      const auto translation_color =
          msime::windows::candidate_row_translation_color(theme, highlighted);
      const auto row_color = msime::windows::candidate_row_text_color(
          theme, theme.text, highlighted, false);
      auto number = make_text(std::to_wstring(i + 1), 13, to_color(number_color));
      number.VerticalAlignment(VerticalAlignment::Center);
      cell.Children().Append(number);
      StackPanel words;
      auto candidate = make_text(word, font, to_color(row_color));
      candidate.TextWrapping(TextWrapping::NoWrap);
      words.Children().Append(candidate);
      if (translations)
        words.Children().Append(make_text(gloss, 13, to_color(translation_color)));
      cell.Children().Append(words);
      item.Children().Append(cell);
      list.Children().Append(item);
    }
    body.Children().Append(list);
    preview_host_.Child(body);
  }

  // ---- 候选窗口 ----

  void build_candidate_page(StackPanel const &page) {
    auto layout = add_group(page, L"布局");
    segment_row(layout, 0xE8FD, L"候选项排列方式", L"", L"candidate_layout",
                {{L"horizontal", L"横向"}, {L"vertical", L"纵向"}}, L"vertical");
    slider_row(layout, 0xE8CB, L"每页候选项数量", L"每页显示的候选项（1–9）",
               L"candidate_page_size", 1, 9, 6);
    slider_row(layout, 0xE8E9, L"候选窗预编辑字号", L"候选窗口中拼音的字号（12–32）",
               L"candidate_preedit_font_size", 12, 32, 15);
    bool_row(layout, 0xE718, L"候选窗口跟随光标",
             L"关闭后保持首次出现的位置，直到候选窗口消失。",
             L"candidate_follow_cursor", true);

    auto preedit = add_group(page, L"预编辑");
    select_row(preedit, 0xE70F, L"候选窗预编辑", L"", L"candidate_preedit_style",
               {{L"pinyin", L"拼音分词"}, {L"empty", L"不显示"}}, L"pinyin", true);
    select_row(preedit, 0xE8D2, L"行内预编辑", L"", L"tsf_preedit_style",
               {{L"raw", L"原始按键"}, {L"pinyin", L"拼音分词"}, {L"empty", L"不显示"}},
               L"raw");

    auto paging = add_group(page, L"翻页");
    const bool word_character = document_.Boolean(L"word_character.enabled", false);
    const auto word_keys = document_.String(L"word_character.keys", L"brackets");
    const std::array<std::pair<const wchar_t *, const wchar_t *>, 7> keys{{
        {L"minus_equal", L"- / ="},
        {L"comma_period", L", / ."},
        {L"brackets", L"[ / ]"},
        {L"tab", L"Shift+Tab / Tab"},
        {L"page_up_down", L"PageUp / PageDown"},
        {L"mouse_wheel", L"鼠标滚轮（候选面板支持时翻页）"},
        {L"arrows", L"上 / 下（移动候选项）"},
    }};
    std::vector<Check> checks;
    for (const auto &[id, label] : keys) {
      const std::wstring key = std::wstring(L"navigation.") + id;
      const std::wstring pair = id;
      checks.push_back(
          {label, document_.Boolean(key, false),
           [this, key, pair, word_character, word_keys](bool on) {
             // 以词定字 and 翻页 cannot share a key pair: taking the pair for paging turns 以词定字 off.
             const bool conflict = on && word_character && pair == word_keys;
             change([&](PreferencesDocument &doc) {
               doc.SetBoolean(key, on);
               if (conflict)
                 doc.SetBoolean(L"word_character.enabled", false);
             }, conflict);
           }});
    }
    add_row(paging, 0xE8AB, L"翻页方式", L"选择用于翻页的按键", nullptr,
            checks_panel(std::move(checks), 190));
  }

  // ---- 悬浮工具栏 ----

  void build_toolbar_page(StackPanel const &page) {
    auto general = add_group(page, L"");
    bool_row(general, 0xE7F4, L"在桌面显示悬浮工具栏", L"快速访问输入法状态与常用功能",
             L"floating_toolbar.enabled", true);
    auto appearance = add_group(page, L"外观");
    number_select_row(appearance, 0xE740, L"工具栏缩放",
                      L"相对系统 DPI 的额外缩放，不改变系统显示缩放",
                      L"floating_toolbar.scale_percent", {75, 100, 125, 150}, 100,
                      L"%");
    number_select_row(appearance, 0xE8E9, L"图标尺寸",
                      L"图标基准大小（像素），再乘以上方缩放",
                      L"floating_toolbar.font_size", {16, 18, 20, 22, 24, 26, 28},
                      24, L"");
    const std::array<std::pair<const wchar_t *, const wchar_t *>, 7> components{{
        {L"english_mode", L"英文输入模式"},
        {L"fullwidth", L"全角 / 半角"},
        {L"punctuation", L"中英文标点"},
        {L"character_set", L"简繁切换"},
        {L"emoji", L"表情与符号"},
        {L"screen_keyboard", L"屏幕键盘"},
        {L"settings", L"设置"},
    }};
    std::vector<Check> checks;
    checks.push_back({L"中英文切换（始终显示）", true, [](bool) {}, false});
    for (const auto &[id, label] : components) {
      const std::wstring key = std::wstring(L"floating_toolbar.") + id;
      checks.push_back({label, document_.Boolean(key, false), [this, key](bool on) {
                          change([&](PreferencesDocument &doc) {
                            doc.SetBoolean(key, on);
                          }, false);
                        }});
    }
    auto parts = add_group(page, L"组件");
    add_row(parts, 0xE71D, L"工具栏组件", L"勾选要显示在悬浮工具栏中的功能", nullptr,
            checks_panel(std::move(checks)));
  }

  // ---- 输入 ----

  void build_typing_page(StackPanel const &page) {
    const auto scheme = document_.String(L"scheme", L"quanpin");
    auto schemes = add_group(page, L"输入方案");
    add_row(schemes, 0xE765, L"输入方案", L"全拼、双拼、五笔或日语", segmented_control(
        L"输入方案",
        {{L"quanpin", L"全拼"}, {L"shuangpin", L"双拼"}, {L"wubi", L"五笔"},
         {L"japanese", L"日语"}},
        scheme, [this](std::wstring const &next) { select_scheme(next); }));
    if (scheme == L"shuangpin" || indexing_)
      select_row(schemes, 0xE8AB, L"双拼方案", L"", L"shuangpin_profile",
                 {{L"xiaohe", L"小鹤双拼"}, {L"ziranma", L"自然码双拼"},
                  {L"microsoft", L"微软双拼"}, {L"shoudao", L"首道双拼"}},
                 L"xiaohe");
    if (scheme == L"wubi" || indexing_) {
      bool_row(schemes, 0xE8D2, L"编码打不出时用拼音候选",
               L"五笔词库无法回答当前编码时，用同一串字母查询全拼；词库能回答时不影响。",
               L"wubi_mixed_pinyin", false);
      bool_row(schemes, 0xE8CB, L"候选显示剩余编码",
               L"在候选后面标出还要再打哪几个字母才能单独打出它。已经打完整码的候选不标。",
               L"wubi_code_hint", true);
    }

    auto language = add_group(page, L"中英文");
    const std::array<std::pair<const wchar_t *, const wchar_t *>, 3> switches{{
        {L"switch_language_shift", L"Shift 切换中英文"},
        {L"switch_language_ctrl", L"单击 Ctrl 切换中英文"},
        {L"switch_language_ctrl_alt_space", L"Ctrl+Alt+Space 切换中英文"},
    }};
    std::vector<Check> keys;
    for (const auto &[id, label] : switches) {
      const std::wstring key = std::wstring(L"keybindings.") + id;
      keys.push_back({label, document_.Boolean(key, false), [this, key](bool on) {
                        change([&](PreferencesDocument &doc) {
                          doc.SetBoolean(key, on);
                        }, false);
                      }});
    }
    add_row(language, 0xE8AB, L"中英文切换键", L"选择用于切换中英文的按键", nullptr,
            checks_panel(std::move(keys), 230));
    segment_row(language, 0xE774, L"默认中英文", L"新焦点会话开始时使用的中文或英文状态",
                L"default_ime_mode", {{L"chinese", L"中文"}, {L"english", L"英文"}},
                L"english");
    segment_row(language, 0xE71D, L"中英文状态",
                L"按应用分别记忆输入状态，或让所有输入上下文保持同一状态",
                L"ime_mode_scope", {{L"app", L"按应用记忆"}, {L"global", L"全局统一"}},
                L"app");
    bool_row(language, 0xE8C1, L"简繁输入", L"将提交的简体中文转换为繁体中文",
             L"traditional_chinese_output", false);
    add_row(language, 0xE8E9, L"全角输入",
            L"将英文字符和空格提交为全角形式，会话开始时生效；工具栏、键盘的更多工具或快捷键可临时切换",
            toggle_control(L"全角输入",
                           document_.String(L"character_width", L"halfwidth") ==
                               L"fullwidth",
                           [this](bool on) {
                             change([&](PreferencesDocument &doc) {
                               doc.SetString(L"character_width",
                                             on ? L"fullwidth" : L"halfwidth");
                             }, false);
                           }));

    auto word = add_group(page, L"以词定字");
    const bool word_enabled = document_.Boolean(L"word_character.enabled", false);
    const auto word_keys = document_.String(L"word_character.keys", L"brackets");
    add_row(word, 0xE8C1, L"以词定字",
            L"开启后，按所选键组的左键上屏高亮候选的首个汉字，右键上屏末个汉字",
            select_control(L"以词定字",
                           {{L"off", L"关闭"}, {L"brackets", L"[ / ]"},
                            {L"minus_equal", L"- / ="}},
                           word_enabled ? word_keys : L"off",
                           [this](std::wstring const &value) {
                             change([&](PreferencesDocument &doc) {
                               if (value == L"off") {
                                 doc.SetBoolean(L"word_character.enabled", false);
                                 return;
                               }
                               // The chosen pair stops paging, which the store requires.
                               doc.SetString(L"word_character.keys", value);
                               doc.SetBoolean(L"word_character.enabled", true);
                               doc.SetBoolean(L"navigation." + value, false);
                             }, true);
                           }));

    auto helpcode = add_group(page, L"辅助码");
    helpcode_rows(helpcode, L"全拼", L"quanpin_helpcode");
    helpcode_rows(helpcode, L"双拼", L"shuangpin_helpcode");
    shell_row(helpcode, 0xE8A7, L"辅助码详细设置", L"在水杉输入法应用中查看辅助码说明与编码表",
              L"打开", nav::shell_links::helpcode);

    auto mixed = add_group(page, L"中英混输");
    bool_row(mixed, 0xE774, L"中英混输", L"中文输入时在候选项中补充英文单词",
             L"mixed_input.english", true, true);
    slider_row(mixed, 0xE8CB, L"触发字符数", L"预编辑字母达到该长度后才出现英文候选项",
               L"mixed_input.minimum_prefix", 1, 8, 5);
    bool_row(mixed, 0xE76E, L"emoji 混输",
             L"中文输入时在候选项中加入匹配的 emoji（位于英文候选之后；云候选与 AI 联想会使其相应顺移）",
             L"mixed_input.emoji", false);
    bool_row(mixed, 0xE76E, L"颜文字混输",
             L"中文输入时在候选项中加入匹配的颜文字（排在 emoji 之后；云候选与 AI 联想会使其相应顺移）",
             L"mixed_input.kaomoji", false);
  }

  // Choosing Japanese remembers the Chinese scheme it replaces, so switching back returns to it; choosing a Chinese scheme makes it the one remembered. The same rule as the tray (store_input_scheme in server_main.cpp).
  void select_scheme(std::wstring const &next) {
    const auto current = document_.String(L"scheme", L"quanpin");
    if (current == next)
      return;
    change([&](PreferencesDocument &doc) {
      doc.SetString(L"last_chinese_scheme", next == L"japanese" ? current : next);
      doc.SetString(L"scheme", next);
    }, true);
  }

  void helpcode_rows(StackPanel const &group, std::wstring const &label,
                     std::wstring const &prefix) {
    const bool enabled = document_.Boolean(prefix + L".enabled", false);
    bool_row(group, 0xE8CB, label + L"辅助码",
             L"再输入的字母作为辅助码交给输入引擎，用于缩小候选。五笔、日语和本地输入模式不使用辅助码。",
             prefix + L".enabled", false, true);
    select_row(group, 0xE8D2, label + L"辅助码方案", L"", prefix + L".schema",
               {{L"lantian", L"蓝天小雨点"}, {L"ziranma", L"自然码"},
                {L"shouyou2_0", L"首右2.0"}, {L"shouyouplus", L"首右plus"},
                {L"xiaohe", L"小鹤"}, {L"jiajia", L"加加"}},
               L"lantian", false, enabled);
    bool_row(group, 0xE8FD, L"在候选窗口中显示" + label + L"辅助码", L"",
             prefix + L".show_in_candidate_window", false, false, enabled);
  }

  // ---- 表达 ----

  void build_expression_page(StackPanel const &page) {
    auto punctuation = add_group(page, L"标点");
    bool_row(punctuation, 0xE8C1, L"中文标点", L"在中文模式下使用全角标点。",
             L"chinese_punctuation", true);
    select_row(punctuation, 0xE72E, L"固定标点", L"切换中英文时的标点形态，三者互斥",
               L"punctuation_lock",
               {{L"follow", L"跟随中英文状态"}, {L"chinese", L"始终使用中文标点"},
                {L"english", L"始终使用英文标点"}},
               L"follow");
    const bool smart = document_.Boolean(L"smart_punctuation", false);
    bool_row(punctuation, 0xE945, L"智能标点",
             L"中文标点模式下，字母或数字后的 , . : 自动使用英文标点",
             L"smart_punctuation", false, true);
    bool_row(punctuation, 0xE8EE, L"重复标点转中文",
             L"智能标点输出英文标点后，2 秒内再次输入同一标点时替换为中文标点",
             L"smart_punctuation_repeat", false, false, smart);
    bool_row(punctuation, 0xE8EE, L"中文标点后按空格转换",
             L"刚输入中文标点后按空格，转换为对应英文标点",
             L"smart_punctuation_space_convert", false, false, smart);
    bool_row(punctuation, 0xE8EE, L"数字后直出",
             L"数字后输入逗号、句点或冒号时保留 ASCII 标点",
             L"smart_punctuation_direct_digit", false, false, smart);
    bool_row(punctuation, 0xE8EE, L"字母后直出",
             L"字母后输入逗号、句点或冒号时保留 ASCII 标点",
             L"smart_punctuation_direct_letter", false, false, smart);
    bool_row(punctuation, 0xE943, L"成对标点自动补全",
             L"输入左侧符号时自动补全右侧符号，并将光标置于中间",
             L"paired_punctuation", false);

    auto correction = add_group(page, L"纠错与模糊音");
    bool_row(correction, 0xE70F, L"自动纠错", L"修正常见的相邻按键顺序。",
             L"autocorrect", false);
    fuzzy_row(correction);

    auto candidates = add_group(page, L"翻译与云候选");
    const bool translations = document_.Boolean(L"candidate_translations", false);
    bool_row(candidates, 0xE8C1, L"候选词翻译", L"为当前候选请求翻译结果并显示在候选行",
             L"candidate_translations", false, true);
    select_row(candidates, 0xE774, L"目标语言", L"候选词翻译目标语言",
               L"translation_target_language",
               {{L"en", L"英语"}, {L"fr", L"法语"}, {L"ja", L"日语"}, {L"es", L"西班牙语"},
                {L"ru", L"俄语"}, {L"de", L"德语"}, {L"ko", L"韩语"}},
               L"en", false, translations);
    bool_row(candidates, 0xE82D, L"显示英文释义",
             L"在候选词后面标出它的英文意思，中文候选给英文、英文候选给中文。释义来自随键盘打包的离线词库，不联网。",
             L"candidate_english_gloss", false);
    bool_row(candidates, 0xE753, L"云候选", L"向在线服务请求额外候选",
             L"cloud_candidates", false);

    auto ai = add_group(page, L"AI");
    bool_row(ai, 0xE945, L"启用 AI 辅助", L"为拼音联想提供共享配置",
             L"ai_assistant.enabled", false);
    shell_row(ai, 0xE713, L"AI 辅助", L"服务商、模型与密钥在水杉输入法应用中设置", L"设置",
              nav::shell_links::ai);
    shell_row(ai, 0xE8BD, L"AI 对话", L"在水杉输入法应用中与 AI 对话", L"打开",
              nav::shell_links::chat);
  }

  void fuzzy_row(StackPanel const &group) {
    const bool enabled = document_.Boolean(L"fuzzy_pinyin.enabled", false);
    auto rules = document_.Strings(L"fuzzy_pinyin.rules");
    static constexpr std::array<const wchar_t *, 11> all_rules{
        L"z-zh", L"c-ch", L"s-sh", L"n-l", L"f-h", L"r-l",
        L"an-ang", L"en-eng", L"in-ing", L"ian-iang", L"uan-uang"};
    auto toggle = toggle_control(L"启用模糊音", enabled, [this](bool on) {
      change([&](PreferencesDocument &doc) {
        doc.SetBoolean(L"fuzzy_pinyin.enabled", on);
        // The first time it is switched on every rule starts checked; afterwards the user's choice is kept, also while it is off.
        if (on && !doc.Boolean(L"fuzzy_pinyin.seeded", false)) {
          doc.SetStrings(L"fuzzy_pinyin.rules",
                         std::vector<std::wstring>(all_rules.begin(), all_rules.end()));
          doc.SetBoolean(L"fuzzy_pinyin.seeded", true);
        }
      }, true);
    });
    std::vector<Check> checks;
    for (const auto *rule : all_rules) {
      const std::wstring id(rule);
      const auto dash = id.find(L'-');
      const std::wstring label = id.substr(0, dash) + L" ↔ " + id.substr(dash + 1);
      const bool on = std::find(rules.begin(), rules.end(), id) != rules.end();
      checks.push_back({label, on, [this, id](bool checked) {
                          change([&](PreferencesDocument &doc) {
                            auto current = doc.Strings(L"fuzzy_pinyin.rules");
                            current.erase(std::remove(current.begin(), current.end(), id),
                                          current.end());
                            if (checked)
                              current.push_back(id);
                            doc.SetStrings(L"fuzzy_pinyin.rules", std::move(current));
                          }, false);
                        },
                        enabled});
    }
    StackPanel below;
    below.Children().Append(checks_panel(std::move(checks), 120));
    auto note = make_text(L"平翘舌 z c s、声母 n-l f-h r-l、前后鼻音 an en in、其他韵母 ian uan。勾选容易混淆的读音后，会补充对应候选。关闭总开关会保留已选规则。",
                          12, palette_.sub);
    note.Margin(Thickness{61, 0, 25, 16});
    below.Children().Append(note);
    add_row(group, 0xE8D2, L"模糊音", L"全拼、九键与双拼均支持；更改会在当前输入结束后生效",
            toggle, below);
  }

  // ---- 快捷键 ----

  void build_shortcuts_page(StackPanel const &page) {
    auto keys = add_group(page, L"快捷键");
    bool_row(keys, 0xE8C1, L"Ctrl+Shift+F 切换简繁", L"在简体与繁体输出之间切换",
             L"keybindings.toggle_character_set_ctrl_shift_f", true);
    add_row(keys, 0xE765, L"中英文切换键", L"在「输入」页选择",
            button_control(L"前往", [this] { navigate("typing", true); }));

    const std::array<std::pair<const wchar_t *, const wchar_t *>, 8> modes{{
        {L"quick_phrase", L"快捷短语(K 模式)"},
        {L"date_time", L"日期与时间快捷输入(T 模式)"},
        {L"unicode", L"Unicode 便捷录入(U 模式)"},
        {L"emoji", L"Emoji 快捷输入(E 模式)"},
        {L"kaomoji", L"颜文字快捷输入(M 模式)"},
        {L"super_jianpin", L"超级简拼(J 模式)"},
        {L"temporary_english", L"临时英文(Y 模式)"},
        {L"temporary_japanese", L"临时日语(R 模式)"},
    }};
    std::vector<Check> checks;
    for (const auto &[id, label] : modes) {
      const std::wstring key = std::wstring(L"local_modes.") + id;
      checks.push_back({label, document_.Boolean(key, true), [this, key](bool on) {
                          change([&](PreferencesDocument &doc) {
                            doc.SetBoolean(key, on);
                          }, false);
                        }});
    }
    auto local = add_group(page, L"快捷输入模式");
    add_row(local, 0xE945, L"快捷输入模式", L"用引导字母进入对应的输入模式", nullptr,
            checks_panel(std::move(checks), 250));

    auto reset = add_group(page, L"");
    add_row(reset, 0xE72C, L"恢复默认快捷键",
            L"中英文切换键、Ctrl+Shift+F 和快捷输入模式恢复为默认设置",
            button_control(L"恢复默认",
                           [this] {
                             change([](PreferencesDocument &doc) {
                               doc.Restore(L"keybindings");
                               doc.Restore(L"local_modes");
                             }, true);
                           },
                           loaded_));
  }

  // ---- 词库 ----

  void build_lexicon_page(StackPanel const &page) {
    auto frequency = add_group(page, L"拼音方案调频");
    select_row(frequency, 0xE8CB, L"调频方式", L"", L"frequency.mode",
               {{L"disabled", L"关闭"}, {L"pin", L"一次置顶"}, {L"halve", L"折半调频"},
                {L"linear", L"线性调频"}, {L"promote", L"一次置前"}},
               L"promote");
    number_select_row(frequency, 0xE8EF, L"触发频次(第几次上屏触发)", L"",
                      L"frequency.trigger_count", {1, 2, 3, 4, 5, 6}, 1, L"");
    number_select_row(frequency, 0xE8EF, L"线性调频步长", L"",
                      L"frequency.linear_step", {1, 2, 3, 4, 5, 6}, 1, L"");

    auto learning = add_group(page, L"学习");
    bool_row(learning, 0xE82D, L"开启词语学习", L"记录已上屏的词语，用于调整候选顺序。",
             L"learning", true);
    bool_row(learning, 0xE77F, L"剪贴板历史", L"保存复制过的文本供剪贴板面板使用。",
             L"clipboard_history", false);

    auto manage = add_group(page, L"管理");
    shell_row(manage, 0xE82D, L"管理词库", L"导入、导出和编辑词库与快捷短语", L"打开",
              nav::shell_links::dictionary);
    shell_row(manage, 0xE8F1, L"背单词", L"在水杉输入法应用中背单词", L"打开",
              nav::shell_links::vocabulary);
  }

  // ---- 屏幕键盘、语音、手写 ----

  void build_osk_page(StackPanel const &page) {
    auto group = add_group(page, L"");
    shell_row(group, 0xE92E, L"屏幕键盘", L"在屏幕上显示键盘，点击按键向当前窗口输入",
              L"打开", nav::shell_links::keyboard_panel);
    shell_row(group, 0xE713, L"屏幕键盘设置", L"在水杉输入法应用中调整屏幕键盘", L"设置",
              nav::shell_links::screen_keyboard);
  }

  void build_voice_page(StackPanel const &page) {
    auto general = add_group(page, L"");
    const bool enabled = document_.Boolean(L"voice_input.enabled", false);
    add_row(general, 0xE720, L"语音输入", L"使用语音识别将录音转换为文字",
            toggle_control(L"启用语音输入", enabled, [this](bool on) {
              change([&](PreferencesDocument &doc) {
                doc.SetBoolean(L"voice_input.enabled", on);
              }, true);
            }));
    select_row(general, 0xE8C8, L"结果提交策略", L"由当前桌面宿主决定如何把识别结果交给前台窗口",
               L"voice_input.commit_mode",
               {{L"tsf", L"输入法会话"}, {L"sendinput", L"系统按键"}, {L"ctrl_v", L"剪贴板粘贴"}},
               L"tsf", false, enabled);
    shell_row(general, 0xE713, L"识别服务与润色", L"选择识别服务、语言与文字润色", L"设置",
              nav::shell_links::voice);

    auto recording = add_group(page, L"录音行为");
    const std::array<std::pair<const wchar_t *, const wchar_t *>, 4> sounds{{
        {L"sound_enabled", L"语音提示音"},
        {L"start_sound", L"开始录音提示音"},
        {L"end_sound", L"结束录音提示音"},
        {L"mute_system_audio", L"录音时静音其他声音"},
    }};
    std::vector<Check> sound_checks;
    for (const auto &[id, label] : sounds) {
      const std::wstring key = std::wstring(L"voice_input.") + id;
      sound_checks.push_back({label, document_.Boolean(key, false),
                              [this, key](bool on) {
                                change([&](PreferencesDocument &doc) {
                                  doc.SetBoolean(key, on);
                                }, false);
                              },
                              enabled});
    }
    add_row(recording, 0xE767, L"录音行为", L"录音期间的提示音与静音由输入法在本机处理",
            nullptr, checks_panel(std::move(sound_checks), 200));

    const std::array<std::pair<const wchar_t *, const wchar_t *>, 5> hotkeys{{
        {L"hotkey_ctrl_f9", L"Ctrl+F9 切换语音"},
        {L"hotkey_ralt", L"长按右 Alt 录音"},
        {L"hotkey_rctrl_ralt", L"长按右 Ctrl+右 Alt 录音"},
        {L"hotkey_ctrl_win", L"长按 Ctrl+Win 录音"},
        {L"hotkey_hold_space_lock", L"长按录音时按空格锁定"},
    }};
    std::vector<Check> hotkey_checks;
    for (const auto &[id, label] : hotkeys) {
      const std::wstring key = std::wstring(L"voice_input.") + id;
      hotkey_checks.push_back({label, document_.Boolean(key, false),
                               [this, key](bool on) {
                                 change([&](PreferencesDocument &doc) {
                                   doc.SetBoolean(key, on);
                                 }, false);
                               },
                               enabled});
    }
    auto shortcuts = add_group(page, L"语音快捷键");
    add_row(shortcuts, 0xEDA7, L"语音快捷键",
            L"输入法运行时全局生效。长按快捷键录音，松开结束；按住期间按空格锁定录音，锁定后再按一次快捷键或点 ✓ 结束，Escape 或 ✗ 取消。Ctrl+F9 按一次开始、再按一次结束。",
            nullptr, checks_panel(std::move(hotkey_checks), 230));
  }

  void build_hand_page(StackPanel const &page) {
    auto group = add_group(page, L"");
    shell_row(group, 0xE929, L"手写输入", L"打开手写识别板，书写后向当前窗口输入", L"打开",
              nav::shell_links::handwriting_panel);
    shell_row(group, 0xE713, L"手写设置", L"在水杉输入法应用中调整手写识别", L"设置",
              nav::shell_links::handwriting);
  }

  // ---- 开发者选项 ----

  void build_dev_page(StackPanel const &page) {
    auto mcp = add_group(page, L"连接 AI 助手");
    record(L"连接 AI 助手", L"MCP msime-mcp Claude Desktop Cursor");
    if (!indexing_) {
      auto card = make_card();
      StackPanel content;
      content.Spacing(10);
      content.Padding(Thickness{25, 20, 25, 20});
      append_mcp_section(content);
      card.Child(content);
      mark_search_target(L"连接 AI 助手", card);
      mcp.Children().Append(card);
    }

    auto diagnostics = add_group(page, L"诊断");
    bool_row(diagnostics, 0xEBE8, L"Server 端日志",
             L"排查 Server 启动和通信问题时开启。记录 Server 启停原因和各组件是否就绪，限量轮转，不记录按键、输入内容或候选文本。文件是数据目录下的 logs\\server.log，TSF 端日志也写进这个文件，复现后可直接发送。",
             L"diagnostic_log.server", false);
    bool_row(diagnostics, 0xEBE8, L"TSF 端日志",
             L"排查应用内预编辑和输入延迟时开启。日志在内存中限量缓冲，并通过独立管道批量汇总，不记录按键、输入内容或候选文本。",
             L"diagnostic_log.tsf", false);
    const auto directory = state_directory();
    add_row(diagnostics, 0xE838, L"数据目录",
            directory.empty() ? L"未找到数据目录" : directory.wstring(),
            button_control(L"打开", [this, directory] { open_folder(directory); },
                           !directory.empty()));
  }

  // ---- 反馈、关于 ----

  void build_feedback_page(StackPanel const &page) {
    auto group = add_group(page, L"");
    shell_row(group, 0xED15, L"反馈", L"描述遇到的问题并提交给开发者", L"反馈",
              nav::shell_links::feedback);
    shell_row(group, 0xE897, L"帮助", L"使用说明与常见问题", L"打开",
              nav::shell_links::help);
    add_row(group, 0xEBE8, L"诊断日志", L"复现问题前在「开发者选项」中开启日志，反馈时一起发送",
            button_control(L"前往", [this] { navigate("dev", true); }));
  }

  void build_about_page(StackPanel const &page) {
    if (!indexing_) {
      auto card = make_card();
      Grid identity;
      identity.Padding(Thickness{25, 20, 25, 20});
      identity.ColumnSpacing(20);
      ColumnDefinition logo_column;
      logo_column.Width(GridLength{64, GridUnitType::Pixel});
      ColumnDefinition text_column;
      text_column.Width(GridLength{1, GridUnitType::Star});
      identity.ColumnDefinitions().Append(logo_column);
      identity.ColumnDefinitions().Append(text_column);
      Image logo;
      logo.Width(64);
      logo.Height(64);
      show_logo(logo, 128);
      Grid::SetColumn(logo, 0);
      identity.Children().Append(logo);
      StackPanel names;
      names.Spacing(4);
      names.VerticalAlignment(VerticalAlignment::Center);
      auto product = make_text(L"水杉输入法", 20, palette_.text);
      product.FontWeight(Windows::UI::Text::FontWeight{600});
      names.Children().Append(product);
      names.Children().Append(make_text(L"© 2026 Metasequoia", 12, palette_.faint));
      Grid::SetColumn(names, 1);
      identity.Children().Append(names);
      card.Child(identity);
      page.Children().Append(card);
    }

    auto updates = add_group(page, L"更新");
    shell_row(updates, 0xE895, L"检查更新", L"在水杉输入法应用中检查并安装新版本", L"检查",
              nav::shell_links::about);
    url_row(updates, 0xE8A5, L"版本发布记录", L"每个版本的更新内容", L"查看", releases_url);

    auto privacy = add_group(page, L"数据与隐私");
    bool_row(privacy, 0xE9D2, L"匿名使用统计",
             L"默认关闭。开启后，Server 每次启动向 https://api.msime.app/v1/telemetry/events 发送一条事件，只含随机事件 id、类型、平台名 windows 和版本号；Server 崩溃时再发一条，另带固定文本 std::terminate。不含输入内容、候选、剪贴板或账号信息。",
             L"telemetry_enabled", false);
    url_row(privacy, 0xEA18, L"隐私政策", L"设置保存到当前输入法数据目录，详细说明见隐私政策。",
            L"查看", privacy_url);
    url_row(privacy, 0xE8A5, L"开源许可", L"水杉输入法的开源许可证", L"查看", license_url);
  }

  // ---- 连接 AI 助手 ----

  void append_section(StackPanel const &panel, hstring heading,
                      hstring description) {
    auto heading_text = TextBlock();
    heading_text.Text(heading);
    heading_text.FontSize(14);
    heading_text.FontWeight(Windows::UI::Text::FontWeight{600});
    heading_text.Foreground(brush(palette_.text));
    panel.Children().Append(heading_text);
    auto description_text = TextBlock();
    description_text.Text(description);
    description_text.TextWrapping(TextWrapping::Wrap);
    description_text.IsTextSelectionEnabled(true);
    description_text.Foreground(brush(palette_.sub));
    panel.Children().Append(description_text);
  }

  void append_text(StackPanel const &panel, std::wstring const &value,
                   bool emphasis = false) {
    auto block = TextBlock();
    block.Text(value);
    block.TextWrapping(TextWrapping::Wrap);
    block.IsTextSelectionEnabled(true);
    block.Foreground(brush(emphasis ? palette_.text : palette_.sub));
    panel.Children().Append(block);
  }

  // 「连接 AI 助手」: the msime-mcp entry an assistant runs, to copy or to write into Claude Desktop's or Cursor's configuration. The same section as the shared settings page, through msime_client_mcp_status and msime_client_mcp_install.
  void append_mcp_section(StackPanel const &panel) {
    append_text(panel, L"连接后，直接告诉 AI 助手输入法哪里不对劲（比如卡顿、候选框不见了），它会打开诊断日志、请你把出问题的操作再做一遍，然后读日志帮你找原因。它也能读取快捷短语、设置和打字统计。通过 MCP（Model Context Protocol）在本机运行，不联网；除了开关诊断日志，默认不改动任何设置。");
    const auto response =
        call_mcp(msime_client_mcp_status, mcp_request(runtime_options_path()));
    JsonObject status{nullptr};
    if (response.ok) {
      try {
        status =
            JsonObject::Parse(text(response.text)).GetNamedObject(L"value");
      } catch (...) {
      }
    }
    if (!status) {
      append_text(panel, L"无法读取 MCP 服务器的状态。", true);
      return;
    }
    const bool installed = status.GetNamedBoolean(L"installed", false);
    append_section(
        panel, L"服务器程序",
        hstring(std::wstring(status.GetNamedString(L"command", L"").c_str()) +
                (installed ? L"" : L"（未找到，请重新安装输入法）")));
    const auto config =
        status.GetNamedValue(L"config", JsonValue::CreateNullValue());
    if (config.ValueType() != JsonValueType::String) {
      append_text(panel, L"输入法尚未完成初始化，完成设置向导后即可连接。", true);
      append_mcp_result(panel);
      return;
    }
    const auto snippet = config.GetString();
    auto code = TextBox();
    code.Text(snippet);
    code.IsReadOnly(true);
    code.AcceptsReturn(true);
    code.TextWrapping(TextWrapping::NoWrap);
    code.FontFamily(Media::FontFamily(L"Consolas"));
    code.Header(box_value(L"MCP 配置"));
    panel.Children().Append(code);
    append_text(panel,
                L"要让助手修改快捷短语和设置，在 args 中加入 "
                L"--allow-write；要让它读取你的用户词库、查看编码的候选，加入 "
                L"--allow-dictionary-"
                L"read；两项都加才能增删、调整和导入词。这两项只应在你信任该助"
                L"手时开启。");
    auto copy = Button();
    copy.Content(box_value(L"复制配置"));
    copy.Click([snippet](Inspectable const &sender, RoutedEventArgs const &) {
      Windows::ApplicationModel::DataTransfer::DataPackage package;
      package.SetText(snippet);
      Windows::ApplicationModel::DataTransfer::Clipboard::SetContent(package);
      sender.as<Button>().Content(box_value(L"已复制"));
    });
    panel.Children().Append(copy);
    if (installed) {
      for (auto const &value : status.GetNamedArray(L"clients", JsonArray())) {
        const auto client = value.GetObject();
        const std::wstring id(client.GetNamedString(L"id", L"").c_str());
        const auto name = mcp_client_name(id);
        const bool configured = client.GetNamedBoolean(L"configured", false);
        append_section(
            panel, hstring(name),
            hstring(std::wstring(client.GetNamedString(L"path", L"").c_str()) +
                    (configured ? L"（已连接）" : L"")));
        auto write = Button();
        write.Content(box_value(hstring(L"写入 " + name)));
        write.IsEnabled(!configured && !mcp_busy_);
        write.Click([this, id](Inspectable const &, RoutedEventArgs const &) {
          write_mcp_client(id);
        });
        panel.Children().Append(write);
      }
    }
    append_mcp_result(panel);
  }

  void append_mcp_result(StackPanel const &panel) {
    if (!mcp_result_.empty())
      append_text(panel, mcp_result_, true);
  }

  fire_and_forget write_mcp_client(std::wstring id) {
    if (mcp_busy_)
      co_return;
    auto strong = get_strong();
    const auto name = mcp_client_name(id);
    mcp_busy_ = true;
    mcp_result_.clear();
    auto request = mcp_request(runtime_options_path());
    request.SetNamedValue(L"client", JsonValue::CreateStringValue(id));
    request.SetNamedValue(L"replace", JsonValue::CreateBooleanValue(false));
    auto response = call_mcp(msime_client_mcp_install, request);
    if (!response.ok && response_error(response) == L"mcp_entry_exists") {
      ContentDialog dialog;
      dialog.Title(box_value(hstring(L"替换 " + name + L" 中的 msime？")));
      dialog.Content(box_value(
          hstring(name + L" 的配置里已有另一个名为 msime "
                         L"的服务器。替换后，它原来的命令和参数（包括手动加上的"
                         L" --allow-write）会被这里的设置覆盖。")));
      dialog.PrimaryButtonText(L"替换");
      dialog.CloseButtonText(L"取消");
      dialog.DefaultButton(ContentDialogButton::Close);
      dialog.XamlRoot(root_.XamlRoot());
      dialog.RequestedTheme(palette_.dark ? ElementTheme::Dark
                                          : ElementTheme::Light);
      if (co_await dialog.ShowAsync() != ContentDialogResult::Primary) {
        mcp_busy_ = false;
        co_return;
      }
      request.SetNamedValue(L"replace", JsonValue::CreateBooleanValue(true));
      response = call_mcp(msime_client_mcp_install, request);
    }
    if (response.ok) {
      std::wstring outcome;
      try {
        outcome = JsonObject::Parse(text(response.text))
                      .GetNamedString(L"value", L"")
                      .c_str();
      } catch (...) {
      }
      mcp_result_ =
          outcome == L"unchanged"
              ? name + L" 已经连接，无需改动。"
              : L"已写入 " + name + L" 的配置。重新启动 " + name + L" 后生效。";
    } else {
      mcp_result_ = mcp_failure(response_error(response), name);
    }
    mcp_busy_ = false;
    if (!closed_ && current_page_ == "dev")
      render_page(true);
  }

  PreferencesDocument document_;
  bool loaded_ = false;
  std::wstring load_error_;
  std::vector<ThemeEntry> themes_;
  std::string resolved_request_;
  std::optional<msime::windows::CandidatePalette> resolved_theme_;
  Windows::UI::ViewManagement::UISettings ui_settings_;
  bool system_dark_ = false;
  InputMethodState input_method_ = InputMethodState::unknown;
  Palette palette_;

  std::string current_page_;
  std::vector<std::string> history_;
  bool selecting_ = false;
  bool refresh_queued_ = false;
  bool closed_ = false;
  bool pending_save_ = false;
  bool preview_visible_ = true;
  std::wstring notice_;
  InfoBarSeverity notice_severity_ = InfoBarSeverity::Error;

  std::vector<SearchEntry> search_index_;
  bool indexing_ = false;
  bool index_valid_ = false;
  uint64_t index_revision_ = 0;
  std::wstring focus_row_;
  // What was last typed in the title bar search, which the no-results line must not replace.
  std::wstring search_query_;
  FrameworkElement focus_target_{nullptr};

  std::wstring mcp_result_;
  bool mcp_busy_ = false;

  Grid root_{nullptr};
  Grid titlebar_{nullptr};
  Button back_{nullptr};
  AutoSuggestBox search_{nullptr};
  AutoSuggestBox filter_{nullptr};
  NavigationView navigation_{nullptr};
  ScrollViewer scroll_{nullptr};
  Border preview_host_{nullptr};
  Microsoft::UI::Dispatching::DispatcherQueueTimer save_timer_{nullptr};
  // A redraw held back until the click that activated the window has been delivered.
  bool pointer_down_ = false;
  bool refresh_after_pointer_ = false;
  Microsoft::UI::Dispatching::DispatcherQueueTimer pointer_timer_{nullptr};
};

// Without a XAML compiler the application supplies the metadata provider itself; the WinUI controls' own provider answers for every type this window uses.
struct App : ApplicationT<App, Markup::IXamlMetadataProvider> {
  void OnLaunched(LaunchActivatedEventArgs const &) {
    Resources().MergedDictionaries().Append(XamlControlsResources());
    window_ = make<MainWindow>(route_page());
    window_.Activate();
  }

  Markup::IXamlType GetXamlType(Windows::UI::Xaml::Interop::TypeName const &type) {
    return provider_.GetXamlType(type);
  }

  Markup::IXamlType GetXamlType(hstring const &name) {
    return provider_.GetXamlType(name);
  }

  com_array<Markup::XmlnsDefinition> GetXmlnsDefinitions() {
    return provider_.GetXmlnsDefinitions();
  }

private:
  XamlTypeInfo::XamlControlsXamlMetaDataProvider provider_;
  Window window_{nullptr};
};

} // namespace

int WINAPI wWinMain(HINSTANCE, HINSTANCE, PWSTR, int) {
  init_apartment(apartment_type::single_threaded);
  Application::Start([](auto &&) { make<App>(); });
  return 0;
}

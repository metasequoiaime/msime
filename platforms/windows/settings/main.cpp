// windows.h comes first so shellapi.h sees its types; the two macros it defines that collide with WinRT member names are dropped before any WinRT header is read.
#include <windows.h>

#include <shellapi.h>
#include <shobjidl_core.h>
#include <d2d1.h>
#include <dwrite.h>
#include <wincodec.h>
#include "../common/StateDirectory.h"
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
#include <fstream>
#include <functional>
#include <initializer_list>
#include <iterator>
#include <map>
#include <optional>
#include <set>
#include <stdexcept>
#include <string>
#include <string_view>
#include <type_traits>
#include <utility>
#include <vector>

#include "../../../shared/contracts/msime_edition.h"
#include "../../common/HostApiString.h"
#include "../common/GameCompatProfile.h"
#include "AccountPageModel.h"
#include "AiSettings.h"
#include "AppInputModeRuleList.h"
#include "CandidateCanvasPreview.h"
#include "CandidateFontPresets.h"
#include "CandidatePalette.h"
#include "CandidateWindowStyle.h"
#include "ClientAgent.h"
#include "CloudClipboardRoute.h"
#include "CommunityPageModel.h"
#include "FeedbackReport.h"
#include "FloatingToolbarSizeModel.h"
#include "GameProcessList.h"
#include "LexiconImportModel.h"
#include "PluginPageModel.h"
#include "PreferencePath.h"
#include "PunctuationSettings.h"
#include "SettingsDocumentFile.h"
#include "SettingsNavigation.h"
#include "SettingsStateDirectory.h"
#include "ShellLauncher.h"
#include "ThemeSkinModel.h"
#include "ToolbarIcons.h"
#include "ToolbarLayout.h"
#include "TranslationServiceSettings.h"
#include "TypingStatisticsModel.h"
#include "VoiceSettings.h"
#include "msime_client.h"
#include "msimeui/AnimatedImage.h"

#include <winrt/base.h>

#include <winrt/Microsoft.UI.Dispatching.h>
#include <winrt/Microsoft.UI.Input.h>
#include <winrt/Microsoft.UI.Windowing.h>
#include <winrt/Microsoft.UI.Xaml.Automation.Peers.h>
#include <winrt/Microsoft.UI.Xaml.Automation.h>
#include <winrt/Microsoft.UI.Xaml.Controls.Primitives.h>
#include <winrt/Microsoft.UI.Xaml.Controls.h>
#include <winrt/Microsoft.UI.Xaml.Documents.h>
#include <winrt/Microsoft.UI.Xaml.Input.h>
#include <winrt/Microsoft.UI.Xaml.Markup.h>
#include <winrt/Microsoft.UI.Xaml.Media.Imaging.h>
#include <winrt/Microsoft.UI.Xaml.Media.h>
#include <winrt/Microsoft.UI.Xaml.Shapes.h>
#include <winrt/Microsoft.UI.Xaml.XamlTypeInfo.h>
#include <winrt/Microsoft.UI.Xaml.h>
#include <winrt/Microsoft.UI.h>
#include <winrt/Windows.ApplicationModel.DataTransfer.h>
#include <winrt/Windows.Data.Json.h>
#include <winrt/Windows.Foundation.Collections.h>
#include <winrt/Windows.Foundation.h>
#include <winrt/Windows.Graphics.h>
#include <winrt/Windows.Security.Cryptography.h>
#include <winrt/Windows.Storage.Streams.h>
#include <winrt/Windows.System.h>
#include <winrt/Windows.UI.Text.h>
#include <winrt/Windows.UI.ViewManagement.h>
#include <winrt/Windows.UI.Xaml.Interop.h>
#include <winrt/Windows.UI.h>

using namespace winrt;
using namespace winrt::Microsoft::UI::Xaml;
using namespace winrt::Microsoft::UI::Xaml::Controls;
using namespace winrt::Microsoft::UI::Xaml::Media;
using namespace winrt::Windows::Data::Json;

namespace {

// WRL 同时声明全局 Microsoft/Windows；本文件的界面名称始终指向 WinRT 投影。
namespace Microsoft = winrt::Microsoft;
namespace Windows = winrt::Windows;

// windows.h may declare the COM ::IInspectable as well; this alias always means the WinRT projection.
using Inspectable = winrt::Windows::Foundation::IInspectable;
using Color = winrt::Windows::UI::Color;
using A11y = Microsoft::UI::Xaml::Automation::AutomationProperties;
namespace nav = msime::settings;
namespace pg = msime::settings::plugins;
namespace vs = msime::settings::voice;
namespace ps = msime::settings::punctuation;
namespace ais = msime::settings::ai;
namespace am = msime::settings::account;

struct Response {
  std::string text;
  bool ok = false;
};

std::string utf8(hstring value) { return to_string(value); }

hstring text(std::string_view value) { return to_hstring(std::string(value)); }

Response take_response(char *raw) {
  auto owned = msime::host_api::own_string(raw);
  if (!owned) {
    return {"{\"ok\":false,\"error\":\"empty host response\"}", false};
  }
  std::string value(owned.get());
  try {
    const auto object = JsonObject::Parse(text(value));
    return {std::move(value), object.GetNamedBoolean(L"ok", false)};
  } catch (...) {
    return {std::move(value), false};
  }
}

struct AccountNamePresentation {
  std::wstring normalized;
  std::wstring preferred;
  bool valid = false;
  int32_t maximum_utf16_units = 0;
};

// 纯计算接口可在 UI 线程调用；失败时不猜测昵称规则，也不提交未经校验的值。
std::optional<AccountNamePresentation> account_name_presentation(std::wstring const &name,
                                                               std::wstring const &id = {}) {
  try {
    JsonObject request;
    request.SetNamedValue(L"name", JsonValue::CreateStringValue(hstring(name)));
    request.SetNamedValue(L"id", JsonValue::CreateStringValue(hstring(id)));
    const auto body = utf8(request.Stringify());
    const auto response = take_response(msime_client_account_name(
        reinterpret_cast<const uint8_t *>(body.data()), body.size()));
    if (!response.ok)
      return std::nullopt;
    const auto value = JsonObject::Parse(text(response.text)).GetNamedObject(L"value");
    const auto maximum = value.GetNamedNumber(L"maximum_utf16_units");
    if (maximum < 1 || maximum > INT32_MAX || std::floor(maximum) != maximum)
      return std::nullopt;
    const auto normalized = value.GetNamedString(L"normalized");
    const auto preferred = value.GetNamedString(L"preferred");
    return AccountNamePresentation{std::wstring(normalized.c_str(), normalized.size()),
                                   std::wstring(preferred.c_str(), preferred.size()),
                                   value.GetNamedBoolean(L"valid"), static_cast<int32_t>(maximum)};
  } catch (...) {
    return std::nullopt;
  }
}

std::wstring preferred_account_name(std::wstring const &name, std::wstring const &id) {
  if (const auto presentation = account_name_presentation(name, id))
    return presentation->preferred;
  return name.empty() ? L"水杉账号" : name;
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

std::filesystem::path injected_state_directory() {
  return std::filesystem::path(environment(L"MSIME_CLIENT_STATE_DIR"));
}

std::filesystem::path runtime_options_directory() {
  if (auto injected = injected_state_directory(); injected.is_absolute())
    return injected;
  return msime::windows::resolve_state_directory();
}

std::filesystem::path configured_preferences_directory(
    const std::filesystem::path &directory) {
  if (directory.empty())
    return {};
  std::error_code error;
  const auto file = directory / L"runtime-options.json";
  if (!std::filesystem::is_regular_file(file, error))
    return {};
  std::ifstream stream(file, std::ios::binary);
  const std::string content((std::istreambuf_iterator<char>(stream)),
                            std::istreambuf_iterator<char>());
  JsonObject options{nullptr};
  if (!JsonObject::TryParse(text(content), options) ||
      !options.HasKey(L"preferences_directory"))
    return {};
  const auto value = options.Lookup(L"preferences_directory");
  if (value.ValueType() != JsonValueType::String)
    return {};
  return std::filesystem::path(std::wstring(value.GetString().c_str()));
}

std::filesystem::path state_directory() {
  const auto root = msime::windows::resolve_state_directory();
  return msime::settings::settings_state_directory(
      injected_state_directory(), root, configured_preferences_directory(root));
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

// 本版本的文本服务和语言 profile，与 TSF 的 Globals.cpp 和看门狗读同一份 shared/contracts/msime_edition.h。
constexpr GUID input_method_clsid = MSIME_EDITION_CLSID;
constexpr GUID input_method_profile = MSIME_EDITION_PROFILE_GUID;
// The same profile in the <LangID>:<CLSID><profile> form InstallLayoutOrTip and SetDefaultLayoutOrTip take.
constexpr const wchar_t *input_method_id =
    MSIME_EDITION_LANGID_STRING L":" MSIME_EDITION_CLSID_STRING
    MSIME_EDITION_PROFILE_GUID_STRING;

// 本版本是否提供这个方案（版本表 input_schemes）。不提供的方案不出现在输入方案的选项里。
bool edition_offers_scheme(std::wstring_view scheme) {
  for (const char *name : {MSIME_EDITION_INPUT_SCHEMES}) {
    const std::string_view narrow(name);
    if (scheme.size() == narrow.size() &&
        std::equal(narrow.begin(), narrow.end(), scheme.begin(),
                   [](char left, wchar_t right) { return static_cast<wchar_t>(left) == right; }))
      return true;
  }
  return false;
}

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

// 保留启动请求的类别，窗口解析承载页时也恢复旧路由对应的顶部标签。
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
  return page.value_or(std::string());
}

// The runtime options file the Server hands this window, or the one the Server reads at startup when started from the Start menu. None before the input method is set up.
std::optional<std::filesystem::path> runtime_options_file() {
  std::error_code error;
  if (const auto raw = environment(L"MSIME_CLIENT_HOST_OPTIONS"); !raw.empty()) {
    std::filesystem::path path(raw);
    if (path.is_absolute() && std::filesystem::is_regular_file(path, error))
      return path;
  }
  const auto directory = runtime_options_directory();
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

// ---- 词库 ----

// The host options the input method runs with, which every dictionary request carries; absent until the setup wizard has prepared them.
std::optional<std::string> runtime_options_document() {
  const auto file = runtime_options_file();
  if (!file)
    return std::nullopt;
  std::ifstream stream(*file, std::ios::binary);
  std::string content((std::istreambuf_iterator<char>(stream)),
                      std::istreambuf_iterator<char>());
  JsonObject parsed{nullptr};
  if (content.empty() || !JsonObject::TryParse(text(content), parsed))
    return std::nullopt;
  return content;
}

// One request of the 词库 page through msime_client_settings_dictionary, the same code the Tauri settings page runs: a write first has the Server let go of its sessions, and a refusal comes back as the page's code. Blocks, so only off the UI thread.
Response settings_dictionary_call(std::string const &options, std::string const &action) {
  const auto body = "{\"options\":" + options + ",\"action\":" + action + "}";
  return take_response(msime_client_settings_dictionary(
      reinterpret_cast<const uint8_t *>(body.data()), body.size()));
}

// The dictionaries the page manages, in the shared page's order and words.
constexpr std::array<std::pair<std::wstring_view, std::wstring_view>, 5> lexicon_kinds{{
    {L"pinyin", L"拼音"},
    {L"wubi", L"86 五笔"},
    {L"wubi98", L"98 五笔"},
    {L"english", L"英文"},
    {L"quick_phrase", L"快捷短语"},
}};

std::wstring lexicon_kind_label(std::wstring_view kind) {
  for (auto const &[id, label] : lexicon_kinds)
    if (id == kind)
      return std::wstring(label);
  return L"词库";
}

// What a code looks like in each dictionary, as the shared page hints it.
std::wstring lexicon_key_hint(std::wstring_view kind) {
  if (kind == L"wubi" || kind == L"wubi98")
    return L"1–4 个字母";
  if (kind == L"quick_phrase")
    return L"1–32 个字母";
  if (kind == L"english")
    return L"1–64 个字母";
  if (kind == L"pinyin")
    return L"完整音节，用 ' 分隔，如 ni'hao";
  return {};
}

// The shared page's sentence for a dictionary refusal code (dictionaryErrorMessage in packages/ui/src/dictionary/dictionary-errors.ts).
std::wstring dictionary_error_text(std::wstring const &code, std::wstring_view kind,
                                   std::wstring const &fallback) {
  // 细胞词库转换（msime_client_dictionary_scel_text）的出错说明。
  if (auto scel = msime::settings::lexicon_import::scel_error_text(code); !scel.empty())
    return scel;
  if (code == L"dictionary_busy")
    return L"词库正在被输入法占用，请关闭正在使用输入法的程序后重试。";
  if (code == L"dictionary_import_rejected")
    return L"词库拒绝了这次写入，请检查编码与词是否匹配。";
  if (code == L"dictionary_too_large")
    return L"词库文件过大：文件不能超过 32 MB，单行不能超过 60 KB，请拆分后再导入。";
  if (code == L"dictionary_read_rejected")
    return L"词库拒绝了这次读取，请稍后重试。";
  if (code == L"dictionary_bundled_readonly")
    return L"内置词条只能调整权重或删除，不能修改编码和词。";
  if (code == L"dictionary_pinyin_unavailable")
    return L"拼音表不可用，无法校验这条词的读音。";
  if (code == L"dictionary_reset_rejected")
    return L"清除学习数据失败，请关闭正在使用输入法的程序后重试。";
  if (code == L"dictionary_unavailable")
    return L"无法打开用户词库，请检查输入法是否正在运行。";
  if (code == L"dictionary_invalid_word")
    return L"词条内容为空、过长或含控制字符，或权重超出 1 到 100000000 的范围。";
  if (code == L"dictionary_invalid_entry") {
    if (kind == L"pinyin")
      return L"拼音必须由完整音节组成，音节数需与汉字数一致，例如“你好”填 nihao 或 ni'hao。";
    if (kind == L"wubi" || kind == L"wubi98")
      return L"五笔编码须为 1 到 4 个字母。";
    if (kind == L"quick_phrase")
      return L"快捷短语编码只能包含英文字母，长度 1 到 32。";
    if (kind == L"english")
      return L"英文编码只能包含字母、连字符和撇号。";
    return L"词条不符合词库规则，请检查编码与词条后再保存。";
  }
  return fallback;
}

// The user's local day, YYYY-MM-DD, which 背单词 counts its reviews by.
std::wstring local_day() {
  SYSTEMTIME now{};
  GetLocalTime(&now);
  wchar_t day[16] = {};
  swprintf_s(day, L"%04u-%02u-%02u", now.wYear, now.wMonth, now.wDay);
  return day;
}

std::wstring named_string(JsonObject const &object, wchar_t const *key) {
  const auto value = object.GetNamedValue(key, JsonValue::CreateNullValue());
  return value.ValueType() == JsonValueType::String ? std::wstring(value.GetString().c_str())
                                                    : std::wstring();
}

int64_t named_count(JsonObject const &object, wchar_t const *key) {
  const auto value = object.GetNamedValue(key, JsonValue::CreateNullValue());
  return value.ValueType() == JsonValueType::Number
             ? static_cast<int64_t>(std::llround(value.GetNumber()))
             : 0;
}

// A request id the host keeps receipts by, so a retried write is not applied twice.
std::wstring request_id(std::wstring_view prefix) {
  const auto now = std::chrono::duration_cast<std::chrono::milliseconds>(
                       std::chrono::system_clock::now().time_since_epoch())
                       .count();
  return std::wstring(prefix) + std::to_wstring(now);
}

// ---- 剪贴板 ----

// The account directory the shared app signs in to (windows_account.rs) and the Server reads (server_main.cpp's anonymous_account_directory): %LOCALAPPDATA%\<this edition's user directory>\account. Empty when the known folder cannot be read.
std::filesystem::path account_directory() {
  PWSTR local = nullptr;
  if (FAILED(SHGetKnownFolderPath(FOLDERID_LocalAppData, 0, nullptr, &local))) {
    CoTaskMemFree(local);
    return {};
  }
  const auto directory =
      std::filesystem::path(local) / MSIME_EDITION_USER_DATA_DIRECTORY / L"account";
  CoTaskMemFree(local);
  return directory;
}

// Where older versions of the shared app kept the session (its Tauri app_local_data_dir, %LOCALAPPDATA%\<this edition's Tauri identifier>); msime_client_account moves a session found there into account_directory() the first time, as the shared app does on start. Empty when the known folder cannot be read.
std::filesystem::path legacy_account_directory() {
  PWSTR local = nullptr;
  if (FAILED(SHGetKnownFolderPath(FOLDERID_LocalAppData, 0, nullptr, &local))) {
    CoTaskMemFree(local);
    return {};
  }
  const auto directory = std::filesystem::path(local) / MSIME_EDITION_TAURI_IDENTIFIER;
  CoTaskMemFree(local);
  return directory;
}

// A history entry's time as the shared page writes it (`new Date(timestampMs).toLocaleString()`): the short date and the time in the user's locale and time zone.
std::wstring local_date_time(uint64_t milliseconds) {
  // FILETIME counts 100-nanosecond intervals from 1601; anything past the year 30000 is not a real capture.
  constexpr uint64_t unix_epoch = 116444736000000000ULL;
  if (milliseconds > 900000000000000ULL)
    return {};
  ULARGE_INTEGER ticks{};
  ticks.QuadPart = milliseconds * 10000ULL + unix_epoch;
  const FILETIME utc{ticks.LowPart, ticks.HighPart};
  SYSTEMTIME universal{}, local{};
  if (!FileTimeToSystemTime(&utc, &universal) ||
      !SystemTimeToTzSpecificLocalTime(nullptr, &universal, &local))
    return {};
  wchar_t date[80] = {};
  wchar_t time[80] = {};
  if (!GetDateFormatEx(LOCALE_NAME_USER_DEFAULT, DATE_SHORTDATE, &local, nullptr, date, 80,
                       nullptr) ||
      !GetTimeFormatEx(LOCALE_NAME_USER_DEFAULT, 0, &local, nullptr, time, 80))
    return {};
  return std::wstring(date) + L" " + time;
}

// An entry's text on one line, as the shared row shows it (`white-space: nowrap`): every run of white space, line breaks included, becomes one space.
std::wstring single_line(std::wstring const &value) {
  std::wstring line;
  line.reserve(value.size());
  bool space = false;
  for (const wchar_t c : value) {
    if (c == L' ' || c == L'\t' || c == L'\r' || c == L'\n' || c == L'\f') {
      space = true;
      continue;
    }
    if (space && !line.empty())
      line.push_back(L' ');
    space = false;
    line.push_back(c);
  }
  return line;
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

// 本版本在 AI 助手配置里登记的服务器名，与 client-core 的 `Edition::mcp_server_name` 相同：full 是 msime，其他版本是 msime-<id>。
std::wstring mcp_server_name() {
  const std::string_view id = MSIME_EDITION_ID;
  return MSIME_EDITION_IS_FULL ? std::wstring(L"msime")
                               : L"msime-" + std::wstring(id.begin(), id.end());
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

// PreferencePath.h walks preference paths over this: WinRT JSON objects share by reference, so a nested object found on the way is edited in place.
struct JsonPreferenceTree {
  using Object = JsonObject;
  using Value = IJsonValue;
  static std::optional<JsonObject> object(JsonObject const &parent, std::wstring const &name) {
    const hstring key(name);
    if (!parent.HasKey(key))
      return std::nullopt;
    const auto value = parent.Lookup(key);
    if (value.ValueType() != JsonValueType::Object)
      return std::nullopt;
    return value.GetObject();
  }
  static IJsonValue value(JsonObject const &parent, std::wstring const &name) {
    const hstring key(name);
    return parent.HasKey(key) ? parent.Lookup(key) : nullptr;
  }
  static IJsonValue none() { return nullptr; }
  static JsonObject copy(JsonObject const &source) { return JsonObject::Parse(source.Stringify()); }
  static JsonObject empty() { return JsonObject(); }
  static void set(JsonObject const &parent, std::wstring const &name, IJsonValue const &value) {
    parent.SetNamedValue(hstring(name), value);
  }
  static void set_object(JsonObject const &parent, std::wstring const &name, JsonObject const &child) {
    parent.SetNamedValue(hstring(name), child);
  }
  static void remove(JsonObject const &parent, std::wstring const &name) {
    if (const hstring key(name); parent.HasKey(key))
      parent.Remove(key);
  }
};

std::optional<JsonObject> present(JsonObject const &object) {
  return object ? std::optional<JsonObject>(object) : std::nullopt;
}

// The stored value at a path (see PreferencePath.h), or null when it is absent.
IJsonValue lookup(JsonObject const &root, std::vector<std::wstring> const &path) {
  return nav::find_preference<JsonPreferenceTree>(present(root), path);
}

uint64_t strict_revision(JsonObject const &object, std::wstring_view key,
                         uint64_t fallback) {
  if (!object.HasKey(hstring(key))) return fallback;
  const auto value = object.Lookup(hstring(key));
  if (value.ValueType() != JsonValueType::Number) throw std::runtime_error("invalid revision");
  const double raw = value.GetNumber();
  // WinRT exposes JSON numbers as double; keep only exact non-negative integers.
  constexpr double kMaxExactJsonInteger = 9007199254740991.0; // 2^53 - 1
  if (!std::isfinite(raw) || raw < 0 || std::trunc(raw) != raw || raw > kMaxExactJsonInteger)
    throw std::runtime_error("invalid revision");
  return static_cast<uint64_t>(raw);
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
      revision_ = strict_revision(document_, L"revision", 0);
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
      revision_ = strict_revision(document_, L"revision", revision_);
      return true;
    } catch (...) {
      error = L"设置已写入，但返回内容无法读取。";
      return false;
    }
  }

  uint64_t revision() const { return revision_; }

  // The stored value, else the shipped default, else null. A stored null (an unset optional field) is returned as it is.
  // A key is a dotted path of any depth, such as `plugins.key_sound.enabled`.
  IJsonValue Value(std::wstring_view key) const {
    return value_at(nav::preference_path(key));
  }

  // The shipped default for a key, for the rows that restore defaults.
  IJsonValue Default(std::wstring_view key) const {
    return lookup(defaults_, nav::preference_path(key));
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
    result.reserve(value.GetArray().Size());
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

  // Stores the list sorted and without repeats, for the lists whose order means nothing.
  void SetStrings(std::wstring_view key, std::vector<std::wstring> values) {
    std::sort(values.begin(), values.end());
    values.erase(std::unique(values.begin(), values.end()), values.end());
    SetOrderedStrings(key, values);
  }

  // Stores the list exactly as given, for the lists whose order is the user's.
  void SetOrderedStrings(std::wstring_view key, std::vector<std::wstring> const &values) {
    JsonArray array;
    for (auto const &value : values)
      array.Append(JsonValue::CreateStringValue(hstring(value)));
    SetValue(key, array);
  }

  // One entry of a string map such as `ai_assistant.tokens`. The entry key is taken literally, dots included, so an origin can be a key.
  std::wstring Entry(std::wstring_view map_key, std::wstring_view entry,
                     std::wstring_view fallback) const {
    const auto value = value_at(nav::preference_entry_path(map_key, entry));
    return value && value.ValueType() == JsonValueType::String
               ? std::wstring(value.GetString().c_str())
               : std::wstring(fallback);
  }

  void SetEntry(std::wstring_view map_key, std::wstring_view entry, std::wstring_view value) {
    set_at(nav::preference_entry_path(map_key, entry), JsonValue::CreateStringValue(hstring(value)));
  }

  void RemoveEntry(std::wstring_view map_key, std::wstring_view entry) {
    if (preferences_)
      nav::remove_preference<JsonPreferenceTree>(preferences_,
                                                 nav::preference_entry_path(map_key, entry));
  }

  // Replaces a value with a copy of its shipped default. A key without a default is left as it is.
  void Restore(std::wstring_view key) {
    const auto value = Default(key);
    if (value)
      SetValue(key, JsonValue::Parse(value.Stringify()));
  }

  // Creates the objects missing on the way, each from its shipped default (see PreferencePath.h).
  void SetValue(std::wstring_view key, IJsonValue const &value) {
    set_at(nav::preference_path(key), value);
  }

  // Drops a stored key at any depth, so reads fall back to the shipped default. Nothing is created on the way.
  void Remove(std::wstring_view key) {
    if (preferences_)
      nav::remove_preference<JsonPreferenceTree>(preferences_, nav::preference_path(key));
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

  // Value() and Entry() on a path already split.
  IJsonValue value_at(std::vector<std::wstring> const &path) const {
    if (auto value = lookup(preferences_, path))
      return value;
    return lookup(defaults_, path);
  }

  void set_at(std::vector<std::wstring> const &path, IJsonValue const &value) {
    if (preferences_)
      nav::set_preference<JsonPreferenceTree>(preferences_, present(defaults_), path, value);
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

// 设置与输入浮层采用 macOS 的品牌绿及分层表面；Windows 控件仍由 WinUI 提供。
struct Palette {
  bool dark = false;
  Color text, sub, faint, subtle, subtle2, card, stroke, stroke2, divider,
      accent, on_accent, seg_well, preview_bg, window_bg, content_bg, edge;
};

Palette make_palette(bool dark) {
  Palette p;
  p.dark = dark;
  p.accent = dark ? rgba(0x5F, 0xBF, 0x84, 1) : rgba(0x2C, 0x7A, 0x4B, 1);
  p.on_accent = rgba(255, 255, 255, 1);
  if (dark) {
    p.text = rgba(0xF5, 0xF5, 0xF7, 1);
    p.sub = rgba(0x98, 0x98, 0x9D, 1);
    p.faint = rgba(255, 255, 255, .6);
    p.subtle = rgba(255, 255, 255, .0605);
    p.subtle2 = rgba(255, 255, 255, .0837);
    p.card = rgba(0x35, 0x35, 0x35, 1);
    p.stroke = rgba(255, 255, 255, .07);
    p.stroke2 = rgba(255, 255, 255, .093);
    p.divider = rgba(255, 255, 255, .0837);
    p.seg_well = rgba(0, 0, 0, .12);
    p.preview_bg = rgba(0x2C, 0x2C, 0x2C, 1);
    p.window_bg = rgba(0x2A, 0x2A, 0x2A, 1);
    p.content_bg = rgba(0x1E, 0x1E, 0x1E, 1);
    p.edge = rgba(0, 0, 0, .1);
  } else {
    p.text = rgba(0x1D, 0x1D, 0x1F, 1);
    p.sub = rgba(0x6E, 0x6E, 0x73, 1);
    p.faint = rgba(0, 0, 0, .45);
    p.subtle = rgba(0, 0, 0, .0373);
    p.subtle2 = rgba(0, 0, 0, .0578);
    p.card = rgba(255, 255, 255, 1);
    p.stroke = rgba(0, 0, 0, .0578);
    p.stroke2 = rgba(0, 0, 0, .0803);
    p.divider = rgba(0, 0, 0, .0803);
    p.seg_well = rgba(0, 0, 0, .04);
    p.preview_bg = rgba(255, 255, 255, 1);
    p.window_bg = rgba(0xF6, 0xF6, 0xF6, 1);
    p.content_bg = rgba(0xF6, 0xF6, 0xF6, 1);
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

// 页面名称、说明与侧栏图标，按稳定的路由 id 取值。
struct PageLabel {
  std::string_view id;
  const wchar_t *title;
  wchar_t glyph;
  const wchar_t *summary;
};

// 「手写输入」并进了「屏幕键盘」页（见 `SettingsNavigation.h`），提供手写的版本里那一页叫「键盘与手写」，与共享设置 UI 的 `screenKeyboardAndHandwritingTitle` 相同。
constexpr std::array<PageLabel, 16> page_labels{{
    {"typing", L"输入", 0xE765, L"选择输入方式；专属选项随当前方案变化。"},
    {"expression", L"标点与翻译", 0xE8C1, L"调整标点、混输、输出和选词习惯。"},
    {"shortcuts", L"快捷键", 0xEDA7, L"设置中英文切换、选词和翻页使用的按键。"},
    {"lexicon", L"词库", 0xE82D, L"管理本机词库、个人词条和常用语。"},
    {"themes", L"主题", 0xE790, L"选择皮肤与配色；布局、字体和显示方式在「候选窗口」中设置。"},
    {"candidate", L"候选窗口", 0xE8FD, L"调整布局、字体和显示方式；皮肤与配色在「主题」中设置。"},
    {"toolbar", L"悬浮工具栏", 0xE7F4, L"选择悬浮工具栏显示的内容和交互方式。"},
    {"osk", MSIME_EDITION_HANDWRITING ? L"键盘与手写" : L"屏幕键盘", 0xE92E,
     MSIME_EDITION_HANDWRITING ? L"设置屏幕键盘的布局、按键和外观，打开手写识别板。"
                               : L"设置屏幕键盘的布局、按键和外观。"},
    {"voice", L"语音输入", 0xE720, L"选择识别方式，配置语音转写和文本整理。"},
    {"clip", L"剪贴板", 0xE77F, L"管理剪贴板记录和跨设备发送。"},
    {"stats", L"打字统计", 0xE9D2, L"查看输入量、速度和使用趋势。"},
    {"plugins", L"插件", 0xEA86, L"管理输入扩展，浏览和安装社区插件。"},
    {"ai", L"AI", 0xE99A, L"配置 AI 辅助功能，连接本机 AI 助手。"},
    {"account", L"账号与同步", 0xE77B, L"登录并管理跨设备同步与云端内容。"},
    {"dev", L"维护与诊断", 0xE90F, L"导入导出设置，检查本机状态和诊断信息。"},
    {"about", L"关于", 0xE946, L"查看版本和更新，查阅使用帮助、反馈问题，以及许可信息。"},
}};
static_assert(page_labels.size() == nav::pages.size());
static_assert(page_labels[0].id == nav::default_page);

// 侧栏的分组标题，按 `SettingsNavigation.h` 里的 `group` 索引，措辞与共享设置 UI 的 `settingsNavGroups` 一致。
constexpr std::array<const wchar_t *, nav::page_group_count> group_titles{
    {L"打字", L"输入方式", L"外观", L"工具", L"数据", L"支持"}};

// 未知 id 取默认页面的标签。
const PageLabel &page_label(std::string_view id) {
  for (const auto &label : page_labels)
    if (label.id == id)
      return label;
  return page_labels[0];
}

// The line the search list shows when nothing matches. It is a list item, so it can be highlighted and submitted like a result; the search box treats it as neither.
constexpr std::wstring_view no_search_results = L"没有找到相关设置";

constexpr const wchar_t *download_url = L"https://msime.app/download/";
constexpr const wchar_t *privacy_url = L"https://msime.app/privacy/";
// 共享设置「帮助」页「完整文档」打开的地址（`packages/ui/src/settings/app-resources.ts` 的 `documentationUrl`）。
constexpr const wchar_t *documentation_url = L"https://msime.app/docs/";
constexpr const wchar_t *license_url =
    L"https://github.com/metasequoiaime/msime/blob/develop/LICENSE";
constexpr const wchar_t *releases_url =
    L"https://github.com/metasequoiaime/msime/releases";

// 本次构建运行时的版本号：发布构建取 `Build-Client.ps1` 打上的值（`TargetVersion`），开发构建取 `platforms/windows/version.txt`，与 Server 报告的值相同。
constexpr std::wstring_view app_version = L"" MSIME_WINDOWS_VERSION;

// 本次构建的发布日期 `YYYY-MM-DD`，由 `MSIME.Settings.vcxproj` 依次取 `MSIME_RELEASE_DATE`、`platforms/windows/release-date.txt` 和 git 当前提交的日期，都没有时为空。只加在「关于」页的版本文字上，检查更新和反馈报告仍用 `app_version`。
constexpr std::wstring_view app_release_date = L"" MSIME_WINDOWS_RELEASE_DATE;

// 「关于」页的版本文字：`v0.2.1（2026-10-19 发布）`，没有发布日期时只是 `v0.2.1`。
std::wstring app_version_label() {
  std::wstring label = L"v" + std::wstring(app_version);
  if (!app_release_date.empty())
    label += L"（" + std::wstring(app_release_date) + L" 发布）";
  return label;
}

// 检查更新的结果，措辞与共享设置页一致。
struct UpdateOutcome {
  std::wstring status;
  // 未签名构建的警告以及如何校验安装程序；只有提供了更新的发布时才非空。
  std::wstring detail;
  // 要打开的发布页面；只有提供了更新的发布时才非空。
  std::wstring release_url;
};

// `msime_client_update_check` 的请求：本版本的 Windows 发布。不带架构：同一个安装程序同时支持 x64 和 Arm64。
JsonObject update_check_request() {
  JsonObject request;
  request.SetNamedValue(L"platform", JsonValue::CreateStringValue(L"windows"));
  request.SetNamedValue(L"current_version",
                        JsonValue::CreateStringValue(hstring(app_version)));
  request.SetNamedValue(L"edition", JsonValue::CreateStringValue(text(MSIME_EDITION_ID)));
  return request;
}

// 系统版本「主.次.内部版本号」，写进反馈报告的诊断信息。取自 ntdll 的 `RtlGetVersion`：`GetVersionEx` 按应用清单声明的兼容系统作答，可能报低。Windows 11 的主版本号仍是 10，内部版本号从 22000 起。读不到时为空，报告改写「平台：windows」。
std::wstring windows_os_version() {
  using RtlGetVersion = LONG(WINAPI *)(PRTL_OSVERSIONINFOW);
  const auto ntdll = GetModuleHandleW(L"ntdll.dll");
  const auto get = ntdll ? reinterpret_cast<RtlGetVersion>(reinterpret_cast<void *>(
                               GetProcAddress(ntdll, "RtlGetVersion")))
                         : nullptr;
  RTL_OSVERSIONINFOW info{};
  info.dwOSVersionInfoSize = sizeof(info);
  if (!get || get(&info) != 0)
    return {};
  return std::to_wstring(info.dwMajorVersion) + L"." + std::to_wstring(info.dwMinorVersion) +
         L"." + std::to_wstring(info.dwBuildNumber);
}

// 本版本发布的安装程序名，用于发布里没有给出安装程序时的警告：`MetasequoiaIME-<Id>_Setup_v<版本>.exe`，与 `update_check.rs` 和共享页面的推导方式相同。
std::wstring installer_placeholder() {
  std::wstring id(text(MSIME_EDITION_ID).c_str());
  if (!id.empty())
    id[0] = static_cast<wchar_t>(std::towupper(id[0]));
  return L"MetasequoiaIME-" + id + L"_Setup_v<版本>.exe";
}

// 读取 `msime_client_update_check` 的答复。不在本仓库 tag 下的发布页面视为检查失败，不会打开。
UpdateOutcome describe_update_check(Response const &response) {
  const UpdateOutcome failed{L"检查失败，请稍后重试", {}, {}};
  if (!response.ok)
    return failed;
  try {
    const auto value = JsonObject::Parse(text(response.text)).GetNamedObject(L"value");
    const std::wstring status(value.GetNamedString(L"status", L"").c_str());
    if (status == L"none")
      return {L"暂无可用发行版", {}, {}};
    const auto update = value.GetNamedObject(L"update");
    const std::wstring version(
        update.GetNamedObject(L"version").GetNamedString(L"display", L"").c_str());
    const std::wstring url(update.GetNamedString(L"release_url", L"").c_str());
    if (version.empty() || !url.starts_with(std::wstring(releases_url) + L"/tag/"))
      return failed;
    if (status == L"current")
      return {L"已是最新版本", {}, {}};
    if (status != L"available")
      return failed;
    UpdateOutcome outcome{L"发现新版本 v" + version, {}, url};
    const auto is_string = [&](wchar_t const *key) {
      return update.GetNamedValue(key, JsonValue::CreateNullValue()).ValueType() ==
             JsonValueType::String;
    };
    const auto signed_value = update.GetNamedValue(L"signed", JsonValue::CreateNullValue());
    if (signed_value.ValueType() == JsonValueType::Boolean && !signed_value.GetBoolean()) {
      // 沿用正式发布的设置页的措辞：未签名的安装程序不只会触发 SmartScreen 提示，还会失去 uiAccess。
      const std::wstring name = is_string(L"installer_name")
                                    ? std::wstring(update.GetNamedString(L"installer_name").c_str())
                                    : installer_placeholder();
      outcome.detail =
          L"该版本未经代码签名，SmartScreen 会拦截，且 uiAccess 失效（候选窗口无法浮在以管理员身份运行的程序之上）。";
      if (is_string(L"installer_sha256"))
        outcome.detail += L"下载 " + name + L" 后请核对 SHA256：" +
                          std::wstring(update.GetNamedString(L"installer_sha256").c_str());
      else
        outcome.detail += L"请从发行页一并下载 " + name + L".sha256 核对。";
    }
    return outcome;
  } catch (...) {
    return failed;
  }
}

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
  // 不可选的选项照常显示，但选不了：分段控件的按钮按不下去（「候选字体」在 Windows 上的「圆体」），下拉框里那一项是灰的（「麦克风」里已忽略的其他平台录音设置和未连接的已选设备）。
  bool enabled = true;
};

// ---- 系统字体列表 ----

// 共享页用 localeCompare 给字体列表排序；这里按用户的区域设置比较。
bool font_name_before(std::wstring const &a, std::wstring const &b) {
  return CompareStringEx(LOCALE_NAME_USER_DEFAULT, 0, a.c_str(), static_cast<int>(a.size()),
                         b.c_str(), static_cast<int>(b.size()), nullptr, nullptr,
                         0) == CSTR_LESS_THAN;
}

// 共享页用 toLocaleLowerCase 筛选字体；这里按用户的区域设置转小写，ASCII 以外的字母也能不分大小写地匹配。转换失败时保留原样。
std::wstring font_name_lower(std::wstring_view value) {
  if (value.empty())
    return {};
  constexpr DWORD flags = LCMAP_LOWERCASE | LCMAP_LINGUISTIC_CASING;
  const int length = static_cast<int>(value.size());
  const int needed = LCMapStringEx(LOCALE_NAME_USER_DEFAULT, flags, value.data(), length,
                                   nullptr, 0, nullptr, nullptr, 0);
  if (needed <= 0)
    return std::wstring(value);
  std::wstring result(static_cast<std::size_t>(needed), L'\0');
  if (LCMapStringEx(LOCALE_NAME_USER_DEFAULT, flags, value.data(), length, result.data(),
                    needed, nullptr, nullptr, 0) != needed)
    return std::wstring(value);
  return result;
}

// 已安装的字体名，与 Tauri 的 list_font_families 读的是同一份目录（host-api 的 system_fonts）。读取失败、返回内容不是字符串数组或含不合法的名字时为空，页面显示读取失败。枚举系统字体，只在 UI 线程之外调用。
std::optional<std::vector<std::wstring>> read_font_catalog() {
  const auto response = take_response(msime_client_list_font_families());
  if (!response.ok)
    return std::nullopt;
  try {
    const auto value = JsonObject::Parse(text(response.text)).GetNamedArray(L"value");
    if (value.Size() > nav::font_catalog_limit)
      return std::nullopt;
    std::vector<std::wstring> fonts;
    fonts.reserve(value.Size());
    for (auto const &item : value) {
      if (item.ValueType() != JsonValueType::String)
        return std::nullopt;
      fonts.emplace_back(item.GetString().c_str());
    }
    return nav::normalize_font_catalog(std::move(fonts), font_name_before);
  } catch (...) {
    return std::nullopt;
  }
}

// ---- 辅助码方案的来源 ----

// 资源目录 helpcodes/custom 下的自定义辅助码表，与 Tauri 的 list_helpcode_schemas 相同：资源目录是输入法宿主选项（runtime-options.json）的 resources。还没有宿主选项时是失败的响应，页面当作没有自定义码表。读目录，只在 UI 线程之外调用。
Response helpcode_schemas_call() {
  const auto options = runtime_options_document();
  std::wstring resources;
  if (options) {
    try {
      resources = JsonObject::Parse(text(*options)).GetNamedString(L"resources", L"").c_str();
    } catch (...) {
      resources.clear();
    }
  }
  // 与 Tauri 一样只接受绝对路径。
  if (resources.empty() || !std::filesystem::path(resources).is_absolute())
    return {"{\"ok\":false,\"error\":\"no host options\"}", false};
  const auto path = utf8(hstring(resources));
  return take_response(msime_client_helpcode_schemas(
      reinterpret_cast<const uint8_t *>(path.data()), path.size()));
}

// 本安装随包带的内置声音包：Server 状态目录下的 sound-packs，与 Tauri 的 builtin_sound_packs 在 Windows 上取的 `server_state_directory()/sound-packs` 相同（StateDirectory.h 的 resolve_state_directory 是同一顺序的 C++ 版本）。目录不在时为空，只列已安装的包。会查文件系统，只在 UI 线程之外调用。
std::filesystem::path builtin_sound_packs() {
  const auto root = msime::windows::resolve_state_directory();
  if (root.empty() || !root.is_absolute())
    return {};
  auto directory = root / L"sound-packs";
  std::error_code error;
  return std::filesystem::is_directory(directory, error) ? directory : std::filesystem::path();
}

// 桌面安装包随带的官方插件包（uuid 脚本插件、latex-symbols 短语表）：本程序所在目录（安装后是 Program Files 下的 Server 目录，只有管理员能写）的 plugin-packs，与 Server 写进会话选项的 plugin_packs（src/system/PluginPackRoot.h）、Tauri 的 builtin_plugin_packs 在 Windows 上取的是同一个目录。目录不在（开发运行）时也照样给出：不给的话 client-services 会去内置音效包旁边找，而内置音效包在普通用户可写的数据目录里。只取模块路径，不碰文件系统。
std::filesystem::path official_plugin_packs() {
  std::wstring buffer(MAX_PATH, L'\0');
  for (;;) {
    const DWORD length =
        GetModuleFileNameW(nullptr, buffer.data(), static_cast<DWORD>(buffer.size()));
    if (length == 0)
      return {};
    if (length < buffer.size()) {
      buffer.resize(length);
      break;
    }
    buffer.resize(buffer.size() * 2);
  }
  return std::filesystem::path(buffer).parent_path() / L"plugin-packs";
}

// 输入法宿主选项（runtime-options.json）里的 user_data：输入法把停用了的脚本插件记在它下面的 script-health.json 里，与 Tauri 的 user_data 取的是同一项。还没有宿主选项或不是绝对路径时为空，授权页就不显示停用记录。读文件，只在 UI 线程之外调用。
std::wstring runtime_user_data() {
  const auto options = runtime_options_document();
  if (!options)
    return {};
  try {
    std::wstring value = JsonObject::Parse(text(*options)).GetNamedString(L"user_data", L"").c_str();
    return !value.empty() && std::filesystem::path(value).is_absolute() ? value : std::wstring();
  } catch (...) {
    return {};
  }
}

// 一次 msime_client_plugins 请求：插件目录是数据目录下的 plugins，与 Tauri 的 PluginsState 相同；sounds 为真时带上内置声音包的目录。script_page 为真时这是「插件」页自己的请求：本窗口有脚本插件的授权页，目录列出脚本包、导入接受脚本包，并带上输入法的 user_data 读停用记录；社区发布、辅助码方案这些别处的请求不带它，看不到脚本包。这两种请求都带上官方插件包的目录：目录要列出官方包，授权要从那里读官方脚本插件、核对它的哈希。读写文件，只在 UI 线程之外调用。
Response plugins_call(JsonObject const &action, bool sounds, bool script_page = false) {
  const auto root = state_directory();
  if (root.empty() || !root.is_absolute())
    return {"{\"ok\":false,\"error\":\"plugin_storage\"}", false};
  JsonObject request;
  request.SetNamedValue(L"state_root", JsonValue::CreateStringValue(hstring(root.wstring())));
  const auto packs = sounds ? builtin_sound_packs() : std::filesystem::path();
  request.SetNamedValue(L"sound_packs", packs.empty()
                                            ? JsonValue::CreateNullValue()
                                            : JsonValue::CreateStringValue(hstring(packs.wstring())));
  if (sounds || script_page) {
    if (const auto official = official_plugin_packs(); official.is_absolute())
      request.SetNamedValue(L"plugin_packs", JsonValue::CreateStringValue(hstring(official.wstring())));
  }
  if (script_page) {
    request.SetNamedValue(L"script_page", JsonValue::CreateBooleanValue(true));
    if (const auto user_data = runtime_user_data(); !user_data.empty())
      request.SetNamedValue(L"user_data", JsonValue::CreateStringValue(hstring(user_data)));
  }
  request.SetNamedValue(L"action", JsonValue::Parse(action.Stringify()));
  const auto body = utf8(request.Stringify());
  return take_response(
      msime_client_plugins(reinterpret_cast<const uint8_t *>(body.data()), body.size()));
}

JsonObject plugin_action(wchar_t const *operation) {
  JsonObject action;
  action.SetNamedValue(L"operation", JsonValue::CreateStringValue(operation));
  return action;
}

// 数据目录下 plugins 里已安装的插件，与 Tauri 的 plugin_catalog 是同一份 msime_client_plugins catalog。辅助码表插件都是装上的包，所以不传内置声音包的目录。读目录，只在 UI 线程之外调用。
Response plugin_catalog_call() { return plugins_call(plugin_action(L"catalog"), false); }

// 失败的插件请求在页面上的那句话：error 是 client-services 的失败码，detail 是它报告的规则（共享页的 pluginErrorMessage）。
std::wstring plugin_failure(Response const &response, std::wstring const &fallback) {
  std::wstring code, detail;
  try {
    const auto object = JsonObject::Parse(text(response.text));
    code = named_string(object, L"error");
    detail = named_string(object, L"detail");
  } catch (...) {
  }
  return msime::settings::plugins::error_message(code, detail, fallback);
}

std::optional<double> named_number(JsonObject const &object, wchar_t const *key) {
  const auto value = object.GetNamedValue(key, JsonValue::CreateNullValue());
  if (value.ValueType() == JsonValueType::Number)
    return value.GetNumber();
  return std::nullopt;
}

std::vector<std::wstring> named_strings(JsonObject const &object, wchar_t const *key) {
  std::vector<std::wstring> result;
  const auto value = object.GetNamedValue(key, JsonValue::CreateNullValue());
  if (value.ValueType() != JsonValueType::Array)
    return result;
  for (auto const &item : value.GetArray())
    if (item.ValueType() == JsonValueType::String)
      result.emplace_back(item.GetString().c_str());
  return result;
}

std::vector<JsonObject> named_objects(JsonObject const &object, wchar_t const *key) {
  std::vector<JsonObject> result;
  const auto value = object.GetNamedValue(key, JsonValue::CreateNullValue());
  if (value.ValueType() != JsonValueType::Array)
    return result;
  for (auto const &item : value.GetArray())
    if (item.ValueType() == JsonValueType::Object)
      result.push_back(item.GetObject());
  return result;
}

// catalog 和 import 回答里的一个包（client-services 的 PluginSummary，各类内容平铺在里面）。
msime::settings::plugins::Package plugin_package(JsonObject const &object) {
  msime::settings::plugins::Package pack;
  pack.id = named_string(object, L"id");
  pack.name = named_string(object, L"name");
  pack.version = named_string(object, L"version");
  pack.license = named_string(object, L"license");
  pack.author = named_string(object, L"author");
  pack.description = named_string(object, L"description");
  pack.builtin = object.GetNamedBoolean(L"builtin", false);
  pack.kind = named_string(object, L"kind");
  pack.mode = named_string(object, L"mode");
  pack.tracks = named_strings(object, L"tracks");
  for (auto const &item : named_objects(object, L"commands"))
    pack.commands.push_back({named_string(item, L"trigger"), named_string(item, L"title"),
                             named_string(item, L"template")});
  for (auto const &item : named_objects(object, L"phrases"))
    pack.phrases.push_back({named_string(item, L"key"), named_string(item, L"text")});
  pack.table = named_string(object, L"table");
  pack.entries = named_number(object, L"entries");
  for (auto const &item : named_objects(object, L"preview"))
    pack.preview.push_back({named_string(item, L"character"), named_string(item, L"code")});
  pack.file = named_string(object, L"file");
  pack.word_count = named_number(object, L"word_count");
  pack.first_words = named_strings(object, L"first_words");
  for (auto const &item : named_objects(object, L"groups"))
    pack.groups.push_back({named_string(item, L"tab"), named_string(item, L"title"),
                           named_string(item, L"keywords"), named_strings(item, L"items")});
  pack.style = named_string(object, L"style");
  pack.intensity = named_number(object, L"intensity");
  pack.colors = named_strings(object, L"colors");
  pack.duration_ms = named_number(object, L"duration_ms");
  pack.particles = named_number(object, L"particles");
  pack.hooks = named_strings(object, L"hooks");
  pack.permissions = named_strings(object, L"permissions");
  pack.content_hash = named_string(object, L"content_hash");
  return pack;
}

// catalog 的回答；读不懂时为空。
std::optional<msime::settings::plugins::Catalog> plugin_catalog(Response const &response) {
  if (!response.ok)
    return std::nullopt;
  try {
    const auto value = JsonObject::Parse(text(response.text)).GetNamedObject(L"value");
    msime::settings::plugins::Catalog catalog;
    for (auto const &item : named_objects(value, L"packages"))
      catalog.packages.push_back(plugin_package(item));
    for (auto const &item : named_objects(value, L"issues"))
      catalog.issues.push_back({named_string(item, L"kind"), named_string(item, L"folder"),
                                named_string(item, L"reason")});
    return catalog;
  } catch (...) {
    return std::nullopt;
  }
}

// script_grants、set_script_grant 和 reorder_scripts 的回答（client-services 的 ScriptGrantsView）；读不懂时为空。
std::optional<msime::settings::plugins::ScriptGrants> plugin_script_grants(Response const &response) {
  if (!response.ok)
    return std::nullopt;
  try {
    const auto value = JsonObject::Parse(text(response.text)).GetNamedObject(L"value");
    msime::settings::plugins::ScriptGrants grants;
    for (auto const &item : named_objects(value, L"plugins"))
      grants.plugins.push_back({named_string(item, L"id"), named_string(item, L"hash"),
                                item.GetNamedBoolean(L"enabled", false),
                                named_strings(item, L"granted")});
    for (auto const &item : named_objects(value, L"health"))
      grants.health.push_back(
          {named_string(item, L"id"), named_string(item, L"hash"), named_string(item, L"disabled")});
    for (auto const &item : named_objects(value, L"commands"))
      grants.commands.push_back({named_string(item, L"id"), named_string(item, L"trigger"),
                                 named_string(item, L"title"), named_string(item, L"status"),
                                 named_string(item, L"conflict"), named_string(item, L"conflict_id")});
    return grants;
  } catch (...) {
    return std::nullopt;
  }
}

// load_mentions 的回答；读不懂时为空。
std::optional<std::vector<msime::settings::plugins::Mention>> plugin_mentions(Response const &response) {
  if (!response.ok)
    return std::nullopt;
  try {
    std::vector<msime::settings::plugins::Mention> mentions;
    for (auto const &item : JsonObject::Parse(text(response.text)).GetNamedArray(L"value")) {
      if (item.ValueType() != JsonValueType::Object)
        return std::nullopt;
      const auto entry = item.GetObject();
      mentions.push_back({named_string(entry, L"text"), named_string(entry, L"key")});
    }
    return mentions;
  } catch (...) {
    return std::nullopt;
  }
}

// 公开预设没有文件或网络访问，首次绘制时读取一次；持有字段的副本后即可释放 C 接口的 JSON。
struct VoiceProviderCatalog {
  std::vector<msime::settings::voice::ProviderDefaults> asr;
  std::vector<msime::settings::voice::ProviderDefaults> polish;
};

VoiceProviderCatalog const &voice_provider_catalog() {
  static const VoiceProviderCatalog catalog = [] {
    const auto response = take_response(msime_client_voice_provider_catalog());
    if (!response.ok)
      throw std::runtime_error("voice provider catalog unavailable");
    const auto value = JsonObject::Parse(text(response.text)).GetNamedObject(L"value");
    auto presets = [&value](wchar_t const *key) {
      std::vector<msime::settings::voice::ProviderDefaults> entries;
      for (auto const &item : value.GetNamedArray(key)) {
        const auto preset = item.GetObject();
        entries.push_back({std::wstring(preset.GetNamedString(L"id").c_str()),
                           std::wstring(preset.GetNamedString(L"endpoint").c_str()),
                           std::wstring(preset.GetNamedString(L"model").c_str()),
                           named_strings(preset, L"models"),
                           std::wstring(preset.GetNamedString(L"documentation").c_str()),
                           std::wstring(preset.GetNamedString(L"title").c_str())});
      }
      return entries;
    };
    VoiceProviderCatalog result{presets(L"asr"), presets(L"polish")};
    result.asr.push_back({L"system", L"", L"", {}, L"", L"Windows 系统识别"});
    result.asr.push_back({L"local", L"", L"", {}, L"", L"本地模型（离线）"});
    return result;
  }();
  return catalog;
}

// 偏好库会不会接受本地识别的下载镜像（asr_model_mirror）或模型目录（asr_model_path）这个值，见 msime_client_voice_model_setting_check。语音页只把认可的值写进设置，免得一个没输完的地址让保存失败。
bool voice_model_setting_accepted(wchar_t const *field, std::wstring const &value) {
  JsonObject request;
  request.SetNamedValue(field, JsonValue::CreateStringValue(hstring(value)));
  const auto body = utf8(request.Stringify());
  const auto response = take_response(msime_client_voice_model_setting_check(
      reinterpret_cast<const uint8_t *>(body.data()), body.size()));
  if (!response.ok)
    return false;
  try {
    return JsonObject::Parse(text(response.text)).GetNamedObject(L"value").GetNamedBoolean(field, false);
  } catch (...) {
    return false;
  }
}

// 自定义码表在下拉框里的名字，与共享页的 customSchemaLabel 相同：中英文名都有且不同时是「中文名 (英文名)」，否则依次退到中文名、英文名、文件名和方案 id。
std::wstring custom_helpcode_label(JsonObject const &schema) {
  const auto name = named_string(schema, L"name");
  const auto name_en = named_string(schema, L"name_en");
  if (!name.empty() && !name_en.empty() && name != name_en)
    return name + L" (" + name_en + L")";
  for (auto const &label : {name, name_en, named_string(schema, L"file_stem")})
    if (!label.empty())
      return label;
  return named_string(schema, L"schema");
}

// msime_client_helpcode_schemas 的回答变成下拉框选项（值是 custom/<文件名>）；失败或读不懂时没有自定义码表。
std::vector<Option> custom_helpcode_options(Response const &response) {
  std::vector<Option> options;
  if (!response.ok)
    return options;
  try {
    for (auto const &item : JsonObject::Parse(text(response.text)).GetNamedArray(L"value")) {
      const auto schema = item.GetObject();
      const auto value = named_string(schema, L"schema");
      if (value.starts_with(L"custom/"))
        options.push_back({value, custom_helpcode_label(schema)});
    }
  } catch (...) {
    options.clear();
  }
  return options;
}

// catalog 里种类为 helpcode 的包，按 catalog 的顺序：值是插件 id，标签是插件名。失败或读不懂时没有插件可选，与共享页的 useHelpcodePacks 相同。
std::vector<Option> helpcode_pack_options(Response const &response) {
  std::vector<Option> packs;
  if (!response.ok)
    return packs;
  try {
    const auto value = JsonObject::Parse(text(response.text)).GetNamedObject(L"value");
    for (auto const &item : value.GetNamedArray(L"packages")) {
      const auto pack = item.GetObject();
      if (named_string(pack, L"kind") == L"helpcode")
        packs.push_back({named_string(pack, L"id"), named_string(pack, L"name")});
    }
  } catch (...) {
    packs.clear();
  }
  return packs;
}

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

// A theme's touch keyboard colours as the shared preview draws them (`keyboardThemeLook` in packages/ui/src/keyboard/screen-keyboard-preview.tsx): the catalog's `function_key` paints the action keys and its `text` labels both kinds.
struct KeyboardColors {
  Color background, key, foreground, accent, action, action_foreground;
};

struct ThemeEntry {
  std::wstring id;
  std::wstring title;
  // The catalog's `appearance`: a built-in theme of one fixed mode, none for the themes that follow the mode.
  std::optional<bool> dark;
  std::optional<ThemePreview> preview;
  std::optional<KeyboardColors> keyboard;
};

// How the 屏幕键盘 page paints its keyboard preview, read the way the shared `ScreenKeyboardPreview` (packages/ui/src/keyboard/screen-keyboard-preview.tsx) reads a theme or a custom keyboard design. Fields a design leaves out keep the theme's look: no border, no pattern, rounded flat keys.
struct KeyboardArt {
  KeyboardColors colors;
  double corner_radius = 8;
  double border_width = 0;
  // A design's own border colour; the accent otherwise.
  std::optional<Color> border_color;
  double border_opacity = 0.28;
  bool monospaced = false;
  int pattern = 0;
  double pattern_opacity = 0.15;
  std::wstring key_shape = L"rounded";
  std::wstring key_material = L"flat";
  double key_opacity = 1;
  std::optional<Color> gradient_end;
  bool gradient_horizontal = false;
  // Base64 image bytes drawn under a shade, and where the cropped picture is anchored (below 0.34 the start, above 0.66 the end).
  std::wstring photo;
  double photo_shade = 0.25;
  double photo_position = 0.5;
};

Color opaque(uint8_t red, uint8_t green, uint8_t blue) { return Color{255, red, green, blue}; }

// `system`, and any theme without a keyboard palette, previews the design's iOS system keyboard tokens, as the shared preview does: there is no Windows palette to ask for.
KeyboardColors system_keyboard(bool dark) {
  return dark ? KeyboardColors{opaque(0x2B, 0x2B, 0x2D), opaque(0x6B, 0x6B, 0x6E),
                               opaque(0xFF, 0xFF, 0xFF), opaque(0x5F, 0xBF, 0x84),
                               opaque(0x46, 0x46, 0x48), opaque(0xFF, 0xFF, 0xFF)}
              : KeyboardColors{opaque(0xD1, 0xD4, 0xDB), opaque(0xFF, 0xFF, 0xFF),
                               opaque(0x00, 0x00, 0x00), opaque(0x2C, 0x7A, 0x4B),
                               opaque(0xAB, 0xB0, 0xBB), opaque(0x00, 0x00, 0x00)};
}

// `native` is offered only on iOS, so this window's catalog never lists it; a custom theme synced from an iPhone can still name it as its base, and the shared preview then draws the iOS 26 system keyboard.
KeyboardColors native_keyboard(bool dark) {
  return dark ? KeyboardColors{opaque(0x22, 0x22, 0x23), opaque(0x46, 0x46, 0x46),
                               opaque(0xFF, 0xFF, 0xFF), opaque(0x40, 0x9C, 0xFF),
                               opaque(0x46, 0x46, 0x46), opaque(0xFF, 0xFF, 0xFF)}
              : KeyboardColors{opaque(0xE2, 0xE3, 0xE8), opaque(0xFF, 0xFF, 0xFF),
                               opaque(0x00, 0x00, 0x00), opaque(0x00, 0x40, 0xDD),
                               opaque(0xFF, 0xFF, 0xFF), opaque(0x00, 0x00, 0x00)};
}

uint32_t skin_rgb(double value) {
  return static_cast<uint32_t>(std::clamp(std::round(value), 0.0, 16777215.0));
}

// A keyboard design stores its colours as 0xRRGGBB numbers (`skinColor` in touch-keyboard-skin-design.ts).
Color skin_color(double value) {
  const auto rgb = skin_rgb(value);
  return opaque(static_cast<uint8_t>((rgb >> 16) & 0xFF), static_cast<uint8_t>((rgb >> 8) & 0xFF),
                static_cast<uint8_t>(rgb & 0xFF));
}

// Black or white, whichever reads on `background` (`readableSkinText`): black above a relative luminance of 0.179.
Color readable_skin_text(double background) {
  const auto rgb = skin_rgb(background);
  auto channel = [rgb](int shift) {
    const double value = static_cast<double>((rgb >> shift) & 255) / 255.0;
    return value <= 0.04045 ? value / 12.92 : std::pow((value + 0.055) / 1.055, 2.4);
  };
  const double luminance = 0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0);
  return luminance > 0.179 ? opaque(0, 0, 0) : opaque(255, 255, 255);
}

Color faded(Color color, double opacity) {
  color.A = static_cast<uint8_t>(std::lround(std::clamp(opacity, 0.0, 1.0) * 255));
  return color;
}

Windows::Foundation::Point point_at(double x, double y) {
  return {static_cast<float>(x), static_cast<float>(y)};
}

// One key's outline in each shape a keyboard design offers, the same path as `keyboardKeyPath` (packages/ui/src/keyboard/keyboard-shape.ts).
PathFigure key_outline(double x, double y, double width, double height, std::wstring_view shape,
                       double radius) {
  PathFigure figure;
  figure.IsClosed(true);
  figure.IsFilled(true);
  auto line = [&figure](double px, double py) {
    LineSegment segment;
    segment.Point(point_at(px, py));
    figure.Segments().Append(segment);
  };
  auto quad = [&figure](double cx, double cy, double px, double py) {
    QuadraticBezierSegment segment;
    segment.Point1(point_at(cx, cy));
    segment.Point2(point_at(px, py));
    figure.Segments().Append(segment);
  };
  auto cubic = [&figure](double ax, double ay, double bx, double by, double px, double py) {
    BezierSegment segment;
    segment.Point1(point_at(ax, ay));
    segment.Point2(point_at(bx, by));
    segment.Point3(point_at(px, py));
    figure.Segments().Append(segment);
  };
  auto arc = [&figure](double r, double px, double py) {
    ArcSegment segment;
    segment.Size({static_cast<float>(r), static_cast<float>(r)});
    segment.Point(point_at(px, py));
    segment.SweepDirection(SweepDirection::Counterclockwise);
    figure.Segments().Append(segment);
  };
  const double w = width;
  const double h = height;
  if (shape == L"pebble") {
    figure.StartPoint(point_at(x + w * 0.35, y));
    cubic(x + w * 0.83, y, x + w, y + h * 0.04, x + w, y + h * 0.3);
    cubic(x + w, y + h * 0.85, x + w * 0.9, y + h, x + w * 0.68, y + h);
    cubic(x + w * 0.18, y + h, x, y + h * 0.97, x, y + h * 0.7);
    cubic(x, y + h * 0.2, x + w * 0.06, y, x + w * 0.35, y);
  } else if (shape == L"ticket") {
    // A notch in each side, cut by an arc that turns the way SVG's sweep flag 0 does.
    const double r = std::min(w, h) * 0.12;
    figure.StartPoint(point_at(x, y));
    line(x + w, y);
    line(x + w, y + h / 2 - r);
    arc(r, x + w, y + h / 2 + r);
    line(x + w, y + h);
    line(x, y + h);
    line(x, y + h / 2 + r);
    arc(r, x, y + h / 2 - r);
  } else {
    const double r = shape == L"capsule" ? h / 2 : std::min({radius, h / 2, w / 2});
    figure.StartPoint(point_at(x + r, y));
    line(x + w - r, y);
    quad(x + w, y, x + w, y + r);
    line(x + w, y + h - r);
    quad(x + w, y + h, x + w - r, y + h);
    line(x + r, y + h);
    quad(x, y + h, x, y + h - r);
    line(x, y + r);
    quad(x, y, x + r, y);
  }
  return figure;
}

// A key of the desktop on-screen keyboard the shared preview draws on Windows (`desktopKeyboardRows` in packages/ui/src/keyboard/keyboard-layouts.ts), with its share of the row's width.
struct PreviewKey {
  std::wstring label;
  double weight;
};

std::vector<std::vector<PreviewKey>> const &desktop_keyboard_rows() {
  static const std::vector<std::vector<PreviewKey>> rows = [] {
    auto letters = [](std::wstring_view text, std::vector<PreviewKey> &row) {
      for (const wchar_t c : text)
        row.push_back({std::wstring(1, c), 1});
    };
    std::vector<std::vector<PreviewKey>> result(5);
    letters(L"`1234567890-=", result[0]);
    result[0].push_back({L"Backspace", 1.9});
    result[1].push_back({L"Tab", 1.5});
    letters(L"qwertyuiop[]", result[1]);
    result[1].push_back({L"\\", 1.4});
    result[2].push_back({L"Caps Lock", 1.85});
    letters(L"asdfghjkl;'", result[2]);
    result[2].push_back({L"Enter", 2});
    result[3].push_back({L"Shift", 2.35});
    letters(L"zxcvbnm,./", result[3]);
    result[3].push_back({L"Shift", 2.15});
    result[4] = {{L"Ctrl", 1.25}, {L"Win", 1.25}, {L"Alt", 1.25}, {L"Space", 6.7},
                 {L"Alt", 1.25},  {L"Win", 1.25}, {L"Del", 1.25}, {L"Ctrl", 1.25}};
    return result;
  }();
  return rows;
}

// The keys painted in the action colour (`actionKeyboardLabels`).
bool action_key(std::wstring_view label) {
  return label == L"Backspace" || label == L"Enter" || label == L"Shift" || label == L"Del";
}

// Decodes a design's photo into `bitmap`; a photo that cannot be decoded leaves the keyboard's own background showing.
fire_and_forget show_keyboard_photo(Imaging::BitmapImage bitmap, std::wstring base64) {
  try {
    const auto bytes = Windows::Security::Cryptography::CryptographicBuffer::DecodeFromBase64String(
        hstring(base64));
    Windows::Storage::Streams::InMemoryRandomAccessStream stream;
    co_await stream.WriteAsync(bytes);
    stream.Seek(0);
    co_await bitmap.SetSourceAsync(stream);
  } catch (hresult_error const &) {
  }
}

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

// ---- 外部皮肤 ----

namespace sk = msime::settings::skins;

// The palette a msime_client_resolve_theme answer draws, as the Server reads it: each slot the answer sets over the native palette of the answer's own appearance, else of `dark`. A refused request, or an answer this reader does not recognise, draws the native tokens.
msime::windows::CandidatePalette resolved_candidate_palette(Response const &response, bool dark) {
  if (!response.ok)
    return msime::windows::candidate_native_palette(dark);
  try {
    const auto value = JsonObject::Parse(text(response.text)).GetNamedObject(L"value");
    std::optional<bool> theme_dark;
    const auto appearance = value.GetNamedValue(L"appearance", JsonValue::CreateNullValue());
    if (appearance.ValueType() == JsonValueType::String) {
      if (appearance.GetString() == L"dark")
        theme_dark = true;
      else if (appearance.GetString() == L"light")
        theme_dark = false;
    }
    msime::windows::CandidatePaletteOverrides slots;
    const auto candidate = value.GetNamedValue(L"candidate", JsonValue::CreateNullValue());
    if (candidate.ValueType() == JsonValueType::Object) {
      const auto colors = candidate.GetObject();
      auto slot = [&colors](const wchar_t *key, std::optional<std::string> &target) {
        const auto item = colors.GetNamedValue(key, JsonValue::CreateNullValue());
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
      const auto bar = colors.GetNamedValue(L"show_selected_bar", JsonValue::CreateNullValue());
      if (bar.ValueType() == JsonValueType::Boolean)
        slots.show_selected_bar = bar.GetBoolean();
    }
    return msime::windows::candidate_palette(
        slots, msime::windows::candidate_native_palette(theme_dark.value_or(dark)));
  } catch (hresult_error const &) {
    return msime::windows::candidate_native_palette(dark);
  }
}

// 共享 catalog 的一款皮肤：工作线程读取两种明暗的配色及包内相对图片，设置卡片与社区预览共用。
struct SkinPackage {
  std::wstring id, name, version, base, author, description;
  std::vector<std::wstring> layouts, themes;
  // The card radius, 0-32; none keeps the theme's own.
  std::optional<double> corner_radius;
  double min_width = 0;
  // The decoration band, set only for a decorated package (both sizes above zero).
  double decoration_top = 0;
  double decoration_width = 0;
  std::wstring decoration_image;
  std::wstring decoration_align = L"right";
  std::wstring background_image;
  std::wstring background_fit = L"cover";
  double background_opacity = 1;
  msime::windows::CandidatePalette light, dark, panel_light, panel_dark;
  msime::windows::CandidateSkinCanvas canvas;
};

struct SkinIssue {
  std::wstring folder, reason;
};

// One package image as msime_client_skin_resource answered it; `bytes` is empty when it could not be read.
struct SkinImageBytes {
  std::wstring key;
  std::string content_type;
  std::vector<uint8_t> bytes;
};

// What one scan of the skin root found. `signature` is the catalog answer itself, so an automatic rescan that found the same thing leaves the cards alone, as the shared page compares JSON.stringify of two scans.
struct SkinScan {
  bool ok = false;
  std::string signature;
  std::wstring directory;
  std::vector<SkinPackage> packages;
  std::vector<SkinIssue> issues;
  std::vector<SkinImageBytes> images;
};

// canvas 样例的缓存键不可能与合法包 id 开头的资源键冲突。
std::wstring skin_canvas_preview_key(std::wstring const &id, bool dark, bool vertical) {
  return L"\ncanvas-preview\n" + id + (dark ? L"/dark/" : L"/light/") +
         (vertical ? L"vertical" : L"horizontal");
}

std::optional<std::vector<uint8_t>> render_canvas_preview(SkinPackage const &skin,
                                                          std::vector<SkinImageBytes> const &images,
                                                          bool dark, bool vertical);

// The key a package image is kept under: the package id and its relative path.
std::wstring skin_image_key(std::wstring const &id, std::wstring const &relative) {
  return id + L"\n" + relative;
}

// `dimension` in external-skins.tsx: a manifest size inside [0, maximum], else 0.
double skin_dimension(double value, double maximum) {
  return std::isfinite(value) && value >= 0 && value <= maximum ? value : 0;
}

std::wstring json_text(JsonObject const &object, wchar_t const *key) {
  const auto value = object.GetNamedValue(key, JsonValue::CreateNullValue());
  return value.ValueType() == JsonValueType::String ? std::wstring(value.GetString().c_str())
                                                    : std::wstring();
}

std::optional<double> json_number(JsonObject const &object, wchar_t const *key) {
  const auto value = object.GetNamedValue(key, JsonValue::CreateNullValue());
  if (value.ValueType() != JsonValueType::Number)
    return std::nullopt;
  return value.GetNumber();
}

std::vector<std::wstring> json_texts(JsonObject const &object, wchar_t const *key) {
  std::vector<std::wstring> values;
  const auto value = object.GetNamedValue(key, JsonValue::CreateNullValue());
  if (value.ValueType() != JsonValueType::Array)
    return values;
  for (auto const &item : value.GetArray())
    if (item.ValueType() == JsonValueType::String)
      values.emplace_back(item.GetString().c_str());
  return values;
}

// The content type and bytes of a msime_client_skin_resource answer, {"ok":true,"value":{"contentType":...,"bytes":[...]}}. The byte array is read by hand: an image of a few hundred kilobytes is that many JSON numbers, which Windows.Data.Json would turn into as many objects.
std::optional<std::pair<std::string, std::vector<uint8_t>>>
skin_resource_bytes(std::string const &answer) {
  const std::string type_key = "\"contentType\":\"";
  const std::string bytes_key = "\"bytes\":[";
  const auto type_at = answer.find(type_key);
  const auto bytes_at = answer.find(bytes_key);
  if (type_at == std::string::npos || bytes_at == std::string::npos)
    return std::nullopt;
  const auto type_start = type_at + type_key.size();
  const auto type_end = answer.find('"', type_start);
  if (type_end == std::string::npos)
    return std::nullopt;
  std::vector<uint8_t> bytes;
  size_t at = bytes_at + bytes_key.size();
  unsigned value = 0;
  bool digits = false;
  bool closed = false;
  for (; at < answer.size() && !closed; ++at) {
    const char c = answer[at];
    if (c >= '0' && c <= '9') {
      value = value * 10 + static_cast<unsigned>(c - '0');
      if (value > 255)
        return std::nullopt;
      digits = true;
    } else if (c == ',' || c == ']') {
      if (digits)
        bytes.push_back(static_cast<uint8_t>(value));
      else if (c == ',' || !bytes.empty())
        return std::nullopt;
      value = 0;
      digits = false;
      closed = c == ']';
    } else if (c != ' ') {
      return std::nullopt;
    }
  }
  if (!closed)
    return std::nullopt;
  return std::pair{answer.substr(type_start, type_end - type_start), std::move(bytes)};
}

// 只由这款包组成的自定义主题配色；排列取包支持的一种，面板确认可画后才使用 panel_drawn 配色。
msime::windows::CandidatePalette skin_card_palette(SkinPackage const &skin, std::string const &root,
                                                   bool dark, bool panel_drawn = false) {
  JsonObject custom;
  custom.SetNamedValue(L"base", JsonValue::CreateStringValue(hstring(skin.base)));
  custom.SetNamedValue(L"candidate_skin", JsonValue::CreateStringValue(hstring(skin.id)));
  custom.SetNamedValue(L"candidate_skin_dark", JsonValue::CreateStringValue(hstring(skin.id)));
  JsonObject request;
  request.SetNamedValue(L"global_theme", JsonValue::CreateStringValue(L"custom"));
  request.SetNamedValue(L"custom_theme", custom);
  request.SetNamedValue(L"dark", JsonValue::CreateBooleanValue(dark));
  request.SetNamedValue(L"canvas_panel_drawn", JsonValue::CreateBooleanValue(panel_drawn));
  const bool horizontal = !skin.layouts.empty() && skin.layouts.front() == L"horizontal";
  request.SetNamedValue(L"layout",
                        JsonValue::CreateStringValue(horizontal ? L"horizontal" : L"vertical"));
  request.SetNamedValue(L"skins_directory", JsonValue::CreateStringValue(text(root)));
  const auto body = utf8(request.Stringify());
  return resolved_candidate_palette(
      take_response(msime_client_resolve_theme(reinterpret_cast<const uint8_t *>(body.data()),
                                               body.size())),
      dark);
}

// One package image through msime_client_skin_resource, which checks again that it is an image inside the package. Empty when it cannot be read.
std::vector<uint8_t> skin_image(std::string const &root, std::wstring const &id,
                                std::wstring const &relative, std::string &content_type) {
  JsonObject request;
  request.SetNamedValue(L"directory", JsonValue::CreateStringValue(text(root)));
  request.SetNamedValue(L"id", JsonValue::CreateStringValue(hstring(id)));
  request.SetNamedValue(L"relative", JsonValue::CreateStringValue(hstring(relative)));
  request.SetNamedValue(L"kind", JsonValue::CreateStringValue(L"image"));
  const auto body = utf8(request.Stringify());
  const auto response = take_response(
      msime_client_skin_resource(reinterpret_cast<const uint8_t *>(body.data()), body.size()));
  if (!response.ok)
    return {};
  auto resource = skin_resource_bytes(response.text);
  if (!resource)
    return {};
  content_type = std::move(resource->first);
  return std::move(resource->second);
}

// catalog 已验证几何；这里仅把 JSON 投影成候选窗共用的纯数值模型，图片保持包内相对键。
msime::windows::CandidateSkinCanvas skin_canvas(JsonObject const &object) {
  using namespace msime::windows;
  CandidateSkinCanvas canvas;
  const auto active = object.GetNamedValue(L"canvas", JsonValue::CreateNullValue());
  if (active.ValueType() != JsonValueType::Boolean || !active.GetBoolean())
    return canvas;
  canvas.active = true;
  auto box = [](JsonObject const &value,
                wchar_t const *key) -> std::optional<CandidateCanvasEdges> {
    const auto field = value.GetNamedValue(key, JsonValue::CreateNullValue());
    if (field.ValueType() != JsonValueType::Array)
      return std::nullopt;
    std::vector<double> numbers;
    for (const auto &item : field.GetArray()) {
      if (item.ValueType() != JsonValueType::Number || !std::isfinite(item.GetNumber()) ||
          item.GetNumber() < 0 || item.GetNumber() > 2000)
        return std::nullopt;
      numbers.push_back(item.GetNumber());
    }
    if (numbers.size() != 1 && numbers.size() != 2 && numbers.size() != 4)
      return std::nullopt;
    return candidate_canvas_box(numbers);
  };
  const auto border = object.GetNamedValue(L"borderImage", JsonValue::CreateNullValue());
  if (border.ValueType() == JsonValueType::Object) {
    const auto fields = border.GetObject();
    const auto slice = box(fields, L"sliceDip");
    if (slice) {
      CandidateCanvasBorder image;
      image.source = json_text(fields, L"source");
      image.light_source = json_text(fields, L"lightSource");
      image.slice = box(fields, L"stretchDip").value_or(*slice);
      image.width = box(fields, L"widthDip").value_or(image.slice);
      const auto fill = fields.GetNamedValue(L"fill", JsonValue::CreateNullValue());
      image.fill = fill.ValueType() != JsonValueType::Boolean || fill.GetBoolean();
      canvas.border = std::move(image);
    }
  }
  auto insets = [&](wchar_t const *key) -> std::optional<CandidateCanvasEdges> {
    const auto value = object.GetNamedValue(key, JsonValue::CreateNullValue());
    if (value.ValueType() != JsonValueType::Object)
      return std::nullopt;
    const auto fields = value.GetObject();
    const auto top = json_number(fields, L"top"), right = json_number(fields, L"right"),
               bottom = json_number(fields, L"bottom"), left = json_number(fields, L"left");
    if (!top || !right || !bottom || !left)
      return std::nullopt;
    return CandidateCanvasEdges{*top, *right, *bottom, *left};
  };
  canvas.content_insets = insets(L"contentInsetsDip");
  canvas.outer_insets = insets(L"outerInsetsDip");
  canvas.line_gap = json_number(object, L"lineGapDip");
  const auto decorations = object.GetNamedValue(L"decorations", JsonValue::CreateNullValue());
  if (decorations.ValueType() == JsonValueType::Array)
    for (const auto &value : decorations.GetArray()) {
      if (canvas.decorations.size() >= 32)
        break;
      if (value.ValueType() != JsonValueType::Object)
        continue;
      const auto fields = value.GetObject();
      CandidateCanvasDecoration decoration;
      decoration.image = json_text(fields, L"image");
      const auto name = json_text(fields, L"anchor");
      const auto anchor = candidate_canvas_anchor(name.empty() ? "right" : utf8(hstring(name)));
      if (!anchor)
        continue;
      decoration.anchor = *anchor;
      decoration.offset_x = json_number(fields, L"offsetXDip").value_or(0);
      decoration.offset_y = json_number(fields, L"offsetYDip").value_or(0);
      decoration.width = json_number(fields, L"widthDip").value_or(0);
      decoration.height = json_number(fields, L"heightDip").value_or(0);
      decoration.z_index = static_cast<int>(json_number(fields, L"zIndex").value_or(2));
      canvas.decorations.push_back(std::move(decoration));
    }
  return canvas;
}

// 预览没有用户取色器；仅补 catalog 中共享主题响应未携带、而静态样例实际绘制的细分槽位。
msime::windows::CandidatePalette skin_preview_palette(
    JsonObject const &object, msime::windows::CandidatePalette palette, bool dark,
    bool panel_drawn) {
  auto candidate = object.GetNamedValue(panel_drawn ? L"panelCandidate" : L"candidate",
                                        JsonValue::CreateNullValue());
  if (candidate.ValueType() != JsonValueType::Object)
    candidate = object.GetNamedValue(L"candidate", JsonValue::CreateNullValue());
  if (candidate.ValueType() != JsonValueType::Object)
    return palette;
  const auto mode = candidate.GetObject().GetNamedValue(dark ? L"dark" : L"light",
                                                       JsonValue::CreateNullValue());
  if (mode.ValueType() != JsonValueType::Object)
    return palette;
  const auto colors = mode.GetObject();
  auto optional = [&](wchar_t const *key,
                       std::optional<msime::windows::CandidateColor> &target) {
    if (const auto parsed = msime::windows::try_parse_css_color(utf8(hstring(json_text(colors, key)))))
      target = *parsed;
  };
  optional(L"candidate_text", palette.candidate_text);
  optional(L"preedit_text", palette.preedit_text);
  optional(L"preedit_background", palette.preedit_background);
  optional(L"preedit_divider", palette.preedit_divider);
  return palette;
}

// Scans the skin root the Server reads (`skins` under the state directory, the root the Tauri shell scans as well), then resolves each package's palettes and reads its images. Reads the disk: call on a worker. A root that cannot be named fails the scan; an unreadable root is an empty catalog, as msime_client_skin_catalog answers it.
SkinScan read_skin_catalog(std::filesystem::path const &root) {
  SkinScan scan;
  const auto path = root.is_absolute() ? path_utf8(root) : std::string();
  if (path.empty())
    return scan;
  auto response = take_response(
      msime_client_skin_catalog(reinterpret_cast<const uint8_t *>(path.data()), path.size()));
  if (!response.ok)
    return scan;
  try {
    const auto value = JsonObject::Parse(text(response.text)).GetNamedObject(L"value");
    for (auto const &item : value.GetNamedArray(L"packages")) {
      if (item.ValueType() != JsonValueType::Object)
        continue;
      const auto object = item.GetObject();
      SkinPackage skin;
      skin.id = json_text(object, L"id");
      skin.name = json_text(object, L"name");
      if (skin.id.empty())
        continue;
      if (skin.name.empty())
        skin.name = skin.id;
      skin.version = json_text(object, L"version");
      skin.base = json_text(object, L"base");
      if (skin.base.empty())
        skin.base = L"system";
      skin.author = json_text(object, L"author");
      skin.description = json_text(object, L"description");
      skin.layouts = json_texts(object, L"layouts");
      skin.themes = json_texts(object, L"themes");
      if (const auto radius = json_number(object, L"cornerRadiusDip"))
        skin.corner_radius = skin_dimension(*radius, 32);
      skin.min_width = skin_dimension(json_number(object, L"minWidthDip").value_or(0), 1000);
      const double top = skin_dimension(json_number(object, L"decorationTopDip").value_or(0), 500);
      const double width =
          skin_dimension(json_number(object, L"decorationWidthDip").value_or(0), 1000);
      if (top > 0 && width > 0) {
        skin.decoration_top = top;
        skin.decoration_width = width;
        skin.decoration_image = json_text(object, L"decorationImage");
      }
      if (const auto align = json_text(object, L"decorationAlign");
          align == L"left" || align == L"center")
        skin.decoration_align = align;
      if (const auto background =
              object.GetNamedValue(L"background", JsonValue::CreateNullValue());
          background.ValueType() == JsonValueType::Object) {
        const auto fields = background.GetObject();
        skin.background_image = json_text(fields, L"image");
        if (const auto fit = json_text(fields, L"fit"); fit == L"contain" || fit == L"stretch")
          skin.background_fit = fit;
        const double opacity = json_number(fields, L"opacity").value_or(0);
        skin.background_opacity = std::isfinite(opacity) ? std::clamp(opacity, 0.0, 1.0) : 0.0;
      }
      skin.canvas = skin_canvas(object);
      skin.light = skin_preview_palette(object, skin_card_palette(skin, path, false), false, false);
      skin.dark = skin_preview_palette(object, skin_card_palette(skin, path, true), true, false);
      skin.panel_light =
          skin.canvas.active ? skin_preview_palette(object, skin_card_palette(skin, path, false, true),
                                                     false, true) : skin.light;
      skin.panel_dark =
          skin.canvas.active ? skin_preview_palette(object, skin_card_palette(skin, path, true, true),
                                                     true, true) : skin.dark;
      std::set<std::wstring> resources{skin.decoration_image, skin.background_image};
      if (skin.canvas.border) {
        resources.insert(skin.canvas.border->source);
        resources.insert(skin.canvas.border->light_source);
      }
      for (const auto &decoration : skin.canvas.decorations)
        resources.insert(decoration.image);
      for (const auto &relative : resources) {
        if (relative.empty())
          continue;
        SkinImageBytes image;
        image.key = skin_image_key(skin.id, relative);
        image.bytes = skin_image(path, skin.id, relative, image.content_type);
        scan.images.push_back(std::move(image));
      }
      if (skin.canvas.active)
        for (const bool dark : {false, true})
          for (const bool vertical : {false, true}) {
            if (std::find(skin.themes.begin(), skin.themes.end(), dark ? L"dark" : L"light") ==
                    skin.themes.end() ||
                std::find(skin.layouts.begin(), skin.layouts.end(),
                          vertical ? L"vertical" : L"horizontal") == skin.layouts.end())
              continue;
            if (auto bytes = render_canvas_preview(skin, scan.images, dark, vertical))
              scan.images.push_back({skin_canvas_preview_key(skin.id, dark, vertical),
                                     (*bytes)[0] == 0x89 ? "image/png" : "image/jpeg",
                                     std::move(*bytes)});
          }
      scan.packages.push_back(std::move(skin));
    }
    for (auto const &item : value.GetNamedArray(L"issues")) {
      if (item.ValueType() != JsonValueType::Object)
        continue;
      const auto object = item.GetObject();
      scan.issues.push_back({json_text(object, L"folder"), json_text(object, L"reason")});
    }
  } catch (hresult_error const &) {
    return SkinScan{};
  }
  scan.ok = true;
  scan.signature = std::move(response.text);
  scan.directory = root.wstring();
  return scan;
}

// 共享页解不出 UTF-8 时按 GB18030 读（代码页 54936），与 decodeDictionaryBytes 的最后一步相同；读不出时为空。调用方已把文件限制在 32 MB 以内。
std::optional<std::string> gb18030_to_utf8(std::string_view bytes) {
  if (bytes.empty())
    return std::string();
  const int length = static_cast<int>(bytes.size());
  const int units = MultiByteToWideChar(54936, 0, bytes.data(), length, nullptr, 0);
  if (units <= 0)
    return std::nullopt;
  std::wstring wide(static_cast<size_t>(units), L'\0');
  if (MultiByteToWideChar(54936, 0, bytes.data(), length, wide.data(), units) != units)
    return std::nullopt;
  return to_string(std::wstring_view(wide));
}

namespace li = msime::settings::lexicon_import;

// ---- 社区 ----

namespace cm = msime::settings::community;

// 一次 msime_client_community 请求：账号目录与「账号与同步」页相同（共享应用登录的那一份会话），皮肤根目录是状态目录下的 skins，插件的状态根目录是状态目录本身，与 msime_client_plugins 相同。读不到本机的应用数据目录时没有会话可用。会阻塞网络，只在 UI 线程之外调用。
Response community_call(JsonObject const &action) {
  const auto directory = account_directory();
  if (directory.empty())
    return {"{\"ok\":false,\"error\":\"community_storage\"}", false};
  JsonObject request;
  request.SetNamedValue(L"directory", JsonValue::CreateStringValue(hstring(directory.wstring())));
  if (const auto legacy = legacy_account_directory(); !legacy.empty())
    request.SetNamedValue(L"legacy_directory",
                          JsonValue::CreateStringValue(hstring(legacy.wstring())));
  if (const auto state = state_directory(); state.is_absolute()) {
    request.SetNamedValue(L"skins_directory",
                          JsonValue::CreateStringValue(hstring((state / L"skins").wstring())));
    request.SetNamedValue(L"state_root", JsonValue::CreateStringValue(hstring(state.wstring())));
  }
  request.SetNamedValue(L"action", JsonValue::Parse(action.Stringify()));
  const auto body = utf8(request.Stringify());
  return take_response(
      msime_client_community(reinterpret_cast<const uint8_t *>(body.data()), body.size()));
}

JsonObject community_action(wchar_t const *operation) {
  JsonObject action;
  action.SetNamedValue(L"operation", JsonValue::CreateStringValue(operation));
  return action;
}

// 一次失败的社区请求：error 是社区或打包的错误码，detail 是插件打包时违反的那条规则（可能没有）。
struct CommunityFailure {
  std::wstring code, detail;
};

CommunityFailure community_failure(Response const &response) {
  CommunityFailure failure;
  try {
    const auto object = JsonObject::Parse(text(response.text));
    failure.code = named_string(object, L"error");
    failure.detail = named_string(object, L"detail");
  } catch (...) {
  }
  return failure;
}

// The `value` of an answer, or null when it cannot be read.
JsonValue community_value(Response const &response) {
  try {
    return JsonObject::Parse(text(response.text))
        .GetNamedValue(L"value", JsonValue::CreateNullValue());
  } catch (...) {
    return JsonValue::CreateNullValue();
  }
}

// 发布的 id：随机的 UUID（共享页的 randomUuid），小写、带连字符。
std::wstring random_uuid() {
  GUID guid{};
  if (FAILED(CoCreateGuid(&guid)))
    return {};
  wchar_t buffer[40] = {};
  swprintf_s(buffer, L"%08lx-%04hx-%04hx-%02x%02x-%02x%02x%02x%02x%02x%02x", guid.Data1,
             guid.Data2, guid.Data3, guid.Data4[0], guid.Data4[1], guid.Data4[2], guid.Data4[3],
             guid.Data4[4], guid.Data4[5], guid.Data4[6], guid.Data4[7]);
  return buffer;
}

// 社区里的一款候选窗口皮肤（client-services 的 CandidateSkinItem，共享页的 CommunityCandidateSkin）。
struct CommunitySkinItem {
  std::wstring id, package_id, name, description, author, version;
  std::wstring license_code, license_assets, license_source;
  double size = 0, file_count = 0, downloads = 0, rating_count = 0, rating_average = 0;
  bool owned = false;
  int64_t my_rating = 0;
  // public 或 private；早于私有皮肤的服务端不返回，都是公开的。
  std::wstring visibility = L"public";
  // 只有自己的作品带审核状态，removed 显示「已下架」。
  std::wstring moderation;
  // 发布分类；早于分类功能的服务端不返回，为空。
  std::wstring category;
};

// 社区里的一个插件（client-services 的 CommunityPlugin）。
struct CommunityPluginItem {
  std::wstring id, kind, plugin_id, name, description, author, version, license;
  double size = 0, downloads = 0, rating_count = 0, rating_average = 0;
  bool owned = false;
  int64_t my_rating = 0;
  std::wstring moderation;
};

void parse_community_item(JsonObject const &object, CommunitySkinItem &skin) {
  skin.id = named_string(object, L"id");
  skin.package_id = named_string(object, L"package_id");
  skin.name = named_string(object, L"name");
  skin.description = named_string(object, L"description");
  skin.author = named_string(object, L"author");
  skin.version = named_string(object, L"version");
  const auto license = object.GetNamedValue(L"license", JsonValue::CreateNullValue());
  if (license.ValueType() == JsonValueType::Object) {
    const auto fields = license.GetObject();
    skin.license_code = named_string(fields, L"code");
    skin.license_assets = named_string(fields, L"assets");
    skin.license_source = named_string(fields, L"source");
  }
  skin.size = named_number(object, L"size").value_or(0);
  skin.file_count = named_number(object, L"file_count").value_or(0);
  skin.downloads = named_number(object, L"downloads").value_or(0);
  skin.rating_count = named_number(object, L"rating_count").value_or(0);
  skin.rating_average = named_number(object, L"rating_average").value_or(0);
  const auto owned = object.GetNamedValue(L"owned", JsonValue::CreateNullValue());
  skin.owned = owned.ValueType() == JsonValueType::Boolean && owned.GetBoolean();
  skin.my_rating = named_count(object, L"my_rating");
  if (named_string(object, L"visibility") == L"private")
    skin.visibility = L"private";
  skin.moderation = named_string(object, L"moderation");
  skin.category = named_string(object, L"category");
}

void parse_community_item(JsonObject const &object, CommunityPluginItem &plugin) {
  plugin.id = named_string(object, L"id");
  plugin.kind = named_string(object, L"kind");
  plugin.plugin_id = named_string(object, L"plugin_id");
  plugin.name = named_string(object, L"name");
  plugin.description = named_string(object, L"description");
  plugin.author = named_string(object, L"author");
  plugin.version = named_string(object, L"version");
  plugin.license = named_string(object, L"license");
  plugin.size = named_number(object, L"size").value_or(0);
  plugin.downloads = named_number(object, L"downloads").value_or(0);
  plugin.rating_count = named_number(object, L"rating_count").value_or(0);
  plugin.rating_average = named_number(object, L"rating_average").value_or(0);
  const auto owned = object.GetNamedValue(L"owned", JsonValue::CreateNullValue());
  plugin.owned = owned.ValueType() == JsonValueType::Boolean && owned.GetBoolean();
  plugin.my_rating = named_count(object, L"my_rating");
  plugin.moderation = named_string(object, L"moderation");
}

// 社区里的一个词库或回复模板（client-services 的 CommunityResource，共享页的 CommunityResource）。词库带词条，回复模板带提示词。
struct CommunityResourceItem {
  std::wstring id, kind, name, description, author, prompt;
  std::vector<cm::SharedWord> entries;
  int64_t revision = 0;
  double saves = 0, rating_count = 0, rating_average = 0;
  bool saved = false, owned = false;
  int64_t my_rating = 0;
  std::wstring moderation;
};

void parse_community_item(JsonObject const &object, CommunityResourceItem &resource) {
  resource.id = named_string(object, L"id");
  resource.kind = named_string(object, L"kind");
  resource.name = named_string(object, L"name");
  resource.description = named_string(object, L"description");
  resource.author = named_string(object, L"author");
  const auto content = object.GetNamedValue(L"content", JsonValue::CreateNullValue());
  if (content.ValueType() == JsonValueType::Object) {
    const auto fields = content.GetObject();
    resource.prompt = named_string(fields, L"prompt");
    for (auto const &entry : named_objects(fields, L"entries"))
      resource.entries.push_back({named_string(entry, L"kind"), named_string(entry, L"code"),
                                  named_string(entry, L"word"), named_count(entry, L"weight")});
  }
  resource.revision = named_count(object, L"revision");
  resource.saves = named_number(object, L"saves").value_or(0);
  resource.rating_count = named_number(object, L"rating_count").value_or(0);
  resource.rating_average = named_number(object, L"rating_average").value_or(0);
  const auto flag = [&object](wchar_t const *key) {
    const auto value = object.GetNamedValue(key, JsonValue::CreateNullValue());
    return value.ValueType() == JsonValueType::Boolean && value.GetBoolean();
  };
  resource.saved = flag(L"saved");
  resource.owned = flag(L"owned");
  resource.my_rating = named_count(object, L"my_rating");
  resource.moderation = named_string(object, L"moderation");
}

// 一个作品的回答（detail、publish、set_visibility、set_category），读不出来时为空。
template <typename Item> std::optional<Item> community_item(Response const &response) {
  const auto value = community_value(response);
  if (!response.ok || value.ValueType() != JsonValueType::Object)
    return std::nullopt;
  Item item;
  parse_community_item(value.GetObject(), item);
  if (item.id.empty())
    return std::nullopt;
  return item;
}

// 图库读到的一页：items 之外还有它从哪个 offset 读起，和宿主略过的本机装不了的条目数（只有插件有），下一页从它们后面读起。
template <typename Item> struct CommunityListPage {
  bool ok = false;
  CommunityFailure failure;
  std::vector<Item> items;
  bool has_more = false;
  size_t offset = 0;
  size_t skipped = 0;
};

// useCommunityGallery 读一页时最多再往后读几页被宿主整页略过的条目，余下的留给「加载更多」。
constexpr int community_max_skipped_pages = 5;

// 读图库的一页。整页都被略过却还有下一页时，在工作线程上接着读，最多五页，免得列表看起来是空的而后面还有能装的。`filter` 是候选窗皮肤的分类或插件的类型，空着就是全部。
template <typename Item>
CommunityListPage<Item> read_community_page(size_t offset, std::wstring const &search, bool mine,
                                            std::optional<std::wstring> const &filter) {
  constexpr bool skins = std::is_same_v<Item, CommunitySkinItem>;
  CommunityListPage<Item> page;
  page.offset = offset;
  for (int extra = 0;; ++extra) {
    auto action = community_action(skins ? L"candidate_skin_list" : L"plugin_list");
    action.SetNamedValue(L"offset", JsonValue::CreateNumberValue(static_cast<double>(page.offset)));
    action.SetNamedValue(L"search", JsonValue::CreateStringValue(hstring(search)));
    action.SetNamedValue(L"mine", JsonValue::CreateBooleanValue(mine));
    if (filter)
      action.SetNamedValue(skins ? L"category" : L"kind",
                           JsonValue::CreateStringValue(hstring(*filter)));
    const auto response = community_call(action);
    const auto value = community_value(response);
    if (!response.ok || value.ValueType() != JsonValueType::Object) {
      page.ok = false;
      page.failure = community_failure(response);
      if (page.failure.code.empty())
        page.failure.code = L"community_unavailable";
      return page;
    }
    const auto object = value.GetObject();
    page.items.clear();
    for (auto const &entry : named_objects(object, skins ? L"skins" : L"plugins")) {
      Item item;
      parse_community_item(entry, item);
      if (!item.id.empty())
        page.items.push_back(std::move(item));
    }
    const auto more = object.GetNamedValue(L"has_more", JsonValue::CreateNullValue());
    page.has_more = more.ValueType() == JsonValueType::Boolean && more.GetBoolean();
    page.skipped = static_cast<size_t>(std::max<int64_t>(named_count(object, L"skipped"), 0));
    page.ok = true;
    if (extra >= community_max_skipped_pages || !page.items.empty() || !page.has_more ||
        page.skipped == 0)
      return page;
    page.offset += page.skipped;
  }
}

// 读词库或回复模板图库的一页（CommunityResourcesPage 的 load）：kind 是 dictionary 或 reply，scope 是空（全部）、saved 或 mine。宿主不略过条目。
CommunityListPage<CommunityResourceItem> read_resource_page(size_t offset, std::wstring const &search,
                                                            std::wstring const &kind,
                                                            std::wstring const &scope) {
  CommunityListPage<CommunityResourceItem> page;
  page.offset = offset;
  auto action = community_action(L"resource_list");
  action.SetNamedValue(L"kind", JsonValue::CreateStringValue(hstring(kind)));
  action.SetNamedValue(L"scope", JsonValue::CreateStringValue(hstring(scope)));
  action.SetNamedValue(L"search", JsonValue::CreateStringValue(hstring(search)));
  action.SetNamedValue(L"offset", JsonValue::CreateNumberValue(static_cast<double>(offset)));
  const auto response = community_call(action);
  const auto value = community_value(response);
  if (!response.ok || value.ValueType() != JsonValueType::Object) {
    page.failure = community_failure(response);
    if (page.failure.code.empty())
      page.failure.code = L"community_unavailable";
    return page;
  }
  const auto object = value.GetObject();
  for (auto const &entry : named_objects(object, L"items")) {
    CommunityResourceItem item;
    parse_community_item(entry, item);
    if (!item.id.empty())
      page.items.push_back(std::move(item));
  }
  const auto more = object.GetNamedValue(L"has_more", JsonValue::CreateNullValue());
  page.has_more = more.ValueType() == JsonValueType::Boolean && more.GetBoolean();
  page.ok = true;
  return page;
}

// 「发布皮肤」和装皮肤前要知道的本机皮肤：只要 id，与 msime_client_skin_catalog 的扫描相同，但不读图片、不解析配色。读不到时为空。读目录，只在 UI 线程之外调用。
std::optional<std::vector<std::wstring>> read_skin_ids(std::filesystem::path const &root) {
  const auto path = root.is_absolute() ? path_utf8(root) : std::string();
  if (path.empty())
    return std::nullopt;
  const auto response = take_response(
      msime_client_skin_catalog(reinterpret_cast<const uint8_t *>(path.data()), path.size()));
  if (!response.ok)
    return std::nullopt;
  try {
    std::vector<std::wstring> ids;
    const auto value = JsonObject::Parse(text(response.text)).GetNamedObject(L"value");
    for (auto const &item : value.GetNamedArray(L"packages"))
      if (item.ValueType() == JsonValueType::Object)
        if (auto id = json_text(item.GetObject(), L"id"); !id.empty())
          ids.push_back(std::move(id));
    return ids;
  } catch (hresult_error const &) {
    return std::nullopt;
  }
}

// 一张皮肤图片解码成 Direct2D 位图；读不到、不是位图（SVG）或解码失败时为空，预览里就不画这一部分。
com_ptr<ID2D1Bitmap> decode_preview_image(IWICImagingFactory *factory, ID2D1RenderTarget *target,
                                          std::vector<SkinImageBytes> const &images,
                                          std::wstring const &key, D2D1_SIZE_F &size) {
  for (auto const &image : images) {
    if (image.key != key || image.bytes.empty() || image.content_type == "image/svg+xml")
      continue;
    com_ptr<IWICStream> stream;
    com_ptr<IWICBitmapDecoder> decoder;
    com_ptr<IWICBitmapFrameDecode> frame;
    com_ptr<IWICFormatConverter> converter;
    com_ptr<ID2D1Bitmap> bitmap;
    if (FAILED(factory->CreateStream(stream.put())) ||
        FAILED(stream->InitializeFromMemory(const_cast<BYTE *>(image.bytes.data()),
                                            static_cast<DWORD>(image.bytes.size()))) ||
        FAILED(factory->CreateDecoderFromStream(stream.get(), nullptr,
                                                WICDecodeMetadataCacheOnDemand, decoder.put())) ||
        FAILED(decoder->GetFrame(0, frame.put())) ||
        FAILED(factory->CreateFormatConverter(converter.put())) ||
        FAILED(converter->Initialize(frame.get(), GUID_WICPixelFormat32bppPBGRA,
                                     WICBitmapDitherTypeNone, nullptr, 0,
                                     WICBitmapPaletteTypeCustom)) ||
        FAILED(target->CreateBitmapFromWicBitmap(converter.get(), nullptr, bitmap.put())))
      return nullptr;
    size = bitmap->GetSize();
    return size.width > 0 && size.height > 0 ? bitmap : nullptr;
  }
  return nullptr;
}

// 把画好的预览编码成 PNG 或 JPEG（quality 是 JPEG 的质量）。
std::optional<std::vector<uint8_t>> encode_preview(IWICImagingFactory *factory, IWICBitmap *bitmap,
                                                   GUID const &format, float quality) {
  com_ptr<IStream> stream;
  if (FAILED(CreateStreamOnHGlobal(nullptr, TRUE, stream.put())))
    return std::nullopt;
  com_ptr<IWICBitmapEncoder> encoder;
  com_ptr<IWICBitmapFrameEncode> frame;
  com_ptr<IPropertyBag2> properties;
  if (FAILED(factory->CreateEncoder(format, nullptr, encoder.put())) ||
      FAILED(encoder->Initialize(stream.get(), WICBitmapEncoderNoCache)) ||
      FAILED(encoder->CreateNewFrame(frame.put(), properties.put())))
    return std::nullopt;
  if (format == GUID_ContainerFormatJpeg && properties) {
    PROPBAG2 option{};
    option.pstrName = const_cast<LPOLESTR>(L"ImageQuality");
    VARIANT value;
    VariantInit(&value);
    value.vt = VT_R4;
    value.fltVal = quality;
    properties->Write(1, &option, &value);
  }
  UINT width = 0, height = 0;
  bitmap->GetSize(&width, &height);
  WICPixelFormatGUID pixel = format == GUID_ContainerFormatJpeg ? GUID_WICPixelFormat24bppBGR
                                                                : GUID_WICPixelFormat32bppBGRA;
  if (FAILED(frame->Initialize(properties.get())) || FAILED(frame->SetSize(width, height)) ||
      FAILED(frame->SetPixelFormat(&pixel)) || FAILED(frame->WriteSource(bitmap, nullptr)) ||
      FAILED(frame->Commit()) || FAILED(encoder->Commit()))
    return std::nullopt;
  STATSTG stat{};
  HGLOBAL global = nullptr;
  if (FAILED(stream->Stat(&stat, STATFLAG_NONAME)) ||
      FAILED(GetHGlobalFromStream(stream.get(), &global)))
    return std::nullopt;
  const auto *data = static_cast<uint8_t const *>(GlobalLock(global));
  if (!data)
    return std::nullopt;
  std::vector<uint8_t> bytes(data, data + static_cast<size_t>(stat.cbSize.QuadPart));
  GlobalUnlock(global);
  return bytes;
}

// 社区的预览图上限（client-services 里的 `MAX_PREVIEW_BYTES`）。
constexpr size_t skin_preview_limit = 256 * 1024;

// canvas 卡片和导出使用同一静态绘制，WIC 在工作线程完整解码；不借用用户当前的候选窗口。
std::optional<std::vector<uint8_t>> render_canvas_preview(SkinPackage const &skin,
                                                          std::vector<SkinImageBytes> const &images,
                                                          bool dark, bool vertical) {
  const HRESULT apartment = CoInitializeEx(nullptr, COINIT_MULTITHREADED);
  struct Uninitialize {
    bool active;
    ~Uninitialize() {
      if (active)
        CoUninitialize();
    }
  } uninitialize{SUCCEEDED(apartment)};
  com_ptr<IWICImagingFactory> factory;
  if (FAILED(CoCreateInstance(CLSID_WICImagingFactory, nullptr, CLSCTX_INPROC_SERVER,
                              IID_PPV_ARGS(factory.put()))))
    return std::nullopt;
  const auto bitmap = msime::windows::candidate_canvas_preview(
      factory.get(), skin.canvas, dark ? skin.dark : skin.light,
      dark ? skin.panel_dark : skin.panel_light, dark, vertical, skin.min_width,
      [&](const std::wstring &relative) -> ::Microsoft::WRL::ComPtr<IWICBitmap> {
        const auto key = skin_image_key(skin.id, relative);
        for (const auto &image : images) {
          if (image.key != key || image.bytes.empty() || image.bytes.size() > 8 * 1024 * 1024)
            continue;
          // 与真实候选窗共用合成器，GIF 首帧必须包含逻辑画布和帧偏移。
          const auto animation = msimeui::AnimatedImage::DecodeMemory(
              factory.get(), image.bytes.data(), image.bytes.size());
          if (!animation || animation->FrameCount() == 0)
            return nullptr;
          const auto &pixels = animation->FramePixels(0);
          ::Microsoft::WRL::ComPtr<IWICBitmap> decoded;
          if (FAILED(factory->CreateBitmapFromMemory(
                  animation->Width(), animation->Height(), GUID_WICPixelFormat32bppPBGRA,
                  animation->Width() * 4, static_cast<UINT>(pixels.size()),
                  const_cast<BYTE *>(pixels.data()), decoded.GetAddressOf())))
            return nullptr;
          return decoded;
        }
        return nullptr;
      });
  if (!bitmap)
    return std::nullopt;
  auto png = encode_preview(factory.get(), bitmap.Get(), GUID_ContainerFormatPng, 1);
  if (png && png->size() <= skin_preview_limit)
    return png;
  auto jpeg = encode_preview(factory.get(), bitmap.Get(), GUID_ContainerFormatJpeg, 0.85f);
  return jpeg && jpeg->size() <= skin_preview_limit ? jpeg : std::nullopt;
}

// 给没有预览图的已装皮肤生成静态样例：canvas 与设置卡片共用绘制，传统皮肤保留装饰带与背景。优先浅色，使用包支持的排列；每 DIP 两像素，PNG 超过 256 KiB 时尝试 JPEG，仍超限则失败。仅在工作线程调用。
std::optional<std::vector<uint8_t>> render_skin_preview(SkinPackage const &skin,
                                                        std::vector<SkinImageBytes> const &images) {
  const HRESULT apartment = CoInitializeEx(nullptr, COINIT_MULTITHREADED);
  struct Uninitialize {
    bool active;
    ~Uninitialize() {
      if (active)
        CoUninitialize();
    }
  } uninitialize{SUCCEEDED(apartment)};
  auto has = [&skin](wchar_t const *theme) {
    return std::find(skin.themes.begin(), skin.themes.end(), theme) != skin.themes.end();
  };
  const bool light = has(L"light") || !has(L"dark");
  if (skin.canvas.active)
    return render_canvas_preview(skin, images, !light,
                                 !skin.layouts.empty() && skin.layouts.front() == L"vertical");
  const auto &palette = light ? skin.light : skin.dark;
  constexpr float scale = 2, padding = 16, card_height = 68;
  const bool decorated =
      skin.decoration_top > 0 && skin.decoration_width > 0 && !skin.decoration_image.empty();
  const float card_width = static_cast<float>(std::max(skin.min_width, 340.0));
  const float radius = static_cast<float>(skin.corner_radius.value_or(8));

  com_ptr<IWICImagingFactory> wic;
  com_ptr<ID2D1Factory> d2d;
  com_ptr<IDWriteFactory> dwrite;
  if (FAILED(CoCreateInstance(CLSID_WICImagingFactory, nullptr, CLSCTX_INPROC_SERVER,
                              IID_PPV_ARGS(wic.put()))) ||
      FAILED(D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, d2d.put())) ||
      FAILED(DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED, __uuidof(IDWriteFactory),
                                 reinterpret_cast<::IUnknown **>(dwrite.put()))))
    return std::nullopt;

  // 装饰图读不出来时整条装饰带都不画，卡片从上边距开始，与共享页一样；所以先用一个临时的目标解码图片，量出高度再建画布。
  auto draw = [&](float band, bool measure_only, com_ptr<ID2D1Bitmap> *decoration_out)
      -> std::optional<std::vector<uint8_t>> {
    const auto width = static_cast<UINT>(std::lround((card_width + padding * 2) * scale));
    const auto height = static_cast<UINT>(std::lround((band + card_height + padding * 2) * scale));
    com_ptr<IWICBitmap> bitmap;
    com_ptr<ID2D1RenderTarget> target;
    const auto properties = D2D1::RenderTargetProperties(
        D2D1_RENDER_TARGET_TYPE_DEFAULT,
        D2D1::PixelFormat(DXGI_FORMAT_B8G8R8A8_UNORM, D2D1_ALPHA_MODE_PREMULTIPLIED), 96, 96);
    if (FAILED(wic->CreateBitmap(width, height, GUID_WICPixelFormat32bppPBGRA,
                                 WICBitmapCacheOnLoad, bitmap.put())) ||
        FAILED(d2d->CreateWicBitmapRenderTarget(bitmap.get(), properties, target.put())))
      return std::nullopt;
    D2D1_SIZE_F decoration_size{}, background_size{};
    auto decoration =
        decorated ? decode_preview_image(wic.get(), target.get(), images,
                                         skin_image_key(skin.id, skin.decoration_image),
                                         decoration_size)
                  : nullptr;
    if (measure_only) {
      *decoration_out = decoration;
      return std::vector<uint8_t>{};
    }
    auto background = skin.background_image.empty()
                          ? nullptr
                          : decode_preview_image(wic.get(), target.get(), images,
                                                 skin_image_key(skin.id, skin.background_image),
                                                 background_size);
    auto color = [](msime::windows::CandidateColor const &value) {
      return D2D1::ColorF(value.r, value.g, value.b, value.a);
    };
    auto solid = [&target](D2D1_COLOR_F const &value) {
      com_ptr<ID2D1SolidColorBrush> paint;
      target->CreateSolidColorBrush(value, paint.put());
      return paint;
    };
    auto format = [&dwrite](float size) {
      com_ptr<IDWriteTextFormat> text_format;
      dwrite->CreateTextFormat(L"Segoe UI", nullptr, DWRITE_FONT_WEIGHT_NORMAL,
                               DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL, size,
                               L"zh-CN", text_format.put());
      if (text_format) {
        text_format->SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP);
        text_format->SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
      }
      return text_format;
    };
    auto layout = [&dwrite](std::wstring_view value, IDWriteTextFormat *text_format) {
      com_ptr<IDWriteTextLayout> text_layout;
      if (text_format)
        dwrite->CreateTextLayout(value.data(), static_cast<UINT32>(value.size()), text_format,
                                 4096, 40, text_layout.put());
      return text_layout;
    };
    auto advance = [](IDWriteTextLayout *text_layout) {
      DWRITE_TEXT_METRICS metrics{};
      if (text_layout)
        text_layout->GetMetrics(&metrics);
      return metrics.widthIncludingTrailingWhitespace;
    };
    // textBaseline = "middle"：文字的中线落在 y 上。
    auto text_at = [&target](IDWriteTextLayout *text_layout, float x, float y,
                             ID2D1SolidColorBrush *paint) {
      if (text_layout && paint)
        target->DrawTextLayout(D2D1::Point2F(x, y - 20), text_layout, paint);
    };

    target->BeginDraw();
    target->SetTransform(D2D1::Matrix3x2F::Scale(scale, scale));
    target->Clear(light ? D2D1::ColorF(0xE9EDF3) : D2D1::ColorF(0x16181D));
    if (decoration) {
      const float decoration_width = static_cast<float>(skin.decoration_width);
      const float x = skin.decoration_align == L"left" ? padding + radius
                      : skin.decoration_align == L"center"
                          ? padding + (card_width - decoration_width) / 2
                          : padding + card_width - radius - decoration_width;
      target->DrawBitmap(decoration.get(),
                         D2D1::RectF(x, padding, x + decoration_width, padding + band));
    }
    const float top = padding + band;
    const auto card = D2D1::RoundedRect(
        D2D1::RectF(padding, top, padding + card_width, top + card_height), radius, radius);
    if (auto paint = solid(color(palette.surface)))
      target->FillRoundedRectangle(card, paint.get());
    if (background) {
      com_ptr<ID2D1RoundedRectangleGeometry> clip;
      com_ptr<ID2D1Layer> layer;
      if (SUCCEEDED(d2d->CreateRoundedRectangleGeometry(card, clip.put())) &&
          SUCCEEDED(target->CreateLayer(nullptr, layer.put()))) {
        target->PushLayer(D2D1::LayerParameters(D2D1::InfiniteRect(), clip.get()), layer.get());
        // fitted()：cover 和 contain 按比例放大或缩小后居中，stretch 铺满。
        float x = 0, y = 0, w = card_width, h = card_height;
        if (skin.background_fit != L"stretch") {
          const float ratio =
              skin.background_fit == L"contain"
                  ? std::min(card_width / background_size.width, card_height / background_size.height)
                  : std::max(card_width / background_size.width,
                             card_height / background_size.height);
          w = background_size.width * ratio;
          h = background_size.height * ratio;
          x = (card_width - w) / 2;
          y = (card_height - h) / 2;
        }
        target->DrawBitmap(background.get(),
                           D2D1::RectF(padding + x, top + y, padding + x + w, top + y + h),
                           static_cast<float>(skin.background_opacity));
        target->PopLayer();
      }
    }
    if (palette.border.a > 0)
      if (auto paint = solid(color(palette.border)))
        target->DrawRoundedRectangle(
            D2D1::RoundedRect(D2D1::RectF(padding + 0.5f, top + 0.5f, padding + card_width - 0.5f,
                                          top + card_height - 0.5f),
                              radius, radius),
            paint.get(), 1);

    const auto label_format = format(12), body_format = format(13), candidate_format = format(16);
    {
      auto pinyin = layout(L"ni'mf", body_format.get());
      auto paint = solid(color(palette.translation.value_or(palette.number)));
      text_at(pinyin.get(), padding + 12, top + 18, paint.get());
    }
    // 一排横向的候选，第一个选中，就像打字时看到的那样。
    static constexpr std::array<std::wstring_view, 5> samples{L"你们", L"你", L"尼", L"妮", L"泥"};
    float x = padding + 8;
    const float row_top = top + 32;
    constexpr float row_height = 28;
    for (size_t index = 0; index < samples.size(); ++index) {
      const auto label_text = std::to_wstring(index + 1);
      auto label = layout(label_text, label_format.get());
      auto candidate = layout(samples[index], candidate_format.get());
      const float label_width = advance(label.get());
      const float cell = label_width + 4 + advance(candidate.get()) + 16;
      if (x + cell > padding + card_width - 8)
        break;
      const bool selected = index == 0;
      if (selected)
        if (auto paint = solid(color(palette.selected)))
          target->FillRoundedRectangle(
              D2D1::RoundedRect(D2D1::RectF(x, row_top, x + cell, row_top + row_height),
                                std::min(6.0f, radius), std::min(6.0f, radius)),
              paint.get());
      // 选中行的颜色 alpha 为 0 表示沿用未选中时的颜色。
      const auto number_color = selected && palette.selected_number.a > 0 ? palette.selected_number
                                                                     : palette.number;
      const auto text_color = selected && palette.selected_text.a > 0 ? palette.selected_text
                                                                : palette.text;
      auto number_paint = solid(color(number_color));
      auto text_paint = solid(color(text_color));
      text_at(label.get(), x + 8, row_top + row_height / 2, number_paint.get());
      text_at(candidate.get(), x + 8 + label_width + 4, row_top + row_height / 2,
              text_paint.get());
      x += cell + 4;
    }
    if (FAILED(target->EndDraw()))
      return std::nullopt;
    target = nullptr;
    auto png = encode_preview(wic.get(), bitmap.get(), GUID_ContainerFormatPng, 1);
    if (png && png->size() <= skin_preview_limit)
      return png;
    auto jpeg = encode_preview(wic.get(), bitmap.get(), GUID_ContainerFormatJpeg, 0.85f);
    if (jpeg && jpeg->size() <= skin_preview_limit)
      return jpeg;
    return std::nullopt;
  };

  com_ptr<ID2D1Bitmap> decoration;
  if (decorated && !draw(0, true, &decoration))
    return std::nullopt;
  return draw(decoration ? static_cast<float>(skin.decoration_top) : 0, false, nullptr);
}

// ---- 打字统计 ----

namespace typing = msime::settings::typing;

// 打字统计的一次状态：统计、文件是否写过和最后写入的时间。signature 是整个回答，轮询时据此判断有没有新数据，与共享页比较 JSON.stringify(next) 一样。
struct StatisticsStatus {
  typing::Statistics statistics;
  bool never_written = true;
  std::optional<uint64_t> last_written_ms;
  std::string signature;
};

// 今天，按本机时区。
typing::CivilDay local_civil_day() {
  SYSTEMTIME now{};
  GetLocalTime(&now);
  return {now.wYear, now.wMonth, now.wDay};
}

// 统计文件的最后写入时间，写成共享页 `new Date(ms).toLocaleString("zh-CN")` 的样子；换算不了时为空。
std::wstring statistics_written_text(uint64_t milliseconds) {
  constexpr uint64_t unix_epoch = 116444736000000000ULL;
  if (milliseconds > 900000000000000ULL)
    return {};
  ULARGE_INTEGER ticks{};
  ticks.QuadPart = milliseconds * 10000ULL + unix_epoch;
  const FILETIME utc{ticks.LowPart, ticks.HighPart};
  SYSTEMTIME universal{}, local{};
  if (!FileTimeToSystemTime(&utc, &universal) ||
      !SystemTimeToTzSpecificLocalTime(nullptr, &universal, &local))
    return {};
  return typing::zh_date_time(local.wYear, local.wMonth, local.wDay, local.wHour, local.wMinute,
                              local.wSecond);
}

// 一张 `{键: 数}` 表；不是数的项跳过。
typing::Counts statistics_counts(IJsonValue const &value) {
  typing::Counts counts;
  if (!value || value.ValueType() != JsonValueType::Object)
    return counts;
  for (auto const &entry : value.GetObject())
    if (entry.Value().ValueType() == JsonValueType::Number)
      counts[utf8(entry.Key())] = entry.Value().GetNumber();
  return counts;
}

typing::Breakdown statistics_breakdown(IJsonValue const &value) {
  typing::Breakdown breakdown;
  if (!value || value.ValueType() != JsonValueType::Object)
    return breakdown;
  const auto object = value.GetObject();
  breakdown.characters =
      statistics_counts(object.GetNamedValue(L"characters", JsonValue::CreateNullValue()));
  breakdown.sources =
      statistics_counts(object.GetNamedValue(L"sources", JsonValue::CreateNullValue()));
  return breakdown;
}

// msime_client_typing_statistics 带 status:true 的回答，读不了时为空。在工作线程上读：保留期长时文件有几兆。
std::optional<StatisticsStatus> statistics_status(Response const &response) {
  if (!response.ok)
    return std::nullopt;
  try {
    const auto value = JsonObject::Parse(text(response.text)).GetNamedObject(L"value");
    const auto object = value.GetNamedObject(L"statistics");
    auto named = [&object](wchar_t const *key) {
      return object.GetNamedValue(key, JsonValue::CreateNullValue());
    };
    StatisticsStatus status;
    status.signature = response.text;
    status.never_written = named_string(value, L"availability") != L"ready";
    const auto written = value.GetNamedValue(L"lastWrittenMs", JsonValue::CreateNullValue());
    if (written.ValueType() == JsonValueType::Number && written.GetNumber() > 0 &&
        written.GetNumber() < 9007199254740992.0)
      status.last_written_ms = static_cast<uint64_t>(written.GetNumber());
    auto &statistics = status.statistics;
    statistics.enabled = object.GetNamedBoolean(L"enabled", false);
    statistics.retention = utf8(hstring(named_string(object, L"retention")));
    if (const auto total = named(L"total"); total.ValueType() == JsonValueType::Number)
      statistics.total = total.GetNumber();
    statistics.days = statistics_counts(named(L"days"));
    statistics.detail = statistics_breakdown(named(L"detail"));
    if (const auto daily = named(L"dailyDetails"); daily.ValueType() == JsonValueType::Object)
      for (auto const &entry : daily.GetObject())
        statistics.daily_details[utf8(entry.Key())] = statistics_breakdown(entry.Value());
    if (const auto selections = named(L"selections");
        selections.ValueType() == JsonValueType::Object) {
      const auto counts = selections.GetObject();
      const auto ranks = counts.GetNamedValue(L"ranks", JsonValue::CreateNullValue());
      if (ranks.ValueType() == JsonValueType::Array)
        for (auto const &rank : ranks.GetArray())
          statistics.ranks.push_back(rank.ValueType() == JsonValueType::Number ? rank.GetNumber()
                                                                               : 0);
      if (const auto beyond = counts.GetNamedValue(L"beyond", JsonValue::CreateNullValue());
          beyond.ValueType() == JsonValueType::Number)
        statistics.beyond = beyond.GetNumber();
    }
    statistics.daily_active_ms = statistics_counts(named(L"dailyActiveMs"));
    if (const auto hours = named(L"dailyHours"); hours.ValueType() == JsonValueType::Object)
      for (auto const &entry : hours.GetObject()) {
        if (entry.Value().ValueType() != JsonValueType::Array)
          continue;
        auto &buckets = statistics.daily_hours[utf8(entry.Key())];
        for (auto const &bucket : entry.Value().GetArray())
          buckets.push_back(bucket.ValueType() == JsonValueType::Number ? bucket.GetNumber() : 0);
      }
    if (const auto keys = named(L"dailyKeys"); keys.ValueType() == JsonValueType::Object)
      for (auto const &entry : keys.GetObject())
        statistics.daily_keys[utf8(entry.Key())] = statistics_counts(entry.Value());
    return status;
  } catch (...) {
    return std::nullopt;
  }
}

Color chart_color(uint32_t rgb, double alpha = 1) {
  return rgba(static_cast<int>((rgb >> 16) & 0xFF), static_cast<int>((rgb >> 8) & 0xFF),
              static_cast<int>(rgb & 0xFF), alpha);
}

// 图表上的一条线；虚线是参照线（平均值），与共享页的 ReferenceLine 一样只标阈值，不是网格。
void chart_line(Canvas const &canvas, double x1, double y1, double x2, double y2,
                Color const &color, double thickness, bool dashed) {
  Shapes::Line line;
  line.X1(x1);
  line.Y1(y1);
  line.X2(x2);
  line.Y2(y2);
  line.Stroke(brush(color));
  line.StrokeThickness(thickness);
  if (dashed) {
    DoubleCollection dash;
    dash.Append(4);
    dash.Append(4);
    line.StrokeDashArray(dash);
  }
  canvas.Children().Append(line);
}

void chart_polyline(Canvas const &canvas, std::vector<Windows::Foundation::Point> const &points,
                    Color const &color, double thickness) {
  Shapes::Polyline line;
  PointCollection collection;
  for (auto const &point : points)
    collection.Append(point);
  line.Points(collection);
  line.Stroke(brush(color));
  line.StrokeThickness(thickness);
  line.StrokeLineJoin(PenLineJoin::Round);
  line.StrokeStartLineCap(PenLineCap::Round);
  line.StrokeEndLineCap(PenLineCap::Round);
  canvas.Children().Append(line);
}

// 带一圈底色描边的圆点，圆心在 (x, y)。
void chart_dot(Canvas const &canvas, double x, double y, double diameter, Color const &fill,
               Color const &ring) {
  Shapes::Ellipse dot;
  const double outer = diameter + 4;
  dot.Width(outer);
  dot.Height(outer);
  dot.Fill(brush(fill));
  dot.Stroke(brush(ring));
  dot.StrokeThickness(2);
  Canvas::SetLeft(dot, x - outer / 2);
  Canvas::SetTop(dot, y - outer / 2);
  canvas.Children().Append(dot);
}

// 铺满所在格子的画布，尺寸一变就按新尺寸重画：图表的横向随窗口伸缩，线和点要按实际像素画，圆点才不会被拉成椭圆。
Canvas chart_overlay(std::function<void(Canvas const &, double, double)> draw) {
  Canvas canvas;
  canvas.IsHitTestVisible(false);
  canvas.HorizontalAlignment(HorizontalAlignment::Stretch);
  canvas.VerticalAlignment(VerticalAlignment::Stretch);
  canvas.SizeChanged([draw = std::move(draw)](Inspectable const &sender,
                                              SizeChangedEventArgs const &args) {
    const auto self = sender.as<Canvas>();
    self.Children().Clear();
    draw(self, args.NewSize().Width, args.NewSize().Height);
  });
  return canvas;
}

struct MainWindow : WindowT<MainWindow> {
  explicit MainWindow(std::string page)
      : current_page_(nav::offered_page_for_route(page, MSIME_EDITION_HANDWRITING != 0)) {
    ai_mcp_view_ = page == "mcp";
    // 窗口标题和侧栏的产品名按版本取，full 仍是「水杉输入法」。
    Title(MSIME_EDITION_DISPLAY_NAME L"设置");
    ExtendsContentIntoTitleBar(true);
    reload_document();
    load_catalog();
    system_dark_ = system_dark();
    input_method_ = input_method_state();
    palette_ = make_palette(resolve_dark());
    // 初始尺寸按系统缩放换算，正文保留 macOS 的紧凑密度。
    const double scale = static_cast<double>(GetDpiForSystem()) / 96.0;
    AppWindow().Resize(
        {static_cast<int32_t>(std::lround(1000 * scale)),
         static_cast<int32_t>(std::lround(760 * scale))});
    // 参照 macOS 的最小窗口尺寸，避免侧栏把设置正文挤到无法操作。
    if (const auto presenter = AppWindow().Presenter().try_as<Microsoft::UI::Windowing::OverlappedPresenter>()) {
      presenter.PreferredMinimumWidth(
          box_value(static_cast<int32_t>(std::lround(800 * scale)))
              .as<Windows::Foundation::IReference<int32_t>>());
      presenter.PreferredMinimumHeight(
          box_value(static_cast<int32_t>(std::lround(560 * scale)))
              .as<Windows::Foundation::IReference<int32_t>>());
    }
    build_shell(0.0);
    // 设置窗口打开时补传一次打字统计（没打开上传、没登录或离上次不到十分钟时不发请求）。
    sync_typing_upload();
    dictionary_update_request(L"automatic");
    dictionary_update_timer_ = DispatcherQueue().CreateTimer();
    dictionary_update_timer_.Interval(std::chrono::seconds(2));
    dictionary_update_timer_.Tick([this](auto const &, auto const &) {
      if (!closed_ && current_page_ == "lexicon" && !dictionary_update_busy_) dictionary_update_request(L"status");
    });
    dictionary_update_timer_.Start();
    Activated({this, &MainWindow::on_activated});
    Closed([this](Inspectable const &, WindowEventArgs const &) {
      // A save refused while closing has no window left to report in; the stored settings stay as they were.
      closed_ = true;
      if (dictionary_update_timer_) dictionary_update_timer_.Stop();
      flush_pending();
      // 本地模型的下载随窗口一起停下，下次安装时清掉没装完的临时文件。
      take_response(msime_client_voice_local_model_cancel(nullptr, 0));
      // 等待中的 Google 登录也随窗口结束，回环端口不再等一个没人看的浏览器。
      cancel_account_google();
    });
  }

private:
  // ---- Document and palette ----

  void reload_document() {
    PreferencesDocument fresh;
    std::wstring error;
    if (fresh.Load(error)) {
      document_ = std::move(fresh);
      synchronize_cloud_clipboard_configuration();
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
        if (const auto appearance = json_text(theme, L"appearance");
            appearance == L"dark" || appearance == L"light")
          entry.dark = appearance == L"dark";
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
        const auto keyboard =
            theme.GetNamedValue(L"keyboard", JsonValue::CreateNullValue());
        if (keyboard.ValueType() == JsonValueType::Object) {
          const auto colors = keyboard.GetObject();
          auto color = [&colors](const wchar_t *key) {
            return parse_hex(colors.GetNamedString(key, L"").c_str());
          };
          const auto background = color(L"background");
          const auto key = color(L"key");
          const auto action = color(L"function_key");
          const auto foreground = color(L"text");
          const auto accent = color(L"accent");
          if (background && key && action && foreground && accent)
            entry.keyboard = KeyboardColors{*background, *key, *foreground,
                                            *accent, *action, *foreground};
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

  // 设置窗口跟随颜色模式（HostCapabilities::surface_themes_follow），不读存下的 settings_theme；跟随系统时看 Windows 的应用模式。
  bool resolve_dark() {
    const auto theme = document_.String(L"theme", L"system");
    if (theme == L"dark")
      return true;
    if (theme == L"light")
      return false;
    return system_dark_;
  }

  // The candidate window's own light/dark mode, decided as the Server decides it (candidate_theme_dark in src/candidate/CandidateThemeSettings.h): the 候选窗口主题 override, else 颜色模式.
  bool candidate_dark() const {
    const auto surface = document_.String(L"candidate_theme", L"follow");
    if (surface == L"dark" || surface == L"light")
      return surface == L"dark";
    const auto theme = document_.String(L"theme", L"system");
    return theme == L"system" ? system_dark_ : theme != L"light";
  }

  // The candidate palette the Server draws for a global theme. The request is the one the Server sends (candidate_theme_request): the stored custom theme without its keyboard design, the candidate window's mode and layout, and the skin root, so a custom theme's candidate_colors and skin package reach the preview exactly as they reach the real window. A request the shared layer refuses, or a response this reader does not recognise, draws the native tokens, as the Server does. The last answer is kept for its request, since the preview redraws on every slider step.
  // Without `skins` the custom theme is resolved without its skin packages and without the skin root, which is how the shared page draws the 自定义 card: what the custom theme is made of besides a package (`customCandidateStyle(base, candidate_colors)`). That answer is kept apart, so the card and the preview do not evict each other.
  msime::windows::CandidatePalette candidate_theme(
      std::wstring const &id, bool skins = true, std::optional<bool> appearance = std::nullopt) {
    const bool dark = appearance.value_or(candidate_dark());
    JsonObject request;
    request.SetNamedValue(L"global_theme",
                          JsonValue::CreateStringValue(hstring(id)));
    if (const auto custom = document_.Value(L"custom_theme");
        custom && custom.ValueType() == JsonValueType::Object) {
      auto copy = JsonObject::Parse(custom.Stringify());
      std::vector<const wchar_t *> dropped{L"keyboard"};
      if (!skins) {
        dropped.push_back(L"candidate_skin");
        dropped.push_back(L"candidate_skin_dark");
      }
      for (const auto *key : dropped)
        if (copy.HasKey(key))
          copy.Remove(key);
      // 编号色和边框色跟随主题，Server 不读这两个槽位（candidate_theme_values），预览也不画，与真窗口一致。
      if (copy.HasKey(L"candidate_colors") &&
          copy.GetNamedValue(L"candidate_colors").ValueType() == JsonValueType::Object) {
        auto colors = copy.GetNamedObject(L"candidate_colors");
        for (const auto *slot : {L"number", L"border"})
          if (colors.HasKey(slot))
            colors.Remove(slot);
        copy.SetNamedValue(L"candidate_colors", colors);
      }
      request.SetNamedValue(L"custom_theme", copy);
    }
    request.SetNamedValue(L"dark", JsonValue::CreateBooleanValue(dark));
    request.SetNamedValue(
        L"layout",
        JsonValue::CreateStringValue(
            document_.String(L"candidate_layout", L"vertical") == L"horizontal"
                ? L"horizontal"
                : L"vertical"));
    if (const auto state = state_directory(); skins && state.is_absolute())
      if (const auto root = path_utf8(state / L"skins"); !root.empty())
        request.SetNamedValue(L"skins_directory",
                              JsonValue::CreateStringValue(text(root)));
    auto body = utf8(request.Stringify());
    auto &cached_request = skins ? resolved_request_ : unskinned_request_;
    auto &cached = skins ? resolved_theme_ : unskinned_theme_;
    if (cached && body == cached_request)
      return *cached;

    const auto palette = resolved_candidate_palette(
        take_response(msime_client_resolve_theme(
            reinterpret_cast<const uint8_t *>(body.data()), body.size())),
        dark);
    cached_request = std::move(body);
    cached = palette;
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

  // 切换明暗时保留滚动位置和侧栏搜索内容；恢复文字不会主动打开建议列表。
  void rebuild_shell() {
    const double offset = scroll_ ? scroll_.VerticalOffset() : 0.0;
    const hstring query = search_ ? search_.Text() : hstring();
    build_shell(offset);
    search_.Text(query);
  }

  void on_activated(Inspectable const &, WindowActivatedEventArgs const &args) {
    if (args.WindowActivationState() == WindowActivationState::Deactivated) {
      flush_pending();
      return;
    }
    // 与共享页一样，窗口获得焦点时重读打字统计并按日节流检查词库。
    poll_statistics();
    dictionary_update_request(L"automatic");
    // Another window (the tray, the shared app, another device through sync) may have changed the settings or Windows may have switched between light and dark while this window was in the background.
    bool changed = false;
    PreferencesDocument fresh;
    std::wstring error;
    if (fresh.Load(error) &&
        (!loaded_ || fresh.revision() != document_.revision())) {
      document_ = std::move(fresh);
      synchronize_cloud_clipboard_configuration();
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
    // 与共享页一样，「主题」页显示时窗口重新获得焦点就重扫皮肤目录（用户可能刚把皮肤文件夹拷进去）；扫到的与上次相同时不重画。
    if (root_ && current_page_ == "themes")
      scan_skins(false);
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
    if (document_.Save(error)) {
      synchronize_cloud_clipboard_configuration();
      return true;
    }
    notice_ = L"无法保存这项设置：设置可能已在其他窗口中更新，或与其他设置冲突。"
              L"已重新读取当前设置。";
    notice_severity_ = InfoBarSeverity::Error;
    // The refused edit is still in memory; the stored settings replace it, and if they cannot be read the controls are disabled rather than showing a value that was never saved.
    PreferencesDocument fresh;
    std::wstring reload_error;
    const bool reloaded = fresh.Load(reload_error);
    document_ = std::move(fresh);
    synchronize_cloud_clipboard_configuration();
    loaded_ = reloaded;
    load_error_ = reloaded ? std::wstring() : reload_error;
    queue_refresh();
    return false;
  }

  void change(std::function<void(PreferencesDocument &)> const &edit,
              bool redraw) {
    flush_pending();
    edit(document_);
    if (commit()) {
      update_preview();
      update_toolbar_preview();
      if (redraw)
        queue_refresh();
    }
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

  // 直接覆盖控件模板查询的资源；全局强调色不会重算 WinUI 已解析的静态别名。
  void apply_accent_resources(ResourceDictionary const &resources) {
    if (Windows::UI::ViewManagement::AccessibilitySettings().HighContrast())
      return;
    for (const auto *key : {L"SystemAccentColor", L"SystemAccentColorLight1",
                            L"SystemAccentColorLight2", L"SystemAccentColorLight3",
                            L"SystemAccentColorDark1", L"SystemAccentColorDark2",
                            L"SystemAccentColorDark3"})
      resources.Insert(box_value(hstring(key)), box_value(palette_.accent));
    for (const auto *key : {L"SystemControlHighlightAccentBrush", L"AccentFillColorDefaultBrush",
                            L"AccentTextFillColorPrimaryBrush", L"ToggleSwitchFillOn",
                            L"ToggleSwitchFillOnPointerOver", L"ToggleSwitchFillOnPressed"})
      resources.Insert(box_value(hstring(key)), brush(palette_.accent));
    for (const auto *key : {L"ToggleSwitchKnobFillOn", L"ToggleSwitchKnobFillOnPointerOver",
                            L"ToggleSwitchKnobFillOnPressed"})
      resources.Insert(box_value(hstring(key)), brush(palette_.on_accent));

    for (const auto &[suffix, opacity] :
         std::array<std::pair<const wchar_t *, double>, 3>{
             {{L"", 1.0}, {L"PointerOver", 0.9}, {L"Pressed", 0.8}}}) {
      for (const auto *key : {
               L"CheckBoxCheckBackgroundFillChecked", L"CheckBoxCheckBackgroundStrokeChecked",
               L"CheckBoxCheckBackgroundFillIndeterminate", L"CheckBoxCheckBackgroundStrokeIndeterminate",
               L"RadioButtonOuterEllipseCheckedFill", L"RadioButtonOuterEllipseCheckedStroke",
               L"SliderTrackValueFill", L"SliderThumbBackground", L"HyperlinkButtonForeground"})
        resources.Insert(box_value(hstring(std::wstring(key) + suffix)),
                         brush(faded(palette_.accent, opacity)));
      for (const auto *key : {L"CheckBoxCheckGlyphForegroundChecked",
                              L"CheckBoxCheckGlyphForegroundIndeterminate",
                              L"RadioButtonCheckGlyphFill"})
        resources.Insert(box_value(hstring(std::wstring(key) + suffix)),
                         brush(palette_.on_accent));
    }
  }

  // ---- Window chrome ----

  void build_shell(double offset) {
    Grid root;
    root.RequestedTheme(palette_.dark ? ElementTheme::Dark : ElementTheme::Light);
    root.Background(brush(palette_.window_bg));
    apply_accent_resources(root.Resources());

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
    ColumnDefinition sidebar;
    sidebar.Width(GridLength{224, GridUnitType::Pixel});
    ColumnDefinition content;
    content.Width(GridLength{1, GridUnitType::Star});
    ColumnDefinition caption;
    caption.Width(GridLength{140, GridUnitType::Pixel});
    bar.ColumnDefinitions().Append(sidebar);
    bar.ColumnDefinitions().Append(content);
    bar.ColumnDefinitions().Append(caption);

    StackPanel identity;
    identity.Orientation(Orientation::Horizontal);
    identity.Spacing(10);
    identity.Margin(Thickness{18, 0, 12, 0});
    identity.VerticalAlignment(VerticalAlignment::Center);
    Image logo;
    logo.Width(24);
    logo.Height(24);
    show_logo(logo, 48);
    identity.Children().Append(logo);
    auto product = make_text(MSIME_EDITION_DISPLAY_NAME, 13, palette_.text);
    product.FontWeight(Windows::UI::Text::FontWeight{600});
    product.VerticalAlignment(VerticalAlignment::Center);
    identity.Children().Append(product);
    bar.Children().Append(identity);

    StackPanel tools;
    tools.Orientation(Orientation::Horizontal);
    tools.Spacing(8);
    tools.Margin(Thickness{12, 0, 0, 0});
    tools.VerticalAlignment(VerticalAlignment::Center);
    back_ = Button();
    back_.Width(32);
    back_.Height(30);
    back_.Padding(Thickness{0, 0, 0, 0});
    back_.CornerRadius(CornerRadius{6, 6, 6, 6});
    back_.Background(brush(transparent_color()));
    back_.BorderThickness(Thickness{0, 0, 0, 0});
    back_.Content(make_icon(0xE72B, 12, palette_.text));
    back_.IsEnabled(!history_.empty());
    A11y::SetName(back_, L"返回");
    back_.Click([this](Inspectable const &, RoutedEventArgs const &) { go_back(); });
    tools.Children().Append(back_);
    toolbar_title_ = make_text(page_label(current_page_).title, 13, palette_.sub);
    toolbar_title_.VerticalAlignment(VerticalAlignment::Center);
    tools.Children().Append(toolbar_title_);
    Grid::SetColumn(tools, 1);
    bar.Children().Append(tools);
    bar.Loaded([this](Inspectable const &, RoutedEventArgs const &) { update_passthrough(); });
    bar.SizeChanged([this](Inspectable const &, SizeChangedEventArgs const &) { update_passthrough(); });
    return bar;
  }

  AutoSuggestBox make_search_field() {
    search_ = AutoSuggestBox();
    search_.PlaceholderText(L"搜索设置");
    search_.QueryIcon(SymbolIcon(Symbol::Find));
    search_.Height(32);
    search_.Margin(Thickness{12, 4, 12, 12});
    search_.HorizontalAlignment(HorizontalAlignment::Stretch);
    search_.VerticalAlignment(VerticalAlignment::Center);
    A11y::SetName(search_, L"搜索设置");
    A11y::SetAutomationId(search_, L"settings-search");
    search_.TextChanged([this](AutoSuggestBox const &sender,
                               AutoSuggestBoxTextChangedEventArgs const &args) {
      const auto reason = args.Reason();
      if (reason == AutoSuggestionBoxTextChangeReason::UserInput) {
        search_query_ = sender.Text().c_str();
        update_suggestions(search_query_);
      } else if (reason == AutoSuggestionBoxTextChangeReason::SuggestionChosen &&
                 std::wstring_view(sender.Text()) == no_search_results) {
        // 无结果提示不能替换用户已经输入的查询。
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
    return search_;
  }

  // 标题栏保留拖动区域；返回按钮接收自己的点击。
  void update_passthrough() {
    if (!titlebar_ || !titlebar_.XamlRoot())
      return;
    const double scale = titlebar_.XamlRoot().RasterizationScale();
    std::vector<Windows::Graphics::RectInt32> rects;
    rects.reserve(2);
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
    Microsoft::UI::Input::InputNonClientPointerSource::GetForWindowId(
        AppWindow().Id())
        .SetRegionRects(Microsoft::UI::Input::NonClientRegionKind::Passthrough,
                        rects);
  }

  NavigationView make_navigation() {
    NavigationView view;
    view.PaneDisplayMode(NavigationViewPaneDisplayMode::Left);
    view.OpenPaneLength(224);
    view.CompactModeThresholdWidth(0);
    view.ExpandedModeThresholdWidth(0);
    view.IsPaneToggleButtonVisible(false);
    view.IsBackButtonVisible(NavigationViewBackButtonVisible::Collapsed);
    view.IsSettingsVisible(false);
    view.IsTitleBarAutoPaddingEnabled(false);
    view.AlwaysShowHeader(false);

    // 通过模板资源统一侧栏和正文配色，保留原生导航行为。
    auto resources = view.Resources();
    auto put = [&resources](const wchar_t *key, Color const &color) {
      resources.Insert(box_value(hstring(key)), brush(color));
    };
    put(L"NavigationViewContentBackground", palette_.content_bg);
    put(L"NavigationViewContentGridBorderBrush", palette_.edge);
    put(L"NavigationViewExpandedPaneBackground", transparent_color());
    put(L"NavigationViewDefaultPaneBackground", transparent_color());
    put(L"NavigationViewSelectionIndicatorForeground", transparent_color());
    put(L"NavigationViewItemBackgroundSelected", palette_.accent);
    put(L"NavigationViewItemBackgroundSelectedPointerOver", palette_.accent);
    put(L"NavigationViewItemBackgroundPointerOver", palette_.subtle);
    put(L"NavigationViewItemForegroundSelected", palette_.on_accent);
    put(L"NavigationViewItemForegroundSelectedPointerOver", palette_.on_accent);
    put(L"NavigationViewItemForegroundSelectedPressed", palette_.on_accent);
    put(L"NavigationViewItemIconForegroundSelected", palette_.on_accent);
    put(L"NavigationViewItemIconForegroundSelectedPointerOver", palette_.on_accent);
    put(L"NavigationViewItemIconForegroundSelectedPressed", palette_.on_accent);
    put(L"NavigationViewItemBackgroundSelectedPressed", palette_.accent);
    // 行高 28 包含模板上下各 2 DIP 的边距；内部仍用默认 36 会裁掉文字。
    resources.Insert(box_value(L"NavigationViewItemOnLeftMinHeight"), box_value(24.0));
    // 分组标题的模板把文字放进写死 40 DIP 高的 InnerHeaderGrid 里垂直居中，标题本身压到 24 就从中间截断，下半截不见（「工具」看着像「丁目」）。上下各 -8 DIP 把那 40 收进 24 里，文字整个落在其中；左右保持模板默认的 16。
    resources.Insert(box_value(L"NavigationViewItemInnerHeaderMargin"),
                     box_value(Thickness{16, -8, 16, -8}));
    put(L"NavigationViewItemSeparatorForeground", palette_.divider);

    view.PaneHeader(make_search_field());

    // 每组以标题开头；`SettingsNavigation.h` 的页面按组排序，所以组一变就该插入标题。
    std::optional<std::size_t> group;
    for (const auto &page : nav::pages) {
      if (page.group != group) {
        group = page.group;
        NavigationViewItemHeader header;
        header.Content(box_value(hstring(group_titles[page.group])));
        // 这里设不了字号和左右边距：模板里 HeaderText 的 NavigationViewItemHeaderTextStyle 是 StaticResource，在 generic.xaml 内就解析完了，页面资源覆盖不到，字号固定 14 DIP；模板根也不读 Padding，水平边距走上面那条 NavigationViewItemInnerHeaderMargin。Foreground 是本控件上的本地值，优先于模板样式，照常生效。
        header.MinHeight(24);
        header.Height(24);
        header.Foreground(brush(palette_.sub));
        view.MenuItems().Append(header);
      }
      const auto &label = page_label(page.id);
      NavigationViewItem item;
      item.Content(box_value(hstring(label.title)));
      auto icon = make_icon(label.glyph, 14, palette_.text);
      icon.ClearValue(IconElement::ForegroundProperty());
      item.Icon(icon);
      item.FontSize(13);
      item.MinHeight(28);
      item.Height(28);
      item.Margin(Thickness{6, 0, 6, 0});
      item.Padding(Thickness{8, 0, 8, 0});
      item.VerticalContentAlignment(VerticalAlignment::Center);
      item.Tag(box_value(text(page.id)));
      A11y::SetAutomationId(item, text(page.id));
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
    A11y::SetAutomationId(scroll_, L"settings-content");
    scroll_.VerticalScrollBarVisibility(ScrollBarVisibility::Auto);
    scroll_.HorizontalScrollBarVisibility(ScrollBarVisibility::Disabled);
    view.Content(scroll_);
    return view;
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
    // 每次进入「输入」页都重读已安装的辅助码表插件。
    if (id == "typing" && id != current_page_)
      helpcode_packs_stale_ = true;
    // 与共享页一样，每次进入「剪贴板」页都重读历史，并重新查一次能否发到云剪贴板。
    if (id == "clip" && id != current_page_) {
      cloud_relay_draft_.reset();
      clip_stale_ = true;
      reset_cloud_clipboard();
    }
    if (id == "stats" && id != current_page_)
      reset_statistics_page();
    if (id == "plugins" && id != current_page_)
      enter_plugins_page();
    // 社区图库与共享外壳一样随页面挂上和卸下：离开时作废在途的回答，回来时从头读起。标签停在哪一边跨页面保留。
    if (current_page_ == "plugins" && id != current_page_ && plugin_community_view_)
      leave_plugin_community();
    if (id == "plugins" && id != current_page_ && plugin_community_view_)
      enter_plugin_community();
    if (current_page_ == "themes" && id != current_page_ && skin_community_view_)
      leave_skin_community();
    if (id == "themes" && id != current_page_ && skin_community_view_)
      enter_skin_community();
    // 账号页与共享页一样每次进入都重新读状态；离开时结束等待中的 Google 登录，在途的回答作废。
    if (current_page_ == "account" && id != current_page_)
      leave_account_page();
    if (id == "account" && id != current_page_)
      enter_account_page();
    // 每次进入「主题」页都重扫皮肤目录，与共享页重新显示时一样。
    if (id == "themes" && id != current_page_)
      skins_stale_ = true;
    // 每次进入「语音输入」页都读一次录音设备，与共享页的「麦克风」下拉变为可见时一样。
    if (id == "voice" && id != current_page_)
      capture_stale_ = true;
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
    search_folds_.clear();
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
    if (!fold_index_.empty())
      search_folds_[title] = fold_index_;
    SearchEntry entry;
    entry.title = title;
    entry.detail = detail;
    entry.page = current_page_;
    entry.display = title + L" · " + page_label(current_page_).title;
    search_index_.push_back(std::move(entry));
  }

  // 搜索结果落在折叠区里的一行时，那个折叠区要展开才看得见它：索引时 record 记下每行所在的折叠区（`fold_index_`）。
  bool fold_holds_focus(std::wstring const &fold) const {
    const auto found = search_folds_.find(focus_row_);
    return found != search_folds_.end() && found->second == fold;
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
    matches.reserve(12);
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
    items.reserve(13);
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
    toolbar_title_.Text(page_label(current_page_).title);
    preview_host_ = nullptr;
    toolbar_preview_host_ = nullptr;
    toolbar_preview_size_ = nullptr;
    font_preset_host_ = nullptr;
    font_catalog_text_ = nullptr;
    font_catalog_button_ = nullptr;
    font_menu_updaters_.clear();
    focus_target_ = nullptr;

    StackPanel page;
    page.Padding(Thickness{22, 22, 22, 32});
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

    // 页面持有回调，弱引用避免页面与控件互相持有。
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
    row.Margin(Thickness{0, 0, 0, 8});
    ColumnDefinition title_column;
    title_column.Width(GridLength{1, GridUnitType::Star});
    ColumnDefinition tools_column;
    tools_column.Width(GridLength{1, GridUnitType::Auto});
    row.ColumnDefinitions().Append(title_column);
    row.ColumnDefinitions().Append(tools_column);

    auto title = make_text(page_label(current_page_).title, 22, palette_.text);
    A11y::SetAutomationId(title, text("settings-page-" + current_page_));
    title.FontWeight(Windows::UI::Text::FontWeight{600});
    title.VerticalAlignment(VerticalAlignment::Center);
    StackPanel heading;
    heading.Spacing(4);
    heading.Children().Append(title);
    heading.Children().Append(make_text(page_label(current_page_).summary, 11, palette_.sub));
    Grid::SetColumnSpan(heading, 2);
    row.Children().Append(heading);
    RowDefinition heading_row;
    heading_row.Height(GridLength{1, GridUnitType::Auto});
    RowDefinition actions_row;
    actions_row.Height(GridLength{1, GridUnitType::Auto});
    row.RowDefinitions().Append(heading_row);
    row.RowDefinitions().Append(actions_row);

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
      Grid::SetRow(tools, 1);
      Grid::SetColumnSpan(tools, 2);
      tools.HorizontalAlignment(HorizontalAlignment::Right);
      tools.Margin(Thickness{0, 8, 0, 0});
      row.Children().Append(tools);
    }
    return row;
  }

  static bool has_preview(std::string_view page) {
    return page == "candidate" || page == "themes";
  }

  FrameworkElement make_load_failure() {
    InfoBar bar;
    bar.IsOpen(true);
    bar.IsClosable(false);
    bar.Severity(InfoBarSeverity::Error);
    bar.Title(L"无法读取设置");
    A11y::SetAutomationId(bar, L"settings-load-error");
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
    A11y::SetAutomationId(bar, L"settings-notice");
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

  // 标题位于卡片之外；同一功能组的行共用一张圆角卡片。
  StackPanel add_group(StackPanel const &page, std::wstring const &title) {
    StackPanel section;
    section.Spacing(6);
    section.Margin(Thickness{0, 14, 0, 0});
    if (!title.empty()) {
      auto heading = make_text(title, 13, palette_.text);
      heading.FontWeight(Windows::UI::Text::FontWeight{600});
      section.Children().Append(heading);
    }
    StackPanel rows;
    rows.Padding(Thickness{12, 6, 12, 6});
    auto card = make_card();
    card.Child(rows);
    section.Children().Append(card);
    page.Children().Append(section);
    return rows;
  }

  Border make_card() {
    Border card;
    card.Background(brush(palette_.card));
    card.CornerRadius(CornerRadius{10, 10, 10, 10});
    return card;
  }

  // 组末的「更多选项」折叠区，对应共享页的 MoreOptions，写法与本地识别模型、AI 辅助两处的 Expander 相同：展开与否记在 `open` 这个成员里，跨页面重建保留；`focused` 为真（搜索落到折叠区里的某一行）时直接展开。返回放行的面板。
  StackPanel add_more_options(StackPanel const &group, bool &open, bool focused) {
    Expander more;
    more.Header(box_value(L"更多选项"));
    more.HorizontalAlignment(HorizontalAlignment::Stretch);
    more.HorizontalContentAlignment(HorizontalAlignment::Stretch);
    more.IsExpanded(open || focused);
    // `open` 是窗口自己的成员，与这些控件同寿。
    bool *const state = &open;
    more.Expanding([state](Expander const &, ExpanderExpandingEventArgs const &) {
      *state = true;
    });
    more.Collapsed([state](Expander const &, ExpanderCollapsedEventArgs const &) {
      *state = false;
    });
    StackPanel rows;
    rows.Spacing(4);
    more.Content(rows);
    group.Children().Append(more);
    return rows;
  }

  // 行内不重复画图标；标签和控件分别贴住卡片两边，窄窗口将控件放在下一行。
  Border add_row(StackPanel const &group, wchar_t,
                 std::wstring const &title, std::wstring const &subtitle,
                 FrameworkElement const &control,
                 FrameworkElement const &below = nullptr) {
    record(title, subtitle);
    Grid header;
    header.MinHeight(subtitle.empty() ? 30 : 38);
    header.Padding(Thickness{0, 4, 0, 4});
    header.ColumnSpacing(12);
    ColumnDefinition text_column;
    text_column.Width(GridLength{1, GridUnitType::Star});
    ColumnDefinition control_column;
    control_column.Width(GridLength{1, GridUnitType::Auto});
    header.ColumnDefinitions().Append(text_column);
    header.ColumnDefinitions().Append(control_column);
    RowDefinition label_row;
    label_row.Height(GridLength{1, GridUnitType::Auto});
    RowDefinition field_row;
    field_row.Height(GridLength{1, GridUnitType::Auto});
    header.RowDefinitions().Append(label_row);
    header.RowDefinitions().Append(field_row);

    StackPanel texts;
    texts.Spacing(2);
    texts.VerticalAlignment(VerticalAlignment::Center);
    texts.Children().Append(make_text(title, 13, palette_.text));
    if (!subtitle.empty())
      texts.Children().Append(make_text(subtitle, 11, palette_.sub));
    header.Children().Append(texts);

    if (control) {
      control.VerticalAlignment(VerticalAlignment::Center);
      control.HorizontalAlignment(HorizontalAlignment::Right);
      Grid::SetColumn(control, 1);
      header.Children().Append(control);
      // 用控件自身的期望宽度判断，长按钮、字体组合框和滑块都不挤掉标签。
      header.SizeChanged([weak_control = make_weak(control), weak_texts = make_weak(texts)](
                             Inspectable const &sender, SizeChangedEventArgs const &) {
        const auto row = sender.as<Grid>();
        const auto field = weak_control.get();
        const auto labels = weak_texts.get();
        if (!field || !labels)
          return;
        const bool stacked = row.ActualWidth() < field.DesiredSize().Width + 192;
        Grid::SetColumnSpan(labels, stacked ? 2 : 1);
        Grid::SetColumn(field, stacked ? 0 : 1);
        Grid::SetRow(field, stacked ? 1 : 0);
        Grid::SetColumnSpan(field, stacked ? 2 : 1);
        field.Margin(Thickness{0, stacked ? 6.0 : 0.0, 0, 0});
      });
    } else {
      Grid::SetColumnSpan(texts, 2);
    }

    StackPanel body;
    body.Children().Append(header);
    if (below)
      body.Children().Append(below);
    Border row;
    if (group.Children().Size() > 0) {
      row.BorderBrush(brush(palette_.divider));
      row.BorderThickness(Thickness{0, 1, 0, 0});
    }
    row.Child(body);
    group.Children().Append(row);
    mark_search_target(title, row);
    return row;
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

  // 开关、下拉框和分段控件写的是偏好，偏好没读到时不可用；`document` 为假的那些（打字统计页的开关、保留期和标签）不碰偏好，照常可用。
  FrameworkElement toggle_control(std::wstring const &name, bool on,
                                  std::function<void(bool)> apply,
                                  bool enabled = true, bool document = true) {
    ToggleSwitch toggle;
    toggle.OnContent(box_value(L""));
    toggle.OffContent(box_value(L""));
    toggle.MinWidth(0);
    toggle.IsOn(on);
    toggle.IsEnabled(enabled && (loaded_ || !document));
    A11y::SetName(toggle, hstring(name));
    toggle.Toggled([on_toggle = std::move(apply)](
                       Inspectable const &sender, RoutedEventArgs const &) {
      on_toggle(sender.as<ToggleSwitch>().IsOn());
    });
    return toggle;
  }

  FrameworkElement select_control(std::wstring const &name,
                                  std::vector<Option> options,
                                  std::wstring const &current,
                                  std::function<void(std::wstring const &)> apply,
                                  bool enabled = true, bool document = true) {
    ComboBox combo;
    combo.Width(168);
    combo.MinHeight(28);
    combo.FontSize(13);
    int32_t selected = -1;
    for (size_t i = 0; i < options.size(); ++i) {
      if (options[i].enabled) {
        combo.Items().Append(box_value(hstring(options[i].label)));
      } else {
        ComboBoxItem item;
        item.Content(box_value(hstring(options[i].label)));
        item.IsEnabled(false);
        combo.Items().Append(item);
      }
      if (options[i].value == current)
        selected = static_cast<int32_t>(i);
    }
    combo.SelectedIndex(selected);
    combo.IsEnabled(enabled && (loaded_ || !document));
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

  // 分段控件用浮起的选中底；默认超过三项用下拉框，紧凑页内标签可显式允许四项。
  FrameworkElement segmented_control(std::wstring const &name,
                                     std::vector<Option> const &options,
                                     std::wstring const &current,
                                     std::function<void(std::wstring const &)> apply,
                                     bool enabled = true, bool document = true,
                                     size_t maximum_segments = 3,
                                     std::wstring const &automation_prefix = L"") {
    if (options.size() > maximum_segments)
      return select_control(name, options, current, std::move(apply), enabled, document);
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
      button.Height(26);
      button.MinWidth(0);
      button.Padding(Thickness{10, 0, 10, 0});
      button.CornerRadius(CornerRadius{4, 4, 4, 4});
      button.BorderThickness(Thickness{1, 1, 1, 1});
      button.Background(brush(on ? (palette_.dark ? rgba(0x5A, 0x5A, 0x5E, 1) : rgba(255, 255, 255, 1)) : transparent_color()));
      button.BorderBrush(brush(on ? palette_.stroke : transparent_color()));
      button.HorizontalContentAlignment(HorizontalAlignment::Stretch);
      button.VerticalContentAlignment(VerticalAlignment::Stretch);
      Grid content;
      auto label = make_text(option.label, 13, on ? palette_.text : palette_.sub);
      label.TextWrapping(TextWrapping::NoWrap);
      label.HorizontalAlignment(HorizontalAlignment::Center);
      label.VerticalAlignment(VerticalAlignment::Center);
      content.Children().Append(label);
      button.Content(content);
      A11y::SetName(button, hstring(name + L" " + option.label));
      if (!automation_prefix.empty())
        A11y::SetAutomationId(button, hstring(automation_prefix + L"-" + option.value));
      if (on)
        A11y::SetItemStatus(button, L"已选择");
      button.IsEnabled(enabled && (loaded_ || !document) && option.enabled);
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
                                  bool enabled = true, double step = 1,
                                  std::wstring const &suffix = L"") {
    return formatted_slider_control(
        name, minimum, maximum, value, std::move(apply),
        [suffix](double shown) { return number_text(shown) + suffix; }, enabled, step);
  }

  // 数值由 format 格式化；两列布局给滑块与数值各自留出空间。
  Grid formatted_slider_control(std::wstring const &name, double minimum,
                                            double maximum, double value,
                                            std::function<void(double)> apply,
                                            std::function<std::wstring(double)> format,
                                            bool enabled = true, double step = 1) {
    Grid box;
    box.Width(218);
    box.ColumnSpacing(8);
    ColumnDefinition track_column;
    track_column.Width(GridLength{1, GridUnitType::Star});
    ColumnDefinition value_column;
    value_column.Width(GridLength{1, GridUnitType::Auto});
    box.ColumnDefinitions().Append(track_column);
    box.ColumnDefinitions().Append(value_column);
    Slider slider;
    slider.MinWidth(100);
    slider.Minimum(minimum);
    slider.Maximum(maximum);
    slider.StepFrequency(step);
    slider.SmallChange(step);
    slider.LargeChange(step);
    slider.Value(std::clamp(value, minimum, maximum));
    slider.VerticalAlignment(VerticalAlignment::Center);
    slider.IsEnabled(enabled && loaded_);
    A11y::SetName(slider, hstring(name));
    auto label = make_text(format(std::clamp(value, minimum, maximum)), 12,
                           palette_.text);
    label.MinWidth(48);
    label.TextWrapping(TextWrapping::NoWrap);
    label.TextAlignment(TextAlignment::Right);
    label.VerticalAlignment(VerticalAlignment::Center);
    Grid::SetColumn(label, 1);
    slider.ValueChanged(
        [weak_label = make_weak(label), on_slide = std::move(apply), step,
         format](Inspectable const &,
                 Controls::Primitives::RangeBaseValueChangedEventArgs const &args) {
          const double rounded = std::round(args.NewValue() / step) * step;
          if (auto block = weak_label.get())
            block.Text(hstring(format(rounded)));
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

  // 按可用宽度等分列，重排已有子项，保留按钮与复选框的焦点和选择状态。
  Grid responsive_grid(double minimum_width, double gap, int maximum_columns = 0) {
    Grid grid;
    grid.ColumnSpacing(gap);
    grid.RowSpacing(gap);
    grid.SizeChanged([minimum_width, gap, maximum_columns](
                         Inspectable const &sender, SizeChangedEventArgs const &) {
      const auto panel = sender.as<Grid>();
      auto columns = std::max(
          1, static_cast<int>((panel.ActualWidth() + gap) / (minimum_width + gap)));
      if (maximum_columns > 0)
        columns = std::min(columns, maximum_columns);
      if (static_cast<int>(panel.ColumnDefinitions().Size()) == columns)
        return;
      panel.ColumnDefinitions().Clear();
      panel.RowDefinitions().Clear();
      for (int column = 0; column < columns; ++column) {
        ColumnDefinition definition;
        definition.Width(GridLength{1, GridUnitType::Star});
        panel.ColumnDefinitions().Append(definition);
      }
      const auto count = static_cast<int>(panel.Children().Size());
      for (int index = 0; index < count; ++index) {
        if (index % columns == 0) {
          RowDefinition definition;
          definition.Height(GridLength{1, GridUnitType::Auto});
          panel.RowDefinitions().Append(definition);
        }
        const auto child = panel.Children()
                               .GetAt(static_cast<uint32_t>(index))
                               .as<FrameworkElement>();
        Grid::SetRow(child, index / columns);
        Grid::SetColumn(child, index % columns);
      }
    });
    return grid;
  }

  FrameworkElement checks_panel(std::vector<Check> checks,
                                double item_width = 174) {
    auto grid = responsive_grid(item_width, 8);
    grid.Margin(Thickness{0, 4, 0, 8});
    for (auto &check : checks) {
      record(check.label, L"");
      CheckBox box;
      box.Content(make_text(check.label, 12, palette_.text));
      box.IsChecked(check.on);
      box.MinWidth(0);
      box.MinHeight(28);
      box.HorizontalContentAlignment(HorizontalAlignment::Stretch);
      box.HorizontalAlignment(HorizontalAlignment::Stretch);
      A11y::SetName(box, hstring(check.label));
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
    options.reserve(values.size());
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
                  double fallback, double step = 1,
                  std::wstring const &suffix = L"") {
    add_row(group, glyph, title, subtitle,
            slider_control(title, minimum, maximum,
                           document_.Number(key, fallback),
                           [this, key](double value) {
                             document_.SetNumber(key, value);
                             schedule_save();
                             update_preview();
                           },
                           true, step, suffix));
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

  // ---- 检查更新 ----

  // 在「关于」页的控件上显示正在检查或上一次检查的结果；该页没有构建出来时什么都不做。
  void show_update_outcome() {
    if (!update_button_)
      return;
    const bool download =
        !update_checking_ && update_outcome_ && !update_outcome_->release_url.empty();
    update_button_.Content(box_value(hstring(
        update_checking_ ? L"正在检查…" : download ? L"前往下载" : L"检查")));
    update_button_.IsEnabled(!update_checking_);
    const std::wstring status = update_outcome_ ? update_outcome_->status : L"";
    const std::wstring detail = update_outcome_ ? update_outcome_->detail : L"";
    update_status_.Text(hstring(status));
    update_detail_.Text(hstring(detail));
    update_detail_.Visibility(detail.empty() ? Visibility::Collapsed : Visibility::Visible);
    update_lines_.Visibility(status.empty() ? Visibility::Collapsed : Visibility::Visible);
  }

  void on_update_button() {
    if (update_checking_)
      return;
    if (update_outcome_ && !update_outcome_->release_url.empty()) {
      const auto url = update_outcome_->release_url;
      open_url(url.c_str());
      return;
    }
    check_for_update();
  }

  // GitHub 可能要十秒才回应，所以请求在 UI 线程之外执行，只把结果传回来。
  fire_and_forget check_for_update() {
    update_checking_ = true;
    update_outcome_.reset();
    show_update_outcome();
    auto weak = get_weak();
    auto queue = DispatcherQueue();
    const auto body = utf8(update_check_request().Stringify());
    co_await resume_background();
    auto outcome = describe_update_check(take_response(msime_client_update_check(
        reinterpret_cast<const uint8_t *>(body.data()), body.size())));
    queue.TryEnqueue([weak, outcome = std::move(outcome)] {
      if (auto self = weak.get()) {
        self->update_checking_ = false;
        self->update_outcome_ = outcome;
        self->show_update_outcome();
      }
    });
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

  // 安装器随包装入的第三方组件许可声明，用系统关联的程序（通常是记事本）打开。
  void open_third_party_notices() {
    const auto notices = msime::settings::third_party_notices_path(shell_executable());
    std::error_code error;
    if (notices.empty() || !std::filesystem::is_regular_file(notices, error)) {
      show_notice(L"找不到第三方组件许可声明。请重新安装输入法后再试。");
      return;
    }
    const auto result = reinterpret_cast<INT_PTR>(ShellExecuteW(
        nullptr, L"open", notices.c_str(), nullptr, nullptr, SW_SHOWNORMAL));
    if (result <= 32)
      show_notice(L"无法打开第三方组件许可声明：" + notices.wstring());
  }

  // 「导出设置」「导入设置」的文件对话框：Windows 自己的保存或打开对话框，只列 .json。用户取消或对话框打不开时返回空。
  std::optional<std::filesystem::path> pick_settings_file(bool save) {
    com_ptr<IFileDialog> dialog;
    if (FAILED(CoCreateInstance(save ? CLSID_FileSaveDialog : CLSID_FileOpenDialog,
                                nullptr, CLSCTX_INPROC_SERVER,
                                IID_PPV_ARGS(dialog.put()))))
      return std::nullopt;
    const COMDLG_FILTERSPEC filter{L"设置文件 (*.json)", L"*.json"};
    dialog->SetFileTypes(1, &filter);
    dialog->SetDefaultExtension(L"json");
    dialog->SetTitle(save ? L"导出设置" : L"导入设置");
    dialog->SetOkButtonLabel(save ? L"导出" : L"导入");
    if (save)
      dialog->SetFileName(MSIME_EDITION_DISPLAY_NAME L"设置.json");
    FILEOPENDIALOGOPTIONS options = 0;
    if (SUCCEEDED(dialog->GetOptions(&options)))
      dialog->SetOptions(options | FOS_FORCEFILESYSTEM |
                         (save ? FOS_OVERWRITEPROMPT : FOS_FILEMUSTEXIST));
    // 点击按钮时本窗口就是这个线程的活动窗口，对话框以它为所有者，模态地挡住设置窗口。
    if (dialog->Show(GetActiveWindow()) != S_OK)
      return std::nullopt;
    com_ptr<IShellItem> item;
    if (FAILED(dialog->GetResult(item.put())))
      return std::nullopt;
    PWSTR raw = nullptr;
    if (FAILED(item->GetDisplayName(SIGDN_FILESYSPATH, &raw)) || !raw)
      return std::nullopt;
    std::filesystem::path chosen(raw);
    CoTaskMemFree(raw);
    return chosen;
  }

  // 导出的是已经保存的设置：先把还在等待的滑块修改存下，再由 host-api 读出并换算成设置文件。
  void export_settings() {
    flush_pending();
    const auto directory = state_directory();
    if (directory.empty()) {
      show_notice(L"找不到水杉输入法的数据目录。请先完成输入法安装。");
      return;
    }
    const auto target = pick_settings_file(true);
    if (!target)
      return;
    const auto path = path_utf8(directory);
    auto response = take_response(msime_client_export_settings(
        reinterpret_cast<const uint8_t *>(path.data()), path.size()));
    std::string contents;
    if (response.ok) {
      try {
        contents = utf8(
            JsonObject::Parse(text(response.text)).GetNamedString(L"value"));
      } catch (...) {
        response.ok = false;
      }
    }
    if (!response.ok) {
      show_notice(L"导出设置未能完成：无法读取当前设置。");
      return;
    }
    if (!msime::settings::write_settings_document(*target, contents)) {
      show_notice(L"导出设置未能完成：无法写入所选位置。");
      return;
    }
    show_notice(L"设置已导出到 " + target->wstring() + L"。", InfoBarSeverity::Success);
  }

  // 导入按本窗口读到的修订号比较并交换写回，与每个控件的保存一样；别处刚改过设置时拒绝导入，什么也不写。
  void import_settings() {
    flush_pending();
    const auto directory = state_directory();
    if (directory.empty()) {
      show_notice(L"找不到水杉输入法的数据目录。请先完成输入法安装。");
      return;
    }
    const auto source = pick_settings_file(false);
    if (!source)
      return;
    const auto contents = msime::settings::read_settings_document(*source);
    if (!contents) {
      show_notice(L"无法导入这个文件：读取失败，或者文件太大，不是水杉输入法导出的设置文件。");
      return;
    }
    const auto path = path_utf8(directory);
    auto response = take_response(msime_client_import_settings(
        reinterpret_cast<const uint8_t *>(path.data()), path.size(),
        document_.revision(), reinterpret_cast<const uint8_t *>(contents->data()),
        contents->size()));
    // 成功时控件要显示导入后的设置，修订号冲突时要换成别处刚存下的设置，所以两种情况都重新读取。
    reload_document();
    if (!response.ok) {
      std::string code;
      try {
        code = utf8(JsonObject::Parse(text(response.text)).GetNamedString(L"error", L""));
      } catch (...) {
      }
      show_notice(msime::settings::settings_import_error_message(code));
      return;
    }
    show_notice(L"设置已导入并立即生效。本机的服务配置与密钥、诊断日志、使用统计和剪贴板历史开关保持不变。",
                InfoBarSeverity::Success);
  }

  // 把文字放进剪贴板，成功时返回 true。别的程序正占着剪贴板时 SetContent 会抛 hresult_error，按钮的点击回调里不能让它漏出去；Flush 让内容在本窗口关闭后仍留在剪贴板里（否则 DataPackage 是延迟提供的，进程退出就没了）。
  static bool copy_text(std::wstring const &value) {
    try {
      Windows::ApplicationModel::DataTransfer::DataPackage package;
      package.SetText(hstring(value));
      Windows::ApplicationModel::DataTransfer::Clipboard::SetContent(package);
      Windows::ApplicationModel::DataTransfer::Clipboard::Flush();
      return true;
    } catch (hresult_error const &) {
      return false;
    }
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
    // 社区皮肤图库顶替主题页，候选窗口由自己的标签页管理预览。
    if (id == "themes")
      follow_skin_search();
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
    else if (id == "clip")
      build_clip_page(page);
    else if (id == "stats")
      build_stats_page(page);
    else if (id == "plugins")
      build_plugins_page(page);
    else if (id == "ai")
      build_ai_page(page);
    else if (id == "account")
      build_account_page(page);
    else if (id == "dev")
      build_dev_page(page);
    else if (id == "about")
      build_about_page(page);
  }

  // 交给共享应用的设置页（现在没有这样的页）只有一行，在共享应用里打开它。
  void build_shell_page(nav::Page const &model, StackPanel const &page) {
    const auto &label = page_label(model.id);
    auto group = add_group(page, L"");
    add_row(group, label.glyph, label.title, L"",
            button_control(L"打开", [this, target = model.shell] {
              open_shell(target);
            }));
    auto note = make_text(L"这一页由水杉输入法应用提供，点击「打开」后在应用中显示。",
                          12, palette_.faint);
    note.Margin(Thickness{4, 8, 4, 0});
    page.Children().Append(note);
  }

  // ---- 账号与同步 ----

  // 共享账号页（packages/ui/src/account/account-page.tsx）在 Windows 上画的是桌面那一支：没有 App 图标卡片（只有手机有）、没有「我的设计」（Windows 没有触屏键盘皮肤）、没有「关于」一组和登录后返回的「取消」（都只给手机或从别的页面跳来登录时）。没登录时是登录卡片（Google、邮箱、手机号验证码登录；没有可用的登录方式时给「刷新登录方式」，读不出本机登录状态时给「清除失效登录状态」）；登录后是资料卡片（点开「编辑资料」对话框：更换和移除头像、社区昵称、账号信息）、「设置同步」卡片、「云端」一组（云剪贴板面板是输入工具，仍由共享应用打开；云词库的入口在「词库」页，不登录也看得到，这里不再重复）、「我的内容」一组（共享应用的社区页，Windows 上只有词包与回复模板）和「账号」一组（退出登录、退出所有设备、注销账号）。所有账号操作经 msime_client_account，用共享应用和输入法读的同一份会话文件，除状态和取消 Google 登录外都在工作线程上进行；文案和规则在 AccountPageModel.h。

  struct AccountUser {
    std::wstring id, display_name, created_at, email, avatar_url;
    bool avatar_uploaded = false;
  };

  enum class AccountChannel { none, email, phone };

  // 本机自报的 User-Agent（#8201 的 P11），由 Rust 拼出：登录、绑定和匿名账号的注册都以它发出，服务端「我的设备」据此显示 windows、版本号和设备。整个进程拼一次，所有账号请求带同一个（msime_client_account 按 directory 和 client_agent 各有一个会话对象）；拼不出时为空，请求照旧不带它。
  static std::wstring const &account_client_agent() {
    static const std::wstring agent = [] {
      const auto device = msime::windows::windows_device_model();
      const auto system = msime::windows::windows_system_description();
      JsonObject request;
      request.SetNamedValue(L"platform", JsonValue::CreateStringValue(L"windows"));
      request.SetNamedValue(L"version", JsonValue::CreateStringValue(hstring(app_version)));
      request.SetNamedValue(L"device", device.empty() ? JsonValue::CreateNullValue()
                                                      : JsonValue::CreateStringValue(text(device)));
      request.SetNamedValue(L"system", system.empty() ? JsonValue::CreateNullValue()
                                                      : JsonValue::CreateStringValue(text(system)));
      request.SetNamedValue(L"edition", JsonValue::CreateStringValue(text(MSIME_EDITION_ID)));
      const auto body = utf8(request.Stringify());
      const auto response = take_response(msime_client_client_agent(
          reinterpret_cast<const uint8_t *>(body.data()), body.size()));
      if (!response.ok)
        return std::wstring{};
      const auto value = account_value(response);
      return value.ValueType() == JsonValueType::String ? std::wstring(value.GetString().c_str())
                                                        : std::wstring{};
    }();
    return agent;
  }

  // One msime_client_account request: the account directory the shared app signs in to, and the directory older versions kept the session in.
  static std::string account_body(JsonObject const &action) {
    JsonObject request;
    request.SetNamedValue(L"directory",
                          JsonValue::CreateStringValue(hstring(account_directory().wstring())));
    if (const auto legacy = legacy_account_directory(); !legacy.empty())
      request.SetNamedValue(L"legacy_directory",
                            JsonValue::CreateStringValue(hstring(legacy.wstring())));
    if (const auto &agent = account_client_agent(); !agent.empty())
      request.SetNamedValue(L"client_agent", JsonValue::CreateStringValue(hstring(agent)));
    request.SetNamedValue(L"action", JsonValue::Parse(action.Stringify()));
    return utf8(request.Stringify());
  }

  static Response account_call(std::string const &body) {
    return take_response(msime_client_account(reinterpret_cast<const uint8_t *>(body.data()),
                                              body.size()));
  }

  static JsonObject account_action(wchar_t const *operation) {
    JsonObject action;
    action.SetNamedValue(L"operation", JsonValue::CreateStringValue(operation));
    return action;
  }

  // The `value` of a successful answer, or null when it cannot be read.
  static JsonValue account_value(Response const &response) {
    try {
      return JsonObject::Parse(text(response.text))
          .GetNamedValue(L"value", JsonValue::CreateNullValue());
    } catch (...) {
      return JsonValue::CreateNullValue();
    }
  }

  static std::optional<AccountUser> parse_account_user(JsonValue const &value) {
    if (value.ValueType() != JsonValueType::Object)
      return std::nullopt;
    const auto object = value.GetObject();
    AccountUser user;
    user.id = named_string(object, L"id");
    if (user.id.empty())
      return std::nullopt;
    user.display_name = named_string(object, L"displayName");
    user.created_at = named_string(object, L"createdAt");
    user.email = named_string(object, L"email");
    user.avatar_url = named_string(object, L"avatarUrl");
    const auto uploaded = object.GetNamedValue(L"avatarUploaded", JsonValue::CreateNullValue());
    user.avatar_uploaded = uploaded.ValueType() == JsonValueType::Boolean && uploaded.GetBoolean();
    return user;
  }

  // `{user}` (status, login): the user, or nothing when signed out.
  static std::optional<AccountUser> status_user(JsonValue const &value) {
    if (value.ValueType() != JsonValueType::Object)
      return std::nullopt;
    return parse_account_user(
        value.GetObject().GetNamedValue(L"user", JsonValue::CreateNullValue()));
  }

  // Runs one account action off the UI thread; `done` gets the answer on it unless the page was left meanwhile.
  void account_request(JsonObject const &action, std::function<void(Response const &)> done) {
    const auto generation = account_generation_;
    if (account_directory().empty()) {
      // 读不到本机的应用数据目录，会话文件无从读写。
      done(Response{"{\"ok\":false,\"error\":\"account_storage\"}", false});
      return;
    }
    run_host([body = account_body(action)] { return account_call(body); },
             [this, generation, done = std::move(done)](Response const &response) {
               if (generation == account_generation_)
                 done(response);
             });
  }

  void redraw_account() {
    if (closed_)
      return;
    if (account_dialog_refresh_)
      account_dialog_refresh_();
    if (current_page_ == "account")
      queue_refresh();
  }

  // 共享页的 perform：一次只做一件；开始时清掉上一次的错误和提示。
  bool account_begin() {
    if (account_busy_)
      return false;
    account_busy_ = true;
    account_error_.clear();
    account_notice_.clear();
    redraw_account();
    return true;
  }

  // 结束一次操作；失败时（取消除外）在页面顶部说明。
  void account_finish(std::wstring const &code = {}, std::wstring notice = {}) {
    account_busy_ = false;
    if (!code.empty()) {
      if (!am::cancelled(code))
        account_error_ = am::error_message(code);
    } else if (!notice.empty()) {
      account_notice_ = std::move(notice);
    }
    redraw_account();
  }

  static std::wstring account_failure(Response const &response) {
    const auto code = response_error(response);
    return code.empty() ? L"account_unavailable" : code;
  }

  // 共享页的 applyProfile：资料、用户和昵称输入一起换。applyAvatar 只换资料和用户，对话框里正在输入的昵称留着。
  bool apply_account_profile(Response const &response, bool keep_name) {
    const auto value = account_value(response);
    if (value.ValueType() != JsonValueType::Object)
      return false;
    const auto object = value.GetObject();
    auto user = parse_account_user(object.GetNamedValue(L"user", JsonValue::CreateNullValue()));
    if (!user)
      return false;
    std::vector<std::wstring> providers;
    const auto list = object.GetNamedValue(L"providers", JsonValue::CreateNullValue());
    if (list.ValueType() == JsonValueType::Array)
      for (auto const &item : list.GetArray())
        if (item.ValueType() == JsonValueType::String)
          providers.emplace_back(item.GetString().c_str());
    account_user_ = std::move(user);
    account_providers_linked_ = std::move(providers);
    if (!keep_name)
      account_name_ = account_user_->display_name;
    return true;
  }

  void account_signed_out() {
    account_user_.reset();
    account_providers_linked_.clear();
    account_name_.clear();
    reset_settings_sync();
    reset_qq_links();
  }

  void reset_qq_links() {
    qq_links_.reset();
    qq_links_requested_ = false;
    qq_link_code_.clear();
    qq_link_busy_ = false;
    qq_link_message_.clear();
  }

  void enter_account_page() {
    ++account_generation_;
    account_stale_ = true;
    account_loading_ = true;
    account_busy_ = false;
    account_error_.clear();
    account_notice_.clear();
    account_channel_ = AccountChannel::none;
    account_target_.clear();
    account_code_.clear();
    account_challenge_.reset();
    account_resend_at_ = {};
    account_google_waiting_ = false;
    account_session_unreadable_ = false;
    // 登录方式每次进入本页重读：上一次读到的结果不能留下来，否则这次读失败时旧的 true 会藏起「暂无可用登录方式」和「刷新」。共享页每次重新挂载，状态本来就是新的。
    account_email_ = false;
    account_phone_ = false;
    account_google_ = false;
    account_avatar_requested_.clear();
    reset_settings_sync();
    reset_qq_links();
  }

  void leave_account_page() {
    if (resource_view_)
      leave_resource_community();
    cancel_account_google();
    ++account_generation_;
    account_busy_ = false;
    account_google_waiting_ = false;
    sync_busy_ = false;
    if (account_timer_)
      account_timer_.Stop();
    account_resend_button_ = nullptr;
    account_login_button_ = nullptr;
    account_code_panel_ = nullptr;
  }

  // 共享页挂载时并行读状态和登录方式：登录方式读不到时报错；有用户时再读资料。都读完才画页面。
  void load_account() {
    account_stale_ = false;
    account_loading_ = true;
    account_pending_status_.reset();
    account_pending_providers_.reset();
    account_request(account_action(L"status"), [this](Response const &response) {
      account_pending_status_ = response;
      finish_account_load();
    });
    account_request(account_action(L"providers"), [this](Response const &response) {
      account_pending_providers_ = response;
      finish_account_load();
    });
  }

  void finish_account_load() {
    if (!account_pending_status_ || !account_pending_providers_)
      return;
    const auto status = std::move(*account_pending_status_);
    const auto providers = std::move(*account_pending_providers_);
    account_pending_status_.reset();
    account_pending_providers_.reset();
    if (providers.ok)
      apply_account_providers(providers);
    else if (const auto code = account_failure(providers); !am::cancelled(code))
      account_error_ = am::error_message(code);
    if (!status.ok) {
      if (const auto code = account_failure(status); !am::cancelled(code)) {
        account_error_ = am::error_message(code);
        account_session_unreadable_ = true;
      }
      account_signed_out();
      account_loading_ = false;
      redraw_account();
      return;
    }
    account_session_unreadable_ = false;
    account_user_ = status_user(account_value(status));
    if (!account_user_) {
      account_signed_out();
      account_loading_ = false;
      redraw_account();
      return;
    }
    account_name_ = account_user_->display_name;
    account_request(account_action(L"profile"), [this](Response const &response) {
      if (!response.ok || !apply_account_profile(response, false)) {
        if (const auto code = account_failure(response); !am::cancelled(code))
          account_error_ = am::error_message(code);
      }
      account_loading_ = false;
      redraw_account();
    });
  }

  void apply_account_providers(Response const &response) {
    const auto value = account_value(response);
    if (value.ValueType() != JsonValueType::Object)
      return;
    const auto object = value.GetObject();
    auto flag = [&object](wchar_t const *key) {
      const auto item = object.GetNamedValue(key, JsonValue::CreateNullValue());
      return item.ValueType() == JsonValueType::Boolean && item.GetBoolean();
    };
    account_email_ = flag(L"email");
    account_phone_ = flag(L"phone");
    account_google_ = flag(L"google");
  }

  // 登录（验证码或 Google）之后：记下用户，读资料，成功时说「登录成功。」。
  void account_signed_in(Response const &response) {
    auto user = status_user(account_value(response));
    if (!user) {
      account_finish(L"account_unavailable");
      return;
    }
    // 登录成功说明保存的登录状态是可读的，之前那次读不出来的结论作废；否则退出登录后又会冒出「清除失效登录状态」。
    account_session_unreadable_ = false;
    account_user_ = std::move(user);
    account_name_ = account_user_->display_name;
    account_providers_linked_.clear();
    account_request(account_action(L"profile"), [this](Response const &profile) {
      if (!profile.ok || !apply_account_profile(profile, false)) {
        account_finish(account_failure(profile));
        return;
      }
      account_finish({}, L"登录成功。");
    });
  }

  void choose_account_channel(AccountChannel channel) {
    cancel_account_google();
    account_channel_ = channel;
    account_target_.clear();
    account_code_.clear();
    account_challenge_.reset();
    account_error_.clear();
    account_notice_.clear();
    redraw_account();
  }

  static int64_t account_now_ms() {
    return std::chrono::duration_cast<std::chrono::milliseconds>(
               std::chrono::steady_clock::now().time_since_epoch())
        .count();
  }

  void request_account_code() {
    if (account_channel_ == AccountChannel::none || !account_begin())
      return;
    const auto target = am::trim_login_target(account_target_);
    if (target.empty()) {
      account_finish(L"account_invalid");
      return;
    }
    auto action = account_action(L"request_code");
    action.SetNamedValue(L"provider", JsonValue::CreateStringValue(
                                          account_channel_ == AccountChannel::email ? L"email"
                                                                                     : L"phone"));
    action.SetNamedValue(L"target", JsonValue::CreateStringValue(hstring(target)));
    account_request(action, [this](Response const &response) {
      if (!response.ok) {
        account_finish(account_failure(response));
        return;
      }
      const auto value = account_value(response);
      if (value.ValueType() != JsonValueType::Object) {
        account_finish(L"account_unavailable");
        return;
      }
      const auto object = value.GetObject();
      const auto id = named_string(object, L"challengeId");
      const auto expires = object.GetNamedValue(L"expiresIn", JsonValue::CreateNullValue());
      if (id.empty() || expires.ValueType() != JsonValueType::Number) {
        account_finish(L"account_unavailable");
        return;
      }
      const auto now = account_now_ms();
      account_challenge_ = id;
      account_expires_at_ = now + static_cast<int64_t>(std::llround(expires.GetNumber() * 1000));
      account_resend_at_ = now + am::resend_interval_ms;
      account_code_.clear();
      start_account_timer();
      account_finish({}, L"验证码已发送，请查收。");
    });
  }

  void sign_in_with_code() {
    if (!account_begin())
      return;
    if (!account_challenge_ || !am::six_digits(account_code_) ||
        account_expires_at_ <= account_now_ms()) {
      account_finish(L"account_invalid");
      return;
    }
    auto action = account_action(L"login");
    action.SetNamedValue(L"challenge_id", JsonValue::CreateStringValue(hstring(*account_challenge_)));
    action.SetNamedValue(L"code", JsonValue::CreateStringValue(hstring(account_code_)));
    account_request(action, [this](Response const &response) {
      if (!response.ok) {
        account_finish(account_failure(response));
        return;
      }
      account_channel_ = AccountChannel::none;
      account_challenge_.reset();
      account_code_.clear();
      account_signed_in(response);
    });
  }

  // 共享页的 signInWithGoogle：Rust 监听回环端口、用系统浏览器打开授权页并等它回跳，最多约 5 分钟；等待时按钮显示在等浏览器，旁边是「取消 Google 登录」。
  void sign_in_with_google() {
    if (account_busy_)
      return;
    account_google_waiting_ = true;
    account_begin();
    account_request(account_action(L"google_login"), [this](Response const &response) {
      account_google_waiting_ = false;
      if (!response.ok) {
        account_finish(account_failure(response));
        return;
      }
      account_signed_in(response);
    });
  }

  // 结束等待中的 Google 登录，它随后以取消结束。只是清掉内存里的等待，不联网，所以在 UI 线程上直接调用。
  void cancel_account_google() {
    if (!account_google_waiting_ || account_directory().empty())
      return;
    account_call(account_body(account_action(L"google_cancel")));
  }

  void refresh_account_providers() {
    if (!account_begin())
      return;
    account_request(account_action(L"providers"), [this](Response const &response) {
      if (!response.ok) {
        account_finish(account_failure(response));
        return;
      }
      apply_account_providers(response);
      account_finish();
    });
  }

  // 「清除失效登录状态」「退出登录」「退出所有设备」「注销账号」：成功后回到未登录，说明结束的方式。
  void end_account_session(JsonObject const &action, std::wstring notice) {
    if (!account_begin())
      return;
    account_request(action, [this, notice = std::move(notice)](Response const &response) {
      if (!response.ok) {
        account_finish(account_failure(response));
        return;
      }
      account_signed_out();
      account_session_unreadable_ = false;
      account_finish({}, notice);
    });
  }

  void sign_out_account(bool all) {
    auto action = account_action(L"logout");
    action.SetNamedValue(L"all", JsonValue::CreateBooleanValue(all));
    end_account_session(action, all ? L"已退出所有设备。" : L"已退出登录。");
  }

  // 「退出所有设备」「注销账号」先确认（AccountConfirmation）。
  fire_and_forget confirm_account_action(am::Confirmation which) {
    if (account_busy_)
      co_return;
    auto strong = get_strong();
    const auto copy = am::confirmation_text(which);
    if (!co_await confirm(std::wstring(copy.label), std::wstring(copy.message),
                          std::wstring(copy.label)))
      co_return;
    if (which == am::Confirmation::logout_all)
      sign_out_account(true);
    else
      // account_delete 与旧的 delete 相同，只是失败时带细节：不是 10 分钟内登录的会话是 account_recent_login_required，本机登录留着，提示先退出再重新登录。
      end_account_session(account_action(L"account_delete"), L"账号已注销。");
  }

  void rename_account() {
    if (!account_begin())
      return;
    const auto name = account_name_presentation(account_name_);
    if (!name) {
      account_finish(L"account_unavailable");
      return;
    }
    if (!name->valid) {
      account_finish(L"account_invalid");
      return;
    }
    auto action = account_action(L"rename");
    action.SetNamedValue(L"display_name",
                         JsonValue::CreateStringValue(hstring(name->normalized)));
    account_request(action, [this](Response const &response) {
      if (!response.ok || !apply_account_profile(response, false)) {
        account_finish(account_failure(response));
        return;
      }
      account_finish({}, L"昵称已更新。");
    });
  }

  // 共享应用的 account_choose_avatar：系统打开对话框（标题「选择头像」，只列 png、jpg、jpeg），取消时什么也不做；Rust 拒绝不是 1 MiB 以内 PNG 或 JPEG 的文件（account_invalid），这时说的是图片不对。
  void choose_account_avatar() {
    if (!account_begin())
      return;
    std::optional<std::filesystem::path> picked;
    com_ptr<IFileOpenDialog> dialog;
    if (SUCCEEDED(CoCreateInstance(CLSID_FileOpenDialog, nullptr, CLSCTX_INPROC_SERVER,
                                   IID_PPV_ARGS(dialog.put())))) {
      const COMDLG_FILTERSPEC filter{L"图片", L"*.png;*.jpg;*.jpeg"};
      dialog->SetFileTypes(1, &filter);
      dialog->SetTitle(L"选择头像");
      FILEOPENDIALOGOPTIONS options = 0;
      if (SUCCEEDED(dialog->GetOptions(&options)))
        dialog->SetOptions(options | FOS_FORCEFILESYSTEM | FOS_FILEMUSTEXIST);
      com_ptr<IShellItem> item;
      PWSTR raw = nullptr;
      if (dialog->Show(GetActiveWindow()) == S_OK && SUCCEEDED(dialog->GetResult(item.put())) &&
          SUCCEEDED(item->GetDisplayName(SIGDN_FILESYSPATH, &raw)) && raw)
        picked = std::filesystem::path(raw);
      CoTaskMemFree(raw);
    }
    if (!picked) {
      account_finish();
      return;
    }
    auto action = account_action(L"upload_avatar");
    action.SetNamedValue(L"path", JsonValue::CreateStringValue(hstring(picked->wstring())));
    account_request(action, [this](Response const &response) {
      if (!response.ok) {
        const auto code = account_failure(response);
        account_finish(code == L"account_invalid" ? L"account_avatar_invalid" : code);
        return;
      }
      if (!apply_account_profile(response, true)) {
        account_finish(L"account_unavailable");
        return;
      }
      account_finish({}, L"头像已更新。");
    });
  }

  void remove_account_avatar() {
    if (!account_begin())
      return;
    account_request(account_action(L"remove_avatar"), [this](Response const &response) {
      if (!response.ok || !apply_account_profile(response, true)) {
        account_finish(account_failure(response));
        return;
      }
      account_finish({}, L"已移除头像。");
    });
  }

  // 共享页的 copyAccountId：复制完整的账号 ID，按钮显示「已复制」1.8 秒；复制不了时说明。
  void copy_account_id() {
    if (!account_user_)
      return;
    if (!copy_text(account_user_->id)) {
      account_error_ = L"账号 ID 暂时无法复制，请稍后重试。";
      redraw_account();
      return;
    }
    account_id_copied_ = true;
    if (!account_copied_timer_) {
      account_copied_timer_ = DispatcherQueue().CreateTimer();
      account_copied_timer_.Interval(std::chrono::milliseconds(1800));
      account_copied_timer_.IsRepeating(false);
      account_copied_timer_.Tick([this](Microsoft::UI::Dispatching::DispatcherQueueTimer const &,
                                        Inspectable const &) {
        account_id_copied_ = false;
        if (account_dialog_refresh_)
          account_dialog_refresh_();
      });
    }
    account_copied_timer_.Stop();
    account_copied_timer_.Start();
    if (account_dialog_refresh_)
      account_dialog_refresh_();
  }

  // ---- 头像 ----

  // 页面不取远程图片：头像经 msime_client_account 的 avatar 取成 data: 地址再解码。avatarUrl 变了才再取；取不到时显示名字的第一个字。
  void load_account_avatar() {
    if (!account_user_ || account_user_->avatar_url.empty())
      return;
    const auto url = account_user_->avatar_url;
    if (url == account_avatar_url_ || url == account_avatar_requested_)
      return;
    account_avatar_requested_ = url;
    account_request(account_action(L"avatar"), [this, url](Response const &response) {
      const auto value = account_value(response);
      if (!response.ok || value.ValueType() != JsonValueType::String)
        return;
      show_account_avatar(url, std::wstring(value.GetString().c_str()));
    });
  }

  fire_and_forget show_account_avatar(std::wstring url, std::wstring data) {
    auto strong = get_strong();
    const auto comma = data.find(L',');
    if (!(data.starts_with(L"data:image/png;base64,") ||
          data.starts_with(L"data:image/jpeg;base64,")) ||
        comma == std::wstring::npos)
      co_return;
    Imaging::BitmapImage bitmap;
    try {
      const auto bytes =
          Windows::Security::Cryptography::CryptographicBuffer::DecodeFromBase64String(
              hstring(data.substr(comma + 1)));
      Windows::Storage::Streams::InMemoryRandomAccessStream stream;
      co_await stream.WriteAsync(bytes);
      stream.Seek(0);
      co_await bitmap.SetSourceAsync(stream);
    } catch (hresult_error const &) {
      co_return;
    }
    if (closed_ || !account_user_ || account_user_->avatar_url != url)
      co_return;
    account_avatar_ = bitmap;
    account_avatar_url_ = url;
    redraw_account();
  }

  // 圆形头像（AccountAvatar）：取到的图片，或强调色底上名字的第一个字。`name` 是代替图片的那个名字。
  FrameworkElement make_account_avatar(double size, double font_size, std::wstring const &name) {
    Grid avatar;
    avatar.Width(size);
    avatar.Height(size);
    Shapes::Ellipse circle;
    circle.Fill(brush(palette_.accent));
    avatar.Children().Append(circle);
    const bool image = account_user_ && account_avatar_ && !account_user_->avatar_url.empty() &&
                       account_user_->avatar_url == account_avatar_url_;
    if (image) {
      Shapes::Ellipse photo;
      ImageBrush fill;
      fill.ImageSource(account_avatar_);
      fill.Stretch(Stretch::UniformToFill);
      photo.Fill(fill);
      avatar.Children().Append(photo);
    } else {
      auto letter = make_text(am::initial(name), font_size, palette_.on_accent);
      letter.FontWeight(Windows::UI::Text::FontWeight{650});
      letter.TextWrapping(TextWrapping::NoWrap);
      letter.HorizontalAlignment(HorizontalAlignment::Center);
      letter.VerticalAlignment(VerticalAlignment::Center);
      avatar.Children().Append(letter);
    }
    return avatar;
  }

  // ---- 验证码 ----

  // 「获取验证码」的倒计时和「登录」按钮随时间变（共享页每秒更新 now）；只改这两个按钮，不重画页面，输入框的焦点留着。重发倒计时没走完时也走，换了号码之后倒计时照样结束。
  void start_account_timer() {
    if (!account_timer_) {
      account_timer_ = DispatcherQueue().CreateTimer();
      account_timer_.Interval(std::chrono::seconds(1));
      account_timer_.IsRepeating(true);
      account_timer_.Tick([this](Microsoft::UI::Dispatching::DispatcherQueueTimer const &,
                                 Inspectable const &) {
        update_account_code_controls();
        if ((!account_challenge_ && account_resend_at_ <= account_now_ms()) ||
            current_page_ != "account")
          account_timer_.Stop();
      });
    }
    account_timer_.Start();
  }

  void update_account_code_controls() {
    const auto now = account_now_ms();
    const auto seconds = am::resend_seconds(account_resend_at_ - now);
    const bool expired = account_challenge_ && account_expires_at_ <= now;
    if (account_resend_button_) {
      account_resend_button_.Content(box_value(hstring(am::resend_label(seconds))));
      account_resend_button_.IsEnabled(!account_busy_ &&
                                       !am::trim_login_target(account_target_).empty() &&
                                       seconds == 0);
    }
    if (account_login_button_) {
      account_login_button_.Content(box_value(hstring(am::login_label(expired, account_busy_))));
      account_login_button_.IsEnabled(!account_busy_ && !expired && am::six_digits(account_code_));
    }
  }

  // 「加入水杉」：createdAt 在本机时区的日期（toLocaleDateString("zh-CN")），读不懂时为空。
  static std::wstring account_join_date(std::wstring const &created_at) {
    const auto milliseconds = am::parse_iso_time(created_at);
    // FILETIME counts 100-nanosecond intervals from 1601.
    if (!milliseconds || *milliseconds < -11644473600000LL || *milliseconds > 900000000000000LL)
      return {};
    ULARGE_INTEGER ticks{};
    ticks.QuadPart = static_cast<uint64_t>(*milliseconds + 11644473600000LL) * 10000ULL;
    const FILETIME utc{ticks.LowPart, ticks.HighPart};
    SYSTEMTIME universal{}, local{};
    if (!FileTimeToSystemTime(&utc, &universal) ||
        !SystemTimeToTzSpecificLocalTime(nullptr, &universal, &local))
      return {};
    return am::zh_date(local.wYear, local.wMonth, local.wDay);
  }

  // ---- 设置同步 ----

  // Windows 的设置同步卡片（SettingsSyncCard 的桌面卡片，createDesktopSettingsSync）：读字段表和云端设置，上传本机设置，下载并应用云端设置。只同步各平台共有的 input.* 设置，偏好目录与本窗口读写的相同。卡片有自己的忙碌状态和一行说明（错误和结果都写在这里）；换了账号就重新读。

  void reset_settings_sync() {
    ++sync_generation_;
    sync_user_.clear();
    sync_ready_ = false;
    sync_cloud_.reset();
    sync_busy_ = false;
    sync_message_.clear();
  }

  // One settings sync action; the answer is dropped when the account or page changed meanwhile.
  void sync_request(JsonObject const &action, std::function<void(Response const &)> done) {
    const auto generation = sync_generation_;
    account_request(action, [this, generation, done = std::move(done)](Response const &response) {
      if (generation == sync_generation_)
        done(response);
    });
  }

  bool sync_begin() {
    if (sync_busy_)
      return false;
    sync_busy_ = true;
    sync_message_.clear();
    redraw_account();
    return true;
  }

  void sync_finish(std::wstring const &code = {}, std::wstring message = {}) {
    sync_busy_ = false;
    if (!code.empty()) {
      if (!am::cancelled(code))
        sync_message_ = am::error_message(code);
    } else {
      sync_message_ = std::move(message);
    }
    redraw_account();
  }

  // 云端设置文档 {revision, settings}；读不懂时为空。
  static std::optional<JsonObject> sync_document(Response const &response) {
    const auto value = account_value(response);
    if (value.ValueType() != JsonValueType::Object)
      return std::nullopt;
    const auto object = value.GetObject();
    const auto revision = object.GetNamedValue(L"revision", JsonValue::CreateNullValue());
    const auto settings = object.GetNamedValue(L"settings", JsonValue::CreateNullValue());
    if (revision.ValueType() != JsonValueType::Number ||
        settings.ValueType() != JsonValueType::Object)
      return std::nullopt;
    return object;
  }

  // 卡片出现时和「刷新云端设置」：先读字段表，再读云端设置，两样都有了上传和应用才能用。
  void load_settings_sync() {
    if (!account_user_ || !sync_begin())
      return;
    sync_user_ = account_user_->id;
    const auto generation = sync_generation_;
    if (account_directory().empty()) {
      sync_finish(L"account_storage");
      return;
    }
    run_host(
        [schema = account_body(account_action(L"preferences_schema")),
         load = account_body(account_action(L"preferences_load"))] {
          auto response = account_call(schema);
          return response.ok ? account_call(load) : response;
        },
        [this, generation](Response const &response) {
          if (generation != sync_generation_ || closed_)
            return;
          if (!response.ok) {
            sync_finish(account_failure(response));
            return;
          }
          auto document = sync_document(response);
          if (!document) {
            sync_finish(L"account_unavailable");
            return;
          }
          sync_ready_ = true;
          sync_cloud_ = std::move(*document);
          sync_finish();
        });
  }

  fire_and_forget upload_settings_sync() {
    if (sync_busy_ || !sync_ready_ || !sync_cloud_)
      co_return;
    auto strong = get_strong();
    if (!co_await confirm(L"确认上传本机设置", std::wstring(am::sync_upload_confirmation),
                          L"确认上传"))
      co_return;
    // 上传的是已经保存的设置，先把还在等待的滑块修改存下。
    flush_pending();
    if (!sync_begin())
      co_return;
    auto action = account_action(L"preferences_upload");
    action.SetNamedValue(L"preferences_directory",
                         JsonValue::CreateStringValue(hstring(state_directory().wstring())));
    sync_request(action, [this](Response const &response) {
      if (!response.ok) {
        sync_finish(account_failure(response));
        return;
      }
      if (auto document = sync_document(response))
        sync_cloud_ = std::move(*document);
      sync_finish({}, std::wstring(am::sync_uploaded));
    });
  }

  fire_and_forget apply_settings_sync() {
    if (sync_busy_ || !sync_ready_ || !sync_cloud_ || !sync_has_settings())
      co_return;
    auto strong = get_strong();
    if (!co_await confirm(L"确认应用云端设置", std::wstring(am::sync_apply_confirmation),
                          L"确认应用"))
      co_return;
    flush_pending();
    if (!sync_cloud_ || !sync_begin())
      co_return;
    auto action = account_action(L"preferences_apply");
    action.SetNamedValue(L"preferences_directory",
                         JsonValue::CreateStringValue(hstring(state_directory().wstring())));
    action.SetNamedValue(L"user_id", JsonValue::CreateStringValue(hstring(sync_user_)));
    action.SetNamedValue(L"preferences", JsonValue::Parse(sync_cloud_->Stringify()));
    if (account_directory().empty()) {
      sync_finish(L"account_storage");
      co_return;
    }
    // 不经 sync_request：应用期间离开了这一页或换了账号，卡片的结果作废，但本机设置可能已经换掉，本窗口的设置文档照样要重读，免得之后在别的页上按旧文档保存。
    const auto generation = sync_generation_;
    const auto page_generation = account_generation_;
    run_host([body = account_body(action)] { return account_call(body); },
             [this, generation, page_generation](Response const &response) {
               // 应用成功时本机设置已换成云端的那一份，写回被拒绝时也可能是别处刚改过，两种情况都重读，各页显示存下的值。
               reload_document();
               if (generation != sync_generation_ || page_generation != account_generation_) {
                 queue_refresh();
                 return;
               }
               if (!response.ok) {
                 sync_finish(account_failure(response));
                 return;
               }
               sync_finish({}, std::wstring(am::sync_applied));
             });
  }

  bool sync_has_settings() const {
    return sync_cloud_ && sync_cloud_->GetNamedObject(L"settings").Size() > 0;
  }

  void build_settings_sync(StackPanel const &page) {
    if (!indexing_ && account_user_ && sync_user_ != account_user_->id)
      load_settings_sync();
    auto group = add_group(page, L"设置同步");
    record(L"设置同步", std::wstring(am::sync_description));
    StackPanel body;
    body.Spacing(12);
    body.Padding(Thickness{0, 8, 0, 8});
    body.Children().Append(make_text(am::sync_description, 13, palette_.sub));
    if (sync_cloud_)
      body.Children().Append(make_text(
          L"云端版本：" +
              std::to_wstring(std::llround(sync_cloud_->GetNamedNumber(L"revision", 0))),
          12, palette_.sub));
    StackPanel actions;
    actions.Spacing(8);
    actions.Children().Append(
        button_control(L"刷新云端设置", [this] { load_settings_sync(); }, !sync_busy_));
    auto upload = button_control(L"上传本机设置", [this] { upload_settings_sync(); },
                                 !sync_busy_ && sync_ready_ && sync_cloud_.has_value());
    make_primary(upload.as<Button>());
    actions.Children().Append(upload);
    actions.Children().Append(button_control(L"下载并应用云端设置",
                                             [this] { apply_settings_sync(); },
                                             !sync_busy_ && sync_ready_ && sync_has_settings()));
    body.Children().Append(actions);
    if (sync_busy_)
      body.Children().Append(make_text(L"正在处理…", 12, palette_.sub));
    if (!sync_message_.empty())
      body.Children().Append(make_text(sync_message_, 12, palette_.text));
    A11y::SetName(body, L"设置同步");
    group.Children().Append(body);
    mark_search_target(L"设置同步", body);
  }

  // ---- 绑定 QQ 群助手 ----
  // 共享账号页的 QqLinkCard（`packages/ui/src/account/qq-link-card.tsx`）：输入机器人给的绑定码，列出已有的绑定，可以解绑。经 msime_client_account 的 qq_links、qq_link_redeem、qq_link_delete，只用登录账号；离开账号页或换了账号时 account_generation_ 变了，旧的回答作废。文案和错误说法在 AccountPageModel.h。

  void load_qq_links() {
    qq_links_requested_ = true;
    account_request(account_action(L"qq_links"), [this](Response const &response) {
      const auto value = account_value(response);
      if (response.ok && value.ValueType() == JsonValueType::Object) {
        std::vector<std::pair<std::wstring, std::wstring>> links;
        const auto list = value.GetObject().GetNamedValue(L"links", JsonValue::CreateNullValue());
        if (list.ValueType() == JsonValueType::Array)
          for (auto const &item : list.GetArray()) {
            if (item.ValueType() != JsonValueType::Object)
              continue;
            const auto link = item.GetObject();
            auto id = named_string(link, L"id");
            if (!id.empty())
              links.emplace_back(std::move(id), named_string(link, L"created_at"));
          }
        qq_links_ = std::move(links);
      } else if (!response.ok) {
        qq_link_message_ = am::error_message(account_failure(response));
      }
      redraw_account();
    });
  }

  void redeem_qq_link() {
    const auto digits = am::qq_link_digits(qq_link_code_);
    if (qq_link_busy_ || !am::eight_digits(digits))
      return;
    qq_link_busy_ = true;
    qq_link_message_.clear();
    auto action = account_action(L"qq_link_redeem");
    action.SetNamedValue(L"code", JsonValue::CreateStringValue(hstring(digits)));
    account_request(action, [this](Response const &response) {
      qq_link_busy_ = false;
      if (!response.ok) {
        // 绑定码的几种失败各有说法，不交给通用的账号错误。
        qq_link_message_ = am::qq_link_message(account_failure(response));
        redraw_account();
        return;
      }
      qq_link_code_.clear();
      qq_link_message_ = std::wstring(am::qq_link_bound);
      load_qq_links();
    });
    redraw_account();
  }

  void remove_qq_link(std::wstring const &id) {
    if (qq_link_busy_)
      return;
    qq_link_busy_ = true;
    qq_link_message_.clear();
    auto action = account_action(L"qq_link_delete");
    action.SetNamedValue(L"id", JsonValue::CreateStringValue(hstring(id)));
    account_request(action, [this](Response const &response) {
      qq_link_busy_ = false;
      if (!response.ok) {
        qq_link_message_ = am::error_message(account_failure(response));
        redraw_account();
        return;
      }
      qq_link_message_ = std::wstring(am::qq_link_removed);
      load_qq_links();
    });
    redraw_account();
  }

  // 绑定时间：服务端给的 RFC 3339 换成本地时间，读不懂时原样写。
  static std::wstring qq_link_time(std::wstring const &created_at) {
    if (const auto milliseconds = am::parse_iso_time(created_at); milliseconds && *milliseconds > 0)
      if (auto text = statistics_written_text(static_cast<uint64_t>(*milliseconds)); !text.empty())
        return text;
    return created_at;
  }

  void build_qq_link_card(StackPanel const &page) {
    if (!qq_links_requested_)
      load_qq_links();
    auto group = add_group(page, L"绑定 QQ 群助手");
    record(L"绑定 QQ 群助手", std::wstring(am::qq_link_note));
    StackPanel body;
    body.Spacing(12);
    body.Padding(Thickness{0, 8, 0, 8});
    body.Children().Append(make_text(am::qq_link_note, 13, palette_.sub));

    TextBox code;
    code.Width(160);
    code.FontSize(13);
    code.MaxLength(10);
    code.PlaceholderText(L"8 位数字");
    code.Text(hstring(qq_link_code_));
    code.IsEnabled(!qq_link_busy_);
    A11y::SetName(code, L"绑定码");
    {
      Input::InputScope scope;
      Input::InputScopeName name;
      name.NameValue(Input::InputScopeNameValue::Number);
      scope.Names().Append(name);
      code.InputScope(scope);
    }
    auto bind = button_control(L"绑定", [this] { redeem_qq_link(); },
                               !qq_link_busy_ && am::eight_digits(am::qq_link_digits(qq_link_code_)))
                    .as<Button>();
    make_primary(bind);
    code.TextChanged([this, bind](Inspectable const &sender, TextChangedEventArgs const &) {
      // 只记下输入、改按钮能不能点，不重画页面，输入框的焦点留着。
      qq_link_code_ = std::wstring(sender.as<TextBox>().Text().c_str());
      bind.IsEnabled(!qq_link_busy_ && am::eight_digits(am::qq_link_digits(qq_link_code_)));
    });
    code.KeyDown([this](Inspectable const &, Input::KeyRoutedEventArgs const &args) {
      if (args.Key() == Windows::System::VirtualKey::Enter) {
        args.Handled(true);
        redeem_qq_link();
      }
    });
    StackPanel field;
    field.Orientation(Orientation::Horizontal);
    field.Spacing(8);
    field.Children().Append(code);
    field.Children().Append(bind);
    add_row(body, 0, L"绑定码", L"在 QQ 群里 @机器人 发 /bind 拿到，10 分钟内有效", field);

    if (qq_links_)
      for (auto const &[id, created_at] : *qq_links_)
        add_row(body, 0, L"绑定于 " + qq_link_time(created_at), L"",
                button_control(L"解绑", [this, id = id] { remove_qq_link(id); }, !qq_link_busy_));
    if (qq_link_busy_)
      body.Children().Append(make_text(L"正在处理…", 12, palette_.sub));
    if (!qq_link_message_.empty())
      body.Children().Append(make_text(qq_link_message_, 12, palette_.text));
    A11y::SetName(body, L"绑定 QQ 群助手");
    group.Children().Append(body);
    mark_search_target(L"绑定 QQ 群助手", body);
  }

  // ---- 页面 ----

  InfoBar account_message(std::wstring const &message, InfoBarSeverity severity) {
    InfoBar bar;
    bar.IsOpen(true);
    bar.IsClosable(false);
    bar.Severity(severity);
    bar.Message(hstring(message));
    bar.Margin(Thickness{0, 0, 0, 12});
    return bar;
  }

  // 卡片底部的小链接（account.link）。
  HyperlinkButton account_link(std::wstring const &label, std::function<void()> click,
                               bool enabled = true) {
    HyperlinkButton link;
    link.Content(box_value(hstring(label)));
    link.FontSize(12);
    link.IsEnabled(enabled);
    link.Click([on_click = std::move(click)](Inspectable const &, RoutedEventArgs const &) {
      on_click();
    });
    return link;
  }

  void build_account_page(StackPanel const &page) {
    if (indexing_) {
      // 搜索索引不读账号状态：记下这一页在各种状态下的主要条目，结果都打开这一页。
      record(L"使用 Google 登录", L"");
      record(L"邮箱登录", L"我们会发送一个 6 位验证码完成登录");
      record(L"手机号登录", L"我们会发送一个 6 位验证码完成登录");
      record(L"编辑资料", L"社区昵称");
      record(L"设置同步", std::wstring(am::sync_description));
      record(L"绑定 QQ 群助手", std::wstring(am::qq_link_note));
      record(L"云剪贴板", L"");
      record(L"退出登录", L"");
      record(L"退出所有设备", L"");
      record(L"注销账号", L"");
      return;
    }
    // 「我的内容」打开的社区词库与回复模板换下整页。
    if (resource_view_) {
      build_resource_community(page);
      return;
    }
    if (account_stale_)
      load_account();
    account_resend_button_ = nullptr;
    account_login_button_ = nullptr;
    account_code_panel_ = nullptr;
    if (account_loading_) {
      auto status = make_text(L"正在读取账号状态…", 14, palette_.sub);
      status.Margin(Thickness{4, 8, 4, 0});
      page.Children().Append(status);
      return;
    }
    if (!account_error_.empty())
      page.Children().Append(account_message(account_error_, InfoBarSeverity::Error));
    if (!account_notice_.empty())
      page.Children().Append(account_message(account_notice_, InfoBarSeverity::Success));
    if (account_user_) {
      load_account_avatar();
      build_account_profile_card(page);
      build_settings_sync(page);
      build_qq_link_card(page);
      build_account_cloud(page);
      build_account_content(page);
      build_account_actions(page);
    } else {
      build_account_sign_in(page);
    }
    auto footnote = make_text(L"皮肤设计和打字统计保存在本机。只有你主动发布的作品会分享至社区；"
                              L"账号登录不会自动上传本地设计或输入记录。",
                              12, palette_.faint);
    footnote.Margin(Thickness{4, 24, 4, 0});
    page.Children().Append(footnote);
  }

  // 账号介绍与登录方式分别成组，沿用设置页的标签、说明和操作两列布局。
  void build_account_sign_in(StackPanel const &page) {
    auto identity = add_group(page, L"");
    Grid intro;
    intro.ColumnSpacing(14);
    intro.Padding(Thickness{0, 8, 0, 8});
    ColumnDefinition avatar_column;
    avatar_column.Width(GridLength{1, GridUnitType::Auto});
    ColumnDefinition text_column;
    text_column.Width(GridLength{1, GridUnitType::Star});
    intro.ColumnDefinitions().Append(avatar_column);
    intro.ColumnDefinitions().Append(text_column);
    intro.Children().Append(make_account_avatar(52, 22, L"杉"));
    StackPanel texts;
    texts.Spacing(3);
    texts.VerticalAlignment(VerticalAlignment::Center);
    auto heading = make_text(L"水杉账号", 17, palette_.text);
    heading.FontWeight(Windows::UI::Text::FontWeight{600});
    texts.Children().Append(heading);
    texts.Children().Append(make_text(
        L"在设备之间同步设置和词库，分享候选窗口皮肤。", 11, palette_.sub));
    Grid::SetColumn(texts, 1);
    intro.Children().Append(texts);
    identity.Children().Append(intro);

    const bool email = account_channel_ == AccountChannel::email;
    auto group = add_group(page, account_channel_ == AccountChannel::none
                                    ? L"登录" : email ? L"邮箱登录" : L"手机号登录");
    if (account_channel_ == AccountChannel::none) {
      if (account_google_)
        add_row(group, 0, L"Google",
                account_google_waiting_ ? L"正在等待浏览器完成登录…" : L"使用已有的 Google 账号",
                button_control(L"使用 Google 登录", [this] { sign_in_with_google(); },
                               !account_busy_));
      if (account_google_waiting_)
        add_row(group, 0, L"取消浏览器登录", L"可以改用邮箱或手机号",
                account_link(L"取消", [this] { cancel_account_google(); }));
      // 选择验证码方式时仍结束等待中的 Google 登录。
      if (account_email_)
        add_row(group, 0, L"邮箱", L"通过邮箱接收 6 位验证码",
                button_control(L"邮箱登录",
                               [this] { choose_account_channel(AccountChannel::email); }));
      if (account_phone_)
        add_row(group, 0, L"手机号", L"通过短信接收 6 位验证码",
                button_control(L"手机号登录",
                               [this] { choose_account_channel(AccountChannel::phone); }));
      if (!account_email_ && !account_phone_ && !account_google_)
        add_row(group, 0, L"暂无可用登录方式", L"请稍后刷新重试。", nullptr);
    } else {
      build_account_code_form(group, email);
    }
    mark_search_target(L"使用 Google 登录", group);
    mark_search_target(L"邮箱登录", group);
    mark_search_target(L"手机号登录", group);

    // 两个恢复动作只在出了问题时出现，与共享页相同：登录方式只在进入本页时读一次，读失败或一个都没有时（上面那行「暂无可用登录方式」）才给「刷新」；读不出本机登录状态（status 失败）时才给「清除失效登录状态」。
    const bool no_providers = !account_email_ && !account_phone_ && !account_google_;
    if (account_channel_ == AccountChannel::none && (no_providers || account_session_unreadable_)) {
      auto actions = add_group(page, L"登录状态");
      if (no_providers)
        add_row(actions, 0, L"登录方式", L"重新获取当前可用的登录服务",
                button_control(L"刷新", [this] { refresh_account_providers(); }, !account_busy_));
      if (account_session_unreadable_)
        add_row(actions, 0, L"清除失效登录状态", L"无法正常登录时可清除本机失效状态后重试",
                button_control(L"清除", [this] {
                  end_account_session(account_action(L"forget"), L"已清除失效登录状态。");
                }, !account_busy_));
    }
  }

  void build_account_code_form(StackPanel const &body, bool email) {
    StackPanel form;
    form.Spacing(8);
    form.Padding(Thickness{0, 6, 0, 6});
    const auto label = email ? L"邮箱地址" : L"手机号（含国家区号）";
    TextBox target;
    target.Width(240);
    target.FontSize(13);
    target.MaxLength(320);
    target.Text(hstring(account_target_));
    target.IsEnabled(!account_busy_);
    A11y::SetName(target, hstring(label));
    {
      Input::InputScope scope;
      Input::InputScopeName name;
      name.NameValue(email ? Input::InputScopeNameValue::EmailSmtpAddress
                           : Input::InputScopeNameValue::TelephoneNumber);
      scope.Names().Append(name);
      target.InputScope(scope);
    }
    target.TextChanged([this](Inspectable const &sender, TextChangedEventArgs const &) {
      // TextChanged 是异步事件，重画页面时给框填回的号码也会晚一步报上来；号码没变就不是用户改的，不能把刚发的验证码丢掉。
      std::wstring typed(sender.as<TextBox>().Text().c_str());
      if (typed == account_target_)
        return;
      account_target_ = std::move(typed);
      // 共享页改号码就丢掉已发的验证码：验证码一栏收起，不重画页面，输入框的焦点留着。
      if (account_challenge_) {
        account_challenge_.reset();
        account_code_.clear();
        if (account_code_panel_)
          account_code_panel_.Visibility(Visibility::Collapsed);
      }
      update_account_code_controls();
    });
    add_row(form, 0, label, L"我们会发送一个 6 位验证码完成登录", target);

    auto resend = button_control(L"获取验证码", [this] { request_account_code(); }).as<Button>();
    resend.HorizontalAlignment(HorizontalAlignment::Right);
    if (!account_challenge_)
      make_primary(resend);
    form.Children().Append(resend);
    account_resend_button_ = resend;

    StackPanel code_panel;
    code_panel.Spacing(12);
    code_panel.Visibility(account_challenge_ ? Visibility::Visible : Visibility::Collapsed);
    TextBox code;
    code.Width(160);
    code.FontSize(13);
    code.MaxLength(6);
    code.Text(hstring(account_code_));
    code.IsEnabled(!account_busy_);
    A11y::SetName(code, L"6 位验证码");
    {
      Input::InputScope scope;
      Input::InputScopeName name;
      name.NameValue(Input::InputScopeNameValue::Number);
      scope.Names().Append(name);
      code.InputScope(scope);
    }
    code.TextChanged([this](Inspectable const &sender, TextChangedEventArgs const &) {
      auto box = sender.as<TextBox>();
      const std::wstring typed(box.Text().c_str());
      const auto digits = am::code_digits(typed);
      account_code_ = digits;
      if (digits != typed) {
        box.Text(hstring(digits));
        box.SelectionStart(static_cast<int32_t>(digits.size()));
      }
      update_account_code_controls();
    });
    add_row(code_panel, 0, L"验证码", L"输入本次收到的 6 位数字", code);
    auto login = button_control(L"登录", [this] { sign_in_with_code(); }).as<Button>();
    login.HorizontalAlignment(HorizontalAlignment::Right);
    make_primary(login);
    code_panel.Children().Append(login);
    form.Children().Append(code_panel);
    account_login_button_ = login;
    account_code_panel_ = code_panel;

    auto hint = make_text(L"验证码只用于本次登录，请勿向他人透露。", 12, palette_.sub);
    hint.Margin(Thickness{0, 4, 0, 0});
    form.Children().Append(hint);
    auto cancel = account_link(L"取消", [this] {
      account_channel_ = AccountChannel::none;
      redraw_account();
    }, !account_busy_);
    cancel.HorizontalAlignment(HorizontalAlignment::Left);
    form.Children().Append(cancel);
    body.Children().Append(form);
    update_account_code_controls();
    if (account_challenge_ || account_resend_at_ > account_now_ms())
      start_account_timer();
  }

  // 资料卡片：头像、名字和邮箱（没有时说已登录），点开「编辑资料」。
  void build_account_profile_card(StackPanel const &page) {
    auto group = add_group(page, L"");
    const auto &user = *account_user_;
    Grid content;
    content.ColumnSpacing(16);
    ColumnDefinition avatar_column;
    avatar_column.Width(GridLength{1, GridUnitType::Auto});
    ColumnDefinition text_column;
    text_column.Width(GridLength{1, GridUnitType::Star});
    ColumnDefinition chevron_column;
    chevron_column.Width(GridLength{1, GridUnitType::Auto});
    content.ColumnDefinitions().Append(avatar_column);
    content.ColumnDefinitions().Append(text_column);
    content.ColumnDefinitions().Append(chevron_column);
    const auto name = preferred_account_name(user.display_name, user.id);
    auto avatar = make_account_avatar(52, 22, name);
    Grid::SetColumn(avatar, 0);
    content.Children().Append(avatar);
    StackPanel texts;
    texts.Spacing(2);
    texts.VerticalAlignment(VerticalAlignment::Center);
    auto heading = make_text(name, 17, palette_.text);
    heading.FontWeight(Windows::UI::Text::FontWeight{600});
    texts.Children().Append(heading);
    texts.Children().Append(
        make_text(user.email.empty() ? L"水杉账号已登录" : user.email, 13, palette_.sub));
    Grid::SetColumn(texts, 1);
    content.Children().Append(texts);
    auto chevron = make_icon(0xE76C, 14, palette_.sub);
    chevron.VerticalAlignment(VerticalAlignment::Center);
    Grid::SetColumn(chevron, 2);
    content.Children().Append(chevron);

    Button card;
    card.Content(content);
    card.HorizontalAlignment(HorizontalAlignment::Stretch);
    card.HorizontalContentAlignment(HorizontalAlignment::Stretch);
    card.Padding(Thickness{0, 8, 0, 8});
    card.Background(brush(transparent_color()));
    card.BorderThickness(Thickness{0, 0, 0, 0});
    card.CornerRadius(CornerRadius{6, 6, 6, 6});
    card.IsEnabled(!account_busy_);
    A11y::SetName(card, L"编辑个人资料");
    card.Click([this](Inspectable const &, RoutedEventArgs const &) { edit_account_profile(); });
    group.Children().Append(card);
    mark_search_target(L"编辑资料", card);
  }

  // 「编辑资料」对话框：头像（点击更换）、名字预览和「移除头像」，昵称输入框，账号信息；「保存修改」保存昵称后关上，忙的时候关不上。对话框开着时页面的状态变了（头像换了、资料读到了、「已复制」）由 account_dialog_refresh_ 就地更新。
  fire_and_forget edit_account_profile() {
    if (!account_user_ || account_busy_ || account_dialog_refresh_)
      co_return;
    auto strong = get_strong();
    account_name_ = account_user_->display_name;

    StackPanel form;
    form.Spacing(12);
    form.MinWidth(400);
    Grid preview;
    preview.ColumnSpacing(12);
    ColumnDefinition avatar_column;
    avatar_column.Width(GridLength{1, GridUnitType::Auto});
    ColumnDefinition name_column;
    name_column.Width(GridLength{1, GridUnitType::Star});
    ColumnDefinition remove_column;
    remove_column.Width(GridLength{1, GridUnitType::Auto});
    preview.ColumnDefinitions().Append(avatar_column);
    preview.ColumnDefinitions().Append(name_column);
    preview.ColumnDefinitions().Append(remove_column);
    Button avatar_button;
    avatar_button.Padding(Thickness{0, 0, 0, 0});
    avatar_button.BorderThickness(Thickness{0, 0, 0, 0});
    avatar_button.Background(brush(transparent_color()));
    avatar_button.CornerRadius(CornerRadius{23, 23, 23, 23});
    A11y::SetName(avatar_button, L"更换头像");
    ToolTipService::SetToolTip(avatar_button, box_value(L"更换头像"));
    avatar_button.Click(
        [this](Inspectable const &, RoutedEventArgs const &) { choose_account_avatar(); });
    Grid::SetColumn(avatar_button, 0);
    preview.Children().Append(avatar_button);
    auto preview_name = make_text(L"", 18, palette_.text);
    preview_name.FontWeight(Windows::UI::Text::FontWeight{600});
    preview_name.VerticalAlignment(VerticalAlignment::Center);
    Grid::SetColumn(preview_name, 1);
    preview.Children().Append(preview_name);
    auto remove = account_link(L"移除头像", [this] { remove_account_avatar(); });
    remove.VerticalAlignment(VerticalAlignment::Center);
    Grid::SetColumn(remove, 2);
    preview.Children().Append(remove);
    form.Children().Append(preview);
    form.Children().Append(make_text(L"点头像可更换，支持 1 MiB 以内的 PNG 或 JPEG。", 12, palette_.sub));

    TextBox nickname;
    nickname.Header(box_value(L"社区昵称"));
    if (const auto name = account_name_presentation(account_name_))
      nickname.MaxLength(name->maximum_utf16_units);
    nickname.Text(hstring(account_name_));
    A11y::SetName(nickname, L"编辑社区昵称");
    form.Children().Append(nickname);
    form.Children().Append(
        make_text(L"昵称会显示在社区作品中，已发布的作品也会同步更新。", 12, palette_.sub));

    // AccountIdentityDetails：账号 ID（点按复制）、邮箱、登录方式和加入日期。
    Grid details;
    details.ColumnSpacing(24);
    details.RowSpacing(6);
    ColumnDefinition label_column;
    label_column.Width(GridLength{1, GridUnitType::Auto});
    ColumnDefinition value_column;
    value_column.Width(GridLength{1, GridUnitType::Star});
    details.ColumnDefinitions().Append(label_column);
    details.ColumnDefinitions().Append(value_column);
    int32_t row = 0;
    auto add_detail = [&](wchar_t const *label, FrameworkElement const &value) -> TextBlock {
      RowDefinition definition;
      definition.Height(GridLength{1, GridUnitType::Auto});
      details.RowDefinitions().Append(definition);
      auto title = make_text(label, 13, palette_.sub);
      title.VerticalAlignment(VerticalAlignment::Center);
      Grid::SetRow(title, row);
      Grid::SetColumn(title, 0);
      details.Children().Append(title);
      value.VerticalAlignment(VerticalAlignment::Center);
      Grid::SetRow(value, row);
      Grid::SetColumn(value, 1);
      details.Children().Append(value);
      ++row;
      return title;
    };
    auto copy = account_link(L"", [this] { copy_account_id(); });
    copy.FontFamily(Media::FontFamily(L"Consolas"));
    add_detail(L"账号 ID", copy);
    auto email = make_text(L"", 13, palette_.text);
    email.IsTextSelectionEnabled(true);
    auto email_label = add_detail(L"邮箱", email);
    auto providers = make_text(L"", 13, palette_.text);
    add_detail(L"登录方式", providers);
    auto joined = make_text(L"", 13, palette_.text);
    add_detail(L"加入水杉", joined);
    form.Children().Append(details);

    ContentDialog dialog;
    dialog.Title(box_value(L"编辑资料"));
    dialog.Content(form);
    dialog.PrimaryButtonText(L"保存修改");
    dialog.CloseButtonText(L"取消");
    dialog.DefaultButton(ContentDialogButton::Primary);
    dialog.XamlRoot(root_.XamlRoot());
    dialog.RequestedTheme(palette_.dark ? ElementTheme::Dark : ElementTheme::Light);
    // 与共享对话框一样，忙的时候「取消」和 Esc 都关不上。
    dialog.Closing([this](ContentDialog const &, ContentDialogClosingEventArgs const &args) {
      if (account_busy_)
        args.Cancel(true);
    });

    account_dialog_refresh_ = [this, weak_dialog = make_weak(dialog),
                               weak_avatar = make_weak(avatar_button),
                               weak_name = make_weak(preview_name), weak_remove = make_weak(remove),
                               weak_nickname = make_weak(nickname), weak_copy = make_weak(copy),
                               weak_email = make_weak(email), weak_email_label = make_weak(email_label),
                               weak_providers = make_weak(providers),
                               weak_joined = make_weak(joined)] {
      if (!account_user_) {
        // 对话框开着时会话结束了（别处退出或会话失效），没有可编辑的资料。
        if (auto shown = weak_dialog.get())
          shown.Hide();
        return;
      }
      const auto &user = *account_user_;
      const auto name = account_name_presentation(account_name_);
      const auto typed = name ? name->normalized : account_name_;
      if (auto button = weak_avatar.get()) {
        button.Content(make_account_avatar(46, 19, preferred_account_name(user.display_name, user.id)));
        button.IsEnabled(!account_busy_);
      }
      if (auto label = weak_name.get())
        label.Text(hstring(typed.empty() ? L"你的昵称" : typed));
      if (auto link = weak_remove.get()) {
        link.Visibility(user.avatar_uploaded ? Visibility::Visible : Visibility::Collapsed);
        link.IsEnabled(!account_busy_);
      }
      if (auto box = weak_nickname.get())
        box.IsEnabled(!account_busy_);
      if (auto link = weak_copy.get())
        link.Content(box_value(hstring(account_id_copied_ ? L"已复制" : am::id_label(user.id))));
      const auto shown_email = user.email.empty() ? Visibility::Collapsed : Visibility::Visible;
      if (auto value = weak_email.get()) {
        value.Text(hstring(user.email));
        value.Visibility(shown_email);
      }
      if (auto label = weak_email_label.get())
        label.Visibility(shown_email);
      if (auto value = weak_providers.get()) {
        std::wstring names;
        for (auto const &provider : account_providers_linked_)
          names += (names.empty() ? L"" : L"、") + am::provider_name(provider);
        value.Text(hstring(names.empty() ? L"正在读取" : names));
      }
      if (auto value = weak_joined.get())
        value.Text(hstring(account_join_date(user.created_at)));
      if (auto shown = weak_dialog.get())
        shown.IsPrimaryButtonEnabled(!account_busy_ && typed != user.display_name);
    };
    nickname.TextChanged([this](Inspectable const &sender, TextChangedEventArgs const &) {
      account_name_ = std::wstring(sender.as<TextBox>().Text().c_str());
      if (account_dialog_refresh_)
        account_dialog_refresh_();
    });
    account_dialog_refresh_();
    const bool save = co_await show_dialog(dialog);
    account_dialog_refresh_ = nullptr;
    if (save && account_user_)
      rename_account();
  }

  // 「云端」：云剪贴板是输入工具的面板，仍由共享应用打开（本窗口只剩这几处会拉起共享应用，另外是「词库」页的云词库和屏幕键盘、语音输入和手写的面板）。云词库与共享页一样只在「词库」页给入口。
  void build_account_cloud(StackPanel const &page) {
    auto group = add_group(page, L"云端");
    add_row(group, 0xE77F, L"云剪贴板", L"",
            button_control(L"打开云剪贴板",
                           [this] { open_shell(nav::shell_links::cloud_clipboard); },
                           !account_busy_));
  }

  // 「我的内容」（共享页桌面版的 MeRow）：Windows 的社区只有词库与回复模板（没有皮肤画廊，「我发布的皮肤」不在这里）。每一行在本页打开社区词库与回复模板的图库，停在那一行的分类和范围上（communityDestinationView）。
  void build_account_content(StackPanel const &page) {
    auto group = add_group(page, L"我的内容");
    constexpr std::array<wchar_t, 4> glyphs{0xE82D, 0xE8BD, 0xE734, 0xEB51};
    for (size_t i = 0; i < cm::resource_destinations.size(); ++i) {
      auto const &destination = cm::resource_destinations[i];
      auto row = plugin_nav_row(group, glyphs[i], std::wstring(destination.title), L"", L"",
                                [this, kind = std::wstring(destination.kind),
                                 scope = std::wstring(destination.scope)] {
                                  open_resource_community(kind, scope);
                                });
      row.IsEnabled(!account_busy_);
    }
  }

  // ---- 社区词库与回复模板 ----

  // 共享应用在 Windows 上的社区页（community-page.tsx 只有 resources 时的 CommunityHomePage）：「词库」「回复模板」两个分类，各是一个图库（community-resources.tsx 的 CommunityResourcesPage）——搜索、全部 / 收藏 / 我的作品、卡片、加载更多；详情里看词条或提示词，导入本机词库或账号云词库，收藏、评分、举报，作者发布新版本和下架；「发布作品」是对话框（ResourceEditor）。本窗口没有单独的社区页，「我的内容」的几行在「账号与同步」页里换上这个图库，顶上的「账号与同步」回到本页；离开本页时卸下，下次从头读起。社区操作经 msime_client_community，导入本机词库经 msime_client_settings_dictionary（与「词库」页相同），都在工作线程上。

  void open_resource_community(std::wstring const &kind, std::wstring const &scope) {
    if (account_busy_)
      return;
    resource_view_ = true;
    resource_kind_ = kind;
    resource_scope_ = scope;
    reset_gallery<CommunityResourceItem>();
    gallery_list<CommunityResourceItem>(L"", false, false);
    render_page(false);
  }

  void leave_resource_community() {
    ++resource_gallery_.owner;
    resource_gallery_.action_busy = false;
    resource_view_ = false;
    ++resource_editor_.generation;
    resource_editor_.busy = false;
    if (auto shown = resource_editor_.dialog.get())
      shown.Hide();
  }

  void close_resource_community() {
    if (resource_gallery_.action_busy)
      return;
    leave_resource_community();
    render_page(false);
  }

  // 换分类时范围和搜索词照旧，从第一页读（共享页的图库换了 kind 不重新挂上）。
  void set_resource_kind(std::wstring const &kind) {
    if (kind == resource_kind_)
      return;
    resource_kind_ = kind;
    gallery_list<CommunityResourceItem>(resource_gallery_.active_search, false, false);
  }

  void build_resource_community(StackPanel const &page) {
    auto &gallery = resource_gallery_;
    // 回到「账号与同步」，样子同插件页视图的返回。
    StackPanel label;
    label.Orientation(Orientation::Horizontal);
    label.Spacing(6);
    auto chevron = make_icon(0xE76B, 12, palette_.text);
    chevron.VerticalAlignment(VerticalAlignment::Center);
    label.Children().Append(chevron);
    label.Children().Append(make_text(page_label("account").title, 14, palette_.text));
    Button back;
    back.Content(label);
    back.Height(32);
    back.Padding(Thickness{12, 0, 12, 0});
    back.IsEnabled(!gallery.action_busy);
    A11y::SetName(back, hstring(std::wstring(L"返回") + page_label("account").title));
    back.Click([this](Inspectable const &, RoutedEventArgs const &) { close_resource_community(); });
    back.Margin(Thickness{0, 0, 0, 12});
    page.Children().Append(back);
    std::vector<Option> kinds;
    for (auto const &kind : cm::resource_kinds)
      kinds.push_back({std::wstring(kind.value), std::wstring(kind.label)});
    auto tabs = segmented_control(L"社区分类", std::move(kinds), resource_kind_,
                                  [this](std::wstring const &value) { set_resource_kind(value); },
                                  true, false);
    tabs.HorizontalAlignment(HorizontalAlignment::Left);
    tabs.Margin(Thickness{0, 0, 0, 12});
    page.Children().Append(tabs);
    if (gallery.selected) {
      build_resource_detail(page);
      return;
    }
    const auto title = cm::resource_kind_title(resource_kind_);
    community_search<CommunityResourceItem>(page, L"搜索" + title);
    community_heading<CommunityResourceItem>(
        page, cm::resource_heading(resource_kind_, resource_scope_),
        cm::resource_heading_note(resource_kind_), title + L"范围", L"全部", L"发布作品",
        [this] { open_resource_editor(false); });
    if (!gallery.error.empty())
      community_error(page, gallery.error, false, nullptr);
    else if (!gallery.list_busy && gallery.items.empty())
      community_empty(page, L"这里还没有" + title + L"作品。");
    constexpr double width = 232, height = 156;
    auto grid = community_grid(width, height);
    for (auto const &item : gallery.items) {
      StackPanel body;
      body.Spacing(4);
      Border tile;
      tile.Width(32);
      tile.Height(32);
      tile.CornerRadius(CornerRadius{6, 6, 6, 6});
      tile.Background(brush(palette_.accent));
      auto glyph = make_text(cm::resource_card_glyph(item.kind), 15, palette_.on_accent);
      glyph.HorizontalAlignment(HorizontalAlignment::Center);
      glyph.VerticalAlignment(VerticalAlignment::Center);
      tile.Child(glyph);
      tile.HorizontalAlignment(HorizontalAlignment::Left);
      body.Children().Append(tile);
      auto name = card_line(item.name, 14, palette_.text);
      name.FontWeight(Windows::UI::Text::FontWeight{600});
      body.Children().Append(name);
      body.Children().Append(card_line(
          cm::card_author(L"", item.author, item.owned, false, item.moderation == L"removed"), 12,
          palette_.sub));
      body.Children().Append(
          card_line(cm::resource_card_description(item.kind, item.description), 12, palette_.sub, 2));
      body.Children().Append(card_line(
          cm::resource_card_metrics(item.saves, item.rating_count, item.rating_average), 12,
          palette_.sub));
      grid.Children().Append(community_card(L"查看" + cm::resource_kind_title(item.kind) + L" " +
                                                item.name,
                                            body, width, height,
                                            [this, item] { gallery_open(item); }));
    }
    page.Children().Append(grid);
    community_load_more<CommunityResourceItem>(page, L"正在读取社区…");
  }

  // ResourceDetail：作者和版本，收藏与评分；词库是词条预览和两种导入，回复模板是提示词预览；之后是收藏、作者的「编辑并发布新版本」、评分或下架，别人的作品可以举报。
  void build_resource_detail(StackPanel const &page) {
    auto &gallery = resource_gallery_;
    auto const item = *gallery.selected;
    const bool busy = gallery.action_busy;
    auto body = community_detail_frame<CommunityResourceItem>(page);
    community_detail_header(body, item.name, item.author + L" · v" + std::to_wstring(item.revision),
                            item.owned, item.moderation, L"我的作品", item.description);
    body.Children().Append(make_text(
        cm::rating_metrics(item.saves, L"收藏", item.rating_count, item.rating_average), 13,
        palette_.sub));
    auto subheading = [this, &body](std::wstring const &value) {
      auto heading = make_text(value, 15, palette_.text);
      heading.FontWeight(Windows::UI::Text::FontWeight{600});
      heading.Margin(Thickness{0, 6, 0, 0});
      body.Children().Append(heading);
    };
    auto preview_box = [this](UIElement const &content) {
      ScrollViewer scroller;
      scroller.MaxHeight(260);
      scroller.Content(content);
      Border frame;
      frame.BorderBrush(brush(palette_.stroke));
      frame.BorderThickness(Thickness{1, 1, 1, 1});
      frame.CornerRadius(CornerRadius{4, 4, 4, 4});
      frame.Padding(Thickness{12, 8, 12, 8});
      frame.Child(scroller);
      return frame;
    };
    auto left = [](FrameworkElement const &element) {
      element.HorizontalAlignment(HorizontalAlignment::Left);
      return element;
    };
    if (item.kind == L"dictionary") {
      subheading(L"词条预览 · " + std::to_wstring(item.entries.size()) + L" 条");
      StackPanel words;
      words.Spacing(4);
      for (auto const &entry : item.entries) {
        Grid line;
        line.ColumnSpacing(16);
        for (auto width : {GridLength{1, GridUnitType::Star}, GridLength{1, GridUnitType::Auto}}) {
          ColumnDefinition column;
          column.Width(width);
          line.ColumnDefinitions().Append(column);
        }
        line.Children().Append(make_text(entry.word, 13, palette_.text));
        auto code = make_text(entry.code, 13, palette_.sub);
        code.FontFamily(Media::FontFamily(L"Consolas"));
        Grid::SetColumn(code, 1);
        line.Children().Append(code);
        words.Children().Append(line);
      }
      A11y::SetName(words, L"词条预览");
      body.Children().Append(preview_box(words));
      body.Children().Append(
          left(primary_button(L"导入这版词库到本机", [this] { import_resource_locally(); }, !busy)));
      body.Children().Append(
          left(button_control(L"导入这版词库到云端", [this] { apply_resource(); }, !busy)));
      body.Children().Append(make_text(
          L"本机导入只更新当前设备；云端导入会合并到账号云词库。版本发生变化时云端导入会停止并要求重新查看。",
          12, palette_.sub));
    } else {
      subheading(L"提示词预览");
      auto prompt = make_text(item.prompt, 13, palette_.text);
      prompt.IsTextSelectionEnabled(true);
      prompt.FontFamily(Media::FontFamily(L"Consolas"));
      A11y::SetName(prompt, L"提示词预览");
      body.Children().Append(preview_box(prompt));
    }
    if (!gallery.notice.empty())
      community_notice(body, gallery.notice);
    body.Children().Append(left(button_control(item.saved ? L"取消收藏" : L"收藏，关注后续更新",
                                               [this] { save_resource(); }, !busy)));
    if (item.owned)
      body.Children().Append(left(
          button_control(L"编辑并发布新版本", [this] { open_resource_editor(true); }, !busy)));
    community_moderation<CommunityResourceItem>(
        body, true, L"下架作品", L"确认下架作品",
        L"下架后其他用户无法获取此作品，已有本地回复模板和云词库副本不会被删除。确定下架“" +
            item.name + L"”吗？",
        L"");
    if (!item.owned)
      community_report<CommunityResourceItem>(body);
  }

  // save：收藏或取消收藏，再重读详情。
  void save_resource() {
    if (!gallery_begin<CommunityResourceItem>(true))
      return;
    auto const &item = *resource_gallery_.selected;
    const auto id = item.id;
    const bool saved = !item.saved;
    gallery_call<CommunityResourceItem>(
        [id, saved] {
          auto save = community_action(L"resource_save");
          save.SetNamedValue(L"id", JsonValue::CreateStringValue(hstring(id)));
          save.SetNamedValue(L"saved", JsonValue::CreateBooleanValue(saved));
          auto done = community_call(save);
          if (!done.ok)
            return done;
          auto detail = community_action(L"resource_detail");
          detail.SetNamedValue(L"id", JsonValue::CreateStringValue(hstring(id)));
          return community_call(detail);
        },
        [this](Response const &response) {
          auto item = community_item<CommunityResourceItem>(response);
          if (!item) {
            gallery_end<CommunityResourceItem>(community_failure(response));
            return;
          }
          resource_gallery_.selected = std::move(*item);
          gallery_end<CommunityResourceItem>();
        });
  }

  // apply：把这一版词库导入账号的云端词库，说明新增或更新了几条。
  void apply_resource() {
    if (!gallery_begin<CommunityResourceItem>(true))
      return;
    auto const &item = *resource_gallery_.selected;
    auto action = community_action(L"resource_apply");
    action.SetNamedValue(L"id", JsonValue::CreateStringValue(hstring(item.id)));
    action.SetNamedValue(L"resource_revision",
                         JsonValue::CreateNumberValue(static_cast<double>(item.revision)));
    gallery_call<CommunityResourceItem>(
        [action] { return community_call(action); },
        [this](Response const &response) {
          const auto value = community_value(response);
          if (!response.ok || value.ValueType() != JsonValueType::Object) {
            gallery_end<CommunityResourceItem>(community_failure(response));
            return;
          }
          resource_gallery_.notice = L"已导入云端词库，新增或更新 " +
                                     std::to_wstring(named_count(value.GetObject(), L"imported")) +
                                     L" 个词条。";
          gallery_end<CommunityResourceItem>();
        });
  }

  // applyLocal（宿主没有命名词库，Windows 就是这样）：词条按类型分组，每组一次标准格式的导入，与共享应用的 dictionary_request 相同；说明本机词库应用了几条。读不到宿主选项时没有词库可写，按存储失败说明。
  void import_resource_locally() {
    if (!gallery_begin<CommunityResourceItem>(true))
      return;
    const auto options = runtime_options_document();
    if (!options) {
      gallery_end<CommunityResourceItem>(CommunityFailure{L"storage", L""});
      return;
    }
    struct Outcome {
      bool ok = true;
      int64_t applied = 0;
      CommunityFailure failure;
    };
    const auto groups = cm::local_import_groups(resource_gallery_.selected->entries);
    const auto stamp = request_id(L"community-local-");
    const auto owner = resource_gallery_.owner;
    run_worker<Outcome>(
        [options = *options, groups, stamp] {
          Outcome outcome;
          for (auto const &group : groups) {
            // 请求 id 是 community-local-<毫秒>-<词条原来的类型>，与共享页相同。
            JsonObject action;
            action.SetNamedValue(L"operation", JsonValue::CreateStringValue(L"import"));
            action.SetNamedValue(L"kind", JsonValue::CreateStringValue(hstring(group.kind)));
            action.SetNamedValue(L"format", JsonValue::CreateStringValue(L"standard"));
            action.SetNamedValue(L"text", JsonValue::CreateStringValue(hstring(group.text)));
            action.SetNamedValue(L"request_id",
                                 JsonValue::CreateStringValue(hstring(stamp + L"-" + group.source)));
            const auto response = settings_dictionary_call(options, utf8(action.Stringify()));
            const auto value = community_value(response);
            if (!response.ok || value.ValueType() != JsonValueType::Object) {
              outcome.ok = false;
              outcome.failure = CommunityFailure{response_error(response), L""};
              return outcome;
            }
            outcome.applied += named_count(value.GetObject(), L"applied");
          }
          return outcome;
        },
        [this, owner](Outcome &outcome) {
          if (owner != resource_gallery_.owner)
            return;
          if (!outcome.ok) {
            gallery_end<CommunityResourceItem>(outcome.failure);
            return;
          }
          resource_gallery_.notice =
              L"已导入本机词库，应用 " + std::to_wstring(outcome.applied) + L" 个词条。";
          gallery_end<CommunityResourceItem>();
        });
  }

  // ---- 发布词库与回复模板 ----

  // ResourceEditor 的状态。id 在打开时定下（新作品随机生成，新版本沿用原来的），改名称或说明不换；generation 在打开和关上时递增，作废在途的回答。
  struct ResourceEditor {
    uint64_t generation = 0;
    std::wstring kind;
    bool existing = false;
    int64_t revision = 0;
    std::wstring id;
    std::wstring name, description, prompt;
    std::vector<cm::SharedWord> entries;
    std::wstring entry_kind = L"pinyin";
    std::wstring code, word, weight = std::wstring(cm::default_shared_word_weight);
    bool agreed = false;
    // publication_fields 改名称时会换的发布 id；这里的 id 不随它变，换的是这一份。
    std::wstring field_id;
    std::wstring error;
    bool busy = false;
    std::function<void()> refresh;
    weak_ref<ContentDialog> dialog;
  };

  // 打开「发布词库」「发布回复模板」，或作者在详情里的「编辑并发布新版本」（existing）：后者带上这一版的内容。
  fire_and_forget open_resource_editor(bool existing) {
    if (dialog_open_ || resource_editor_.refresh)
      co_return;
    if (existing && !resource_gallery_.selected)
      co_return;
    auto strong = get_strong();
    const auto generation = resource_editor_.generation + 1;
    resource_editor_ = ResourceEditor{};
    auto &state = resource_editor_;
    state.generation = generation;
    state.existing = existing;
    if (existing) {
      auto const &item = *resource_gallery_.selected;
      state.kind = item.kind;
      state.id = item.id;
      state.revision = item.revision;
      state.name = item.name;
      state.description = item.description;
      state.prompt = item.prompt;
      state.entries = item.entries;
    } else {
      state.kind = resource_kind_;
      state.id = random_uuid();
    }
    const auto title = existing ? std::wstring(L"更新作品") : L"发布" + cm::resource_kind_title(state.kind);

    ScrollViewer scroller;
    scroller.MaxHeight(560);
    StackPanel form;
    form.Spacing(12);
    form.MinWidth(460);
    form.Padding(Thickness{0, 0, 12, 0});
    scroller.Content(form);
    auto dialog = publish_dialog(title, [this] { return resource_editor_.busy; },
                                 [this] { submit_resource_editor(); });
    dialog.Content(scroller);
    A11y::SetName(dialog, hstring(existing ? std::wstring(L"更新社区作品") : title));
    const auto weak_dialog = make_weak(dialog);
    const auto weak_form = make_weak(form);
    state.dialog = weak_dialog;
    state.refresh = [this, weak_dialog, weak_form] {
      DispatcherQueue().TryEnqueue([this, weak_dialog, weak_form] {
        auto shown = weak_dialog.get();
        auto content = weak_form.get();
        if (shown && content && resource_editor_.refresh)
          build_resource_editor(content, shown);
      });
    };
    build_resource_editor(form, dialog);
    co_await show_dialog(dialog);
    ++resource_editor_.generation;
    resource_editor_.refresh = nullptr;
    resource_editor_.busy = false;
  }

  void build_resource_editor(StackPanel const &form, ContentDialog const &dialog) {
    auto &state = resource_editor_;
    form.Children().Clear();
    dialog_error(form, state.error, false, make_weak(dialog));
    publication_fields(form, state.name, state.description, state.agreed, state.field_id, state.busy,
                       L"作品名称", L"社区作品名称", L"作品说明", L"社区作品说明",
                       L"我拥有发布所用内容的权利，并同意其他用户查看和使用", L"确认拥有发布内容权利",
                       [] {}, [this] { submit_resource_editor(); }, !state.existing);
    if (state.kind == L"reply") {
      TextBox prompt;
      prompt.Header(box_value(L"回复提示词"));
      prompt.AcceptsReturn(true);
      prompt.TextWrapping(TextWrapping::Wrap);
      prompt.MinHeight(168);
      prompt.MaxLength(static_cast<int32_t>(cm::reply_prompt_limit));
      prompt.Text(hstring(state.prompt));
      prompt.IsEnabled(!state.busy);
      A11y::SetName(prompt, L"社区回复提示词");
      prompt.TextChanged([this](Inspectable const &sender, TextChangedEventArgs const &) {
        resource_editor_.prompt = cm::line_feeds(sender.as<TextBox>().Text().c_str());
      });
      form.Children().Append(prompt);
    } else {
      build_resource_entry_form(form);
    }
    form.Children().Append(make_text(cm::resource_publish_warning, 12, palette_.sub));
    dialog.PrimaryButtonText(state.busy ? L"正在发布…" : state.existing ? L"发布新版本" : L"公开发布");
    dialog.IsPrimaryButtonEnabled(!state.busy);
  }

  // 词库的词条表单：类型、编码、词语、权重和「添加词条」，下面是待发布的词条，每条可以移除。
  void build_resource_entry_form(StackPanel const &form) {
    auto &state = resource_editor_;
    VariableSizedWrapGrid fields;
    fields.Orientation(Orientation::Horizontal);
    fields.ItemWidth(150);
    std::vector<Option> kinds;
    for (auto const &kind : cm::shared_word_kinds)
      kinds.push_back({std::wstring(kind.value), std::wstring(kind.label)});
    auto kind = select_control(L"社区词条类型", std::move(kinds), state.entry_kind,
                               [this](std::wstring const &value) { resource_editor_.entry_kind = value; },
                               !state.busy, false)
                    .as<ComboBox>();
    kind.Header(box_value(L"类型"));
    kind.HorizontalAlignment(HorizontalAlignment::Stretch);
    kind.Margin(Thickness{0, 0, 8, 8});
    fields.Children().Append(kind);
    auto field = [this, &fields, &state](std::wstring const &label, std::wstring const &name,
                                         std::wstring ResourceEditor::*value) {
      TextBox box;
      box.Header(box_value(hstring(label)));
      box.Text(hstring(state.*value));
      box.IsEnabled(!state.busy);
      box.Margin(Thickness{0, 0, 8, 8});
      A11y::SetName(box, hstring(name));
      box.TextChanged([this, value](Inspectable const &sender, TextChangedEventArgs const &) {
        resource_editor_.*value = std::wstring(sender.as<TextBox>().Text().c_str());
      });
      fields.Children().Append(box);
    };
    field(L"编码", L"社区词条编码", &ResourceEditor::code);
    field(L"词语", L"社区词条文字", &ResourceEditor::word);
    field(L"权重", L"社区词条权重", &ResourceEditor::weight);
    form.Children().Append(fields);
    auto add = button_control(L"添加词条", [this] { add_resource_entry(); }, !state.busy);
    add.HorizontalAlignment(HorizontalAlignment::Left);
    form.Children().Append(add);
    StackPanel list;
    list.Spacing(4);
    for (size_t i = 0; i < state.entries.size(); ++i) {
      auto const &entry = state.entries[i];
      Grid line;
      line.ColumnSpacing(12);
      for (auto width : {GridLength{1, GridUnitType::Star}, GridLength{1, GridUnitType::Auto}}) {
        ColumnDefinition column;
        column.Width(width);
        line.ColumnDefinitions().Append(column);
      }
      auto words = make_text(entry.word + L" · " + entry.code + L" · " + std::to_wstring(entry.weight),
                             13, palette_.text);
      words.VerticalAlignment(VerticalAlignment::Center);
      line.Children().Append(words);
      auto remove = button_control(L"移除", [this, i] {
        auto &current = resource_editor_;
        if (current.busy || i >= current.entries.size())
          return;
        current.entries.erase(current.entries.begin() + static_cast<std::ptrdiff_t>(i));
        if (current.refresh)
          current.refresh();
      }, !state.busy);
      Grid::SetColumn(remove, 1);
      line.Children().Append(remove);
      list.Children().Append(line);
    }
    A11y::SetName(list, hstring(L"待发布词条 " + std::to_wstring(state.entries.size()) + L"/" +
                                std::to_wstring(cm::shared_word_limit)));
    form.Children().Append(list);
  }

  // addEntry：不合规则时说明一句；加上后清空编码和词语，类型和权重留着。
  void add_resource_entry() {
    auto &state = resource_editor_;
    if (state.busy)
      return;
    if (auto entry = cm::shared_word(state.entries, state.entry_kind, state.code, state.word,
                                     state.weight)) {
      state.entries.push_back(std::move(*entry));
      state.code.clear();
      state.word.clear();
      state.error.clear();
    } else {
      state.error = std::wstring(cm::shared_word_invalid);
    }
    if (state.refresh)
      state.refresh();
  }

  // submit：先照共享页校验，再发布；成功后关上对话框，新版本重读详情，新作品从第一页重读图库。
  void submit_resource_editor() {
    auto &state = resource_editor_;
    if (state.busy)
      return;
    if (auto error = cm::resource_publish_error(state.kind, state.name, state.description,
                                                state.prompt, state.entries.size(), state.existing,
                                                state.agreed);
        !error.empty()) {
      state.error = std::move(error);
      if (state.refresh)
        state.refresh();
      return;
    }
    const auto fields = cm::publish_fields(state.name, state.description);
    JsonObject content;
    if (state.kind == L"reply") {
      content.SetNamedValue(L"prompt", JsonValue::CreateStringValue(hstring(state.prompt)));
    } else {
      JsonArray entries;
      for (auto const &entry : state.entries) {
        JsonObject word;
        word.SetNamedValue(L"kind", JsonValue::CreateStringValue(hstring(entry.kind)));
        word.SetNamedValue(L"code", JsonValue::CreateStringValue(hstring(entry.code)));
        word.SetNamedValue(L"word", JsonValue::CreateStringValue(hstring(entry.word)));
        word.SetNamedValue(L"weight", JsonValue::CreateNumberValue(static_cast<double>(entry.weight)));
        entries.Append(word);
      }
      content.SetNamedValue(L"entries", entries);
    }
    auto action = community_action(L"resource_publish");
    action.SetNamedValue(L"id", JsonValue::CreateStringValue(hstring(state.id)));
    action.SetNamedValue(L"kind", JsonValue::CreateStringValue(hstring(state.kind)));
    action.SetNamedValue(L"name", JsonValue::CreateStringValue(hstring(fields.name)));
    action.SetNamedValue(L"description", JsonValue::CreateStringValue(hstring(fields.description)));
    action.SetNamedValue(L"content", content);
    action.SetNamedValue(L"revision",
                         JsonValue::CreateNumberValue(static_cast<double>(state.existing ? state.revision : 0)));
    state.busy = true;
    state.error.clear();
    const auto generation = state.generation;
    const bool existing = state.existing;
    const auto id = state.id;
    run_host([action] { return community_call(action); },
             [this, generation, existing, id](Response const &response) {
               auto &current = resource_editor_;
               if (generation != current.generation)
                 return;
               current.busy = false;
               if (!response.ok) {
                 current.error = cm::resource_message(community_failure(response).code);
                 if (current.refresh)
                   current.refresh();
                 return;
               }
               const auto dialog = current.dialog;
               ++current.generation;
               current.refresh = nullptr;
               if (auto shown = dialog.get())
                 shown.Hide();
               if (!resource_view_)
                 return;
               if (!existing) {
                 gallery_list<CommunityResourceItem>(resource_gallery_.active_search, false, false);
                 return;
               }
               auto detail = community_action(L"resource_detail");
               detail.SetNamedValue(L"id", JsonValue::CreateStringValue(hstring(id)));
               gallery_call<CommunityResourceItem>(
                   [detail] { return community_call(detail); },
                   [this, id](Response const &answer) {
                     auto &gallery = resource_gallery_;
                     if (!gallery.selected || gallery.selected->id != id)
                       return;
                     if (auto item = community_item<CommunityResourceItem>(answer))
                       gallery.selected = std::move(*item);
                     else
                       gallery_fail<CommunityResourceItem>(community_failure(answer));
                     redraw_account();
                   });
             });
    if (state.refresh)
      state.refresh();
  }

  // 「账号」：这台设备退出、所有设备退出（先确认）和注销账号（先确认）。
  void build_account_actions(StackPanel const &page) {
    auto group = add_group(page, L"账号");
    auto device = add_row(
        group, 0xE7E8, L"这台设备",
        L"退出后，设置同步、云词库、云剪贴板和发布作品都需要重新登录才能使用。",
        button_control(L"退出登录", [this] { sign_out_account(false); }, !account_busy_));
    mark_search_target(L"退出登录", device);
    auto everywhere = add_row(
        group, 0xE977, L"所有设备", L"所有已登录的设备都需要重新登录。",
        button_control(L"退出所有设备",
                       [this] { confirm_account_action(am::Confirmation::logout_all); },
                       !account_busy_));
    mark_search_target(L"退出所有设备", everywhere);
    auto remove = button_control(L"注销账号",
                                 [this] { confirm_account_action(am::Confirmation::remove); },
                                 !account_busy_);
    remove.as<Button>().Foreground(brush(palette_.dark ? rgba(0xFF, 0x99, 0xA4, 1)
                                                       : rgba(0xC4, 0x2B, 0x1C, 1)));
    add_row(group, 0xE74D, L"注销账号", L"删除账号及已发布的作品、评分等云端数据，无法撤销。",
            remove);
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
    // The custom card is what the custom theme is made of besides a skin package, as on the shared page: the candidate card as the shared layer resolves its base and candidate_colors, over the base theme's backdrop. Each package has its own card after the built-in ones. The catalog's own entry for it only shows the default custom colours.
    if (id == L"custom") {
      const auto candidate = candidate_theme(id, false);
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
    const auto candidate = candidate_theme(theme.id, false);
    Button card;
    card.HorizontalAlignment(HorizontalAlignment::Stretch);
    card.Padding(Thickness{10, 10, 10, 10});
    card.CornerRadius(CornerRadius{10, 10, 10, 10});
    card.Background(brush(palette_.card));
    card.BorderBrush(brush(on ? palette_.accent : palette_.stroke));
    card.BorderThickness(on ? Thickness{2, 2, 2, 2} : Thickness{1, 1, 1, 1});
    card.HorizontalContentAlignment(HorizontalAlignment::Stretch);

    StackPanel body;
    body.Spacing(8);
    Border surface;
    surface.Height(84);
    surface.CornerRadius(CornerRadius{8, 8, 8, 8});
    surface.Background(brush(preview.background));
    Border panel;
    panel.CornerRadius(CornerRadius{8, 8, 8, 8});
    panel.Padding(Thickness{4, 4, 4, 4});
    panel.Background(brush(to_color(candidate.surface)));
    panel.HorizontalAlignment(HorizontalAlignment::Center);
    panel.VerticalAlignment(VerticalAlignment::Center);
    StackPanel words;
    words.Spacing(2);
    Border selected;
    selected.Background(brush(to_color(candidate.selected)));
    selected.CornerRadius(CornerRadius{6, 6, 6, 6});
    selected.Padding(Thickness{8, 4, 8, 4});
    selected.Child(make_text(L"1  水杉输入法", 12, to_color(candidate.selected_text)));
    words.Children().Append(selected);
    auto alternate = make_text(L"2  水杉", 12, to_color(candidate.text));
    alternate.Margin(Thickness{8, 2, 8, 2});
    words.Children().Append(alternate);
    panel.Child(words);
    surface.Child(panel);
    body.Children().Append(surface);

    StackPanel footer;
    footer.Spacing(2);
    footer.Margin(Thickness{2, 0, 2, 0});
    footer.Children().Append(make_text(theme.title, 13, palette_.text));
    const auto detail = on ? L"使用中" : theme.id == L"custom" ? L"自定义候选配色"
                                    : theme.dark ? (*theme.dark ? L"深色主题" : L"浅色主题")
                                                 : L"跟随颜色模式";
    footer.Children().Append(make_text(detail, 11, on ? palette_.accent : palette_.sub));
    body.Children().Append(footer);
    card.Content(body);
    A11y::SetName(card, hstring(L"主题 " + theme.title));
    if (on)
      A11y::SetItemStatus(card, L"使用中");
    card.IsEnabled(loaded_);
    card.Click([this, id = theme.id, on](Inspectable const &,
                                         RoutedEventArgs const &) {
      // 选中的「自定义」卡片槽位里仍可能有目录里没有的包（删掉了或还没扫描到）；共享页点它照样清掉两个槽位，其他选中的卡片再点不起作用。
      if (on && (id != L"custom" || sk::skins_in_use(true, custom_skins(document_)).empty()))
        return;
      change([&](PreferencesDocument &doc) {
        doc.SetString(L"global_theme", id);
        // 选「自定义」卡片本身会去掉两个槽位的皮肤包，保留自定义主题的其余部分，画在它自己的底上。
        if (id == L"custom") {
          doc.SetValue(L"custom_theme.candidate_skin", JsonValue::CreateNullValue());
          doc.SetValue(L"custom_theme.candidate_skin_dark", JsonValue::CreateNullValue());
        }
      }, true);
    });
    return card;
  }

  // 皮肤页提供内置主题、外部皮肤包、自定义配色和社区图库；字体在「候选窗口」页管理。
  // 搜索「社区皮肤」时进入图库，其他本页条目回到「我的皮肤」；图库不画完整实时预览。
  void follow_skin_search() {
    if (indexing_ || focus_row_.empty())
      return;
    const bool community = focus_row_ == L"社区皮肤";
    if (community == skin_community_view_)
      return;
    skin_community_view_ = community;
    if (community)
      enter_skin_community();
    else
      leave_skin_community();
  }

  void build_themes_page(StackPanel const &page) {
    const bool community = skin_community_view_ && !indexing_;
    community_tabs(page, L"皮肤来源", L"我的皮肤", L"社区皮肤",
                   L"浏览、安装和发布候选窗口皮肤", community,
                   [this](bool shown) { set_skin_view(shown); });
    if (community) {
      build_skin_community(page);
      return;
    }
    if (!indexing_ && skins_stale_)
      scan_skins(!skins_scanned_);
    const auto current = document_.String(L"global_theme", L"system");
    const bool custom_selected = current == L"custom";
    const auto custom = custom_skins(document_);
    const bool active_dark = candidate_dark();
    const auto listed = listed_skins();
    const auto shown = sk::shown_package(custom_selected, custom, active_dark, listed);

    // 主题选择在前，当前设置的完整预览在卡片之后，避免预览占满首屏。
    auto intro = make_text(
        L"选择候选窗口和悬浮工具栏使用的主题。卡片展示每款主题的候选配色。", 11,
        palette_.sub);
    intro.Margin(Thickness{4, 0, 4, 12});
    page.Children().Append(intro);

    if (themes_.empty()) {
      auto group = add_group(page, L"");
      add_row(group, 0xE790, L"全局主题", L"无法读取主题列表。请重新安装输入法后再试。",
              nullptr);
    } else {
      record(L"全局主题", L"候选窗口、悬浮工具栏、菜单和屏幕键盘的配色");
      auto cards = responsive_grid(174, 10);
      mark_search_target(L"全局主题", cards);
      for (const auto &theme : themes_) {
        record(theme.title, L"主题");
        if (!indexing_) {
          // 自定义主题用着目录里的皮肤包时，选中的是那款包的卡片，「自定义」卡片代表不带皮肤包的自定义主题。
          const bool on = theme.id == current && (theme.id != L"custom" || !shown);
          const auto card = make_theme_card(theme, on);
          mark_search_target(theme.title, card);
          cards.Children().Append(card);
        }
      }
      page.Children().Append(cards);
    }

    if (preview_visible_ && !indexing_)
      page.Children().Append(make_preview());

    if (!skin_packages_.empty()) {
      StackPanel list;
      list.Spacing(8);
      list.Margin(Thickness{0, 4, 0, 0});
      const std::wstring layout =
          document_.String(L"candidate_layout", L"vertical") == L"horizontal" ? L"horizontal"
                                                                               : L"vertical";
      for (const auto &skin : skin_packages_) {
        record(skin.name, L"皮肤");
        if (!indexing_)
          list.Children().Append(make_skin_card(
              skin, sk::skin_in_use(custom_selected, custom, skin.id), active_dark, layout));
      }
      page.Children().Append(list);
    }
    if (const auto summary = sk::slot_summary(custom_selected, custom, listed);
        !summary.empty()) {
      auto note = make_text(summary, 12, palette_.sub);
      note.Margin(Thickness{4, 10, 4, 0});
      page.Children().Append(note);
    }
    // 从卡片发布的结果（共享页的 published）。
    if (!skin_published_notice_.empty() && !indexing_) {
      auto note = make_text(skin_published_notice_, 12, palette_.sub);
      note.Margin(Thickness{4, 6, 4, 0});
      A11y::SetLiveSetting(note, Automation::Peers::AutomationLiveSetting::Polite);
      page.Children().Append(note);
    }

    // 设置窗口、工具栏、菜单和各面板一律跟随颜色模式（HostCapabilities::surface_themes_follow），只剩候选窗口还能单独设明暗，放在颜色模式下面（共享页「明暗」组里的 CandidateThemeRow）。
    auto look = add_group(page, L"外观");
    segment_row(look, 0xE706, L"颜色模式", L"设置窗口和各界面的明暗模式；候选窗口可在下面单独设置",
                L"theme",
                {{L"system", L"跟随系统"}, {L"light", L"浅色"}, {L"dark", L"深色"}},
                L"system");
    select_row(look, 0xE8FD, L"候选窗口主题", L"覆盖颜色模式，只影响候选窗口",
               L"candidate_theme", surface_theme_options(), L"follow", true);

    auto more = add_group(page, L"更多皮肤");
    external_skins_row(more);

    // 取色器收在默认收起的「自定义配色」里（共享页的 MoreOptions title），悬停色也在其中。搜索命中「自定义主题」或某个取色器时展开。
    auto colors = add_group(page, L"自定义主题");
    record(L"自定义主题", L"候选颜色取色器");
    Expander custom_colors;
    custom_colors.Header(box_value(L"自定义配色"));
    custom_colors.HorizontalAlignment(HorizontalAlignment::Stretch);
    custom_colors.HorizontalContentAlignment(HorizontalAlignment::Stretch);
    custom_colors.IsExpanded(custom_colors_open_ || focus_row_ == L"自定义主题" ||
                             fold_holds_focus(L"custom-colors"));
    custom_colors.Expanding([this](Expander const &, ExpanderExpandingEventArgs const &) {
      custom_colors_open_ = true;
    });
    custom_colors.Collapsed([this](Expander const &, ExpanderCollapsedEventArgs const &) {
      custom_colors_open_ = false;
    });
    mark_search_target(L"自定义主题", custom_colors);
    StackPanel color_rows;
    color_rows.Spacing(4);
    fold_index_ = L"custom-colors";
    candidate_color_rows(color_rows);
    fold_index_.clear();
    custom_colors.Content(color_rows);
    colors.Children().Append(custom_colors);
  }

  // ---- 皮肤包 ----

  void redraw_themes() {
    // 取色器打开时不重画：重画会拿掉它所在的页面。它关上时整页重画一次。
    if (!closed_ && current_page_ == "themes" && !color_picker_open_)
      queue_refresh();
  }

  // Runs `call` on a worker and hands its result to `done` on the UI thread, unless the window has closed by then.
  template <typename T>
  fire_and_forget run_worker(std::function<T()> call, std::function<void(T &)> done) {
    auto weak = get_weak();
    auto queue = DispatcherQueue();
    co_await resume_background();
    auto result = std::make_shared<T>(call());
    queue.TryEnqueue([weak, done = std::move(done), result] {
      if (auto self = weak.get(); self && !self->closed_)
        done(*result);
    });
  }

  // 皮肤根目录：Server 和 Tauri 外壳读的同一个，状态目录下的 skins。
  std::filesystem::path skins_root() const {
    const auto state = state_directory();
    return state.is_absolute() ? state / L"skins" : std::filesystem::path();
  }

  // 以 `base` 为底的皮肤包放在哪个槽位，按主题目录里那个主题的明暗。
  sk::Slot theme_slot(std::wstring const &base) const {
    for (const auto &entry : themes_)
      if (entry.id == base && entry.dark)
        return *entry.dark ? sk::Slot::dark : sk::Slot::light;
    return sk::Slot::either;
  }

  std::wstring theme_title(std::wstring const &id) const {
    for (const auto &entry : themes_)
      if (entry.id == id)
        return entry.title;
    return id;
  }

  std::vector<sk::Listed> listed_skins() const {
    std::vector<sk::Listed> listed;
    listed.reserve(skin_packages_.size());
    for (const auto &skin : skin_packages_)
      listed.push_back({skin.id, skin.name, theme_slot(skin.base)});
    return listed;
  }

  static sk::CustomSkins custom_skins(PreferencesDocument const &doc) {
    sk::CustomSkins value;
    value.base = doc.String(L"custom_theme.base", L"system");
    value.light = doc.String(L"custom_theme.candidate_skin", L"");
    value.dark = doc.String(L"custom_theme.candidate_skin_dark", L"");
    return value;
  }

  static void write_custom_skins(PreferencesDocument &doc, sk::CustomSkins const &value) {
    doc.SetString(L"custom_theme.base", value.base);
    auto slot = [&doc](wchar_t const *key, std::wstring const &id) {
      if (id.empty())
        doc.SetValue(key, JsonValue::CreateNullValue());
      else
        doc.SetString(key, id);
    };
    slot(L"custom_theme.candidate_skin", value.light);
    slot(L"custom_theme.candidate_skin_dark", value.dark);
  }

  // 打开一款包的开关：放进它所属的槽位，清单的 base 成为自定义主题的底，这样预览一致，取下皮肤后也保留这个底；关掉已在使用的包则把它从所在的槽位取下。两种都选上自定义主题。
  void use_skin(std::wstring const &id, std::wstring const &base, bool use) {
    const auto listed = listed_skins();
    const auto slot = theme_slot(base);
    auto slot_of = [&listed](std::wstring const &other) -> std::optional<sk::Slot> {
      for (const auto &entry : listed)
        if (entry.id == other)
          return entry.slot;
      return std::nullopt;
    };
    change([&](PreferencesDocument &doc) {
      const auto current = custom_skins(doc);
      write_custom_skins(doc, use ? sk::apply_candidate_skin(current, id, base, slot, slot_of)
                                  : sk::remove_candidate_skin(current, id));
      doc.SetString(L"global_theme", L"custom");
    }, true);
  }

  // 扫描皮肤目录，同一时间只有一次在进行。`manual`（第一次进入页面和「刷新皮肤」）的结果总会落地，并重读皮肤图片；自动重扫（再次进入页面、窗口重新获得焦点）的结果和上次相同时保留原来的卡片，也不显示「正在读取」，免得窗口一获得焦点整页就重画。进行中又点了「刷新皮肤」时，这次结束后再扫一次。
  void scan_skins(bool manual) {
    if (skins_pending_) {
      skins_manual_queued_ = skins_manual_queued_ || manual;
      return;
    }
    skins_pending_ = true;
    skins_stale_ = false;
    if (manual) {
      skins_busy_ = true;
      skins_failed_ = false;
      redraw_themes();
    }
    run_worker<SkinScan>([root = skins_root()] { return read_skin_catalog(root); },
                         [this, manual](SkinScan &scan) { finish_skin_scan(scan, manual); });
  }

  void finish_skin_scan(SkinScan &scan, bool manual) {
    skins_pending_ = false;
    const bool was_busy = skins_busy_;
    skins_busy_ = false;
    bool redraw = was_busy;
    if (!scan.ok) {
      // 读取失败保留上次扫描的结果。
      if (manual || !skins_failed_) {
        skins_failed_ = true;
        redraw = true;
      }
    } else {
      const bool failed = skins_failed_;
      skins_failed_ = false;
      redraw = redraw || failed;
      if (manual || !skins_scanned_ || scan.signature != skins_signature_) {
        skins_scanned_ = true;
        skins_signature_ = std::move(scan.signature);
        skins_directory_ = std::move(scan.directory);
        skin_packages_ = std::move(scan.packages);
        skin_issues_ = std::move(scan.issues);
        // 新的一次扫描：旧图片作废，在途的解码按 revision 丢弃。
        ++skins_revision_;
        skin_images_.clear();
        skin_image_failures_.clear();
        // 用着的包可能改过或删掉了：顶部预览重新解析。
        resolved_theme_.reset();
        for (auto &image : scan.images) {
          if (image.bytes.empty())
            skin_image_failures_.insert(image.key);
          else
            load_skin_image(std::move(image.key), std::move(image.content_type),
                            std::move(image.bytes), skins_revision_);
        }
        redraw = true;
      }
    }
    if (skins_manual_queued_) {
      skins_manual_queued_ = false;
      scan_skins(true);
      return;
    }
    if (redraw)
      redraw_themes();
  }

  // 把一张皮肤图片解码成卡片能画的图像，SVG 用 SvgImageSource，其余交给 BitmapImage。解码不了时记为失败，卡片上说明并保留基础预览。
  fire_and_forget load_skin_image(std::wstring key, std::string content_type,
                                  std::vector<uint8_t> bytes, uint64_t revision) {
    auto strong = get_strong();
    ImageSource source{nullptr};
    try {
      Windows::Storage::Streams::InMemoryRandomAccessStream stream;
      Windows::Storage::Streams::DataWriter writer(stream);
      writer.WriteBytes(array_view<uint8_t const>(bytes.data(), bytes.data() + bytes.size()));
      co_await writer.StoreAsync();
      writer.DetachStream();
      stream.Seek(0);
      if (content_type == "image/svg+xml") {
        Imaging::SvgImageSource svg;
        if (co_await svg.SetSourceAsync(stream) == Imaging::SvgImageSourceLoadStatus::Success)
          source = svg;
      } else {
        Imaging::BitmapImage bitmap;
        co_await bitmap.SetSourceAsync(stream);
        source = bitmap;
      }
    } catch (hresult_error const &) {
      source = nullptr;
    }
    if (closed_ || revision != skins_revision_)
      co_return;
    if (source)
      skin_images_.insert_or_assign(key, source);
    else
      skin_image_failures_.insert(key);
    redraw_themes();
  }

  // 一张已解码的皮肤图片；还在解码、读不到或解码失败时为空。
  ImageSource skin_image_source(std::wstring const &id, std::wstring const &relative) const {
    if (relative.empty())
      return nullptr;
    const auto found = skin_images_.find(skin_image_key(id, relative));
    return found == skin_images_.end() ? nullptr : found->second;
  }

  bool skin_image_failed(std::wstring const &id, std::wstring const &relative) const {
    return !relative.empty() && skin_image_failures_.contains(skin_image_key(id, relative));
  }

  // 画到屏幕上才发现解码不了的图片同样记为失败。`image` 是装饰图的 Image 或背景图的 ImageBrush。
  template <typename Element> void watch_skin_image(Element const &image, std::wstring key) {
    image.ImageFailed([this, key = std::move(key), revision = skins_revision_](
                          Inspectable const &, ExceptionRoutedEventArgs const &) {
      if (revision != skins_revision_ || skin_image_failures_.contains(key))
        return;
      skin_images_.erase(key);
      skin_image_failures_.insert(key);
      redraw_themes();
    });
  }

  // 一张皮肤包卡片（ExternalSkinCard）：标题带上卡片画的明暗和「使用中」，说明、槽位、id · 版本 · 作者，开关、「预览浅色 / 预览深色」和「发布到社区」，以及按 resolve() 给这款包的配色画的横排、竖排两个候选窗口，带上包的圆角、最小宽度、背景图和装饰图。悬浮工具栏的预览要用包的 CSS 画，这里不画，与内置主题的卡片一致。
  FrameworkElement make_skin_card(SkinPackage const &skin, bool selected, bool active_dark,
                                  std::wstring const &layout) {
    const auto slot = theme_slot(skin.base);
    std::optional<bool> override_dark;
    // 卡片上选的预览明暗在当前明暗变了以后作废，与共享卡片一样。
    if (const auto found = skin_preview_overrides_.find(skin.id);
        found != skin_preview_overrides_.end() && found->second.first == active_dark)
      override_dark = found->second.second;
    const bool dark = sk::card_dark(slot, override_dark, active_dark, skin.themes);
    const bool compatible =
        sk::card_compatible(slot, skin.layouts, skin.themes, layout, active_dark);

    Border card;
    card.Background(brush(palette_.card));
    card.BorderBrush(brush(selected ? palette_.accent : palette_.stroke));
    card.BorderThickness(selected ? Thickness{2, 2, 2, 2} : Thickness{1, 1, 1, 1});
    card.CornerRadius(CornerRadius{6, 6, 6, 6});
    card.Padding(Thickness{12, 12, 16, 12});
    Grid body;
    body.ColumnSpacing(16);
    ColumnDefinition preview_column;
    preview_column.Width(GridLength{1, GridUnitType::Auto});
    ColumnDefinition text_column;
    text_column.Width(GridLength{1, GridUnitType::Star});
    body.ColumnDefinitions().Append(preview_column);
    body.ColumnDefinitions().Append(text_column);

    auto preview = make_skin_preview(skin, dark);
    Grid::SetColumn(preview, 0);
    body.Children().Append(preview);

    StackPanel texts;
    texts.Spacing(4);
    texts.VerticalAlignment(VerticalAlignment::Center);
    StackPanel title;
    title.Orientation(Orientation::Horizontal);
    title.Spacing(8);
    title.Children().Append(
        make_text(skin.name + (dark ? L"（深色）" : L"（浅色）"), 14, palette_.text));
    if (selected) {
      auto badge = make_text(L"使用中", 12, palette_.accent);
      badge.VerticalAlignment(VerticalAlignment::Center);
      title.Children().Append(badge);
    }
    texts.Children().Append(title);
    texts.Children().Append(make_text(
        sk::card_description(compatible, skin.description, theme_title(skin.base), skin.layouts,
                             skin.themes),
        13, palette_.sub));
    texts.Children().Append(make_text(sk::slot_label(slot), 12, palette_.faint));
    if (const auto meta = sk::card_meta(skin.id, skin.version, skin.author); !meta.empty())
      texts.Children().Append(make_text(meta, 12, palette_.faint));

    StackPanel actions;
    actions.Orientation(Orientation::Horizontal);
    actions.Spacing(16);
    actions.Margin(Thickness{0, 6, 0, 0});
    // 已在使用的包总能关掉，哪怕布局变了以后它不再能选。
    actions.Children().Append(toggle_control(
        skin.name, selected,
        [this, id = skin.id, base = skin.base, selected](bool) { use_skin(id, base, !selected); },
        compatible || selected));
    if (slot == sk::Slot::either) {
      auto toggle = button_control(dark ? L"预览浅色" : L"预览深色",
                                   [this, id = skin.id, active_dark, dark] {
                                     skin_preview_overrides_[id] = {active_dark, !dark};
                                     redraw_themes();
                                   });
      toggle.VerticalAlignment(VerticalAlignment::Center);
      actions.Children().Append(toggle);
    }
    // 「发布到社区」：打开发布对话框，先选中这一款。
    auto publish = button_control(L"发布到社区", [this, id = skin.id] { open_skin_publish(id, true); });
    publish.VerticalAlignment(VerticalAlignment::Center);
    actions.Children().Append(publish);
    texts.Children().Append(actions);
    if (skin_image_failed(skin.id, skin.decoration_image) ||
        skin_image_failed(skin.id, skin.background_image)) {
      auto failed = make_text(L"皮肤图片加载失败，保留基础预览。可刷新皮肤重试。", 12,
                              palette_.sub);
      A11y::SetLiveSetting(failed, Automation::Peers::AutomationLiveSetting::Polite);
      texts.Children().Append(failed);
    }
    Grid::SetColumn(texts, 1);
    body.Children().Append(texts);
    card.Child(body);
    A11y::SetName(card, hstring(skin.name));
    mark_search_target(skin.name, card);
    return card;
  }

  // 卡片左边的预览：底的背景色上，横排和竖排两个候选窗口，配色是 resolve() 给这款包在这种明暗下的配色（包画在它的 base 之上）。
  FrameworkElement make_skin_preview(SkinPackage const &skin, bool dark) {
    const auto &theme = dark ? skin.dark : skin.light;
    Border stage;
    stage.Width(260);
    stage.MinHeight(132);
    stage.CornerRadius(CornerRadius{4, 4, 4, 4});
    stage.Padding(Thickness{10, 10, 10, 10});
    std::optional<Color> backdrop;
    for (const auto &entry : themes_)
      if (entry.id == skin.base && entry.preview)
        backdrop = entry.preview->background;
    stage.Background(brush(backdrop.value_or(dark ? rgba(0x20, 0x20, 0x20, 1)
                                                  : rgba(0xF3, 0xF3, 0xF3, 1))));
    StackPanel samples;
    samples.Orientation(Orientation::Horizontal);
    samples.Spacing(10);
    samples.HorizontalAlignment(HorizontalAlignment::Center);
    samples.VerticalAlignment(VerticalAlignment::Center);
    if (skin.canvas.active) {
      for (const bool vertical : {false, true}) {
        if (std::find(skin.layouts.begin(), skin.layouts.end(),
                      vertical ? L"vertical" : L"horizontal") == skin.layouts.end())
          continue;
        const auto key = skin_canvas_preview_key(skin.id, dark, vertical);
        const auto found = skin_images_.find(key);
        if (found == skin_images_.end())
          continue;
        Image image;
        image.Source(found->second);
        image.Stretch(Stretch::Uniform);
        image.MaxWidth(115);
        image.MaxHeight(180);
        watch_skin_image(image, key);
        A11y::SetAutomationId(image, hstring(L"canvas-preview-" + skin.id +
                                             (vertical ? L"-vertical" : L"-horizontal")));
        A11y::SetName(image, hstring(skin.name + (vertical ? L"竖排候选预览" : L"横排候选预览")));
        samples.Children().Append(image);
      }
      if (samples.Children().Size() == 0)
        samples.Children().Append(make_text(L"皮肤预览暂不可用", 12, palette_.sub));
    } else {
      samples.Children().Append(make_skin_sample(skin, theme, false));
      samples.Children().Append(make_skin_sample(skin, theme, true));
    }
    stage.Child(samples);
    return stage;
  }

  // 一个缩小的候选窗口：拼音行和三个候选，首个选中。装饰带按清单的高度和宽度画在卡片上方，按 decorationAlign 对齐；背景图画在表面之上、候选之下，按卡片的圆角裁切。
  FrameworkElement make_skin_sample(SkinPackage const &skin,
                                    msime::windows::CandidatePalette const &theme,
                                    bool vertical) {
    constexpr double scale = 0.7;
    const double radius = (skin.corner_radius ? *skin.corner_radius : theme.radius) * scale;
    StackPanel column;
    column.VerticalAlignment(VerticalAlignment::Center);
    if (skin.decoration_top > 0) {
      Grid band;
      band.Height(skin.decoration_top * scale);
      if (const auto source = skin_image_source(skin.id, skin.decoration_image)) {
        Image image;
        image.Source(source);
        image.Stretch(Stretch::Uniform);
        image.Width(std::min(skin.decoration_width * scale, 120.0));
        watch_skin_image(image, skin_image_key(skin.id, skin.decoration_image));
        image.HorizontalAlignment(skin.decoration_align == L"left"     ? HorizontalAlignment::Left
                                  : skin.decoration_align == L"center" ? HorizontalAlignment::Center
                                                                       : HorizontalAlignment::Right);
        band.Children().Append(image);
      }
      column.Children().Append(band);
    }

    Border box;
    box.Background(brush(to_color(theme.surface)));
    box.BorderBrush(brush(to_color(theme.border)));
    const double edge = theme.border_width;
    box.BorderThickness(Thickness{edge, edge, edge, edge});
    box.CornerRadius(CornerRadius{radius, radius, radius, radius});
    box.MinWidth(std::min(skin.min_width * scale, vertical ? 90.0 : 150.0));
    Grid layers;
    if (const auto source = skin_image_source(skin.id, skin.background_image)) {
      Border fill;
      const double inner = std::max(0.0, radius - edge);
      fill.CornerRadius(CornerRadius{inner, inner, inner, inner});
      ImageBrush image;
      image.ImageSource(source);
      image.Stretch(skin.background_fit == L"contain"   ? Stretch::Uniform
                    : skin.background_fit == L"stretch" ? Stretch::Fill
                                                        : Stretch::UniformToFill);
      image.Opacity(skin.background_opacity);
      watch_skin_image(image, skin_image_key(skin.id, skin.background_image));
      fill.Background(image);
      layers.Children().Append(fill);
    }
    StackPanel content;
    content.Padding(Thickness{6, 4, 6, 4});
    content.Spacing(2);
    auto preedit = make_text(L"ni'mf", 11, to_color(theme.accent));
    preedit.FontWeight(Windows::UI::Text::FontWeight{600});
    preedit.Margin(Thickness{3, 0, 3, 0});
    content.Children().Append(preedit);
    StackPanel list;
    list.Orientation(vertical ? Orientation::Vertical : Orientation::Horizontal);
    list.Spacing(2);
    const double row_radius = theme.item_radius * scale;
    constexpr std::array<const wchar_t *, 3> words{L"你们", L"你", L"尼"};
    for (size_t i = 0; i < words.size(); ++i) {
      const bool highlighted = i == 0;
      Grid item;
      item.Padding(Thickness{5, 2, 6, 2});
      item.CornerRadius(CornerRadius{row_radius, row_radius, row_radius, row_radius});
      if (highlighted)
        item.Background(brush(to_color(theme.selected)));
      if (highlighted && theme.show_selected_bar) {
        Border bar;
        bar.Width(2);
        bar.Height(11);
        bar.CornerRadius(CornerRadius{1, 1, 1, 1});
        bar.Background(brush(to_color(theme.accent)));
        bar.HorizontalAlignment(HorizontalAlignment::Left);
        bar.VerticalAlignment(VerticalAlignment::Center);
        bar.Margin(Thickness{-5, 0, 0, 0});
        item.Children().Append(bar);
      }
      StackPanel cell;
      cell.Orientation(Orientation::Horizontal);
      cell.Spacing(4);
      auto number = make_text(std::to_wstring(i + 1), 10,
                              to_color(msime::windows::candidate_row_number_color(theme, highlighted)));
      number.VerticalAlignment(VerticalAlignment::Center);
      cell.Children().Append(number);
      auto word = make_text(words[i], 13,
                            to_color(msime::windows::candidate_row_text_color(
                                theme, theme.text, highlighted, false)));
      word.TextWrapping(TextWrapping::NoWrap);
      cell.Children().Append(word);
      item.Children().Append(cell);
      list.Children().Append(item);
    }
    content.Children().Append(list);
    layers.Children().Append(content);
    box.Child(layers);
    column.Children().Append(box);
    return column;
  }

  // 「外部皮肤」一行（ExternalSkinDirectoryRow）：说明、上次扫描的结果和皮肤目录，「打开目录」「刷新皮肤」，以及打不开、读不到和被忽略的皮肤目录。Windows 打开的是文件夹，不是导入。
  void external_skins_row(StackPanel const &group) {
    StackPanel buttons;
    buttons.Orientation(Orientation::Horizontal);
    buttons.Spacing(8);
    buttons.Children().Append(button_control(skins_opening_ ? L"正在打开…" : L"打开目录",
                                             [this] { open_skin_directory(); }, !skins_opening_));
    buttons.Children().Append(button_control(skins_busy_ ? L"正在扫描…" : L"刷新皮肤",
                                             [this] { scan_skins(true); }, !skins_busy_));

    StackPanel below;
    below.Spacing(4);
    below.Margin(Thickness{0, 0, 0, 14});
    const std::wstring status = skins_busy_       ? L"正在读取皮肤目录。"
                                : !skins_scanned_ ? L"尚未扫描。点击“刷新皮肤”读取皮肤目录。"
                                : skin_packages_.empty() ? L"没有发现外部皮肤。"
                                                         : L"";
    auto state = make_text(status, 12, palette_.sub);
    A11y::SetLiveSetting(state, Automation::Peers::AutomationLiveSetting::Polite);
    state.Visibility(status.empty() ? Visibility::Collapsed : Visibility::Visible);
    below.Children().Append(state);
    if (skins_scanned_ && !skins_directory_.empty()) {
      auto directory = make_text(skins_directory_, 12, palette_.sub);
      directory.FontFamily(FontFamily(L"Consolas"));
      directory.IsTextSelectionEnabled(true);
      ToolTipService::SetToolTip(directory, box_value(hstring(skins_directory_)));
      below.Children().Append(directory);
    }
    auto alert = [this, &below](std::wstring const &message) {
      auto line = make_text(message, 12, danger_color());
      A11y::SetLiveSetting(line, Automation::Peers::AutomationLiveSetting::Assertive);
      below.Children().Append(line);
    };
    if (skins_open_failed_)
      alert(L"无法打开皮肤目录，请重试。");
    if (skins_failed_)
      alert(std::wstring(L"读取皮肤目录失败，请重试。") +
            (skins_scanned_ ? L"仍显示上次扫描结果。" : L""));
    if (!skin_issues_.empty()) {
      Expander issues;
      issues.Header(box_value(hstring(L"已忽略 " + std::to_wstring(skin_issues_.size()) +
                                      L" 个无效皮肤目录")));
      issues.HorizontalAlignment(HorizontalAlignment::Stretch);
      issues.HorizontalContentAlignment(HorizontalAlignment::Left);
      StackPanel lines;
      lines.Spacing(4);
      for (const auto &issue : skin_issues_) {
        auto line = make_text(issue.folder + L"：" + issue.reason, 12, palette_.sub);
        line.IsTextSelectionEnabled(true);
        lines.Children().Append(line);
      }
      issues.Content(lines);
      below.Children().Append(issues);
    }
    add_row(group, 0xE8B7, L"外部皮肤",
            L"把包含 skin.toml 的皮肤文件夹复制到下面的目录，回到这个窗口时会自动列出。",
            buttons, below);
  }

  // 「打开目录」：没有皮肤目录时先建出来，再在资源管理器里打开，与 Tauri 外壳的 open_skin_directory 相同。建目录在工作线程上，打开在 UI 线程上。
  void open_skin_directory() {
    if (skins_opening_)
      return;
    skins_opening_ = true;
    skins_open_failed_ = false;
    redraw_themes();
    const auto root = skins_root();
    run_worker<bool>(
        [root] {
          if (root.empty())
            return false;
          std::error_code error;
          std::filesystem::create_directories(root, error);
          return std::filesystem::is_directory(root, error);
        },
        [this, root](bool &ready) {
          const bool opened =
              ready && reinterpret_cast<INT_PTR>(ShellExecuteW(nullptr, L"open", root.c_str(),
                                                               nullptr, nullptr,
                                                               SW_SHOWNORMAL)) > 32;
          skins_opening_ = false;
          skins_open_failed_ = !opened;
          redraw_themes();
        });
  }

  // ---- 社区皮肤与社区插件 ----

  // 共享外壳在桌面上给「主题」和「插件」两页顶上各放一对标签（index.tsx 的「皮肤来源」「插件来源」）：「我的皮肤」「我的插件」是本页的设置，「社区皮肤」「社区插件」换成社区图库（community-candidate-skins.tsx 的 CommunityCandidateSkinsPage、community-plugins.tsx 的 CommunityPluginsPage）。图库挂上时读第一页，候选窗皮肤还和账号的皮肤库同步一次（candidate-skin-sync.tsx）；离开页面或切回本页设置就卸下，下次从头读起。浏览、详情、安装、评分、举报、作者改公开范围和分类、下架都在页面里；发布是对话框（candidate-skin-publish-dialog.tsx、CommunityPluginPublishDialog），皮肤还能从「我的皮肤」里各自的卡片发布。所有社区操作经 msime_client_community，用「账号与同步」页同一个账号会话，都在工作线程上；文案和规则在 CommunityPageModel.h。

  // 一个图库的状态（共享页 useCommunityGallery 和 useCommunityInstallState 的组件状态）。owner 在挂上和卸下时递增，作废在途的回答；list_request、detail_request 作废同一种更早的请求。
  template <typename Item> struct Gallery {
    std::vector<Item> items;
    bool has_more = false;
    bool list_busy = true;
    bool detail_busy = false;
    bool action_busy = false;
    std::wstring error;
    bool sign_in = false;
    std::optional<Item> selected;
    std::wstring notice;
    bool mine = false;
    bool confirm_unpublish = false;
    uint64_t owner = 0;
    uint64_t list_request = 0;
    uint64_t detail_request = 0;
    size_t next_offset = 0;
    std::wstring active_search;
    bool active_mine = false;
    // 搜索框里正在输入的词，「搜索」后才成为 active_search。
    std::wstring search;
    bool installed = false;
    bool confirm_replace = false;
    // CommunityReportSection 的表单。
    bool report_open = false;
    std::wstring report_reason;
    std::wstring report_detail;
  };

  // 一张社区皮肤预览图：解码好的图片，或读取失败；都没有时还在读。
  struct CommunityPreview {
    ImageSource image{nullptr};
    bool failed = false;
  };

  template <typename Item> static constexpr bool skin_item = std::is_same_v<Item, CommunitySkinItem>;
  template <typename Item>
  static constexpr bool resource_item = std::is_same_v<Item, CommunityResourceItem>;

  template <typename Item> Gallery<Item> &gallery_for() {
    if constexpr (skin_item<Item>)
      return skin_gallery_;
    else if constexpr (resource_item<Item>)
      return resource_gallery_;
    else
      return plugin_gallery_;
  }

  // 三个图库里同一件事的操作名：detail、rate，以及作者的下架（插件叫 delete）。
  template <typename Item> static wchar_t const *community_operation(std::wstring_view verb) {
    const bool detail = verb == L"detail";
    const bool rate = verb == L"rate";
    if constexpr (skin_item<Item>)
      return detail ? L"candidate_skin_detail" : rate ? L"candidate_skin_rate" : L"candidate_skin_unpublish";
    else if constexpr (resource_item<Item>)
      return detail ? L"resource_detail" : rate ? L"resource_rate" : L"resource_unpublish";
    else
      return detail ? L"plugin_detail" : rate ? L"plugin_rate" : L"plugin_delete";
  }

  // 出错时给不给「去登录」：词库与回复模板的图库不给（共享页的资源图库没有 onLogin），其余未登录时给。
  template <typename Item> static bool gallery_sign_in(std::wstring const &code) {
    return !resource_item<Item> && cm::needs_sign_in(code);
  }

  // 图库的出错说明：皮肤用 candidateSkinMessage，词库与回复模板用 resourceMessage，插件用 communityPluginMessage。
  template <typename Item>
  static std::wstring gallery_message(CommunityFailure const &failure, bool publishing = false) {
    if constexpr (skin_item<Item>)
      return cm::candidate_skin_message(failure.code, publishing);
    else if constexpr (resource_item<Item>)
      return cm::resource_message(failure.code);
    else
      return cm::plugin_message(failure.code, failure.detail, publishing);
  }

  template <typename Item> void redraw_gallery() {
    if constexpr (skin_item<Item>)
      redraw_themes();
    else if constexpr (resource_item<Item>)
      redraw_account();
    else
      redraw_plugins();
  }

  // 共享页的 fail：说明出错，未登录时给「去登录」。
  template <typename Item> void gallery_fail(CommunityFailure const &failure) {
    auto &gallery = gallery_for<Item>();
    gallery.error = gallery_message<Item>(failure);
    gallery.sign_in = gallery_sign_in<Item>(failure.code);
  }

  // 共享页的 openAccountLogin：去「账号与同步」页登录。
  void open_account_login() { navigate("account", true); }

  // ---- 标签 ----

  // 页首的一对来源标签。「社区皮肤」「社区插件」进搜索索引，选中这样的结果时打开图库，选中本页的其他设置时回到本页。
  void community_tabs(StackPanel const &page, std::wstring const &name, std::wstring const &mine,
                      std::wstring const &community, std::wstring const &detail, bool shown,
                      std::function<void(bool)> choose) {
    record(community, detail);
    auto tabs = segmented_control(
        name, {{L"mine", mine}, {L"community", community}}, shown ? L"community" : L"mine",
        [choose = std::move(choose)](std::wstring const &value) { choose(value == L"community"); },
        true, false);
    tabs.HorizontalAlignment(HorizontalAlignment::Left);
    tabs.Margin(Thickness{0, 0, 0, 12});
    mark_search_target(community, tabs);
    page.Children().Append(tabs);
  }

  void set_skin_view(bool community) {
    if (community == skin_community_view_)
      return;
    skin_community_view_ = community;
    if (community)
      enter_skin_community();
    else
      leave_skin_community();
    queue_refresh();
  }

  void set_plugin_view(bool community) {
    if (community == plugin_community_view_)
      return;
    plugin_community_view_ = community;
    if (community) {
      // 切到社区插件时关上打开的视图，下次回来从列表开始。
      plugin_view_ = PluginView::list;
      plugin_pending_focus_.clear();
      enter_plugin_community();
    } else {
      leave_plugin_community();
      // 社区里装的插件要在「我的插件」里列出来：回来时重读目录。
      plugins_stale_ = true;
    }
    queue_refresh();
  }

  // 图库挂上：从头读第一页。皮肤图库还和账号的皮肤库同步一次。
  template <typename Item> void reset_gallery() {
    auto &gallery = gallery_for<Item>();
    const auto owner = gallery.owner + 1;
    gallery = Gallery<Item>{};
    gallery.owner = owner;
  }

  void enter_skin_community() {
    reset_gallery<CommunitySkinItem>();
    skin_category_request_.reset();
    skin_category_active_.reset();
    skin_category_shown_.reset();
    // 读失败的预览图留在缓存里显示「预览图加载失败」，重新挂上图库时再试。
    // 顺序表一起删，否则同一款会在里面出现两次，按旧的那一次被挤掉，正在读的图读完找不到缓存，卡片一直停在「正在读取预览…」。
    std::erase_if(community_preview_order_, [this](std::wstring const &id) {
      const auto found = community_previews_.find(id);
      return found != community_previews_.end() && found->second.failed;
    });
    std::erase_if(community_previews_, [](auto const &entry) { return entry.second.failed; });
    gallery_list<CommunitySkinItem>(L"", false, false);
    run_skin_sync();
  }

  void leave_skin_community() {
    ++skin_gallery_.owner;
    skin_gallery_.action_busy = false;
  }

  void enter_plugin_community() {
    reset_gallery<CommunityPluginItem>();
    plugin_kind_request_.reset();
    plugin_kind_shown_.reset();
    gallery_list<CommunityPluginItem>(L"", false, false);
  }

  void leave_plugin_community() {
    ++plugin_gallery_.owner;
    plugin_gallery_.action_busy = false;
  }

  // ---- 读取 ----

  // 共享页的 requestList：从第一页读（append 为假）或接着读下一页。`then` 收到 false 只在这是最新的一次请求而且失败了，列表照旧；成功或被更新的请求取代时收到 true。
  template <typename Item>
  void gallery_list(std::wstring const &query, bool append, bool mine,
                    std::function<void(bool)> then = nullptr) {
    auto &gallery = gallery_for<Item>();
    const auto owner = gallery.owner;
    const auto request = ++gallery.list_request;
    const size_t offset = append ? gallery.next_offset : 0;
    gallery.list_busy = true;
    gallery.error.clear();
    gallery.sign_in = false;
    redraw_gallery<Item>();
    // 词库与回复模板按分类（kind）和范围（scope）读，没有「我的作品」开关和筛选。
    std::optional<std::wstring> filter;
    std::wstring scope;
    if constexpr (skin_item<Item>)
      filter = skin_category_request_;
    else if constexpr (resource_item<Item>) {
      filter = resource_kind_;
      scope = resource_scope_;
    } else
      filter = plugin_kind_request_;
    run_worker<CommunityListPage<Item>>(
        [offset, query, mine, filter, scope] {
          if constexpr (resource_item<Item>)
            return read_resource_page(offset, query, filter.value_or(L"dictionary"), scope);
          else
            return read_community_page<Item>(offset, query, mine, filter);
        },
        [this, owner, request, append, query, mine,
         then = std::move(then)](CommunityListPage<Item> &page) {
          auto &current = gallery_for<Item>();
          if (owner != current.owner)
            return;
          if (request != current.list_request) {
            if (then)
              then(true);
            return;
          }
          current.list_busy = false;
          if (!page.ok) {
            gallery_fail<Item>(page.failure);
            if (!append)
              current.mine = current.active_mine;
            // 资源图库读第一页失败时，列着的是上一次查询的条目，不能接着它们的 offset 读下一页。
            if constexpr (resource_item<Item>)
              if (!append)
                current.has_more = false;
            redraw_gallery<Item>();
            if (then)
              then(false);
            return;
          }
          if (append) {
            // appendUniqueById：只加还没有的。
            for (auto &item : page.items)
              if (std::none_of(current.items.begin(), current.items.end(),
                               [&item](Item const &listed) { return listed.id == item.id; }))
                current.items.push_back(item);
          } else {
            current.items = page.items;
            current.active_search = query;
            current.active_mine = mine;
          }
          current.next_offset = page.offset + page.items.size() + page.skipped;
          current.has_more = page.has_more;
          redraw_gallery<Item>();
          if (then)
            then(true);
        });
  }

  // 共享页的 open：先显示列表里的那一项，再读详情换上。
  template <typename Item> void gallery_open(Item const &item) {
    auto &gallery = gallery_for<Item>();
    const auto owner = gallery.owner;
    const auto request = ++gallery.detail_request;
    gallery.selected = item;
    gallery.detail_busy = true;
    gallery.error.clear();
    gallery.sign_in = false;
    gallery.notice.clear();
    gallery.confirm_unpublish = false;
    gallery.report_open = false;
    gallery.report_reason.clear();
    gallery.report_detail.clear();
    redraw_gallery<Item>();
    auto action = community_action(community_operation<Item>(L"detail"));
    action.SetNamedValue(L"id", JsonValue::CreateStringValue(hstring(item.id)));
    run_host([action] { return community_call(action); },
             [this, owner, request](Response const &response) {
               auto &current = gallery_for<Item>();
               if (owner != current.owner || request != current.detail_request)
                 return;
               if (auto value = community_item<Item>(response))
                 current.selected = std::move(*value);
               else
                 gallery_fail<Item>(community_failure(response));
               current.detail_busy = false;
               redraw_gallery<Item>();
             });
  }

  // 共享页的 closeDetail（连同 useCommunityInstallState 的那部分）：操作进行中时不能返回。
  template <typename Item> void gallery_close() {
    auto &gallery = gallery_for<Item>();
    if (gallery.action_busy)
      return;
    ++gallery.detail_request;
    gallery.selected.reset();
    gallery.detail_busy = false;
    gallery.error.clear();
    gallery.sign_in = false;
    gallery.confirm_unpublish = false;
    gallery.installed = false;
    gallery.confirm_replace = false;
    gallery.report_open = false;
    redraw_gallery<Item>();
  }

  // 共享页的 runAction：同一时间只有一个；开始时清掉出错说明（clear_notice 时连提示也清掉）。
  template <typename Item> bool gallery_begin(bool clear_notice) {
    auto &gallery = gallery_for<Item>();
    if (gallery.action_busy || !gallery.selected)
      return false;
    gallery.action_busy = true;
    gallery.sign_in = false;
    gallery.error.clear();
    if (clear_notice)
      gallery.notice.clear();
    redraw_gallery<Item>();
    return true;
  }

  // 操作结束；失败时说明出错（ignore 为真的那种除外）。
  template <typename Item>
  void gallery_end(std::optional<CommunityFailure> const &failure = std::nullopt) {
    auto &gallery = gallery_for<Item>();
    gallery.action_busy = false;
    if (failure) {
      gallery.error = gallery_message<Item>(*failure);
      gallery.sign_in = gallery_sign_in<Item>(failure->code);
    }
    redraw_gallery<Item>();
  }

  // 一次图库操作：在工作线程上调用，回到 UI 线程时图库还是同一个才交给 `done`。
  template <typename Item>
  void gallery_call(std::function<Response()> call, std::function<void(Response const &)> done) {
    const auto owner = gallery_for<Item>().owner;
    run_host(std::move(call), [this, owner, done = std::move(done)](Response const &response) {
      if (owner == gallery_for<Item>().owner)
        done(response);
    });
  }

  // rateSelected：评分后重读详情，显示「已评分」。
  template <typename Item> void gallery_rate(int stars) {
    if (!gallery_begin<Item>(false))
      return;
    const auto id = gallery_for<Item>().selected->id;
    gallery_call<Item>(
        [id, stars] {
          auto rate = community_action(community_operation<Item>(L"rate"));
          rate.SetNamedValue(L"id", JsonValue::CreateStringValue(hstring(id)));
          rate.SetNamedValue(L"stars", JsonValue::CreateNumberValue(stars));
          auto rated = community_call(rate);
          if (!rated.ok)
            return rated;
          auto detail = community_action(community_operation<Item>(L"detail"));
          detail.SetNamedValue(L"id", JsonValue::CreateStringValue(hstring(id)));
          return community_call(detail);
        },
        [this, stars](Response const &response) {
          auto item = community_item<Item>(response);
          if (!item) {
            gallery_end<Item>(community_failure(response));
            return;
          }
          auto &gallery = gallery_for<Item>();
          gallery.selected = std::move(*item);
          gallery.notice = L"已评分：" + std::to_wstring(stars) + L" 星。";
          gallery_end<Item>();
        });
  }

  // unpublishSelected：下架后回到列表，说明结果，从第一页重读；重读完操作才结束。
  template <typename Item> void gallery_unpublish(std::wstring message) {
    if (!gallery_begin<Item>(false))
      return;
    const auto id = gallery_for<Item>().selected->id;
    gallery_call<Item>(
        [id] {
          auto action = community_action(community_operation<Item>(L"unpublish"));
          action.SetNamedValue(L"id", JsonValue::CreateStringValue(hstring(id)));
          return community_call(action);
        },
        [this, message = std::move(message)](Response const &response) {
          if (!response.ok) {
            gallery_end<Item>(community_failure(response));
            return;
          }
          auto &gallery = gallery_for<Item>();
          ++gallery.detail_request;
          gallery.selected.reset();
          gallery.confirm_unpublish = false;
          // ResourceDetail 的 unpublish 只关上详情：列表不重读，也不说明。
          if constexpr (resource_item<Item>) {
            gallery_end<Item>();
            return;
          }
          gallery.installed = false;
          gallery.confirm_replace = false;
          gallery.notice = message;
          gallery_list<Item>(gallery.active_search, false, gallery.active_mine,
                             [this](bool) { gallery_end<Item>(); });
        });
  }

  // reportSelected：举报被接受后关上表单，显示「已收到举报」。
  template <typename Item> void gallery_report() {
    auto &gallery = gallery_for<Item>();
    if (gallery.report_reason.empty() || !gallery_begin<Item>(true))
      return;
    JsonObject action = community_action(L"report");
    std::wstring kind = skin_item<Item> ? L"candidate-skins" : L"plugins";
    if constexpr (resource_item<Item>)
      kind = gallery.selected->kind == L"dictionary" ? L"dictionaries" : L"replies";
    action.SetNamedValue(L"kind", JsonValue::CreateStringValue(hstring(kind)));
    action.SetNamedValue(L"id", JsonValue::CreateStringValue(hstring(gallery.selected->id)));
    action.SetNamedValue(L"reason", JsonValue::CreateStringValue(hstring(gallery.report_reason)));
    action.SetNamedValue(L"detail",
                         JsonValue::CreateStringValue(hstring(cm::trim(gallery.report_detail))));
    gallery_call<Item>([action] { return community_call(action); },
                       [this](Response const &response) {
                         if (!response.ok) {
                           gallery_end<Item>(community_failure(response));
                           return;
                         }
                         auto &current = gallery_for<Item>();
                         current.notice = std::wstring(cm::reported_notice);
                         current.report_open = false;
                         current.report_reason.clear();
                         current.report_detail.clear();
                         gallery_end<Item>();
                       });
  }

  // ---- 皮肤图库的操作 ----

  // useCandidateSkinSync：本机皮肤目录与账号的皮肤库同步一次；进行中又要同步时，这次结束后再来一次。下载或删除了本机的皮肤时「我的皮肤」重扫目录。未登录时 Rust 在读会话这一步就结束，不发请求；失败不说明。
  void run_skin_sync() {
    if (skin_sync_running_) {
      skin_sync_queued_ = true;
      return;
    }
    skin_sync_running_ = true;
    skin_sync_queued_ = false;
    run_host([] { return community_call(community_action(L"candidate_skin_sync")); },
             [this](Response const &response) {
               skin_sync_running_ = false;
               const auto value = community_value(response);
               if (response.ok && value.ValueType() == JsonValueType::Object) {
                 const auto report = value.GetObject();
                 if (!named_strings(report, L"downloaded").empty() ||
                     !named_strings(report, L"deleted_local").empty()) {
                   skins_stale_ = true;
                   // 已经切回「我的皮肤」时现在就重扫。
                   if (!skin_community_view_)
                     redraw_themes();
                 }
               }
               if (skin_sync_queued_ && skin_community_view_ && current_page_ == "themes")
                 run_skin_sync();
               skin_sync_queued_ = false;
             });
  }

  // 一键安装：先看本机有没有同名的皮肤，有就先问（省下一次约 3 MB 的下载）；看不了时由安装自己的 candidate_skin_exists 拦住覆盖。装好后「我的皮肤」重扫，再同步一次皮肤库。
  void install_community_skin(bool replace) {
    auto &gallery = skin_gallery_;
    if (!gallery.selected)
      return;
    gallery.installed = false;
    if (!gallery_begin<CommunitySkinItem>(true))
      return;
    const auto target = *gallery.selected;
    struct Outcome {
      bool exists = false;
      Response response;
    };
    const auto owner = gallery.owner;
    run_worker<Outcome>(
        [target, replace, root = skins_root()] {
          Outcome outcome;
          if (!replace) {
            if (const auto ids = read_skin_ids(root);
                ids && std::find(ids->begin(), ids->end(), target.package_id) != ids->end()) {
              outcome.exists = true;
              return outcome;
            }
          }
          auto action = community_action(L"candidate_skin_install");
          action.SetNamedValue(L"id", JsonValue::CreateStringValue(hstring(target.id)));
          action.SetNamedValue(L"replace", JsonValue::CreateBooleanValue(replace));
          outcome.response = community_call(action);
          return outcome;
        },
        [this, owner](Outcome &outcome) {
          // 皮肤已经写进目录时，即使图库已经换了，「我的皮肤」也该重扫。
          if (outcome.response.ok)
            skins_stale_ = true;
          if (owner != skin_gallery_.owner)
            return;
          if (outcome.exists) {
            skin_gallery_.confirm_replace = true;
            gallery_end<CommunitySkinItem>();
            return;
          }
          if (!outcome.response.ok) {
            const auto failure = community_failure(outcome.response);
            if (failure.code == L"candidate_skin_exists") {
              skin_gallery_.confirm_replace = true;
              gallery_end<CommunitySkinItem>();
            } else {
              gallery_end<CommunitySkinItem>(failure);
            }
            return;
          }
          skin_gallery_.confirm_replace = false;
          skin_gallery_.installed = true;
          gallery_end<CommunitySkinItem>();
          run_skin_sync();
        });
  }

  // 作者把自己的皮肤改成公开或私有。
  void change_community_skin_visibility(std::wstring visibility) {
    if (!gallery_begin<CommunitySkinItem>(true))
      return;
    auto action = community_action(L"candidate_skin_set_visibility");
    action.SetNamedValue(L"id", JsonValue::CreateStringValue(hstring(skin_gallery_.selected->id)));
    action.SetNamedValue(L"visibility", JsonValue::CreateStringValue(hstring(visibility)));
    gallery_call<CommunitySkinItem>(
        [action] { return community_call(action); },
        [this, visibility](Response const &response) {
          auto item = community_item<CommunitySkinItem>(response);
          if (!item) {
            gallery_end<CommunitySkinItem>(community_failure(response));
            return;
          }
          replace_selected_skin(std::move(*item));
          skin_gallery_.notice = visibility == L"public"
                                     ? L"已公开，其他用户现在可以下载这款皮肤。"
                                     : L"已设为私有，只有你能看到这款皮肤。";
          gallery_end<CommunitySkinItem>();
        });
  }

  // 作者改自己作品的发布分类。
  void change_community_skin_category(std::wstring category) {
    if (!gallery_begin<CommunitySkinItem>(true))
      return;
    auto action = community_action(L"candidate_skin_set_category");
    action.SetNamedValue(L"id", JsonValue::CreateStringValue(hstring(skin_gallery_.selected->id)));
    action.SetNamedValue(L"category", JsonValue::CreateStringValue(hstring(category)));
    gallery_call<CommunitySkinItem>(
        [action] { return community_call(action); },
        [this, category](Response const &response) {
          auto item = community_item<CommunitySkinItem>(response);
          if (!item) {
            gallery_end<CommunitySkinItem>(community_failure(response));
            return;
          }
          replace_selected_skin(std::move(*item));
          skin_gallery_.notice = L"已改为「" + cm::category_label(category) + L"」分类。";
          gallery_end<CommunitySkinItem>();
        });
  }

  // replaceSelected：详情和列表里的那一项一起换。
  void replace_selected_skin(CommunitySkinItem updated) {
    for (auto &item : skin_gallery_.items)
      if (item.id == updated.id)
        item = updated;
    skin_gallery_.selected = std::move(updated);
  }

  // 分类筛选（useCommunitySkinCategoryFilter）：换分类后从第一页重读；第一页读失败时列表和 offset 还是上一次成功的分类的，分类也退回去，否则「加载更多」会在旧分类的 offset 上接新分类的页。
  void change_skin_category(std::optional<std::wstring> next) {
    if (next == skin_category_request_)
      return;
    const auto previous = skin_category_active_;
    skin_category_request_ = next;
    skin_category_shown_ = next;
    gallery_list<CommunitySkinItem>(
        skin_gallery_.active_search, false, skin_gallery_.active_mine,
        [this, next, previous](bool listed) {
          if (skin_category_request_ != next)
            return;
          if (listed) {
            skin_category_active_ = next;
          } else {
            skin_category_request_ = previous;
            skin_category_shown_ = previous;
            redraw_themes();
          }
        });
  }

  // 插件类型筛选：与皮肤分类一样，第一页读失败时类型退回去。
  void change_plugin_kind(std::optional<std::wstring> next) {
    const auto previous = plugin_kind_request_;
    if (next == previous)
      return;
    plugin_kind_request_ = next;
    plugin_kind_shown_ = next;
    gallery_list<CommunityPluginItem>(
        plugin_gallery_.active_search, false, plugin_gallery_.active_mine,
        [this, next, previous](bool listed) {
          if (!listed && plugin_kind_request_ == next) {
            plugin_kind_request_ = previous;
            plugin_kind_shown_ = previous;
            redraw_plugins();
          }
        });
  }

  // 一键安装插件：同 kind 同 id 的包会被整个替换，所以先问；读不了本机目录时照常安装，与从本机 .zip 导入一样。
  void install_community_plugin(bool replace) {
    auto &gallery = plugin_gallery_;
    if (!gallery.selected)
      return;
    gallery.installed = false;
    if (!gallery_begin<CommunityPluginItem>(true))
      return;
    const auto target = *gallery.selected;
    struct Outcome {
      bool exists = false;
      Response response;
    };
    const auto owner = gallery.owner;
    run_worker<Outcome>(
        [target, replace] {
          Outcome outcome;
          if (!replace) {
            if (const auto catalog = plugin_catalog(plugin_catalog_call()))
              for (auto const &pack : catalog->packages)
                if (pack.kind == target.kind && pack.id == target.plugin_id) {
                  outcome.exists = true;
                  return outcome;
                }
          }
          auto action = community_action(L"plugin_install");
          action.SetNamedValue(L"id", JsonValue::CreateStringValue(hstring(target.id)));
          action.SetNamedValue(L"kind", JsonValue::CreateStringValue(hstring(target.kind)));
          action.SetNamedValue(L"plugin_id", JsonValue::CreateStringValue(hstring(target.plugin_id)));
          outcome.response = community_call(action);
          return outcome;
        },
        [this, owner](Outcome &outcome) {
          if (outcome.response.ok)
            plugins_stale_ = true;
          if (owner != plugin_gallery_.owner)
            return;
          if (outcome.exists) {
            plugin_gallery_.confirm_replace = true;
            gallery_end<CommunityPluginItem>();
            return;
          }
          if (!outcome.response.ok) {
            gallery_end<CommunityPluginItem>(community_failure(outcome.response));
            return;
          }
          plugin_gallery_.confirm_replace = false;
          plugin_gallery_.installed = true;
          gallery_end<CommunityPluginItem>();
        });
  }

  // ---- 预览图 ----

  // 社区皮肤的预览图：每款读一次，卡片和详情共用（共享页的 loadPreview）；最多留 16 张，再加上当前列表里的那些，免得列表一重画就把刚读到的又读一遍。读到后就地换上正在显示它的卡片，不重画整页。
  void show_community_preview(Border const &host, std::wstring const &id, std::wstring const &name) {
    community_preview_views_.emplace(id, make_weak(host));
    auto found = community_previews_.find(id);
    if (found == community_previews_.end()) {
      community_previews_.emplace(id, CommunityPreview{});
      community_preview_order_.push_back(id);
      trim_community_previews();
      load_community_preview(id);
      fill_community_preview(host, CommunityPreview{}, name);
      return;
    }
    // 最近用过的排到最后。
    std::erase(community_preview_order_, id);
    community_preview_order_.push_back(id);
    fill_community_preview(host, found->second, name);
  }

  void trim_community_previews() {
    const size_t capacity = 16 + skin_gallery_.items.size();
    while (community_preview_order_.size() > capacity) {
      community_previews_.erase(community_preview_order_.front());
      community_preview_order_.erase(community_preview_order_.begin());
    }
  }

  void fill_community_preview(Border const &host, CommunityPreview const &preview,
                              std::wstring const &name) {
    if (preview.image) {
      Image image;
      image.Source(preview.image);
      image.Stretch(Stretch::Uniform);
      image.MaxHeight(220);
      A11y::SetName(image, hstring(name + L" 预览"));
      host.Child(image);
      return;
    }
    auto status = make_text(preview.failed ? L"预览图加载失败" : L"正在读取预览…", 12, palette_.sub);
    status.HorizontalAlignment(HorizontalAlignment::Center);
    status.VerticalAlignment(VerticalAlignment::Center);
    host.Child(status);
  }

  // 读一张预览图：宿主回答 PNG 或 JPEG 的 data: 地址（类型和文件头已校验），解码后交给正在显示它的卡片。
  void load_community_preview(std::wstring const &id) {
    auto action = community_action(L"candidate_skin_preview");
    action.SetNamedValue(L"id", JsonValue::CreateStringValue(hstring(id)));
    run_host([action] { return community_call(action); },
             [this, id](Response const &response) {
               std::wstring url;
               if (const auto value = community_value(response);
                   response.ok && value.ValueType() == JsonValueType::Object)
                 url = named_string(value.GetObject(), L"dataUrl");
               decode_community_preview(id, std::move(url));
             });
  }

  fire_and_forget decode_community_preview(std::wstring id, std::wstring url) {
    auto strong = get_strong();
    ImageSource source{nullptr};
    const auto comma = url.find(L',');
    if (url.starts_with(L"data:") && comma != std::wstring::npos) {
      try {
        const auto buffer = Windows::Security::Cryptography::CryptographicBuffer::
            DecodeFromBase64String(hstring(url.substr(comma + 1)));
        Windows::Storage::Streams::InMemoryRandomAccessStream stream;
        co_await stream.WriteAsync(buffer);
        stream.Seek(0);
        Imaging::BitmapImage bitmap;
        co_await bitmap.SetSourceAsync(stream);
        source = bitmap;
      } catch (hresult_error const &) {
        source = nullptr;
      }
    }
    if (closed_)
      co_return;
    const auto found = community_previews_.find(id);
    if (found == community_previews_.end())
      co_return;
    found->second.image = source;
    found->second.failed = !source;
    std::wstring name;
    for (auto const &item : skin_gallery_.items)
      if (item.id == id)
        name = item.name;
    if (skin_gallery_.selected && skin_gallery_.selected->id == id)
      name = skin_gallery_.selected->name;
    const auto [first, last] = community_preview_views_.equal_range(id);
    for (auto it = first; it != last; ++it)
      if (auto host = it->second.get())
        fill_community_preview(host, found->second, name);
  }

  // ---- 图库的界面 ----

  // 出错说明（CommunityErrorAlert）：未登录时后面跟一个「去登录」。
  void community_error(StackPanel const &panel, std::wstring const &message, bool sign_in,
                       std::function<void()> login) {
    StackPanel line;
    line.Orientation(Orientation::Horizontal);
    line.Spacing(8);
    auto words = make_text(message, 13, danger_color());
    words.VerticalAlignment(VerticalAlignment::Center);
    words.MaxWidth(560);
    A11y::SetLiveSetting(words, Automation::Peers::AutomationLiveSetting::Assertive);
    line.Children().Append(words);
    if (sign_in && login)
      line.Children().Append(button_control(L"去登录", std::move(login)));
    line.Margin(Thickness{4, 4, 4, 8});
    panel.Children().Append(line);
  }

  // 完成一个操作后的说明（CommunityActionNotice）。
  void community_notice(StackPanel const &panel, std::wstring const &message) {
    auto words = make_text(message, 13, palette_.accent);
    A11y::SetLiveSetting(words, Automation::Peers::AutomationLiveSetting::Polite);
    words.Margin(Thickness{4, 4, 4, 8});
    panel.Children().Append(words);
  }

  // 一行带强调色的主按钮。
  FrameworkElement primary_button(std::wstring const &label, std::function<void()> click,
                                  bool enabled = true) {
    auto button = button_control(label, std::move(click), enabled).as<Button>();
    make_primary(button);
    return button;
  }

  // 一排按下其中一个的筛选按钮（「全部」加各个分类或类型），选中的是主按钮。
  void community_filter(StackPanel const &panel, std::wstring const &name,
                        std::vector<std::pair<std::optional<std::wstring>, std::wstring>> const &choices,
                        std::optional<std::wstring> const &current,
                        std::function<void(std::optional<std::wstring>)> choose) {
    // VariableSizedWrapGrid 不给格子大小时每格都取第一个按钮（「全部」）的大小，「科技夜色」「辅助码表」会被截掉；格子定宽，按钮铺满格子。
    VariableSizedWrapGrid row;
    row.Orientation(Orientation::Horizontal);
    row.ItemWidth(104);
    row.ItemHeight(40);
    row.Margin(Thickness{0, 4, 0, 8});
    for (auto const &[value, label] : choices) {
      const bool on = value == current;
      auto button = button_control(label, [choose, value] { choose(value); }).as<Button>();
      button.Margin(Thickness{0, 0, 8, 8});
      button.MinWidth(0);
      button.HorizontalAlignment(HorizontalAlignment::Stretch);
      if (on) {
        make_primary(button);
        A11y::SetItemStatus(button, L"已选择");
      }
      row.Children().Append(button);
    }
    A11y::SetName(row, hstring(name));
    panel.Children().Append(row);
  }

  // 搜索框和「搜索」按钮（CommunitySearchForm）：最多 128 个字，回车或「搜索」从第一页读这个词。
  template <typename Item> void community_search(StackPanel const &panel, std::wstring const &label) {
    TextBox box;
    box.Width(320);
    box.PlaceholderText(hstring(label));
    box.Text(hstring(gallery_for<Item>().search));
    A11y::SetName(box, hstring(label));
    box.TextChanged([this](Inspectable const &sender, TextChangedEventArgs const &) {
      const auto input = sender.as<TextBox>();
      const std::wstring typed(input.Text().c_str());
      const auto bounded = cm::bounded_graphemes(typed, cm::search_limit);
      gallery_for<Item>().search = bounded;
      if (bounded != typed) {
        input.Text(hstring(bounded));
        input.SelectionStart(static_cast<int32_t>(bounded.size()));
      }
    });
    auto submit = [this] {
      auto &gallery = gallery_for<Item>();
      gallery_list<Item>(gallery.search, false, gallery.active_mine);
    };
    box.KeyDown([submit](Inspectable const &, Input::KeyRoutedEventArgs const &args) {
      if (args.Key() == Windows::System::VirtualKey::Enter) {
        args.Handled(true);
        submit();
      }
    });
    StackPanel row;
    row.Orientation(Orientation::Horizontal);
    row.Spacing(8);
    row.Margin(Thickness{0, 0, 0, 8});
    row.Children().Append(box);
    row.Children().Append(button_control(L"搜索", submit));
    panel.Children().Append(row);
  }

  // 图库的标题（CommunityGalleryHeading）：标题和说明，右边是「全部 / 我的作品」和发布按钮。
  template <typename Item>
  void community_heading(StackPanel const &panel, std::wstring const &title, std::wstring const &note,
                         std::wstring const &scope_name, std::wstring const &all_label,
                         std::wstring const &publish_label, std::function<void()> publish) {
    Grid heading;
    heading.ColumnSpacing(16);
    ColumnDefinition text_column;
    text_column.Width(GridLength{1, GridUnitType::Star});
    ColumnDefinition action_column;
    action_column.Width(GridLength{1, GridUnitType::Auto});
    heading.ColumnDefinitions().Append(text_column);
    heading.ColumnDefinitions().Append(action_column);
    StackPanel texts;
    texts.Spacing(2);
    auto head = make_text(title, 20, palette_.text);
    head.FontWeight(Windows::UI::Text::FontWeight{600});
    texts.Children().Append(head);
    texts.Children().Append(make_text(note, 12, palette_.sub));
    heading.Children().Append(texts);
    StackPanel actions;
    actions.Orientation(Orientation::Horizontal);
    actions.Spacing(8);
    actions.VerticalAlignment(VerticalAlignment::Center);
    if constexpr (resource_item<Item>) {
      // CommunityResourceScopeButtons：全部、收藏、我的作品，换了就从第一页读。
      std::vector<Option> scopes;
      for (auto const &scope : cm::resource_scopes)
        scopes.push_back({scope.value.empty() ? std::wstring(L"all") : std::wstring(scope.value),
                          std::wstring(scope.label)});
      actions.Children().Append(segmented_control(
          scope_name, std::move(scopes), resource_scope_.empty() ? L"all" : resource_scope_,
          [this](std::wstring const &value) {
            resource_scope_ = value == L"all" ? L"" : value;
            gallery_list<Item>(resource_gallery_.active_search, false, false);
          },
          true, false));
    } else {
      const bool mine = gallery_for<Item>().mine;
      actions.Children().Append(segmented_control(
          scope_name, {{L"all", all_label}, {L"mine", L"我的作品"}}, mine ? L"mine" : L"all",
          [this](std::wstring const &value) {
            auto &gallery = gallery_for<Item>();
            gallery.mine = value == L"mine";
            gallery_list<Item>(gallery.active_search, false, gallery.mine);
          },
          true, false));
    }
    actions.Children().Append(primary_button(publish_label, std::move(publish)));
    Grid::SetColumn(actions, 1);
    heading.Children().Append(actions);
    heading.Margin(Thickness{0, 8, 0, 8});
    panel.Children().Append(heading);
  }

  // 卡片的网格（CommunityGalleryGrid）：整张卡片是一个按钮，点开详情。
  VariableSizedWrapGrid community_grid(double width, double height) {
    VariableSizedWrapGrid grid;
    grid.Orientation(Orientation::Horizontal);
    grid.ItemWidth(width + 12);
    grid.ItemHeight(height + 12);
    return grid;
  }

  Button community_card(std::wstring const &name, StackPanel const &body, double width,
                        double height, std::function<void()> open) {
    Button card;
    card.Width(width);
    card.Height(height);
    card.Padding(Thickness{12, 12, 12, 12});
    card.HorizontalContentAlignment(HorizontalAlignment::Stretch);
    card.VerticalContentAlignment(VerticalAlignment::Top);
    card.Background(brush(palette_.card));
    card.BorderBrush(brush(palette_.stroke));
    card.BorderThickness(Thickness{1, 1, 1, 1});
    card.CornerRadius(CornerRadius{6, 6, 6, 6});
    card.Content(body);
    A11y::SetName(card, hstring(name));
    card.Click([open = std::move(open)](Inspectable const &, RoutedEventArgs const &) { open(); });
    return card;
  }

  TextBlock card_line(std::wstring const &value, double size, Color const &color, int lines = 1) {
    auto line = make_text(value, size, color);
    line.TextTrimming(TextTrimming::CharacterEllipsis);
    line.MaxLines(lines);
    return line;
  }

  // CommunityCardMetrics：下载量和评分。
  TextBlock card_metrics(double downloads, double rating_count, double rating_average) {
    return card_line(L"↓ " + cm::zh_number(downloads) + L"    ☆ " +
                         cm::rating(rating_count, rating_average),
                     12, palette_.sub);
  }

  // 加载更多（CommunityGalleryLoadMore）和读取中的一句。
  template <typename Item> void community_load_more(StackPanel const &panel, std::wstring const &loading) {
    auto &gallery = gallery_for<Item>();
    if (gallery.has_more) {
      auto more = button_control(L"加载更多", [this] {
        auto &current = gallery_for<Item>();
        gallery_list<Item>(current.active_search, true, current.active_mine);
      }, !gallery.list_busy);
      more.HorizontalAlignment(HorizontalAlignment::Center);
      more.Margin(Thickness{0, 8, 0, 0});
      panel.Children().Append(more);
    }
    if (gallery.list_busy) {
      auto status = make_text(loading, 13, palette_.sub);
      status.HorizontalAlignment(HorizontalAlignment::Center);
      status.Margin(Thickness{0, 8, 0, 0});
      A11y::SetLiveSetting(status, Automation::Peers::AutomationLiveSetting::Polite);
      panel.Children().Append(status);
    }
  }

  // 空列表的一句（CommunityNotice）。
  void community_empty(StackPanel const &panel, std::wstring const &message) {
    auto words = make_text(message, 13, palette_.sub);
    words.Margin(Thickness{4, 8, 4, 8});
    panel.Children().Append(words);
  }

  // ---- 详情 ----

  // 详情的外框（CommunityDetailFrame）：「← 社区」、出错说明和放内容的卡片。返回卡片里的那一列。
  template <typename Item> StackPanel community_detail_frame(StackPanel const &page) {
    auto &gallery = gallery_for<Item>();
    auto back = button_control(L"← 社区", [this] { gallery_close<Item>(); }, !gallery.action_busy)
                    .as<Button>();
    A11y::SetName(back, L"返回社区");
    back.HorizontalAlignment(HorizontalAlignment::Left);
    page.Children().Append(back);
    if (!gallery.error.empty())
      community_error(page, gallery.error, gallery.sign_in, [this] { open_account_login(); });
    StackPanel body;
    body.Spacing(10);
    body.Padding(Thickness{25, 18, 25, 18});
    auto card = make_card();
    card.Margin(Thickness{0, 8, 0, 0});
    card.Child(body);
    page.Children().Append(card);
    return body;
  }

  // 标题、作者一行、「我的作品」和「已下架」标记、说明（CommunityDetailHeader）。
  void community_detail_header(StackPanel const &body, std::wstring const &title,
                               std::wstring const &note, bool owned,
                               std::wstring const &moderation, std::wstring const &owned_label,
                               std::wstring const &description) {
    StackPanel heading;
    heading.Orientation(Orientation::Horizontal);
    heading.Spacing(10);
    auto name = make_text(title, 20, palette_.text);
    name.FontWeight(Windows::UI::Text::FontWeight{600});
    heading.Children().Append(name);
    auto badge = [this, &heading](std::wstring const &label, std::wstring const &tip) {
      Border mark;
      mark.Padding(Thickness{8, 2, 8, 2});
      mark.CornerRadius(CornerRadius{10, 10, 10, 10});
      mark.BorderBrush(brush(palette_.stroke));
      mark.BorderThickness(Thickness{1, 1, 1, 1});
      mark.VerticalAlignment(VerticalAlignment::Center);
      mark.Child(make_text(label, 12, palette_.sub));
      if (!tip.empty())
        ToolTipService::SetToolTip(mark, box_value(hstring(tip)));
      heading.Children().Append(mark);
    };
    if (owned)
      badge(owned_label, L"");
    // 审核只显示「已下架」，不说原因，也不显示待审。
    if (owned && moderation == L"removed")
      badge(L"已下架", L"已被管理员下架，其他用户看不到这个作品");
    body.Children().Append(heading);
    if (!note.empty())
      body.Children().Append(make_text(note, 13, palette_.sub));
    if (!description.empty()) {
      auto text_block = make_text(description, 14, palette_.text);
      text_block.IsTextSelectionEnabled(true);
      body.Children().Append(text_block);
    }
  }

  // 下载、评分、我的评分、读取中和操作结果（CommunityDetailStatus）。
  template <typename Item> void community_detail_status(StackPanel const &body, std::wstring const &loading) {
    auto &gallery = gallery_for<Item>();
    auto const &item = *gallery.selected;
    body.Children().Append(make_text(
        cm::rating_metrics(item.downloads, L"下载", item.rating_count, item.rating_average), 13,
        palette_.sub));
    if (item.my_rating > 0)
      body.Children().Append(
          make_text(L"我的评分：" + std::to_wstring(item.my_rating) + L" 星", 13, palette_.sub));
    if (gallery.detail_busy) {
      auto status = make_text(loading, 13, palette_.sub);
      A11y::SetLiveSetting(status, Automation::Peers::AutomationLiveSetting::Polite);
      body.Children().Append(status);
    }
    if (!gallery.notice.empty())
      community_notice(body, gallery.notice);
  }

  // 一段确认（CommunityConfirmation）：说明，危险的确认按钮和「取消」。
  void community_confirmation(StackPanel const &body, std::wstring const &name,
                              std::wstring const &message, std::wstring const &confirm_label,
                              bool busy, std::function<void()> confirm, std::function<void()> cancel) {
    StackPanel panel;
    panel.Spacing(10);
    panel.Padding(Thickness{16, 12, 16, 12});
    panel.Children().Append(make_text(message, 13, palette_.text));
    StackPanel actions;
    actions.Orientation(Orientation::Horizontal);
    actions.Spacing(8);
    auto yes = button_control(confirm_label, std::move(confirm), !busy).as<Button>();
    yes.Foreground(brush(danger_color()));
    actions.Children().Append(yes);
    actions.Children().Append(button_control(L"取消", std::move(cancel), !busy));
    panel.Children().Append(actions);
    Border frame;
    frame.BorderBrush(brush(danger_color()));
    frame.BorderThickness(Thickness{1, 1, 1, 1});
    frame.CornerRadius(CornerRadius{4, 4, 4, 4});
    frame.Child(panel);
    A11y::SetName(frame, hstring(name));
    body.Children().Append(frame);
  }

  // 评分、作者的下架和确认（CommunityModerationSection）。
  template <typename Item>
  void community_moderation(StackPanel const &body, bool unpublishable, std::wstring const &unpublish_label,
                            std::wstring const &confirm_name, std::wstring const &message,
                            std::wstring const &done) {
    auto &gallery = gallery_for<Item>();
    auto const &item = *gallery.selected;
    if (!item.owned) {
      StackPanel rating;
      rating.Spacing(6);
      // 词库与回复模板不用先安装（ResourceDetail 的 ratingDescription）。
      rating.Children().Append(make_text(resource_item<Item> ? L"我的评分（可重新选择）"
                                                             : L"我的评分（安装后可评，可重新选择）",
                                         12, palette_.sub));
      StackPanel stars;
      stars.Orientation(Orientation::Horizontal);
      stars.Spacing(6);
      for (int value = 1; value <= 5; ++value) {
        auto button = button_control(std::to_wstring(value) + L" 星",
                                     [this, value] { gallery_rate<Item>(value); },
                                     !gallery.action_busy)
                          .as<Button>();
        A11y::SetName(button, hstring(L"评 " + std::to_wstring(value) + L" 星"));
        stars.Children().Append(button);
      }
      rating.Children().Append(stars);
      A11y::SetName(rating, L"我的评分");
      body.Children().Append(rating);
    }
    if (item.owned && unpublishable) {
      auto button = button_control(unpublish_label, [this] {
        gallery_for<Item>().confirm_unpublish = true;
        redraw_gallery<Item>();
      }, !gallery.action_busy).as<Button>();
      button.Foreground(brush(danger_color()));
      body.Children().Append(button);
    }
    if (gallery.confirm_unpublish)
      community_confirmation(
          body, confirm_name, message, L"确认下架", gallery.action_busy,
          [this, done] { gallery_unpublish<Item>(done); },
          [this] {
            gallery_for<Item>().confirm_unpublish = false;
            redraw_gallery<Item>();
          });
  }

  // 别人的作品下面的「举报」（CommunityReportSection）：从固定的原因里选一个，补充说明可不填。
  template <typename Item> void community_report(StackPanel const &body) {
    auto &gallery = gallery_for<Item>();
    if (!gallery.report_open) {
      auto button = button_control(L"举报", [this] {
        gallery_for<Item>().report_open = true;
        redraw_gallery<Item>();
      }, !gallery.action_busy).as<Button>();
      button.Foreground(brush(danger_color()));
      body.Children().Append(button);
      return;
    }
    StackPanel form;
    form.Spacing(8);
    form.Padding(Thickness{16, 12, 16, 12});
    form.Children().Append(make_text(L"请选择举报原因", 13, palette_.text));
    for (auto const reason : cm::report_reasons) {
      RadioButton choice;
      choice.Content(box_value(hstring(reason)));
      choice.GroupName(L"community-report-reason");
      choice.IsChecked(gallery.report_reason == reason);
      choice.IsEnabled(!gallery.action_busy);
      choice.Checked([this, value = std::wstring(reason)](Inspectable const &, RoutedEventArgs const &) {
        gallery_for<Item>().report_reason = value;
        redraw_gallery<Item>();
      });
      form.Children().Append(choice);
    }
    TextBox detail;
    detail.Header(box_value(L"补充说明（选填）"));
    detail.AcceptsReturn(true);
    detail.TextWrapping(TextWrapping::Wrap);
    detail.MinHeight(72);
    detail.MaxLength(static_cast<int32_t>(cm::report_detail_limit));
    detail.Text(hstring(gallery.report_detail));
    detail.IsEnabled(!gallery.action_busy);
    A11y::SetName(detail, L"举报补充说明");
    detail.TextChanged([this](Inspectable const &sender, TextChangedEventArgs const &) {
      gallery_for<Item>().report_detail = cm::line_feeds(sender.as<TextBox>().Text().c_str());
    });
    form.Children().Append(detail);
    StackPanel actions;
    actions.Orientation(Orientation::Horizontal);
    actions.Spacing(8);
    auto submit = button_control(L"提交举报", [this] { gallery_report<Item>(); },
                                 !gallery.action_busy && !gallery.report_reason.empty())
                      .as<Button>();
    submit.Foreground(brush(danger_color()));
    actions.Children().Append(submit);
    actions.Children().Append(button_control(L"取消", [this] {
      auto &current = gallery_for<Item>();
      current.report_open = false;
      current.report_reason.clear();
      current.report_detail.clear();
      redraw_gallery<Item>();
    }, !gallery.action_busy));
    form.Children().Append(actions);
    Border frame;
    frame.BorderBrush(brush(palette_.stroke));
    frame.BorderThickness(Thickness{1, 1, 1, 1});
    frame.CornerRadius(CornerRadius{4, 4, 4, 4});
    frame.Child(form);
    A11y::SetName(frame, L"举报作品");
    body.Children().Append(frame);
  }

  // 一键安装（CommunityInstallButton）。
  template <typename Item> FrameworkElement community_install_button(std::function<void()> install) {
    auto &gallery = gallery_for<Item>();
    auto button = primary_button(gallery.action_busy ? L"正在安装…" : L"一键安装", std::move(install),
                                 !gallery.action_busy && !gallery.detail_busy && !gallery.confirm_replace);
    button.HorizontalAlignment(HorizontalAlignment::Left);
    return button;
  }

  // ---- 社区皮肤 ----

  void build_skin_community(StackPanel const &page) {
    if (skin_gallery_.selected) {
      build_skin_community_detail(page);
      return;
    }
    community_preview_views_.clear();
    community_search<CommunitySkinItem>(page, L"搜索候选窗口皮肤");
    community_heading<CommunitySkinItem>(
        page, L"社区皮肤", L"为输入候选窗口换一身新装，安装后在「我的皮肤」中启用",
        L"候选窗口皮肤范围", L"全部", L"发布我的皮肤", [this] { open_skin_publish(L"", false); });
    std::vector<std::pair<std::optional<std::wstring>, std::wstring>> categories{
        {std::nullopt, L"全部"}};
    for (auto const &item : cm::skin_categories)
      categories.emplace_back(std::wstring(item.value), std::wstring(item.label));
    community_filter(page, L"候选窗口皮肤分类", categories, skin_category_shown_,
                     [this](std::optional<std::wstring> next) { change_skin_category(next); });
    auto &gallery = skin_gallery_;
    if (!gallery.error.empty())
      community_error(page, gallery.error, gallery.sign_in, [this] { open_account_login(); });
    if (!gallery.notice.empty())
      community_notice(page, gallery.notice);
    if (gallery.error.empty() && !gallery.list_busy && gallery.items.empty())
      community_empty(page, gallery.mine ? L"你的皮肤库里还没有候选窗口皮肤。"
                                         : L"暂时没有匹配的候选窗口皮肤。");
    constexpr double width = 232, height = 252;
    auto grid = community_grid(width, height);
    for (auto const &skin : gallery.items) {
      StackPanel body;
      body.Spacing(4);
      Border stage;
      stage.Height(120);
      stage.CornerRadius(CornerRadius{4, 4, 4, 4});
      stage.Background(brush(palette_.subtle2));
      show_community_preview(stage, skin.id, skin.name);
      body.Children().Append(stage);
      auto title = card_line(skin.name, 14, palette_.text);
      title.FontWeight(Windows::UI::Text::FontWeight{600});
      body.Children().Append(title);
      body.Children().Append(card_line(cm::card_author(cm::category_label(skin.category), skin.author,
                                                       skin.owned, skin.visibility == L"private",
                                                       skin.moderation == L"removed"),
                                       12, palette_.sub));
      body.Children().Append(card_metrics(skin.downloads, skin.rating_count, skin.rating_average));
      if (const auto assets = cm::trim(skin.license_assets); !assets.empty())
        body.Children().Append(card_line(L"素材授权 " + assets, 12, palette_.sub));
      grid.Children().Append(community_card(L"查看候选窗口皮肤 " + skin.name, body, width, height,
                                            [this, skin] { gallery_open(skin); }));
    }
    page.Children().Append(grid);
    community_load_more<CommunitySkinItem>(page, L"正在读取候选窗口皮肤…");
  }

  void build_skin_community_detail(StackPanel const &page) {
    community_preview_views_.clear();
    auto &gallery = skin_gallery_;
    auto const selected = *gallery.selected;
    auto body = community_detail_frame<CommunitySkinItem>(page);
    Border stage;
    stage.MinHeight(96);
    stage.CornerRadius(CornerRadius{4, 4, 4, 4});
    stage.Background(brush(palette_.subtle2));
    stage.Padding(Thickness{8, 8, 8, 8});
    show_community_preview(stage, selected.id, selected.name);
    body.Children().Append(stage);
    community_detail_header(
        body, selected.name,
        cm::join_dot({cm::category_label(selected.category), selected.author,
                      selected.version.empty() ? L"" : L"v" + selected.version}),
        selected.owned, selected.moderation,
        selected.visibility == L"private" ? L"私有" : L"我的作品", selected.description);
    if (const auto license = cm::license_line(selected.license_assets, selected.license_code,
                                              selected.license_source);
        !license.empty())
      body.Children().Append(make_text(license, 13, palette_.sub));
    community_detail_status<CommunitySkinItem>(body, L"正在读取皮肤详情…");
    if (gallery.installed) {
      community_notice(body, L"已安装到外部皮肤。");
      auto enable = primary_button(L"去启用", [this] { set_skin_view(false); });
      enable.HorizontalAlignment(HorizontalAlignment::Left);
      body.Children().Append(enable);
    } else {
      body.Children().Append(community_install_button<CommunitySkinItem>(
          [this] { install_community_skin(false); }));
    }
    if (gallery.confirm_replace)
      community_confirmation(
          body, L"确认替换皮肤",
          L"已存在同名皮肤“" + selected.package_id + L"”，安装会整体替换它。", L"替换安装",
          gallery.action_busy, [this] { install_community_skin(true); },
          [this] {
            skin_gallery_.confirm_replace = false;
            redraw_themes();
          });
    if (selected.owned) {
      const bool is_private = selected.visibility == L"private";
      auto visibility = button_control(
          is_private ? L"公开" : L"设为私有",
          [this, is_private] { change_community_skin_visibility(is_private ? L"public" : L"private"); },
          !gallery.action_busy && !gallery.detail_busy);
      visibility.HorizontalAlignment(HorizontalAlignment::Left);
      body.Children().Append(visibility);
      body.Children().Append(skin_category_select(
          L"修改分类", selected.category.empty() ? L"other" : selected.category,
          !gallery.action_busy && !gallery.detail_busy,
          [this](std::wstring const &next) { change_community_skin_category(next); }));
    }
    community_moderation<CommunitySkinItem>(
        body, selected.visibility == L"public", L"下架这款皮肤", L"确认下架皮肤",
        L"下架后其他用户无法再下载，下载数和评分会清空；本地皮肤会保留并以私有方式同步。只想不让别人看到，可以改用“设为私有”。确定下架“" +
            selected.name + L"”吗？",
        L"已下架这款皮肤；其他用户将无法再下载。本地皮肤会保留，并在下次同步时作为私有皮肤存回你的皮肤库。");
    if (!selected.owned)
      community_report<CommunitySkinItem>(body);
  }

  // 分类下拉框（CommunitySkinCategorySelect），发布对话框和作者改分类共用。
  FrameworkElement skin_category_select(std::wstring const &name, std::wstring const &current,
                                        bool enabled, std::function<void(std::wstring const &)> choose) {
    std::vector<Option> options;
    for (auto const &item : cm::skin_categories)
      options.push_back({std::wstring(item.value), std::wstring(item.label)});
    auto combo = select_control(name, std::move(options), current, std::move(choose), enabled, false)
                     .as<ComboBox>();
    combo.Header(box_value(L"分类"));
    combo.HorizontalAlignment(HorizontalAlignment::Left);
    return combo;
  }

  // ---- 社区插件 ----

  void build_plugin_community(StackPanel const &page) {
    if (plugin_gallery_.selected) {
      build_plugin_community_detail(page);
      return;
    }
    community_search<CommunityPluginItem>(page, L"搜索插件");
    community_heading<CommunityPluginItem>(
        page, L"社区插件",
        L"音效、音乐、指令表、特效、短语表、辅助码表、单词本与符号集，安装后在「我的插件」中选用",
        L"插件范围", L"全部插件", L"发布我的插件", [this] { open_plugin_publish(); });
    std::vector<std::pair<std::optional<std::wstring>, std::wstring>> kinds{{std::nullopt, L"全部"}};
    for (auto const kind : cm::plugin_kinds)
      kinds.emplace_back(std::wstring(kind), pg::kind_label(kind));
    community_filter(page, L"插件类型", kinds, plugin_kind_shown_,
                     [this](std::optional<std::wstring> next) { change_plugin_kind(next); });
    auto &gallery = plugin_gallery_;
    if (!gallery.error.empty())
      community_error(page, gallery.error, gallery.sign_in, [this] { open_account_login(); });
    if (!gallery.notice.empty())
      community_notice(page, gallery.notice);
    if (gallery.error.empty() && !gallery.list_busy && gallery.items.empty())
      community_empty(page, cm::plugin_empty_notice(
                                gallery.has_more,
                                !gallery.active_search.empty() || plugin_kind_shown_.has_value(),
                                gallery.mine, true));
    constexpr double width = 232, height = 132;
    auto grid = community_grid(width, height);
    for (auto const &plugin : gallery.items) {
      StackPanel body;
      body.Spacing(4);
      auto title = card_line(plugin.name, 14, palette_.text);
      title.FontWeight(Windows::UI::Text::FontWeight{600});
      body.Children().Append(title);
      body.Children().Append(card_line(cm::card_author(pg::kind_label(plugin.kind), plugin.author,
                                                       plugin.owned, false,
                                                       plugin.moderation == L"removed"),
                                       12, palette_.sub));
      if (!plugin.description.empty())
        body.Children().Append(card_line(plugin.description, 12, palette_.sub, 2));
      body.Children().Append(card_metrics(plugin.downloads, plugin.rating_count, plugin.rating_average));
      grid.Children().Append(community_card(L"查看插件 " + plugin.name, body, width, height,
                                            [this, plugin] { gallery_open(plugin); }));
    }
    page.Children().Append(grid);
    community_load_more<CommunityPluginItem>(page, L"正在读取插件…");
  }

  void build_plugin_community_detail(StackPanel const &page) {
    auto &gallery = plugin_gallery_;
    auto const selected = *gallery.selected;
    auto body = community_detail_frame<CommunityPluginItem>(page);
    community_detail_header(body, selected.name,
                            cm::join_dot({pg::kind_label(selected.kind), selected.author,
                                          selected.version.empty() ? L"" : L"v" + selected.version}),
                            selected.owned, selected.moderation, L"我的作品", selected.description);
    body.Children().Append(make_text(
        cm::join_dot({selected.plugin_id, selected.license.empty() ? L"" : L"授权 " + selected.license,
                      cm::megabytes(selected.size)}),
        13, palette_.sub));
    community_detail_status<CommunityPluginItem>(body, L"正在读取插件详情…");
    if (gallery.installed)
      community_notice(body, L"已安装到插件目录，可在「我的插件」中选用。");
    else
      body.Children().Append(community_install_button<CommunityPluginItem>(
          [this] { install_community_plugin(false); }));
    if (gallery.confirm_replace)
      community_confirmation(body, L"确认替换插件",
                             L"已安装同 id 的" + pg::kind_label(selected.kind) + L"“" +
                                 selected.plugin_id + L"”，安装会整体替换它。",
                             L"替换安装", gallery.action_busy,
                             [this] { install_community_plugin(true); },
                             [this] {
                               plugin_gallery_.confirm_replace = false;
                               redraw_plugins();
                             });
    community_moderation<CommunityPluginItem>(
        body, true, L"下架这个插件", L"确认下架插件",
        L"下架后其他用户无法再下载，下载数和评分会清空；已安装的插件会保留。确定下架“" + selected.name +
            L"”吗？",
        L"已下架这个插件；其他用户将无法再下载。已安装的插件会保留。");
    if (!selected.owned)
      community_report<CommunityPluginItem>(body);
  }

  // ---- 发布 ----

  // 发布对话框里名称、说明和权利确认（CommunityPublicationMetadataFields）。名称最多 32 个字，说明最多 280 个字；改了名称或说明就是另一次发布，换一个发布 id。输入时不重画对话框，只更新主按钮能不能按。`agreement_shown` 为假时没有权利确认（showAgreement，发布已有作品的新版本时）。
  void publication_fields(StackPanel const &form, std::wstring &name, std::wstring &description,
                          bool &agreed, std::wstring &publication_id, bool busy,
                          std::wstring const &name_label, std::wstring const &name_a11y,
                          std::wstring const &description_label, std::wstring const &description_a11y,
                          std::wstring const &agreement, std::wstring const &agreement_a11y,
                          std::function<void()> changed, std::function<void()> submit,
                          bool agreement_shown = true) {
    TextBox name_box;
    name_box.Header(box_value(hstring(name_label)));
    name_box.MaxLength(static_cast<int32_t>(cm::name_limit));
    name_box.Text(hstring(name));
    name_box.IsEnabled(!busy);
    A11y::SetName(name_box, hstring(name_a11y));
    name_box.TextChanged([&name, &publication_id, changed](Inspectable const &sender,
                                                           TextChangedEventArgs const &) {
      const auto input = sender.as<TextBox>();
      const std::wstring typed(input.Text().c_str());
      const auto bounded = cm::bounded_graphemes(typed, cm::name_limit);
      if (bounded == name)
        return;
      publication_id = random_uuid();
      name = bounded;
      if (bounded != typed)
        input.Text(hstring(bounded));
      changed();
    });
    // handleCommunityPublishKeyDown：在输入框里按回车就是发布。
    name_box.KeyDown([submit](Inspectable const &, Input::KeyRoutedEventArgs const &args) {
      if (args.Key() == Windows::System::VirtualKey::Enter) {
        args.Handled(true);
        submit();
      }
    });
    form.Children().Append(name_box);
    TextBox description_box;
    description_box.Header(box_value(hstring(description_label)));
    description_box.AcceptsReturn(true);
    description_box.TextWrapping(TextWrapping::Wrap);
    description_box.MinHeight(92);
    description_box.MaxLength(static_cast<int32_t>(cm::description_limit));
    description_box.Text(hstring(description));
    description_box.IsEnabled(!busy);
    A11y::SetName(description_box, hstring(description_a11y));
    description_box.TextChanged([&description, &publication_id, changed](
                                    Inspectable const &sender, TextChangedEventArgs const &) {
      const auto typed = cm::line_feeds(sender.as<TextBox>().Text().c_str());
      if (typed == description)
        return;
      publication_id = random_uuid();
      description = typed;
      changed();
    });
    form.Children().Append(description_box);
    if (!agreement_shown)
      return;
    CheckBox agree;
    auto words = make_text(agreement, 13, palette_.text);
    agree.Content(words);
    agree.IsChecked(agreed);
    agree.IsEnabled(!busy);
    A11y::SetName(agree, hstring(agreement_a11y));
    agree.Click([&agreed, changed](Inspectable const &sender, RoutedEventArgs const &) {
      const auto state = sender.as<CheckBox>().IsChecked();
      agreed = state && state.Value();
      changed();
    });
    form.Children().Append(agree);
  }

  // 发布对话框的外框：标题、内容可以滚动，「取消」和一个随状态变的主按钮。主按钮按下时对话框不关，由操作自己在成功后关上；忙的时候「取消」和 Esc 都关不上。
  ContentDialog publish_dialog(std::wstring const &title, std::function<bool()> busy,
                               std::function<void()> primary) {
    ContentDialog dialog;
    dialog.Title(box_value(hstring(title)));
    dialog.CloseButtonText(L"取消");
    dialog.DefaultButton(ContentDialogButton::None);
    dialog.XamlRoot(root_.XamlRoot());
    dialog.RequestedTheme(palette_.dark ? ElementTheme::Dark : ElementTheme::Light);
    dialog.Closing([busy](ContentDialog const &, ContentDialogClosingEventArgs const &args) {
      if (busy())
        args.Cancel(true);
    });
    dialog.PrimaryButtonClick([primary = std::move(primary)](
                                  ContentDialog const &, ContentDialogButtonClickEventArgs const &args) {
      args.Cancel(true);
      primary();
    });
    return dialog;
  }

  // 对话框里的一段出错说明，未登录时后面是「去登录」：先关上对话框再去「账号与同步」。
  void dialog_error(StackPanel const &form, std::wstring const &message, bool sign_in,
                    weak_ref<ContentDialog> const &dialog) {
    if (message.empty())
      return;
    community_error(form, message, sign_in, [this, dialog] {
      if (auto shown = dialog.get())
        shown.Hide();
      open_account_login();
    });
  }

  // 一段说明加按钮的框（共享页 style.confirmation 的 role=alert）。
  Border dialog_alert(std::vector<std::wstring> const &lines,
                      std::vector<FrameworkElement> const &buttons = {}) {
    StackPanel panel;
    panel.Spacing(8);
    panel.Padding(Thickness{12, 10, 12, 10});
    for (auto const &line : lines)
      panel.Children().Append(make_text(line, 13, palette_.text));
    if (!buttons.empty()) {
      StackPanel actions;
      actions.Orientation(Orientation::Horizontal);
      actions.Spacing(8);
      for (auto const &button : buttons)
        actions.Children().Append(button);
      panel.Children().Append(actions);
    }
    Border frame;
    frame.BorderBrush(brush(danger_color()));
    frame.BorderThickness(Thickness{1, 1, 1, 1});
    frame.CornerRadius(CornerRadius{4, 4, 4, 4});
    frame.Child(panel);
    A11y::SetLiveSetting(frame, Automation::Peers::AutomationLiveSetting::Assertive);
    return frame;
  }

  // ---- 发布候选窗口皮肤 ----

  // CandidateSkinPublishDialog 的状态。generation 在打开和关上时递增，作废在途的回答；pack_request 作废更早的打包检查。packages 和 images 是打开时扫到的本机皮肤，生成预览图时要用它们的配色和图片。
  struct SkinPublish {
    uint64_t generation = 0;
    bool from_card = false;
    std::vector<SkinPackage> packages;
    std::vector<SkinImageBytes> images;
    std::vector<std::pair<std::wstring, std::wstring>> options;
    bool options_loading = false;
    std::wstring skin_id;
    bool has_pack = false;
    std::wstring suggested_name, license_code, license_assets, license_source;
    double file_count = 0, size = 0;
    std::wstring pack_error, pack_code;
    bool pack_loading = false;
    uint64_t pack_request = 0;
    bool drawing = false, draw_failed = false;
    std::wstring license_choice = std::wstring(cm::asset_licenses[0].value);
    std::wstring custom_license;
    bool writing_license = false, license_failed = false;
    std::wstring name, description;
    bool agreed = false;
    std::wstring publication_id;
    std::wstring visibility = L"public";
    std::wstring category = L"other";
    std::wstring error;
    bool sign_in = false;
    bool open_failed = false;
    bool busy = false;
    std::wstring named_skin;
    std::function<void()> refresh;
    std::function<void()> update_button;
    weak_ref<ContentDialog> dialog;
  };

  // 打开「发布候选窗口皮肤」。从「我的皮肤」的卡片打开时先选中那一款；从图库打开时选第一款。
  fire_and_forget open_skin_publish(std::wstring initial, bool from_card) {
    if (dialog_open_ || skin_publish_.refresh)
      co_return;
    auto strong = get_strong();
    const auto generation = skin_publish_.generation + 1;
    skin_publish_ = SkinPublish{};
    auto &state = skin_publish_;
    state.generation = generation;
    state.from_card = from_card;
    state.skin_id = initial;
    if (!initial.empty())
      state.options.emplace_back(initial, initial);
    state.options_loading = true;
    state.publication_id = random_uuid();
    if (from_card)
      skin_published_notice_.clear();

    ScrollViewer scroller;
    scroller.MaxHeight(560);
    StackPanel form;
    form.Spacing(12);
    form.MinWidth(460);
    form.Padding(Thickness{0, 0, 12, 0});
    scroller.Content(form);
    auto dialog = publish_dialog(L"发布候选窗口皮肤", [this] { return skin_publish_.busy; },
                                 [this] { skin_publish_primary(); });
    dialog.Content(scroller);
    A11y::SetName(dialog, L"发布候选窗口皮肤");
    const auto weak_dialog = make_weak(dialog);
    const auto weak_form = make_weak(form);
    state.dialog = weak_dialog;
    state.update_button = [this, weak_dialog] {
      if (auto shown = weak_dialog.get())
        update_skin_publish_button(shown);
    };
    state.refresh = [this, weak_dialog, weak_form] {
      // 在控件自己的事件里不拆掉它：放到这次事件之后再重建。
      DispatcherQueue().TryEnqueue([this, weak_dialog, weak_form] {
        auto shown = weak_dialog.get();
        auto content = weak_form.get();
        if (shown && content && skin_publish_.refresh)
          build_skin_publish(content, shown);
      });
    };
    build_skin_publish(form, dialog);
    load_skin_publish_options(generation);
    if (!initial.empty())
      load_skin_pack();
    co_await show_dialog(dialog);
    ++skin_publish_.generation;
    skin_publish_.refresh = nullptr;
    skin_publish_.update_button = nullptr;
    skin_publish_.busy = false;
  }

  // 本机的皮肤：发布的选项，生成预览图时也用它们。读不到时说明，并保留已有的选项。
  void load_skin_publish_options(uint64_t generation) {
    run_worker<SkinScan>([root = skins_root()] { return read_skin_catalog(root); },
                         [this, generation](SkinScan &scan) {
                           auto &state = skin_publish_;
                           if (generation != state.generation)
                             return;
                           state.options_loading = false;
                           if (!scan.ok) {
                             state.error = L"读取本地皮肤失败，请重试。";
                             state.sign_in = false;
                           } else {
                             state.packages = std::move(scan.packages);
                             state.images = std::move(scan.images);
                             state.options.clear();
                             for (auto const &skin : state.packages)
                               state.options.emplace_back(skin.id, skin.name);
                             const auto previous = state.skin_id;
                             if (previous.empty() ||
                                 std::none_of(state.options.begin(), state.options.end(),
                                              [&previous](auto const &option) {
                                                return option.first == previous;
                                              }))
                               state.skin_id = state.options.empty() ? L"" : state.options.front().first;
                             if (state.skin_id != previous)
                               load_skin_pack();
                           }
                           if (state.refresh)
                             state.refresh();
                         });
  }

  // 换了皮肤、公开范围或刚补了预览图和授权时重新检查皮肤包（不上传）；检查前的勾选和发布 id 作废。第一次检查这款皮肤时用它建议的名字填上名称，之后只换公开范围时保留用户输入的名称。
  void load_skin_pack() {
    auto &state = skin_publish_;
    const auto request = ++state.pack_request;
    const auto generation = state.generation;
    state.has_pack = false;
    state.pack_error.clear();
    state.pack_code.clear();
    state.draw_failed = false;
    state.license_failed = false;
    state.agreed = false;
    state.publication_id = random_uuid();
    if (state.skin_id.empty()) {
      state.pack_loading = false;
      if (state.refresh)
        state.refresh();
      return;
    }
    state.pack_loading = true;
    if (state.refresh)
      state.refresh();
    auto action = community_action(L"candidate_skin_pack_preview");
    action.SetNamedValue(L"skin_id", JsonValue::CreateStringValue(hstring(state.skin_id)));
    action.SetNamedValue(L"visibility", JsonValue::CreateStringValue(hstring(state.visibility)));
    run_host([action] { return community_call(action); },
             [this, request, generation, skin_id = state.skin_id](Response const &response) {
               auto &current = skin_publish_;
               if (generation != current.generation || request != current.pack_request)
                 return;
               current.pack_loading = false;
               const auto value = community_value(response);
               if (response.ok && value.ValueType() == JsonValueType::Object) {
                 const auto pack = value.GetObject();
                 current.has_pack = true;
                 current.suggested_name = named_string(pack, L"suggestedName");
                 current.file_count = named_number(pack, L"fileCount").value_or(0);
                 current.size = named_number(pack, L"size").value_or(0);
                 const auto license = pack.GetNamedValue(L"license", JsonValue::CreateNullValue());
                 if (license.ValueType() == JsonValueType::Object) {
                   const auto fields = license.GetObject();
                   current.license_code = named_string(fields, L"code");
                   current.license_assets = named_string(fields, L"assets");
                   current.license_source = named_string(fields, L"source");
                 }
                 if (current.named_skin != skin_id) {
                   current.named_skin = skin_id;
                   current.name = cm::bounded_graphemes(current.suggested_name, cm::name_limit);
                 }
               } else {
                 const auto failure = community_failure(response);
                 current.pack_error = cm::candidate_skin_message(failure.code);
                 current.pack_code = failure.code;
               }
               if (current.refresh)
                 current.refresh();
             });
  }

  // 没有预览图的已装皮肤（社区要用预览图展示它）：带装饰却没有自己图片的包用装饰当预览，画一张会改掉它的样子，宿主也会拒绝，这种不给生成。
  SkinPackage const *previewless_skin() const {
    auto const &state = skin_publish_;
    if (state.pack_code != L"candidate_skin_preview_required")
      return nullptr;
    for (auto const &skin : state.packages)
      if (skin.id == state.skin_id &&
          !(skin.decoration_top > 0 && skin.decoration_width > 0 && skin.decoration_image.empty()))
        return &skin;
    return nullptr;
  }

  bool skin_licenseless() const {
    return skin_publish_.pack_code == L"candidate_skin_license_required" &&
           !skin_publish_.skin_id.empty();
  }

  std::wstring skin_license_value() const {
    auto const &state = skin_publish_;
    return state.license_choice == L"other" ? cm::trim(state.custom_license) : state.license_choice;
  }

  bool skin_publish_ready() const {
    auto const &state = skin_publish_;
    const auto fields = cm::publish_fields(state.name, state.description);
    return state.has_pack && !state.pack_loading && fields.name_valid && fields.description_valid &&
           state.agreed;
  }

  // 主按钮：缺素材授权时是「使用此授权并继续」，其余打包问题时没有主按钮，否则是发布。
  void update_skin_publish_button(ContentDialog const &dialog) {
    auto const &state = skin_publish_;
    if (!state.pack_error.empty() && skin_licenseless()) {
      dialog.PrimaryButtonText(state.writing_license ? L"正在写入…" : L"使用此授权并继续");
      dialog.IsPrimaryButtonEnabled(!state.writing_license &&
                                    cm::asset_license_valid(skin_license_value()));
    } else if (state.pack_error.empty()) {
      dialog.PrimaryButtonText(state.busy                       ? L"正在发布…"
                               : state.visibility == L"public" ? L"公开发布"
                                                               : L"保存到我的皮肤库");
      dialog.IsPrimaryButtonEnabled(!state.busy && skin_publish_ready());
    } else {
      dialog.PrimaryButtonText(L"");
    }
  }

  void skin_publish_primary() {
    if (!skin_publish_.pack_error.empty() && skin_licenseless())
      write_skin_license();
    else if (skin_publish_.pack_error.empty())
      submit_skin_publish();
  }

  // 「打开目录」：打开皮肤目录，打不开时在对话框里说明。
  void open_skin_publish_directory() {
    const auto generation = skin_publish_.generation;
    skin_publish_.open_failed = false;
    run_worker<bool>(
        [root = skins_root()] {
          if (root.empty())
            return false;
          std::error_code error;
          std::filesystem::create_directories(root, error);
          return std::filesystem::is_directory(root, error);
        },
        [this, generation, root = skins_root()](bool &ready) {
          const bool opened =
              ready && reinterpret_cast<INT_PTR>(ShellExecuteW(nullptr, L"open", root.c_str(),
                                                               nullptr, nullptr,
                                                               SW_SHOWNORMAL)) > 32;
          if (generation != skin_publish_.generation)
            return;
          skin_publish_.open_failed = !opened;
          if (skin_publish_.refresh)
            skin_publish_.refresh();
        });
  }

  // 「生成预览图」：按皮肤自己的配色和图片画一张，写进皮肤文件夹并加进清单，再重新检查。
  void draw_skin_preview() {
    auto &state = skin_publish_;
    const auto *skin = previewless_skin();
    if (!skin || state.drawing)
      return;
    state.drawing = true;
    state.draw_failed = false;
    const auto generation = state.generation;
    const auto request = state.pack_request;
    struct Drawn {
      bool ok = false;
      SkinScan scan;
    };
    run_worker<Drawn>(
        [skin = *skin, images = state.images, root = skins_root()] {
          Drawn drawn;
          const auto bytes = render_skin_preview(skin, images);
          if (!bytes)
            return drawn;
          const auto buffer = Windows::Security::Cryptography::CryptographicBuffer::CreateFromByteArray(
              array_view<uint8_t const>(bytes->data(), bytes->data() + bytes->size()));
          auto action = community_action(L"candidate_skin_add_preview");
          action.SetNamedValue(L"skin_id", JsonValue::CreateStringValue(hstring(skin.id)));
          action.SetNamedValue(L"image", JsonValue::CreateStringValue(
                                             Windows::Security::Cryptography::CryptographicBuffer::
                                                 EncodeToBase64String(buffer)));
          if (!community_call(action).ok)
            return drawn;
          drawn.ok = true;
          drawn.scan = read_skin_catalog(root);
          return drawn;
        },
        [this, generation, request](Drawn &drawn) {
          auto &current = skin_publish_;
          if (generation != current.generation)
            return;
          current.drawing = false;
          if (request != current.pack_request) {
            if (current.refresh)
              current.refresh();
            return;
          }
          if (!drawn.ok) {
            current.draw_failed = true;
            if (current.refresh)
              current.refresh();
            return;
          }
          if (drawn.scan.ok) {
            current.packages = std::move(drawn.scan.packages);
            current.images = std::move(drawn.scan.images);
          }
          // 预览图已经写进皮肤目录：「我的皮肤」下次显示时重扫。
          skins_stale_ = true;
          load_skin_pack();
        });
    if (state.refresh)
      state.refresh();
  }

  // 「使用此授权并继续」：把选的素材授权写进 skin.toml，再重新检查。
  void write_skin_license() {
    auto &state = skin_publish_;
    const auto value = skin_license_value();
    if (!skin_licenseless() || !cm::asset_license_valid(value) || state.writing_license)
      return;
    state.writing_license = true;
    state.license_failed = false;
    const auto generation = state.generation;
    const auto request = state.pack_request;
    auto action = community_action(L"candidate_skin_add_license");
    action.SetNamedValue(L"skin_id", JsonValue::CreateStringValue(hstring(state.skin_id)));
    action.SetNamedValue(L"assets", JsonValue::CreateStringValue(hstring(value)));
    run_host([action] { return community_call(action); },
             [this, generation, request](Response const &response) {
               auto &current = skin_publish_;
               if (generation != current.generation)
                 return;
               current.writing_license = false;
               if (request != current.pack_request) {
                 if (current.refresh)
                   current.refresh();
                 return;
               }
               if (!response.ok) {
                 current.license_failed = true;
                 if (current.refresh)
                   current.refresh();
                 return;
               }
               skins_stale_ = true;
               load_skin_pack();
             });
    if (state.refresh)
      state.refresh();
  }

  // 发布：同一次发布重试时沿用发布 id，服务端据此去重。
  void submit_skin_publish() {
    auto &state = skin_publish_;
    if (state.busy || !skin_publish_ready() || state.skin_id.empty())
      return;
    state.busy = true;
    state.error.clear();
    state.sign_in = false;
    const auto fields = cm::publish_fields(state.name, state.description);
    auto action = community_action(L"candidate_skin_publish");
    action.SetNamedValue(L"skin_id", JsonValue::CreateStringValue(hstring(state.skin_id)));
    action.SetNamedValue(L"id", JsonValue::CreateStringValue(hstring(state.publication_id)));
    action.SetNamedValue(L"name", JsonValue::CreateStringValue(hstring(fields.name)));
    action.SetNamedValue(L"description", JsonValue::CreateStringValue(hstring(fields.description)));
    action.SetNamedValue(L"visibility", JsonValue::CreateStringValue(hstring(state.visibility)));
    action.SetNamedValue(L"category", JsonValue::CreateStringValue(hstring(state.category)));
    const auto generation = state.generation;
    run_host([action] { return community_call(action); },
             [this, generation](Response const &response) {
               auto &current = skin_publish_;
               if (generation != current.generation)
                 return;
               current.busy = false;
               auto published = community_item<CommunitySkinItem>(response);
               if (!published) {
                 const auto failure = community_failure(response);
                 current.error = cm::candidate_skin_message(failure.code, true);
                 current.sign_in = cm::needs_sign_in(failure.code);
                 if (current.refresh)
                   current.refresh();
                 return;
               }
               skin_published(*published, current.from_card);
             });
    if (state.refresh)
      state.refresh();
  }

  // 发布成功：关上对话框。从卡片发布的在「我的皮肤」的卡片下说明；从图库发布的在图库里说明并从第一页重读。
  void skin_published(CommunitySkinItem const &published, bool from_card) {
    const auto message = published.visibility == L"private" ? L"已保存到你的皮肤库，仅自己可见。"
                                                            : L"已发布到社区。";
    const auto dialog = skin_publish_.dialog;
    ++skin_publish_.generation;
    skin_publish_.refresh = nullptr;
    skin_publish_.update_button = nullptr;
    if (auto shown = dialog.get())
      shown.Hide();
    if (from_card) {
      skin_published_notice_ = message;
      redraw_themes();
      return;
    }
    skin_gallery_.notice = message;
    gallery_list<CommunitySkinItem>(skin_gallery_.active_search, false, skin_gallery_.active_mine);
  }

  void build_skin_publish(StackPanel const &form, ContentDialog const &dialog) {
    auto &state = skin_publish_;
    form.Children().Clear();
    dialog_error(form, state.error, state.sign_in, make_weak(dialog));
    if (state.options_loading)
      form.Children().Append(make_text(L"正在读取本地皮肤…", 13, palette_.sub));
    if (!state.options_loading && state.options.empty())
      form.Children().Append(make_text(
          L"还没有可发布的外部皮肤，请先把皮肤文件夹放进皮肤目录，再在「主题」的外部皮肤中刷新。", 13,
          palette_.sub));
    if (!state.options.empty()) {
      std::vector<Option> options;
      for (auto const &[id, name] : state.options)
        options.push_back({id, cm::local_option_label(name, id)});
      auto combo = select_control(L"发布皮肤", std::move(options), state.skin_id,
                                  [this](std::wstring const &id) {
                                    if (id == skin_publish_.skin_id)
                                      return;
                                    skin_publish_.skin_id = id;
                                    load_skin_pack();
                                  },
                                  !state.busy, false)
                       .as<ComboBox>();
      combo.Header(box_value(L"发布皮肤"));
      combo.MinWidth(320);
      form.Children().Append(combo);
    }
    StackPanel audience;
    audience.Spacing(4);
    audience.Children().Append(make_text(L"谁可以看到", 13, palette_.text));
    for (auto const &[value, label] : {std::pair<std::wstring, std::wstring>{L"public", L"公开（所有人可下载）"},
                                       {L"private", L"仅自己可见"}}) {
      RadioButton choice;
      choice.Content(box_value(hstring(label)));
      choice.GroupName(L"candidate-skin-visibility");
      choice.IsChecked(state.visibility == value);
      choice.IsEnabled(!state.busy);
      choice.Checked([this, value](Inspectable const &, RoutedEventArgs const &) {
        if (skin_publish_.visibility == value)
          return;
        skin_publish_.visibility = value;
        load_skin_pack();
      });
      audience.Children().Append(choice);
    }
    form.Children().Append(audience);
    if (state.pack_loading)
      form.Children().Append(make_text(L"正在检查皮肤包…", 13, palette_.sub));
    const auto *previewless = previewless_skin();
    const bool licenseless = skin_licenseless();
    auto open_button = [this](bool enabled) {
      return button_control(L"打开目录", [this] { open_skin_publish_directory(); }, enabled);
    };
    if (!state.pack_error.empty() && previewless) {
      std::vector<std::wstring> lines{
          L"这款皮肤还没有预览图，社区要用它展示皮肤。可以按皮肤自己的配色和图片生成一张，保存到皮肤文件夹后继续发布。"};
      if (state.draw_failed)
        lines.emplace_back(L"生成预览图失败，请重试，或自己在 skin.toml 中用 preview 指定一张图片。");
      auto draw = button_control(state.drawing ? L"正在生成…" : L"生成预览图",
                                 [this] { draw_skin_preview(); }, !state.drawing)
                      .as<Button>();
      make_primary(draw);
      form.Children().Append(dialog_alert(lines, {open_button(!state.drawing), draw}));
      if (state.open_failed)
        form.Children().Append(make_text(L"无法打开皮肤目录，请重试。", 13, danger_color()));
    }
    if (!state.pack_error.empty() && licenseless) {
      StackPanel license;
      license.Spacing(4);
      license.Children().Append(make_text(L"素材授权", 13, palette_.text));
      license.Children().Append(make_text(
          L"公开发布需要注明别人可以怎样使用皮肤里的图片，选择后会写入 skin.toml。", 12, palette_.sub));
      std::vector<std::pair<std::wstring, std::wstring>> choices;
      for (auto const &item : cm::asset_licenses)
        choices.emplace_back(std::wstring(item.value), std::wstring(item.label));
      choices.emplace_back(L"other", L"其他");
      for (auto const &[value, label] : choices) {
        RadioButton choice;
        choice.Content(box_value(hstring(label)));
        choice.GroupName(L"candidate-skin-asset-license");
        choice.IsChecked(state.license_choice == value);
        choice.IsEnabled(!state.writing_license);
        choice.Checked([this, value](Inspectable const &, RoutedEventArgs const &) {
          if (skin_publish_.license_choice == value)
            return;
          skin_publish_.license_choice = value;
          if (skin_publish_.refresh)
            skin_publish_.refresh();
        });
        license.Children().Append(choice);
      }
      if (state.license_choice == L"other") {
        TextBox custom;
        custom.PlaceholderText(L"例如：仅限个人使用，不得转售");
        custom.Text(hstring(state.custom_license));
        custom.IsEnabled(!state.writing_license);
        A11y::SetName(custom, L"其他素材授权");
        auto warning = make_text(L"授权说明太长，请控制在 40 个汉字以内。", 12, danger_color());
        auto show_warning = [warning = make_weak(warning)](std::wstring const &value) {
          if (auto line = warning.get())
            line.Visibility(!cm::trim(value).empty() && !cm::asset_license_valid(value)
                                ? Visibility::Visible
                                : Visibility::Collapsed);
        };
        show_warning(state.custom_license);
        custom.TextChanged([this, show_warning](Inspectable const &sender, TextChangedEventArgs const &) {
          skin_publish_.custom_license = std::wstring(sender.as<TextBox>().Text().c_str());
          show_warning(skin_publish_.custom_license);
          if (skin_publish_.update_button)
            skin_publish_.update_button();
        });
        license.Children().Append(custom);
        license.Children().Append(warning);
      }
      if (state.license_failed)
        license.Children().Append(make_text(
            L"写入授权失败，请重试，或在 skin.toml 的 [license] 中自己填写 assets。", 12, danger_color()));
      form.Children().Append(license);
    }
    if (!state.pack_error.empty() && !previewless && !licenseless) {
      form.Children().Append(dialog_alert({state.pack_error}, {open_button(true)}));
      if (state.open_failed)
        form.Children().Append(make_text(L"无法打开皮肤目录，请重试。", 13, danger_color()));
    }
    if (state.has_pack) {
      form.Children().Append(make_text(std::to_wstring(std::llround(state.file_count)) + L" 个文件 · " +
                                           cm::megabytes(state.size) + L" / " +
                                           std::wstring(cm::skin_package_limit),
                                       13, palette_.sub));
      auto license = make_text(
          cm::license_line(state.license_assets, state.license_code, state.license_source), 13,
          palette_.sub);
      A11y::SetName(license, L"皮肤授权");
      form.Children().Append(license);
      publication_fields(
          form, state.name, state.description, state.agreed, state.publication_id, state.busy,
          L"皮肤名称", L"发布皮肤名称", L"设计说明", L"发布设计说明",
          state.visibility == L"public" ? L"我拥有发布所用素材的权利，并同意其他用户按上述授权免费下载使用"
                                        : L"我拥有上传所用素材的权利",
          L"确认拥有发布素材权利",
          [this] {
            if (skin_publish_.update_button)
              skin_publish_.update_button();
          },
          [this] { submit_skin_publish(); });
      form.Children().Append(skin_category_select(L"发布分类", state.category, !state.busy,
                                                  [this](std::wstring const &next) {
                                                    // 分类也是这次发布的内容，换了分类就是另一次发布。
                                                    skin_publish_.publication_id = random_uuid();
                                                    skin_publish_.category = next;
                                                  }));
      form.Children().Append(make_text(cm::skin_publish_warning, 12, palette_.sub));
    }
    update_skin_publish_button(dialog);
  }

  // ---- 发布插件 ----

  // CommunityPluginPublishDialog 的状态：本机可发布的插件（内置的不算）、选中的那个、打包检查的结果和表单。
  struct PluginPublish {
    uint64_t generation = 0;
    struct Choice {
      std::wstring key, kind, label, id, name;
    };
    std::vector<Choice> options;
    bool options_loading = true;
    std::wstring selection;
    bool has_pack = false;
    std::wstring version, license;
    double file_count = 0, size = 0;
    std::wstring pack_error;
    bool pack_loading = false;
    uint64_t pack_request = 0;
    std::wstring name, description;
    bool agreed = false;
    std::wstring publication_id;
    std::wstring error;
    bool sign_in = false;
    bool busy = false;
    std::function<void()> refresh;
    std::function<void()> update_button;
    weak_ref<ContentDialog> dialog;
  };

  PluginPublish::Choice const *chosen_plugin() const {
    for (auto const &option : plugin_publish_.options)
      if (option.key == plugin_publish_.selection)
        return &option;
    return nullptr;
  }

  fire_and_forget open_plugin_publish() {
    if (dialog_open_ || plugin_publish_.refresh)
      co_return;
    auto strong = get_strong();
    const auto generation = plugin_publish_.generation + 1;
    plugin_publish_ = PluginPublish{};
    plugin_publish_.generation = generation;
    plugin_publish_.publication_id = random_uuid();

    ScrollViewer scroller;
    scroller.MaxHeight(560);
    StackPanel form;
    form.Spacing(12);
    form.MinWidth(460);
    form.Padding(Thickness{0, 0, 12, 0});
    scroller.Content(form);
    auto dialog = publish_dialog(L"发布插件", [this] { return plugin_publish_.busy; },
                                 [this] { submit_plugin_publish(); });
    dialog.Content(scroller);
    A11y::SetName(dialog, L"发布插件");
    const auto weak_dialog = make_weak(dialog);
    const auto weak_form = make_weak(form);
    plugin_publish_.dialog = weak_dialog;
    plugin_publish_.update_button = [this, weak_dialog] {
      if (auto shown = weak_dialog.get())
        update_plugin_publish_button(shown);
    };
    plugin_publish_.refresh = [this, weak_dialog, weak_form] {
      DispatcherQueue().TryEnqueue([this, weak_dialog, weak_form] {
        auto shown = weak_dialog.get();
        auto content = weak_form.get();
        if (shown && content && plugin_publish_.refresh)
          build_plugin_publish(content, shown);
      });
    };
    build_plugin_publish(form, dialog);
    run_host(plugin_catalog_call, [this, generation](Response const &response) {
      auto &state = plugin_publish_;
      if (generation != state.generation)
        return;
      state.options_loading = false;
      if (auto catalog = plugin_catalog(response)) {
        state.options.clear();
        for (auto const &pack : catalog->packages) {
          if (pack.builtin)
            continue;
          state.options.push_back({pack.kind + L"/" + pack.id, pack.kind, pg::pack_kind_label(pack),
                                   pack.id, pack.name});
        }
        const auto previous = state.selection;
        if (previous.empty() || !chosen_plugin())
          state.selection = state.options.empty() ? L"" : state.options.front().key;
        if (state.selection != previous)
          load_plugin_pack();
      } else {
        state.error = L"读取本地插件失败，请重试。";
        state.sign_in = false;
      }
      if (state.refresh)
        state.refresh();
    });
    co_await show_dialog(dialog);
    ++plugin_publish_.generation;
    plugin_publish_.refresh = nullptr;
    plugin_publish_.update_button = nullptr;
    plugin_publish_.busy = false;
  }

  // 换了插件就重新打包检查（不上传），用它建议的名称和说明填上表单；勾选和发布 id 作废。
  void load_plugin_pack() {
    auto &state = plugin_publish_;
    const auto request = ++state.pack_request;
    const auto generation = state.generation;
    state.has_pack = false;
    state.pack_error.clear();
    state.agreed = false;
    state.publication_id = random_uuid();
    const auto *chosen = chosen_plugin();
    if (!chosen) {
      state.pack_loading = false;
      return;
    }
    state.pack_loading = true;
    auto action = community_action(L"plugin_pack_preview");
    action.SetNamedValue(L"kind", JsonValue::CreateStringValue(hstring(chosen->kind)));
    action.SetNamedValue(L"plugin_id", JsonValue::CreateStringValue(hstring(chosen->id)));
    run_host([action] { return community_call(action); },
             [this, request, generation](Response const &response) {
               auto &current = plugin_publish_;
               if (generation != current.generation || request != current.pack_request)
                 return;
               current.pack_loading = false;
               const auto value = community_value(response);
               if (response.ok && value.ValueType() == JsonValueType::Object) {
                 const auto pack = value.GetObject();
                 current.has_pack = true;
                 current.version = named_string(pack, L"version");
                 current.license = named_string(pack, L"license");
                 current.file_count = named_number(pack, L"fileCount").value_or(0);
                 current.size = named_number(pack, L"size").value_or(0);
                 current.name = cm::bounded_graphemes(named_string(pack, L"suggestedName"), cm::name_limit);
                 current.description = cm::line_feeds(named_string(pack, L"suggestedDescription"));
               } else {
                 const auto failure = community_failure(response);
                 current.pack_error = cm::plugin_message(failure.code, failure.detail);
               }
               if (current.refresh)
                 current.refresh();
             });
  }

  bool plugin_publish_ready() const {
    auto const &state = plugin_publish_;
    const auto fields = cm::publish_fields(state.name, state.description);
    return state.has_pack && !state.pack_loading && fields.name_valid && fields.description_valid &&
           state.agreed;
  }

  void update_plugin_publish_button(ContentDialog const &dialog) {
    auto const &state = plugin_publish_;
    if (state.pack_error.empty()) {
      dialog.PrimaryButtonText(state.busy ? L"正在发布…" : L"公开发布");
      dialog.IsPrimaryButtonEnabled(!state.busy && plugin_publish_ready());
    } else {
      dialog.PrimaryButtonText(L"");
    }
  }

  void submit_plugin_publish() {
    auto &state = plugin_publish_;
    const auto *chosen = chosen_plugin();
    if (state.busy || !plugin_publish_ready() || !chosen)
      return;
    state.busy = true;
    state.error.clear();
    state.sign_in = false;
    const auto fields = cm::publish_fields(state.name, state.description);
    auto action = community_action(L"plugin_publish");
    action.SetNamedValue(L"kind", JsonValue::CreateStringValue(hstring(chosen->kind)));
    action.SetNamedValue(L"plugin_id", JsonValue::CreateStringValue(hstring(chosen->id)));
    action.SetNamedValue(L"id", JsonValue::CreateStringValue(hstring(state.publication_id)));
    action.SetNamedValue(L"name", JsonValue::CreateStringValue(hstring(fields.name)));
    action.SetNamedValue(L"description", JsonValue::CreateStringValue(hstring(fields.description)));
    const auto generation = state.generation;
    run_host([action] { return community_call(action); },
             [this, generation](Response const &response) {
               auto &current = plugin_publish_;
               if (generation != current.generation)
                 return;
               current.busy = false;
               if (!community_item<CommunityPluginItem>(response)) {
                 const auto failure = community_failure(response);
                 current.error = cm::plugin_message(failure.code, failure.detail, true);
                 current.sign_in = cm::needs_sign_in(failure.code);
                 if (current.refresh)
                   current.refresh();
                 return;
               }
               const auto dialog = current.dialog;
               ++current.generation;
               current.refresh = nullptr;
               current.update_button = nullptr;
               if (auto shown = dialog.get())
                 shown.Hide();
               plugin_gallery_.notice = L"已发布到社区。";
               gallery_list<CommunityPluginItem>(plugin_gallery_.active_search, false,
                                                 plugin_gallery_.active_mine);
             });
    if (state.refresh)
      state.refresh();
  }

  void build_plugin_publish(StackPanel const &form, ContentDialog const &dialog) {
    auto &state = plugin_publish_;
    form.Children().Clear();
    dialog_error(form, state.error, state.sign_in, make_weak(dialog));
    if (state.options_loading)
      form.Children().Append(make_text(L"正在读取本地插件…", 13, palette_.sub));
    if (!state.options_loading && state.options.empty())
      form.Children().Append(make_text(
          L"还没有可发布的插件。内置插件不能发布，请先在「我的插件」中导入自己的音效包、音乐包、指令表、特效包、短语表、辅助码表、单词本或符号集。",
          13, palette_.sub));
    if (!state.options.empty()) {
      std::vector<Option> options;
      for (auto const &option : state.options)
        options.push_back({option.key, option.label + L" · " + cm::local_option_label(option.name, option.id)});
      auto combo = select_control(L"发布插件", std::move(options), state.selection,
                                  [this](std::wstring const &key) {
                                    if (key == plugin_publish_.selection)
                                      return;
                                    plugin_publish_.selection = key;
                                    load_plugin_pack();
                                    if (plugin_publish_.refresh)
                                      plugin_publish_.refresh();
                                  },
                                  !state.busy, false)
                       .as<ComboBox>();
      combo.Header(box_value(L"发布插件"));
      combo.MinWidth(320);
      form.Children().Append(combo);
    }
    if (state.pack_loading)
      form.Children().Append(make_text(L"正在检查插件…", 13, palette_.sub));
    if (!state.pack_error.empty())
      form.Children().Append(dialog_alert({state.pack_error}));
    if (state.has_pack) {
      form.Children().Append(make_text(
          cm::join_dot({L"v" + state.version, state.license.empty() ? L"" : L"授权 " + state.license,
                        std::to_wstring(std::llround(state.file_count)) + L" 个文件",
                        cm::megabytes(state.size) + L" / " + std::wstring(cm::plugin_archive_limit)}),
          13, palette_.sub));
      publication_fields(
          form, state.name, state.description, state.agreed, state.publication_id, state.busy,
          L"名称", L"发布插件名称", L"说明", L"发布插件说明",
          L"我拥有插件中音频与文字的发布权利，并同意其他用户按包内授权免费下载使用", L"确认拥有发布内容权利",
          [this] {
            if (plugin_publish_.update_button)
              plugin_publish_.update_button();
          },
          [this] { submit_plugin_publish(); });
      form.Children().Append(make_text(cm::plugin_publish_warning, 12, palette_.sub));
    }
    update_plugin_publish_button(dialog);
  }

  // ---- 候选颜色 ----

  // 「自定义主题」组「自定义配色」里的五个取色器（编号色和边框色跟随主题，见 ThemeSkinModel.h 的 color_slots）。没设的颜色在色块上显示候选窗口当前明暗下的默认色，与共享页按预览明暗取默认色一样。
  void candidate_color_rows(StackPanel const &group) {
    const bool light = !candidate_dark();
    for (const auto &slot : sk::color_slots) {
      const std::wstring key = L"custom_theme.candidate_colors." + std::wstring(slot.key);
      const auto set = sk::candidate_text_color(document_.String(key, L""));
      const std::wstring shown = set.value_or(std::wstring(light ? slot.light : slot.dark));
      add_row(group, 0xE790, std::wstring(slot.title), L"",
              candidate_color_control(slot, key, set.has_value(), shown));
    }
  }

  static std::wstring color_hex(Color const &color) {
    wchar_t buffer[8];
    swprintf_s(buffer, L"#%02x%02x%02x", color.R, color.G, color.B);
    return buffer;
  }

  // 设一个候选颜色或交还给主题（`value` 为空），连同 updateCandidateColor 的其余改动：选了颜色就选上自定义主题；原来不是自定义主题时，把当时的主题留作底并清掉两个槽位的皮肤包。
  static void write_candidate_color(PreferencesDocument &doc, std::wstring const &key,
                                    std::optional<std::wstring> const &value) {
    const auto change = sk::candidate_color_change(doc.String(L"global_theme", L"system"),
                                                   doc.String(L"custom_theme.base", L"system"),
                                                   !value);
    if (value)
      doc.SetString(key, *value);
    else
      doc.SetValue(key, JsonValue::CreateNullValue());
    if (change.select_custom)
      doc.SetString(L"global_theme", L"custom");
    if (change.reset_skins) {
      doc.SetString(L"custom_theme.base", change.base);
      doc.SetValue(L"custom_theme.candidate_skin", JsonValue::CreateNullValue());
      doc.SetValue(L"custom_theme.candidate_skin_dark", JsonValue::CreateNullValue());
    }
  }

  // 色块打开 WinUI 的取色器；拖动时颜色随时写入并刷新顶部的预览，停下后保存，取色器关上时整页重画。「跟随主题」把这一项交还给底下的主题：文字、强调、选中和悬停的按钮在没设颜色时是按下的，再点不起作用；表面色的没有按下状态，点它总会清掉那一项。
  FrameworkElement candidate_color_control(sk::ColorSlot const &slot, std::wstring const &key,
                                           bool set, std::wstring const &shown) {
    const std::wstring title(slot.title);
    const auto color = parse_hex(shown).value_or(palette_.text);
    StackPanel box;
    box.Orientation(Orientation::Horizontal);
    box.Spacing(8);

    Button swatch;
    swatch.Width(52);
    swatch.Height(32);
    swatch.Padding(Thickness{6, 6, 6, 6});
    swatch.HorizontalContentAlignment(HorizontalAlignment::Stretch);
    swatch.VerticalContentAlignment(VerticalAlignment::Stretch);
    Border chip;
    chip.CornerRadius(CornerRadius{3, 3, 3, 3});
    chip.BorderBrush(brush(palette_.stroke2));
    chip.BorderThickness(Thickness{1, 1, 1, 1});
    chip.Background(brush(color));
    swatch.Content(chip);
    A11y::SetName(swatch, hstring(title));
    A11y::SetHelpText(swatch, hstring(shown));
    swatch.IsEnabled(loaded_);

    ColorPicker picker;
    picker.IsAlphaEnabled(false);
    picker.IsHexInputVisible(true);
    picker.IsColorChannelTextInputVisible(true);
    picker.Color(color);
    picker.ColorChanged([this, key, weak_chip = make_weak(chip)](
                            ColorPicker const &, ColorChangedEventArgs const &args) {
      // 只有用户真的换了颜色才写：取色器自己报一次没变的颜色时，不能因此选上自定义主题。
      const auto value = color_hex(args.NewColor());
      if (value == color_hex(args.OldColor()))
        return;
      if (auto target = weak_chip.get())
        target.Background(brush(args.NewColor()));
      write_candidate_color(document_, key, value);
      schedule_save();
      update_preview();
    });
    Flyout flyout;
    flyout.Content(picker);
    flyout.Opened([this](Inspectable const &, Inspectable const &) { color_picker_open_ = true; });
    flyout.Closed([this](Inspectable const &, Inspectable const &) {
      color_picker_open_ = false;
      flush_pending();
      queue_refresh();
    });
    swatch.Flyout(flyout);
    box.Children().Append(swatch);

    const std::wstring reset_name =
        slot.reset_label.empty() ? std::wstring(L"跟随主题") : std::wstring(slot.reset_label);
    auto clear = [this, key] {
      change([&](PreferencesDocument &doc) { write_candidate_color(doc, key, std::nullopt); },
             true);
    };
    if (slot.pressed) {
      Primitives::ToggleButton reset;
      reset.Content(box_value(L"跟随主题"));
      reset.Height(32);
      reset.IsChecked(!set);
      reset.IsEnabled(loaded_);
      A11y::SetName(reset, hstring(reset_name));
      reset.Click([set, clear](Inspectable const &sender, RoutedEventArgs const &) {
        if (set)
          clear();
        else
          sender.as<Primitives::ToggleButton>().IsChecked(true);
      });
      box.Children().Append(reset);
    } else {
      auto reset = button_control(L"跟随主题", clear, loaded_);
      A11y::SetName(reset, hstring(reset_name));
      box.Children().Append(reset);
    }
    return box;
  }

  static std::vector<Option> surface_theme_options() {
    return {{L"follow", L"跟随颜色模式"}, {L"dark", L"深色"}, {L"light", L"浅色"}};
  }

  // ---- 候选字体 ----

  // 共享设置「候选窗口」页的字体各行，按 Windows 宿主的样子：「布局与字体」组里是「候选字体」预设（末尾是「自定义…」）和按需出现的主字体，「更多选项」里是英文字体（candidate_english_font_row）。规则在 CandidateFontPresets.h。与共享页一样没有回退字体列表的编辑器：预设和主字体会写它。返回主字体框画没画出来：读取字体列表的状态和「刷新」按钮跟着当前第一个可见的字体框走，主字体框不在时由英文字体框带着。
  bool candidate_font_rows(StackPanel const &group) {
    StackPanel preset_host;
    group.Children().Append(preset_host);
    candidate_font_preset_row(preset_host);
    if (!indexing_) {
      font_preset_host_ = preset_host;
      font_menu_updaters_.clear();
    }

    // 索引时总要画，搜索才找得到「主字体」；搜索落到它上面时也画出来。
    const bool shown =
        indexing_ || focus_row_ == L"主字体" ||
        nav::candidate_main_font_shown(
            candidate_font_custom_,
            document_.String(L"candidate_font_family", nav::candidate_default_font_family),
            document_.Strings(L"candidate_fallback_fonts"));
    if (!shown)
      return false;
    auto [field, status] = font_field_with_catalog(
        font_input(L"主字体",
                   document_.String(L"candidate_font_family",
                                    nav::candidate_default_font_family),
                   [this](std::wstring const &font) {
                     // Windows 的候选窗从回退列表取汉字，主字体同时排到回退列表最前。
                     const auto patch = nav::candidate_main_font_patch(
                         font,
                         document_.String(L"candidate_font_family",
                                          nav::candidate_default_font_family),
                         document_.Strings(L"candidate_fallback_fonts"));
                     document_.SetString(L"candidate_font_family", patch.family);
                     document_.SetOrderedStrings(L"candidate_fallback_fonts", patch.fallback);
                     schedule_save();
                     update_preview();
                     refresh_font_preset_row();
                   }));
    add_row(group, 0xE8C1, L"主字体", L"", field, status);
    return true;
  }

  // 「更多选项」里的英文字体。`catalog` 为真（主字体框收起）时由它带着字体列表的读取状态和「刷新」按钮，读取失败后的重试和装了新字体后的刷新仍然找得到。
  void candidate_english_font_row(StackPanel const &group, bool catalog) {
    auto input = font_input(L"英文字体",
                            document_.String(L"candidate_english_font",
                                             nav::candidate_default_english_font),
                            [this](std::wstring const &font) {
                              document_.SetString(L"candidate_english_font", font);
                              schedule_save();
                              update_preview();
                            });
    const std::wstring subtitle =
        L"优先用于候选和预编辑；缺字时使用主字体，不限英文输入模式。保存后自动应用。";
    if (!catalog) {
      add_row(group, 0xE8D3, L"英文字体", subtitle, input);
      return;
    }
    auto [field, status] = font_field_with_catalog(input);
    add_row(group, 0xE8D3, L"英文字体", subtitle, field, status);
  }

  // 字体框和它旁边的「刷新」按钮（读屏名「刷新字体列表」），以及放在这一行下面的读取状态：共享页把原来单独的「系统字体列表」一行并进了第一个字体框。状态和按钮绑到这一行，show_font_catalog_status 在原处更新它们。Windows 只有一份字体目录状态，两个字体框共用，不会读两次。
  std::pair<FrameworkElement, TextBlock> font_field_with_catalog(FrameworkElement const &input) {
    StackPanel field;
    field.Orientation(Orientation::Horizontal);
    field.Spacing(8);
    field.Children().Append(input);
    auto refresh_button = button_control(
        L"刷新", [this] { load_font_catalog(); },
        loaded_ && font_catalog_status_ != FontCatalogStatus::loading);
    refresh_button.VerticalAlignment(VerticalAlignment::Top);
    A11y::SetName(refresh_button, hstring(L"刷新字体列表"));
    field.Children().Append(refresh_button);
    // 状态是读屏会播报的区域。
    auto status = make_text(font_catalog_message(), 12, palette_.sub);
    status.Margin(Thickness{0, 0, 0, 8});
    A11y::SetLiveSetting(status, Automation::Peers::AutomationLiveSetting::Polite);
    status.Visibility(status.Text().empty() ? Visibility::Collapsed : Visibility::Visible);
    if (!indexing_) {
      font_catalog_text_ = status;
      font_catalog_button_ = refresh_button.as<Button>();
    }
    return {field, status};
  }

  // 「候选字体」预设一行。它画在自己的容器里，主字体每输入一个字就在原处重画这一行，不必重建整页而丢掉输入框的焦点。末尾的「自定义…」只亮出主字体框、不改字体（共享页的 onCustomChange）；字体不是任何预设时选中的就是它。
  void candidate_font_preset_row(StackPanel const &host) {
    const auto family =
        document_.String(L"candidate_font_family", nav::candidate_default_font_family);
    const auto preset = nav::current_candidate_font_preset(
        family, document_.Strings(L"candidate_fallback_fonts"));
    std::vector<Option> options;
    for (auto const &entry : nav::candidate_font_presets)
      options.push_back({std::wstring(entry.id), std::wstring(entry.label),
                         nav::candidate_font_preset_available(entry.id)});
    options.push_back({L"custom", L"自定义…", true});
    add_row(host, 0xE8D2, L"候选字体",
            preset ? std::wstring(L"Windows 没有自带圆体")
                   : L"当前为自定义字体 " + family + L"，可在下方修改",
            segmented_control(L"候选字体", options,
                              preset && !candidate_font_custom_ ? std::wstring(*preset)
                                                                : std::wstring(L"custom"),
                              [this](std::wstring const &id) {
                                if (id == L"custom") {
                                  candidate_font_custom_ = true;
                                  queue_refresh();
                                  return;
                                }
                                candidate_font_custom_ = false;
                                change([&](PreferencesDocument &doc) {
                                  const auto patch = nav::candidate_font_preset_patch(
                                      id, doc.Strings(L"candidate_fallback_fonts"));
                                  doc.SetString(L"candidate_font_family", patch.family);
                                  doc.SetOrderedStrings(L"candidate_fallback_fonts",
                                                        patch.fallback);
                                }, true);
                              }));
  }

  void refresh_font_preset_row() {
    if (!font_preset_host_)
      return;
    font_preset_host_.Children().Clear();
    candidate_font_preset_row(font_preset_host_);
  }

  // 字体名输入框（共享页的 FontFamilyInput）：可以直接输入，也可以从系统字体列表里选。第一次获得焦点时才读取字体列表；还没打字时列出全部字体，打字后只列名字里含有输入内容的，最多 100 项。每输入一个字就写进设置，停下后保存；不合法的名字（空、超过 128 字节或含控制字符）不写入，输入框标红，与共享页不保存不合法字体的做法相同。
  FrameworkElement font_input(std::wstring const &label, std::wstring const &value,
                              std::function<void(std::wstring const &)> apply) {
    StackPanel box;
    box.Spacing(4);
    box.Width(240);
    AutoSuggestBox input;
    input.Text(hstring(value));
    input.UpdateTextOnSelect(false);
    input.IsEnabled(loaded_);
    A11y::SetName(input, hstring(label));
    auto note = make_text(L"", 12, palette_.sub);
    note.Visibility(Visibility::Collapsed);
    // 名字不合法时的说明，用共享页底部那条错误提示的规则部分（「请在候选窗口页修正字体」那半句指的是页面，这里就在输入框下面）；读屏从输入框的帮助文本读到它，对应共享输入框的 aria-invalid。
    const auto danger = palette_.dark ? rgba(0xFF, 0x99, 0xA4, 1) : rgba(0xC4, 0x2B, 0x1C, 1);
    auto invalid = make_text(L"名称不能为空、不能含控制字符或超过 128 个 UTF-8 字节。", 12, danger);
    invalid.TextWrapping(TextWrapping::Wrap);
    invalid.Visibility(Visibility::Collapsed);
    A11y::SetLiveSetting(invalid, Automation::Peers::AutomationLiveSetting::Assertive);
    box.Children().Append(input);
    box.Children().Append(invalid);
    box.Children().Append(note);

    // 用户打过字以后才按输入内容筛选，选中一项后又列出全部，与共享页的 search 状态相同。
    auto search = std::make_shared<bool>(false);
    auto focused = std::make_shared<bool>(false);
    std::function<void()> update_menu = [this, weak_input = make_weak(input),
                                         weak_note = make_weak(note), search, focused] {
      auto field = weak_input.get();
      auto hint = weak_note.get();
      if (!field || !hint || !*focused)
        return;
      const auto menu =
          nav::font_menu(font_catalog_, std::wstring(field.Text().c_str()), *search,
                         font_name_lower);
      std::vector<Inspectable> items;
      items.reserve(menu.options.size());
      for (auto const &font : menu.options)
        items.push_back(box_value(hstring(font)));
      field.ItemsSource(single_threaded_vector<Inspectable>(std::move(items)));
      std::wstring message;
      if (font_catalog_status_ == FontCatalogStatus::ready && menu.options.empty())
        message = L"没有匹配的字体，可继续手动输入。";
      else if (menu.matches > nav::font_menu_limit)
        message = L"显示前 100 项，请输入名称缩小范围。";
      hint.Text(hstring(message));
      hint.Visibility(message.empty() ? Visibility::Collapsed : Visibility::Visible);
      field.IsSuggestionListOpen(!menu.options.empty());
    };
    if (!indexing_)
      font_menu_updaters_.push_back(update_menu);

    auto write = [this, apply = std::move(apply), weak_input = make_weak(input),
                  weak_invalid = make_weak(invalid), danger](std::wstring const &font) {
      auto field = weak_input.get();
      auto message = weak_invalid.get();
      if (!field || !message)
        return;
      const bool valid = nav::valid_font_family(font);
      if (valid) {
        field.ClearValue(Control::BorderBrushProperty());
        field.ClearValue(A11y::HelpTextProperty());
      } else {
        field.BorderBrush(brush(danger));
        A11y::SetHelpText(field, message.Text());
      }
      // 只在显隐变化时通知读屏，免得每敲一个字都播报一遍。
      const auto shown = valid ? Visibility::Collapsed : Visibility::Visible;
      if (message.Visibility() != shown) {
        message.Visibility(shown);
        if (!valid)
          if (auto peer =
                  Automation::Peers::FrameworkElementAutomationPeer::FromElement(message))
            peer.RaiseAutomationEvent(Automation::Peers::AutomationEvents::LiveRegionChanged);
      }
      if (valid)
        apply(font);
    };

    input.GotFocus([this, focused, update_menu](Inspectable const &, RoutedEventArgs const &) {
      *focused = true;
      request_font_catalog();
      update_menu();
    });
    input.LostFocus([this, focused, weak_note = make_weak(note)](Inspectable const &,
                                                                 RoutedEventArgs const &) {
      *focused = false;
      if (auto hint = weak_note.get())
        hint.Visibility(Visibility::Collapsed);
      flush_pending();
    });
    input.TextChanged([search, update_menu, write](AutoSuggestBox const &sender,
                                                   AutoSuggestBoxTextChangedEventArgs const &args) {
      if (args.Reason() != AutoSuggestionBoxTextChangeReason::UserInput)
        return;
      *search = true;
      write(std::wstring(sender.Text().c_str()));
      update_menu();
    });
    input.QuerySubmitted([search, write](AutoSuggestBox const &sender,
                                         AutoSuggestBoxQuerySubmittedEventArgs const &args) {
      const auto item = args.ChosenSuggestion();
      if (!item)
        return;
      const std::wstring font = unbox_value_or<hstring>(item, L"").c_str();
      sender.Text(hstring(font));
      *search = false;
      sender.IsSuggestionListOpen(false);
      write(font);
    });
    return box;
  }

  std::wstring font_catalog_message() const {
    switch (font_catalog_status_) {
    case FontCatalogStatus::loading:
      return L"正在读取字体列表。";
    case FontCatalogStatus::failed:
      return L"读取字体列表失败，可重试或手动输入。";
    case FontCatalogStatus::ready:
      return font_catalog_.empty() ? L"系统字体列表为空，可手动输入。" : L"";
    case FontCatalogStatus::idle:
      break;
    }
    return L"";
  }

  // 第一次点开字体输入框时读取；读过以后只有「刷新字体列表」重读。
  void request_font_catalog() {
    if (font_catalog_status_ == FontCatalogStatus::idle)
      load_font_catalog();
  }

  // 在 UI 线程之外读取字体列表。读取期间列表是空的；新的一次读取开始后，较早那次的结果到了也丢弃。
  void load_font_catalog() {
    const auto generation = ++font_catalog_generation_;
    font_catalog_status_ = FontCatalogStatus::loading;
    font_catalog_.clear();
    show_font_catalog_status();
    auto fonts = std::make_shared<std::optional<std::vector<std::wstring>>>();
    run_host(
        [fonts] {
          *fonts = read_font_catalog();
          return Response{};
        },
        [this, generation, fonts](Response const &) {
          if (generation != font_catalog_generation_)
            return;
          font_catalog_status_ = *fonts ? FontCatalogStatus::ready : FontCatalogStatus::failed;
          font_catalog_ = *fonts ? std::move(**fonts) : std::vector<std::wstring>();
          show_font_catalog_status();
        });
  }

  // 在原处更新状态行、刷新按钮和正打开的下拉，不重建页面，免得输入框丢掉焦点。
  void show_font_catalog_status() {
    if (font_catalog_text_) {
      const auto message = font_catalog_message();
      font_catalog_text_.Text(hstring(message));
      font_catalog_text_.Visibility(message.empty() ? Visibility::Collapsed
                                                    : Visibility::Visible);
      if (auto peer = Automation::Peers::FrameworkElementAutomationPeer::FromElement(
              font_catalog_text_))
        peer.RaiseAutomationEvent(Automation::Peers::AutomationEvents::LiveRegionChanged);
    }
    if (font_catalog_button_)
      font_catalog_button_.IsEnabled(loaded_ &&
                                     font_catalog_status_ != FontCatalogStatus::loading);
    for (auto const &update : font_menu_updaters_)
      update();
  }

  // ---- 界面预览 ----

  FrameworkElement make_preview() {
    StackPanel holder;
    holder.Spacing(8);
    holder.Margin(Thickness{0, 8, 0, 12});
    auto label = make_text(L"实时预览", 13, palette_.text);
    label.FontWeight(Windows::UI::Text::FontWeight{600});
    holder.Children().Append(label);
    preview_host_ = Border();
    preview_host_.HorizontalAlignment(HorizontalAlignment::Center);
    ScrollViewer viewport;
    viewport.MaxHeight(current_page_ == "candidate" ? 160 : 248);
    viewport.HorizontalScrollBarVisibility(ScrollBarVisibility::Auto);
    viewport.VerticalScrollBarVisibility(ScrollBarVisibility::Auto);
    viewport.HorizontalContentAlignment(HorizontalAlignment::Center);
    viewport.Content(preview_host_);
    Border stage;
    stage.Background(brush(palette_.seg_well));
    stage.CornerRadius(CornerRadius{10, 10, 10, 10});
    stage.Padding(Thickness{16, 16, 16, 16});
    stage.Child(viewport);
    holder.Children().Append(stage);

    Grid controls;
    controls.ColumnSpacing(10);
    ColumnDefinition sample_column;
    sample_column.Width(GridLength{1, GridUnitType::Star});
    ColumnDefinition theme_column;
    theme_column.Width(GridLength{1, GridUnitType::Auto});
    controls.ColumnDefinitions().Append(sample_column);
    controls.ColumnDefinitions().Append(theme_column);
    TextBox sample;
    sample.FontSize(13);
    sample.MaxLength(256);
    sample.PlaceholderText(L"预览示例文字，以空格分隔");
    sample.Text(hstring(preview_sample_));
    A11y::SetName(sample, L"预览示例文字");
    A11y::SetAutomationId(sample, L"candidate-preview-sample");
    sample.TextChanged([this](Inspectable const &sender, TextChangedEventArgs const &) {
      preview_sample_ = sender.as<TextBox>().Text().c_str();
      update_preview();
    });
    controls.Children().Append(sample);
    auto theme = select_control(
        L"预览颜色模式",
        {{L"follow", L"跟随设置"}, {L"light", L"预览浅色"}, {L"dark", L"预览深色"}},
        preview_dark_ ? (*preview_dark_ ? L"dark" : L"light") : L"follow",
        [this](std::wstring const &value) {
          // 只改变预览，本机输入法和其他窗口继续使用保存的主题。
          preview_dark_ = value == L"follow" ? std::nullopt
                                            : std::optional<bool>(value == L"dark");
          update_preview();
        }, true, false);
    A11y::SetAutomationId(theme, L"candidate-preview-appearance");
    Grid::SetColumn(theme, 1);
    controls.Children().Append(theme);
    holder.Children().Append(controls);
    holder.Children().Append(make_text(
        L"输入示例文字比较字体，调整下方设置查看效果。", 11, palette_.sub));
    update_preview();
    return holder;
  }

  void update_preview() {
    if (!preview_host_)
      return;
    // 使用候选窗自身的调色板；预览可独立比较浅深色，不改变输入法的主题。
    const auto theme = candidate_theme(document_.String(L"global_theme", L"system"),
                                       true, preview_dark_);
    const bool vertical =
        document_.String(L"candidate_layout", L"vertical") == L"vertical";
    const auto count = std::clamp<int>(
        static_cast<int>(std::lround(document_.Number(L"candidate_page_size", 6))),
        1, 9);
    const double font = std::clamp(document_.Number(L"candidate_font_size", 18),
                                   12.0, 32.0);
    const bool preedit =
        document_.String(L"candidate_preedit_style", L"pinyin") != L"empty";
    const bool translations = document_.Boolean(L"candidate_translations", false) ||
                              document_.Boolean(L"candidate_english_gloss", false);
    auto main_family = document_.String(L"candidate_font_family", nav::candidate_default_font_family);
    if (main_family.empty())
      main_family = std::wstring(nav::candidate_default_font_family);
    auto english_family = document_.String(L"candidate_english_font", nav::candidate_default_english_font);
    if (english_family.empty())
      english_family = main_family;
    // 原生候选先用英文字体，再按保存的列表逐字回退；示例也沿用这个顺序。
    auto family_chain = english_family;
    for (const auto &family : document_.Strings(L"candidate_fallback_fonts"))
      if (!family.empty())
        family_chain += L"," + family;
    const auto preview_font = FontFamily(hstring(family_chain));
    // Scale, opacity and radius through the rules the card itself uses (CandidateWindowStyle.h). The package's own radius is not known here, so an unset radius previews the theme's.
    const auto style = candidate_window_style();
    const double scale = style.scale();
    const double radius = msime::windows::candidate_card_radius(
        style, std::nullopt, theme.radius);
    const double row_radius =
        msime::windows::candidate_row_radius(style, theme.item_radius,
                                             static_cast<float>(radius));

    preview_host_.Background(
        brush(to_color(msime::windows::candidate_faded(theme.surface, style))));
    preview_host_.BorderBrush(
        brush(to_color(msime::windows::candidate_faded(theme.border, style))));
    preview_host_.BorderThickness(Thickness{1, 1, 1, 1});
    preview_host_.CornerRadius(CornerRadius{radius, radius, radius, radius});
    preview_host_.Padding(
        Thickness{12 * scale, 8 * scale, 12 * scale, 10 * scale});
    A11y::SetName(preview_host_, L"候选窗口预览");

    StackPanel body;
    body.Spacing(6 * scale);
    StackPanel head;
    head.Orientation(Orientation::Horizontal);
    head.Spacing(12 * scale);
    head.Padding(Thickness{4 * scale, 0, 4 * scale, 0});
    auto mode = make_text(L"中", 13 * scale, to_color(theme.accent));
    mode.FontWeight(Windows::UI::Text::FontWeight{600});
    head.Children().Append(mode);
    // The candidate window draws the preedit in the accent colour at semibold.
    if (preedit) {
      auto pre = make_text(L"hou’xuan’xiang",
                           std::clamp(document_.Number(L"candidate_preedit_font_size", 15),
                                      12.0, 32.0) * scale,
                           to_color(theme.accent));
      pre.FontFamily(preview_font);
      pre.FontWeight(Windows::UI::Text::FontWeight{600});
      head.Children().Append(pre);
    }
    body.Children().Append(head);

    // 短示例循环填满设定的每页数量；清空或只输入空白时恢复内置示例。
    std::vector<std::wstring> samples;
    constexpr std::wstring_view whitespace = L" \t\r\n\v\f\u3000";
    for (size_t start = preview_sample_.find_first_not_of(whitespace);
         start != std::wstring::npos && samples.size() < preview_words.size();) {
      const auto end = preview_sample_.find_first_of(whitespace, start);
      samples.push_back(preview_sample_.substr(start, end == std::wstring::npos
                                                        ? end : end - start));
      start = end == std::wstring::npos ? end
                                        : preview_sample_.find_first_not_of(whitespace, end);
    }
    StackPanel list;
    list.Orientation(vertical ? Orientation::Vertical : Orientation::Horizontal);
    list.Spacing(4 * scale);
    for (int i = 0; i < count; ++i) {
      const auto &[default_word, gloss] = preview_words[static_cast<size_t>(i)];
      const std::wstring word = samples.empty() ? default_word
                                                : samples[static_cast<size_t>(i) % samples.size()];
      const bool highlighted = i == 0;
      Grid item;
      item.Padding(Thickness{6 * scale, 4 * scale, 10 * scale, 4 * scale});
      item.CornerRadius(CornerRadius{row_radius, row_radius, row_radius, row_radius});
      if (highlighted)
        item.Background(brush(to_color(theme.selected)));
      if (highlighted && theme.show_selected_bar) {
        Border bar;
        bar.Width(3 * scale);
        bar.Height(16 * scale);
        bar.CornerRadius(CornerRadius{1.5 * scale, 1.5 * scale, 1.5 * scale,
                                      1.5 * scale});
        bar.Background(brush(to_color(theme.accent)));
        bar.HorizontalAlignment(HorizontalAlignment::Left);
        bar.VerticalAlignment(VerticalAlignment::Center);
        bar.Margin(Thickness{-6 * scale, 0, 0, 0});
        item.Children().Append(bar);
      }
      StackPanel cell;
      cell.Orientation(Orientation::Horizontal);
      cell.Spacing(8 * scale);
      // Alpha 0 in the selected slots means "keep the unselected colour", as the renderer reads it. The translation draws in the package's translation colour, or else the number colour, as the renderer draws it.
      const auto number_color =
          msime::windows::candidate_row_number_color(theme, highlighted);
      const auto translation_color =
          msime::windows::candidate_row_translation_color(theme, highlighted);
      const auto row_color = msime::windows::candidate_row_text_color(
          theme, theme.text, highlighted, false);
      auto number =
          make_text(std::to_wstring(i + 1), 13 * scale, to_color(number_color));
      number.VerticalAlignment(VerticalAlignment::Center);
      cell.Children().Append(number);
      StackPanel words;
      auto candidate = make_text(word, font * scale, to_color(row_color));
      A11y::SetAutomationId(candidate, hstring(L"candidate-preview-word-" + std::to_wstring(i)));
      candidate.TextWrapping(TextWrapping::NoWrap);
      candidate.FontFamily(preview_font);
      words.Children().Append(candidate);
      if (translations && samples.empty()) {
        auto translation = make_text(gloss, 13 * scale, to_color(translation_color));
        translation.FontFamily(preview_font);
        words.Children().Append(translation);
      }
      cell.Children().Append(words);
      item.Children().Append(cell);
      list.Children().Append(item);
    }
    body.Children().Append(list);
    preview_host_.Child(body);
  }

  // ---- 候选窗口 ----

  // 共享设置「候选窗口」页的一列布局（appearance-page.tsx）：预览，「布局与字体」组的主流程只有排列方式、每页数量、候选字体（及按需出现的主字体）和字号，其余各行按共享页的顺序收进同一个「更多选项」。展开候选、页码和内边距在 Windows 上没有（能力位为假），不画。「游戏」组管游戏里候选窗能不能出现，是功能开关而不是外观，放在折叠区之后照常展开。
  void build_candidate_page(StackPanel const &page) {
    if (preview_visible_ && !indexing_)
      page.Children().Append(make_preview());

    auto layout = add_group(page, L"布局与字体");
    segment_row(layout, 0xE8FD, L"候选项排列方式", L"", L"candidate_layout",
                {{L"horizontal", L"横向"}, {L"vertical", L"纵向"}},
                L"vertical");
    slider_row(layout, 0xE8CB, L"每页候选项数量", L"每页显示的候选项（1–9）",
               L"candidate_page_size", 1, 9, 6);
    const bool main_font = candidate_font_rows(layout);
    slider_row(layout, 0xE8E9, L"候选窗字号", L"候选词的字号（12–32）",
               L"candidate_font_size", 12, 32, 18);

    // 搜索命中折叠区里的某一行时展开它（fold_holds_focus）。
    Expander more;
    more.Header(box_value(L"更多选项"));
    A11y::SetAutomationId(more, L"candidate-more");
    more.HorizontalAlignment(HorizontalAlignment::Stretch);
    more.HorizontalContentAlignment(HorizontalAlignment::Stretch);
    more.IsExpanded(candidate_more_open_ || fold_holds_focus(L"candidate-more"));
    more.Expanding([this](Expander const &, ExpanderExpandingEventArgs const &) {
      candidate_more_open_ = true;
    });
    more.Collapsed([this](Expander const &, ExpanderCollapsedEventArgs const &) {
      candidate_more_open_ = false;
    });
    StackPanel rows;
    rows.Spacing(4);
    fold_index_ = L"candidate-more";
    // 与共享设置「候选窗口」页的同名开关相同（`HostCapabilities::app_logo`）：新装默认关，缺值读成关。
    bool_row(rows, 0xE790, L"显示水杉 logo",
             L"在候选窗和悬浮工具栏左端显示水杉图标。", L"show_app_logo",
             false);
    bool_row(rows, 0xE718, L"候选窗口跟随光标",
             L"关闭后保持首次出现的位置，直到候选窗口消失。",
             L"candidate_follow_cursor", true);
    candidate_english_font_row(rows, !main_font);
    slider_row(rows, 0xE8E9, L"预编辑字号", L"候选窗口中拼音的字号（12–32）",
               L"candidate_preedit_font_size", 12, 32, 15);
    slider_row(rows, 0xE740, L"整体大小",
               L"候选窗口连同文字一起缩放（75–150%）",
               L"candidate_scale_percent", 75, 150, 100, 5, L"%");
    slider_row(rows, 0xE7B3, L"不透明度",
               L"只淡化候选框的底色和边框，文字保持清晰（50–100%）",
               L"candidate_opacity_percent", 50, 100, 100, 5, L"%");
    corner_radius_row(rows);
    select_row(
        rows, 0xE70F, L"候选窗口预编辑", L"", L"candidate_preedit_style",
        {{L"pinyin", L"拼音分词"}, {L"empty", L"不显示"}}, L"pinyin", true);
    select_row(rows, 0xE8D2, L"行内预编辑", L"", L"tsf_preedit_style",
               {{L"raw", L"原始按键"},
                {L"pinyin", L"拼音分词"},
                {L"empty", L"不显示"}},
               L"raw");
    fold_index_.clear();
    more.Content(rows);
    layout.Children().Append(more);

    auto games = add_group(page, L"游戏");
    bool_row(
        games, 0xE7FC, L"在游戏中显示候选窗口",
        L"游戏声明自己画候选却不画时，由水杉显示候选窗口。CS2、Dota 2 和使用 "
        L"SDL2 "
        L"的游戏会自动识别。候选窗口不出现或跑到左上角的游戏，可以加进「总是"
        L"显示」；游戏自带的候选被替换时，加进「从不显示」。对已打开的游戏，"
        L"切换一次输入法后生效。独占全屏下不显示，请把游戏改为无边框窗口。",
        L"game_compatibility.candidate_overlay", true);
    const bool gameProfile = msime::windows::game_profile::enabled();
    add_row(games, 0xE7FC, L"CS2 / Dota 2 原生候选兼容",
            L"将本版本的系统登记名称改为「拼音输入法」，让游戏自行绘制候选。"
            L"Windows 输入法列表也会显示这个名称，需要管理员授权并重启游戏；"
            L"关闭后恢复本版本名称。用户「总是显示」列表仍优先。",
            button_control(gameProfile ? L"恢复产品名称" : L"启用兼容名称", [this, gameProfile] {
              const auto error = msime::windows::game_profile::apply(GetActiveWindow(), !gameProfile);
              if (!error.empty()) show_notice(error);
              else show_notice(L"登记名称已更新，请重启游戏并重新切换输入法。");
              render_page(true);
            }));
    game_process_row(
        games, 0xE890, L"总是显示候选窗口的程序",
        L"候选窗口不出现或跑到左上角的游戏。填程序文件名，例如 game.exe。",
        L"game_compatibility.overlay_processes");
    game_process_row(
        games, 0xED1A, L"从不显示候选窗口的程序",
        L"自带候选被水杉的候选窗口替换的游戏。填程序文件名，例如 game.exe。",
        L"game_compatibility.excluded_processes");
  }

  // 圆角大小. A value overrides the skin package's radius; 默认 clears it, so the card follows the package again, else the theme. An unset radius shows the theme's on the slider.
  void corner_radius_row(StackPanel const &group) {
    const auto stored = document_.Value(L"candidate_corner_radius");
    const bool overridden = stored && stored.ValueType() == JsonValueType::Number;
    const double fallback =
        msime::windows::candidate_native_palette(candidate_dark()).radius;
    StackPanel box;
    box.Orientation(Orientation::Horizontal);
    box.Spacing(8);
    box.Children().Append(slider_control(
        L"圆角大小", 0, 16, overridden ? stored.GetNumber() : fallback,
        [this](double value) {
          document_.SetNumber(L"candidate_corner_radius", value);
          schedule_save();
          update_preview();
        },
        true, 1, L"pt"));
    box.Children().Append(button_control(L"默认", [this] {
      change([](PreferencesDocument &doc) {
        doc.SetValue(L"candidate_corner_radius", JsonValue::CreateNullValue());
      }, true);
    }, loaded_));
    add_row(group, 0xE739, L"圆角大小",
            overridden ? L"已覆盖皮肤包的圆角，点“默认”恢复跟随皮肤"
                : L"跟随皮肤；设定后会覆盖皮肤包的圆角（0–16pt）",
            box);
  }

  static const wchar_t *app_input_mode_rule_problem(nav::AppInputModeRuleError error) {
    switch (error) {
    case nav::AppInputModeRuleError::Empty:
      return L"请填写程序文件名，例如 code.exe。";
    case nav::AppInputModeRuleError::TooLong:
      return L"程序文件名不能超过 64 个字节（一个汉字算 3 个字节）。";
    case nav::AppInputModeRuleError::InvalidCharacter:
      return L"程序文件名不能包含控制字符。";
    case nav::AppInputModeRuleError::NotExe:
      return L"程序文件名要以 .exe 结尾，例如 code.exe。";
    case nav::AppInputModeRuleError::Duplicate:
      return L"这个程序已经有例外了，在下面的列表里改它的模式即可。";
    case nav::AppInputModeRuleError::TooMany:
      return L"最多 32 个应用例外，请先移除不再需要的。";
    }
    return L"";
  }

  // 文档里的应用例外，复制成一份可以改的对象。共享偏好里同一张表也装着 macOS 的 bundle id，它们原样带过去，不在这里丢掉。
  static JsonObject app_input_mode_rules_of(PreferencesDocument const &doc) {
    JsonObject rules;
    const auto stored = doc.Value(L"app_input_mode_rules");
    if (stored && stored.ValueType() == JsonValueType::Object)
      for (auto const &entry : stored.GetObject())
        rules.SetNamedValue(entry.Key(), JsonValue::Parse(entry.Value().Stringify()));
    return rules;
  }

  // 应用例外（`app_input_mode_rules`）：输入框加「添加」按钮，每条规则一行，可以选中文或英文、可以移除。新加的规则从中文开始，和 macOS 与共享设置页相同。写入前先按偏好库的规则规范化和校验（AppInputModeRuleList.h），不合法时在本行下面说明原因，不写入。
  void app_input_mode_rule_row(StackPanel const &group) {
    std::vector<std::pair<std::wstring, std::wstring>> rules;
    for (auto const &entry : app_input_mode_rules_of(document_))
      if (entry.Value().ValueType() == JsonValueType::String)
        rules.emplace_back(std::wstring(entry.Key().c_str()),
                           std::wstring(entry.Value().GetString().c_str()));
    std::sort(rules.begin(), rules.end());
    const auto set_rule = [this](std::wstring const &name,
                                 std::optional<std::wstring> const &mode) {
      change([&](PreferencesDocument &doc) {
        auto current = app_input_mode_rules_of(doc);
        if (mode)
          current.SetNamedValue(hstring(name), JsonValue::CreateStringValue(hstring(*mode)));
        else if (current.HasKey(hstring(name)))
          current.Remove(hstring(name));
        doc.SetValue(L"app_input_mode_rules", current);
      }, true);
    };
    InfoBar problem;
    problem.IsOpen(false);
    problem.IsClosable(true);
    problem.Severity(InfoBarSeverity::Error);
    problem.Margin(Thickness{0, 0, 0, 16});
    problem.Visibility(Visibility::Collapsed);
    problem.Closed([](InfoBar const &sender, InfoBarClosedEventArgs const &) {
      sender.Visibility(Visibility::Collapsed);
    });
    TextBox input;
    input.Width(180);
    input.PlaceholderText(L"例如 code.exe");
    input.IsEnabled(loaded_);
    A11y::SetName(input, L"应用例外的程序文件名");
    StackPanel box;
    box.Orientation(Orientation::Horizontal);
    box.Spacing(8);
    box.Children().Append(input);
    box.Children().Append(button_control(
        L"添加",
        [this, set_rule, weak_input = make_weak(input), weak_problem = make_weak(problem)] {
          auto field = weak_input.get();
          if (!field)
            return;
          const auto name = nav::normalize_app_input_mode_rule(std::wstring_view(field.Text()));
          std::vector<std::wstring> existing;
          for (auto const &entry : app_input_mode_rules_of(document_))
            existing.emplace_back(entry.Key().c_str());
          if (const auto error = nav::validate_app_input_mode_rule(name, existing)) {
            if (auto bar = weak_problem.get()) {
              bar.Message(hstring(app_input_mode_rule_problem(*error)));
              bar.Visibility(Visibility::Visible);
              bar.IsOpen(true);
            }
            return;
          }
          set_rule(name, std::wstring(L"chinese"));
        },
        loaded_));
    StackPanel below;
    if (!rules.empty()) {
      StackPanel list;
      list.Spacing(4);
      list.Margin(Thickness{0, 0, 0, 16});
      for (auto const &rule : rules) {
        const std::wstring name = rule.first;
        const std::wstring mode = rule.second;
        Grid item;
        item.ColumnSpacing(16);
        ColumnDefinition name_column;
        name_column.Width(GridLength{1, GridUnitType::Star});
        ColumnDefinition mode_column;
        mode_column.Width(GridLength{1, GridUnitType::Auto});
        ColumnDefinition action_column;
        action_column.Width(GridLength{1, GridUnitType::Auto});
        item.ColumnDefinitions().Append(name_column);
        item.ColumnDefinitions().Append(mode_column);
        item.ColumnDefinitions().Append(action_column);
        auto label = make_text(name, 14, palette_.text);
        label.VerticalAlignment(VerticalAlignment::Center);
        Grid::SetColumn(label, 0);
        item.Children().Append(label);
        auto mode_select = select_control(
            name + L" 的输入模式", {{L"chinese", L"中文"}, {L"english", L"英文"}}, mode,
            [set_rule, name](std::wstring const &value) { set_rule(name, value); });
        Grid::SetColumn(mode_select, 1);
        item.Children().Append(mode_select);
        auto remove_button = button_control(L"移除", [set_rule, name] {
          set_rule(name, std::nullopt);
        }, loaded_);
        A11y::SetName(remove_button, hstring(L"移除 " + name));
        Grid::SetColumn(remove_button, 2);
        item.Children().Append(remove_button);
        list.Children().Append(item);
      }
      below.Children().Append(list);
    }
    below.Children().Append(problem);
    add_row(group, 0xE71D, L"应用例外",
            L"切到这些程序时从指定的中文或英文开始，优先于上面的默认状态和记忆，「全局统一」下也生效；在程序里手动切换后，到下次切回这个程序之前不再套用。",
            box, below);
  }

  static const wchar_t *game_process_problem(nav::GameProcessError error) {
    switch (error) {
    case nav::GameProcessError::Empty:
      return L"请填写程序文件名，例如 game.exe。";
    case nav::GameProcessError::TooLong:
      return L"程序文件名不能超过 64 个字符。";
    case nav::GameProcessError::InvalidCharacter:
      return L"只填程序文件名，不要带文件夹路径，也不能包含 \\ / : * ? \" < > | 和控制字符。";
    case nav::GameProcessError::NotExe:
      return L"程序文件名要以 .exe 结尾，例如 game.exe。";
    case nav::GameProcessError::Duplicate:
      return L"这个程序已经在「总是显示」或「从不显示」列表中，同一个程序只能放在一张列表里。";
    case nav::GameProcessError::TooMany:
      return L"两张列表合计最多 32 个程序，请先移除不再需要的。";
    }
    return L"";
  }

  // 游戏组的程序列表：输入框加「添加」按钮，每个程序一行带「移除」。写入前先按偏好库的规则规范化和校验（GameProcessList.h），不合法时在本行下面说明原因，不写入。
  void game_process_row(StackPanel const &group, wchar_t glyph,
                        std::wstring const &title, std::wstring const &subtitle,
                        std::wstring key) {
    const auto names = document_.Strings(key);
    // 没有问题时收起，不占行下的空白；关掉提示后同样收起。
    InfoBar problem;
    problem.IsOpen(false);
    problem.IsClosable(true);
    problem.Severity(InfoBarSeverity::Error);
    problem.Margin(Thickness{0, 0, 0, 16});
    problem.Visibility(Visibility::Collapsed);
    problem.Closed([](InfoBar const &sender, InfoBarClosedEventArgs const &) {
      sender.Visibility(Visibility::Collapsed);
    });
    TextBox input;
    input.Width(180);
    input.PlaceholderText(L"例如 game.exe");
    input.IsEnabled(loaded_);
    A11y::SetName(input, hstring(title));
    StackPanel box;
    box.Orientation(Orientation::Horizontal);
    box.Spacing(8);
    box.Children().Append(input);
    box.Children().Append(button_control(
        L"添加",
        [this, key, weak_input = make_weak(input), weak_problem = make_weak(problem)] {
          auto field = weak_input.get();
          if (!field)
            return;
          const auto name = nav::normalize_game_process(std::wstring_view(field.Text()));
          if (const auto error = nav::validate_game_process(
                  name, document_.Strings(L"game_compatibility.overlay_processes"),
                  document_.Strings(L"game_compatibility.excluded_processes"))) {
            if (auto bar = weak_problem.get()) {
              bar.Message(hstring(game_process_problem(*error)));
              bar.Visibility(Visibility::Visible);
              bar.IsOpen(true);
            }
            return;
          }
          change([&](PreferencesDocument &doc) {
            auto current = doc.Strings(key);
            current.push_back(name);
            doc.SetStrings(key, std::move(current));
          }, true);
        },
        loaded_));
    StackPanel below;
    if (!names.empty()) {
      StackPanel list;
      list.Spacing(4);
      list.Margin(Thickness{0, 0, 0, 16});
      for (auto const &name : names) {
        Grid item;
        item.ColumnSpacing(16);
        ColumnDefinition name_column;
        name_column.Width(GridLength{1, GridUnitType::Star});
        ColumnDefinition action_column;
        action_column.Width(GridLength{1, GridUnitType::Auto});
        item.ColumnDefinitions().Append(name_column);
        item.ColumnDefinitions().Append(action_column);
        auto label = make_text(name, 14, palette_.text);
        label.VerticalAlignment(VerticalAlignment::Center);
        Grid::SetColumn(label, 0);
        item.Children().Append(label);
        auto remove_button = button_control(L"移除", [this, key, name] {
          change([&](PreferencesDocument &doc) {
            auto current = doc.Strings(key);
            current.erase(std::remove(current.begin(), current.end(), name),
                          current.end());
            doc.SetStrings(key, std::move(current));
          }, true);
        }, loaded_);
        A11y::SetName(remove_button, hstring(L"移除 " + name));
        Grid::SetColumn(remove_button, 1);
        item.Children().Append(remove_button);
        list.Children().Append(item);
      }
      below.Children().Append(list);
    }
    below.Children().Append(problem);
    add_row(group, glyph, title, subtitle, box, below);
  }

  // The style fields as the preview draws them. A document out of range previews the defaults; the card itself refuses such a document and keeps its previous style.
  msime::windows::CandidateWindowStyle candidate_window_style() const {
    msime::windows::CandidateWindowStyle style;
    style.scale_percent = static_cast<unsigned>(
        std::lround(document_.Number(L"candidate_scale_percent", 100)));
    style.opacity_percent = static_cast<unsigned>(
        std::lround(document_.Number(L"candidate_opacity_percent", 100)));
    const auto radius = document_.Value(L"candidate_corner_radius");
    if (radius && radius.ValueType() == JsonValueType::Number)
      style.corner_radius =
          static_cast<unsigned>(std::lround((std::max)(0.0, radius.GetNumber())));
    return style.valid() ? style : msime::windows::CandidateWindowStyle{};
  }

  // ---- 悬浮工具栏 ----

  struct ToolbarComponent {
    const wchar_t *id;
    const wchar_t *label;
    bool fallback;
    int button;
  };

  // 顺序和原生 floating_toolbar_slots 一致，预览与复选框使用同一张表。
  static constexpr std::array<ToolbarComponent, 10> toolbar_components_{{
      {L"english_mode", L"中英文切换", true, msime::windows::kToolbarLanguage},
      {L"input_scheme", L"切换输入方案", true,
       msime::windows::kToolbarInputScheme},
      {L"fullwidth", L"全角 / 半角", true, msime::windows::kToolbarFullwidth},
      {L"punctuation", L"中英文标点", true,
       msime::windows::kToolbarPunctuation},
      {L"character_set", L"简繁切换", true,
       msime::windows::kToolbarCharacterSet},
      {L"emoji", L"表情与符号", false, msime::windows::kToolbarEmoji},
      {L"handwriting", L"手写识别板", false,
       msime::windows::kToolbarHandwriting},
      {L"screen_keyboard", L"屏幕键盘", false,
       msime::windows::kToolbarScreenKeyboard},
      {L"voice", L"语音输入", false, msime::windows::kToolbarVoice},
      {L"settings", L"设置", true, msime::windows::kToolbarSettings},
  }};

  FrameworkElement make_toolbar_preview() {
    StackPanel holder;
    holder.Spacing(8);
    holder.Margin(Thickness{0, 8, 0, 12});
    auto title = make_text(L"效果预览", 13, palette_.text);
    title.FontWeight(Windows::UI::Text::FontWeight{600});
    holder.Children().Append(title);
    toolbar_preview_host_ = Border();
    toolbar_preview_host_.HorizontalAlignment(HorizontalAlignment::Center);
    A11y::SetAutomationId(toolbar_preview_host_, L"toolbar-preview");
    ScrollViewer viewport;
    viewport.HorizontalScrollBarVisibility(ScrollBarVisibility::Auto);
    viewport.VerticalScrollBarVisibility(ScrollBarVisibility::Disabled);
    viewport.HorizontalContentAlignment(HorizontalAlignment::Center);
    viewport.Content(toolbar_preview_host_);
    Border stage;
    stage.Background(brush(palette_.seg_well));
    stage.CornerRadius(CornerRadius{10, 10, 10, 10});
    stage.Padding(Thickness{16, 20, 16, 20});
    stage.Child(viewport);
    holder.Children().Append(stage);
    toolbar_preview_size_ = make_text(L"", 11, palette_.sub);
    A11y::SetAutomationId(toolbar_preview_size_, L"toolbar-preview-size");
    holder.Children().Append(toolbar_preview_size_);
    update_toolbar_preview();
    return holder;
  }

  void update_toolbar_preview() {
    if (!toolbar_preview_host_)
      return;
    const double scale =
        document_.Number(L"floating_toolbar.scale_percent", 100) / 100.0;
    const double font_size =
        document_.Number(L"floating_toolbar.font_size", 24);
    const bool logo = document_.Boolean(L"show_app_logo", false);
    const auto metrics =
        msime::windows::toolbar_metrics(font_size, false, logo);
    // 工具栏跟随颜色模式（Server 不读 toolbar_theme），预览也只看 `theme`。
    const auto common_mode = document_.String(L"theme", L"system");
    const bool dark = common_mode == L"dark" || (common_mode == L"system" && system_dark_);
    const auto colors = msime::windows::toolbar_palette(candidate_theme(
        document_.String(L"global_theme", L"system"), true, dark));
    std::vector<ToolbarComponent> components;
    for (const auto &item : toolbar_components_) {
      if (item.button == msime::windows::kToolbarHandwriting &&
          MSIME_EDITION_HANDWRITING == 0)
        continue;
      if (document_.Boolean(L"floating_toolbar." + std::wstring(item.id),
                            item.fallback))
        components.push_back(item);
    }
    const double width =
        msime::windows::toolbar_content_width(components.size(), metrics) *
        scale;
    const double height = metrics.height * scale;
    Canvas canvas;
    canvas.Width(width);
    canvas.Height(height);
    if (logo) {
      Image image;
      const auto rect = msime::windows::toolbar_logo(metrics);
      image.Width((rect.right - rect.left) * scale);
      image.Height((rect.bottom - rect.top) * scale);
      Canvas::SetLeft(image, rect.left * scale);
      Canvas::SetTop(image, rect.top * scale);
      show_logo(image,
                static_cast<int32_t>(std::ceil(metrics.icon * scale * 2)));
      canvas.Children().Append(image);
    } else {
      auto grip = make_text(L"⠿", 12 * scale, to_color(colors.number));
      Canvas::SetLeft(grip, 1 * scale);
      Canvas::SetTop(grip, (metrics.height - 16) * scale / 2);
      canvas.Children().Append(grip);
    }
    Border handle;
    handle.Width(2 * scale);
    handle.Height(height * 0.462);
    handle.CornerRadius(CornerRadius{scale, scale, scale, scale});
    handle.Background(brush(to_color(colors.accent)));
    Canvas::SetLeft(handle, (metrics.logo + 3) * scale);
    Canvas::SetTop(handle, height * 0.269);
    canvas.Children().Append(handle);
    msime::windows::ToolbarLanguageState language;
    language.scheme = msime::windows::scheme::scheme_from_name(utf8(
        hstring(document_.String(L"scheme", MSIME_EDITION_DEFAULT_SCHEME_W))));
    language.mode = msime::windows::scheme::input_mode(language.scheme);
    for (size_t index = 0; index < components.size(); ++index) {
      const auto &component = components[index];
      const auto rect = msime::windows::toolbar_cell(index, metrics);
      Border cell;
      cell.Width((rect.right - rect.left) * scale);
      cell.Height((rect.bottom - rect.top) * scale);
      Canvas::SetLeft(cell, rect.left * scale);
      Canvas::SetTop(cell, rect.top * scale);
      const bool chinese_mode = document_.String(L"default_ime_mode", L"chinese") != L"english";
      // 标点按钮照 TIP 的 ResolvePunctuationOpen 画：锁定时取锁定值，跟随时随中英文模式（PunctuationSettings.h 的 toolbar_chinese_punctuation）。
      const bool state =
          component.button == msime::windows::kToolbarLanguage
              ? chinese_mode
          : component.button == msime::windows::kToolbarFullwidth
              ? document_.String(L"character_width", L"halfwidth") ==
                    L"fullwidth"
          : component.button == msime::windows::kToolbarPunctuation
              ? ps::toolbar_chinese_punctuation(
                    chinese_mode, document_.String(L"punctuation_lock", L"follow"))
          : component.button == msime::windows::kToolbarCharacterSet
              ? document_.Boolean(L"traditional_chinese_output", false)
              : true;
      const auto icon =
          msime::windows::toolbar_icon(component.button, state, language);
      FrameworkElement glyph =
          icon.codepoint ? make_icon(icon.codepoint, metrics.icon * scale,
                                     to_color(colors.text))
                               .as<FrameworkElement>()
                         : make_text(icon.fallback, metrics.icon * scale,
                                     to_color(colors.text))
                               .as<FrameworkElement>();
      glyph.HorizontalAlignment(HorizontalAlignment::Center);
      glyph.VerticalAlignment(VerticalAlignment::Center);
      cell.Child(glyph);
      A11y::SetName(cell, hstring(component.label));
      ToolTipService::SetToolTip(cell, box_value(hstring(component.label)));
      canvas.Children().Append(cell);
    }
    toolbar_preview_host_.Background(brush(to_color(colors.surface)));
    const double border = colors.border_width * scale;
    const double radius = colors.radius * scale;
    toolbar_preview_host_.CornerRadius(
        CornerRadius{radius, radius, radius, radius});
    // 边框叠在固定尺寸的内容内，避免 XAML Border 把外观撑大并移动图标。
    Grid content;
    content.Width(width);
    content.Height(height);
    content.Children().Append(canvas);
    Border outline;
    outline.BorderBrush(brush(to_color(colors.border)));
    outline.BorderThickness(Thickness{border, border, border, border});
    outline.CornerRadius(CornerRadius{radius, radius, radius, radius});
    outline.IsHitTestVisible(false);
    content.Children().Append(outline);
    toolbar_preview_host_.Child(content);
    const auto status = document_.Boolean(L"floating_toolbar.enabled", true)
                            ? L"按默认输入状态预览，调整按钮和尺寸会同步变化。"
                            : L"工具栏已隐藏，按默认输入状态预览开启后的外观。";
    toolbar_preview_size_.Text(hstring(number_text(width) + L" × " +
                                       number_text(height) + L" DIP · " +
                                       status));
  }

  void build_toolbar_page(StackPanel const &page) {
    if (!indexing_)
      page.Children().Append(make_toolbar_preview());
    auto general = add_group(page, L"显示与组件");
    bool_row(general, 0xE7F4, L"在桌面显示悬浮工具栏",
             L"快速访问输入法状态与常用功能", L"floating_toolbar.enabled",
             true);
    // 工具栏跟随「主题」页的颜色模式和主题，没有自己的明暗；这一行只是指向那里（共享页 floating-toolbar-page.tsx 的「颜色与明暗」）。
    add_row(general, 0xE790, L"颜色与明暗", L"悬浮工具栏跟随「主题」页的颜色模式和主题",
            button_control(L"前往", [this] {
              focus_row_ = L"颜色模式";
              navigate("themes", true);
            }));
    std::vector<Check> checks;
    for (const auto &component : toolbar_components_) {
      if (component.button == msime::windows::kToolbarHandwriting &&
          MSIME_EDITION_HANDWRITING == 0)
        continue;
      const std::wstring key =
          L"floating_toolbar." + std::wstring(component.id);
      checks.push_back(
          {component.label, document_.Boolean(key, component.fallback),
           [this, key](bool on) {
             change([&](PreferencesDocument &doc) { doc.SetBoolean(key, on); },
                    false);
           }});
    }
    add_row(general, 0xE71D, L"工具栏按钮", L"按工具栏中的顺序选择要显示的功能",
            nullptr, checks_panel(std::move(checks)));
    // 缩放和图标尺寸相乘才是工具栏的大小，和共享页一样只给一个「工具栏大小」，选一档同时写两个值（见 FloatingToolbarSizeModel.h）。
    auto appearance = add_group(page, L"尺寸");
    add_row(appearance, 0xE740, L"工具栏大小", L"按钮和图标一起放大或缩小，不改变系统显示缩放",
            toolbar_size_control());
  }

  // 「工具栏大小」的下拉框，照共享页 floating-toolbar-page.tsx 的 SelectRow：存下的两个值凑不成任何一档时（以前分开调过），最前面多一项灰着的「自定义（缩放 X%，图标 Y）」并选着它，不改写存储；选一档后重画页面，这一项随之消失。
  FrameworkElement toolbar_size_control() {
    const int scale = static_cast<int>(
        std::lround(document_.Number(L"floating_toolbar.scale_percent", 100)));
    const int font = static_cast<int>(
        std::lround(document_.Number(L"floating_toolbar.font_size", 24)));
    const auto current = nav::toolbar_size::size_for(scale, font);
    std::vector<Option> options;
    if (!current)
      options.push_back({L"custom",
                         L"自定义（缩放 " + std::to_wstring(scale) + L"%，图标 " +
                             std::to_wstring(font) + L"）",
                         false});
    for (const auto &size : nav::toolbar_size::sizes)
      options.push_back({std::to_wstring(size.percent), std::to_wstring(size.percent) + L"%"});
    return select_control(
        L"工具栏大小", std::move(options),
        current ? std::to_wstring(current->percent) : std::wstring(L"custom"),
        [this](std::wstring const &value) {
          for (const auto &size : nav::toolbar_size::sizes) {
            if (std::to_wstring(size.percent) != value)
              continue;
            change([&](PreferencesDocument &doc) {
              doc.SetNumber(L"floating_toolbar.scale_percent", size.scale_percent);
              doc.SetNumber(L"floating_toolbar.font_size", size.font_size);
            }, true);
            return;
          }
        });
  }

  // ---- 输入 ----

  // One text's codes in the 86 and 98 tables, as `wubi_codes` answers them, short codes first.
  struct WubiTextCodes {
    std::wstring text;
    std::vector<std::wstring> wubi86, wubi98;
  };

  struct WubiCodeLookup {
    std::optional<WubiTextCodes> phrase;
    std::vector<WubiTextCodes> characters;
    bool truncated = false;
  };

  // 五笔查码, as the shared page's WubiCodeLookupSection (packages/ui/src/settings/wubi-code-lookup-section.tsx): the text goes to the dictionary request `wubi_codes`, read-only, and every distinct character (and the whole text as one word) is listed with its 86 and 98 codes, short codes first.
  void build_wubi_code_lookup(StackPanel const &page) {
    auto group = add_group(page, L"五笔查码");
    TextBox input;
    input.AcceptsReturn(true);
    input.TextWrapping(TextWrapping::Wrap);
    input.MinHeight(64);
    // The host refuses text over 4096 bytes; a thousand characters, even four-byte extension ones, stay inside it.
    input.MaxLength(1000);
    input.PlaceholderText(L"例如：水杉输入法");
    input.Text(hstring(wubi_lookup_text_));
    A11y::SetName(input, L"要查五笔编码的文字");
    Button search;
    search.Content(box_value(L"查码"));
    search.Height(32);
    search.Padding(Thickness{16, 0, 16, 0});
    const auto searchable = [this] {
      return !wubi_lookup_busy_ &&
             wubi_lookup_text_.find_first_not_of(L" \t\r\n") != std::wstring::npos;
    };
    search.IsEnabled(searchable());
    input.TextChanged([this, searchable, weak_search = make_weak(search)](
                          Inspectable const &sender, TextChangedEventArgs const &) {
      wubi_lookup_text_ = std::wstring(sender.as<TextBox>().Text().c_str());
      if (auto button = weak_search.get())
        button.IsEnabled(searchable());
    });
    search.Click([this](Inspectable const &, RoutedEventArgs const &) { lookup_wubi_codes(); });

    StackPanel below;
    below.Spacing(8);
    below.Margin(Thickness{0, 0, 0, 16});
    below.Children().Append(input);
    below.Children().Append(search);
    if (!wubi_lookup_error_.empty())
      below.Children().Append(make_text(wubi_lookup_error_, 12, palette_.text));
    if (wubi_lookup_) {
      std::vector<WubiTextCodes> rows;
      if (wubi_lookup_->phrase)
        rows.push_back(*wubi_lookup_->phrase);
      rows.insert(rows.end(), wubi_lookup_->characters.begin(), wubi_lookup_->characters.end());
      if (rows.empty())
        below.Children().Append(make_text(L"没有可以查的汉字。", 12, palette_.sub));
      else
        below.Children().Append(wubi_code_table(rows));
      if (wubi_lookup_->truncated)
        below.Children().Append(make_text(
            L"字太多，只列出了前 " + std::to_wstring(wubi_lookup_->characters.size()) + L" 个不同的字。",
            12, palette_.sub));
    }
    add_row(group, 0xE721, L"查汉字的五笔编码",
            L"输入或粘贴汉字，列出每个字的 86 和 98 五笔编码，简码在前、全码在后。打字时也能查：五笔拼音混输关闭时先按 z 再打拼音，候选后面标着它的五笔编码。",
            nullptr, below);
  }

  // The result table: 文字, then the 86 and 98 columns, the chosen table marked （当前）; a table without the text's codes shows —.
  FrameworkElement wubi_code_table(std::vector<WubiTextCodes> const &rows) {
    Grid table;
    table.ColumnSpacing(16);
    table.RowSpacing(6);
    for (auto width : {GridLength{1, GridUnitType::Auto}, GridLength{1, GridUnitType::Star},
                       GridLength{1, GridUnitType::Star}}) {
      ColumnDefinition column;
      column.Width(width);
      table.ColumnDefinitions().Append(column);
    }
    const auto profile = document_.String(L"wubi_profile", L"wubi86");
    const auto place = [&table](FrameworkElement const &cell, int32_t row, int32_t column) {
      Grid::SetRow(cell, row);
      Grid::SetColumn(cell, column);
      table.Children().Append(cell);
    };
    const auto add_row_definition = [&table] {
      RowDefinition definition;
      definition.Height(GridLength{1, GridUnitType::Auto});
      table.RowDefinitions().Append(definition);
    };
    add_row_definition();
    place(make_text(L"文字", 12, palette_.sub), 0, 0);
    place(make_text(profile == L"wubi86" ? L"86 五笔（当前）" : L"86 五笔", 12, palette_.sub), 0, 1);
    place(make_text(profile == L"wubi98" ? L"98 五笔（当前）" : L"98 五笔", 12, palette_.sub), 0, 2);
    const auto joined = [](std::vector<std::wstring> const &codes) {
      if (codes.empty())
        return std::wstring(L"—");
      std::wstring text;
      for (auto const &code : codes)
        text += (text.empty() ? L"" : L" ") + code;
      return text;
    };
    int32_t row = 1;
    for (auto const &item : rows) {
      add_row_definition();
      auto word = make_text(item.text, 14, palette_.text);
      word.IsTextSelectionEnabled(true);
      place(word, row, 0);
      for (int32_t column = 1; column <= 2; ++column) {
        auto codes = make_text(joined(column == 1 ? item.wubi86 : item.wubi98), 14, palette_.text);
        codes.FontFamily(FontFamily(L"Consolas"));
        codes.IsTextSelectionEnabled(true);
        place(codes, row, column);
      }
      ++row;
    }
    return table;
  }

  // Only one lookup at a time: the button is disabled while one runs, so the answer is always for the last text sent.
  void lookup_wubi_codes() {
    if (wubi_lookup_busy_)
      return;
    const auto options = runtime_options_document();
    if (!options) {
      wubi_lookup_.reset();
      wubi_lookup_error_ = L"查码失败，请稍后重试。";
      render_page(true);
      return;
    }
    JsonObject action;
    action.SetNamedValue(L"operation", JsonValue::CreateStringValue(L"wubi_codes"));
    action.SetNamedValue(L"text", JsonValue::CreateStringValue(hstring(wubi_lookup_text_)));
    wubi_lookup_busy_ = true;
    wubi_lookup_error_.clear();
    run_host(
        [options = *options, body = utf8(action.Stringify())] {
          return settings_dictionary_call(options, body);
        },
        [this](Response const &response) {
          wubi_lookup_busy_ = false;
          wubi_lookup_.reset();
          try {
            if (!response.ok)
              throw std::runtime_error("wubi_codes refused");
            const auto value = JsonObject::Parse(text(response.text)).GetNamedObject(L"value");
            const auto read = [](JsonObject const &object) {
              WubiTextCodes codes{named_string(object, L"text"), {}, {}};
              for (auto const &code : object.GetNamedArray(L"wubi86"))
                codes.wubi86.emplace_back(code.GetString().c_str());
              for (auto const &code : object.GetNamedArray(L"wubi98"))
                codes.wubi98.emplace_back(code.GetString().c_str());
              return codes;
            };
            WubiCodeLookup lookup;
            const auto phrase = value.GetNamedValue(L"phrase", JsonValue::CreateNullValue());
            if (phrase.ValueType() == JsonValueType::Object)
              lookup.phrase = read(phrase.GetObject());
            for (auto const &item : value.GetNamedArray(L"characters"))
              lookup.characters.push_back(read(item.GetObject()));
            lookup.truncated = value.GetNamedBoolean(L"truncated", false);
            wubi_lookup_ = std::move(lookup);
          } catch (...) {
            wubi_lookup_error_ = L"查码失败，请稍后重试。";
          }
          if (!closed_ && current_page_ == "typing")
            render_page(true);
        });
    render_page(true);
  }

  void build_typing_page(StackPanel const &page) {
    const auto scheme = document_.String(L"scheme", MSIME_EDITION_DEFAULT_SCHEME_W);
    auto language = add_group(page, L"中英文");
    segment_row(language, 0xE774, L"默认中英文", L"新焦点会话开始时使用的中文或英文状态",
                L"default_ime_mode", {{L"chinese", L"中文"}, {L"english", L"英文"}},
                L"english");
    segment_row(language, 0xE71D, L"中英文状态",
                L"按应用分别记忆输入状态，或让所有输入上下文保持同一状态",
                L"ime_mode_scope", {{L"app", L"按应用记忆"}, {L"global", L"全局统一"}},
                L"app");
    bool_row(language, 0xE946, L"中英文切换提示",
             L"切换输入模式后，在光标附近短暂显示“中”或“英”，不会抢占焦点。",
             L"input_mode_hud", true);
    app_input_mode_rule_row(language);

    auto schemes = add_group(page, L"输入方案");
    // 只列出本版本提供的方案。
    std::vector<Option> scheme_options;
    for (Option const &option : std::vector<Option>{
             {L"quanpin", L"全拼"}, {L"shuangpin", L"双拼"}, {L"wubi", L"五笔"},
             {L"cantonese", L"粤拼"}, {L"zhuyin", L"注音"},
             {L"japanese", L"日语"}, {L"korean", L"韩语"}, {L"vietnamese", L"越南语"},
             {L"tibetan", L"藏文"}, {L"stroke", L"笔画"}})
      if (edition_offers_scheme(option.value))
        scheme_options.push_back(option);
    for (const auto &option : scheme_options) {
      RadioButton radio;
      radio.Content(box_value(hstring(option.label + L"输入")));
      radio.GroupName(L"input-scheme");
      radio.MinHeight(32);
      radio.IsChecked(option.value == scheme);
      radio.IsEnabled(loaded_);
      radio.Checked([this, next = option.value](Inspectable const &, RoutedEventArgs const &) {
        select_scheme(next);
      });
      A11y::SetName(radio, hstring(option.label + L"输入"));
      A11y::SetAutomationId(radio, hstring(L"scheme-" + option.value));
      record(option.label + L"输入", L"输入方案");
      Border choice;
      choice.Padding(Thickness{0, 2, 0, 2});
      if (schemes.Children().Size() > 0) {
        choice.BorderBrush(brush(palette_.divider));
        choice.BorderThickness(Thickness{0, 1, 0, 0});
      }
      choice.Child(radio);
      schemes.Children().Append(choice);
      mark_search_target(option.label + L"输入", radio);
    }
    StackPanel scheme_options_group{nullptr};
    if (scheme == L"shuangpin" || scheme == L"wubi" || scheme == L"vietnamese" ||
        scheme == L"tibetan")
      scheme_options_group = add_group(page, L"当前方案选项");
    if (scheme == L"shuangpin") {
      select_row(scheme_options_group, 0xE8AB, L"双拼方案", L"", L"shuangpin_profile",
                 {{L"xiaohe", L"小鹤双拼"}, {L"ziranma", L"自然码双拼"},
                  {L"microsoft", L"微软双拼"}, {L"shoudao", L"首道双拼"},
                  {L"smart_abc", L"智能 ABC 双拼"}},
                 L"xiaohe");
      // 共享偏好 shuangpin_keymap_hint，与共享设置的同名开关相同；没选过时文档里没有这一项，按关显示。
      bool_row(scheme_options_group, 0xE765, L"输入时显示双拼键位提示",
               L"双拼输入时显示当前方案的键位图，完成上屏后自动隐藏。",
               L"shuangpin_keymap_hint", false);
      // 与共享设置「输入 › 方案」双拼详情里的「双拼预编辑」相同，只在双拼方案下出现：偏好是布尔值 shuangpin_preedit_uses_raw，这里按原始按键 / 拼音分词两项给出，缺省是原始按键。
      add_row(
          scheme_options_group, 0xE8AB, L"双拼预编辑",
          L"保留原始双拼按键，或显示展开后的拼音分词。",
          select_control(L"双拼预编辑",
                         {{L"raw", L"原始按键"}, {L"pinyin", L"拼音分词"}},
                         document_.Boolean(L"shuangpin_preedit_uses_raw", true)
                             ? L"raw"
                             : L"pinyin",
                         [this](std::wstring const &value) {
                           change(
                               [&](PreferencesDocument &doc) {
                                 doc.SetBoolean(L"shuangpin_preedit_uses_raw",
                                                value == L"raw");
                               },
                               false);
                         }));
    }
    if (scheme == L"vietnamese") {
      segment_row(scheme_options_group, 0xE8AB, L"越南语输入法", L"Telex 用字母打声调和变音，VNI 用数字键",
                  L"vietnamese.input_method", {{L"telex", L"Telex"}, {L"vni", L"VNI"}}, L"telex");
      segment_row(scheme_options_group, 0xE8D2, L"声调位置", L"oa、oe、uy 中声调标在哪个元音上：新式 hoà，旧式 hòa",
                  L"vietnamese.tone_style", {{L"modern", L"新式"}, {L"classic", L"旧式"}}, L"modern");
    }
    // 藏文只有威利转写一种输入法，没有可调的选项，这一行只说明按键。
    if (scheme == L"tibetan")
      add_row(scheme_options_group, 0xE8AB, L"藏文输入法",
              L"威利转写（EWTS）：用拉丁字母拼写，区分大小写。空格上屏并加音节点 ་，/ 上屏并加垂符 །，回车只上屏藏文，Esc 先显示拉丁原文",
              nullptr);
    if (scheme == L"wubi") {
      select_row(scheme_options_group, 0xE8AB, L"五笔方案", L"", L"wubi_profile",
                 {{L"wubi86", L"86 五笔"}, {L"wubi98", L"98 五笔"}}, L"wubi86");
      bool_row(scheme_options_group, 0xE8D2, L"五笔拼音混输",
               L"五笔候选之后接着列出同一串字母的全拼候选，五笔编码打不出时直接出拼音候选。",
               L"wubi_mixed_pinyin", MSIME_EDITION_WUBI_MIXED_PINYIN_DEFAULT != 0);
      bool_row(scheme_options_group, 0xE8CB, L"候选显示剩余编码",
               L"在候选后面标出还要再打哪几个字母才能单独打出它。已经打完整码的候选不标。",
               L"wubi_code_hint", true);
      build_wubi_candidate_selection(scheme_options_group);
      bool_row(scheme_options_group, 0xE8CB, L"四码唯一候选自动上屏",
               L"五笔输入达到四码且只有一个候选时，自动提交该候选。关闭后这个词留在候选列表里，用空格或数字键选它。",
               L"wubi_auto_commit_unique", true);
    }
    if (scheme == L"wubi")
      build_wubi_code_lookup(page);

    // 辅助码与搜索索引均只登记当前方案；切换方案后的修订会使索引重建。
    if (scheme == L"quanpin" || scheme == L"shuangpin") {
      if (!indexing_)
        load_helpcode_sources();
      auto helpcode = add_group(page, L"辅助码");
      if (edition_offers_scheme(L"quanpin") && scheme == L"quanpin")
        helpcode_rows(helpcode, L"全拼", L"quanpin", true, L"ziranma", false);
      if (edition_offers_scheme(L"shuangpin") && scheme == L"shuangpin")
        helpcode_rows(helpcode, L"双拼", L"shuangpin", true, L"lantian", true);
    }
  }

  void build_wubi_candidate_selection(StackPanel const &group) {
    const bool enabled = document_.Boolean(L"wubi_candidate_selection.enabled", false);
    auto toggle = toggle_control(L"五笔快捷选词", enabled, [this](bool on) {
      change([&](PreferencesDocument &doc) {
        doc.SetBoolean(L"wubi_candidate_selection.enabled", on);
      }, true);
    });
    A11y::SetAutomationId(toggle, L"wubi-candidate-selection");
    add_row(group, 0xE8C1, L"五笔快捷选词",
            L"组字时用两个标点选当前页第 2、3 候选；候选不足时仍输入标点。", toggle);
    if (!enabled)
      return;

    const auto second_key = document_.String(L"wubi_candidate_selection.second_key", L";");
    const auto third_key = document_.String(L"wubi_candidate_selection.third_key", L"'");
    TextBox second;
    TextBox third;
    for (auto const &field : {second, third}) {
      field.Width(80);
      field.MaxLength(1);
      field.IsSpellCheckEnabled(false);
      field.IsTextPredictionEnabled(false);
      field.IsEnabled(loaded_);
    }
    second.Text(hstring(second_key));
    third.Text(hstring(third_key));
    A11y::SetName(second, L"五笔第 2 候选键");
    A11y::SetName(third, L"五笔第 3 候选键");
    A11y::SetAutomationId(second, L"wubi-second-candidate-key");
    A11y::SetAutomationId(third, L"wubi-third-candidate-key");
    add_row(group, 0xE8C1, L"第 2 候选键", L"", second);
    add_row(group, 0xE8C1, L"第 3 候选键", L"", third);

    Button save;
    save.Content(box_value(L"保存选词键"));
    save.IsEnabled(false);
    A11y::SetName(save, L"保存五笔选词键");
    A11y::SetAutomationId(save, L"wubi-save-candidate-keys");
    // 两个字符一起交给共享偏好库校验和保存，允许交换键位，编辑中的值不进入文档。
    save.Click([this, weak_second = make_weak(second), weak_third = make_weak(third)](
                   Inspectable const &, RoutedEventArgs const &) {
      const auto second_field = weak_second.get();
      const auto third_field = weak_third.get();
      if (!second_field || !third_field)
        return;
      const std::wstring second_value = second_field.Text().c_str();
      const std::wstring third_value = third_field.Text().c_str();
      change([&](PreferencesDocument &doc) {
        doc.SetString(L"wubi_candidate_selection.second_key", second_value);
        doc.SetString(L"wubi_candidate_selection.third_key", third_value);
      }, true);
    });
    const auto edited = [weak_second = make_weak(second), weak_third = make_weak(third),
                         weak_save = make_weak(save), second_key, third_key,
                         loaded = loaded_](Inspectable const &, TextChangedEventArgs const &) {
      const auto second_field = weak_second.get();
      const auto third_field = weak_third.get();
      const auto button = weak_save.get();
      if (second_field && third_field && button)
        button.IsEnabled(loaded && (std::wstring_view(second_field.Text()) != second_key ||
                                    std::wstring_view(third_field.Text()) != third_key));
    };
    second.TextChanged(edited);
    third.TextChanged(edited);
    add_row(group, 0xE8C1, L"自定义选词键",
            L"使用两个不同的英文标点；请选择未用于翻页或以词定字的键。改完后一起保存。",
            save);
  }

  // 设置按任务归页，搜索索引随所属页面一起建立。
  void build_selection_keys(StackPanel const &page) {
    auto word = add_group(page, L"选词与翻页");
    const bool word_enabled = document_.Boolean(L"word_character.enabled", false);
    const auto word_keys = document_.String(L"word_character.keys", L"brackets");
    const auto word_current = word_enabled ? word_keys : std::wstring(L"off");
    // 同一键组不能同时用于以词定字和翻页：与共享页的 wordCharacterChoiceChange 相同，已用于翻页的键组不能选（当前选中的除外），选键组时不去关掉另一项设置。
    std::vector<Option> word_options{Option{L"off", L"关闭"}};
    for (Option option : std::vector<Option>{{L"brackets", L"[ / ]"}, {L"minus_equal", L"- / ="}}) {
      option.enabled = option.value == word_current ||
                       !document_.Boolean(L"navigation." + option.value, false);
      word_options.push_back(std::move(option));
    }
    add_row(word, 0xE8C1, L"以词定字",
            L"按所选键组的左键上屏高亮候选的首个汉字，右键上屏末个汉字；已用于翻页的键组不能选",
            select_control(L"以词定字", std::move(word_options), word_current,
                           [this](std::wstring const &value) {
                             change([&](PreferencesDocument &doc) {
                               // 选「关闭」只关开关，记着的键组不动。
                               if (value == L"off") {
                                 doc.SetBoolean(L"word_character.enabled", false);
                                 return;
                               }
                               doc.SetString(L"word_character.keys", value);
                               doc.SetBoolean(L"word_character.enabled", true);
                             }, true);
                           }));
    const bool second_third_enabled = document_.Boolean(L"second_third_candidate.enabled", false);
    add_row(word, 0xE8C1, L"二三候选",
            L"开启后，输入时按 ; 选第二个候选，按 ' 选第三个候选。组字中的 ' 不再作为音节分隔符。",
            select_control(L"二三候选",
                           {{L"off", L"关闭"}, {L"semicolon_quote", L"; / '"}},
                           second_third_enabled ? L"semicolon_quote" : L"off",
                           [this](std::wstring const &value) {
                             change([&](PreferencesDocument &doc) {
                               // 两个字段一起写，关闭时也写上键位，偏好里不会留下缺字段的对象。
                               doc.SetString(L"second_third_candidate.keys", L"semicolon_quote");
                               doc.SetBoolean(L"second_third_candidate.enabled", value != L"off");
                             }, false);
                           }));

    bool_row(word, 0xE8AB, L"退格键回退已选候选词",
             L"开启时，剩余拼写删到末尾会恢复最近一次选词的拼音；关闭时保留已选词，删空剩余拼写后上屏。",
             L"backspace_reverts_selection", true);

    // Tab 和 PageUp/PageDown 两组固定开启，不再列出（NavigationPolicy.h 的 preference_navigation，HostCapabilities.tab_page_paging_switches）：关掉只会让它们在组字时被吃掉，并不交还应用。
    const std::array<std::pair<const wchar_t *, const wchar_t *>, 6> paging_keys{{
        {L"minus_equal", L"- / ="},
        {L"comma_period", L", / ."},
        {L"brackets", L"[ / ]"},
        {L"mouse_wheel", L"鼠标滚轮（候选面板支持时翻页）"},
        {L"arrows", L"上 / 下（移动候选项）"},
        {L"left_right", L"左 / 右（移动候选项）"},
    }};
    std::vector<Check> paging_checks;
    for (const auto &[id, label] : paging_keys) {
      const std::wstring key = std::wstring(L"navigation.") + id;
      const std::wstring pair = id;
      paging_checks.push_back(
          {label, document_.Boolean(key, false),
           [this, key, pair, word_enabled, word_keys](bool on) {
             // 把以词定字占用的键组改为翻页时，同一次保存关闭以词定字。
             const bool conflict = on && word_enabled && pair == word_keys;
             // 以词定字的两个键组跟着翻页勾选变成可选或不可选，改它们时重画这一页。
             const bool word_group = pair == L"brackets" || pair == L"minus_equal";
             change([&](PreferencesDocument &doc) {
               doc.SetBoolean(key, on);
               if (conflict)
                 doc.SetBoolean(L"word_character.enabled", false);
             }, word_group);
           }});
    }
    add_row(word, 0xE8AB, L"翻页方式",
            L"选择用于翻页的按键；Shift+Tab / Tab 和 PageUp / PageDown 在候选窗口显示时始终翻页", nullptr,
            checks_panel(std::move(paging_checks), 190));

  }

  // 对话框持有草稿，所有字段确认后一次交给 Rust 校验并持久化。
  fire_and_forget edit_cloud_provider() {
    auto lifetime = get_strong();
    const auto stored = document_.Value(L"cloud_candidate_provider");
    if (!stored || stored.ValueType() != JsonValueType::Object) co_return;
    auto config = std::make_shared<JsonObject>(JsonObject::Parse(stored.Stringify()));
    StackPanel form;
    form.Spacing(12);
    form.Children().Append(make_text(L"仅发送当前组字，不发送已上屏内容、词库或历史。配置和凭据只保存在本机。公网使用 HTTPS，本机服务可用 HTTP。", 13, palette_.sub));
    form.Children().Append(select_control(L"云候选来源", {{L"google", L"Google input-tools"}, {L"custom", L"自定义 HTTP 接口"}},
      std::wstring(config->GetNamedString(L"source").c_str()), [config](std::wstring const &value) {
        config->SetNamedValue(L"source", JsonValue::CreateStringValue(hstring(value)));
      }));
    auto field = [&](wchar_t const *key, wchar_t const *title, bool multiline) {
      TextBox box;
      box.Header(box_value(hstring(title)));
      box.Text(config->GetNamedString(key, L""));
      box.MaxLength(4096);
      box.AcceptsReturn(multiline);
      if (multiline) box.MinHeight(80);
      form.Children().Append(box);
      return box;
    };
    const auto endpoint = field(L"endpoint", L"接口地址（GET 用 {query}）", false);
    form.Children().Append(select_control(L"请求方法", {{L"GET", L"GET"}, {L"POST", L"POST"}},
      std::wstring(config->GetNamedString(L"method").c_str()), [config](std::wstring const &value) {
        config->SetNamedValue(L"method", JsonValue::CreateStringValue(hstring(value)));
      }));
    const auto headers = field(L"headers_json", L"请求头（JSON，可填写 Authorization）", true);
    const auto body = field(L"body_template", L"请求体模板（JSON 字符串里的 {query} 自动转义）", true);
    form.Children().Append(select_control(L"响应格式", {{L"candidates", L"标准 candidates 数组"}, {L"google", L"Google input-tools"}, {L"json_path", L"JSON 路径"}},
      std::wstring(config->GetNamedString(L"response_format").c_str()), [config](std::wstring const &value) {
        config->SetNamedValue(L"response_format", JsonValue::CreateStringValue(hstring(value)));
      }));
    const auto path = field(L"response_path", L"JSON 路径（例如 data.0 或 choices.0.message.content）", false);
    NumberBox timeout;
    timeout.Header(box_value(L"超时（毫秒）"));
    timeout.Minimum(100); timeout.Maximum(10000);
    timeout.Value(config->GetNamedNumber(L"timeout_ms"));
    form.Children().Append(timeout);
    CheckBox cache;
    cache.Content(box_value(L"缓存云候选（仅本机内存，按来源隔离，无结果缓存 60 秒）"));
    cache.IsChecked(config->GetNamedBoolean(L"cache_enabled"));
    form.Children().Append(cache);
    auto error = make_text(L"", 13, palette_.sub);
    form.Children().Append(error);
    ScrollViewer scroll;
    scroll.Content(form); scroll.MaxHeight(480);
    ContentDialog dialog;
    dialog.Title(box_value(L"云候选服务"));
    dialog.Content(scroll);
    dialog.PrimaryButtonText(L"保存"); dialog.CloseButtonText(L"取消");
    dialog.XamlRoot(root_.XamlRoot());
    dialog.RequestedTheme(palette_.dark ? ElementTheme::Dark : ElementTheme::Light);
    dialog.PrimaryButtonClick([this, config, endpoint, headers, body, path, timeout, cache, error](
        ContentDialog const &, ContentDialogButtonClickEventArgs const &args) {
      config->SetNamedValue(L"endpoint", JsonValue::CreateStringValue(endpoint.Text()));
      config->SetNamedValue(L"headers_json", JsonValue::CreateStringValue(headers.Text()));
      config->SetNamedValue(L"body_template", JsonValue::CreateStringValue(body.Text()));
      config->SetNamedValue(L"response_path", JsonValue::CreateStringValue(path.Text()));
      if (!std::isfinite(timeout.Value())) { args.Cancel(true); return; }
      config->SetNamedValue(L"timeout_ms", JsonValue::CreateNumberValue(timeout.Value()));
      const auto checked = cache.IsChecked();
      config->SetNamedValue(L"cache_enabled", JsonValue::CreateBooleanValue(checked && checked.Value()));
      flush_pending();
      document_.SetValue(L"cloud_candidate_provider", *config);
      if (!commit()) {
        args.Cancel(true);
        error.Text(L"保存失败，请检查地址、请求头 JSON、请求体模板、响应路径和超时。");
      } else queue_refresh();
    });
    co_await show_dialog(dialog);
  }

  void build_input_habits(StackPanel const &page) {
    auto candidates = add_group(page, L"候选与联想");
    std::wstring cloud_recipient = L"Google input-tools（inputtools.google.com）";
    if (document_.String(L"cloud_candidate_provider.source", L"google") == L"custom") {
      cloud_recipient = L"所选自定义服务";
      try { cloud_recipient = Windows::Foundation::Uri(hstring(document_.String(L"cloud_candidate_provider.endpoint", L""))).Host().c_str(); }
      catch (...) {}
    }
    bool_row(candidates, 0xE753, L"云候选", L"仅将当前拼写发送给 " + cloud_recipient + L"；不发送已上屏内容、词库或历史",
             L"cloud_candidates", false);
    add_row(candidates, 0xE713, L"云候选服务", L"来源、GET/POST、请求头、JSON 模板、响应路径、超时和缓存",
            nullptr, button_control(L"配置", [this] { edit_cloud_provider(); }));
    // 触发字符数固定为默认的 5，不再给出（HostCapabilities.mixed_input_minimum_prefix）：调低会让常见简拼被英文词抢走首选。
    bool_row(candidates, 0xE774, L"中英混输", L"中文输入时在候选项中补充英文单词",
             L"mixed_input.english", true, true);
    // Windows 的 emoji 混输默认开（client-core 的 source_mixed_emoji_default），回退值与它一致；平时由出厂偏好兜底，读不到出厂偏好时才用到这里。
    bool_row(candidates, 0xE76E, L"emoji 混输",
             L"中文输入时在候选项中加入匹配的 emoji，紧跟在它描绘的那个词后面（如「美国」后面是 🇺🇸），对不上候选词的排在末尾",
             L"mixed_input.emoji", true);
    // 学习选词习惯决定下面「拼音方案调频」组画不画，改它时重画这一页。
    bool_row(candidates, 0xE82D, L"学习选词习惯", L"根据选词调整候选顺序", L"learning",
             true, true);
    // 与共享页一样，少有人改的混输开关收在组末的「更多选项」里。
    auto candidates_more = add_more_options(
        candidates, candidates_more_open_,
        focus_row_ == L"符号混输" || focus_row_ == L"颜文字混输" ||
            focus_row_ == L"日文混输（实验）" || focus_row_ == L"只出单字" ||
            focus_row_ == L"本地整句联想");
    // 符号混输是后加的可选键，默认开，文档里平时没有它，回退值必须是 true。
    bool_row(candidates_more, 0xE8EF, L"符号混输",
             L"打出符号的名称时在候选项中加入对应的符号，紧跟在那个词后面（如「正负」后面是 ±、「平方米」后面是 ㎡）；要打完整的名称才出现",
             L"mixed_input.symbols", true);
    bool_row(candidates_more, 0xE76E, L"颜文字混输",
             L"中文输入时在候选项中加入匹配的颜文字，紧跟在它描绘的那个词后面、排在同一个词的 emoji 之后，对不上候选词的排在末尾",
             L"mixed_input.kaomoji", false);
    // 不带临时日语的版本没有这项能力，host-api 也会把偏好固定为关。
    if constexpr (MSIME_EDITION_TEMPORARY_JAPANESE) {
      bool_row(candidates_more, 0xE76E, L"日文混输（实验）",
               L"全拼、双拼里打出完整的日文罗马字时补充一个日文候选（如 ohayou → おはよう）；词典不在时不生效",
               L"mixed_input.japanese", false);
    }
    bool_row(candidates_more, 0xE8C1, L"只出单字",
             L"候选只列单个汉字，不出词组和整句；选一个字后接着拼下一个字",
             L"single_character_only", false);
    bool_row(candidates_more, 0xE8C1, L"本地整句联想",
             L"把词库组合出的整句加入候选；关闭后仍保留单词候选",
             L"sentence_association.word_lattice", true);

    auto output = add_group(page, L"输出");
    bool_row(output, 0xE8C1, L"繁体输出", L"将提交的简体中文转换为繁体中文",
             L"traditional_chinese_output", false);
    bool_row(output, 0xE8D2, L"中英文之间加空格",
             L"一次上屏的文字里，汉字与英文字母、数字相邻处自动加一个空格，如「用Rust写」上屏为「用 Rust 写」。两次上屏之间不加；网址、日期等快捷模式和全角输入不受影响，译文和释义上屏、常用语、剪贴板和语音结果也不加",
             L"chinese_english_spacing", false);
    // 全角输入只是会话开始时的状态，工具栏和快捷键随时可切，收进「更多选项」。
    auto output_more = add_more_options(output, output_more_open_, focus_row_ == L"全角输入");
    add_row(output_more, 0xE8E9, L"全角输入",
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

    struct LocalMode {
      const wchar_t *id;
      const wchar_t *label;
      bool default_on;
    };
    // V、/、@ 会占用原本可直接输入的键，默认关闭。
    // 拆字反查（C 模式）同理在桌面默认关：打开后句首和中英混打的大写 C（C 盘、C 语言）会进模式。共享文档在用户动过开关之前没有这个键，host-api 在桌面构建里把缺键读作关。
    const std::array<LocalMode, 12> modes{{
        {L"quick_phrase", L"快捷短语(K 模式)", true},
        {L"date_time", L"日期与时间快捷输入(T 模式)", true},
        {L"unicode", L"Unicode 便捷录入(U 模式)", true},
        {L"emoji", L"Emoji 快捷输入(E 模式)", true},
        {L"kaomoji", L"颜文字快捷输入(M 模式)", true},
        {L"super_jianpin", L"超级简拼(J 模式)", true},
        {L"temporary_english", L"临时英文(Y 模式)", true},
        {L"temporary_japanese", L"临时日语(R 模式)", true},
        {L"chaizi", L"拆字反查(C 模式)", false},
        {L"expression", L"计算与数字(V 模式)", false},
        {L"command", L"指令(/ 模式)", false},
        {L"mention", L"@ 名字与地点(@ 模式)", false},
    }};
    std::vector<Check> checks;
    checks.reserve(modes.size());
    // 搜索落到某个模式的勾选（checks_panel 逐个登记）或这一行时展开「逐个模式」的折叠区。
    bool modes_focused = focus_row_ == L"快捷模式";
    for (const auto &[id, label, default_on] : modes) {
      // 不带临时日语的版本（版本表 features.temporary_japanese，host-api 也始终把它关掉）不列出这个开关。
      if constexpr (!MSIME_EDITION_TEMPORARY_JAPANESE) {
        if (std::wstring_view(id) == L"temporary_japanese")
          continue;
      }
      const std::wstring key = std::wstring(L"local_modes.") + id;
      modes_focused = modes_focused || focus_row_ == label;
      checks.push_back({label, document_.Boolean(key, default_on), [this, key](bool on) {
                          change([&](PreferencesDocument &doc) {
                            doc.SetBoolean(key, on);
                          }, false);
                        }});
    }
    auto local = add_group(page, L"快捷模式");
    // 总开关：关掉后没有输入时 Shift+字母照常打出大写字母，「逐个模式」里各模式的勾选保留原值；/ 和 @ 不经过它。
    bool_row(local, 0xE8D3, L"Shift+字母进入快捷模式",
             L"没有输入时按 Shift 加各模式的字母进入该模式。关掉后 Shift+字母照常打出大写字母，「逐个模式」里的开关保留，/ 和 @ 不受影响",
             L"local_modes.shift_letter_entry", true);
    // 与共享页一样，各模式的开关收进紧跟其后的「逐个模式」组的「更多选项」。
    auto modes_group = add_group(page, L"逐个模式");
    auto modes_more = add_more_options(modes_group, local_modes_more_open_, modes_focused);
    add_row(modes_more, 0xE945, L"快捷模式", L"用引导字母进入对应的输入模式；V、/、@ 只在拼音方案下没有输入时进入，/ 和 @ 还需中文标点", nullptr,
            checks_panel(std::move(checks), 250));

    auto correction = add_group(page, L"纠错与模糊音");
    bool_row(correction, 0xE70F, L"自动纠错", L"修正常见的相邻按键顺序。",
             L"quanpin.autocorrect_transposition", true);
    fuzzy_row(correction);

    // 与共享页的 FrequencySection 相同，只按条件显示，不改存着的值（这几项会上云）：学习选词习惯关闭时整组不画；调频方式为「关闭」时不画触发频次；步长只有线性调频读。
    if (document_.Boolean(L"learning", true)) {
      const auto frequency_mode = document_.String(L"frequency.mode", L"promote");
      auto frequency = add_group(page, L"拼音方案调频");
      select_row(frequency, 0xE8CB, L"调频方式", L"", L"frequency.mode",
                 {{L"disabled", L"关闭"}, {L"pin", L"一次置顶"}, {L"halve", L"折半调频"},
                  {L"linear", L"线性调频"}, {L"promote", L"一次置前"}},
                 L"promote", true);
      if (frequency_mode != L"disabled")
        number_select_row(frequency, 0xE8EF, L"触发频次(第几次上屏触发)", L"",
                          L"frequency.trigger_count", {1, 2, 3, 4, 5, 6}, 1, L"");
      if (frequency_mode == L"linear")
        number_select_row(frequency, 0xE8EF, L"线性调频步长", L"",
                          L"frequency.linear_step", {1, 2, 3, 4, 5, 6}, 1, L"");
    }
  }

  // 选择日语、韩语、越南语或藏语时记住被替换的中文方案，切回时回到它；选择中文方案（包括粤拼、注音和笔画）时记住这个方案。在这几种语言之间切换保留记住的方案，因为它们都不是存储接受的中文方案。规则与托盘相同（server_main.cpp 的 store_input_scheme）。
  void select_scheme(std::wstring const &next) {
    const auto current = document_.String(L"scheme", MSIME_EDITION_DEFAULT_SCHEME_W);
    if (current == next)
      return;
    const auto chinese = [](std::wstring const &value) {
      return value != L"japanese" && value != L"korean" && value != L"vietnamese" && value != L"tibetan";
    };
    change([&](PreferencesDocument &doc) {
      if (chinese(next))
        doc.SetString(L"last_chinese_scheme", next);
      else if (chinese(current))
        doc.SetString(L"last_chinese_scheme", current);
      doc.SetString(L"scheme", next);
    }, true);
  }

  // 一族（全拼或双拼）的辅助码：fallback 是 client-core 的 default_quanpin_helpcode / default_shuangpin_helpcode，只在拿不到出厂偏好时用到。
  // 方案下拉框照共享的 HelpcodeSettingsGroup 排列：内置方案、资源目录里的自定义码表、已安装的辅助码表插件（值为 pack:<插件 id>）。选插件只写 plugins.helpcode_pack_<族>，原来的方案留着；选内置或自定义方案时不再使用辅助码表插件，同时清空它。
  void helpcode_rows(StackPanel const &group, std::wstring const &label,
                     std::wstring const &family, bool enabled_fallback,
                     std::wstring const &schema_fallback, bool show_fallback) {
    const std::wstring prefix = family + L"_helpcode";
    const std::wstring schema_key = prefix + L".schema";
    const std::wstring pack_key = L"plugins.helpcode_pack_" + family;
    const bool enabled = document_.Boolean(prefix + L".enabled", enabled_fallback);
    // 共享页的 Shift 说明只在 helpcode_shift_entry 的宿主（Android、HarmonyOS、iOS）上显示，Windows 上这一行没有说明。
    bool_row(group, 0xE8CB, label + L"辅助码", L"", prefix + L".enabled", enabled_fallback, true);

    const auto schema = document_.String(schema_key, schema_fallback);
    const auto selected_pack = document_.String(pack_key, L"");
    std::vector<Option> options{
        {L"lantian", L"蓝天小雨点"}, {L"ziranma", L"自然码"},
        {L"shouyou2_0", L"首右2.0"}, {L"shouyouplus", L"首右plus"},
        {L"xiaohe", L"小鹤"}, {L"jiajia", L"加加"}, {L"wubi86", L"五笔 86"}};
    options.insert(options.end(), helpcode_custom_schemas_.begin(),
                   helpcode_custom_schemas_.end());
    const auto listed = [&options](std::wstring const &value) {
      return std::any_of(options.begin(), options.end(),
                         [&value](Option const &option) { return option.value == value; });
    };
    // 资源目录被更换或暂时读不到时，之前选中的自定义码表仍留在列表里。
    if (schema.starts_with(L"custom/") && !listed(schema))
      options.push_back({schema, schema});
    const std::wstring pack_prefix = L"pack:";
    bool pack_found = false;
    for (auto const &pack : helpcode_packs_) {
      options.push_back({pack_prefix + pack.value, pack.label + L"（插件）"});
      pack_found = pack_found || pack.value == selected_pack;
    }
    if (!selected_pack.empty() && !pack_found)
      options.push_back({pack_prefix + selected_pack, selected_pack + L"（插件，未找到）"});

    const auto title = label + L"辅助码方案";
    add_row(group, 0xE8D2, title, L"",
            select_control(title, std::move(options),
                           selected_pack.empty() ? schema : pack_prefix + selected_pack,
                           [this, schema_key, pack_key, pack_prefix](std::wstring const &value) {
                             change([&](PreferencesDocument &doc) {
                               if (value.starts_with(pack_prefix)) {
                                 doc.SetString(pack_key, value.substr(pack_prefix.size()));
                                 return;
                               }
                               doc.SetString(schema_key, value);
                               if (!doc.String(pack_key, L"").empty())
                                 doc.SetString(pack_key, L"");
                             }, true);
                           },
                           enabled));
    // 共享页的这一行不随辅助码开关变灰。
    bool_row(group, 0xE8FD, L"在候选窗口中显示" + label + L"辅助码", L"",
             prefix + L".show_in_candidate_window", show_fallback);
  }

  // 方案下拉框的两份来源。自定义码表只读一次，与共享页一样；辅助码表插件每次进入「输入」页重读，在别处导入或删除的包下次进来就能看到。两者都在 UI 线程之外读，读完内容有变化才重画。
  void load_helpcode_sources() {
    if (!helpcode_schemas_requested_) {
      helpcode_schemas_requested_ = true;
      run_host(helpcode_schemas_call, [this](Response const &response) {
        auto schemas = custom_helpcode_options(response);
        if (!same_options(schemas, helpcode_custom_schemas_)) {
          helpcode_custom_schemas_ = std::move(schemas);
          redraw_typing();
        }
      });
    }
    if (helpcode_packs_stale_ && !helpcode_packs_busy_) {
      helpcode_packs_stale_ = false;
      helpcode_packs_busy_ = true;
      run_host(plugin_catalog_call, [this](Response const &response) {
        helpcode_packs_busy_ = false;
        auto packs = helpcode_pack_options(response);
        if (!same_options(packs, helpcode_packs_)) {
          helpcode_packs_ = std::move(packs);
          redraw_typing();
        } else if (helpcode_packs_stale_ && !closed_ && current_page_ == "typing") {
          // 读取期间又进了一次「输入」页：按那一次再读。
          load_helpcode_sources();
        }
      });
    }
  }

  static bool same_options(std::vector<Option> const &a, std::vector<Option> const &b) {
    return std::equal(a.begin(), a.end(), b.begin(), b.end(),
                      [](Option const &x, Option const &y) {
                        return x.value == y.value && x.label == y.label;
                      });
  }

  void redraw_typing() {
    if (!closed_ && current_page_ == "typing")
      render_page(true);
  }

  // ---- 标点与翻译 ----

  void build_expression_page(StackPanel const &page) {
    // 与共享页 PunctuationSection 相同，主流程只留三行：「标点」选择、引号样式、成对标点；方头括号样式、智能标点总开关和它的四个细分开关收进「更多选项」（智能标点默认开，用户不必先找到它）。规则在 PunctuationSettings.h。
    auto punctuation = add_group(page, L"标点");
    // 「中文标点」开关和「固定标点」合成一个「标点」选择（PunctuationModeRow）：显示值每次从两个键推出，锁定优先，打开页面不改写文档；只有用户选了一项才写入。
    {
      std::vector<Option> modes;
      for (auto const &choice : ps::mode_choices)
        modes.push_back({std::wstring(choice.value), std::wstring(choice.label)});
      const std::wstring mode(ps::mode(document_.Boolean(L"chinese_punctuation", true),
                                       document_.String(L"punctuation_lock", L"follow")));
      add_row(punctuation, 0xE8C1, L"标点", std::wstring(ps::mode_description),
              select_control(L"标点", std::move(modes), mode, [this](std::wstring const &value) {
                const auto patch = ps::mode_patch(value);
                if (!patch)
                  return;
                change([&](PreferencesDocument &doc) {
                  if (patch->chinese_punctuation)
                    doc.SetBoolean(L"chinese_punctuation", *patch->chinese_punctuation);
                  doc.SetString(L"punctuation_lock", patch->punctuation_lock);
                }, true);
              }));
    }
    // 引号样式只在中文标点下起作用，所以紧跟在「标点」之后。取值与共享设置页、偏好文档相同。
    select_row(punctuation, 0xE8C1, L"引号样式",
               L"中文标点下 ' 键和 Shift+' 键输出的引号。直角引号：' 出「」，Shift+' 出『』",
               L"quote_style",
               {{L"curly", L"弯引号 “ ” ‘ ’"}, {L"corner", L"直角引号 「 」『 』"},
                {L"corner_when_traditional", L"繁体输出时用直角引号"}},
               L"curly");
    bool_row(punctuation, 0xE943, L"成对标点自动补全",
             L"输入左侧符号时自动补全右侧符号，并将光标置于中间",
             L"paired_punctuation", false);
    // 「更多选项」：方头括号样式、智能标点总开关和它的四个细分开关。细分开关只在智能标点打开时才起作用，所以总开关关着时变灰，值不动。搜索落在其中任何一行时展开。Windows 没有「大写锁定时使用英文标点」一行（HostCapabilities 的 caps_lock_punctuation 只在 macOS 为真）。
    Expander more;
    more.Header(box_value(L"更多选项"));
    more.HorizontalAlignment(HorizontalAlignment::Stretch);
    more.HorizontalContentAlignment(HorizontalAlignment::Stretch);
    more.IsExpanded(punctuation_more_open_ || focus_row_ == L"方头括号样式" ||
                    focus_row_ == L"智能标点" || focus_row_ == L"重复标点转中文" ||
                    focus_row_ == L"中文标点后按空格转换" || focus_row_ == L"数字后直出" ||
                    focus_row_ == L"字母后直出");
    more.Expanding([this](Expander const &, ExpanderExpandingEventArgs const &) {
      punctuation_more_open_ = true;
    });
    more.Collapsed([this](Expander const &, ExpanderCollapsedEventArgs const &) {
      punctuation_more_open_ = false;
    });
    StackPanel rows;
    rows.Spacing(4);
    select_row(rows, 0xE8C1, L"方头括号样式", L"中文标点下 [ ] 键输出的括号",
               L"bracket_style", {{L"solid", L"实心 【 】"}, {L"hollow", L"空心 〖 〗"}}, L"solid");
    // 智能标点（SmartPunctuationRow）：用户切换时连同数字后直出、字母后直出一起写成同一个值，TIP 的 ResolvePunctuation 要所在一侧的直出也开着才输出 ASCII。四个键默认都是开（client-core 的 smart_punctuation_default，读进来的文档总带着这些键，这里的回退值只是与它一致）。只写过总开关的旧文档在说明里多一句提示，不改写存储。
    const bool smart = document_.Boolean(L"smart_punctuation", true);
    const bool inert =
        ps::smart_punctuation_inert(smart, document_.Boolean(L"smart_punctuation_direct_digit", true),
                                    document_.Boolean(L"smart_punctuation_direct_letter", true));
    add_row(rows, 0xE945, L"智能标点",
            std::wstring(inert ? ps::smart_punctuation_inert_description
                               : ps::smart_punctuation_description),
            toggle_control(L"智能标点", smart, [this](bool value) {
              change([&](PreferencesDocument &doc) {
                for (auto const key : ps::smart_punctuation_keys)
                  doc.SetBoolean(key, value);
              }, true);
            }));
    bool_row(rows, 0xE8EE, L"重复标点转中文",
             L"智能标点输出英文标点后，2 秒内再次输入同一标点时替换为中文标点",
             L"smart_punctuation_repeat", true, false, smart);
    bool_row(rows, 0xE8EE, L"中文标点后按空格转换",
             L"刚输入中文标点后按空格，转换为对应英文标点",
             L"smart_punctuation_space_convert", false, false, smart);
    bool_row(rows, 0xE8EE, L"数字后直出",
             L"数字后输入逗号、句点或冒号时保留 ASCII 标点",
             L"smart_punctuation_direct_digit", true, true, smart);
    bool_row(rows, 0xE8EE, L"字母后直出",
             L"字母后输入逗号、句点或冒号时保留 ASCII 标点",
             L"smart_punctuation_direct_letter", true, true, smart);
    more.Content(rows);
    punctuation.Children().Append(more);

    build_input_habits(page);

    auto gloss = add_group(page, L"多语言与释义");
    bool_row(gloss, 0xE82D, L"显示英文释义",
             L"在候选词后面标出它的英文意思，中文候选给英文、英文候选给中文。释义来自随键盘打包的离线词库，不联网。",
             L"candidate_english_gloss", false, true);

    auto candidates = add_group(page, L"候选词翻译");
    const bool translations = document_.Boolean(L"candidate_translations", false);
    // 与共享页的 candidateGlossLanguagesEnabled 相同：目标语言和第二种语言也决定离线释义查哪种语言（host-api 的 msime_client_translation_query 在只开英文释义时也按它们给出 offline_gloss_languages），所以翻译或英文释义打开时都可选。
    const bool glosses = translations || document_.Boolean(L"candidate_english_gloss", false);
    bool_row(candidates, 0xE8C1, L"候选词翻译", L"为当前候选请求翻译结果并显示在候选行",
             L"candidate_translations", false, true);
    select_row(candidates, 0xE774, L"目标语言", L"候选词翻译目标语言",
               L"translation_target_language",
               {{L"en", L"英语"}, {L"fr", L"法语"}, {L"ja", L"日语"}, {L"es", L"西班牙语"},
                {L"ru", L"俄语"}, {L"de", L"德语"}, {L"ko", L"韩语"}},
               L"en", false, glosses);
    // 「显示读音」是释义的子行，与共享页一样画在目标语言之后，只在任一释义来源（英文释义或候选词翻译）打开时画出来（建搜索索引时照样收录，与下面几组服务设置相同）；两个开关都会重画，已存的值不动。日文罗马音由系统的微软日语输入法（IFELanguage）读出，没有它时只读纯假名的释义。
    if (glosses || indexing_)
      bool_row(candidates, 0xE8D4, L"显示读音",
               L"在释义后面标出怎么读：英文释义给音标，日文释义给罗马音。音标来自随输入法打包的离线词表，罗马音由系统生成，都不联网。",
               L"candidate_pronunciation", false);
    // 第二种语言：候选窗在第一种语言下面再画一行。和共享页 CandidateTranslationOptionsSection 的默认布局一样收进「更多选项」（它会上云，所以只折叠不砍），翻译或英文释义打开时可选，「不显示第二种语言」写回 null。
    {
      Expander translation_more;
      translation_more.Header(box_value(L"更多选项"));
      translation_more.HorizontalAlignment(HorizontalAlignment::Stretch);
      translation_more.HorizontalContentAlignment(HorizontalAlignment::Stretch);
      translation_more.IsExpanded(translation_more_open_ || focus_row_ == L"第二种语言");
      translation_more.Expanding([this](Expander const &, ExpanderExpandingEventArgs const &) {
        translation_more_open_ = true;
      });
      translation_more.Collapsed([this](Expander const &, ExpanderCollapsedEventArgs const &) {
        translation_more_open_ = false;
      });
      StackPanel translation_rows;
      translation_rows.Spacing(4);
      add_row(translation_rows, 0xE774, L"第二种语言", L"候选词下方可同时显示第二种释义",
              select_control(L"第二种语言",
                             {{L"", L"不显示第二种语言"}, {L"en", L"英语"}, {L"fr", L"法语"},
                              {L"ja", L"日语"}, {L"es", L"西班牙语"}, {L"ru", L"俄语"},
                              {L"de", L"德语"}, {L"ko", L"韩语"}},
                             document_.String(L"translation_secondary_language", L""),
                             [this](std::wstring const &value) {
                               change([&](PreferencesDocument &doc) {
                                 if (value.empty())
                                   doc.SetValue(L"translation_secondary_language",
                                                JsonValue::CreateNullValue());
                                 else
                                   doc.SetString(L"translation_secondary_language", value);
                               }, false);
                             },
                             glosses));
      translation_more.Content(translation_rows);
      candidates.Children().Append(translation_more);
    }
    // 翻译服务：共享页「翻译服务」一行（`translation-service-selector-section.tsx`），候选词翻译关着时不可选。Windows 的 Server 会问水杉账号，所以和共享页一样列出它。选了腾讯云、小牛翻译或自定义服务时，下面多一组只属于它的设置；规则在 TranslationServiceSettings.h。
    // `none` 不叫「关闭」：Windows 没有系统翻译，但 host-api 仍把装好的离线词库语言放进查询，TranslationWorker 照样从离线词库填出释义，所以与共享页 offlineFallback 的 dictionary 一支同名同说明。
    const std::wstring provider(nav::translation_provider(translation_choice()));
    add_row(candidates, 0xE774, L"翻译服务",
            L"不联网，候选只显示离线词库查得到的释义：英文来自「显示英文释义」，其他语言要装有对应的离线词库；选择在线服务后由该服务翻译",
            select_control(L"候选词翻译服务",
                           {{L"none", L"不使用在线服务"},
                            {L"tencent", L"腾讯云机器翻译"},
                            {L"niutrans", L"小牛翻译（NiuTrans）"},
                            {L"custom", L"自定义 DeepLX 兼容服务"},
                            {L"account", L"水杉账号（候选词发送到 api.msime.app）"}},
                           provider,
                           [this](std::wstring const &value) { select_translation_provider(value); },
                           translations));
    if (!indexing_)
      credential_test_views_.clear();
    if (provider == L"tencent" || indexing_)
      build_tencent_translation_group(page, translations);
    if (provider == L"niutrans" || indexing_)
      build_niutrans_group(page, translations);
    if (provider == L"custom" || indexing_)
      build_custom_translation_group(page, translations);
  }

  nav::TranslationServiceChoice translation_choice() const {
    nav::TranslationServiceChoice choice;
    choice.account = document_.Boolean(L"translation_account", false);
    choice.niutrans = document_.Boolean(L"niutrans.enabled", false);
    choice.custom = document_.Boolean(L"custom_translation.enabled", false);
    choice.tencent = document_.Boolean(L"tencent_tmt.enabled", true);
    choice.tencent_secret_id = document_.String(L"tencent_tmt.secret_id", L"");
    choice.tencent_secret_key = document_.String(L"tencent_tmt.secret_key", L"");
    choice.tencent_chosen = tencent_chosen_;
    return choice;
  }

  // 共享页的 setTranslationProvider：只打开所选的一种服务，其余两种关掉；选水杉账号时写 translation_account，否则去掉这一项（读作关）。凭据都留着，换回来时还在。
  void select_translation_provider(std::wstring const &provider) {
    // 腾讯云默认就是 enabled，选它不改文档；记下这次选择，没有可用密钥时也显示腾讯云和它的凭据框。
    tencent_chosen_ = provider == L"tencent";
    change([&](PreferencesDocument &doc) {
      doc.SetBoolean(L"custom_translation.enabled", provider == L"custom");
      doc.SetBoolean(L"tencent_tmt.enabled", provider == L"tencent");
      doc.SetBoolean(L"niutrans.enabled", provider == L"niutrans");
      if (provider == L"account")
        doc.SetBoolean(L"translation_account", true);
      else
        doc.Remove(L"translation_account");
    }, true);
  }

  // 校验提示的颜色，与字体输入框的错误提示相同。
  Color danger_color() const {
    return palette_.dark ? rgba(0xFF, 0x99, 0xA4, 1) : rgba(0xC4, 0x2B, 0x1C, 1);
  }

  // 分组里的一条警告（共享页的 `SettingsWarning`），文字为空时收起；读屏在文字变化时播报。
  TextBlock group_warning() {
    auto warning = make_text(L"", 13, danger_color());
    warning.Margin(Thickness{4, 2, 4, 4});
    warning.Visibility(Visibility::Collapsed);
    A11y::SetLiveSetting(warning, Automation::Peers::AutomationLiveSetting::Polite);
    return warning;
  }

  static void show_warning(TextBlock const &warning, std::wstring const &message) {
    if (std::wstring_view(warning.Text()) == message)
      return;
    warning.Text(hstring(message));
    warning.Visibility(message.empty() ? Visibility::Collapsed : Visibility::Visible);
    if (!message.empty())
      if (auto peer = Automation::Peers::FrameworkElementAutomationPeer::FromElement(warning))
        peer.RaiseAutomationEvent(Automation::Peers::AutomationEvents::LiveRegionChanged);
  }

  // 翻译服务的一个凭据输入框：普通的（共享页的 TextInputRow、EndpointSettingRow），或带「显示」「隐藏」按钮的密钥框（SecretInput）。每输入一个字就交给 edit，它更新本组的提示和测试按钮，并在偏好库接受这个值时写进设置、停下后保存，返回是否写入；不接受的值不写入，输入框标红，与候选字体输入框的做法相同。整页不重画，输入框不会丢掉焦点。
  FrameworkElement credential_input(std::wstring const &label, std::wstring const &value,
                                    bool secret, std::wstring const &placeholder,
                                    bool enabled, std::function<bool(std::wstring const &)> edit) {
    // 控件刚建好时也会报一次内容变化，内容没变就不算编辑。
    auto last = std::make_shared<std::wstring>(value);
    auto changed = [edit = std::move(edit), last, danger = danger_color()](
                       Control const &field, std::wstring const &text) {
      if (text == *last)
        return;
      *last = text;
      if (edit(text))
        field.ClearValue(Control::BorderBrushProperty());
      else
        field.BorderBrush(brush(danger));
    };
    if (!secret) {
      TextBox input;
      input.Width(240);
      input.Text(hstring(value));
      input.PlaceholderText(hstring(placeholder));
      input.IsSpellCheckEnabled(false);
      input.IsEnabled(enabled && loaded_);
      A11y::SetName(input, hstring(label));
      input.TextChanged([changed](Inspectable const &sender, TextChangedEventArgs const &) {
        auto field = sender.as<TextBox>();
        changed(field, std::wstring(field.Text().c_str()));
      });
      return input;
    }
    StackPanel box;
    box.Orientation(Orientation::Horizontal);
    box.Spacing(8);
    PasswordBox input;
    input.Width(180);
    input.Password(hstring(value));
    input.PlaceholderText(hstring(placeholder));
    input.PasswordRevealMode(PasswordRevealMode::Hidden);
    input.IsEnabled(enabled && loaded_);
    A11y::SetName(input, hstring(label));
    input.PasswordChanged([changed](Inspectable const &sender, RoutedEventArgs const &) {
      auto field = sender.as<PasswordBox>();
      changed(field, std::wstring(field.Password().c_str()));
    });
    box.Children().Append(input);
    Button reveal;
    reveal.Content(box_value(L"显示"));
    reveal.Height(32);
    reveal.IsEnabled(enabled && loaded_);
    A11y::SetName(reveal, hstring(L"显示" + label));
    ToolTipService::SetToolTip(reveal, box_value(L"显示"));
    reveal.Click([weak_input = make_weak(input), label,
                  revealed = std::make_shared<bool>(false)](Inspectable const &sender,
                                                            RoutedEventArgs const &) {
      auto field = weak_input.get();
      if (!field)
        return;
      *revealed = !*revealed;
      field.PasswordRevealMode(*revealed ? PasswordRevealMode::Visible
                                         : PasswordRevealMode::Hidden);
      const auto action = *revealed ? std::wstring(L"隐藏") : std::wstring(L"显示");
      auto button = sender.as<Button>();
      button.Content(box_value(hstring(action)));
      A11y::SetName(button, hstring(action + label));
      ToolTipService::SetToolTip(button, box_value(hstring(action)));
    });
    box.Children().Append(reveal);
    return box;
  }

  // 写一个已经校验过的凭据：内容变了才写，停下后保存。
  void write_credential(std::wstring_view key, std::wstring const &value) {
    if (document_.String(key, L"") == value)
      return;
    document_.SetString(key, value);
    schedule_save();
  }

  // 「测试配置」按钮和结果（共享页的 `CredentialTestSection`），放在一张卡片里。config 给出输入框里的当前配置，disabled 说明按钮此刻能不能按。结果只在配置与测试时相同时显示，改了任何一项就收起；测试进行中按钮显示「测试中…」。
  void add_credential_test(StackPanel const &group, std::wstring const &service,
                           std::wstring const &label, std::function<JsonObject()> config,
                           std::function<bool()> disabled) {
    Button button;
    button.Content(box_value(L"测试配置"));
    button.Height(32);
    button.Padding(Thickness{16, 0, 16, 0});
    A11y::SetName(button, hstring(label));
    button.Click([this, service, config](Inspectable const &, RoutedEventArgs const &) {
      run_credential_test(service, config());
    });
    auto message = make_text(L"", 13, palette_.sub);
    message.VerticalAlignment(VerticalAlignment::Center);
    A11y::SetLiveSetting(message, Automation::Peers::AutomationLiveSetting::Polite);
    StackPanel content;
    content.Orientation(Orientation::Horizontal);
    content.Spacing(12);
    content.Padding(Thickness{25, 12, 25, 12});
    content.Children().Append(button);
    content.Children().Append(message);
    auto card = make_card();
    card.Child(content);
    group.Children().Append(card);
    if (indexing_)
      return;
    const auto danger = danger_color();
    const auto quiet = palette_.sub;
    auto refresh = [this, service, config, disabled, danger, quiet,
                    weak_button = make_weak(button), weak_message = make_weak(message)] {
      auto shown_button = weak_button.get();
      auto shown_message = weak_message.get();
      if (!shown_button || !shown_message)
        return;
      const auto found = credential_tests_.find(service);
      const bool visible = found != credential_tests_.end() &&
                           found->second.signature == std::wstring_view(config().Stringify());
      const bool busy = visible && found->second.busy;
      shown_button.Content(box_value(busy ? L"测试中…" : L"测试配置"));
      shown_button.IsEnabled(loaded_ && !disabled() && !busy);
      const std::wstring text = visible ? found->second.message : std::wstring();
      shown_message.Foreground(brush(visible && !found->second.ok ? danger : quiet));
      shown_message.Text(hstring(text));
      shown_message.Visibility(text.empty() ? Visibility::Collapsed : Visibility::Visible);
    };
    refresh();
    credential_test_views_[service] = std::move(refresh);
  }

  void refresh_credential_test(std::wstring const &service) {
    if (const auto found = credential_test_views_.find(service);
        found != credential_test_views_.end())
      found->second();
  }

  // 共享页的 runCredentialTest：同一种服务一次只测一个；在 UI 线程之外调用 msime_client_test_api_credential（会联网，最长约 15 秒），结果记在这次测试的配置名下。调用本身失败时显示共享页的同一句。
  void run_credential_test(std::wstring const &service, JsonObject const &config) {
    auto &state = credential_tests_[service];
    if (state.busy)
      return;
    const std::wstring signature(config.Stringify().c_str());
    state = {signature, true, false, L""};
    refresh_credential_test(service);
    JsonObject request;
    request.SetNamedValue(L"service", JsonValue::CreateStringValue(hstring(service)));
    request.SetNamedValue(L"config", JsonObject::Parse(config.Stringify()));
    run_host(
        [body = utf8(request.Stringify())] {
          return take_response(msime_client_test_api_credential(
              reinterpret_cast<const uint8_t *>(body.data()), body.size()));
        },
        [this, service, signature](Response const &response) {
          CredentialTest result{signature, false, false,
                                L"无法连接 provider，请确认服务已启动。"};
          if (response.ok) {
            try {
              const auto value = JsonObject::Parse(text(response.text)).GetNamedObject(L"value");
              result.ok = value.GetNamedBoolean(L"ok", false);
              result.message = value.GetNamedString(L"message", L"").c_str();
            } catch (...) {
            }
          }
          credential_tests_[service] = std::move(result);
          refresh_credential_test(service);
        });
  }

  // 自定义接口地址的提示和偏好库是否接受，见 msime_client_translation_endpoint_issue。超过 2048 字节的地址不必交给它：提示就是「接口地址过长。」，偏好库也不收。
  static std::pair<std::wstring, bool> translation_endpoint_check(std::wstring const &endpoint) {
    const auto bytes = utf8(hstring(endpoint));
    if (bytes.size() > 2048)
      return {L"接口地址过长。", false};
    const auto response = take_response(msime_client_translation_endpoint_issue(
        reinterpret_cast<const uint8_t *>(bytes.data()), bytes.size()));
    try {
      const auto value = JsonObject::Parse(text(response.text)).GetNamedObject(L"value");
      return {std::wstring(value.GetNamedString(L"issue", L"").c_str()),
              value.GetNamedBoolean(L"accepted", false)};
    } catch (...) {
      return {L"", false};
    }
  }

  // 腾讯云机器翻译（`tencent-translation-section.tsx`，Windows 一支）：SecretId、SecretKey，「更多选项」里的地域，「测试配置」，然后是凭据的提示或「未填写腾讯云凭据」的警告，都只在候选词翻译打开时显示。
  void build_tencent_translation_group(StackPanel const &page, bool available) {
    auto group = add_group(page, L"腾讯云机器翻译");
    group.Children().Append(group_note(L"需要填入你自己的腾讯云 API 凭据。"));
    struct Fields {
      std::wstring id, key, region;
    };
    auto fields = std::make_shared<Fields>(Fields{
        document_.String(L"tencent_tmt.secret_id", L""),
        document_.String(L"tencent_tmt.secret_key", L""),
        document_.String(L"tencent_tmt.region", L"ap-guangzhou")});
    auto warning = group_warning();
    auto refresh = [this, fields, available, weak_warning = make_weak(warning)] {
      if (auto shown = weak_warning.get()) {
        const auto issue = nav::tencent_credential_issue(fields->id, fields->key, fields->region);
        std::wstring message;
        if (available && !issue.empty())
          message = issue;
        else if (available && nav::tencent_missing_credentials_warning(
                                  document_.Boolean(L"custom_translation.enabled", false),
                                  fields->id, fields->key))
          // 与共享页 offlineFallback 的 dictionary 一支相同：缺凭据时 host-api 不把腾讯云放进查询，离线词库照样出释义，所以只说不会请求在线翻译，不说没有任何结果。
          message = L"未填写腾讯云凭据，填好之前不会请求在线翻译。请填入 SecretId 与 "
                    L"SecretKey，或在上面的翻译服务中改选其他服务。";
        show_warning(shown, message);
      }
      refresh_credential_test(L"translation.tencent");
    };
    add_row(group, 0xE8D7, L"SecretId", L"",
            credential_input(L"腾讯云 SecretId", fields->id, false, L"AKIDxxxxxxxxxxxxxxxx",
                             available, [this, fields, refresh](std::wstring const &value) {
                               fields->id = value;
                               const bool saved = nav::tencent_secret_id_saveable(value);
                               if (saved)
                                 write_credential(L"tencent_tmt.secret_id", value);
                               refresh();
                               return saved;
                             }));
    add_row(group, 0xE72E, L"SecretKey", L"",
            credential_input(L"腾讯云 SecretKey", fields->key, true, L"", available,
                             [this, fields, refresh](std::wstring const &value) {
                               fields->key = value;
                               const bool saved = nav::tencent_secret_key_saveable(value);
                               if (saved)
                                 write_credential(L"tencent_tmt.secret_key", value);
                               refresh();
                               return saved;
                             }));
    // 地域收进「更多选项」：请求一律发往 tmt.tencentcloudapi.com，地域只写进 X-TC-Region，留空按广州处理（crates/client-core/src/translation.rs），平时用不着改；需要境外数据驻留的人仍能填。与共享页 MoreOptions 的 defaultOpen 相同，第一次画出时已存的地域不合规就展开，让错误旁边就是要改的那一格。
    if (!indexing_ && !tencent_region_open_)
      tencent_region_open_ = !nav::tencent_credential_issue(L"", L"", fields->region).empty();
    Expander more;
    more.Header(box_value(L"更多选项"));
    more.HorizontalAlignment(HorizontalAlignment::Stretch);
    more.HorizontalContentAlignment(HorizontalAlignment::Stretch);
    more.IsExpanded(tencent_region_open_.value_or(false) || focus_row_ == L"地域");
    more.Expanding([this](Expander const &, ExpanderExpandingEventArgs const &) {
      tencent_region_open_ = true;
    });
    more.Collapsed([this](Expander const &, ExpanderCollapsedEventArgs const &) {
      tencent_region_open_ = false;
    });
    StackPanel rows;
    rows.Spacing(4);
    add_row(rows, 0xE774, L"地域", L"",
            credential_input(L"腾讯云地域", fields->region, false, L"ap-guangzhou", available,
                             [this, fields, refresh](std::wstring const &value) {
                               fields->region = value;
                               const bool saved = nav::tencent_region_saveable(value);
                               if (saved)
                                 write_credential(L"tencent_tmt.region", value);
                               refresh();
                               return saved;
                             }));
    more.Content(rows);
    group.Children().Append(more);
    add_credential_test(
        group, L"translation.tencent", L"测试腾讯云翻译配置",
        [fields] {
          JsonObject config;
          config.SetNamedValue(L"secret_id", JsonValue::CreateStringValue(hstring(fields->id)));
          config.SetNamedValue(L"secret_key", JsonValue::CreateStringValue(hstring(fields->key)));
          config.SetNamedValue(L"region", JsonValue::CreateStringValue(hstring(fields->region)));
          return config;
        },
        [fields, available] {
          return nav::issue_credential_test_disabled(
              available, nav::tencent_credential_issue(fields->id, fields->key, fields->region));
        });
    group.Children().Append(warning);
    if (!indexing_)
      refresh();
  }

  // 小牛翻译（`niutrans-section.tsx`）：App ID、API Key 和「测试配置」，两项都填了才能测试。
  void build_niutrans_group(StackPanel const &page, bool available) {
    auto group = add_group(page, L"小牛翻译");
    group.Children().Append(group_note(L"使用 App ID 和 API Key 为候选词提供逐条翻译。"));
    struct Fields {
      std::wstring app_id, apikey;
    };
    auto fields = std::make_shared<Fields>(
        Fields{document_.String(L"niutrans.app_id", L""), document_.String(L"niutrans.apikey", L"")});
    auto credential = [this, fields](std::wstring Fields::*field, std::wstring key) {
      return [this, fields, field, key](std::wstring const &value) {
        (*fields).*field = value;
        const bool saved = nav::niutrans_credential_saveable(value);
        if (saved)
          write_credential(key, value);
        refresh_credential_test(L"translation.niutrans");
        return saved;
      };
    };
    add_row(group, 0xE8D7, L"App ID", L"",
            credential_input(L"NiuTrans App ID", fields->app_id, false, L"", available,
                             credential(&Fields::app_id, L"niutrans.app_id")));
    add_row(group, 0xE72E, L"API Key", L"",
            credential_input(L"NiuTrans API Key", fields->apikey, true, L"", available,
                             credential(&Fields::apikey, L"niutrans.apikey")));
    add_credential_test(
        group, L"translation.niutrans", L"测试 NiuTrans 配置",
        [fields] {
          JsonObject config;
          config.SetNamedValue(L"app_id", JsonValue::CreateStringValue(hstring(fields->app_id)));
          config.SetNamedValue(L"apikey", JsonValue::CreateStringValue(hstring(fields->apikey)));
          return config;
        },
        [fields, available] {
          return nav::niutrans_credential_test_disabled(available, fields->app_id, fields->apikey);
        });
  }

  // 自定义服务（`custom-translation-section.tsx`）：翻译 Endpoint 和它下面的提示、API Key、「测试配置」。地址有问题时不能测试。
  void build_custom_translation_group(StackPanel const &page, bool available) {
    auto group = add_group(page, L"自定义服务");
    group.Children().Append(group_note(L"使用自建的兼容 DeepLX 的 HTTPS 服务。"));
    struct Fields {
      std::wstring endpoint, api_key, issue;
    };
    auto fields = std::make_shared<Fields>(Fields{
        document_.String(L"custom_translation.endpoint", L""),
        document_.String(L"custom_translation.api_key", L""), L""});
    fields->issue = translation_endpoint_check(fields->endpoint).first;
    auto warning = group_warning();
    add_row(group, 0xE71B, L"翻译 Endpoint", L"",
            credential_input(L"自定义翻译 Endpoint", fields->endpoint, false,
                             L"https://example.com/translate", available,
                             [this, fields, available,
                              weak_warning = make_weak(warning)](std::wstring const &value) {
                               fields->endpoint = value;
                               const auto [issue, saved] = translation_endpoint_check(value);
                               fields->issue = issue;
                               if (saved)
                                 write_credential(L"custom_translation.endpoint", value);
                               if (auto shown = weak_warning.get())
                                 show_warning(shown, available ? issue : std::wstring());
                               refresh_credential_test(L"translation.custom");
                               return saved;
                             }));
    group.Children().Append(warning);
    if (!indexing_)
      show_warning(warning, available ? fields->issue : std::wstring());
    add_row(group, 0xE72E, L"API Key", L"",
            credential_input(L"自定义翻译 API Key", fields->api_key, true, L"", available,
                             [this, fields](std::wstring const &value) {
                               fields->api_key = value;
                               const bool saved = nav::custom_translation_api_key_saveable(value);
                               if (saved)
                                 write_credential(L"custom_translation.api_key", value);
                               refresh_credential_test(L"translation.custom");
                               return saved;
                             }));
    add_credential_test(
        group, L"translation.custom", L"测试自定义翻译配置",
        [fields] {
          JsonObject config;
          config.SetNamedValue(L"endpoint", JsonValue::CreateStringValue(hstring(fields->endpoint)));
          config.SetNamedValue(L"api_key", JsonValue::CreateStringValue(hstring(fields->api_key)));
          return config;
        },
        [fields, available] { return nav::issue_credential_test_disabled(available, fields->issue); });
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
    checks.reserve(all_rules.size());
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
    note.Margin(Thickness{0, 0, 0, 16});
    below.Children().Append(note);
    add_row(group, 0xE8D2, L"模糊音", L"全拼、九键与双拼均支持；更改会在当前输入结束后生效",
            toggle, below);
  }

  // ---- 快捷键 ----

  void build_shortcuts_page(StackPanel const &page) {
    auto keys = add_group(page, L"输入模式切换");
    // 偏好里仍是三个独立的布尔值，这里和共用设置页一样只让选一个：同时开着多个时按 Shift > 单击 Ctrl > Ctrl+Alt+Space 显示第一个，选中一项时一次写全三个，「不使用」三个都关。
    const std::array<std::pair<const wchar_t *, const wchar_t *>, 3> switches{{
        {L"shift", L"keybindings.switch_language_shift"},
        {L"ctrl", L"keybindings.switch_language_ctrl"},
        {L"ctrl_alt_space", L"keybindings.switch_language_ctrl_alt_space"},
    }};
    std::wstring language_switch = L"none";
    for (const auto &[choice, key] : switches) {
      if (document_.Boolean(key, false)) {
        language_switch = choice;
        break;
      }
    }
    add_row(keys, 0xE8AB, L"切换中英文", L"选择用于切换中英文的按键",
            select_control(L"切换中英文",
                           {{L"shift", L"Shift"}, {L"ctrl", L"单击 Ctrl"},
                            {L"ctrl_alt_space", L"Ctrl+Alt+Space"}, {L"none", L"不使用"}},
                           language_switch,
                           [this, switches](std::wstring const &value) {
                             change([&](PreferencesDocument &doc) {
                               for (const auto &[choice, key] : switches)
                                 doc.SetBoolean(key, value == choice);
                             }, false);
                           }));
    // 和上面的切换键互不排斥：Caps Lock 是另加的一个键。
    bool_row(keys, 0xE8AB, L"单按 Caps Lock 切换中英文",
             L"轻点（300 毫秒内松开）切换中英文，不打开大写锁定；按住更久照常打开大写锁定",
             L"keybindings.switch_language_caps_lock", false);
    bool_row(keys, 0xE8C1, L"Ctrl+Shift+F 切换繁体输出", L"在简体与繁体输出之间切换",
             L"keybindings.toggle_character_set_ctrl_shift_f", true);
    // 只有同时提供日语和中文方案的版本（full）有可切的两头；其他版本不显示，TIP 也不吃这个组合键。
    const bool offers_chinese =
        edition_offers_scheme(L"quanpin") || edition_offers_scheme(L"shuangpin") ||
        edition_offers_scheme(L"wubi") || edition_offers_scheme(L"cantonese") ||
        edition_offers_scheme(L"zhuyin") || edition_offers_scheme(L"stroke");
    if (offers_chinese && edition_offers_scheme(L"japanese"))
      bool_row(keys, 0xE8AB, L"Ctrl+Shift+J 在中文和日语之间切换",
               L"中文方案下按它切到日语，在日语里再按回到上次的中文方案；英文模式下交给应用。关着时这个组合键交给应用；浏览器里它是开发者工具或下载页的快捷键，打开后在浏览器里会被输入法占用。语言栏图标会变成「日」，系统的输入指示器仍显示中文。",
               L"keybindings.toggle_japanese_ctrl_shift_j", false);
    bool_row(keys, 0xE8D3, L"Alt+Shift+H 切换全半角",
             L"中文模式下切换全角与半角。关掉后这个组合键交给应用处理；工具栏的全半角开关和 Ctrl+Shift+Space 不受影响。",
             L"keybindings.toggle_fullwidth_option_shift_h", true);

    build_selection_keys(page);

    auto reset = add_group(page, L"");
    add_row(reset, 0xE72C, L"恢复默认快捷键",
            L"切换中英文、Caps Lock、Ctrl+Shift+F、Ctrl+Shift+J 和 Alt+Shift+H 恢复为默认设置",
            button_control(L"恢复默认",
                           [this] {
                             change([](PreferencesDocument &doc) {
                               doc.Restore(L"keybindings");
                             }, true);
                           },
                           loaded_));
  }

  // ---- 词库 ----

  // ---- 词库 ----

  struct LexiconEntry {
    std::wstring kind, key, value;
    int64_t weight = 0;
    bool bundled = false;
  };

  struct Wordbook {
    std::wstring id, name;
    int64_t total = 0;
    bool builtin = false;
    bool pack = false;
  };

  struct VocabularyCard {
    std::wstring word, phonetic, meaning;
  };

  struct VocabularyStatus {
    std::vector<Wordbook> books;
    std::wstring wordbook;
    int64_t new_per_day = 0, session_limit = 0;
    int64_t due = 0, answered = 0, remaining = 0;
    std::vector<VocabularyCard> queue;
  };

  static constexpr uint32_t lexicon_page_size = 100;

  // Runs `call` off the UI thread and gives its response to `done` back on it, unless the window has closed.
  fire_and_forget run_host(std::function<Response()> call,
                           std::function<void(Response const &)> done) {
    auto weak = get_weak();
    auto queue = DispatcherQueue();
    co_await resume_background();
    auto response = call();
    queue.TryEnqueue([weak, done = std::move(done), response = std::move(response)] {
      if (auto self = weak.get(); self && !self->closed_)
        done(response);
    });
  }

  void redraw_lexicon() {
    if (!closed_ && current_page_ == "lexicon")
      render_page(true);
  }

  static JsonObject entry_json(LexiconEntry const &entry) {
    JsonObject object;
    object.SetNamedValue(L"kind", JsonValue::CreateStringValue(entry.kind));
    object.SetNamedValue(L"key", JsonValue::CreateStringValue(entry.key));
    object.SetNamedValue(L"value", JsonValue::CreateStringValue(entry.value));
    object.SetNamedValue(L"weight",
                         JsonValue::CreateNumberValue(static_cast<double>(entry.weight)));
    object.SetNamedValue(L"source",
                         JsonValue::CreateStringValue(entry.bundled ? L"bundled" : L"user"));
    return object;
  }

  // Sends one dictionary action; `done` gets the response on the UI thread. Without host options there is nothing to send to.
  void lexicon_request(JsonObject const &action, std::function<void(Response const &)> done) {
    const auto options = runtime_options_document();
    if (!options) {
      lexicon_message_ = L"输入法尚未完成初始化，完成设置向导后即可管理词库。";
      lexicon_error_ = true;
      redraw_lexicon();
      return;
    }
    lexicon_busy_ = true;
    run_host(
        [options = *options, body = utf8(action.Stringify())] {
          return settings_dictionary_call(options, body);
        },
        [this, done = std::move(done)](Response const &response) {
          lexicon_busy_ = false;
          done(response);
          redraw_lexicon();
        });
    redraw_lexicon();
  }

  // 查询: the page of `offset` of the chosen dictionary, filtered by the code prefix.
  void lexicon_list(uint32_t offset) {
    if (lexicon_busy_)
      return;
    JsonObject action;
    action.SetNamedValue(L"operation", JsonValue::CreateStringValue(L"list"));
    action.SetNamedValue(L"offset", JsonValue::CreateNumberValue(offset));
    action.SetNamedValue(L"limit", JsonValue::CreateNumberValue(lexicon_page_size));
    action.SetNamedValue(L"kind", JsonValue::CreateStringValue(lexicon_kind_));
    std::wstring query = lexicon_query_;
    query.erase(0, query.find_first_not_of(L" \t"));
    query.erase(query.find_last_not_of(L" \t") + 1);
    action.SetNamedValue(L"query", JsonValue::CreateStringValue(query));
    lexicon_message_ = L"查询中…";
    lexicon_error_ = false;
    const auto kind = lexicon_kind_;
    lexicon_request(action, [this, offset, kind](Response const &response) {
      // The dictionary was switched while this page was on its way.
      if (kind != lexicon_kind_)
        return;
      if (!response.ok) {
        lexicon_message_ = dictionary_error_text(response_error(response), kind,
                                                 L"无法读取词库。");
        lexicon_error_ = true;
        return;
      }
      try {
        const auto value = JsonObject::Parse(text(response.text)).GetNamedObject(L"value");
        lexicon_entries_.clear();
        for (auto const &item : value.GetNamedArray(L"entries")) {
          const auto object = item.GetObject();
          LexiconEntry entry{named_string(object, L"kind"), named_string(object, L"key"),
                             named_string(object, L"value"), named_count(object, L"weight"),
                             named_string(object, L"source") == L"bundled"};
          if (entry.kind == kind)
            lexicon_entries_.push_back(std::move(entry));
        }
        lexicon_has_more_ = value.GetNamedBoolean(L"has_more", false);
        lexicon_offset_ = offset;
        lexicon_listed_ = true;
        lexicon_message_.clear();
        lexicon_error_ = false;
      } catch (...) {
        lexicon_message_ = L"无法读取词库。";
        lexicon_error_ = true;
      }
    });
  }

  // 新增词条 (no `previous`) or 编辑 / 调权重: a bundled word keeps its code and text, so only its weight can change.
  fire_and_forget edit_lexicon_entry(std::optional<LexiconEntry> previous) {
    if (lexicon_busy_)
      co_return;
    auto strong = get_strong();
    const bool bundled = previous && previous->bundled;
    const auto kind = previous ? previous->kind : lexicon_kind_;
    StackPanel form;
    form.Spacing(12);
    form.MinWidth(360);
    TextBox key;
    key.Header(box_value(hstring(L"编码")));
    key.PlaceholderText(hstring(lexicon_key_hint(kind)));
    key.Text(hstring(previous ? previous->key : L""));
    key.IsReadOnly(bundled);
    TextBox value;
    value.Header(box_value(hstring(kind == L"quick_phrase" ? L"短语" : L"词条")));
    value.Text(hstring(previous ? previous->value : L""));
    value.IsReadOnly(bundled);
    NumberBox weight;
    weight.Header(box_value(hstring(L"权重")));
    weight.Minimum(1);
    weight.Maximum(100000000);
    weight.SpinButtonPlacementMode(NumberBoxSpinButtonPlacementMode::Compact);
    weight.Value(previous ? static_cast<double>(previous->weight) : 10.0);
    form.Children().Append(key);
    form.Children().Append(value);
    form.Children().Append(weight);
    ContentDialog dialog;
    dialog.Title(box_value(hstring(previous ? (bundled ? L"调权重" : L"编辑词条") : L"新增词条")));
    dialog.Content(form);
    dialog.PrimaryButtonText(L"保存");
    dialog.CloseButtonText(L"取消");
    dialog.DefaultButton(ContentDialogButton::Primary);
    dialog.XamlRoot(root_.XamlRoot());
    dialog.RequestedTheme(palette_.dark ? ElementTheme::Dark : ElementTheme::Light);
    if (!co_await show_dialog(dialog))
      co_return;
    std::wstring new_key(key.Text().c_str());
    std::wstring new_value(value.Text().c_str());
    if (new_key.empty() || new_value.empty()) {
      lexicon_message_ = L"编码和短语不能为空。";
      lexicon_error_ = true;
      redraw_lexicon();
      co_return;
    }
    const double chosen = weight.Value();
    LexiconEntry replacement{kind, new_key, new_value,
                             std::isnan(chosen) ? 10 : static_cast<int64_t>(std::llround(chosen)),
                             bundled};
    if (bundled) {
      replacement.key = previous->key;
      replacement.value = previous->value;
    }
    JsonObject action;
    action.SetNamedValue(L"operation", JsonValue::CreateStringValue(L"edit"));
    action.SetNamedValue(L"previous", previous ? JsonValue::Parse(entry_json(*previous).Stringify())
                                               : JsonValue::CreateNullValue());
    action.SetNamedValue(L"replacement", JsonValue::Parse(entry_json(replacement).Stringify()));
    action.SetNamedValue(L"request_id",
                         JsonValue::CreateStringValue(request_id(previous ? L"ui-edit-" : L"ui-add-")));
    const uint32_t reload = previous ? lexicon_offset_ : 0;
    lexicon_message_.clear();
    lexicon_request(action, [this, kind, reload](Response const &response) {
      if (!response.ok) {
        lexicon_message_ = dictionary_error_text(response_error(response), kind,
                                                 L"保存失败，请稍后重试。");
        lexicon_error_ = true;
        return;
      }
      lexicon_list(reload);
    });
  }

  fire_and_forget delete_lexicon_entry(LexiconEntry entry) {
    if (lexicon_busy_)
      co_return;
    auto strong = get_strong();
    if (!co_await confirm(L"删除词条",
                          L"“" + entry.value + L"”（" + entry.key + L"）将被删除，此操作无法撤销。",
                          L"删除"))
      co_return;
    JsonObject action;
    action.SetNamedValue(L"operation", JsonValue::CreateStringValue(L"edit"));
    action.SetNamedValue(L"previous", JsonValue::Parse(entry_json(entry).Stringify()));
    action.SetNamedValue(L"replacement", JsonValue::CreateNullValue());
    action.SetNamedValue(L"request_id", JsonValue::CreateStringValue(request_id(L"ui-remove-")));
    // The last word of a later page leaves that page empty, so step back to the one before it.
    const uint32_t reload = lexicon_entries_.size() == 1 && lexicon_offset_ >= lexicon_page_size
                                ? lexicon_offset_ - lexicon_page_size
                                : lexicon_offset_;
    const auto kind = entry.kind;
    lexicon_message_.clear();
    lexicon_request(action, [this, kind, reload](Response const &response) {
      if (!response.ok) {
        lexicon_message_ = dictionary_error_text(response_error(response), kind,
                                                 L"删除失败，请稍后重试。");
        lexicon_error_ = true;
        return;
      }
      lexicon_list(reload);
    });
  }

  fire_and_forget reset_learned_data() {
    if (lexicon_busy_)
      co_return;
    auto strong = get_strong();
    if (!co_await confirm(L"清除学习数据",
                          L"候选词频、用户词库（包括自己新增和修改的词条）和拼音学习记录将永久删除，此操作无法撤销。输入方案等设置不会改变。",
                          L"清除"))
      co_return;
    JsonObject action;
    action.SetNamedValue(L"operation", JsonValue::CreateStringValue(L"reset"));
    lexicon_request(action, [this](Response const &response) {
      if (!response.ok) {
        show_notice(dictionary_error_text(response_error(response), L"",
                                          L"清除学习数据失败，请稍后重试。"));
        return;
      }
      lexicon_entries_.clear();
      lexicon_listed_ = false;
      lexicon_offset_ = 0;
      lexicon_message_.clear();
      show_notice(L"已清除所有学习数据；输入方案和设置保持不变。", InfoBarSeverity::Success);
    });
  }

  // 导入词库文件的文件对话框：先列所选格式的文件，再列所有文件（改过扩展名的细胞词库按文件头认）。取消时为空。
  std::optional<std::filesystem::path> pick_lexicon_file(std::wstring const &format) {
    const auto *chosen = li::find_format(format);
    if (!chosen)
      return std::nullopt;
    com_ptr<IFileOpenDialog> dialog;
    if (FAILED(CoCreateInstance(CLSID_FileOpenDialog, nullptr, CLSCTX_INPROC_SERVER,
                                IID_PPV_ARGS(dialog.put()))))
      return std::nullopt;
    const std::wstring name(chosen->filter_name);
    const std::wstring spec(chosen->filter);
    const COMDLG_FILTERSPEC filters[] = {{name.c_str(), spec.c_str()}, {L"所有文件", L"*.*"}};
    dialog->SetFileTypes(2, filters);
    dialog->SetTitle(L"导入词库文件");
    FILEOPENDIALOGOPTIONS options = 0;
    if (SUCCEEDED(dialog->GetOptions(&options)))
      dialog->SetOptions(options | FOS_FORCEFILESYSTEM | FOS_FILEMUSTEXIST);
    std::optional<std::filesystem::path> picked;
    com_ptr<IShellItem> item;
    PWSTR raw = nullptr;
    if (dialog->Show(GetActiveWindow()) == S_OK && SUCCEEDED(dialog->GetResult(item.put())) &&
        SUCCEEDED(item->GetDisplayName(SIGDN_FILESYSPATH, &raw)) && raw)
      picked = std::filesystem::path(raw);
    CoTaskMemFree(raw);
    return picked;
  }

  // 导入词库文件：选文件，在工作线程上读、解码并导入，结果写在页面顶上的通知里；导入成功且列表开着时重读当前页。
  void import_lexicon_file() {
    if (lexicon_busy_)
      return;
    const auto options = runtime_options_document();
    if (!options) {
      lexicon_message_ = L"输入法尚未完成初始化，完成设置向导后即可管理词库。";
      lexicon_error_ = true;
      redraw_lexicon();
      return;
    }
    const auto kind = lexicon_kind_;
    const auto chosen = std::wstring(li::usable_format(kind, lexicon_import_format_));
    const auto picked = pick_lexicon_file(chosen);
    if (!picked)
      return;
    const auto format = std::wstring(li::format_for_file(picked->filename().wstring(), chosen));
    if (format != chosen && (format != L"scel" || kind == L"pinyin"))
      lexicon_import_format_ = format;
    lexicon_busy_ = true;
    run_worker<LexiconImportOutcome>(
        [options = *options, kind, format, path = *picked] {
          return import_lexicon_on_worker(options, kind, format, path);
        },
        [this, kind](LexiconImportOutcome &outcome) {
          lexicon_busy_ = false;
          show_notice(outcome.message,
                      outcome.ok ? InfoBarSeverity::Success : InfoBarSeverity::Error);
          if (outcome.ok && lexicon_listed_ && kind == lexicon_kind_)
            lexicon_list(lexicon_offset_);
          else
            redraw_lexicon();
        });
    redraw_lexicon();
  }

  struct LexiconImportOutcome {
    bool ok = false;
    std::wstring message;
  };

  // 在工作线程上：读文件，按格式转成 UTF-8 文本（细胞词库经 msime_client_dictionary_scel_text 转成标准文本），再以一次 import 请求交给 msime_client_settings_dictionary。返回给页面的一句话，措辞与共享页的 importPhrases 相同。
  static LexiconImportOutcome import_lexicon_on_worker(std::string const &options,
                                                       std::wstring const &kind,
                                                       std::wstring format,
                                                       std::filesystem::path const &path) {
    LexiconImportOutcome outcome;
    const auto label = lexicon_kind_label(kind);
    std::error_code error;
    const auto size = std::filesystem::file_size(path, error);
    if (error) {
      outcome.message = L"无法读取所选文件，请重新选择。";
      return outcome;
    }
    if (format == L"scel" && size > li::max_scel_file_bytes) {
      outcome.message = li::scel_too_large();
      return outcome;
    }
    if (size > li::max_text_file_bytes) {
      outcome.message = li::text_too_large();
      return outcome;
    }
    std::string bytes(static_cast<size_t>(size), '\0');
    std::ifstream stream(path, std::ios::binary);
    if (!stream.read(bytes.data(), static_cast<std::streamsize>(bytes.size()))) {
      outcome.message = L"无法读取所选文件，请重新选择。";
      return outcome;
    }
    // 改过扩展名的细胞词库按文件头认出来。
    if (li::is_scel_bytes(bytes))
      format = L"scel";
    std::string body;
    std::wstring source;
    if (format == L"scel") {
      if (kind != L"pinyin") {
        outcome.message = std::wstring(li::scel_needs_pinyin);
        return outcome;
      }
      if (bytes.size() > li::max_scel_file_bytes) {
        outcome.message = li::scel_too_large();
        return outcome;
      }
      const auto *first = reinterpret_cast<uint8_t const *>(bytes.data());
      const auto buffer = Windows::Security::Cryptography::CryptographicBuffer::CreateFromByteArray(
          array_view<uint8_t const>(first, first + bytes.size()));
      JsonObject request;
      request.SetNamedValue(L"bytes_base64",
                            JsonValue::CreateStringValue(
                                Windows::Security::Cryptography::CryptographicBuffer::
                                    EncodeToBase64String(buffer)));
      const auto document = utf8(request.Stringify());
      const auto converted = take_response(msime_client_dictionary_scel_text(
          reinterpret_cast<const uint8_t *>(document.data()), document.size()));
      if (!converted.ok) {
        const auto reason = dictionary_error_text(response_error(converted), kind, L"");
        outcome.message = reason.empty() ? L"细胞词库读取失败，请重新选择文件。" : reason;
        return outcome;
      }
      try {
        const auto value = JsonObject::Parse(text(converted.text)).GetNamedObject(L"value");
        body = utf8(value.GetNamedString(L"text"));
        source = li::scel_source(named_string(value, L"name"), named_count(value, L"entries"));
      } catch (...) {
        outcome.message = L"细胞词库读取失败，请重新选择文件。";
        return outcome;
      }
      format = L"standard";
    } else {
      auto decoded = li::decode_text(bytes, gb18030_to_utf8);
      if (!decoded) {
        outcome.message = label + L"导入失败：文件不是 UTF-8、UTF-16 或 GB18030 编码的文本。";
        return outcome;
      }
      body = std::move(*decoded);
    }
    JsonObject action;
    action.SetNamedValue(L"operation", JsonValue::CreateStringValue(L"import"));
    action.SetNamedValue(L"kind", JsonValue::CreateStringValue(hstring(kind)));
    action.SetNamedValue(L"format", JsonValue::CreateStringValue(hstring(format)));
    action.SetNamedValue(L"text", JsonValue::CreateStringValue(text(body)));
    action.SetNamedValue(L"request_id", JsonValue::CreateStringValue(hstring(request_id(L"ui-import-"))));
    const auto response = settings_dictionary_call(options, utf8(action.Stringify()));
    if (!response.ok) {
      // importFailureMessage：认得的码给出原因，其余是通用的一句。
      const auto reason = dictionary_error_text(response_error(response), kind, L"");
      outcome.message = reason.empty() ? label + L"导入失败，请检查文本格式。"
                                       : label + L"导入失败：" + reason;
      return outcome;
    }
    try {
      const auto value = JsonObject::Parse(text(response.text)).GetNamedObject(L"value");
      li::ImportReport report;
      report.applied = named_count(value, L"applied");
      report.failed = named_count(value, L"failed");
      report.truncated = value.GetNamedBoolean(L"truncated", false);
      report.swapped = value.GetNamedBoolean(L"swapped", false);
      if (value.HasKey(L"first_failures") &&
          value.GetNamedValue(L"first_failures").ValueType() == JsonValueType::Array) {
        for (auto const &item : value.GetNamedArray(L"first_failures")) {
          if (item.ValueType() != JsonValueType::Object)
            continue;
          const auto failure = item.GetObject();
          report.first_failures.emplace_back(named_count(failure, L"line"),
                                             named_string(failure, L"issue"));
        }
      }
      outcome.message = li::describe_import(label, report, source);
      outcome.ok = true;
    } catch (...) {
      // 已经导入了，只是读不懂报告。
      outcome.message = label + L"导入完成。";
      outcome.ok = true;
    }
    return outcome;
  }

  // A yes-or-no question before something that cannot be undone.
  Windows::Foundation::IAsyncOperation<bool> confirm(std::wstring title, std::wstring message, std::wstring action) {
    ContentDialog dialog;
    dialog.Title(box_value(hstring(title)));
    dialog.Content(box_value(hstring(message)));
    dialog.PrimaryButtonText(hstring(action));
    dialog.CloseButtonText(L"取消");
    dialog.DefaultButton(ContentDialogButton::Close);
    dialog.XamlRoot(root_.XamlRoot());
    dialog.RequestedTheme(palette_.dark ? ElementTheme::Dark : ElementTheme::Light);
    co_return co_await show_dialog(dialog);
  }

  // True when the user chose the dialog's primary button. WinUI shows one ContentDialog at a time and throws for a second, which inside a fire_and_forget coroutine ends the process; a click that would open another while one is up, such as a second 删除, is taken as a cancel.
  Windows::Foundation::IAsyncOperation<bool> show_dialog(ContentDialog dialog) {
    if (dialog_open_)
      co_return false;
    dialog_open_ = true;
    bool accepted = false;
    try {
      accepted = co_await dialog.ShowAsync() == ContentDialogResult::Primary;
    } catch (...) {
    }
    dialog_open_ = false;
    co_return accepted;
  }

  // ---- 背单词 ----

  // One 背单词 request body for msime_client_vocabulary_review, against the same directories the shared app's review uses: the user's state directory, the installed resources beside MSIME.exe and the plugins under the state directory.
  static std::string vocabulary_body(JsonObject const &action) {
    const auto directory = state_directory();
    JsonObject request;
    request.SetNamedValue(L"directory", JsonValue::CreateStringValue(hstring(directory.wstring())));
    request.SetNamedValue(L"resources",
                          JsonValue::CreateStringValue(hstring(shell_executable().parent_path().wstring())));
    request.SetNamedValue(L"plugins",
                          JsonValue::CreateStringValue(hstring((directory / L"plugins").wstring())));
    request.SetNamedValue(L"day", JsonValue::CreateStringValue(hstring(local_day())));
    request.SetNamedValue(L"action", JsonValue::Parse(action.Stringify()));
    return utf8(request.Stringify());
  }

  static Response vocabulary_call(std::string const &body) {
    return take_response(msime_client_vocabulary_review(
        reinterpret_cast<const uint8_t *>(body.data()), body.size()));
  }

  // One 背单词 action; the answer replaces the status the page shows.
  void vocabulary_request(JsonObject const &action) {
    if (vocabulary_busy_)
      return;
    vocabulary_busy_ = true;
    run_host([body = vocabulary_body(action)] { return vocabulary_call(body); },
             [this](Response const &response) {
               vocabulary_busy_ = false;
               apply_vocabulary_response(response);
               redraw_lexicon();
             });
    redraw_lexicon();
  }

  // 把背单词的回答记成页面的状态；失败时留下原来的状态和一句说明。
  void apply_vocabulary_response(Response const &response) {
    vocabulary_revealed_ = false;
    if (!response.ok) {
      vocabulary_message_ = vocabulary_
                                ? L"无法读取或保存背单词进度，请稍后重试。已有的进度不会被自动清空。"
                                : L"无法读取背单词进度。";
      return;
    }
    try {
      const auto value = JsonObject::Parse(text(response.text)).GetNamedObject(L"value");
      VocabularyStatus status;
      for (auto const &item : value.GetNamedArray(L"wordbooks")) {
        const auto book = item.GetObject();
        status.books.push_back({named_string(book, L"id"), named_string(book, L"name"),
                                named_count(book, L"total"),
                                book.GetNamedBoolean(L"builtin", false),
                                book.HasKey(L"pack") &&
                                    book.GetNamedValue(L"pack").ValueType() != JsonValueType::Null});
      }
      const auto settings = value.GetNamedObject(L"settings");
      status.wordbook = named_string(settings, L"wordbook");
      status.new_per_day = named_count(settings, L"newPerDay");
      status.session_limit = named_count(settings, L"sessionLimit");
      status.due = named_count(value, L"due");
      status.answered = named_count(value, L"answeredToday");
      status.remaining = named_count(value, L"remaining");
      for (auto const &item : value.GetNamedArray(L"queue")) {
        const auto card = item.GetObject();
        status.queue.push_back({named_string(card, L"word"), named_string(card, L"phonetic"),
                                named_string(card, L"meaning")});
      }
      vocabulary_ = std::move(status);
      vocabulary_message_.clear();
    } catch (...) {
      vocabulary_message_ = L"无法读取背单词进度。";
    }
  }

  static JsonObject vocabulary_action(wchar_t const *operation) {
    JsonObject action;
    action.SetNamedValue(L"operation", JsonValue::CreateStringValue(operation));
    return action;
  }

  void set_vocabulary_settings(std::wstring const &wordbook, int64_t new_per_day) {
    if (!vocabulary_)
      return;
    auto action = vocabulary_action(L"set_settings");
    action.SetNamedValue(L"wordbook", JsonValue::CreateStringValue(hstring(wordbook)));
    action.SetNamedValue(L"new_per_day",
                         JsonValue::CreateNumberValue(static_cast<double>(new_per_day)));
    action.SetNamedValue(L"session_limit",
                         JsonValue::CreateNumberValue(static_cast<double>(vocabulary_->session_limit)));
    vocabulary_request(action);
  }

  void answer_vocabulary_card(bool known) {
    if (!vocabulary_ || vocabulary_->queue.empty())
      return;
    auto action = vocabulary_action(L"answer");
    action.SetNamedValue(L"word", JsonValue::CreateStringValue(hstring(vocabulary_->queue.front().word)));
    action.SetNamedValue(L"known", JsonValue::CreateBooleanValue(known));
    vocabulary_request(action);
  }

  fire_and_forget remove_vocabulary_book(Wordbook book) {
    auto strong = get_strong();
    if (!co_await confirm(L"删除词表", L"删除「" + book.name + L"」及其复习进度？此操作无法撤销。",
                          L"删除"))
      co_return;
    auto action = vocabulary_action(L"remove");
    action.SetNamedValue(L"wordbook", JsonValue::CreateStringValue(hstring(book.id)));
    vocabulary_request(action);
  }

  fire_and_forget reset_vocabulary_progress() {
    auto strong = get_strong();
    if (!co_await confirm(L"清空进度",
                          L"清空全部词书的复习进度？已经学过的词会从头开始，此操作无法撤销。",
                          L"清空"))
      co_return;
    vocabulary_request(vocabulary_action(L"reset"));
  }

  // 原生设置只显示共享状态，不复制版本判断、文件校验和自动检查的节流。
  void dictionary_update_request(std::wstring const &command, std::optional<bool> enabled = std::nullopt) {
    if (dictionary_update_busy_ || closed_) return;
    const auto options = runtime_options_document();
    if (!options) return;
    JsonObject action;
    action.SetNamedValue(L"operation", JsonValue::CreateStringValue(L"updates"));
    action.SetNamedValue(L"command", JsonValue::CreateStringValue(command));
    if (enabled) action.SetNamedValue(L"enabled", JsonValue::CreateBooleanValue(*enabled));
    dictionary_update_busy_ = true;
    run_host([options = *options, body = utf8(action.Stringify())] { return settings_dictionary_call(options, body); },
      [this](Response const &response) {
        dictionary_update_busy_ = false;
        if (!response.ok) { dictionary_update_message_ = L"词库更新暂不可用，请稍后重试"; redraw_lexicon(); return; }
        try {
          const auto value = JsonObject::Parse(text(response.text)).GetNamedObject(L"value");
          // 这里不展示字节进度；下载期间轮询不重建整页，避免打断词条编辑表单。
          const bool same_display = dictionary_update_status_
            && named_string(*dictionary_update_status_, L"phase") == named_string(value, L"phase")
            && named_string(*dictionary_update_status_, L"version") == named_string(value, L"version")
            && named_string(*dictionary_update_status_, L"error") == named_string(value, L"error")
            && dictionary_update_status_->GetNamedBoolean(L"enabled", true) == value.GetNamedBoolean(L"enabled", true)
            && dictionary_update_status_->GetNamedBoolean(L"pending_restart", false) == value.GetNamedBoolean(L"pending_restart", false)
            && dictionary_update_status_->GetNamedBoolean(L"can_rollback", false) == value.GetNamedBoolean(L"can_rollback", false);
          if (same_display && dictionary_update_message_.empty()) { dictionary_update_status_ = value; return; }
          dictionary_update_status_ = value;
          dictionary_update_message_.clear();
          redraw_lexicon();
        } catch (...) { dictionary_update_message_ = L"无法读取词库更新状态"; redraw_lexicon(); }
      });
  }

  void build_dictionary_updates(StackPanel const &page) {
    auto group = add_group(page, L"词库更新");
    const bool automatic = !dictionary_update_status_ || dictionary_update_status_->GetNamedBoolean(L"enabled", true);
    const auto phase = dictionary_update_status_ ? named_string(*dictionary_update_status_, L"phase") : L"";
    const bool running = phase == L"checking" || phase == L"downloading";
    add_row(group, 0xE895, L"自动更新词库", L"每天检查兼容版本，个人词条和学习记录保留。",
      toggle_control(L"自动更新词库", automatic,
        [this](bool value) { dictionary_update_request(L"configure", value); }, !dictionary_update_busy_, false));
    std::wstring message = dictionary_update_message_;
    if (dictionary_update_status_ && message.empty()) {
      const auto &status = *dictionary_update_status_;
      const auto status_phase = named_string(status, L"phase");
      if (status_phase == L"checking") message = L"正在检查词库更新…";
      else if (status_phase == L"downloading") message = L"正在下载词库…";
      else if (status_phase == L"failed" || !named_string(status, L"error").empty()) message = L"更新失败，继续使用原词库";
      else if (status.GetNamedBoolean(L"pending_restart", false)) message = L"词库已准备好，下次启动输入法时生效";
      else message = L"词库版本：" + named_string(status, L"version");
    }
    add_row(group, 0xE72C, L"检查词库更新", message,
      button_control(L"检查更新", [this] { dictionary_update_request(L"check"); }, !dictionary_update_busy_ && !running));
    if (dictionary_update_status_ && dictionary_update_status_->GetNamedBoolean(L"can_rollback", false))
      add_row(group, 0xE777, L"恢复上一版词库", L"恢复后关闭自动更新，下次启动生效。",
        button_control(L"恢复", [this] { dictionary_update_request(L"rollback"); }, !dictionary_update_busy_ && !running));
  }

  void build_lexicon_page(StackPanel const &page) {
    build_dictionary_updates(page);
    auto contribution = add_group(page, L"完善公共词库");
    add_row(contribution, 0xE8F2, L"提交缺失词条",
            L"前往官网填写词语和读音，审核后随公共词库更新。个人学习的词不会自动公开。",
            button_control(L"前往投稿", [this] { open_url(L"https://msime.app/words/"); }));
    // 本地词库管理通过 msime_client_settings_dictionary 提供查询、编辑和删除。
    auto manage = add_group(page, L"本地词库管理");
    std::vector<Option> kinds;
    for (auto const &[id, label] : lexicon_kinds)
      kinds.push_back({std::wstring(id), std::wstring(label)});
    add_row(manage, 0xE82D, L"词库",
            L"查询、新增、编辑和删除用户词库。标有「内置」的是随输入法附带的词条，只能调整权重或删除。",
            select_control(L"本地词库类型", std::move(kinds), lexicon_kind_,
                           [this](std::wstring const &kind) {
                             if (kind == lexicon_kind_)
                               return;
                             lexicon_kind_ = kind;
                             lexicon_entries_.clear();
                             lexicon_listed_ = false;
                             lexicon_offset_ = 0;
                             lexicon_list(0);
                           },
                           !lexicon_busy_));

    TextBox query;
    query.Width(180);
    query.PlaceholderText(L"留空查看全部");
    query.Text(hstring(lexicon_query_));
    A11y::SetName(query, L"编码前缀");
    query.TextChanged([this](Inspectable const &sender, TextChangedEventArgs const &) {
      lexicon_query_ = std::wstring(sender.as<TextBox>().Text().c_str());
    });
    query.KeyDown([this](Inspectable const &, Input::KeyRoutedEventArgs const &args) {
      if (args.Key() == Windows::System::VirtualKey::Enter) {
        args.Handled(true);
        lexicon_list(0);
      }
    });
    StackPanel controls;
    controls.Orientation(Orientation::Horizontal);
    controls.Spacing(8);
    controls.Children().Append(query);
    controls.Children().Append(button_control(L"查询", [this] { lexicon_list(0); }, !lexicon_busy_));
    controls.Children().Append(button_control(
        L"新增词条", [this] { edit_lexicon_entry(std::nullopt); }, !lexicon_busy_));

    StackPanel results;
    results.Spacing(4);
    results.Margin(Thickness{0, 0, 0, 16});
    const auto label = lexicon_kind_label(lexicon_kind_);
    std::wstring status = lexicon_message_;
    if (status.empty() && !lexicon_listed_)
      status = L"点击查询后查看" + label + L"词条";
    if (status.empty() && lexicon_entries_.empty())
      status = lexicon_offset_ == 0 ? L"没有找到词条" : L"没有更多结果";
    if (!status.empty()) {
      results.Children().Append(
          make_text(status, 12, lexicon_error_ ? palette_.text : palette_.sub));
    }
    // 列宽由表头和数据行共用，长编码或词条不会把权重与操作挤出卡片。
    auto table_row = [] {
      Grid item;
      item.ColumnSpacing(8);
      for (auto width : {GridLength{88, GridUnitType::Pixel}, GridLength{1, GridUnitType::Star},
                         GridLength{44, GridUnitType::Pixel}, GridLength{148, GridUnitType::Pixel}}) {
        ColumnDefinition column;
        column.Width(width);
        item.ColumnDefinitions().Append(column);
      }
      return item;
    };
    if (!lexicon_entries_.empty()) {
      auto header = table_row();
      header.Padding(Thickness{0, 6, 0, 6});
      int32_t column = 0;
      for (const auto *title : {L"编码", L"词条", L"权重", L"操作"}) {
        auto text = make_text(title, 11, palette_.sub);
        Grid::SetColumn(text, column++);
        header.Children().Append(text);
      }
      results.Children().Append(header);
    }
    for (auto const &entry : lexicon_entries_) {
      auto item = table_row();
      item.MinHeight(36);
      item.Padding(Thickness{0, 4, 0, 4});
      auto code = make_text(entry.key, 12, palette_.sub);
      code.FontFamily(FontFamily(L"Consolas"));
      code.VerticalAlignment(VerticalAlignment::Center);
      item.Children().Append(code);
      StackPanel words;
      words.Spacing(2);
      words.VerticalAlignment(VerticalAlignment::Center);
      words.Children().Append(make_text(entry.value, 13, palette_.text));
      if (entry.bundled)
        words.Children().Append(make_text(L"内置", 11, palette_.faint));
      Grid::SetColumn(words, 1);
      item.Children().Append(words);
      auto weight = make_text(std::to_wstring(entry.weight), 12, palette_.sub);
      weight.VerticalAlignment(VerticalAlignment::Center);
      Grid::SetColumn(weight, 2);
      item.Children().Append(weight);
      StackPanel actions;
      actions.Orientation(Orientation::Horizontal);
      actions.Spacing(4);
      auto edit = button_control(entry.bundled ? L"调权重" : L"编辑",
                                 [this, entry] { edit_lexicon_entry(entry); }, !lexicon_busy_);
      edit.as<Button>().Padding(Thickness{10, 0, 10, 0});
      actions.Children().Append(edit);
      auto remove = button_control(L"删除", [this, entry] { delete_lexicon_entry(entry); },
                                   !lexicon_busy_);
      remove.as<Button>().Padding(Thickness{10, 0, 10, 0});
      actions.Children().Append(remove);
      Grid::SetColumn(actions, 3);
      item.Children().Append(actions);
      Border divider;
      divider.BorderBrush(brush(palette_.divider));
      divider.BorderThickness(Thickness{0, 1, 0, 0});
      divider.Child(item);
      results.Children().Append(divider);
    }
    if (lexicon_listed_ && (lexicon_offset_ > 0 || lexicon_has_more_)) {
      StackPanel pager;
      pager.Orientation(Orientation::Horizontal);
      pager.Spacing(12);
      pager.Margin(Thickness{0, 8, 0, 0});
      pager.Children().Append(button_control(
          L"上一页", [this] { lexicon_list(lexicon_offset_ - lexicon_page_size); },
          !lexicon_busy_ && lexicon_offset_ >= lexicon_page_size));
      const auto first = lexicon_offset_ + 1;
      const auto last = lexicon_offset_ + static_cast<uint32_t>(lexicon_entries_.size());
      auto range = make_text(L"第 " + std::to_wstring(first) + L"–" + std::to_wstring(last) +
                                 (lexicon_has_more_ ? L" 条，后面还有结果" : L" 条，没有更多结果"),
                             12, palette_.sub);
      range.VerticalAlignment(VerticalAlignment::Center);
      pager.Children().Append(range);
      pager.Children().Append(button_control(
          L"下一页", [this] { lexicon_list(lexicon_offset_ + lexicon_page_size); },
          !lexicon_busy_ && lexicon_has_more_));
      results.Children().Append(pager);
    }
    add_row(manage, 0xE721, L"编码前缀", lexicon_key_hint(lexicon_kind_), controls, results);

    // 导入词库文件：共享页「导入与导出」的导入。格式只列这一类词库能用的，选文件时按扩展名再换一次；导入经 msime_client_settings_dictionary，它让 Server 放开会话并分批送进去。
    const auto import_format = std::wstring(li::usable_format(lexicon_kind_, lexicon_import_format_));
    std::vector<Option> import_formats;
    for (auto const &format : li::formats_for(lexicon_kind_))
      import_formats.push_back({std::wstring(format.id), std::wstring(format.label)});
    StackPanel importer;
    importer.Orientation(Orientation::Horizontal);
    importer.Spacing(8);
    importer.Children().Append(select_control(
        L"导入文件格式", std::move(import_formats), import_format,
        [this](std::wstring const &format) { lexicon_import_format_ = format; }, !lexicon_busy_));
    importer.Children().Append(
        button_control(L"选择文件…", [this] { import_lexicon_file(); }, !lexicon_busy_));
    add_row(manage, 0xE8B5, L"导入词库文件",
            lexicon_kind_ == L"pinyin"
                ? L"导入到上面所选类型的词库；支持标准、Windows TSV、Rime、汉字自动注音和搜狗细胞词库（.scel）。"
                : L"导入到上面所选类型的词库；支持标准、Windows TSV 和 Rime。",
            importer);

    // 云词库 is a panel of its own on every desktop, as in the shared app.
    auto cloud = add_group(page, L"云词库");
    shell_row(cloud, 0xE753, L"云词库", L"管理同步到账号的词条和备份", L"打开云词库",
              nav::shell_links::cloud_dictionary);

    build_vocabulary_group(page);

    auto learned = add_group(page, L"学习数据");
    add_row(learned, 0xE74D, L"清除候选词频、用户词库和拼音学习记录",
            L"自己新增和修改的词条也会删除；输入方案与其他设置不会改变。",
            button_control(L"清除全部学习数据", [this] { reset_learned_data(); }, !lexicon_busy_));
  }

  // 背单词: the books, the daily allowance and the cards, reviewed here rather than in a panel of the shared app.
  void build_vocabulary_group(StackPanel const &page) {
    auto group = add_group(page, L"背单词");
    if (!indexing_ && !vocabulary_ && !vocabulary_busy_ && vocabulary_message_.empty())
      vocabulary_request(vocabulary_action(L"load"));
    if (!vocabulary_) {
      add_row(group, 0xE82D, L"背单词",
              vocabulary_busy_ ? L"正在读取…"
                               : (vocabulary_message_.empty() ? L"用输入过的英文单词复习词汇"
                                                              : vocabulary_message_),
              vocabulary_message_.empty()
                  ? FrameworkElement{nullptr}
                  : button_control(L"重试", [this] {
                      vocabulary_message_.clear();
                      vocabulary_request(vocabulary_action(L"load"));
                    }));
      return;
    }
    auto const &status = *vocabulary_;
    add_row(group, 0xE787, L"今日进度",
            L"今日待复习 " + std::to_wstring(status.due) + L" · 已完成 " +
                std::to_wstring(status.answered) + L" · 未开始 " + std::to_wstring(status.remaining),
            nullptr);
    std::vector<Option> books{{L"", L"未选择"}};
    const Wordbook *selected = nullptr;
    for (auto const &book : status.books) {
      books.push_back({book.id, book.name + L"（" + std::to_wstring(book.total) + L" 词）" +
                                    (book.pack ? L" · 插件" : L"")});
      if (book.id == status.wordbook)
        selected = &book;
    }
    add_row(group, 0xE8F1, L"词书", L"复习不会改变打字时的候选或组词。进度保存在本机，不会上传。",
            select_control(L"词书", std::move(books), status.wordbook,
                           [this](std::wstring const &id) {
                             set_vocabulary_settings(id, vocabulary_->new_per_day);
                           },
                           !vocabulary_busy_));
    NumberBox daily;
    daily.Minimum(0);
    daily.Maximum(200);
    daily.Width(120);
    daily.SpinButtonPlacementMode(NumberBoxSpinButtonPlacementMode::Compact);
    daily.Value(static_cast<double>(status.new_per_day));
    daily.IsEnabled(!vocabulary_busy_);
    A11y::SetName(daily, L"每日新词");
    daily.ValueChanged([this](NumberBox const &, NumberBoxValueChangedEventArgs const &args) {
      const double value = args.NewValue();
      if (std::isnan(value) || !vocabulary_ ||
          static_cast<int64_t>(std::llround(value)) == vocabulary_->new_per_day)
        return;
      set_vocabulary_settings(vocabulary_->wordbook, std::clamp<int64_t>(std::llround(value), 0, 200));
    });
    add_row(group, 0xE710, L"每日新词", L"每天新加入复习的单词数，0 到 200。", daily);

    StackPanel card;
    card.Spacing(8);
    card.Margin(Thickness{0, 0, 0, 16});
    if (status.wordbook.empty()) {
      card.Children().Append(make_text(L"先选一本词书。", 13, palette_.sub));
    } else if (status.queue.empty()) {
      card.Children().Append(make_text(L"今天的复习已经完成。", 13, palette_.sub));
    } else {
      auto const &current = status.queue.front();
      auto word = make_text(current.word, 24, palette_.text);
      word.FontWeight(Windows::UI::Text::FontWeight{600});
      word.IsTextSelectionEnabled(true);
      card.Children().Append(word);
      if (!current.phonetic.empty())
        card.Children().Append(make_text(current.phonetic, 13, palette_.sub));
      if (vocabulary_revealed_) {
        auto meaning = make_text(current.meaning, 14, palette_.text);
        meaning.IsTextSelectionEnabled(true);
        card.Children().Append(meaning);
      }
      StackPanel answers;
      answers.Orientation(Orientation::Horizontal);
      answers.Spacing(8);
      if (!vocabulary_revealed_)
        answers.Children().Append(button_control(L"显示释义", [this] {
          vocabulary_revealed_ = true;
          redraw_lexicon();
        }));
      answers.Children().Append(button_control(L"不认识", [this] { answer_vocabulary_card(false); },
                                               !vocabulary_busy_));
      answers.Children().Append(button_control(L"认识", [this] { answer_vocabulary_card(true); },
                                               !vocabulary_busy_));
      card.Children().Append(answers);
    }
    if (!vocabulary_message_.empty())
      card.Children().Append(make_text(vocabulary_message_, 12, palette_.sub));
    add_row(group, 0xE7BE, L"复习", L"", nullptr, card);

    if (selected && selected->pack) {
      add_row(group, 0xE74C, L"这本书来自插件", L"在「插件」页卸载；卸载后复习进度仍会保留。", nullptr);
    } else if (selected && !selected->builtin) {
      add_row(group, 0xE74D, L"删除这个词表", L"删除「" + selected->name + L"」及其复习进度。",
              button_control(L"删除这个词表",
                             [this, book = *selected] { remove_vocabulary_book(book); },
                             !vocabulary_busy_));
    }
    add_row(group, 0xE777, L"清空复习进度", L"已经学过的词会从头开始。",
            button_control(L"清空复习进度", [this] { reset_vocabulary_progress(); },
                           !vocabulary_busy_));
  }

  // ---- 剪贴板 ----

  // 共享的「剪贴板」页（packages/ui/src/settings/pages/tools-page.tsx 和 clipboard-history-section.tsx）在 Windows 上画三组：「剪贴板」（历史开关、有历史时的清空历史）、开着历史时的「历史记录」（每条可重新复制、发到云剪贴板、固定和删除），以及打开云剪贴板面板的「云剪贴板」。历史文件与共享应用的相同（偏好目录下的 clipboard_history.json），经 msime_client_desktop_clipboard_history 读写；发到云剪贴板先读取共享偏好的来源，再经账号或中继 C ABI 请求。云剪贴板面板是输入工具，仍由共享应用打开。

  struct ClipboardEntry {
    std::wstring text;
    uint64_t timestamp_ms = 0;
    bool pinned = false;
  };

  // 账号和中继共用设备批准状态，但中继令牌失效不应提示登录水杉账号。
  using CloudClipboardState = nav::CloudClipboardState;

  struct CloudClipboardRelayDraft {
    bool relay = false;
    std::wstring url;
    std::wstring token;
  };

  void synchronize_cloud_clipboard_configuration() {
    const std::pair<std::wstring, std::wstring> configuration{
        document_.String(L"cloud_clipboard_relay.url", L""),
        document_.String(L"cloud_clipboard_relay.token", L"")};
    if (configuration == cloud_configuration_)
      return;
    cloud_configuration_ = configuration;
    cloud_relay_draft_.reset();
    reset_cloud_clipboard();
  }

  void redraw_clip() {
    if (!closed_ && current_page_ == "clip")
      queue_refresh();
  }

  bool clipboard_history_enabled() const { return document_.Boolean(L"clipboard_history", false); }

  size_t clipboard_history_limit() const {
    const auto value = document_.Number(L"clipboard_history_limit", 50);
    for (const auto limit : {50.0, 100.0, 200.0, 500.0, 1000.0})
      if (value == limit)
        return static_cast<size_t>(limit);
    return 50;
  }

  static JsonObject clipboard_action(wchar_t const *operation) {
    JsonObject action;
    action.SetNamedValue(L"operation", JsonValue::CreateStringValue(operation));
    return action;
  }

  // The history an answer carries, or nothing when the call failed or the answer cannot be read.
  static std::optional<std::vector<ClipboardEntry>> clipboard_entries(Response const &response) {
    if (!response.ok)
      return std::nullopt;
    try {
      const auto value = JsonObject::Parse(text(response.text)).GetNamedObject(L"value");
      std::vector<ClipboardEntry> entries;
      for (auto const &item : value.GetNamedArray(L"entries")) {
        const auto object = item.GetObject();
        ClipboardEntry entry;
        entry.text = named_string(object, L"text");
        const auto time = object.GetNamedValue(L"timestampMs", JsonValue::CreateNullValue());
        // Past 2^53 the number is not a real capture time, and a double at 2^64 does not convert to uint64_t at all.
        if (time.ValueType() == JsonValueType::Number && time.GetNumber() > 0 &&
            time.GetNumber() < 9007199254740992.0)
          entry.timestamp_ms = static_cast<uint64_t>(time.GetNumber());
        entry.pinned = object.GetNamedBoolean(L"pinned", false);
        if (!entry.text.empty())
          entries.push_back(std::move(entry));
      }
      return entries;
    } catch (...) {
      return std::nullopt;
    }
  }

  // One history action on the state directory, the store the input method and the shared app use; `done` gets the answer on the UI thread.
  void clipboard_request(JsonObject const &action, std::function<void(Response const &)> done) {
    JsonObject request;
    request.SetNamedValue(L"directory",
                          JsonValue::CreateStringValue(hstring(state_directory().wstring())));
    request.SetNamedValue(L"action", action);
    run_host(
        [body = utf8(request.Stringify())] {
          return take_response(msime_client_desktop_clipboard_history(
              reinterpret_cast<const uint8_t *>(body.data()), body.size()));
        },
        std::move(done));
  }

  // 进入页面、修订号变化或打开历史时读一次。读的期间有写入完成或又发了一次读取时以后来的为准，这次读到的丢掉（工作线程上的两次读取可能倒序返回）；读不到时与共享页一样不提示，列表保持原样。
  void load_clipboard_history() {
    clip_stale_ = false;
    clip_read_revision_ = document_.revision();
    const auto generation = clip_generation_;
    const auto read = ++clip_read_;
    clipboard_request(clipboard_action(L"list"), [this, generation, read](Response const &response) {
      if (generation != clip_generation_ || read != clip_read_)
        return;
      if (auto entries = clipboard_entries(response)) {
        clip_entries_ = std::move(*entries);
        clip_clear_armed_ = false;
        redraw_clip();
      }
    });
  }

  // 同步、清空、固定和删除：一次只做一件，做完换成操作后的历史；失败时在页面顶部说明，与共享页的 onError 相同。
  void change_clipboard_history(JsonObject const &action, std::wstring failure) {
    if (clip_busy_)
      return;
    clip_busy_ = true;
    const auto generation = clip_generation_;
    clipboard_request(action, [this, generation, failure = std::move(failure)](Response const &response) {
      // The history was switched on or off meanwhile; the answer belongs to the old state.
      if (generation != clip_generation_)
        return;
      clip_busy_ = false;
      auto entries = clipboard_entries(response);
      if (!entries) {
        show_notice(failure);
        return;
      }
      ++clip_read_;
      clip_clear_armed_ = false;
      clip_entries_ = std::move(*entries);
      redraw_clip();
    });
    redraw_clip();
  }

  void set_clipboard_entry_pinned(std::wstring const &value, bool pinned) {
    auto action = clipboard_action(L"set_pinned");
    action.SetNamedValue(L"text", JsonValue::CreateStringValue(hstring(value)));
    action.SetNamedValue(L"pinned", JsonValue::CreateBooleanValue(pinned));
    change_clipboard_history(action, L"无法更新剪贴板固定状态");
  }

  void remove_clipboard_entry(std::wstring const &value) {
    auto action = clipboard_action(L"remove");
    action.SetNamedValue(L"text", JsonValue::CreateStringValue(hstring(value)));
    change_clipboard_history(action, L"无法删除剪贴板记录");
  }

  // 共享页的 useClipboardHistoryToggle：写偏好；从开到关时再清空已存的历史，从关到开时把系统剪贴板上现有的那一条同步进来一次（Server 的 ClipboardMonitor 只记打开以后的复制，原来要另按「从系统剪贴板同步」才补得上这一条），失败时说明。开关一变，进行中的读写都作废，列表和「确认清空」复位，云剪贴板重新查询。
  void toggle_clipboard_history(bool on) {
    const bool was = clipboard_history_enabled();
    change([on](PreferencesDocument &doc) { doc.SetBoolean(L"clipboard_history", on); }, true);
    // 没存上时 commit 已重读设置、说明了原因，开关回到原样，历史也不能清空。
    if (clipboard_history_enabled() != on)
      return;
    ++clip_generation_;
    clip_busy_ = false;
    clip_stale_ = true;
    reset_cloud_clipboard();
    if (on) {
      if (!was)
        change_clipboard_history(clipboard_action(L"sync"), L"无法同步剪贴板历史");
      return;
    }
    clip_entries_.clear();
    clip_clear_armed_ = false;
    if (was)
      clipboard_request(clipboard_action(L"clear"), [this](Response const &response) {
        if (!response.ok)
          show_notice(L"无法清空剪贴板历史，请稍后重试。");
      });
  }

  // 「重新复制」：放回系统剪贴板，与共享应用的 copy_text 一样，历史开着时也把它记进历史（排到最前）。失败时与共享页一样不提示。
  void recopy_clipboard_entry(std::wstring const &value) {
    if (!copy_text(value))
      return;
    JsonObject request;
    request.SetNamedValue(L"directory",
                          JsonValue::CreateStringValue(hstring(state_directory().wstring())));
    request.SetNamedValue(L"text", JsonValue::CreateStringValue(hstring(value)));
    run_host(
        [body = utf8(request.Stringify())] {
          return take_response(msime_client_capture_clipboard_history(
              reinterpret_cast<const uint8_t *>(body.data()), body.size()));
        },
        [](Response const &) {});
  }

  // 选路和网络请求都在后台执行；中继密钥目录与共享面板相同，不另建设备身份。
  void cloud_clipboard_request(JsonObject const &action, std::function<void(Response const &)> done) {
    JsonObject request;
    request.SetNamedValue(L"directory",
                          JsonValue::CreateStringValue(hstring(account_directory().wstring())));
    request.SetNamedValue(L"action", action);
    JsonObject relay_request = JsonObject::Parse(request.Stringify());
    relay_request.SetNamedValue(L"preferences_directory",
                               JsonValue::CreateStringValue(hstring(state_directory().wstring())));
    run_host(
        [body = utf8(request.Stringify()), relay_body = utf8(relay_request.Stringify()),
         preferences = path_utf8(state_directory())] {
          return nav::cloud_clipboard_request(
              [&] {
                return take_response(msime_client_cloud_clipboard_source(
                    reinterpret_cast<const uint8_t *>(preferences.data()), preferences.size()));
              },
              [](Response const &response) -> std::wstring {
                try {
                  return JsonObject::Parse(text(response.text)).GetNamedObject(L"value")
                      .GetNamedString(L"source").c_str();
                } catch (...) {
                  return {};
                }
              },
              [&] {
                return take_response(msime_client_relay_cloud_clipboard(
                    reinterpret_cast<const uint8_t *>(relay_body.data()), relay_body.size()));
              },
              [&] {
                return take_response(msime_client_account_cloud_clipboard(
                    reinterpret_cast<const uint8_t *>(body.data()), body.size()));
              },
              [] { return Response{"{\"ok\":false,\"error\":\"invalid_cloud_clipboard_source\"}", false}; });
        },
        std::move(done));
  }

  void reset_cloud_clipboard() {
    ++cloud_generation_;
    cloud_checked_ = false;
    cloud_state_ = CloudClipboardState::checking;
    cloud_notice_.clear();
    cloud_sending_ = false;
  }

  // 每次带着历史打开页面时查一次所选服务和服务端的开关，不轮询。
  void check_cloud_clipboard() {
    reset_cloud_clipboard();
    cloud_checked_ = true;
    const auto generation = cloud_generation_;
    auto action = clipboard_action(L"list");
    action.SetNamedValue(L"search", JsonValue::CreateStringValue(L""));
    cloud_clipboard_request(action, [this, generation](Response const &response) {
      if (generation != cloud_generation_)
        return;
      if (!response.ok) {
        cloud_state_ = nav::cloud_clipboard_failure(response_error(response));
      } else {
        cloud_state_ = CloudClipboardState::ready;
        try {
          const auto value = JsonObject::Parse(text(response.text)).GetNamedObject(L"value");
          const auto enabled = value.GetNamedValue(L"enabled", JsonValue::CreateNullValue());
          const auto e2e = value.GetNamedValue(L"e2e", JsonValue::CreateNullValue());
          const auto status = e2e.ValueType() == JsonValueType::Object
                                  ? e2e.GetObject().GetNamedString(L"status", L"")
                                  : hstring{};
          if ((enabled.ValueType() == JsonValueType::Boolean && !enabled.GetBoolean()) ||
              status == L"disabled")
            cloud_state_ = CloudClipboardState::disabled;
          else if (status == L"pending")
            cloud_state_ = CloudClipboardState::pending;
        } catch (...) {
          cloud_state_ = CloudClipboardState::unavailable;
        }
      }
      redraw_clip();
    });
  }

  // 共享页的 sendToCloud：服务端按 UTF-16 单元计长度，最多 4000 个。
  void send_to_cloud_clipboard(std::wstring const &value) {
    if (cloud_state_ != CloudClipboardState::ready || cloud_sending_)
      return;
    constexpr size_t cloud_clipboard_max_utf16 = 4000;
    if (value.size() > cloud_clipboard_max_utf16) {
      cloud_notice_ = L"这条记录超过 " + std::to_wstring(cloud_clipboard_max_utf16) +
                      L" 个 UTF-16 单元，无法发到云剪贴板";
      redraw_clip();
      return;
    }
    const auto generation = cloud_generation_;
    cloud_sending_ = true;
    cloud_notice_ = L"正在发到云剪贴板…";
    auto action = clipboard_action(L"add");
    action.SetNamedValue(L"text", JsonValue::CreateStringValue(hstring(value)));
    cloud_clipboard_request(action, [this, generation](Response const &response) {
      if (generation != cloud_generation_)
        return;
      cloud_sending_ = false;
      if (response.ok) {
        cloud_notice_ = L"已发到云剪贴板";
      } else if (const auto failure = nav::cloud_clipboard_failure(response_error(response));
                 failure != CloudClipboardState::unavailable) {
        // 未登录、本机待批准和云剪贴板已关闭都要先由用户处理，在那之前不再允许发送。
        cloud_state_ = failure;
        cloud_notice_.clear();
      } else {
        cloud_notice_ = L"发到云剪贴板失败，请稍后重试";
      }
      redraw_clip();
    });
    redraw_clip();
  }

  // The note above the history: what the last send did, else why sending is not possible (cloudClipboardAvailabilityNote).
  std::wstring cloud_clipboard_note() const {
    if (!cloud_notice_.empty())
      return cloud_notice_;
    switch (cloud_state_) {
    case CloudClipboardState::signed_out:
      return L"登录水杉账号后可在设备间同步剪贴板";
    case CloudClipboardState::relay_unauthorized:
      return L"中继拒绝了令牌，请检查中继地址和令牌";
    case CloudClipboardState::pending:
      return L"本设备还没有获准使用云剪贴板：请打开云剪贴板查看本设备的核对码，再到一台已在使用云剪贴板的设备上点「允许」";
    case CloudClipboardState::disabled:
      return L"云剪贴板未开启";
    case CloudClipboardState::unavailable:
      return L"云剪贴板服务暂不可用，请稍后重试";
    default:
      return {};
    }
  }

  void build_clip_page(StackPanel const &page) {
    synchronize_cloud_clipboard_configuration();
    const bool enabled = clipboard_history_enabled();
    // 与共享页一样，存下的开关是关着的就不留历史：别的窗口或另一台设备经同步关掉它时，这里也不再显示旧条目和「清空历史」。
    if (!enabled) {
      clip_entries_.clear();
      clip_clear_armed_ = false;
    }
    if (!indexing_ && enabled) {
      if (!clip_busy_ && (clip_stale_ || clip_read_revision_ != document_.revision()))
        load_clipboard_history();
      if (!cloud_checked_)
        check_cloud_clipboard();
    }

    auto group = add_group(page, L"剪贴板");
    // 没有「从系统剪贴板同步」：打开历史的那一次已经自动同步过（toggle_clipboard_history）。只剩有历史时的「清空历史」，没有历史时开关下面不留空行。
    FrameworkElement actions_row{nullptr};
    if (!clip_entries_.empty()) {
      StackPanel actions;
      actions.Orientation(Orientation::Horizontal);
      actions.Spacing(8);
      actions.Margin(Thickness{0, 0, 0, 16});
      actions.Children().Append(button_control(
          clip_clear_armed_ ? L"确认清空" : L"清空历史",
          [this] {
            if (!clip_clear_armed_) {
              clip_clear_armed_ = true;
              redraw_clip();
              return;
            }
            change_clipboard_history(clipboard_action(L"clear"),
                                     L"无法清空剪贴板历史，请稍后重试。");
          },
          enabled && !clip_busy_));
      actions_row = actions;
    }
    add_row(group, 0xE81C, L"剪贴板历史",
            L"开启后记录复制的文本；保存关闭设置后清空已保存记录，且只记录文本类型。",
            toggle_control(L"剪贴板历史", enabled,
                           [this](bool on) { toggle_clipboard_history(on); }, !clip_busy_),
            actions_row);
    const auto history_limit = clipboard_history_limit();
    std::vector<Option> limit_options;
    for (const auto limit : {50, 100, 200, 500, 1000})
      limit_options.push_back({std::to_wstring(limit), std::to_wstring(limit) + L" 条", true});
    add_row(group, 0xE8AB, L"历史记录上限",
            L"达到上限后，最旧的未固定记录会被新记录替换。",
            select_control(L"历史记录上限", std::move(limit_options),
                           std::to_wstring(history_limit), [this](std::wstring const &value) {
                             try {
                               const auto limit = std::stoul(value);
                               change([limit](PreferencesDocument &doc) {
                                 doc.SetNumber(L"clipboard_history_limit", static_cast<double>(limit));
                               }, true);
                             } catch (...) {
                               show_notice(L"历史记录上限无效");
                             }
                           }, true));

    if (enabled) {
      auto history = add_group(page, L"历史记录");
      if (const auto note = cloud_clipboard_note(); !note.empty())
        history.Children().Append(group_note(note));
      StackPanel list;
      if (clip_entries_.empty()) {
        auto empty = make_text(L"暂无历史记录", 12, palette_.sub);
        empty.HorizontalAlignment(HorizontalAlignment::Center);
        empty.Margin(Thickness{16, 24, 16, 24});
        list.Children().Append(empty);
      }
      bool first = true;
      size_t entry_index = 0;
      for (auto const &entry : clip_entries_) {
        Border divider;
        divider.BorderThickness(Thickness{0, first ? 0.0 : 1.0, 0, 0});
        divider.BorderBrush(brush(palette_.divider));
        first = false;
        Grid row;
        row.MinHeight(62);
        row.Padding(Thickness{0, 8, 0, 8});
        row.ColumnSpacing(12);
        const auto entry_id = L"clipboard-entry-" + std::to_wstring(entry_index++);
        ColumnDefinition text_column;
        text_column.Width(GridLength{1, GridUnitType::Star});
        ColumnDefinition actions_column;
        actions_column.Width(GridLength{1, GridUnitType::Auto});
        row.ColumnDefinitions().Append(text_column);
        row.ColumnDefinitions().Append(actions_column);

        StackPanel texts;
        texts.Spacing(2);
        texts.VerticalAlignment(VerticalAlignment::Center);
        auto line = make_text(single_line(entry.text), 13, palette_.text);
        A11y::SetAutomationId(line, hstring(entry_id + L"-preview"));
        line.TextWrapping(TextWrapping::NoWrap);
        line.TextTrimming(TextTrimming::CharacterEllipsis);
        line.MaxLines(1);
        ToolTipService::SetToolTip(line, box_value(hstring(entry.text)));
        texts.Children().Append(line);
        std::wstring meta = entry.pinned ? L"已固定 · " : L"";
        meta += entry.timestamp_ms > 1'000'000'000'000ULL ? local_date_time(entry.timestamp_ms)
                                                           : std::wstring(L"旧记录");
        texts.Children().Append(make_text(meta, 11, palette_.sub));
        Grid::SetColumn(texts, 0);
        row.Children().Append(texts);

        StackPanel buttons;
        buttons.Orientation(Orientation::Horizontal);
        buttons.Spacing(6);
        buttons.VerticalAlignment(VerticalAlignment::Center);
        buttons.Children().Append(button_control(
            L"重新复制", [this, value = entry.text] { recopy_clipboard_entry(value); }));
        // 次要操作放进菜单，给窄窗口中的记录正文留出空间。
        MenuFlyout menu;
        MenuFlyoutItem cloud;
        cloud.Text(L"发到云剪贴板");
        cloud.IsEnabled(cloud_state_ == CloudClipboardState::ready && !cloud_sending_);
        cloud.Click([this, value = entry.text](Inspectable const &, RoutedEventArgs const &) {
          send_to_cloud_clipboard(value);
        });
        menu.Items().Append(cloud);
        MenuFlyoutItem pin;
        pin.Text(entry.pinned ? L"取消固定" : L"固定");
        pin.Click([this, value = entry.text, pinned = !entry.pinned](
                      Inspectable const &, RoutedEventArgs const &) {
          set_clipboard_entry_pinned(value, pinned);
        });
        A11y::SetName(pin, entry.pinned ? L"取消固定剪贴板记录" : L"固定剪贴板记录");
        A11y::SetAutomationId(pin, L"clipboard-entry-pin");
        menu.Items().Append(pin);
        menu.Items().Append(MenuFlyoutSeparator());
        MenuFlyoutItem remove;
        remove.Text(L"删除");
        remove.Click([this, value = entry.text](Inspectable const &, RoutedEventArgs const &) {
          remove_clipboard_entry(value);
        });
        A11y::SetName(remove, L"删除剪贴板记录");
        menu.Items().Append(remove);
        Button more;
        more.Width(32);
        more.Height(32);
        more.Padding(Thickness{0, 0, 0, 0});
        more.Content(make_icon(0xE712, 14, palette_.sub));
        more.IsEnabled(!clip_busy_);
        more.Flyout(menu);
        A11y::SetName(more, L"剪贴板记录的更多操作");
        A11y::SetAutomationId(more, hstring(entry_id + L"-actions"));
        ToolTipService::SetToolTip(more, box_value(L"更多操作"));
        buttons.Children().Append(more);
        Grid::SetColumn(buttons, 1);
        row.Children().Append(buttons);

        divider.Child(row);
        list.Children().Append(divider);
      }
      A11y::SetName(list, L"剪贴板历史");
      history.Children().Append(list);
    }

    // 设备批准和列表管理继续使用共享云剪贴板面板。
    auto cloud = add_group(page, L"云剪贴板");
    build_cloud_clipboard_relay_settings(cloud);
    shell_row(cloud, 0xE753, L"云剪贴板", L"", L"打开云剪贴板", nav::shell_links::cloud_clipboard);
  }

  void build_cloud_clipboard_relay_settings(StackPanel const &group) {
    if (!cloud_relay_draft_) {
      cloud_relay_draft_ = CloudClipboardRelayDraft{
          !cloud_configuration_.first.empty(), cloud_configuration_.first, cloud_configuration_.second};
    }
    auto source = select_control(
        L"云剪贴板同步方式", {{L"account", L"水杉账号"}, {L"relay", L"自建中继"}},
        cloud_relay_draft_->relay ? L"relay" : L"account",
        [this](std::wstring const &value) {
          cloud_relay_draft_->relay = value == L"relay";
          queue_refresh();
        });
    A11y::SetAutomationId(source, L"cloud-clipboard-source");
    add_row(group, 0xE753, L"云剪贴板同步方式",
            L"选择自建中继后只连接该中继，不会在失败时切回水杉账号。", source);

    Button save;
    save.Content(box_value(L"保存同步方式"));
    save.IsEnabled(loaded_ && (!cloud_relay_draft_->relay ||
                              (!cloud_relay_draft_->url.empty() && !cloud_relay_draft_->token.empty())));
    A11y::SetName(save, L"保存云剪贴板同步方式");
    A11y::SetAutomationId(save, L"cloud-clipboard-save-source");
    save.Click([this](Inspectable const &, RoutedEventArgs const &) {
      if (!cloud_relay_draft_)
        return;
      const auto draft = *cloud_relay_draft_;
      flush_pending();
      if (draft.relay) {
        document_.SetString(L"cloud_clipboard_relay.url", draft.url);
        document_.SetString(L"cloud_clipboard_relay.token", draft.token);
      } else {
        document_.Remove(L"cloud_clipboard_relay");
      }
      // 地址和令牌成对交给共享 Rust 偏好库校验；编辑中的值不写盘。
      if (commit()) {
        cloud_relay_draft_.reset();
        reset_cloud_clipboard();
        show_notice(L"已保存云剪贴板同步方式", InfoBarSeverity::Success);
      }
    });
    if (cloud_relay_draft_->relay) {
      TextBox url;
      url.Width(240);
      url.Text(hstring(cloud_relay_draft_->url));
      url.PlaceholderText(L"https://relay.example.com/msime");
      url.IsSpellCheckEnabled(false);
      url.IsTextPredictionEnabled(false);
      url.IsEnabled(loaded_);
      A11y::SetName(url, L"中继地址");
      A11y::SetAutomationId(url, L"cloud-clipboard-relay-url");
      url.TextChanged([this, weak_save = make_weak(save)](Inspectable const &sender, TextChangedEventArgs const &) {
        if (cloud_relay_draft_) {
          cloud_relay_draft_->url = sender.as<TextBox>().Text().c_str();
          if (auto button = weak_save.get())
            button.IsEnabled(loaded_ && !cloud_relay_draft_->url.empty() && !cloud_relay_draft_->token.empty());
        }
      });
      add_row(group, 0xE774, L"中继地址", L"使用 HTTPS 地址；仅本机回环地址允许 HTTP。", url);
      PasswordBox token;
      token.Width(240);
      token.Password(hstring(cloud_relay_draft_->token));
      token.PasswordRevealMode(PasswordRevealMode::Hidden);
      token.IsEnabled(loaded_);
      A11y::SetName(token, L"中继令牌");
      A11y::SetAutomationId(token, L"cloud-clipboard-relay-token");
      token.PasswordChanged([this, weak_save = make_weak(save)](Inspectable const &sender, RoutedEventArgs const &) {
        if (cloud_relay_draft_) {
          cloud_relay_draft_->token = sender.as<PasswordBox>().Password().c_str();
          if (auto button = weak_save.get())
            button.IsEnabled(loaded_ && !cloud_relay_draft_->url.empty() && !cloud_relay_draft_->token.empty());
        }
      });
      add_row(group, 0xE72E, L"中继令牌", L"16–256 个可打印 ASCII 字符，不含空格。", token);
    }
    add_row(group, 0xE74E, L"保存同步方式", L"地址和令牌只保存在本机，不随设置导出或账号同步。", save);
  }

  // ---- 打字统计 ----

  // 共享的「打字统计」页（packages/ui/src/settings/typing-statistics.tsx）在 Windows 上画的那一版：没有手机外观、不是 iOS，有「自动清理」。概览六格、标签（按键、趋势、类型、模式、方案、候选）下的图表、记录开关、保留期、清空和「统计没有数据」的说明都照搬。桌面版没有「刷新统计」：页面开着时每 5 秒和窗口重新激活时自动重读（poll_statistics）。也没有「打开数据目录」：统计文件所在的状态目录就是「维护与诊断 › 数据目录」的「打开」打开的那个，这里不再重复一个入口（Windows 上的共享页没有维护页的打开按钮，仍保留它）；派生数字和文案在 TypingStatisticsModel.h。统计经 msime_client_typing_statistics 读写状态目录下的 typing-statistics.json，与输入法和共享应用是同一份；请求带 status:true，回答里有文件是否写过和最后写入时间。所有读写都在工作线程上。

  static constexpr const wchar_t *statistics_failure =
      L"无法读取或保存统计，请稍后重试。原有统计不会被自动重置。";

  void redraw_stats() {
    if (!closed_ && current_page_ == "stats")
      queue_refresh();
  }

  // 每次进入页面都与共享页重新挂载一样从头开始：重新读取，标签回到「按键」，不选日子；上一次留下的请求作废。
  void reset_statistics_page() {
    ++stats_generation_;
    stats_.reset();
    stats_error_.clear();
    stats_busy_ = true;
    stats_request_ = 0;
    stats_last_request_ = {};
    stats_day_.reset();
    stats_selected_.reset();
    stats_tab_ = L"keys";
    stats_calendar_offset_ = 0;
    stats_details_offset_ = 0;
    stats_upload_.reset();
    stats_upload_busy_ = false;
    stats_upload_message_.clear();
    // 与共享页挂上时一样补传一次：离上次不到十分钟时 Rust 不发请求。
    sync_typing_upload();
  }

  // ---- 打字统计的「上传到云端」 ----
  // 经 msime_client_account 的 typing_upload_*（与账号页同一份会话），平台是 windows，日期是本地日期。离开页面再回来时 stats_generation_ 变了，旧的回答作废。

  JsonObject typing_upload_action(wchar_t const *operation) {
    auto action = account_action(operation);
    action.SetNamedValue(L"statistics_directory",
                         JsonValue::CreateStringValue(hstring(state_directory().wstring())));
    if (std::wstring_view(operation) != L"typing_upload_status") {
      action.SetNamedValue(L"day", JsonValue::CreateStringValue(hstring(local_day())));
      action.SetNamedValue(L"platform", JsonValue::CreateStringValue(L"windows"));
    }
    return action;
  }

  static std::optional<typing::UploadStatus> typing_upload_status(Response const &response) {
    if (!response.ok)
      return std::nullopt;
    const auto value = account_value(response);
    if (value.ValueType() != JsonValueType::Object)
      return std::nullopt;
    const auto object = value.GetObject();
    typing::UploadStatus status;
    status.enabled = object.GetNamedBoolean(L"enabled", false);
    status.signed_in = object.GetNamedBoolean(L"signed_in", false);
    const auto last = object.GetNamedValue(L"last_upload_ms", JsonValue::CreateNullValue());
    if (last.ValueType() == JsonValueType::Number && last.GetNumber() > 0)
      status.last_upload_ms = static_cast<uint64_t>(last.GetNumber());
    return status;
  }

  void typing_upload_request(JsonObject const &action, std::function<void(Response const &)> done) {
    if (account_directory().empty()) {
      done(Response{"{\"ok\":false,\"error\":\"account_storage\"}", false});
      return;
    }
    const auto generation = stats_generation_;
    run_host([body = account_body(action)] { return account_call(body); },
             [this, generation, done = std::move(done)](Response const &response) {
               if (generation == stats_generation_)
                 done(response);
             });
  }

  void read_typing_upload() {
    typing_upload_request(typing_upload_action(L"typing_upload_status"),
                          [this](Response const &response) {
                            if (auto status = typing_upload_status(response)) {
                              stats_upload_ = status;
                              redraw_stats();
                            }
                          });
  }

  // 打开了上传、已登录、离上次超过十分钟时上传最近几天；失败不打扰用户，只读出开关状态。
  void sync_typing_upload() {
    typing_upload_request(typing_upload_action(L"typing_upload"), [this](Response const &response) {
      if (auto status = typing_upload_status(response)) {
        stats_upload_ = status;
        redraw_stats();
      } else {
        read_typing_upload();
      }
    });
  }

  // 打开时要求已登录并立刻上传；关闭时删除云端副本。失败时开关在 Rust 那边可能已经变了（打开后上传失败、关闭后删除失败），重新读一次。
  void set_typing_upload(bool enabled) {
    if (stats_upload_busy_)
      return;
    stats_upload_busy_ = true;
    stats_upload_message_.clear();
    auto action = typing_upload_action(L"typing_upload_set");
    action.SetNamedValue(L"enabled", JsonValue::CreateBooleanValue(enabled));
    typing_upload_request(action, [this, enabled](Response const &response) {
      stats_upload_busy_ = false;
      if (auto status = typing_upload_status(response)) {
        stats_upload_ = status;
        stats_upload_message_ = std::wstring(typing::upload_done_message(enabled));
      } else {
        stats_upload_message_ = typing::upload_error_message(account_failure(response), enabled);
        read_typing_upload();
      }
      redraw_stats();
    });
    redraw_stats();
  }

  static JsonObject statistics_action(wchar_t const *operation) {
    JsonObject action;
    action.SetNamedValue(L"operation", JsonValue::CreateStringValue(operation));
    return action;
  }

  // 一次统计请求，在工作线程上调用并读出回答，`done` 在 UI 线程上拿到状态；读不了时是空。
  fire_and_forget statistics_host(JsonObject action,
                                  std::function<void(std::optional<StatisticsStatus> const &)> done) {
    JsonObject request;
    request.SetNamedValue(L"directory",
                          JsonValue::CreateStringValue(hstring(state_directory().wstring())));
    request.SetNamedValue(L"action", action);
    request.SetNamedValue(L"status", JsonValue::CreateBooleanValue(true));
    const auto body = utf8(request.Stringify());
    auto weak = get_weak();
    auto queue = DispatcherQueue();
    co_await resume_background();
    auto status = statistics_status(take_response(msime_client_typing_statistics(
        reinterpret_cast<const uint8_t *>(body.data()), body.size())));
    queue.TryEnqueue([weak, done = std::move(done), status = std::move(status)] {
      if (auto self = weak.get(); self && !self->closed_)
        done(status);
    });
  }

  // 共享页的 update：同一时间只有一个请求，有请求在途时这次操作作废，开关和下拉框重画回统计里的值。
  void update_statistics(JsonObject const &action, bool overview = false) {
    if (stats_request_ != 0) {
      redraw_stats();
      return;
    }
    const auto generation = stats_generation_;
    const auto request = ++stats_request_counter_;
    stats_request_ = request;
    stats_busy_ = true;
    stats_error_.clear();
    stats_request_started_ = std::chrono::steady_clock::now();
    if (overview)
      stats_last_request_ = stats_request_started_;
    statistics_host(action, [this, generation, request](std::optional<StatisticsStatus> const &status) {
      if (generation != stats_generation_ || stats_request_ != request)
        return;
      stats_request_ = 0;
      stats_busy_ = false;
      if (status)
        stats_ = *status;
      else
        stats_error_ = statistics_failure;
      redraw_stats();
    });
    redraw_stats();
  }

  bool window_minimized() {
    if (const auto presenter =
            AppWindow().Presenter().try_as<Microsoft::UI::Windowing::OverlappedPresenter>())
      return presenter.State() == Microsoft::UI::Windowing::OverlappedPresenterState::Minimized;
    return false;
  }

  // 共享页的 refreshWhenVisible：进入页面、窗口重新激活时和每 5 秒读一次；窗口最小化、上一次请求在途且不到 15 秒，或离上一次不到 1 秒时跳过。读到的与上次相同就不重画。
  void poll_statistics() {
    if (closed_ || current_page_ != "stats" || window_minimized())
      return;
    const auto today = typing::day_key(local_civil_day());
    const bool date_changed = typing::statistics_date_changed(stats_day_, today);
    if (date_changed) {
      stats_day_ = today;
      if (stats_)
        redraw_stats();
    }
    const auto now = std::chrono::steady_clock::now();
    if ((stats_request_ != 0 && now - stats_request_started_ <= std::chrono::seconds(15)) ||
        now - stats_last_request_ < std::chrono::seconds(1))
      return;
    stats_last_request_ = now;
    const bool had_error = !stats_error_.empty();
    stats_error_.clear();
    const auto generation = stats_generation_;
    const auto request = ++stats_request_counter_;
    stats_request_ = request;
    stats_request_started_ = now;
    statistics_host(statistics_action(L"load"),
                    [this, generation, request](std::optional<StatisticsStatus> const &status) {
                      if (generation != stats_generation_ || stats_request_ != request)
                        return;
                      stats_request_ = 0;
                      bool changed = stats_busy_;
                      stats_busy_ = false;
                      if (!status) {
                        stats_error_ = statistics_failure;
                        changed = true;
                      } else if (!stats_ || stats_->signature != status->signature) {
                        stats_ = *status;
                        changed = true;
                      }
                      if (changed)
                        redraw_stats();
                    });
    if (had_error)
      redraw_stats();
  }

  void start_statistics_timer() {
    if (!stats_timer_) {
      stats_timer_ = DispatcherQueue().CreateTimer();
      stats_timer_.Interval(std::chrono::seconds(5));
      stats_timer_.IsRepeating(true);
      stats_timer_.Tick([this](Microsoft::UI::Dispatching::DispatcherQueueTimer const &,
                               Inspectable const &) {
        if (closed_ || current_page_ != "stats") {
          stats_timer_.Stop();
          return;
        }
        poll_statistics();
      });
    }
    if (!stats_timer_.IsRunning())
      stats_timer_.Start();
  }

  void set_statistics_enabled(bool enabled) {
    auto action = statistics_action(L"set_enabled");
    action.SetNamedValue(L"enabled", JsonValue::CreateBooleanValue(enabled));
    update_statistics(action);
  }

  // 保留期从用户的今天往回数，所以带上本地日期，与 Tauri 外壳的 set_typing_statistics_retention 相同。
  void set_statistics_retention(std::wstring const &retention) {
    auto action = statistics_action(L"set_retention");
    action.SetNamedValue(L"retention", JsonValue::CreateStringValue(hstring(retention)));
    action.SetNamedValue(L"day", JsonValue::CreateStringValue(hstring(local_day())));
    update_statistics(action);
  }

  fire_and_forget reset_statistics() {
    auto strong = get_strong();
    if (!co_await confirm(L"清空打字统计",
                          L"累计字数、分类、每日记录和按键次数都会被删除，无法恢复。", L"清空"))
      co_return;
    stats_selected_.reset();
    update_statistics(statistics_action(L"reset"));
  }

  // 点按一天看它的分类与占比，再点一次回到整个时间范围。
  void select_statistics_day(std::string const &key) {
    if (stats_selected_ == key)
      stats_selected_.reset();
    else
      stats_selected_ = key;
    redraw_stats();
  }

  // 日历热力图和按日明细比卡片宽时横向滚动。页面每次重画都换上新的滚动区，这里把横向位置记在 `slot` 里、在新的滚动区上恢复，否则往右滚到最近几周再点一天，重画后又回到最左边，选中的那一格不在视野里；共享页由 React 保留同一个滚动区，没有这个问题。
  void keep_horizontal_offset(ScrollViewer const &scroller, double &slot) {
    scroller.ViewChanged([&slot](Inspectable const &sender, ScrollViewerViewChangedEventArgs const &) {
      slot = sender.as<ScrollViewer>().HorizontalOffset();
    });
    if (slot <= 0)
      return;
    scroller.Loaded([offset = slot](Inspectable const &sender, RoutedEventArgs const &) {
      const auto self = sender.as<ScrollViewer>();
      self.UpdateLayout();
      self.ChangeView(offset, nullptr, nullptr, true);
    });
  }

  // 一节统计：标题（可以没有）和放图表的卡片。
  StackPanel stats_section(StackPanel const &page, std::wstring const &title) {
    auto body = add_group(page, title);
    body.Padding(Thickness{12, 14, 12, 14});
    return body;
  }

  TextBlock stats_caption(std::wstring_view value, double top = 7) {
    auto caption = make_text(value, 12, palette_.faint);
    caption.Margin(Thickness{0, top, 0, 0});
    return caption;
  }

  TextBlock stats_footer(std::wstring_view value) {
    auto footer = make_text(value, 12, palette_.sub);
    footer.Margin(Thickness{0, 14, 0, 0});
    return footer;
  }

  TextBlock stats_empty(std::wstring_view value) {
    auto empty = make_text(value, 13, palette_.sub);
    empty.TextAlignment(TextAlignment::Center);
    empty.HorizontalAlignment(HorizontalAlignment::Center);
    empty.Margin(Thickness{0, 14, 0, 4});
    return empty;
  }

  FrameworkElement stats_error_bar() {
    InfoBar bar;
    bar.IsOpen(true);
    bar.IsClosable(false);
    bar.Severity(InfoBarSeverity::Error);
    bar.Message(hstring(stats_error_));
    bar.Margin(Thickness{0, 0, 0, 12});
    return bar;
  }

  // 热力图的五档颜色：0 档是底色，其余是不同不透明度的强调色。
  Color heat_fill(int level) const {
    if (level <= 0)
      return palette_.subtle2;
    return faded(palette_.accent, typing::heat_opacity[static_cast<size_t>(std::min(level, 4))]);
  }

  Border heat_square(int level, double size) {
    Border square;
    square.Width(size);
    square.Height(size);
    square.CornerRadius(CornerRadius{2, 2, 2, 2});
    square.Background(brush(heat_fill(level)));
    square.VerticalAlignment(VerticalAlignment::Center);
    return square;
  }

  // 「少 ■■■■■ 多」。
  FrameworkElement heat_legend() {
    StackPanel legend;
    legend.Orientation(Orientation::Horizontal);
    legend.Spacing(4);
    legend.Margin(Thickness{0, 8, 0, 0});
    legend.Children().Append(make_text(L"少", 10, palette_.faint));
    for (int level = 0; level < 5; ++level)
      legend.Children().Append(heat_square(level, 12));
    legend.Children().Append(make_text(L"多", 10, palette_.faint));
    A11y::SetAccessibilityView(legend, Microsoft::UI::Xaml::Automation::Peers::AccessibilityView::Raw);
    return legend;
  }

  // 图表下的横轴：两个标签分在两端，三个时中间那个居中。
  FrameworkElement chart_axis(std::vector<std::wstring> const &labels) {
    Grid axis;
    axis.Margin(Thickness{0, 7, 0, 0});
    for (size_t index = 0; index < labels.size(); ++index) {
      ColumnDefinition column;
      column.Width(GridLength{1, GridUnitType::Star});
      axis.ColumnDefinitions().Append(column);
      auto label = make_text(labels[index], 12, palette_.faint);
      label.TextAlignment(index == 0                     ? TextAlignment::Left
                          : index + 1 == labels.size() ? TextAlignment::Right
                                                       : TextAlignment::Center);
      Grid::SetColumn(label, static_cast<int32_t>(index));
      axis.Children().Append(label);
    }
    return axis;
  }

  // 图表上可以点的一格（柱子、热力图的一天），与共享页的 StatisticsChartButton 一样带提示、读屏名和选中状态。
  Button chart_button(UIElement const &face, std::wstring const &tip, std::wstring const &name,
                      bool selected, std::function<void()> click) {
    Button button;
    button.Padding(Thickness{0, 0, 0, 0});
    button.MinWidth(0);
    button.MinHeight(0);
    button.BorderThickness(Thickness{0, 0, 0, 0});
    button.Background(brush(transparent_color()));
    button.HorizontalAlignment(HorizontalAlignment::Stretch);
    button.VerticalAlignment(VerticalAlignment::Stretch);
    button.HorizontalContentAlignment(HorizontalAlignment::Stretch);
    button.VerticalContentAlignment(VerticalAlignment::Stretch);
    button.Content(face);
    ToolTipService::SetToolTip(button, box_value(hstring(tip)));
    A11y::SetName(button, hstring(name));
    if (selected)
      A11y::SetItemStatus(button, L"已选择");
    button.Click([on_click = std::move(click)](Inspectable const &, RoutedEventArgs const &) {
      on_click();
    });
    return button;
  }

  // 参照线的颜色：次要文字色再淡三成。
  Color reference_color() const {
    auto color = palette_.faint;
    color.A = static_cast<uint8_t>(std::lround(color.A * 0.7));
    return color;
  }

  // 标签行上方的六格和两行概况。
  void stats_overview(StackPanel const &page, typing::Statistics const &statistics,
                      std::string const &today, std::wstring const &scope_title, double scope_total,
                      typing::Activity const &activity) {
    auto body = stats_section(page, L"");
    A11y::SetName(body, L"统计概览");
    auto grid = responsive_grid(174, 18, 3);
    const auto metrics = typing::overview_metrics(statistics, today, scope_title, scope_total, activity);
    for (size_t index = 0; index < metrics.size(); ++index) {
      auto const &metric = metrics[index];
      StackPanel cell;
      cell.Spacing(4);
      auto label = make_text(metric.label, 12, palette_.sub);
      if (!metric.title.empty())
        ToolTipService::SetToolTip(label, box_value(hstring(metric.title)));
      cell.Children().Append(label);
      StackPanel line;
      line.Orientation(Orientation::Horizontal);
      line.Spacing(4);
      auto value = make_text(metric.value, 22, palette_.accent);
      value.FontWeight(Windows::UI::Text::FontWeight{650});
      value.TextWrapping(TextWrapping::NoWrap);
      A11y::SetName(value, hstring(metric.accessible + L" " + metric.value + metric.unit));
      line.Children().Append(value);
      if (!metric.unit.empty()) {
        auto unit = make_text(metric.unit, 12, palette_.sub);
        unit.VerticalAlignment(VerticalAlignment::Bottom);
        unit.Margin(Thickness{0, 0, 0, 4});
        line.Children().Append(unit);
      }
      cell.Children().Append(line);
      if (!metric.note.empty())
        cell.Children().Append(make_text(metric.note, 12, palette_.sub));
      grid.Children().Append(cell);
    }
    body.Children().Append(grid);
    const auto details = typing::overview_details(activity);
    Grid lines;
    lines.Margin(Thickness{0, 7, 0, 0});
    lines.ColumnSpacing(16);
    for (int column = 0; column < 2; ++column) {
      ColumnDefinition definition;
      definition.Width(GridLength{1, GridUnitType::Star});
      lines.ColumnDefinitions().Append(definition);
    }
    auto summary = make_text(details.summary, 12, palette_.faint);
    lines.Children().Append(summary);
    auto best = make_text(details.best, 12, palette_.faint);
    best.TextAlignment(TextAlignment::Right);
    Grid::SetColumn(best, 1);
    lines.Children().Append(best);
    body.Children().Append(lines);
    body.Children().Append(stats_footer(
        activity.has_activity
            ? L"速度按连续打字的时间计算，两次上屏间隔超过 10 秒算休息、不计入；只统计汉字与字母，数字和标点不参与。"
            : L"还没有测量到活跃时长。这项从本次更新后开始记录，之前的输入只有字数。"));
  }

  // 「每日趋势」：近 30 天每天一根柱，点按选中那一天，虚线是有记录的日子的日均。
  void stats_trend(StackPanel const &page, typing::Statistics const &statistics,
                   std::vector<typing::DayEntry> const &days) {
    const auto scale = typing::trend_scale(statistics, days);
    auto body = stats_section(page, L"每日趋势 · 近 " + std::to_wstring(days.size()) + L" 天");
    body.Children().Append(stats_caption(typing::trend_caption(scale), 0));
    constexpr double height = 145;
    Grid plot;
    plot.Height(height);
    plot.Margin(Thickness{0, 12, 0, 0});
    plot.ColumnSpacing(3);
    for (size_t index = 0; index < days.size(); ++index) {
      ColumnDefinition column;
      column.Width(GridLength{1, GridUnitType::Star});
      plot.ColumnDefinitions().Append(column);
      auto const &day = days[index];
      const double count = typing::value_of(statistics.days, day.key);
      const bool chosen = stats_selected_ == day.key;
      const bool dimmed = stats_selected_.has_value() && !chosen;
      Grid face;
      Border bar;
      bar.Height(typing::bar_percent(count, scale.maximum) / 100 * height);
      bar.VerticalAlignment(VerticalAlignment::Bottom);
      bar.CornerRadius(CornerRadius{3, 3, 1, 1});
      bar.Background(brush(chosen ? chart_color(typing::selected_color) : palette_.accent));
      bar.Opacity(dimmed ? 0.4 : chosen ? 1.0 : 0.85);
      face.Children().Append(bar);
      auto button = chart_button(face, day.label + L"：" + typing::js_number(count) + L" 字符",
                                 day.label + L"，" + typing::js_number(count) + L" 字符", chosen,
                                 [this, key = day.key] { select_statistics_day(key); });
      Grid::SetColumn(button, static_cast<int32_t>(index));
      plot.Children().Append(button);
    }
    if (scale.average > 0) {
      auto overlay = chart_overlay([fraction = scale.average / scale.maximum,
                                    color = reference_color()](Canvas const &canvas, double width,
                                                               double plot_height) {
        const double y = plot_height - fraction * plot_height;
        chart_line(canvas, 0, y, width, y, color, 1, true);
      });
      Grid::SetColumnSpan(overlay, static_cast<int32_t>(days.size()));
      plot.Children().Append(overlay);
    }
    body.Children().Append(plot);
    body.Children().Append(chart_axis({days.front().label, days.back().label}));
    body.Children().Append(stats_footer(L"点按柱形查看当天的分类与占比。"));
    if (stats_selected_) {
      auto back = button_control(L"返回整个时间范围", [this] {
        stats_selected_.reset();
        redraw_stats();
      });
      back.Margin(Thickness{0, 12, 0, 0});
      back.HorizontalAlignment(HorizontalAlignment::Left);
      body.Children().Append(back);
    }
  }

  // 「今日时段」：今天每个小时一根柱，有平时的数据时再画一条前几天平均的线，同一个刻度。
  void stats_hours(StackPanel const &page, std::vector<double> const &hours,
                   std::optional<typing::UsualHours> const &usual) {
    auto body = stats_section(page, L"今日时段");
    double peak = 1, total = 0, highest = 0;
    for (const double value : hours) {
      peak = std::max(peak, value);
      highest = std::max(highest, value);
      total += value;
    }
    if (usual)
      for (const double value : usual->hours)
        peak = std::max(peak, value);
    body.Children().Append(stats_caption(L"最高 " + typing::zh_number(typing::js_round(highest)) +
                                             L" 字符 / 小时 · 共 " +
                                             typing::zh_number(typing::js_round(total)) + L" 字符",
                                         0));
    if (usual) {
      StackPanel legend;
      legend.Orientation(Orientation::Horizontal);
      legend.Spacing(16);
      legend.Margin(Thickness{0, 8, 0, 0});
      StackPanel today_key;
      today_key.Orientation(Orientation::Horizontal);
      today_key.Spacing(6);
      today_key.Children().Append(heat_square(4, 10));
      today_key.Children().Append(make_text(L"今日", 11, palette_.sub));
      legend.Children().Append(today_key);
      StackPanel usual_key;
      usual_key.Orientation(Orientation::Horizontal);
      usual_key.Spacing(6);
      Border line;
      line.Width(14);
      line.Height(2);
      line.CornerRadius(CornerRadius{1, 1, 1, 1});
      line.Background(brush(palette_.sub));
      line.VerticalAlignment(VerticalAlignment::Center);
      usual_key.Children().Append(line);
      usual_key.Children().Append(make_text(
          L"平时（前 " + typing::zh_number(usual->days) + L" 天平均）", 11, palette_.sub));
      legend.Children().Append(usual_key);
      A11y::SetAccessibilityView(legend,
                                 Microsoft::UI::Xaml::Automation::Peers::AccessibilityView::Raw);
      body.Children().Append(legend);
    }
    constexpr double height = 110;
    Grid plot;
    plot.Height(height);
    plot.Margin(Thickness{0, 12, 0, 0});
    plot.ColumnSpacing(3);
    A11y::SetName(plot, usual ? L"今日各时段输入分布，与平时对比" : L"今日各时段输入分布");
    for (size_t hour = 0; hour < hours.size(); ++hour) {
      ColumnDefinition column;
      column.Width(GridLength{1, GridUnitType::Star});
      plot.ColumnDefinitions().Append(column);
      const double value = hours[hour];
      Grid cell;
      cell.Background(brush(transparent_color()));
      Border bar;
      bar.Height(typing::bar_percent(value, peak) / 100 * height);
      bar.VerticalAlignment(VerticalAlignment::Bottom);
      bar.CornerRadius(CornerRadius{3, 3, 1, 1});
      bar.Background(brush(palette_.accent));
      bar.Opacity(value == 0 ? 0.25 : 0.85);
      cell.Children().Append(bar);
      std::wstring tip = std::to_wstring(hour) + L" 时：今日 " + typing::js_number(value) + L" 字符";
      if (usual)
        tip += L"，平时 " + typing::zh_number(typing::js_round(usual->hours[hour])) + L" 字符";
      ToolTipService::SetToolTip(cell, box_value(hstring(tip)));
      A11y::SetName(cell, hstring(std::to_wstring(hour) + L" 时，" + typing::js_number(value) + L" 字符"));
      Grid::SetColumn(cell, static_cast<int32_t>(hour));
      plot.Children().Append(cell);
    }
    if (usual) {
      std::vector<std::optional<double>> values(usual->hours.begin(), usual->hours.end());
      auto overlay = chart_overlay([runs = typing::line_runs(values), peak,
                                    color = palette_.sub](Canvas const &canvas, double width,
                                                          double plot_height) {
        for (auto const &run : runs) {
          std::vector<Windows::Foundation::Point> points;
          for (auto const &[x, value] : run)
            points.push_back(point_at(x / 100 * width, plot_height - value / peak * plot_height));
          chart_polyline(canvas, points, color, 1.5);
        }
      });
      Grid::SetColumnSpan(overlay, static_cast<int32_t>(hours.size()));
      plot.Children().Append(overlay);
    }
    body.Children().Append(plot);
    body.Children().Append(chart_axis({L"0 时", L"12 时", L"23 时"}));
  }

  // 「速度趋势」：每天每分钟的字数画成折线，纵轴从最慢的那天附近起，两端标出刻度；活跃不足一分钟的日子是断开的。
  void stats_speed(StackPanel const &page, typing::Statistics const &statistics,
                   std::vector<typing::DayEntry> const &days, double average_speed) {
    auto body = stats_section(page, L"速度趋势");
    std::vector<std::string> keys;
    for (auto const &day : days)
      keys.push_back(day.key);
    const auto speeds = typing::daily_speeds(statistics, keys);
    const auto scale = typing::speed_scale(speeds, average_speed);
    if (!scale) {
      body.Children().Append(stats_empty(
          L"这段时间还没有测量到足够的活跃时长。每天连续打字满 1 分钟后，这里会画出当天的速度。"));
      return;
    }
    body.Children().Append(stats_caption(typing::speed_caption(*scale, average_speed), 0));
    Grid plot;
    plot.Height(120);
    A11y::SetName(plot, hstring(L"每日输入速度折线图，" + typing::js_number(static_cast<double>(scale->measured)) +
                                L" 天有测量，平均 " + typing::zh_number(typing::js_round(average_speed)) +
                                L" 字每分钟"));
    auto top = make_text(typing::js_number(scale->top), 10, palette_.faint);
    top.VerticalAlignment(VerticalAlignment::Top);
    top.IsHitTestVisible(false);
    plot.Children().Append(top);
    auto bottom = make_text(typing::js_number(scale->bottom), 10, palette_.faint);
    bottom.VerticalAlignment(VerticalAlignment::Bottom);
    bottom.Margin(Thickness{0, 0, 0, 4});
    bottom.IsHitTestVisible(false);
    plot.Children().Append(bottom);
    std::optional<size_t> chosen;
    for (size_t index = 0; index < days.size(); ++index)
      if (stats_selected_ == days[index].key)
        chosen = index;
    std::vector<std::pair<size_t, double>> dots;
    for (size_t index = 0; index < speeds.size(); ++index)
      if (speeds[index] && (typing::speed_dotted(speeds, index, days.size()) || chosen == index))
        dots.emplace_back(index, typing::speed_height(*scale, *speeds[index]));
    std::vector<std::optional<double>> heights;
    for (auto const &speed : speeds)
      heights.push_back(speed ? std::optional<double>(typing::speed_height(*scale, *speed))
                              : std::nullopt);
    plot.Children().Append(chart_overlay(
        [runs = typing::line_runs(heights), dots, chosen, count = days.size(),
         average = typing::speed_height(*scale, average_speed), reference = reference_color(),
         accent = palette_.accent, ring = palette_.preview_bg,
         selected = chart_color(typing::selected_color)](Canvas const &canvas, double width,
                                                         double height) {
          auto y = [height](double percent) { return height - percent / 100 * height; };
          chart_line(canvas, 0, y(average), width, y(average), reference, 1, true);
          for (auto const &run : runs) {
            std::vector<Windows::Foundation::Point> points;
            for (auto const &[x, value] : run)
              points.push_back(point_at(x / 100 * width, y(value)));
            chart_polyline(canvas, points, accent, 2);
          }
          for (auto const &[index, value] : dots) {
            const bool picked = chosen == index;
            chart_dot(canvas, typing::column_center(index, count) / 100 * width, y(value),
                      picked ? 10 : 7, picked ? selected : accent, ring);
          }
        }));
    Grid columns;
    for (size_t index = 0; index < days.size(); ++index) {
      ColumnDefinition column;
      column.Width(GridLength{1, GridUnitType::Star});
      columns.ColumnDefinitions().Append(column);
      Border hover;
      hover.Background(brush(transparent_color()));
      ToolTipService::SetToolTip(
          hover, box_value(hstring(days[index].label + L"：" +
                                   (speeds[index] ? typing::zh_number(typing::js_round(*speeds[index])) +
                                                        L" 字 / 分钟"
                                                  : std::wstring(L"活跃不足 1 分钟，未计算")))));
      A11y::SetAccessibilityView(hover,
                                 Microsoft::UI::Xaml::Automation::Peers::AccessibilityView::Raw);
      Grid::SetColumn(hover, static_cast<int32_t>(index));
      columns.Children().Append(hover);
    }
    plot.Children().Append(columns);
    Border frame;
    frame.Margin(Thickness{0, 12, 0, 0});
    frame.BorderThickness(Thickness{0, 0, 0, 1});
    frame.BorderBrush(brush(palette_.divider));
    frame.Child(plot);
    body.Children().Append(frame);
    body.Children().Append(chart_axis({days.front().label, days.back().label}));
  }

  // 「日历热力图」：近 53 周每天一格，一列一周，从星期一排起；点按选中那一天。
  void stats_calendar(StackPanel const &page, typing::Statistics const &statistics,
                      typing::CivilDay today) {
    auto body = stats_section(page, L"日历热力图");
    body.Children().Append(stats_caption(L"近 12 个月，颜色越深输入越多", 0));
    const auto weeks = typing::heatmap_weeks(statistics.days, today);
    double maximum = 1;
    for (auto const &week : weeks)
      for (auto const &day : week)
        maximum = std::max(maximum, day.count);
    constexpr double cell = 14, gap = 3, gutter = 18;
    StackPanel grid;
    A11y::SetName(grid, L"每日输入热力图");
    Canvas months;
    months.Height(16);
    months.Width(gutter + 4 + static_cast<double>(weeks.size()) * (cell + gap));
    const auto labels = typing::heatmap_month_labels(weeks);
    for (size_t index = 0; index < labels.size(); ++index) {
      if (labels[index].empty())
        continue;
      auto label = make_text(labels[index], 9, palette_.faint);
      label.TextWrapping(TextWrapping::NoWrap);
      Canvas::SetLeft(label, gutter + 4 + static_cast<double>(index) * (cell + gap));
      Canvas::SetTop(label, 3);
      months.Children().Append(label);
    }
    A11y::SetAccessibilityView(months, Microsoft::UI::Xaml::Automation::Peers::AccessibilityView::Raw);
    grid.Children().Append(months);
    StackPanel rows;
    rows.Orientation(Orientation::Horizontal);
    rows.Spacing(4);
    StackPanel weekdays;
    weekdays.Width(gutter);
    weekdays.Spacing(gap);
    for (const wchar_t *name : {L"一", L"", L"三", L"", L"五", L"", L""}) {
      auto label = make_text(name, 9, palette_.faint);
      label.Height(cell);
      label.TextAlignment(TextAlignment::Right);
      label.LineHeight(cell);
      weekdays.Children().Append(label);
    }
    A11y::SetAccessibilityView(weekdays,
                               Microsoft::UI::Xaml::Automation::Peers::AccessibilityView::Raw);
    rows.Children().Append(weekdays);
    StackPanel columns;
    columns.Orientation(Orientation::Horizontal);
    columns.Spacing(gap);
    for (auto const &week : weeks) {
      StackPanel column;
      column.Spacing(gap);
      for (auto const &day : week) {
        if (day.future) {
          Border blank;
          blank.Width(cell);
          blank.Height(cell);
          column.Children().Append(blank);
          continue;
        }
        const bool chosen = stats_selected_ == day.key;
        Border fill;
        fill.CornerRadius(CornerRadius{3, 3, 3, 3});
        fill.Background(brush(heat_fill(typing::day_heat_level(day.count, maximum))));
        if (chosen) {
          fill.BorderThickness(Thickness{2, 2, 2, 2});
          fill.BorderBrush(brush(chart_color(typing::selected_color)));
        }
        auto button = chart_button(
            fill,
            day.label + L"：" +
                (day.count > 0 ? typing::zh_number(day.count) + L" 字符" : std::wstring(L"无记录")),
            L"热力图：" + day.label + L"，" + typing::js_number(day.count) + L" 字符", chosen,
            [this, key = day.key] { select_statistics_day(key); });
        button.Width(cell);
        button.Height(cell);
        button.CornerRadius(CornerRadius{3, 3, 3, 3});
        column.Children().Append(button);
      }
      columns.Children().Append(column);
    }
    rows.Children().Append(columns);
    grid.Children().Append(rows);
    ScrollViewer scroller;
    scroller.HorizontalScrollBarVisibility(ScrollBarVisibility::Auto);
    scroller.VerticalScrollBarVisibility(ScrollBarVisibility::Disabled);
    scroller.HorizontalScrollMode(ScrollMode::Enabled);
    scroller.VerticalScrollMode(ScrollMode::Disabled);
    scroller.Margin(Thickness{0, 8, 0, 0});
    scroller.Padding(Thickness{0, 0, 0, 4});
    scroller.Content(grid);
    keep_horizontal_offset(scroller, stats_calendar_offset_);
    body.Children().Append(scroller);
    body.Children().Append(heat_legend());
  }

  // 一排排按键，宽度按键的单位数分，颜色按次数分档；没有 id 的是对齐用的空位。
  FrameworkElement key_rows(std::vector<std::vector<typing::HeatKey>> const &rows, double maximum) {
    StackPanel panel;
    panel.Spacing(4);
    for (auto const &row : rows) {
      Grid line;
      line.ColumnSpacing(4);
      for (size_t index = 0; index < row.size(); ++index) {
        auto const &key = row[index];
        ColumnDefinition column;
        column.Width(GridLength{key.weight, GridUnitType::Star});
        line.ColumnDefinitions().Append(column);
        if (key.code.empty())
          continue;
        const int level = typing::key_heat_level(key.count, maximum);
        Border cap;
        cap.Height(36);
        cap.CornerRadius(CornerRadius{5, 5, 5, 5});
        cap.BorderThickness(Thickness{1, 1, 1, 1});
        cap.BorderBrush(brush(palette_.stroke2));
        cap.Background(brush(heat_fill(level)));
        auto label = make_text(key.label, 11, level >= 3 ? palette_.on_accent : palette_.sub);
        label.TextWrapping(TextWrapping::NoWrap);
        label.TextTrimming(TextTrimming::CharacterEllipsis);
        label.HorizontalAlignment(HorizontalAlignment::Center);
        label.VerticalAlignment(VerticalAlignment::Center);
        label.Margin(Thickness{2, 0, 2, 0});
        cap.Child(label);
        const auto name = key.name + L"，" + typing::zh_number(key.count) + L" 次";
        ToolTipService::SetToolTip(cap, box_value(hstring(name)));
        A11y::SetName(cap, hstring(name));
        Grid::SetColumn(cap, static_cast<int32_t>(index));
        line.Children().Append(cap);
      }
      panel.Children().Append(line);
    }
    return panel;
  }

  // 「按键热力图」：当前范围内每个键按了多少次，画在记下这些次数的那种键盘上。
  void stats_keys(StackPanel const &page, typing::Statistics const &statistics,
                  std::optional<std::vector<std::string>> const &scope, std::wstring const &scope_label) {
    const auto model = typing::keyboard_model(typing::scoped_key_counts(statistics.daily_keys, scope));
    auto body = stats_section(page, L"按键热力图 · " + scope_label);
    if (model.total == 0) {
      body.Children().Append(stats_empty(L"这段时间还没有按键记录"));
    } else {
      body.Children().Append(
          stats_caption(L"共 " + typing::zh_number(model.total) + L" 次按键，颜色越深按得越多"));
      auto board = key_rows(model.rows, model.maximum);
      board.Margin(Thickness{0, 12, 0, 0});
      A11y::SetName(board, L"按键热力图");
      body.Children().Append(board);
      if (model.nine) {
        body.Children().Append(stats_caption(L"九宫格", 14));
        auto nine = key_rows(*model.nine, model.maximum);
        nine.MaxWidth(260);
        nine.HorizontalAlignment(HorizontalAlignment::Center);
        nine.Margin(Thickness{0, 8, 0, 0});
        A11y::SetName(nine, L"九宫格按键");
        body.Children().Append(nine);
      }
      if (!model.others.empty()) {
        body.Children().Append(stats_caption(L"其他键", 14));
        VariableSizedWrapGrid others;
        others.Orientation(Orientation::Horizontal);
        others.ItemWidth(190);
        others.ItemHeight(32);
        others.Margin(Thickness{0, 8, 0, 0});
        A11y::SetName(others, L"其他键");
        for (auto const &key : model.others) {
          Border chip;
          chip.CornerRadius(CornerRadius{5, 5, 5, 5});
          chip.BorderThickness(Thickness{1, 1, 1, 1});
          chip.BorderBrush(brush(palette_.stroke2));
          chip.Padding(Thickness{8, 4, 8, 4});
          chip.Margin(Thickness{0, 0, 6, 6});
          chip.HorizontalAlignment(HorizontalAlignment::Left);
          StackPanel content;
          content.Orientation(Orientation::Horizontal);
          content.Spacing(6);
          content.Children().Append(heat_square(typing::key_heat_level(key.count, model.maximum), 10));
          auto label = make_text(key.label, 12, palette_.text);
          label.TextWrapping(TextWrapping::NoWrap);
          content.Children().Append(label);
          auto count = make_text(typing::zh_number(key.count), 12, palette_.sub);
          count.FontWeight(Windows::UI::Text::FontWeight{500});
          content.Children().Append(count);
          chip.Child(content);
          A11y::SetName(chip, hstring(key.label + L"，" + typing::zh_number(key.count) + L" 次"));
          others.Children().Append(chip);
        }
        body.Children().Append(others);
      }
      body.Children().Append(heat_legend());
      StackPanel top;
      top.Spacing(6);
      top.Margin(Thickness{0, 14, 0, 0});
      A11y::SetName(top, L"最常按的键");
      for (size_t index = 0; index < model.top.size(); ++index) {
        auto const &key = model.top[index];
        Grid row;
        row.ColumnSpacing(8);
        ColumnDefinition rank;
        rank.Width(GridLength{16, GridUnitType::Pixel});
        ColumnDefinition name;
        name.Width(GridLength{1, GridUnitType::Star});
        ColumnDefinition count;
        count.Width(GridLength{1, GridUnitType::Auto});
        row.ColumnDefinitions().Append(rank);
        row.ColumnDefinitions().Append(name);
        row.ColumnDefinitions().Append(count);
        auto number = make_text(std::to_wstring(index + 1), 12, palette_.faint);
        number.TextAlignment(TextAlignment::Right);
        row.Children().Append(number);
        auto label = make_text(key.label, 12, palette_.text);
        label.TextWrapping(TextWrapping::NoWrap);
        label.TextTrimming(TextTrimming::CharacterEllipsis);
        Grid::SetColumn(label, 1);
        row.Children().Append(label);
        auto times = make_text(typing::zh_number(key.count) + L" 次", 12, palette_.sub);
        times.FontWeight(Windows::UI::Text::FontWeight{500});
        Grid::SetColumn(times, 2);
        row.Children().Append(times);
        top.Children().Append(row);
      }
      body.Children().Append(top);
    }
    body.Children().Append(stats_footer(
        L"只保存每个键每天被按下的次数，不保存按键顺序和输入内容；按住不放只算一次，密码框中的按键不计入。"));
  }

  // 环形图：扇区从 12 点钟方向顺时针排，中间是总数。几何与共享页的 SVG 相同，按 190 像素画。
  FrameworkElement make_donut(std::vector<typing::Slice> const &slices, double total,
                              std::wstring const &title) {
    constexpr double size = 190;
    constexpr double scale = size / 100;
    Grid holder;
    holder.Width(size);
    holder.Height(size);
    holder.HorizontalAlignment(HorizontalAlignment::Center);
    holder.VerticalAlignment(VerticalAlignment::Center);
    A11y::SetName(holder, hstring(title + L"环形图"));
    Canvas canvas;
    canvas.Width(size);
    canvas.Height(size);
    auto at = [](typing::DonutPoint point) { return point_at(point.x * scale, point.y * scale); };
    for (auto const &segment : typing::donut_segments(slices)) {
      if (segment.ring) {
        Shapes::Ellipse ring;
        const double diameter = (typing::donut_outer - typing::donut_thickness / 2) * 2 * scale;
        ring.Width(diameter);
        ring.Height(diameter);
        ring.Stroke(brush(chart_color(segment.color)));
        ring.StrokeThickness(typing::donut_thickness * scale);
        Canvas::SetLeft(ring, (size - diameter) / 2);
        Canvas::SetTop(ring, (size - diameter) / 2);
        canvas.Children().Append(ring);
        continue;
      }
      PathFigure figure;
      figure.IsClosed(true);
      figure.IsFilled(true);
      figure.StartPoint(at(segment.outer_start));
      ArcSegment outer;
      outer.Point(at(segment.outer_end));
      outer.Size({static_cast<float>(typing::donut_outer * scale),
                  static_cast<float>(typing::donut_outer * scale)});
      outer.SweepDirection(SweepDirection::Clockwise);
      outer.IsLargeArc(segment.large);
      figure.Segments().Append(outer);
      LineSegment across;
      across.Point(at(segment.inner_end));
      figure.Segments().Append(across);
      ArcSegment inner;
      inner.Point(at(segment.inner_start));
      inner.Size({static_cast<float>(typing::donut_inner * scale),
                  static_cast<float>(typing::donut_inner * scale)});
      inner.SweepDirection(SweepDirection::Counterclockwise);
      inner.IsLargeArc(segment.large);
      figure.Segments().Append(inner);
      PathGeometry geometry;
      geometry.Figures().Append(figure);
      Shapes::Path path;
      path.Data(geometry);
      path.Fill(brush(chart_color(segment.color)));
      canvas.Children().Append(path);
    }
    holder.Children().Append(canvas);
    StackPanel center;
    center.HorizontalAlignment(HorizontalAlignment::Center);
    center.VerticalAlignment(VerticalAlignment::Center);
    auto number = make_text(typing::zh_number(total), 25, palette_.text);
    number.FontWeight(Windows::UI::Text::FontWeight{600});
    number.HorizontalAlignment(HorizontalAlignment::Center);
    center.Children().Append(number);
    auto unit = make_text(L"字符", 11, palette_.faint);
    unit.HorizontalAlignment(HorizontalAlignment::Center);
    center.Children().Append(unit);
    holder.Children().Append(center);
    return holder;
  }

  // 「字符类型」「语言模式」：环形图和图例，卡片够宽（520 以上）时并排，窄了环形图回到图例上方。
  void stats_donut(StackPanel const &page, std::wstring const &title,
                   std::vector<typing::Slice> const &slices, std::wstring const &footer) {
    auto body = stats_section(page, title);
    const double total = typing::slices_total(slices);
    if (total == 0) {
      body.Children().Append(stats_empty(L"暂无输入记录"));
    } else {
      auto donut = make_donut(slices, total, title);
      StackPanel legend;
      legend.Spacing(11);
      legend.VerticalAlignment(VerticalAlignment::Center);
      for (auto const &slice : typing::legend_slices(slices)) {
        Grid row;
        row.ColumnSpacing(9);
        for (auto const &width : {GridLength{22, GridUnitType::Pixel}, GridLength{1, GridUnitType::Star},
                                  GridLength{1, GridUnitType::Auto}, GridLength{58, GridUnitType::Pixel}}) {
          ColumnDefinition column;
          column.Width(width);
          row.ColumnDefinitions().Append(column);
        }
        Border dot;
        dot.Width(22);
        dot.Height(22);
        dot.CornerRadius(CornerRadius{7, 7, 7, 7});
        dot.Background(brush(chart_color(slice.color, 0x1a / 255.0)));
        auto symbol = make_text(slice.symbol, 12, chart_color(slice.color));
        symbol.FontWeight(Windows::UI::Text::FontWeight{650});
        symbol.TextWrapping(TextWrapping::NoWrap);
        symbol.HorizontalAlignment(HorizontalAlignment::Center);
        symbol.VerticalAlignment(VerticalAlignment::Center);
        dot.Child(symbol);
        row.Children().Append(dot);
        auto name = make_text(slice.title, 14, palette_.text);
        name.VerticalAlignment(VerticalAlignment::Center);
        Grid::SetColumn(name, 1);
        row.Children().Append(name);
        auto count = make_text(typing::zh_number(slice.count), 14, palette_.text);
        count.FontWeight(Windows::UI::Text::FontWeight{500});
        count.VerticalAlignment(VerticalAlignment::Center);
        Grid::SetColumn(count, 2);
        row.Children().Append(count);
        auto share = make_text(typing::zh_percent(slice.count, total), 12, palette_.sub);
        share.TextAlignment(TextAlignment::Right);
        share.VerticalAlignment(VerticalAlignment::Center);
        Grid::SetColumn(share, 3);
        row.Children().Append(share);
        A11y::SetName(row, hstring(slice.title + L" " + typing::js_number(slice.count) + L" 字符，" +
                                   typing::zh_percent(slice.count, total)));
        legend.Children().Append(row);
      }
      Grid layout;
      layout.Margin(Thickness{0, 16, 0, 0});
      ColumnDefinition chart_column;
      chart_column.Width(GridLength{1, GridUnitType::Auto});
      ColumnDefinition legend_column;
      legend_column.Width(GridLength{1, GridUnitType::Star});
      layout.ColumnDefinitions().Append(chart_column);
      layout.ColumnDefinitions().Append(legend_column);
      RowDefinition first;
      first.Height(GridLength{1, GridUnitType::Auto});
      RowDefinition second;
      second.Height(GridLength{1, GridUnitType::Auto});
      layout.RowDefinitions().Append(first);
      layout.RowDefinitions().Append(second);
      layout.Children().Append(donut);
      layout.Children().Append(legend);
      auto arrange = [](Grid const &grid, FrameworkElement const &chart, FrameworkElement const &list,
                        bool wide) {
        grid.ColumnSpacing(wide ? 32 : 0);
        Grid::SetColumnSpan(chart, wide ? 1 : 2);
        Grid::SetRow(list, wide ? 0 : 1);
        Grid::SetColumn(list, wide ? 1 : 0);
        Grid::SetColumnSpan(list, wide ? 1 : 2);
        list.Margin(Thickness{0, wide ? 0.0 : 20.0, 0, 0});
      };
      arrange(layout, donut, legend, true);
      layout.SizeChanged([arrange, weak_chart = make_weak(donut), weak_list = make_weak(legend)](
                             Inspectable const &sender, SizeChangedEventArgs const &args) {
        auto chart = weak_chart.get();
        auto list = weak_list.get();
        if (chart && list)
          arrange(sender.as<Grid>(), chart, list, args.NewSize().Width >= 520);
      });
      body.Children().Append(layout);
    }
    if (!footer.empty())
      body.Children().Append(stats_footer(footer));
  }

  // 排行的一行：名字、按最多的那一项缩放的横条、数量和占比。
  FrameworkElement rank_row(std::wstring const &label, double count, double total, double peak,
                            Color const &fill, std::wstring const &name) {
    Grid row;
    row.ColumnSpacing(9);
    ColumnDefinition title_column;
    title_column.Width(GridLength{1, GridUnitType::Star});
    title_column.MinWidth(88);
    ColumnDefinition track_column;
    track_column.Width(GridLength{2, GridUnitType::Star});
    track_column.MinWidth(80);
    ColumnDefinition count_column;
    count_column.Width(GridLength{72, GridUnitType::Pixel});
    ColumnDefinition share_column;
    share_column.Width(GridLength{48, GridUnitType::Pixel});
    row.ColumnDefinitions().Append(title_column);
    row.ColumnDefinitions().Append(track_column);
    row.ColumnDefinitions().Append(count_column);
    row.ColumnDefinitions().Append(share_column);
    auto title = make_text(label, 12, palette_.text);
    title.TextWrapping(TextWrapping::NoWrap);
    title.TextTrimming(TextTrimming::CharacterEllipsis);
    title.VerticalAlignment(VerticalAlignment::Center);
    row.Children().Append(title);
    Grid track;
    track.Height(11);
    track.CornerRadius(CornerRadius{5.5, 5.5, 5.5, 5.5});
    track.Background(brush(palette_.subtle2));
    track.VerticalAlignment(VerticalAlignment::Center);
    const double fraction = std::clamp(count / peak, 0.0, 1.0);
    ColumnDefinition filled;
    filled.Width(GridLength{fraction, GridUnitType::Star});
    ColumnDefinition rest;
    rest.Width(GridLength{1 - fraction, GridUnitType::Star});
    track.ColumnDefinitions().Append(filled);
    track.ColumnDefinitions().Append(rest);
    Border bar;
    bar.CornerRadius(CornerRadius{5.5, 5.5, 5.5, 5.5});
    bar.Background(brush(fill));
    track.Children().Append(bar);
    Grid::SetColumn(track, 1);
    row.Children().Append(track);
    auto number = make_text(typing::zh_number(count), 12, palette_.sub);
    number.TextAlignment(TextAlignment::Right);
    number.VerticalAlignment(VerticalAlignment::Center);
    Grid::SetColumn(number, 2);
    row.Children().Append(number);
    auto share = make_text(typing::zh_percent(count, total), 12, palette_.faint);
    share.TextAlignment(TextAlignment::Right);
    share.VerticalAlignment(VerticalAlignment::Center);
    Grid::SetColumn(share, 3);
    row.Children().Append(share);
    A11y::SetName(row, hstring(name));
    return row;
  }

  // 「输入方案」：方案可能多到颜色分不清，所以是单色排行，名字说明是哪一项。
  void stats_rank(StackPanel const &page, std::wstring const &title,
                  std::vector<typing::Slice> const &slices, std::wstring const &footer) {
    auto body = stats_section(page, title);
    const double total = typing::slices_total(slices);
    if (total == 0) {
      body.Children().Append(stats_empty(L"暂无输入记录"));
    } else {
      const auto ranked = typing::ranked_slices(slices);
      double peak = 1;
      for (auto const &slice : ranked)
        peak = std::max(peak, slice.count);
      StackPanel chart;
      chart.Spacing(11);
      chart.Margin(Thickness{0, 16, 0, 18});
      A11y::SetName(chart, hstring(title + L"排行"));
      for (auto const &slice : ranked)
        chart.Children().Append(rank_row(slice.title, slice.count, total, peak, palette_.accent,
                                         slice.title + L" " + typing::js_number(slice.count) +
                                             L" 字符，" + typing::zh_percent(slice.count, total)));
      body.Children().Append(chart);
    }
    if (!footer.empty())
      body.Children().Append(stats_footer(footer));
  }

  // 「候选命中位置」：按位置排，不按多少排，看的是从第一条往后掉得多快。
  void stats_candidate_ranks(StackPanel const &page, typing::Statistics const &statistics) {
    const auto ranks = typing::candidate_ranks(statistics);
    auto body = stats_section(page, L"候选命中位置");
    Grid head;
    head.Margin(Thickness{0, 0, 0, 4});
    head.ColumnSpacing(8);
    for (auto const &width : {GridLength{1, GridUnitType::Auto}, GridLength{1, GridUnitType::Auto},
                              GridLength{1, GridUnitType::Star}}) {
      ColumnDefinition column;
      column.Width(width);
      head.ColumnDefinitions().Append(column);
    }
    auto rate = make_text(typing::zh_percent(ranks.first, ranks.total), 30, palette_.text);
    rate.TextWrapping(TextWrapping::NoWrap);
    A11y::SetName(rate, hstring(L"首选命中率 " + typing::zh_percent(ranks.first, ranks.total)));
    head.Children().Append(rate);
    auto caption = make_text(L"首选命中率", 13, palette_.sub);
    caption.VerticalAlignment(VerticalAlignment::Bottom);
    caption.Margin(Thickness{0, 0, 0, 6});
    A11y::SetAccessibilityView(caption, Microsoft::UI::Xaml::Automation::Peers::AccessibilityView::Raw);
    Grid::SetColumn(caption, 1);
    head.Children().Append(caption);
    auto summary = make_text(ranks.total == 0 ? std::wstring(L"暂无记录")
                                              : L"共 " + typing::zh_number(ranks.total) + L" 次上屏",
                             12, palette_.faint);
    summary.TextAlignment(TextAlignment::Right);
    summary.VerticalAlignment(VerticalAlignment::Bottom);
    summary.Margin(Thickness{0, 0, 0, 6});
    Grid::SetColumn(summary, 2);
    head.Children().Append(summary);
    body.Children().Append(head);
    if (ranks.total == 0) {
      body.Children().Append(stats_empty(L"暂无候选记录。用水杉键盘上屏几次后再回来查看。"));
    } else {
      StackPanel chart;
      chart.Spacing(11);
      chart.Margin(Thickness{0, 16, 0, 18});
      A11y::SetName(chart, L"候选命中位置分布");
      for (auto const &row : ranks.rows)
        chart.Children().Append(rank_row(row.label, row.count, ranks.total, ranks.peak,
                                         chart_color(typing::chart_palette[0]),
                                         row.label + L"：" + typing::js_number(row.count) + L" 次，" +
                                             typing::zh_percent(row.count, ranks.total)));
      body.Children().Append(chart);
    }
    body.Children().Append(stats_footer(
        L"每次上屏记录选中的是第几条候选，只记位置，不记任何文字。首选命中率越高，说明排序越贴合你的输入。"));
  }

  // 「按日明细」：最近 30 个有记录的日子，新的在上。
  void stats_details(StackPanel const &page, typing::Statistics const &statistics,
                     std::string const &today) {
    const auto rows = typing::daily_detail_rows(statistics, today);
    auto body = stats_section(page, L"按日明细 · 最近 " + std::to_wstring(typing::detail_days) + L" 天");
    if (rows.empty()) {
      body.Children().Append(stats_empty(L"暂无输入记录"));
    } else {
      constexpr std::array<const wchar_t *, 9> titles{L"日期", L"字数", L"汉字", L"字母", L"数字",
                                                      L"标点", L"其他", L"活跃", L"速度"};
      Grid table;
      table.MinWidth(560);
      for (size_t index = 0; index < titles.size(); ++index) {
        ColumnDefinition column;
        column.Width(GridLength{index == 8 ? 1.6 : 1, GridUnitType::Star});
        table.ColumnDefinitions().Append(column);
      }
      auto place = [&table](FrameworkElement const &element, size_t row, size_t column) {
        Grid::SetRow(element, static_cast<int32_t>(row));
        Grid::SetColumn(element, static_cast<int32_t>(column));
        table.Children().Append(element);
      };
      auto cell = [this](std::wstring const &value, Color const &color, size_t column, bool header) {
        auto block = make_text(value, 12, color);
        block.TextWrapping(TextWrapping::NoWrap);
        block.TextAlignment(column == 0 ? TextAlignment::Left : TextAlignment::Right);
        block.Padding(header ? Thickness{8, 0, 8, 6} : Thickness{8, 6, 8, 6});
        if (header)
          block.FontWeight(Windows::UI::Text::FontWeight{500});
        return block;
      };
      for (size_t row = 0; row <= rows.size(); ++row) {
        RowDefinition definition;
        definition.Height(GridLength{1, GridUnitType::Auto});
        table.RowDefinitions().Append(definition);
      }
      for (size_t column = 0; column < titles.size(); ++column)
        place(cell(titles[column], palette_.faint, column, true), 0, column);
      for (size_t index = 0; index < rows.size(); ++index) {
        auto const &row = rows[index];
        Border line;
        line.BorderThickness(Thickness{0, 1, 0, 0});
        line.BorderBrush(brush(palette_.divider));
        Grid::SetColumnSpan(line, static_cast<int32_t>(titles.size()));
        place(line, index + 1, 0);
        place(cell(typing::day_label(row.key), palette_.text, 0, false), index + 1, 0);
        auto total = cell(typing::zh_number(row.total), palette_.text, 1, false);
        total.FontWeight(Windows::UI::Text::FontWeight{500});
        place(total, index + 1, 1);
        const std::array<double, 5> values{row.han, row.latin, row.number, row.punctuation, row.other};
        for (size_t column = 0; column < values.size(); ++column)
          place(cell(typing::zh_number(values[column]), palette_.sub, column + 2, false), index + 1,
                column + 2);
        place(cell(row.active_ms ? typing::active_time(*row.active_ms) : std::wstring(L"—"),
                   palette_.text, 7, false),
              index + 1, 7);
        place(cell(row.speed ? typing::zh_number(typing::js_round(*row.speed)) + L" 字/分钟"
                             : std::wstring(L"—"),
                   palette_.text, 8, false),
              index + 1, 8);
      }
      A11y::SetName(table, hstring(L"按日明细 · 最近 " + std::to_wstring(typing::detail_days) + L" 天"));
      ScrollViewer scroller;
      scroller.HorizontalScrollBarVisibility(ScrollBarVisibility::Auto);
      scroller.VerticalScrollBarVisibility(ScrollBarVisibility::Disabled);
      scroller.HorizontalScrollMode(ScrollMode::Enabled);
      scroller.VerticalScrollMode(ScrollMode::Disabled);
      scroller.Margin(Thickness{0, 12, 0, 0});
      scroller.Content(table);
      keep_horizontal_offset(scroller, stats_details_offset_);
      body.Children().Append(scroller);
    }
    body.Children().Append(stats_footer(
        L"列出最近 " + std::to_wstring(typing::detail_days) +
        L" 个有记录的日期，新的在上；「其他」含其他文字、表情、符号与历史未分类。活跃与速度显示「—」的日期早于活跃时长的记录。"));
  }

  void build_stats_page(StackPanel const &page) {
    if (indexing_) {
      // 统计要从工作线程读回来才画得出，搜索索引里先登记页面底部的两项设置。
      record(L"记录打字统计", L"关闭后，新提交不会增加统计。");
      record(L"上传到云端", std::wstring(typing::upload_description(true, false)));
      record(L"自动清理", L"按保留策略删除超期的每日记录并从累计中扣除，跨天后首次记录时执行。");
      return;
    }
    start_statistics_timer();
    if (!stats_ && stats_request_ == 0 && stats_error_.empty())
      poll_statistics();
    if (!stats_) {
      if (stats_error_.empty())
        page.Children().Append(make_text(L"正在读取打字统计…", 14, palette_.sub));
      else
        page.Children().Append(stats_error_bar());
      return;
    }
    // 引用而不是复制：整份统计连同回答原文可能有几兆，构建页面时不会改动它（请求的回答在之后的调度里才到）。
    auto const &status = *stats_;
    auto const &statistics = status.statistics;
    const auto today_date = local_civil_day();
    const auto today = typing::day_key(today_date);
    const auto trend_days = typing::recent_days(typing::desktop_trend_days, today_date);
    std::optional<std::vector<std::string>> scope;
    if (stats_selected_)
      scope = std::vector<std::string>{*stats_selected_};
    const auto breakdown = typing::scoped_breakdown(statistics, scope);
    const double scope_total =
        stats_selected_ ? typing::value_of(statistics.days, *stats_selected_) : statistics.total;
    const auto selected_label =
        stats_selected_ ? std::optional<std::wstring>(typing::day_label(*stats_selected_)) : std::nullopt;
    const auto activity = typing::activity_metrics(statistics, today);

    if (!stats_error_.empty())
      page.Children().Append(stats_error_bar());
    if (!statistics.enabled) {
      auto body = stats_section(page, L"输入统计已关闭");
      body.Children().Append(
          make_text(typing::disabled_notice(stats_upload_ && stats_upload_->enabled), 14, palette_.sub));
      auto enable = button_control(stats_busy_ ? L"处理中…" : L"启用输入统计",
                                   [this] { set_statistics_enabled(true); }, !stats_busy_);
      enable.Margin(Thickness{0, 12, 0, 0});
      enable.HorizontalAlignment(HorizontalAlignment::Left);
      body.Children().Append(enable);
    }
    stats_overview(page, statistics, today, selected_label.value_or(L"累计输入"), scope_total, activity);

    auto tabs = segmented_control(
        L"统计内容",
        {{L"keys", L"按键"}, {L"trend", L"趋势"}, {L"kind", L"类型"}, {L"mode", L"模式"},
         {L"scheme", L"方案"}, {L"ranks", L"候选"}},
        stats_tab_,
        [this](std::wstring const &value) {
          // 桌面上切换标签时保留选中的那一天：它的柱子在「趋势」里，其他标签正是用来看这一天的分项。
          stats_tab_ = value;
          redraw_stats();
        },
        true, false, 6, L"statistics-tab");
    tabs.HorizontalAlignment(HorizontalAlignment::Left);
    tabs.Margin(Thickness{0, 24, 0, 0});
    page.Children().Append(tabs);

    if (stats_tab_ == L"trend") {
      stats_trend(page, statistics, trend_days);
      if (activity.today_hours)
        stats_hours(page, *activity.today_hours, typing::usual_hours(statistics, today));
      stats_speed(page, statistics, trend_days, activity.average_speed);
      stats_calendar(page, statistics, today_date);
    }
    if (stats_tab_ == L"keys")
      stats_keys(page, statistics, scope, selected_label.value_or(L"累计"));
    if (stats_tab_ == L"kind")
      stats_donut(page, L"字符类型", typing::character_slices(breakdown), L"");
    if (stats_tab_ == L"mode")
      stats_donut(page, L"语言模式", typing::language_slices(breakdown),
                  L"按提交时使用的键盘模式统计，不推测文本语言；中文模式下输入的数字仍计入中文模式。AI 润色和语音输入单独按来源统计。");
    if (stats_tab_ == L"scheme")
      stats_rank(page, L"输入方案", typing::source_slices(breakdown),
                 L"输入方案统计其上屏字符数；拼音等按键另由按键热力图计数，只记每个键每天的按下次数。来源无法归类的字数计入历史未分类。");
    if (stats_tab_ == L"trend")
      stats_details(page, statistics, today);
    if (stats_tab_ == L"ranks")
      stats_candidate_ranks(page, statistics);

    auto group = add_group(page, L"");
    group.Margin(Thickness{0, 28, 0, 0});
    add_row(group, 0xE9D2, L"记录打字统计", L"关闭后，新提交不会增加统计。",
            toggle_control(L"记录打字统计", statistics.enabled,
                           [this](bool on) { set_statistics_enabled(on); }, !stats_busy_, false));
    {
      // 「上传到云端」：没读到状态时不可点；已经打开的开关在退出登录后仍能关掉。
      const bool upload_on = stats_upload_ && stats_upload_->enabled;
      const bool signed_in = stats_upload_ && stats_upload_->signed_in;
      StackPanel below;
      below.Spacing(4);
      if (upload_on && stats_upload_->last_upload_ms)
        below.Children().Append(make_text(
            L"上次上传：" + statistics_written_text(*stats_upload_->last_upload_ms), 12, palette_.sub));
      if (stats_upload_busy_)
        below.Children().Append(make_text(L"处理中…", 12, palette_.sub));
      if (!stats_upload_message_.empty())
        below.Children().Append(make_text(stats_upload_message_, 12, palette_.text));
      FrameworkElement below_row{nullptr};
      if (below.Children().Size() > 0)
        below_row = below;
      add_row(group, 0xE753, L"上传到云端", std::wstring(typing::upload_description(signed_in, upload_on)),
              toggle_control(L"上传到云端", upload_on, [this](bool on) { set_typing_upload(on); },
                             !stats_upload_busy_ && stats_upload_ && (signed_in || upload_on), false),
              below_row);
    }
    std::vector<Option> retention;
    for (auto const &choice : typing::retention_choices)
      retention.push_back({typing::widen(choice.value), choice.label});
    const auto current = typing::widen(typing::retention_value(statistics.retention));
    add_row(group, 0xE823, L"自动清理",
            L"按保留策略删除超期的每日记录并从累计中扣除，跨天后首次记录时执行。",
            select_control(L"自动清理", std::move(retention), current,
                           [this, current](std::wstring const &value) {
                             if (value != current)
                               set_statistics_retention(value);
                           },
                           !stats_busy_, false));
    StackPanel actions;
    actions.Padding(Thickness{0, 12, 0, 12});
    StackPanel buttons;
    buttons.Orientation(Orientation::Horizontal);
    buttons.Spacing(9);
    auto clear = button_control(L"清空统计", [this] { reset_statistics(); }, !stats_busy_);
    clear.as<Button>().Foreground(brush(danger_color()));
    buttons.Children().Append(clear);
    actions.Children().Append(buttons);
    auto privacy = make_text(
        L"字数统计水杉键盘提交的字符，以及英文模式和放行给应用的字母、数字与符号（按按键时估计），含标点及表情，不含空格和换行。组合表情计为一个字符，删除文字不扣减。按键热力图只保存每个键每天被按下的次数，不保存按键顺序和输入内容。" +
            std::wstring(stats_upload_ && stats_upload_->enabled
                             ? L"日期、分类和数量保存在本机，不保存输入内容；已打开「上传到云端」，最近 7 天的每日计数和候选位次会上传到你的水杉账号，按键热力图不上传。"
                             : L"仅在本机保存日期、分类和数量，不保存输入内容。") +
            L"每日明细默认永久保留，可在「自动清理」中改为只保留最近一段时间；清理删除的日期同时从累计总数与分类中扣除。",
        12, palette_.sub);
    privacy.Margin(Thickness{0, 16, 0, 0});
    actions.Children().Append(privacy);
    Border actions_row;
    actions_row.BorderBrush(brush(palette_.divider));
    actions_row.BorderThickness(Thickness{0, 1, 0, 0});
    actions_row.Child(actions);
    group.Children().Append(actions_row);

    const auto message = typing::availability_notice(
        statistics.enabled, statistics.total, status.never_written,
        status.last_written_ms ? statistics_written_text(*status.last_written_ms) : std::wstring());
    if (!message.empty()) {
      auto body = stats_section(page, L"统计没有数据");
      body.Children().Append(make_text(message, 14, palette_.sub));
    }
  }

  // ---- 插件 ----

  // 共享的「插件」页（packages/ui/src/settings/pages/plugins-page.tsx 和 plugins-section.tsx）在 Windows 上的「我的插件」：列表（plugin-list-view.tsx）先是「声音与效果」和「@ 名单」两个入口，再是按八类分组的已安装插件和缺失的选择、「脚本插件（实验）」，然后是「导入插件」和「无法载入的插件」；点一行在原处打开它的视图（plugin-sound-effects-view.tsx、plugin-mentions-view.tsx、plugin-detail-view.tsx、plugin-script-view.tsx），视图头上的「我的插件」回到列表。Windows 宿主的能力全为真（播放音效和音乐、路由 / 与 @、画特效样式和特效包、背单词列单词本、符号面板列符号集），辅助码表随版本（与「输入」页的辅助码组同一个条件）。插件目录、导入、删除和名单都经 msime_client_plugins，与 Tauri 的 desktop_plugins.rs 同一份 client-services 规则；内置声音包取 Server 状态目录下的 sound-packs。规则和文案在 PluginPageModel.h。所有读写都在工作线程上。页首的「社区插件」标签见「社区皮肤与社区插件」一节。

  enum class PluginView { list, pack, missing, sound_effects, mentions };
  enum class PluginCatalogState { loading, loaded, failed };

  // 授权对话框（共享页的 ScriptGrantDialog）的状态。generation 在打开和关上时递增，作废在途的回答；id 是对话框授权的那个包，每次绘制时到目录里找，包被换过、页面重读之后对话框跟着换成新版本；granted 是勾着的能力；opener 是关上后焦点要回到的控件（启用开关或「调整授权」）；boxes 是现在画着的复选框，勾选时就地更新，不重建对话框，焦点留在刚点的那一项上。
  struct ScriptGrantDialog {
    struct Box {
      std::wstring capability;
      weak_ref<CheckBox> box;
      weak_ref<TextBlock> description;
    };
    uint64_t generation = 0;
    std::wstring id;
    std::vector<std::wstring> granted;
    bool busy = false;
    std::wstring error;
    std::wstring opener;
    std::vector<Box> boxes;
    std::function<void()> refresh;
    weak_ref<ContentDialog> dialog;
  };

  void redraw_plugins() {
    if (!closed_ && current_page_ == "plugins")
      queue_refresh();
  }

  // 每次进入页面都与共享页重新激活一样：回到列表，作废上一次留下的请求，下一次绘制时重读目录和名单。没存的名单改动留着。进入时读一次打字统计开没开（共享页的 statisticsEnabled），用来决定「成就音效」能不能打开；读不了时按打开处理。
  void enter_plugins_page() {
    ++plugins_generation_;
    plugins_busy_ = false;
    plugins_stale_ = true;
    plugin_view_ = PluginView::list;
    plugin_pending_focus_.clear();
    plugin_statistics_enabled_.reset();
    const auto generation = plugins_generation_;
    statistics_host(statistics_action(L"load"),
                    [this, generation](std::optional<StatisticsStatus> const &status) {
                      if (generation != plugins_generation_ || !status)
                        return;
                      plugin_statistics_enabled_ = status->statistics.enabled;
                      if (plugin_view_ == PluginView::sound_effects)
                        redraw_plugins();
                    });
  }

  static bool helpcode_offered() {
    return edition_offers_scheme(L"quanpin") || edition_offers_scheme(L"shuangpin");
  }

  // 文档里的 plugins，每一项缺省时取出厂值（共享页的 pluginPreferences）。
  pg::Preferences plugin_preferences() const {
    pg::Preferences value;
    value.key_sound_enabled = document_.Boolean(L"plugins.key_sound.enabled", value.key_sound_enabled);
    value.key_sound_mode = document_.String(L"plugins.key_sound.mode", value.key_sound_mode);
    value.key_sound_pack = document_.String(L"plugins.key_sound.pack", value.key_sound_pack);
    value.key_sound_volume = document_.Number(L"plugins.key_sound.volume", value.key_sound_volume);
    value.commit_sound_enabled =
        document_.Boolean(L"plugins.commit_sound.enabled", value.commit_sound_enabled);
    value.melody_pack = document_.String(L"plugins.melody.pack", value.melody_pack);
    value.music_enabled = document_.Boolean(L"plugins.music.enabled", value.music_enabled);
    value.music_pack = document_.String(L"plugins.music.pack", value.music_pack);
    value.music_volume = document_.Number(L"plugins.music.volume", value.music_volume);
    value.achievements_enabled =
        document_.Boolean(L"plugins.achievements.enabled", value.achievements_enabled);
    value.command_tables = document_.Strings(L"plugins.command_tables");
    value.effect_style = document_.String(L"plugins.effect_style", value.effect_style);
    value.effect_intensity = document_.Number(L"plugins.effect_intensity", value.effect_intensity);
    value.effect_pack = document_.String(L"plugins.effect_pack", value.effect_pack);
    value.combo_counter = document_.Boolean(L"plugins.combo_counter", value.combo_counter);
    value.combo_tier_sound = document_.Boolean(L"plugins.combo_tier_sound", value.combo_tier_sound);
    value.phrase_tables = document_.Strings(L"plugins.phrase_tables");
    value.helpcode_pack_quanpin =
        document_.String(L"plugins.helpcode_pack_quanpin", value.helpcode_pack_quanpin);
    value.helpcode_pack_shuangpin =
        document_.String(L"plugins.helpcode_pack_shuangpin", value.helpcode_pack_shuangpin);
    return value;
  }

  // 只写变了的项；两张表的顺序是用户的，原样写入。
  static void write_plugin_preferences(PreferencesDocument &doc, pg::Preferences const &before,
                                       pg::Preferences const &after) {
    auto flag = [&doc](wchar_t const *key, bool from, bool to) {
      if (from != to)
        doc.SetBoolean(key, to);
    };
    auto word = [&doc](wchar_t const *key, std::wstring const &from, std::wstring const &to) {
      if (from != to)
        doc.SetString(key, to);
    };
    auto number = [&doc](wchar_t const *key, double from, double to) {
      if (from != to)
        doc.SetNumber(key, to);
    };
    auto list = [&doc](wchar_t const *key, std::vector<std::wstring> const &from,
                       std::vector<std::wstring> const &to) {
      if (from != to)
        doc.SetOrderedStrings(key, to);
    };
    flag(L"plugins.key_sound.enabled", before.key_sound_enabled, after.key_sound_enabled);
    word(L"plugins.key_sound.mode", before.key_sound_mode, after.key_sound_mode);
    word(L"plugins.key_sound.pack", before.key_sound_pack, after.key_sound_pack);
    number(L"plugins.key_sound.volume", before.key_sound_volume, after.key_sound_volume);
    flag(L"plugins.commit_sound.enabled", before.commit_sound_enabled, after.commit_sound_enabled);
    word(L"plugins.melody.pack", before.melody_pack, after.melody_pack);
    flag(L"plugins.music.enabled", before.music_enabled, after.music_enabled);
    word(L"plugins.music.pack", before.music_pack, after.music_pack);
    number(L"plugins.music.volume", before.music_volume, after.music_volume);
    flag(L"plugins.achievements.enabled", before.achievements_enabled, after.achievements_enabled);
    list(L"plugins.command_tables", before.command_tables, after.command_tables);
    word(L"plugins.effect_style", before.effect_style, after.effect_style);
    number(L"plugins.effect_intensity", before.effect_intensity, after.effect_intensity);
    word(L"plugins.effect_pack", before.effect_pack, after.effect_pack);
    flag(L"plugins.combo_counter", before.combo_counter, after.combo_counter);
    flag(L"plugins.combo_tier_sound", before.combo_tier_sound, after.combo_tier_sound);
    list(L"plugins.phrase_tables", before.phrase_tables, after.phrase_tables);
    word(L"plugins.helpcode_pack_quanpin", before.helpcode_pack_quanpin, after.helpcode_pack_quanpin);
    word(L"plugins.helpcode_pack_shuangpin", before.helpcode_pack_shuangpin,
         after.helpcode_pack_shuangpin);
  }

  // 共享页的 onChange：存下新的插件设置并重画。
  void change_plugins(pg::Preferences const &next) {
    const auto before = plugin_preferences();
    if (next == before)
      return;
    change([&](PreferencesDocument &doc) { write_plugin_preferences(doc, before, next); }, true);
  }

  // 指令表详情和 @ 名单里的「打开」按钮就地打开「输入 → 快捷模式」里的开关，写的是同一个键。
  void enable_local_mode(wchar_t const *mode) {
    change([mode](PreferencesDocument &doc) {
      doc.SetBoolean(std::wstring(L"local_modes.") + mode, true);
    }, true);
  }

  pg::TableModes plugin_table_modes() const {
    return {document_.Boolean(L"local_modes.quick_phrase", true),
            document_.Boolean(L"local_modes.command", false),
            pg::scheme_opens_table_modes(document_.String(L"scheme", MSIME_EDITION_DEFAULT_SCHEME_W))};
  }

  bool plugin_mentions_dirty() const { return plugin_mentions_ != plugin_saved_mentions_; }

  // 进入页面后读一次目录和名单。读到的目录一直留着，重读失败时不换掉；第一次就失败时列表说没读成功。
  void load_plugins() {
    plugins_stale_ = false;
    const auto generation = plugins_generation_;
    if (plugin_catalog_state_ != PluginCatalogState::loaded)
      plugin_catalog_state_ = PluginCatalogState::loading;
    run_host([] { return plugins_call(plugin_action(L"catalog"), true, true); },
             [this, generation](Response const &response) {
               if (generation != plugins_generation_)
                 return;
               if (auto catalog = plugin_catalog(response)) {
                 plugin_catalog_ = std::move(*catalog);
                 plugin_catalog_state_ = PluginCatalogState::loaded;
                 redraw_plugins();
                 return;
               }
               if (plugin_catalog_state_ != PluginCatalogState::loaded)
                 plugin_catalog_state_ = PluginCatalogState::failed;
               show_notice(plugin_failure(response, L"无法读取插件列表，请重试。"));
             });
    load_script_grants(generation);
    run_host([] { return plugins_call(plugin_action(L"load_mentions"), false); },
             [this, generation](Response const &response) {
               if (generation != plugins_generation_)
                 return;
               auto mentions = plugin_mentions(response);
               if (!mentions) {
                 show_notice(plugin_failure(response, L"无法读取 @ 名单，请重试。"));
                 return;
               }
               // 没存的改动不被重读覆盖。
               if (!plugin_mentions_dirty())
                 plugin_mentions_ = *mentions;
               plugin_saved_mentions_ = std::move(*mentions);
               redraw_plugins();
             });
  }

  // 导入或删除已经成功之后重读目录：重读失败按重读失败报，不说导入或删除失败。导入或删除脚本包时宿主在同一把锁里改了授权条目，授权也一起重读。
  void refresh_plugin_catalog(uint64_t generation, std::function<void()> then) {
    run_host([] { return plugins_call(plugin_action(L"catalog"), true, true); },
             [this, generation, then = std::move(then)](Response const &response) mutable {
               if (generation != plugins_generation_)
                 return;
               if (auto catalog = plugin_catalog(response)) {
                 plugin_catalog_ = std::move(*catalog);
                 plugin_catalog_state_ = PluginCatalogState::loaded;
               } else {
                 show_notice(plugin_failure(response, L"无法读取插件列表，请重试。"));
               }
               load_script_grants(generation, std::move(then));
             });
  }

  // 读一次脚本插件的授权（共享页的 scriptGrants）。读不到时留着上一次读到的，并说没读成功。then 在之后调用，没有时重画页面。
  void load_script_grants(uint64_t generation, std::function<void()> then = {}) {
    run_host([] { return plugins_call(plugin_action(L"script_grants"), false, true); },
             [this, generation, then = std::move(then)](Response const &response) {
               if (generation != plugins_generation_)
                 return;
               if (auto grants = plugin_script_grants(response))
                 plugin_scripts_ = std::move(*grants);
               else
                 show_notice(plugin_failure(response, L"无法读取脚本插件的授权，请重试。"));
               if (then)
                 then();
               else
                 redraw_plugins();
             });
  }

  // 共享页的 runPluginAction：同一时间只有一个；开始时清掉页面上的出错说明和导入提示。
  bool begin_plugin_action() {
    if (plugins_busy_)
      return false;
    plugins_busy_ = true;
    notice_.clear();
    plugins_notice_.clear();
    return true;
  }

  void end_plugin_action(uint64_t generation) {
    if (generation != plugins_generation_)
      return;
    plugins_busy_ = false;
    redraw_plugins();
  }

  // 导入用的系统对话框，与 Tauri 一样分成文件夹和 .zip 两种：系统的对话框只能选文件或只能选文件夹。用户取消或对话框打不开时返回空。
  std::optional<std::filesystem::path> pick_plugin_source(bool folder) {
    com_ptr<IFileDialog> dialog;
    if (FAILED(CoCreateInstance(CLSID_FileOpenDialog, nullptr, CLSCTX_INPROC_SERVER,
                                IID_PPV_ARGS(dialog.put()))))
      return std::nullopt;
    FILEOPENDIALOGOPTIONS options = 0;
    dialog->GetOptions(&options);
    if (folder) {
      dialog->SetTitle(L"选择插件文件夹");
      dialog->SetOptions(options | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST);
    } else {
      const COMDLG_FILTERSPEC filter{L"插件", L"*.zip"};
      dialog->SetFileTypes(1, &filter);
      dialog->SetTitle(L"选择插件");
      dialog->SetOptions(options | FOS_FORCEFILESYSTEM | FOS_FILEMUSTEXIST);
    }
    // 点击按钮时本窗口就是这个线程的活动窗口，对话框以它为所有者，模态地挡住设置窗口。
    if (dialog->Show(GetActiveWindow()) != S_OK)
      return std::nullopt;
    com_ptr<IShellItem> item;
    if (FAILED(dialog->GetResult(item.put())))
      return std::nullopt;
    PWSTR raw = nullptr;
    if (FAILED(item->GetDisplayName(SIGDN_FILESYSPATH, &raw)) || !raw)
      return std::nullopt;
    std::filesystem::path chosen(raw);
    CoTaskMemFree(raw);
    return chosen;
  }

  void import_plugin_pack(bool folder) {
    if (!begin_plugin_action())
      return;
    const auto generation = plugins_generation_;
    const auto source = pick_plugin_source(folder);
    if (!source) {
      end_plugin_action(generation);
      return;
    }
    run_host(
        [source = source->wstring()] {
          auto action = plugin_action(L"import");
          action.SetNamedValue(L"source", JsonValue::CreateStringValue(hstring(source)));
          return plugins_call(action, false, true);
        },
        [this, generation](Response const &response) {
          if (generation != plugins_generation_)
            return;
          if (!response.ok) {
            show_notice(plugin_failure(response, L"导入失败，请重试。"));
            end_plugin_action(generation);
            return;
          }
          try {
            const auto value = JsonObject::Parse(text(response.text)).GetNamedValue(L"value");
            if (value.ValueType() == JsonValueType::Object)
              plugins_notice_ = pg::imported_notice(plugin_package(value.GetObject()));
          } catch (...) {
          }
          refresh_plugin_catalog(generation, [this, generation] { end_plugin_action(generation); });
        });
    redraw_plugins();
  }

  fire_and_forget remove_plugin_pack(pg::Package pack) {
    if (plugins_busy_)
      co_return;
    auto strong = get_strong();
    if (!co_await confirm(L"删除" + pg::pack_kind_label(pack),
                          L"删除「" + pack.name + L"」？包里的文件会从本机移除。", L"删除"))
      co_return;
    if (!begin_plugin_action())
      co_return;
    const auto generation = plugins_generation_;
    run_host(
        [kind = pack.kind, id = pack.id] {
          auto action = plugin_action(L"remove");
          action.SetNamedValue(L"kind", JsonValue::CreateStringValue(hstring(kind)));
          action.SetNamedValue(L"id", JsonValue::CreateStringValue(hstring(id)));
          return plugins_call(action, false);
        },
        [this, generation, kind = pack.kind, id = pack.id](Response const &response) {
          if (generation != plugins_generation_)
            return;
          if (!response.ok) {
            show_notice(plugin_failure(response, L"删除失败，请重试。"));
            end_plugin_action(generation);
            return;
          }
          change_plugins(pg::without_removed_pack(plugin_preferences(), kind, id));
          refresh_plugin_catalog(generation, [this, generation] {
            if (generation != plugins_generation_)
              return;
            // 详情是从这个包的那一行打开的，那一行随包一起没了：焦点落到同一类的第一行。
            plugin_pending_focus_ = plugin_opened_from_;
            plugin_view_ = PluginView::list;
            end_plugin_action(generation);
          });
        });
    redraw_plugins();
  }

  void save_plugin_mentions() {
    if (pg::mention_list_issue(plugin_mentions_) || !begin_plugin_action())
      return;
    const auto generation = plugins_generation_;
    auto trimmed = pg::trimmed_mentions(plugin_mentions_);
    run_host(
        [trimmed] {
          JsonArray entries;
          for (auto const &entry : trimmed) {
            JsonObject item;
            item.SetNamedValue(L"text", JsonValue::CreateStringValue(hstring(entry.text)));
            item.SetNamedValue(L"key", JsonValue::CreateStringValue(hstring(entry.key)));
            entries.Append(item);
          }
          auto action = plugin_action(L"save_mentions");
          action.SetNamedValue(L"entries", entries);
          return plugins_call(action, false);
        },
        [this, generation, trimmed](Response const &response) {
          if (generation != plugins_generation_)
            return;
          if (!response.ok) {
            show_notice(plugin_failure(response, L"名单未能保存，请重试。"));
          } else {
            plugin_mentions_ = trimmed;
            plugin_saved_mentions_ = trimmed;
          }
          end_plugin_action(generation);
        });
    redraw_plugins();
  }

  // 单词本详情的「去背单词」：先在背单词里选中这本书（其余设置照旧），再打开「词库」页上的背单词。
  void open_plugin_wordbook(std::wstring const &book) {
    if (vocabulary_busy_)
      return;
    vocabulary_busy_ = true;
    run_host(
        [load = vocabulary_body(vocabulary_action(L"load")), book] {
          auto loaded = vocabulary_call(load);
          if (!loaded.ok)
            return loaded;
          try {
            const auto settings = JsonObject::Parse(text(loaded.text))
                                      .GetNamedObject(L"value")
                                      .GetNamedObject(L"settings");
            auto action = vocabulary_action(L"set_settings");
            action.SetNamedValue(L"wordbook", JsonValue::CreateStringValue(hstring(book)));
            action.SetNamedValue(L"new_per_day",
                                 JsonValue::CreateNumberValue(settings.GetNamedNumber(L"newPerDay")));
            action.SetNamedValue(L"session_limit",
                                 JsonValue::CreateNumberValue(settings.GetNamedNumber(L"sessionLimit")));
            return vocabulary_call(vocabulary_body(action));
          } catch (...) {
            return Response{"{\"ok\":false,\"error\":\"invalid\"}", false};
          }
        },
        [this](Response const &response) {
          vocabulary_busy_ = false;
          if (!response.ok) {
            show_notice(L"没能打开背单词，请重试。");
            return;
          }
          apply_vocabulary_response(response);
          focus_row_ = L"词书";
          navigate("lexicon", true);
        });
  }

  void open_plugin_view(PluginView view, std::wstring const &kind, std::wstring const &id,
                        std::wstring const &row_key) {
    plugin_view_ = view;
    plugin_view_kind_ = kind;
    plugin_view_id_ = id;
    plugin_opened_from_ = row_key;
    plugin_pending_focus_ = L"back";
    queue_refresh();
  }

  void back_to_plugin_list() {
    plugin_view_ = PluginView::list;
    plugin_pending_focus_ = plugin_opened_from_;
    queue_refresh();
  }

  // 「保存名单」是主按钮：设计里固定的强调色填充，悬停和按下时也不换成系统强调色。
  void make_primary(Button const &button) {
    auto resources = button.Resources();
    auto put = [&resources](const wchar_t *key, Color const &color) {
      resources.Insert(box_value(hstring(key)), brush(color));
    };
    const auto accent = palette_.accent;
    auto faded = [accent](double alpha) {
      return Color{static_cast<uint8_t>(std::lround(alpha * 255)), accent.R, accent.G, accent.B};
    };
    put(L"ButtonBackground", accent);
    put(L"ButtonBackgroundPointerOver", faded(.9));
    put(L"ButtonBackgroundPressed", faded(.8));
    put(L"ButtonForeground", palette_.on_accent);
    put(L"ButtonForegroundPointerOver", palette_.on_accent);
    put(L"ButtonForegroundPressed", palette_.on_accent);
  }

  // 列表里打开一个视图的一行（共享页的 PluginNavRow）：整张卡片是一个按钮，读屏读到标题，说明和行尾标记作为帮助文本。
  Button plugin_nav_row(StackPanel const &group, wchar_t glyph, std::wstring const &title,
                        std::wstring const &description, std::wstring const &marker,
                        std::function<void()> open) {
    record(title, description);
    Grid header;
    header.ColumnSpacing(16);
    for (auto width : {GridLength{20, GridUnitType::Pixel}, GridLength{1, GridUnitType::Star},
                       GridLength{1, GridUnitType::Auto}, GridLength{1, GridUnitType::Auto}}) {
      ColumnDefinition column;
      column.Width(width);
      header.ColumnDefinitions().Append(column);
    }
    auto icon = make_icon(glyph, 20, palette_.text);
    icon.VerticalAlignment(VerticalAlignment::Center);
    header.Children().Append(icon);
    StackPanel texts;
    texts.Spacing(1);
    texts.VerticalAlignment(VerticalAlignment::Center);
    texts.Children().Append(make_text(title, 14, palette_.text));
    if (!description.empty())
      texts.Children().Append(make_text(description, 12, palette_.sub));
    Grid::SetColumn(texts, 1);
    header.Children().Append(texts);
    if (!marker.empty()) {
      auto mark = make_text(marker, 12, palette_.sub);
      mark.TextWrapping(TextWrapping::NoWrap);
      mark.VerticalAlignment(VerticalAlignment::Center);
      Grid::SetColumn(mark, 2);
      header.Children().Append(mark);
    }
    auto chevron = make_icon(0xE76C, 12, palette_.sub);
    chevron.VerticalAlignment(VerticalAlignment::Center);
    Grid::SetColumn(chevron, 3);
    header.Children().Append(chevron);

    Button row;
    row.Content(header);
    row.HorizontalAlignment(HorizontalAlignment::Stretch);
    row.HorizontalContentAlignment(HorizontalAlignment::Stretch);
    row.MinHeight(62);
    row.Padding(Thickness{12, 8, 12, 8});
    row.Background(brush(palette_.card));
    row.BorderBrush(brush(palette_.stroke));
    row.BorderThickness(Thickness{1, 1, 1, 1});
    row.CornerRadius(CornerRadius{3, 3, 3, 3});
    A11y::SetName(row, hstring(title));
    std::wstring help = description;
    if (!marker.empty())
      help += (help.empty() ? L"" : L"，") + marker;
    if (!help.empty())
      A11y::SetHelpText(row, hstring(help));
    row.Click([open = std::move(open)](Inspectable const &, RoutedEventArgs const &) { open(); });
    group.Children().Append(row);
    mark_search_target(title, row);
    return row;
  }

  // 视图的头（共享页的 PluginViewHeader）：回到列表的「我的插件」，然后是视图的标题。视图刚打开时焦点落在返回上。
  void plugin_view_header(StackPanel const &page, std::wstring const &title) {
    StackPanel label;
    label.Orientation(Orientation::Horizontal);
    label.Spacing(6);
    auto chevron = make_icon(0xE76B, 12, palette_.text);
    chevron.VerticalAlignment(VerticalAlignment::Center);
    label.Children().Append(chevron);
    label.Children().Append(make_text(L"我的插件", 14, palette_.text));
    Button back;
    back.Content(label);
    back.Height(32);
    back.Padding(Thickness{12, 0, 12, 0});
    A11y::SetName(back, L"返回我的插件");
    back.Click([this](Inspectable const &, RoutedEventArgs const &) { back_to_plugin_list(); });
    page.Children().Append(back);
    auto heading = make_text(title, 20, palette_.text);
    heading.FontWeight(Windows::UI::Text::FontWeight{600});
    heading.Padding(Thickness{4, 16, 4, 0});
    page.Children().Append(heading);
    if (!indexing_ && plugin_pending_focus_ == L"back") {
      plugin_pending_focus_.clear();
      focus_target_ = back;
    }
  }

  // 一段说明和一排按钮（共享页的 ActionBlock）。
  void plugin_action_block(StackPanel const &group, std::wstring const &note,
                           std::vector<FrameworkElement> const &buttons = {}) {
    StackPanel body;
    body.Spacing(10);
    body.Padding(Thickness{25, 14, 25, 14});
    if (!note.empty())
      body.Children().Append(make_text(note, 13, palette_.sub));
    if (!buttons.empty()) {
      StackPanel actions;
      actions.Orientation(Orientation::Horizontal);
      actions.Spacing(8);
      for (auto const &button : buttons)
        actions.Children().Append(button);
      body.Children().Append(actions);
    }
    auto card = make_card();
    card.Child(body);
    group.Children().Append(card);
  }

  // 只显示一个值的一行（共享页的 SummaryRow）：值可以选中复制，长的折行。
  void plugin_summary_row(StackPanel const &group, wchar_t glyph, std::wstring const &title,
                          std::wstring const &description, std::wstring const &value) {
    auto shown = make_text(value, 14, palette_.sub);
    shown.MaxWidth(360);
    shown.TextAlignment(TextAlignment::Right);
    shown.IsTextSelectionEnabled(true);
    add_row(group, glyph, title, description, shown);
  }

  // 一列只有文字的行（共享页没有控件的 Row）：标题和可选的说明，行间有分隔线。
  void plugin_line_card(StackPanel const &group,
                        std::vector<std::pair<std::wstring, std::wstring>> const &lines) {
    StackPanel list;
    bool first = true;
    for (auto const &[title, description] : lines) {
      Border divider;
      divider.BorderThickness(Thickness{0, first ? 0.0 : 1.0, 0, 0});
      divider.BorderBrush(brush(palette_.divider));
      first = false;
      StackPanel texts;
      texts.Spacing(1);
      texts.Padding(Thickness{12, 8, 12, 8});
      auto head = make_text(title, 14, palette_.text);
      head.IsTextSelectionEnabled(true);
      texts.Children().Append(head);
      if (!description.empty()) {
        auto detail = make_text(description, 12, palette_.sub);
        detail.IsTextSelectionEnabled(true);
        texts.Children().Append(detail);
      }
      divider.Child(texts);
      list.Children().Append(divider);
    }
    auto card = make_card();
    card.Child(list);
    group.Children().Append(card);
  }

  // 共享页的 SettingsEmptyMessage：列表还没读到、没读成功或空的时候的一句。
  TextBlock plugin_empty_message(std::wstring_view value) {
    auto message = make_text(value, 13, palette_.sub);
    message.HorizontalAlignment(HorizontalAlignment::Center);
    message.TextAlignment(TextAlignment::Center);
    message.Margin(Thickness{16, 20, 16, 12});
    return message;
  }

  void build_plugins_page(StackPanel const &page) {
    if (indexing_) {
      // 搜索既能找到列表上的入口，也能找到「声音与效果」里的设置；记下后者的标题，选中这样的结果时打开那个视图。
      community_tabs(page, L"插件来源", L"我的插件", L"社区插件", L"浏览、安装和发布插件", false,
                     [](bool) {});
      build_plugin_list(page);
      const auto first = search_index_.size();
      StackPanel scratch;
      build_plugin_sound_effects(scratch);
      plugin_sound_titles_.clear();
      for (auto i = first; i < search_index_.size(); ++i)
        plugin_sound_titles_.insert(search_index_[i].title);
      return;
    }
    plugin_mentions_warning_ = nullptr;
    plugin_mentions_save_ = nullptr;
    // 搜索选中「社区插件」时打开图库，选中「我的插件」里的设置时回到列表。
    if (!focus_row_.empty()) {
      const bool community = focus_row_ == L"社区插件";
      if (community != plugin_community_view_) {
        plugin_community_view_ = community;
        if (community)
          enter_plugin_community();
        else
          leave_plugin_community();
      }
    }
    community_tabs(page, L"插件来源", L"我的插件", L"社区插件", L"浏览、安装和发布插件",
                   plugin_community_view_, [this](bool shown) { set_plugin_view(shown); });
    if (plugin_community_view_) {
      build_plugin_community(page);
      return;
    }
    if (plugins_stale_)
      load_plugins();
    if (!focus_row_.empty() && focus_row_ != L"社区插件") {
      const bool sound = plugin_sound_titles_.contains(focus_row_);
      plugin_view_ = sound ? PluginView::sound_effects : PluginView::list;
      plugin_opened_from_ = sound ? L"sound-effects" : L"";
      plugin_pending_focus_.clear();
    }
    if (plugin_view_ == PluginView::pack) {
      for (auto const &pack : plugin_catalog_.packages)
        if (pack.kind == plugin_view_kind_ && pack.id == plugin_view_id_) {
          if (pack.kind == L"script")
            build_script_detail(page, pack);
          else
            build_plugin_detail(page, pack);
          return;
        }
    }
    if (plugin_view_ == PluginView::missing && plugin_catalog_state_ == PluginCatalogState::loaded) {
      for (auto const &entry : pg::missing_selections(plugin_preferences(), plugin_catalog_.packages,
                                                      pg::acted_kinds(helpcode_offered())))
        if (entry.kind == plugin_view_kind_ && entry.id == plugin_view_id_) {
          build_plugin_missing(page, entry);
          return;
        }
    }
    if (plugin_view_ == PluginView::sound_effects) {
      build_plugin_sound_effects(page);
      return;
    }
    if (plugin_view_ == PluginView::mentions) {
      build_plugin_mentions(page);
      return;
    }
    // 打开的包或缺失的选择在重读后不在了：回到列表，焦点落到同一类的第一行。
    if (plugin_view_ != PluginView::list && plugin_pending_focus_.empty())
      plugin_pending_focus_ = plugin_opened_from_;
    plugin_view_ = PluginView::list;
    build_plugin_list(page);
  }

  // 我的插件：先是不属于任何一个包的两个入口，再是按类型分组的已安装插件和缺失的选择，然后是导入（少见的操作，放在后面）和无法载入的文件夹。
  void build_plugin_list(StackPanel const &page) {
    const auto preferences = plugin_preferences();
    const auto kinds = pg::acted_kinds(helpcode_offered());
    const auto modes = plugin_table_modes();
    const bool loaded = plugin_catalog_state_ == PluginCatalogState::loaded;
    const auto missing = loaded ? pg::missing_selections(preferences, plugin_catalog_.packages, kinds)
                                : std::vector<pg::MissingSelection>{};
    std::vector<std::pair<std::wstring, Button>> rows;

    auto entries = add_group(page, L"");
    rows.emplace_back(L"sound-effects",
                      plugin_nav_row(entries, 0xE767, L"声音与效果", pg::sound_effects_description(), L"",
                                     [this] {
                                       open_plugin_view(PluginView::sound_effects, L"", L"",
                                                        L"sound-effects");
                                     }));
    rows.emplace_back(
        L"mentions",
        plugin_nav_row(entries, 0xE716, L"@ 名单",
                       pg::mentions_description(plugin_mentions_dirty(), plugin_saved_mentions_.size(),
                                                modes.scheme_opens,
                                                document_.Boolean(L"local_modes.mention", false)),
                       L"", [this] { open_plugin_view(PluginView::mentions, L"", L"", L"mentions"); }));

    // 指令表这一组带着 / 模式的说明，所以空着也显示。
    struct KindGroup {
      std::wstring kind;
      std::vector<pg::Package const *> packs;
      std::vector<pg::MissingSelection const *> gone;
    };
    std::vector<KindGroup> groups;
    for (auto const kind : pg::kind_order) {
      KindGroup group{std::wstring(kind), {}, {}};
      for (auto const &pack : plugin_catalog_.packages)
        if (pack.kind == kind)
          group.packs.push_back(&pack);
      for (auto const &entry : missing)
        if (entry.kind == kind)
          group.gone.push_back(&entry);
      if (!group.packs.empty() || !group.gone.empty() || kind == L"command_table")
        groups.push_back(std::move(group));
    }
    if (plugin_catalog_state_ == PluginCatalogState::loading && !indexing_)
      page.Children().Append(plugin_empty_message(L"正在读取插件…"));
    if (plugin_catalog_state_ == PluginCatalogState::failed)
      page.Children().Append(
          plugin_empty_message(L"插件列表没有读取成功，重新打开这个页面会再试一次。"));
    std::vector<pg::Package const *> scripts;
    for (auto const &pack : plugin_catalog_.packages)
      if (pack.kind == L"script")
        scripts.push_back(&pack);
    if (loaded && groups.empty() && scripts.empty())
      page.Children().Append(plugin_empty_message(L"没有插件"));
    for (auto const &[kind, packs, gone] : groups) {
      auto group = add_group(page, pg::kind_label(kind));
      if (kind == L"command_table") {
        group.Children().Append(group_note(
            L"在「输入 → 快捷模式」打开 / 指令后，按 / 再输入指令字母即可使用。启用的指令表按启用顺序排列，同一指令以靠前的表为准；最多启用 " +
            std::to_wstring(pg::max_command_tables) + L" 个。"));
        if (packs.empty() && gone.empty() && loaded)
          group.Children().Append(group_note(L"还没有导入指令表。"));
      }
      for (auto const *pack : packs) {
        const auto key = L"pack/" + pack->kind + L"/" + pack->id;
        rows.emplace_back(key, plugin_nav_row(group, page_label("plugins").glyph, pack->name,
                                              pg::pack_row_description(*pack),
                                              pg::pack_marker(*pack, preferences, kinds, modes),
                                              [this, kind = pack->kind, id = pack->id, key] {
                                                open_plugin_view(PluginView::pack, kind, id, key);
                                              }));
      }
      for (auto const *entry : gone) {
        const auto key = L"missing/" + entry->kind + L"/" + entry->id;
        rows.emplace_back(key, plugin_nav_row(group, 0xE7BA, pg::missing_title(*entry),
                                              pg::missing_description(*entry), L"",
                                              [this, kind = entry->kind, id = entry->id, key] {
                                                open_plugin_view(PluginView::missing, kind, id, key);
                                              }));
      }
    }

    // 脚本插件（共享页的「脚本插件（实验）」分组）：说明和同一用户下的程序能伪造授权的残余风险写在分组开头，行尾是授权状态和执行顺序。
    if (!scripts.empty()) {
      const auto running = plugin_running_scripts();
      auto group = add_group(page, L"脚本插件（实验）");
      group.Children().Append(group_note(pg::script_group_note()));
      for (auto const *pack : scripts) {
        const auto key = L"pack/script/" + pack->id;
        rows.emplace_back(key, plugin_nav_row(group, 0xE943, pack->name, pg::script_row_description(*pack),
                                              pg::script_marker(*pack, plugin_grants(), running),
                                              [this, id = pack->id, key] {
                                                open_plugin_view(PluginView::pack, L"script", id, key);
                                              }));
      }
    }

    auto import = add_group(page, L"导入插件");
    StackPanel buttons;
    buttons.Orientation(Orientation::Horizontal);
    buttons.Spacing(8);
    buttons.Children().Append(
        button_control(L"导入文件夹", [this] { import_plugin_pack(true); }, !plugins_busy_));
    buttons.Children().Append(
        button_control(L"导入 .zip", [this] { import_plugin_pack(false); }, !plugins_busy_));
    add_row(import, 0xE8B5, L"从文件导入", std::wstring(pg::import_description), buttons);
    if (!plugins_notice_.empty()) {
      auto notice = group_note(plugins_notice_);
      notice.Margin(Thickness{4, 4, 4, 4});
      A11y::SetLiveSetting(notice, Microsoft::UI::Xaml::Automation::Peers::AutomationLiveSetting::Polite);
      import.Children().Append(notice);
    }

    if (!plugin_catalog_.issues.empty()) {
      auto issues = add_group(page, L"无法载入的插件");
      StackPanel list;
      list.Spacing(6);
      list.Padding(Thickness{25, 14, 25, 14});
      for (auto const &issue : plugin_catalog_.issues)
        list.Children().Append(make_text(pg::kind_label(issue.kind) + L" " +
                                             (issue.folder.empty() ? std::wstring(L"目录") : issue.folder) +
                                             L" 无法载入：" + issue.reason,
                                         13, palette_.sub));
      auto card = make_card();
      A11y::SetName(card, L"无法载入的插件");
      card.Child(list);
      issues.Children().Append(card);
    }

    // 关上视图后焦点回到打开它的那一行；那一行没了（包已删除、缺失的选择已去掉）就落到同一类的第一行，再没有就是列表的第一行。
    if (!indexing_ && !plugin_pending_focus_.empty() && !rows.empty()) {
      const auto target = plugin_pending_focus_;
      plugin_pending_focus_.clear();
      auto kind_of = [](std::wstring const &key) -> std::wstring {
        for (std::wstring_view prefix : {std::wstring_view(L"pack/"), std::wstring_view(L"missing/")})
          if (key.starts_with(prefix)) {
            const auto rest = key.substr(prefix.size());
            return rest.substr(0, rest.find(L'/'));
          }
        return {};
      };
      Button chosen{nullptr};
      for (auto const &[key, row] : rows)
        if (key == target) {
          chosen = row;
          break;
        }
      if (!chosen && !kind_of(target).empty())
        for (auto const &[key, row] : rows)
          if (kind_of(key) == kind_of(target)) {
            chosen = row;
            break;
          }
      focus_target_ = chosen ? chosen : rows.front().second;
    }
  }

  // 声音与效果：不属于哪一个包的开关、方式和音量。当前用哪个包在包自己的详情里选，这里只写出它的名字。
  void build_plugin_sound_effects(StackPanel const &page) {
    plugin_view_header(page, L"声音与效果");
    const auto preferences = plugin_preferences();
    const auto &packages = plugin_catalog_.packages;
    const bool listed = plugin_catalog_state_ == PluginCatalogState::loaded;
    const std::wstring choose(pg::choose_note);
    // 从点下时存着的设置改起：滑块拖过之后页面没有重画，这一页画出来时的那份已经旧了。
    auto set = [this](std::function<void(pg::Preferences &)> const &edit) {
      auto next = plugin_preferences();
      edit(next);
      change_plugins(next);
    };
    // 音量像别的滑块一样拖动时写文档、停下后保存。
    auto percent = [this](std::wstring const &title, wchar_t const *key, double value, bool enabled) {
      return slider_control(title, 0, 100, value,
                            [this, key = std::wstring(key)](double next) {
                              document_.SetNumber(key, next);
                              schedule_save();
                            },
                            enabled, 1, L"%");
    };

    auto keys = add_group(page, L"按键音效");
    add_row(keys, 0xE767, L"按键音", L"打字时按键发声。密码等安全输入框中不发声。",
            toggle_control(L"按键音", preferences.key_sound_enabled, [set](bool on) {
              set([on](pg::Preferences &next) { next.key_sound_enabled = on; });
            }));
    add_row(keys, 0xE8D6, L"发声方式",
            L"按键音效按普通键、空格、回车和退格各自发声；按键旋律每按一键弹出旋律的下一个音，停顿 3 秒后从头开始。",
            segmented_control(L"发声方式", {{L"keys", L"按键音效"}, {L"melody", L"按键旋律"}},
                              preferences.key_sound_mode,
                              [set](std::wstring const &mode) {
                                set([mode](pg::Preferences &next) { next.key_sound_mode = mode; });
                              },
                              preferences.key_sound_enabled));
    plugin_summary_row(keys, 0xE8D6, L"音效包", L"按键、上屏和成就音效都取自这个音效包。" + choose,
                       pg::selected_name(packages, listed, L"sound", preferences.key_sound_pack,
                                         preferences.key_sound_pack, L"keys"));
    if (preferences.key_sound_mode == L"melody")
      plugin_summary_row(keys, 0xEC4F, L"旋律", choose,
                         pg::selected_name(packages, listed, L"sound", preferences.melody_pack,
                                           preferences.melody_pack, L"sequence"));
    add_row(keys, 0xE767, L"音效音量", L"按键音、旋律、上屏音和成就音效共用。",
            percent(L"音效音量", L"plugins.key_sound.volume", preferences.key_sound_volume, true));
    add_row(keys, 0xE8FB, L"上屏音", L"文字上屏时播放音效包的上屏音，与按键音各自开关。",
            toggle_control(L"上屏音", preferences.commit_sound_enabled, [set](bool on) {
              set([on](pg::Preferences &next) { next.commit_sound_enabled = on; });
            }));
    // 成就音效和升档音都是默认关闭的小玩法，收进「更多音效」，主列表只留常用的几项；搜索落到其中一项时展开。
    Expander more;
    more.Header(box_value(L"更多音效"));
    more.HorizontalAlignment(HorizontalAlignment::Stretch);
    more.HorizontalContentAlignment(HorizontalAlignment::Stretch);
    more.IsExpanded(plugin_sound_more_open_ || focus_row_ == L"成就音效" || focus_row_ == L"升档音");
    more.Expanding([this](Expander const &, ExpanderExpandingEventArgs const &) {
      plugin_sound_more_open_ = true;
    });
    more.Collapsed([this](Expander const &, ExpanderCollapsedEventArgs const &) {
      plugin_sound_more_open_ = false;
    });
    StackPanel extra;
    extra.Spacing(4);
    const bool achievements_blocked =
        pg::achievements_blocked(plugin_statistics_enabled_, preferences.achievements_enabled);
    add_row(extra, 0xE734, L"成就音效",
            achievements_blocked
                ? L"上屏字数累计达到 100、1000、1 万等里程碑时播放一段短音。按打字统计计数，请先在「打字统计」页打开记录。"
                : L"上屏字数累计达到 100、1000、1 万等里程碑时播放一段短音。按打字统计计数，需要打开打字统计。",
            toggle_control(L"成就音效", preferences.achievements_enabled,
                           [set](bool on) {
                             set([on](pg::Preferences &next) { next.achievements_enabled = on; });
                           },
                           !achievements_blocked));
    add_row(extra, 0xE767, L"升档音",
            L"连击达到 10、25、50、100 时播放音效包的上屏音，每升一档音调更高，音量随音效音量。需要打开「打字效果」里的连击计数。",
            toggle_control(L"升档音", preferences.combo_tier_sound,
                           [set](bool on) {
                             set([on](pg::Preferences &next) { next.combo_tier_sound = on; });
                           },
                           preferences.combo_counter));
    more.Content(extra);
    keys.Children().Append(more);

    // 选了特效包时由它决定样式，这一项只描述不选包时画什么。强度在 Windows 上固定为 50（client-core 的 `HostPlatform::fixed_effect_intensity`），宿主不读存下的值，所以没有「效果强度」。
    const bool pack_selected = !preferences.effect_pack.empty();
    auto effects = add_group(page, L"打字效果");
    plugin_summary_row(effects, 0xE790, L"特效包",
                       pack_selected ? L"特效包决定样式、强度、颜色和时长，下面的样式和强度不再生效。" : choose,
                       pg::selected_name(packages, listed, L"effect", preferences.effect_pack, L"不使用"));
    std::vector<Option> styles;
    for (auto const &option : pg::effect_style_options)
      styles.push_back({std::wstring(option.value), std::wstring(option.label)});
    add_row(effects, 0xE945, L"效果样式",
            L"闪光：按键时候选栏闪一下；火花：按键和上屏时迸出火花；Power Mode：火花随连击变大。",
            segmented_control(L"效果样式", styles, preferences.effect_style,
                              [set](std::wstring const &style) {
                                set([style](pg::Preferences &next) { next.effect_style = style; });
                              },
                              !pack_selected));
    add_row(effects, 0xE8EF, L"连击计数",
            L"连续打字时在候选栏显示连击数，停顿 3 秒或按退格后重新计数。自动重复的按键不计数。",
            toggle_control(L"连击计数", preferences.combo_counter, [set](bool on) {
              set([on](pg::Preferences &next) { next.combo_counter = on; });
            }));

    // 宿主只在开关打开并且选了音乐包时播放；还没选包时，打开开关顺手选一个随带的音乐包，而不是打开一个听不到声音的开关。一个音乐包都没有时开关不能打开。
    const auto music_pack = preferences.music_pack.empty()
                                ? pg::music_pack_to_select(packages, listed)
                                : preferences.music_pack;
    auto music = add_group(page, L"背景音乐");
    add_row(music, 0xEC4F, L"背景音乐", L"默认关闭。只在输入法处于活动状态时播放，切换到其他输入法时暂停。",
            toggle_control(L"背景音乐", preferences.music_enabled,
                           [set, music_pack](bool on) {
                             set([on, music_pack](pg::Preferences &next) {
                               next.music_enabled = on;
                               if (on && next.music_pack.empty())
                                 next.music_pack = music_pack;
                             });
                           },
                           preferences.music_enabled || !music_pack.empty()));
    const bool no_music = listed && std::none_of(packages.begin(), packages.end(), [](pg::Package const &pack) {
                            return pack.kind == L"music";
                          });
    plugin_summary_row(music, 0xE93C, L"音乐包", no_music ? std::wstring(L"还没有导入音乐包。") : choose,
                       pg::selected_name(packages, listed, L"music", preferences.music_pack, L"未选择"));
    add_row(music, 0xE767, L"音乐音量", L"",
            percent(L"音乐音量", L"plugins.music.volume", preferences.music_volume, true));
  }

  // @ 名单：改动留在这个窗口里，离开视图也不丢，按「保存名单」才写入。@ 模式关着或方案打不开它时，名单上方说明名字不会出候选。输入时就地更新提示和按钮，不重建页面，输入框不会丢掉焦点。
  void build_plugin_mentions(StackPanel const &page) {
    plugin_view_header(page, L"@ 名单");
    const bool mode = document_.Boolean(L"local_modes.mention", false);
    const auto scheme_note = pg::scheme_table_mode_note(
        document_.String(L"scheme", MSIME_EDITION_DEFAULT_SCHEME_W), L"名字与地点（@ 模式）");
    if (!mode || !scheme_note.empty()) {
      auto notes = add_group(page, L"");
      if (!mode)
        plugin_action_block(notes, L"@ 名字与地点（「输入 → 快捷模式」里的开关）已关闭，名单里的名字不会出候选。",
                            {button_control(L"打开 @ 模式", [this] { enable_local_mode(L"mention"); },
                                            loaded_)});
      if (!scheme_note.empty())
        plugin_action_block(notes, scheme_note);
    }

    auto group = add_group(page, L"");
    StackPanel body;
    body.Spacing(10);
    body.Padding(Thickness{25, 14, 25, 14});
    body.Children().Append(make_text(
        L"在「输入 → 快捷模式」打开 @ 名字与地点后，按 @ 再输入拼音或首字母，就会从这份名单里出候选。名单只保存在本机，不随账号同步，也不会读取通讯录或位置。拼音可以留空，中文名字会自动取读音。",
        13, palette_.sub));
    for (std::size_t index = 0; index < plugin_mentions_.size(); ++index) {
      Grid row;
      row.ColumnSpacing(8);
      for (auto width : {GridLength{1, GridUnitType::Star}, GridLength{1, GridUnitType::Star},
                         GridLength{1, GridUnitType::Auto}}) {
        ColumnDefinition column;
        column.Width(width);
        row.ColumnDefinitions().Append(column);
      }
      TextBox name;
      name.Header(box_value(L"名字或地点"));
      name.MaxLength(static_cast<int32_t>(pg::max_mention_text_utf16));
      name.Text(hstring(plugin_mentions_[index].text));
      A11y::SetName(name, L"名字或地点");
      name.TextChanged([this, index](Inspectable const &sender, TextChangedEventArgs const &) {
        edit_plugin_mention(index, std::wstring(sender.as<TextBox>().Text().c_str()), true);
      });
      row.Children().Append(name);
      TextBox key;
      key.Header(box_value(L"拼音"));
      key.PlaceholderText(L"zhang'san");
      key.MaxLength(static_cast<int32_t>(pg::max_mention_key_bytes));
      key.IsSpellCheckEnabled(false);
      key.IsTextPredictionEnabled(false);
      key.Text(hstring(plugin_mentions_[index].key));
      A11y::SetName(key, L"拼音");
      key.TextChanged([this, index](Inspectable const &sender, TextChangedEventArgs const &) {
        edit_plugin_mention(index, std::wstring(sender.as<TextBox>().Text().c_str()), false);
      });
      Grid::SetColumn(key, 1);
      row.Children().Append(key);
      auto remove = button_control(L"删除", [this, index] {
        if (index < plugin_mentions_.size()) {
          plugin_mentions_.erase(plugin_mentions_.begin() + static_cast<std::ptrdiff_t>(index));
          queue_refresh();
        }
      });
      remove.VerticalAlignment(VerticalAlignment::Bottom);
      A11y::SetName(remove, hstring(L"删除第 " + std::to_wstring(index + 1) + L" 行"));
      Grid::SetColumn(remove, 2);
      row.Children().Append(remove);
      body.Children().Append(row);
    }
    plugin_mentions_warning_ = make_text(L"", 13, palette_.text);
    A11y::SetLiveSetting(plugin_mentions_warning_,
                         Microsoft::UI::Xaml::Automation::Peers::AutomationLiveSetting::Assertive);
    body.Children().Append(plugin_mentions_warning_);
    StackPanel actions;
    actions.Orientation(Orientation::Horizontal);
    actions.Spacing(8);
    actions.Children().Append(button_control(
        L"添加",
        [this] {
          plugin_mentions_.push_back({});
          queue_refresh();
        },
        plugin_mentions_.size() < pg::max_mentions));
    Button save;
    save.Content(box_value(L"保存名单"));
    save.Height(32);
    save.Padding(Thickness{16, 0, 16, 0});
    make_primary(save);
    save.Click([this](Inspectable const &, RoutedEventArgs const &) { save_plugin_mentions(); });
    plugin_mentions_save_ = save;
    actions.Children().Append(save);
    body.Children().Append(actions);
    auto card = make_card();
    card.Child(body);
    group.Children().Append(card);
    update_plugin_mentions_state();
  }

  // 输入框里的一次改动：只改这一行，再就地更新提示和保存按钮。
  void edit_plugin_mention(std::size_t index, std::wstring value, bool name) {
    if (index >= plugin_mentions_.size())
      return;
    auto &slot = name ? plugin_mentions_[index].text : plugin_mentions_[index].key;
    if (slot == value)
      return;
    slot = std::move(value);
    update_plugin_mentions_state();
  }

  // 有改动且不能保存时显示原因；保存按钮只在有改动、能保存、没有别的操作在进行时可按。
  void update_plugin_mentions_state() {
    const auto issue = pg::mention_list_issue(plugin_mentions_);
    const bool dirty = plugin_mentions_dirty();
    if (plugin_mentions_warning_) {
      const bool shown = issue && dirty;
      plugin_mentions_warning_.Text(hstring(shown ? *issue : std::wstring()));
      plugin_mentions_warning_.Visibility(shown ? Visibility::Visible : Visibility::Collapsed);
    }
    if (plugin_mentions_save_)
      plugin_mentions_save_.IsEnabled(!plugins_busy_ && dirty && !issue);
  }

  // 一个已安装的包：里面有什么，怎样把它设为这一类的当前选择，不是内置的还可以删除。
  void build_plugin_detail(StackPanel const &page, pg::Package const &pack) {
    plugin_view_header(page, pack.name);
    auto info = add_group(page, L"信息");
    plugin_summary_row(info, 0xE8EC, L"类型", L"", pg::pack_kind_label(pack));
    plugin_summary_row(info, 0xE946, L"版本", L"", pack.version);
    if (!pack.author.empty())
      plugin_summary_row(info, 0xE77B, L"作者", L"", pack.author);
    plugin_summary_row(info, 0xE8D7, L"许可证", L"", pack.license);
    if (pack.builtin)
      plugin_summary_row(info, 0xE7B8, L"来源", L"", L"内置，不能删除");
    if (!pack.description.empty())
      info.Children().Append(group_note(pack.description));
    build_plugin_content(page, pack);
    build_plugin_actions(add_group(page, L"使用"), pack);
    if (!pack.builtin) {
      auto remove = button_control(L"删除", [this, pack] { remove_plugin_pack(pack); }, !plugins_busy_);
      A11y::SetName(remove, hstring(L"删除" + pack.name));
      plugin_action_block(add_group(page, L""),
                          L"删除后包里的文件会从本机移除；正在使用的话，设置会回到默认选择。", {remove});
    }
  }

  // 各类型自己的内容：符号集的各组、单词本和辅助码表的前几条、短语表的前 20 行、音乐包的曲目、指令表的指令、特效包的样式和参数。
  void build_plugin_content(StackPanel const &page, pg::Package const &pack) {
    using Lines = std::vector<std::pair<std::wstring, std::wstring>>;
    if (pack.kind == L"symbol_set") {
      for (auto const &symbols : pack.groups) {
        auto group = add_group(page, pg::symbol_group_title(symbols));
        auto note = group_note(pg::symbol_group_preview(symbols));
        note.IsTextSelectionEnabled(true);
        group.Children().Append(note);
      }
      return;
    }
    if (pack.kind == L"wordbook") {
      const auto shown = std::min(pack.first_words.size(), pg::wordbook_preview_words);
      auto group = add_group(
          page, L"单词（" + pg::js_number(pack.word_count.value_or(static_cast<double>(shown))) + L"）");
      Lines lines;
      for (std::size_t i = 0; i < shown; ++i)
        lines.emplace_back(pack.first_words[i], L"");
      if (!lines.empty())
        plugin_line_card(group, lines);
      if (pack.word_count.value_or(0) > static_cast<double>(shown))
        group.Children().Append(group_note(L"只显示前 " + std::to_wstring(shown) + L" 个单词。"));
      return;
    }
    if (pack.kind == L"helpcode") {
      auto group = add_group(
          page, L"辅助码（" +
                    pg::js_number(pack.entries.value_or(static_cast<double>(pack.preview.size()))) +
                    L" 个字）");
      Lines lines;
      for (auto const &entry : pack.preview)
        lines.emplace_back(entry.character, entry.code);
      if (!lines.empty())
        plugin_line_card(group, lines);
      if (pack.entries.value_or(0) > static_cast<double>(pack.preview.size()))
        group.Children().Append(
            group_note(L"只显示前 " + std::to_wstring(pack.preview.size()) + L" 个字。"));
      return;
    }
    if (pack.kind == L"phrase_table") {
      auto group = add_group(page, L"短语（" + std::to_wstring(pack.phrases.size()) + L"）");
      Lines lines;
      for (std::size_t i = 0; i < std::min(pack.phrases.size(), pg::phrase_preview_rows); ++i)
        lines.emplace_back(pack.phrases[i].key, pack.phrases[i].text);
      if (!lines.empty())
        plugin_line_card(group, lines);
      if (pack.phrases.size() > lines.size())
        group.Children().Append(
            group_note(L"只显示前 " + std::to_wstring(pg::phrase_preview_rows) + L" 条。"));
      return;
    }
    if (pack.kind == L"music") {
      auto group = add_group(page, L"曲目（" + std::to_wstring(pack.tracks.size()) + L"）");
      if (pack.tracks.empty()) {
        group.Children().Append(group_note(L"没有曲目。"));
        return;
      }
      Lines lines;
      for (auto const &track : pack.tracks)
        lines.emplace_back(track, L"");
      plugin_line_card(group, lines);
      return;
    }
    if (pack.kind == L"command_table") {
      auto group = add_group(page, L"指令（" + std::to_wstring(pack.commands.size()) + L"）");
      if (pack.commands.empty()) {
        group.Children().Append(group_note(L"没有指令。"));
        return;
      }
      Lines lines;
      for (auto const &command : pack.commands)
        lines.emplace_back(L"/" + command.trigger, command.title);
      plugin_line_card(group, lines);
      return;
    }
    if (pack.kind == L"effect") {
      auto group = add_group(page, L"特效");
      if (!pack.style.empty())
        plugin_summary_row(group, 0xE945, L"样式", L"", pg::effect_style_label(pack.style));
      if (pack.intensity)
        plugin_summary_row(group, 0xE9D2, L"强度", L"", pg::js_number(*pack.intensity) + L"%");
      if (!pack.colors.empty()) {
        StackPanel swatches;
        swatches.Orientation(Orientation::Horizontal);
        swatches.Spacing(10);
        for (auto const &color : pack.colors) {
          StackPanel item;
          item.Orientation(Orientation::Horizontal);
          item.Spacing(4);
          if (const auto parsed = parse_hex(color)) {
            Shapes::Ellipse dot;
            dot.Width(12);
            dot.Height(12);
            dot.Fill(brush(*parsed));
            dot.Stroke(brush(palette_.stroke2));
            dot.StrokeThickness(1);
            dot.VerticalAlignment(VerticalAlignment::Center);
            item.Children().Append(dot);
          }
          auto label = make_text(color, 14, palette_.sub);
          label.IsTextSelectionEnabled(true);
          item.Children().Append(label);
          swatches.Children().Append(item);
        }
        add_row(group, 0xE790, L"颜色", L"", swatches);
      }
      if (pack.duration_ms)
        plugin_summary_row(group, 0xE916, L"时长", L"", pg::js_number(*pack.duration_ms) + L" 毫秒");
      if (pack.particles)
        plugin_summary_row(group, 0xE9D2, L"粒子数", L"", pg::js_number(*pack.particles));
    }
  }

  // 「使用」：各类型怎样成为当前选择（共享页的 PackActions，取 Windows 宿主的分支）。
  void build_plugin_actions(StackPanel const &group, pg::Package const &pack) {
    const auto preferences = plugin_preferences();
    const auto scheme = document_.String(L"scheme", MSIME_EDITION_DEFAULT_SCHEME_W);
    auto select = [this, pack] { change_plugins(pg::with_pack_selected(plugin_preferences(), pack)); };
    auto go_typing = [this] { navigate("typing", true); };
    if (pack.kind == L"sound") {
      const auto action = pg::sound_pack_action(pack, preferences);
      plugin_action_block(group, action.note,
                          {button_control(action.label, select, loaded_ && !action.disabled)});
      return;
    }
    if (pack.kind == L"effect") {
      const bool current = preferences.effect_pack == pack.id;
      plugin_action_block(
          group, L"特效包决定样式、强度、颜色和时长，使用后「声音与效果」里的样式和强度不再生效。",
          {current ? button_control(L"停用",
                                    [this] {
                                      auto next = plugin_preferences();
                                      next.effect_pack.clear();
                                      change_plugins(next);
                                    },
                                    loaded_)
                   : button_control(L"使用此特效包", select, loaded_)});
      return;
    }
    if (pack.kind == L"music") {
      const auto action = pg::music_pack_action(pack, preferences);
      std::vector<FrameworkElement> buttons{
          button_control(action.label, select, loaded_ && !action.disabled)};
      if (preferences.music_pack == pack.id)
        buttons.push_back(button_control(L"不再使用",
                                         [this] {
                                           auto next = plugin_preferences();
                                           next.music_pack.clear();
                                           change_plugins(next);
                                         },
                                         loaded_));
      plugin_action_block(group, action.note, buttons);
      return;
    }
    if (pack.kind == L"command_table" || pack.kind == L"phrase_table") {
      const bool command = pack.kind == L"command_table";
      const auto table = command ? pg::command_table_switch(preferences, pack.id)
                                 : pg::phrase_table_switch(preferences, pack.id);
      add_row(group, command ? 0xE756 : 0xE8C1, L"启用", table.description,
              toggle_control(L"启用", table.on,
                             [this, command, id = pack.id](bool on) {
                               auto next = plugin_preferences();
                               auto &tables = command ? next.command_tables : next.phrase_tables;
                               tables = pg::with_table(tables, id, on);
                               change_plugins(next);
                             },
                             !table.full));
      if (command && !document_.Boolean(L"local_modes.command", false))
        plugin_action_block(
            group,
            L"/ 指令（「输入 → 快捷模式」里的开关）已关闭，启用的指令表不会生效。打开后，在中文标点下没有输入时按 / 再输入指令字母即可使用。",
            {button_control(L"打开 / 指令", [this] { enable_local_mode(L"command"); }, loaded_)});
      if (!command && !document_.Boolean(L"local_modes.quick_phrase", true))
        plugin_action_block(
            group,
            L"快捷短语（K 模式）已关闭。在「输入 → 快捷模式」打开后，按 Shift+K 再输入编码即可用到短语表。",
            {button_control(L"前往输入设置", go_typing)});
      if (const auto note =
              pg::scheme_table_mode_note(scheme, command ? L"指令（/ 模式）" : L"快捷短语（K 模式）");
          !note.empty())
        plugin_action_block(group, note);
      return;
    }
    if (pack.kind == L"symbol_set") {
      plugin_action_block(group, L"装上即在符号面板里显示：符号组以插件名为分类排在内置符号之后，颜文字组排在颜文字的 All 之后。卸载后不再显示。");
      return;
    }
    if (pack.kind == L"wordbook") {
      plugin_action_block(group, L"这本书出现在背单词的词书里。卸载插件后复习进度仍会保留，重新安装后可以接着复习。",
                          {button_control(L"去背单词",
                                          [this, book = pg::wordbook_book_id(pack.id)] {
                                            open_plugin_wordbook(book);
                                          },
                                          !vocabulary_busy_)});
      return;
    }
    if (pack.kind == L"helpcode") {
      if (!helpcode_offered()) {
        plugin_action_block(group, L"这台设备不使用辅助码。");
        return;
      }
      for (const bool quanpin : {true, false}) {
        const std::wstring title = quanpin ? L"用于全拼" : L"用于双拼";
        const auto &current = quanpin ? preferences.helpcode_pack_quanpin : preferences.helpcode_pack_shuangpin;
        add_row(group, 0xE8CB, title,
                quanpin ? L"替换全拼的辅助码方案；关闭后回到原来的方案。"
                        : L"替换双拼的辅助码方案；关闭后回到原来的方案。",
                toggle_control(title, current == pack.id, [this, quanpin, id = pack.id](bool on) {
                  auto next = plugin_preferences();
                  (quanpin ? next.helpcode_pack_quanpin : next.helpcode_pack_shuangpin) =
                      on ? id : std::wstring();
                  change_plugins(next);
                }));
      }
      plugin_action_block(group, L"辅助码的开关和显示方式在「输入 → 辅助码」里。",
                          {button_control(L"前往辅助码设置", go_typing)});
    }
  }

  // 指名的包已不在本机、或者是另一种方式的声音包的选择，只有一条出路：像删除那个包一样去掉这个选择。
  void build_plugin_missing(StackPanel const &page, pg::MissingSelection const &entry) {
    plugin_view_header(page, pg::missing_title(entry));
    plugin_action_block(add_group(page, L""), pg::missing_note(entry),
                        {button_control(pg::missing_action(entry),
                                        [this, entry] {
                                          change_plugins(pg::missing_dropped(plugin_preferences(), entry));
                                          back_to_plugin_list();
                                        },
                                        loaded_)});
  }

  // ---- 脚本插件（实验） ----

  // 共享页 plugin-script-view.tsx 在 Windows 上的样子：列表里的「脚本插件（实验）」分组见 build_plugin_list，这里是详情和授权对话框。授权、撤销和排序都经 msime_client_plugins 的 set_script_grant 和 reorder_scripts，规则在 client-services 的 script_grants，与 Tauri 的同名命令相同；页面的勾选联动、标记和文案在 PluginPageModel.h。

  pg::ScriptGrants const *plugin_grants() const { return plugin_scripts_ ? &*plugin_scripts_ : nullptr; }

  // 输入法会运行的插件，按执行顺序。
  std::vector<std::wstring> plugin_running_scripts() const {
    return pg::running_script_ids(plugin_grants(), plugin_catalog_.packages);
  }

  pg::Package const *plugin_script_pack(std::wstring const &id) const {
    for (auto const &pack : plugin_catalog_.packages)
      if (pack.kind == L"script" && pack.id == id)
        return &pack;
    return nullptr;
  }

  // 写一个包的授权（共享页的 writeScriptGrant）：成功时换上宿主回答的整份授权。包在页面读到之后被换过（plugin_script_changed）时先重读目录和授权，页面换成现在的内容哈希，再交给 done 说明。
  void write_script_grant(pg::Package const &pack, bool enabled, std::vector<std::wstring> granted,
                          std::function<void(Response const &)> done) {
    const auto generation = plugins_generation_;
    run_host(
        [id = pack.id, hash = pack.content_hash, enabled, granted = std::move(granted)] {
          JsonArray capabilities;
          for (auto const &capability : granted)
            capabilities.Append(JsonValue::CreateStringValue(hstring(capability)));
          auto action = plugin_action(L"set_script_grant");
          action.SetNamedValue(L"id", JsonValue::CreateStringValue(hstring(id)));
          action.SetNamedValue(L"hash", JsonValue::CreateStringValue(hstring(hash)));
          action.SetNamedValue(L"enabled", JsonValue::CreateBooleanValue(enabled));
          action.SetNamedValue(L"granted", capabilities);
          return plugins_call(action, false, true);
        },
        [this, generation, done = std::move(done)](Response const &response) {
          if (generation != plugins_generation_)
            return;
          if (auto grants = plugin_script_grants(response)) {
            plugin_scripts_ = std::move(*grants);
            done(response);
            return;
          }
          if (response_error(response) == L"plugin_script_changed") {
            refresh_plugin_catalog(generation, [done, response] { done(response); });
            return;
          }
          done(response);
        });
  }

  // 详情的启用开关关掉：停用并撤销全部授权（set(enabled = false, granted = [])），输入法在下一次聚焦时停止运行它。
  void revoke_script(pg::Package const &pack) {
    if (!begin_plugin_action()) {
      redraw_plugins();
      return;
    }
    const auto generation = plugins_generation_;
    write_script_grant(pack, false, {}, [this, generation](Response const &response) {
      if (!response.ok)
        show_notice(plugin_failure(response, L"停用失败，请重试。"));
      plugin_pending_focus_ = L"script-switch";
      end_plugin_action(generation);
    });
    redraw_plugins();
  }

  // 「提前」「推后」：只在输入法会运行的插件之间换位（共享页的 moveScript）。
  void move_script(std::wstring const &id, int offset) {
    auto order = pg::moved_order(plugin_running_scripts(), id, offset);
    if (!order || !begin_plugin_action())
      return;
    const auto generation = plugins_generation_;
    run_host(
        [order = std::move(*order)] {
          JsonArray ids;
          for (auto const &item : order)
            ids.Append(JsonValue::CreateStringValue(hstring(item)));
          auto action = plugin_action(L"reorder_scripts");
          action.SetNamedValue(L"ids", ids);
          return plugins_call(action, false, true);
        },
        [this, generation, offset](Response const &response) {
          if (generation != plugins_generation_)
            return;
          if (auto grants = plugin_script_grants(response))
            plugin_scripts_ = std::move(*grants);
          else
            show_notice(plugin_failure(response, L"执行顺序没有保存，请重试。"));
          plugin_pending_focus_ = offset < 0 ? L"script-up" : L"script-down";
          end_plugin_action(generation);
        });
    redraw_plugins();
  }

  // 打开授权对话框：启用开关打开时（opener 是 script-switch），或已启用时的「调整授权」（script-adjust）。对话框从「取消」开始：它是从开关打开的，按住不放的空格或回车不该直接把读写文字的权限授出去。关上之后焦点回到打开它的控件，页面重画一次，取消时开关回到「关」。
  fire_and_forget open_script_grant(pg::Package pack, std::wstring opener) {
    if (dialog_open_ || plugin_grant_.refresh) {
      redraw_plugins();
      co_return;
    }
    auto strong = get_strong();
    const auto generation = plugin_grant_.generation + 1;
    plugin_grant_ = ScriptGrantDialog{};
    plugin_grant_.generation = generation;
    plugin_grant_.id = pack.id;
    plugin_grant_.granted = pg::grant_dialog_initial(pack, pg::script_grant(plugin_grants(), pack.id));
    plugin_grant_.opener = std::move(opener);

    ScrollViewer scroller;
    scroller.MaxHeight(560);
    StackPanel form;
    form.Spacing(12);
    form.MinWidth(440);
    form.Padding(Thickness{0, 0, 12, 0});
    scroller.Content(form);
    auto dialog = publish_dialog(pg::grant_dialog_title(pack), [this] { return plugin_grant_.busy; },
                                 [this] { submit_script_grant(); });
    dialog.Content(scroller);
    dialog.DefaultButton(ContentDialogButton::Close);
    A11y::SetName(dialog, hstring(pg::grant_dialog_title(pack)));
    const auto weak_dialog = make_weak(dialog);
    const auto weak_form = make_weak(form);
    plugin_grant_.dialog = weak_dialog;
    plugin_grant_.refresh = [this, weak_dialog, weak_form] {
      DispatcherQueue().TryEnqueue([this, weak_dialog, weak_form] {
        auto shown = weak_dialog.get();
        auto content = weak_form.get();
        if (shown && content && plugin_grant_.refresh)
          build_script_grant(content, shown);
      });
    };
    build_script_grant(form, dialog);
    co_await show_dialog(dialog);
    ++plugin_grant_.generation;
    plugin_grant_.refresh = nullptr;
    plugin_grant_.busy = false;
    plugin_grant_.boxes.clear();
    plugin_pending_focus_ = plugin_grant_.opener;
    redraw_plugins();
  }

  // 对话框的内容：「已更新，需要重新授权」、说明、清单申请的每项能力一个复选框（或没有申请任何权限的说明）、残余风险，以及被宿主拒绝时的原因。对话框留着，用户看过原因可以改了再试。
  void build_script_grant(StackPanel const &form, ContentDialog const &dialog) {
    auto &state = plugin_grant_;
    form.Children().Clear();
    state.boxes.clear();
    const auto *pack = plugin_script_pack(state.id);
    if (!pack) {
      // 包在对话框开着时被删掉了：没有东西可授权。
      dialog.Hide();
      return;
    }
    dialog.Title(box_value(hstring(pg::grant_dialog_title(*pack))));
    if (pg::script_grant_state(*pack, pg::script_grant(plugin_grants(), pack->id)) ==
        pg::ScriptGrantState::stale)
      form.Children().Append(account_message(pg::grant_dialog_stale(*pack), InfoBarSeverity::Warning));
    form.Children().Append(make_text(pg::grant_dialog_intro, 14, palette_.sub));
    // 提供 / 指令的插件：列出它的指令；调整授权时（已启用）每条后面接着现在的状态。
    if (pack->kind == L"script" && !pack->commands.empty()) {
      const bool enabled = pg::script_grant_state(*pack, pg::script_grant(plugin_grants(), pack->id)) ==
                           pg::ScriptGrantState::enabled;
      StackPanel commands;
      commands.Spacing(2);
      A11y::SetName(commands, L"它提供的 / 指令");
      commands.Children().Append(make_text(L"它在 / 指令里提供：", 14, palette_.sub));
      for (auto const &command : pack->commands)
        commands.Children().Append(make_text(
            pg::script_command_line(command, enabled ? pg::script_command_status(*pack, command.trigger,
                                                                                   plugin_grants(),
                                                                                   plugin_catalog_.packages)
                                                     : std::wstring()),
            14, palette_.sub));
      form.Children().Append(commands);
    }
    const auto requested = pg::sort_capabilities(pack->permissions);
    if (requested.empty()) {
      form.Children().Append(make_text(pg::grant_dialog_no_permissions, 14, palette_.sub));
    } else {
      StackPanel list;
      list.Spacing(6);
      A11y::SetName(list, hstring(pg::grant_dialog_legend));
      auto legend = make_text(pg::grant_dialog_legend, 14, palette_.text);
      legend.FontWeight(Windows::UI::Text::FontWeight{600});
      list.Children().Append(legend);
      for (auto const &item : pg::grant_items(requested, state.granted, state.busy)) {
        StackPanel texts;
        texts.Spacing(1);
        texts.Children().Append(make_text(item.title, 14, palette_.text));
        auto description = make_text(item.description, 12, palette_.sub);
        texts.Children().Append(description);
        CheckBox box;
        box.Content(texts);
        box.IsChecked(item.checked);
        box.IsEnabled(!item.disabled);
        A11y::SetName(box, hstring(item.title));
        A11y::SetHelpText(box, hstring(item.description));
        box.Click([this, capability = item.capability](Inspectable const &sender, RoutedEventArgs const &) {
          const auto checked = sender.as<CheckBox>().IsChecked();
          toggle_script_capability(capability, checked && checked.Value());
        });
        state.boxes.push_back({item.capability, make_weak(box), make_weak(description)});
        list.Children().Append(box);
      }
      form.Children().Append(list);
    }
    form.Children().Append(account_message(std::wstring(pg::script_residual_risk), InfoBarSeverity::Warning));
    if (!state.error.empty()) {
      auto error = account_message(state.error, InfoBarSeverity::Error);
      A11y::SetLiveSetting(error, Automation::Peers::AutomationLiveSetting::Assertive);
      form.Children().Append(error);
    }
    dialog.PrimaryButtonText(hstring(pg::grant_dialog_confirm));
    dialog.IsPrimaryButtonEnabled(!state.busy);
  }

  // 勾选或取消一项（共享页的 toggleCapability）：依赖的一起勾上、依赖它的一起取消、互斥的那一项禁用并写明原因。复选框就地更新，焦点留在刚点的那一项上。
  void toggle_script_capability(std::wstring const &capability, bool checked) {
    auto &state = plugin_grant_;
    const auto *pack = plugin_script_pack(state.id);
    if (!pack || state.busy)
      return;
    state.granted = pg::toggle_capability(state.granted, capability, checked, pack->permissions);
    for (auto const &item : pg::grant_items(pg::sort_capabilities(pack->permissions), state.granted, state.busy))
      for (auto const &entry : state.boxes) {
        if (entry.capability != item.capability)
          continue;
        if (auto box = entry.box.get()) {
          box.IsChecked(item.checked);
          box.IsEnabled(!item.disabled);
          A11y::SetHelpText(box, hstring(item.description));
        }
        if (auto description = entry.description.get())
          description.Text(hstring(item.description));
      }
  }

  // 「授权并启用」：只交出对话框现在列着、勾着的那几项。被宿主拒绝时（超出内存总账、同时启用超过 8 个、包被换过）对话框留着并说明原因；包被换过时页面已经重读，对话框换成新版本申请的那几项。
  void submit_script_grant() {
    auto &state = plugin_grant_;
    const auto *pack = plugin_script_pack(state.id);
    if (state.busy || !pack)
      return;
    state.busy = true;
    state.error.clear();
    const auto generation = state.generation;
    write_script_grant(*pack, true, pg::selected_capabilities(state.granted, pack->permissions),
                       [this, generation](Response const &response) {
                         auto &current = plugin_grant_;
                         if (generation != current.generation)
                           return;
                         current.busy = false;
                         if (response.ok) {
                           if (auto shown = current.dialog.get())
                             shown.Hide();
                           return;
                         }
                         current.error = plugin_failure(response, std::wstring(pg::grant_dialog_failed));
                         if (current.refresh)
                           current.refresh();
                       });
    if (state.refresh)
      state.refresh();
  }

  // 一个脚本插件（共享页的 ScriptPluginDetailView）：它在什么时候运行、申请了什么、授予了什么、输入法有没有停用它、排第几个执行，以及启用（经授权对话框）、调整授权和删除。
  void build_script_detail(StackPanel const &page, pg::Package const &pack) {
    plugin_view_header(page, pack.name);
    const auto *grants = plugin_grants();
    const auto *grant = pg::script_grant(grants, pack.id);
    const auto state = pg::script_grant_state(pack, grant);
    const bool active = state == pg::ScriptGrantState::enabled;
    const auto requested = pg::sort_capabilities(pack.permissions);
    const auto granted = active ? pg::sort_capabilities(grant->granted) : std::vector<std::wstring>{};
    const auto running = plugin_running_scripts();
    const auto position = pg::running_position(running, pack.id);

    auto info = add_group(page, L"信息");
    plugin_summary_row(info, 0xE8EC, L"类型", L"", L"脚本插件（实验）");
    plugin_summary_row(info, 0xE946, L"版本", L"", pack.version);
    if (!pack.author.empty())
      plugin_summary_row(info, 0xE77B, L"作者", L"", pack.author);
    plugin_summary_row(info, 0xE8D7, L"许可证", L"", pack.license);
    if (pack.builtin)
      plugin_summary_row(info, 0xE7B8, L"来源", L"", std::wstring(pg::script_builtin_note));
    plugin_summary_row(info, 0xE928, L"内容指纹",
                       L"插件内容的哈希前 12 位；内容一变，指纹就变，之前的授权随之作废。",
                       pg::short_hash(pack.content_hash));
    if (!pack.description.empty())
      info.Children().Append(group_note(pack.description));

    auto hooks = add_group(page, L"在这些时候运行");
    std::vector<std::pair<std::wstring, std::wstring>> lines;
    for (auto const &hook : pack.hooks)
      lines.emplace_back(pg::hook_label(hook), L"");
    if (!lines.empty())
      plugin_line_card(hooks, lines);

    auto permissions = add_group(page, L"申请的权限");
    if (requested.empty())
      permissions.Children().Append(
          group_note(L"没有申请任何权限：只能知道发生了什么事件，读不到也改不了文字。"));
    for (auto const &capability : requested) {
      const auto label = pg::capability_label(capability);
      plugin_summary_row(permissions, 0xE72E, label.title, label.description,
                         pg::contains(granted, capability) ? L"已授予" : L"未授予");
    }

    // 提供的 / 指令：每条一行，右边是现在的状态；/ 指令关着时说明并给「打开 / 指令」，授权本身从不改这个开关。
    if (!pack.commands.empty()) {
      auto commands = add_group(page, L"提供的 / 指令");
      for (auto const &command : pack.commands)
        plugin_summary_row(commands, 0xE756, L"/" + command.trigger, command.title,
                           pg::script_command_status(pack, command.trigger, grants, plugin_catalog_.packages));
      if (!document_.Boolean(L"local_modes.command", false))
        plugin_action_block(
            commands, std::wstring(pg::script_command_mode_note),
            {button_control(L"打开 / 指令", [this] { enable_local_mode(L"command"); }, loaded_)});
    }

    auto status = add_group(page, L"状态");
    auto note =
        group_note(pg::script_status(pack, state, active ? pg::script_health(grants, pack) : nullptr));
    A11y::SetLiveSetting(note, Automation::Peers::AutomationLiveSetting::Polite);
    status.Children().Append(note);
    auto control = toggle_control(L"启用", active,
                                  [this, pack](bool on) {
                                    if (on)
                                      open_script_grant(pack, L"script-switch");
                                    else
                                      revoke_script(pack);
                                  },
                                  !plugins_busy_, false);
    add_row(status, 0xE7E8, L"启用",
            L"打开时先确认要授予的权限。关闭会撤销全部授权，输入法在下一次切换输入框时停止运行它。", control);
    FrameworkElement adjust{nullptr};
    if (active && !requested.empty()) {
      adjust = button_control(L"调整授权", [this, pack] { open_script_grant(pack, L"script-adjust"); },
                              !plugins_busy_);
      plugin_action_block(status, L"", {adjust});
    }

    FrameworkElement up{nullptr}, down{nullptr};
    if (active && pg::script_order_shown(position, running.size())) {
      const bool can_up = position > 0;
      const bool can_down = position >= 0 && static_cast<std::size_t>(position) + 1 < running.size();
      up = button_control(L"提前", [this, id = pack.id] { move_script(id, -1); }, !plugins_busy_ && can_up);
      down = button_control(L"推后", [this, id = pack.id] { move_script(id, 1); }, !plugins_busy_ && can_down);
      plugin_action_block(add_group(page, L"执行顺序"), pg::script_order_note(position, running.size()),
                          {up, down});
      // 挪到头或尾之后那个按钮不能按了，焦点落到另一个上。
      if (plugin_pending_focus_ == L"script-up" && !can_up)
        plugin_pending_focus_ = L"script-down";
      if (plugin_pending_focus_ == L"script-down" && !can_down)
        plugin_pending_focus_ = can_up ? L"script-up" : L"script-switch";
    }

    auto safety = add_group(page, L"安全");
    safety.Children().Append(group_note(
        L"脚本插件在输入法里的沙箱中运行，只能做你授予的事，读不到文件和网络，也不会在密码框里运行。"));
    safety.Children().Append(group_note(pg::script_residual_risk));

    // 随输入法分发的官方插件不能删除，同共享页。
    if (!pack.builtin) {
      auto remove = button_control(L"删除", [this, pack] { remove_plugin_pack(pack); }, !plugins_busy_);
      A11y::SetName(remove, hstring(L"删除" + pack.name));
      plugin_action_block(add_group(page, L""), L"删除后插件的文件会从本机移除，它的授权也一起去掉。", {remove});
    }

    // 授权对话框关上、撤销或换位之后，焦点回到刚才那个控件；它不在了（调整授权在停用后消失）就落到启用开关上。
    if (!indexing_ && plugin_pending_focus_.starts_with(L"script-")) {
      const auto target = plugin_pending_focus_;
      plugin_pending_focus_.clear();
      FrameworkElement chosen{nullptr};
      if (target == L"script-adjust")
        chosen = adjust;
      else if (target == L"script-up")
        chosen = up;
      else if (target == L"script-down")
        chosen = down;
      if (!chosen)
        chosen = control;
      focus_target_ = chosen;
    }
  }

  // ---- 屏幕键盘、语音、手写 ----

  // 共享的「屏幕键盘」页（packages/ui/src/settings/pages/screen-keyboard-page.tsx 和 touch-keyboard-geometry-section.tsx）在 Windows 上画四组：「屏幕键盘」（打开面板和可拖动的预览）、「尺寸」（高度、两种间距和恢复默认）、「工具栏」，以及指向「主题」的「外观」；提供手写的版本在「工具栏」和「外观」之间多一组「手写识别板」，即并进来的「手写输入」页（build_hand_page），与共享页 `handwritingOnScreenKeyboardPage` 的顺序相同。Windows 不是 `touch_toolbar_components` 的宿主，「工具栏」里只有顶部语音入口；「布局」「手势」两组和触屏反馈只在手机上画，这里也不画。Windows 有触屏的二合一设备，面板也写明「使用鼠标或触控方式」，所以高度和间距都留着（macOS 没有触屏，那里去掉了间距）。面板本身是输入工具，仍由共享应用打开。
  void build_osk_page(StackPanel const &page) {
    if (!indexing_) {
      osk_drag_.reset();
      osk_preview_ = nullptr;
      osk_key_slider_ = nullptr;
      osk_row_slider_ = nullptr;
    }

    auto keyboard = add_group(page, L"屏幕键盘");
    add_row(keyboard, 0xE92E, L"打开屏幕键盘", L"使用鼠标或触控方式输入文字与快捷按键",
            button_control(L"打开", [this] { open_shell(nav::shell_links::keyboard_panel); }));
    if (!indexing_)
      keyboard.Children().Append(make_osk_preview_card());

    // 滑块旁的数值与共享页的说明相同：高度是带符号的 dp，间距以十分之一 dp 存、显示一位小数。
    auto size = add_group(page, L"尺寸");
    size.Children().Append(group_note(
        L"只改变触屏键位的外观，不改变输入方案；也可以直接在上方预览上左右拖动调节键距、上下拖动调节行距。"));
    add_row(size, 0xE740, L"键盘高度", L"",
            formatted_slider_control(
                L"键盘高度", -12, 48,
                document_.Number(L"touch_keyboard_height_adjustment", 0),
                [this](double value) {
                  set_touch_geometry(L"touch_keyboard_height_adjustment", 0, value);
                },
                [](double value) {
                  return std::wstring(value > 0 ? L"+" : L"") + number_text(value) + L" dp";
                },
                true));
    auto key_slider = formatted_slider_control(
        L"按键间距", 30, 60, document_.Number(L"touch_key_spacing_tenths", 60),
        [this](double value) { set_touch_geometry(L"touch_key_spacing_tenths", 60, value); },
        tenths_text, true);
    add_row(size, 0xE8E4, L"按键间距", L"", key_slider);
    auto row_slider = formatted_slider_control(
        L"行间距", 40, 100, document_.Number(L"touch_row_spacing_tenths", 70),
        [this](double value) { set_touch_geometry(L"touch_row_spacing_tenths", 70, value); },
        tenths_text, true);
    add_row(size, 0xE8FD, L"行间距", L"", row_slider);
    if (!indexing_) {
      osk_key_slider_ = key_slider.Children().GetAt(0).as<Slider>();
      osk_row_slider_ = row_slider.Children().GetAt(0).as<Slider>();
    }
    // 恢复默认也管下面「工具栏」组的顶部语音入口，所以和共享页一样放在它重置的第一组末尾；说明只列这一页看得到的几项，与共享页的 `touchKeyboardResetSubjects` 给 Windows 的相同。
    auto reset = button_control(L"恢复默认", [this] { reset_screen_keyboard(); }, loaded_);
    A11y::SetName(reset, L"恢复屏幕键盘默认设置");
    add_row(size, 0xE777, L"恢复默认", L"高度、间距和顶部语音入口回到默认", reset);

    auto toolbar = add_group(page, L"工具栏");
    bool_row(toolbar, 0xE720, L"顶部语音入口", L"在键盘顶部显示一个语音按钮，点按开始语音输入",
             L"touch_voice_shortcut", false);

    if constexpr (MSIME_EDITION_HANDWRITING != 0)
      build_hand_page(page);

    // 屏幕键盘跟随「主题」页的颜色模式和主题，没有自己的明暗（共享页在跟随颜色模式的宿主上同样这样说），按钮落到「颜色模式」那一行。
    auto look = add_group(page, L"外观");
    add_row(look, 0xE790, L"屏幕键盘外观", L"屏幕键盘跟随「主题」页的颜色模式和主题",
            button_control(L"前往", [this] {
              focus_row_ = L"颜色模式";
              navigate("themes", true);
            }));
  }

  static std::wstring tenths_text(double tenths) {
    const auto value = static_cast<long long>(std::llround(tenths));
    return std::to_wstring(value / 10) + L"." + std::to_wstring(value % 10) + L" dp";
  }

  // The preview and the surface that takes the drag: as on the shared page, dragging sideways sets the key gap and dragging up or down the row gap, 1.8 pixels to a tenth, from the value it had when the press began; the first 4 pixels only decide which.
  FrameworkElement make_osk_preview_card() {
    using Microsoft::UI::Xaml::Input::PointerRoutedEventArgs;
    StackPanel block;
    block.Spacing(8);
    block.Padding(Thickness{25, 14, 25, 18});
    block.Children().Append(make_text(L"预览", 12, palette_.sub));
    Border surface;
    surface.Background(brush(transparent_color()));
    // A touch drag moves the gaps, not the page: the scroll viewer would otherwise take it as a pan.
    surface.ManipulationMode(Microsoft::UI::Xaml::Input::ManipulationModes::TranslateX |
                             Microsoft::UI::Xaml::Input::ManipulationModes::TranslateY);
    A11y::SetName(surface, L"拖动预览调整键盘间距");
    surface.PointerPressed([this](Inspectable const &sender, PointerRoutedEventArgs const &args) {
      begin_osk_drag(sender.as<UIElement>(), args);
    });
    surface.PointerMoved([this](Inspectable const &, PointerRoutedEventArgs const &args) {
      move_osk_drag(args);
    });
    auto release = [this](Inspectable const &sender, PointerRoutedEventArgs const &args) {
      if (!osk_drag_ || osk_drag_->pointer != args.Pointer().PointerId())
        return;
      osk_drag_.reset();
      sender.as<UIElement>().ReleasePointerCapture(args.Pointer());
    };
    surface.PointerReleased(release);
    surface.PointerCanceled(release);
    surface.PointerCaptureLost([this](Inspectable const &, PointerRoutedEventArgs const &args) {
      if (osk_drag_ && osk_drag_->pointer == args.Pointer().PointerId())
        osk_drag_.reset();
    });
    osk_preview_ = surface;
    draw_osk_preview();
    block.Children().Append(surface);
    auto card = make_card();
    card.Child(block);
    A11y::SetName(card, L"屏幕键盘预览");
    return card;
  }

  void begin_osk_drag(UIElement const &surface,
                      Microsoft::UI::Xaml::Input::PointerRoutedEventArgs const &args) {
    if (!loaded_)
      return;
    const auto point = args.GetCurrentPoint(nullptr);
    if (args.Pointer().PointerDeviceType() == Microsoft::UI::Input::PointerDeviceType::Mouse &&
        !point.Properties().IsLeftButtonPressed())
      return;
    osk_drag_ = GeometryDrag{args.Pointer().PointerId(),
                             point.Position().X,
                             point.Position().Y,
                             document_.Number(L"touch_key_spacing_tenths", 60),
                             document_.Number(L"touch_row_spacing_tenths", 70),
                             GeometryDrag::Axis::none};
    surface.CapturePointer(args.Pointer());
    args.Handled(true);
  }

  void move_osk_drag(Microsoft::UI::Xaml::Input::PointerRoutedEventArgs const &args) {
    if (!osk_drag_ || osk_drag_->pointer != args.Pointer().PointerId() || !loaded_)
      return;
    const auto position = args.GetCurrentPoint(nullptr).Position();
    const double dx = position.X - osk_drag_->x;
    const double dy = position.Y - osk_drag_->y;
    if (osk_drag_->axis == GeometryDrag::Axis::none) {
      if (std::abs(dx) + std::abs(dy) < 4)
        return;
      osk_drag_->axis =
          std::abs(dy) >= std::abs(dx) ? GeometryDrag::Axis::row : GeometryDrag::Axis::key;
    }
    args.Handled(true);
    const bool row = osk_drag_->axis == GeometryDrag::Axis::row;
    // Math.round: halves go up.
    const double value =
        std::floor((row ? osk_drag_->row : osk_drag_->key) + (row ? dy : dx) * 10 / 18 + 0.5);
    if (row)
      set_touch_geometry(L"touch_row_spacing_tenths", 70, std::clamp(value, 40.0, 100.0),
                         osk_row_slider_);
    else
      set_touch_geometry(L"touch_key_spacing_tenths", 60, std::clamp(value, 30.0, 60.0),
                         osk_key_slider_);
  }

  // Writes one of the keyboard's sizes the way the other sliders do, into the document at once and to disk when it rests, and redraws the preview. A drag moves the slider along; the slider's own change then finds the value already written.
  void set_touch_geometry(std::wstring const &key, double fallback, double value,
                          Slider const &slider = nullptr) {
    if (document_.Number(key, fallback) == value)
      return;
    document_.SetNumber(key, value);
    schedule_save();
    draw_osk_preview();
    if (slider && slider.Value() != value)
      slider.Value(value);
  }

  // 共享页的 `resetTouchKeyboardSettings`（use-touch-keyboard-settings-reset.ts）：确认后把五项从文档里删掉而不是写入当前默认值，宿主以后改默认值时也跟着变。确认文案照 `touchKeyboardResetSubjects` 只说这一页看得到的高度、间距和顶部语音入口；`touch_toolbar` 照样删掉，与共享页删的键相同，只是 Windows 页面上没有工具栏按钮的开关，不在文案里提它。
  fire_and_forget reset_screen_keyboard() {
    auto strong = get_strong();
    if (!loaded_)
      co_return;
    if (!co_await confirm(L"恢复屏幕键盘默认值", L"高度、间距和顶部语音入口都会回到默认。",
                          L"恢复"))
      co_return;
    flush_pending();
    for (const auto *key : {L"touch_key_spacing_tenths", L"touch_row_spacing_tenths",
                            L"touch_keyboard_height_adjustment", L"touch_voice_shortcut",
                            L"touch_toolbar"})
      document_.Remove(key);
    if (commit())
      show_notice(L"屏幕键盘设置已恢复默认。", InfoBarSeverity::Success);
  }

  // 预览的明暗：屏幕键盘跟随颜色模式（surface_themes_follow），不读存下的 screen_keyboard_theme，与共享页的 keyboardPreviewTheme 一致。
  bool keyboard_preview_dark() const {
    const auto theme = document_.String(L"theme", L"system");
    return theme == L"system" ? system_dark_ : theme != L"light";
  }

  // The theme the preview paints (`keyboardThemeId` over `offeredGlobalTheme`): a global theme this build does not offer reads as 跟随系统, and the custom theme draws its base's keyboard until it carries a keyboard design of its own, which then paints every key.
  KeyboardArt keyboard_art() {
    const bool dark = keyboard_preview_dark();
    auto offered = [this](std::wstring const &id) {
      return std::any_of(themes_.begin(), themes_.end(),
                         [&id](ThemeEntry const &entry) { return entry.id == id; });
    };
    std::wstring skin = document_.String(L"global_theme", L"system");
    if (!offered(skin))
      skin = L"system";
    std::optional<JsonObject> design;
    if (skin == L"custom") {
      if (const auto keyboard = document_.Value(L"custom_theme.keyboard");
          keyboard && keyboard.ValueType() == JsonValueType::Object)
        design = keyboard.GetObject();
      else
        skin = document_.String(L"custom_theme.base", L"system");
    }

    KeyboardArt art;
    art.colors = system_keyboard(dark);
    if (skin == L"native") {
      art.colors = native_keyboard(dark);
      art.corner_radius = 6;
    }
    for (const auto &entry : themes_)
      if (entry.id == skin && entry.keyboard)
        art.colors = *entry.keyboard;
    if (!design)
      return art;

    // The design's required fields fall back to the editor's default design should one be missing; its optional ones, stored as null when unset, keep the look above.
    const auto &source = *design;
    auto number = [&source](const wchar_t *key) -> std::optional<double> {
      const auto value = source.GetNamedValue(key, JsonValue::CreateNullValue());
      if (value.ValueType() != JsonValueType::Number)
        return std::nullopt;
      return value.GetNumber();
    };
    auto flag = [&source](const wchar_t *key) {
      const auto value = source.GetNamedValue(key, JsonValue::CreateNullValue());
      return value.ValueType() == JsonValueType::Boolean && value.GetBoolean();
    };
    auto word = [&source](const wchar_t *key) -> std::optional<std::wstring> {
      const auto value = source.GetNamedValue(key, JsonValue::CreateNullValue());
      if (value.ValueType() != JsonValueType::String)
        return std::nullopt;
      return std::wstring(value.GetString().c_str());
    };
    const double action = number(L"actionBackground").value_or(0x185C47);
    art.colors = KeyboardColors{skin_color(number(L"background").value_or(0xE8F0EB)),
                                skin_color(number(L"keyBackground").value_or(0xFFFFFF)),
                                skin_color(number(L"keyForeground").value_or(0x17251D)),
                                skin_color(number(L"accent").value_or(0x185C47)),
                                skin_color(action),
                                readable_skin_text(action)};
    art.corner_radius = number(L"cornerRadius").value_or(8);
    art.border_width = number(L"borderWidth").value_or(0);
    if (const auto border = number(L"customBorderColor"))
      art.border_color = skin_color(*border);
    art.border_opacity = 1;
    art.monospaced = flag(L"monospaced");
    art.pattern = static_cast<int>(std::clamp(number(L"pattern").value_or(0), 0.0, 3.0));
    art.pattern_opacity = number(L"patternOpacity").value_or(0.15);
    art.key_shape = word(L"keyShape").value_or(L"rounded");
    art.key_material = word(L"keyMaterial").value_or(L"flat");
    art.key_opacity = number(L"keyOpacity").value_or(1);
    if (const auto end = number(L"gradientEnd"))
      art.gradient_end = skin_color(*end);
    art.gradient_horizontal = flag(L"gradientHorizontal");
    art.photo = word(L"photo").value_or(L"");
    art.photo_shade = number(L"photoShade").value_or(0.25);
    art.photo_position = number(L"photoPosition").value_or(0.5);
    return art;
  }

  void draw_osk_preview() {
    if (osk_preview_)
      osk_preview_.Child(make_keyboard_art());
  }

  // The shared preview's desktop keyboard on its 1100-unit canvas, whose height follows 键盘高度 and whose gaps follow the two spacings (4 units at the defaults), scaled down to at most 760 pixels wide as the shared page shows it.
  FrameworkElement make_keyboard_art() {
    const auto art = keyboard_art();
    const auto &rows = desktop_keyboard_rows();
    const double key_spacing =
        std::clamp(document_.Number(L"touch_key_spacing_tenths", 60) / 10, 3.0, 6.0);
    const double row_spacing =
        std::clamp(document_.Number(L"touch_row_spacing_tenths", 70) / 10, 4.0, 10.0);
    const double width = 1100;
    const double height =
        400 + std::clamp(document_.Number(L"touch_keyboard_height_adjustment", 0), -12.0, 48.0);
    const double inset = 7;
    const double rows_top = 28;
    const double key_gap = 4 + key_spacing - 6;
    const double row_gap = 4 + row_spacing - 7;
    const double count = static_cast<double>(rows.size());
    const double key_height = (height - 28 - 7 - row_gap * (count - 1)) / count;

    Canvas canvas;
    canvas.Width(width);
    canvas.Height(height);
    RectangleGeometry clip;
    clip.Rect({0, 0, static_cast<float>(width), static_cast<float>(height)});
    canvas.Clip(clip);
    auto place = [&canvas](UIElement const &element, double left, double top) {
      Canvas::SetLeft(element, left);
      Canvas::SetTop(element, top);
      canvas.Children().Append(element);
    };

    Shapes::Rectangle backdrop;
    backdrop.Width(width);
    backdrop.Height(height);
    backdrop.RadiusX(8);
    backdrop.RadiusY(8);
    if (art.gradient_end) {
      LinearGradientBrush fill;
      fill.StartPoint(point_at(0, 0));
      fill.EndPoint(art.gradient_horizontal ? point_at(1, 0) : point_at(0, 1));
      GradientStop from;
      from.Color(art.colors.background);
      from.Offset(0);
      GradientStop to;
      to.Color(*art.gradient_end);
      to.Offset(1);
      fill.GradientStops().Append(from);
      fill.GradientStops().Append(to);
      backdrop.Fill(fill);
    } else {
      backdrop.Fill(brush(art.colors.background));
    }
    place(backdrop, 0, 0);

    if (!art.photo.empty()) {
      if (art.photo != keyboard_photo_source_ || !keyboard_photo_) {
        keyboard_photo_source_ = art.photo;
        keyboard_photo_ = Imaging::BitmapImage();
        show_keyboard_photo(keyboard_photo_, art.photo);
      }
      ImageBrush picture;
      picture.ImageSource(keyboard_photo_);
      picture.Stretch(Stretch::UniformToFill);
      const auto anchor = art.photo_position < 0.34   ? 0
                          : art.photo_position > 0.66 ? 2
                                                      : 1;
      picture.AlignmentX(anchor == 0   ? AlignmentX::Left
                         : anchor == 2 ? AlignmentX::Right
                                       : AlignmentX::Center);
      picture.AlignmentY(anchor == 0   ? AlignmentY::Top
                         : anchor == 2 ? AlignmentY::Bottom
                                       : AlignmentY::Center);
      Shapes::Rectangle photo;
      photo.Width(width);
      photo.Height(height);
      photo.Fill(picture);
      place(photo, 0, 0);
      Shapes::Rectangle shade;
      shade.Width(width);
      shade.Height(height);
      shade.Fill(brush(faded(opaque(0, 0, 0), art.photo_shade)));
      place(shade, 0, 0);
    }

    if (art.pattern != 0)
      place(keyboard_pattern(art.pattern, faded(art.colors.accent, art.pattern_opacity), width,
                             height),
            0, 0);

    // A monospaced design sets the font of every label, the caption included.
    const auto font = Media::FontFamily(art.monospaced ? L"Consolas" : L"Segoe UI");
    Grid header;
    header.Height(28);
    auto title = make_text(L"Touch keyboard", 12, art.colors.accent);
    title.FontFamily(font);
    title.VerticalAlignment(VerticalAlignment::Center);
    header.Children().Append(title);
    place(header, 10, 0);

    // The desktop panel floats in a window the user can close, so the preview draws its close glyph.
    PathGeometry cross;
    for (const auto &[from_x, to_x] : {std::pair{1075.0, 1085.0}, std::pair{1085.0, 1075.0}}) {
      PathFigure stroke;
      stroke.IsFilled(false);
      stroke.StartPoint(point_at(from_x, 9));
      LineSegment segment;
      segment.Point(point_at(to_x, 19));
      stroke.Segments().Append(segment);
      cross.Figures().Append(stroke);
    }
    Shapes::Path close;
    close.Data(cross);
    close.Stroke(brush(art.colors.foreground));
    close.StrokeThickness(2);
    place(close, 0, 0);

    const bool raised = art.key_material == L"raised";
    const bool glass = art.key_material == L"glass";
    const bool paper = art.key_material == L"paper";
    auto material = [&art, glass](Color middle) {
      LinearGradientBrush fill;
      fill.StartPoint(point_at(0, 0));
      fill.EndPoint(point_at(0, 1));
      const std::array<std::pair<double, Color>, 3> stops{{
          {0, faded(opaque(255, 255, 255), glass ? 0.24 : 0.13)},
          {0.48, middle},
          {1, faded(opaque(0, 0, 0), glass ? 0.03 : 0.1)},
      }};
      for (const auto &[offset, color] : stops) {
        GradientStop stop;
        stop.Offset(offset);
        stop.Color(color);
        fill.GradientStops().Append(stop);
      }
      return Brush(fill);
    };
    // A key's fill opacity applies over its material, so a translucent design is fainter still on the glass and raised gradients, as in the shared preview.
    Brush key_fill = raised || glass ? material(faded(art.colors.key, art.key_opacity))
                                     : Brush(brush(art.colors.key));
    key_fill.Opacity(art.key_opacity);
    Brush action_fill =
        raised || glass ? material(art.colors.action) : Brush(brush(art.colors.action));
    Brush border = art.border_width > 0
                       ? Brush(brush(faded(art.border_color.value_or(art.colors.accent),
                                           art.border_opacity)))
                       : Brush(nullptr);

    for (size_t index = 0; index < rows.size(); ++index) {
      const auto &row = rows[index];
      double total = 0;
      for (const auto &item : row)
        total += item.weight;
      const double available = width - inset * 2 - key_gap * static_cast<double>(row.size() - 1);
      const double top = rows_top + static_cast<double>(index) * (key_height + row_gap);
      double x = inset;
      for (const auto &item : row) {
        const double key_width = available * item.weight / total;
        const double left = x;
        x += key_width + key_gap;
        const bool action = action_key(item.label);
        if (raised) {
          PathGeometry depth;
          depth.Figures().Append(
              key_outline(left, top + 3, key_width, key_height - 3, art.key_shape,
                          art.corner_radius));
          Shapes::Path under;
          under.Data(depth);
          under.Fill(brush(action ? art.colors.action : art.colors.key));
          under.Opacity(0.72);
          place(under, 0, 0);
        }
        PathGeometry outline;
        outline.Figures().Append(key_outline(left, top, key_width,
                                             key_height - (raised ? 3 : 0), art.key_shape,
                                             art.corner_radius));
        Shapes::Path face;
        face.Data(outline);
        face.Fill(action ? action_fill : key_fill);
        if (border) {
          face.Stroke(border);
          face.StrokeThickness(art.border_width);
        }
        place(face, 0, 0);
        if (paper) {
          PathGeometry ruled;
          ruled.Figures().Append(key_outline(left, top, key_width, key_height, art.key_shape,
                                             art.corner_radius));
          for (const double line_y : {10.0, 18.0, 26.0}) {
            PathFigure rule;
            rule.IsFilled(false);
            rule.StartPoint(point_at(left + 3, top + line_y));
            LineSegment segment;
            segment.Point(point_at(left + key_width - 3, top + line_y));
            rule.Segments().Append(segment);
            ruled.Figures().Append(rule);
          }
          Shapes::Path lines;
          lines.Data(ruled);
          lines.Stroke(brush(faded(opaque(0, 0, 0), 0.08)));
          lines.StrokeThickness(0.5);
          place(lines, 0, 0);
        }
        Grid cell;
        cell.Width(key_width);
        cell.Height(key_height);
        auto label = make_text(item.label, item.label.size() == 1 ? 15 : 12,
                               action ? art.colors.action_foreground : art.colors.foreground);
        label.FontFamily(font);
        label.TextWrapping(TextWrapping::NoWrap);
        label.HorizontalAlignment(HorizontalAlignment::Center);
        label.VerticalAlignment(VerticalAlignment::Center);
        cell.Children().Append(label);
        place(cell, left, top);
      }
    }

    Viewbox box;
    box.Stretch(Stretch::Uniform);
    box.MaxWidth(760);
    box.HorizontalAlignment(HorizontalAlignment::Center);
    box.Child(canvas);
    A11y::SetName(box, L"屏幕键盘完整布局预览");
    return box;
  }

  // The design's background pattern (`Pattern` in screen-keyboard-preview.tsx), tiled from the top-left corner: 1 a dot every 16 units, 2 a hairline grid every 20, 3 two waves every 48. Each tile clips what it draws, so the waves are drawn tile by tile and the grid's half-width lines are thin bands.
  UIElement keyboard_pattern(int pattern, Color color, double width, double height) {
    if (pattern == 1) {
      GeometryGroup dots;
      for (double y = 0; y < height; y += 16)
        for (double x = 0; x < width; x += 16) {
          EllipseGeometry dot;
          dot.Center(point_at(x + 8.75, y + 8.75));
          dot.RadiusX(0.75);
          dot.RadiusY(0.75);
          dots.Children().Append(dot);
        }
      Shapes::Path path;
      path.Data(dots);
      path.Fill(brush(color));
      return path;
    }
    if (pattern == 2) {
      GeometryGroup grid;
      grid.FillRule(FillRule::Nonzero);
      for (double x = 0; x < width; x += 20) {
        RectangleGeometry band;
        band.Rect({static_cast<float>(x), 0, 0.25f, static_cast<float>(height)});
        grid.Children().Append(band);
      }
      for (double y = 0; y < height; y += 20) {
        RectangleGeometry band;
        band.Rect({0, static_cast<float>(y), static_cast<float>(width), 0.25f});
        grid.Children().Append(band);
      }
      Shapes::Path path;
      path.Data(grid);
      path.Fill(brush(color));
      return path;
    }
    Canvas tiles;
    for (double y = 0; y < height; y += 48)
      for (double x = 0; x < width; x += 48) {
        PathGeometry waves;
        for (const double offset : {0.0, 24.0}) {
          PathFigure wave;
          wave.IsFilled(false);
          wave.StartPoint(point_at(-12, 42 + offset));
          BezierSegment curve;
          curve.Point1(point_at(4, 0 + offset));
          curve.Point2(point_at(27, 65 + offset));
          curve.Point3(point_at(60, 1 + offset));
          wave.Segments().Append(curve);
          waves.Figures().Append(wave);
        }
        Shapes::Path path;
        path.Data(waves);
        path.Stroke(brush(color));
        path.StrokeThickness(2);
        Canvas tile;
        tile.Width(48);
        tile.Height(48);
        RectangleGeometry bounds;
        bounds.Rect({0, 0, 48, 48});
        tile.Clip(bounds);
        tile.Children().Append(path);
        Canvas::SetLeft(tile, x);
        Canvas::SetTop(tile, y);
        tiles.Children().Append(tile);
      }
    return tiles;
  }

  // ---- 语音输入 ----

  // 本地识别模型目录里的一个模型（msime_client_voice_local_models 的一项）和「不联网安装」要的一个文件。
  struct LocalModelFile {
    std::wstring name;
    std::wstring url;
    double size = 0;
  };
  struct LocalModel {
    std::wstring id, title, description;
    std::vector<std::wstring> languages;
    bool streaming = false;
    bool is_default = false;
    bool installed = false;
    std::wstring path;
    double installed_size = 0, archive_size = 0, memory = 0;
    std::wstring license_spdx, license_source, license_terms, license_notice;
    std::vector<LocalModelFile> import_files;
  };
  struct LocalModelProgress {
    std::wstring stage;
    double downloaded = 0;
    double total = 0;
  };
  struct CaptureDevice {
    std::wstring backend, id, label;
  };

  // 对齐共享桌面页的内容归属：识别与结果提交、快捷键、录音行为与设备、识别服务接入、文本润色。原生页继续使用直列，逐项搜索仍由现有索引负责。输入框就地更新以保留焦点，服务切换后重画页面。
  void build_voice_page(StackPanel const &page) {
    if (!indexing_) {
      credential_test_views_.clear();
      voice_sync_.clear();
      local_model_views_.clear();
    }
    const auto provider = document_.String(L"voice_input.asr_provider", L"doubao");
    const bool system_voice = provider == L"system";
    const bool local_voice = provider == L"local";
    const bool service_voice = !system_voice && !local_voice;
    const bool doubao = provider == L"doubao";
    const std::wstring auth_mode(
        vs::doubao_auth_mode(document_.String(L"voice_input.doubao_auth_mode", L"")));

    // 页首一组（VoiceInputIntroSection）：本地识别和 Windows 系统识别各是一段说明，其余服务是打开语音面板的一行，面板仍由 MSIME.exe 显示。
    if (local_voice || indexing_) {
      auto intro = add_group(page, L"本地识别");
      intro.Children().Append(group_note(
          L"录音和识别都在这台设备上完成，音频不会离开本机，也不需要任何 API Key。在下方下载一个模型并点击“使用”即可生效；下载只会连接 GitHub 或你配置的镜像。你的用户词库会作为热词提高专有名词的识别率。可选的文本润色仍会调用你配置的云服务。"));
    }
    if (system_voice || indexing_) {
      auto intro = add_group(page, L"Windows 系统语音");
      intro.Children().Append(group_note(
          L"用下方语音快捷键或悬浮工具栏开始语音输入，由 Windows 自带的语音识别在这台电脑上完成识别，音频不会离开本机，也不需要识别 API Key。需要先在 Windows 设置 › 时间和语言 › 语言和区域 中为识别语言安装“语音识别”。可选文本润色仍使用你配置的云服务。"));
    }
    if (service_voice || indexing_) {
      auto intro = add_group(page, L"语音面板");
      shell_row(intro, 0xE720, L"打开语音输入", L"在本机录音，音频发送给下方选择的识别服务转写",
                L"打开", nav::shell_links::voice_panel);
    }

    // 「识别」组照共享页桌面主页面的「识别」：开关、服务、识别语言、豆包的自动添加标点和边说边显示。豆包的识别请求不读识别语言（DoubaoAsrClient 不发它），选豆包时不画这一行，值照旧保存、照旧上云。
    auto recognition = add_group(page, L"识别");
    add_row(recognition, 0xE720, L"语音输入", L"使用语音识别将录音转换为文字",
            toggle_control(L"启用语音输入", document_.Boolean(L"voice_input.enabled", true),
                           [this](bool on) {
                             change([&](PreferencesDocument &doc) {
                               doc.SetBoolean(L"voice_input.enabled", on);
                             }, false);
                           }));
    std::vector<Option> providers;
    for (auto const &choice : vs::asr_provider_choices(voice_provider_catalog().asr))
      providers.push_back({std::wstring(choice.value), std::wstring(choice.label)});
    auto provider_combo = select_control(
        L"识别服务", std::move(providers), provider,
        [this](std::wstring const &value) { select_asr_provider(value); });
    provider_combo.Width(200);
    add_row(recognition, 0xE753, L"识别服务", L"", provider_combo);
    if (!doubao || indexing_)
      add_row(recognition, 0xE8C1, L"识别语言",
              system_voice ? L"选择明确的语言代码，例如 zh-CN、en-US；可用语言由系统决定" : L"",
              language_input(document_.String(L"voice_input.language", L""), system_voice));
    if (doubao || indexing_)
      bool_row(recognition, 0xE8D2, L"自动添加标点", L"", L"voice_input.doubao_enable_punc", true);
    bool_row(recognition, 0xE8D2, L"边说边显示", L"识别服务支持时显示实时识别片段",
             L"voice_input.stream_inline_preedit", true);

    // 「识别结果」（VoiceRecognitionResultSection）：边说边显示已经移到「识别」组，这里只剩结果提交策略。
    auto result = add_group(page, L"识别结果");
    select_row(result, 0xE8C8, L"结果提交策略", L"由当前桌面宿主决定如何把识别结果交给前台窗口",
               L"voice_input.commit_mode",
               {{L"tsf", L"输入法会话"}, {L"sendinput", L"系统按键"}, {L"ctrl_v", L"剪贴板粘贴"}},
               L"tsf");


    build_voice_hotkey_group(page);

    // 「录音行为」只剩提示音总开关和静音：宿主不再分别读开始、结束提示音（`voice_cue_switches` 为 false）。
    auto recording = add_group(page, L"录音行为");
    const std::array<std::pair<const wchar_t *, const wchar_t *>, 2> sounds{{
        {L"sound_enabled", L"语音提示音"},
        {L"mute_system_audio", L"录音时静音其他声音"},
    }};
    std::vector<Check> sound_checks;
    sound_checks.reserve(sounds.size());
    for (const auto &[id, label] : sounds) {
      const std::wstring key = std::wstring(L"voice_input.") + id;
      sound_checks.push_back({label, document_.Boolean(key, false),
                              [this, key](bool on) {
                                change([&](PreferencesDocument &doc) {
                                  doc.SetBoolean(key, on);
                                }, false);
                              }});
    }
    add_row(recording, 0xE767, L"录音行为", L"录音期间的提示音与静音由输入法在本机处理",
            nullptr, checks_panel(std::move(sound_checks), 200));

    build_capture_device_group(page);

    // 「识别服务配置」只负责服务接入与测试；共享页在「服务配置」下钻显示，原生页接在录音设备后面。
    if (!system_voice || indexing_) {
      auto config = add_group(page, L"识别服务配置");
      if (service_voice || indexing_)
        build_asr_service_rows(config, provider, doubao, auth_mode);
      if (local_voice || indexing_)
        build_local_model_rows(config);
      if (vs::is_asr_service_provider(voice_provider_catalog().asr, provider) || indexing_) {
        auto silence = make_text(
            L"测试会向当前服务发送一秒合成静音，不使用麦克风；服务可能计入 API 用量。", 12,
            palette_.sub);
        silence.Margin(Thickness{4, 8, 4, 4});
        config.Children().Append(silence);
        add_credential_test(config, L"voice.asr",
                            doubao ? L"测试豆包识别配置" : L"测试语音识别配置",
                            [this] { return asr_test_config(); },
                            [this] {
                              const auto current = document_.String(L"voice_input.asr_provider", L"doubao");
                              return vs::asr_test_disabled(
                                  current,
                                  vs::doubao_auth_mode(
                                      document_.String(L"voice_input.doubao_auth_mode", L"")),
                                  document_.String(L"voice_input.asr_token", L""),
                                  document_.String(L"voice_input.asr_app_key", L""));
                            });
      }
    }


    build_polish_group(page);
  }

  // 「语音快捷键」（VoiceHotkeysSection）：三个长按录音的组合键合成一个「长按录音」选择，末项「不使用」，宿主同一时刻只认一个。以前可以同时打开几个，这样的配置在用户改动之前照旧显示第一个，打开页面时不改写存储；换一项时三个一起写。空格锁定固定为开（宿主不再读 hotkey_hold_space_lock），不给开关。Ctrl+F9 收在「更多选项」里，与共享页一致。
  void build_voice_hotkey_group(StackPanel const &page) {
    auto shortcuts = add_group(page, L"语音快捷键");
    shortcuts.Children().Append(group_note(
        L"输入法运行时全局生效。长按快捷键录音，松开结束；按住期间按空格锁定录音，锁定后再按一次快捷键或点 ✓ 结束，Escape 或 ✗ 取消。Ctrl+F9 按一次开始、再按一次结束。"));
    // document_.Boolean 在用户没存时取 client-core 的出厂值（defaults_），与 HoldKey::fallback 相同。
    std::map<std::wstring_view, bool> stored;
    for (auto const &hold : vs::hold_keys)
      stored[hold.key] =
          document_.Boolean(L"voice_input." + std::wstring(hold.key), hold.fallback);
    std::vector<Option> holds;
    for (auto const &hold : vs::hold_keys)
      holds.push_back({std::wstring(hold.key), std::wstring(hold.label)});
    holds.push_back({std::wstring(vs::no_hold_key), L"不使用"});
    add_row(shortcuts, 0xEDA7, L"长按录音", L"",
            select_control(L"长按录音", std::move(holds),
                           std::wstring(vs::selected_hold_key(stored)),
                           [this](std::wstring const &value) {
                             change([&](PreferencesDocument &doc) {
                               for (auto const &[key, on] : vs::hold_key_patch(value))
                                 doc.SetBoolean(L"voice_input." + std::wstring(key), on);
                             }, true);
                           }));
    Expander more;
    more.Header(box_value(L"更多选项"));
    more.HorizontalAlignment(HorizontalAlignment::Stretch);
    more.HorizontalContentAlignment(HorizontalAlignment::Stretch);
    more.IsExpanded(voice_hotkeys_more_open_ || focus_row_ == L"Ctrl+F9 切换语音");
    more.Expanding([this](Expander const &, ExpanderExpandingEventArgs const &) {
      voice_hotkeys_more_open_ = true;
    });
    more.Collapsed([this](Expander const &, ExpanderCollapsedEventArgs const &) {
      voice_hotkeys_more_open_ = false;
    });
    StackPanel rows;
    rows.Spacing(4);
    bool_row(rows, 0xEDA7, L"Ctrl+F9 切换语音", L"", L"voice_input.hotkey_ctrl_f9", true);
    more.Content(rows);
    shortcuts.Children().Append(more);
  }

  void redraw_voice() {
    if (!closed_ && current_page_ == "voice")
      render_page(true);
  }

  // 在当前事件处理完后再 sync_voice：下拉框不在自己的 SelectionChanged 里改选中项。
  void resync_voice() {
    DispatcherQueue().TryEnqueue([weak = get_weak()] {
      if (auto self = weak.get(); self && !self->closed_)
        self->sync_voice();
    });
  }

  // 语音页上随输入内容变化的控件：下拉框的选中项和两个「测试配置」按钮。
  void sync_voice() {
    for (auto const &update : voice_sync_)
      update();
    refresh_credential_test(L"voice.asr");
    refresh_credential_test(L"voice.polish");
  }

  // 语音页的一个文字输入框（共享页的 TextInputRow、EndpointSettingRow、SecretSettingRow）。每输入一个字就写进设置，停下后保存；accepted 不认可的值不写入，输入框标红。
  void voice_text_row(StackPanel const &group, wchar_t glyph, std::wstring const &title,
                      std::wstring const &subtitle, std::wstring const &label, std::wstring key,
                      bool secret = false, std::wstring const &placeholder = L"",
                      int32_t max_length = 0,
                      std::function<bool(std::wstring const &)> accepted = nullptr) {
    add_row(group, glyph, title, subtitle,
            voice_text_input(label, key, secret, placeholder, max_length, std::move(accepted)));
  }

  FrameworkElement voice_text_input(std::wstring const &label, std::wstring key, bool secret,
                                    std::wstring const &placeholder, int32_t max_length,
                                    std::function<bool(std::wstring const &)> accepted) {
    auto input = credential_input(
        label, document_.String(key, L""), secret, placeholder, true,
        [this, key, accepted = std::move(accepted)](std::wstring const &value) {
          const bool saved = !accepted || accepted(value);
          if (saved)
            write_credential(key, value);
          sync_voice();
          return saved;
        });
    if (max_length > 0)
      if (auto box = input.try_as<TextBox>())
        box.MaxLength(max_length);
    return input;
  }

  // 识别语言（共享页带 datalist 的 TextInputRow，最多 64 个字符）：可以直接输入，也可以从建议里选。刚点进去时列出全部建议，打字后只列名字或代码里含有输入内容的；选中一项填入它的代码。每输入一个字就写进设置，停下后保存。
  FrameworkElement language_input(std::wstring const &value, bool system_voice) {
    AutoSuggestBox input;
    input.Width(240);
    input.Text(hstring(value));
    input.UpdateTextOnSelect(false);
    input.IsEnabled(loaded_);
    A11y::SetName(input, L"识别语言");
    auto entries = std::make_shared<std::vector<std::pair<std::wstring, std::wstring>>>();
    for (auto const &choice : vs::language_choices(system_voice))
      entries->push_back(
          {std::wstring(choice.label) + L" · " + std::wstring(choice.value), std::wstring(choice.value)});
    auto search = std::make_shared<bool>(false);
    auto update_menu = [entries, search](AutoSuggestBox const &field) {
      const auto typed = lowercase(std::wstring(field.Text().c_str()));
      std::vector<Inspectable> items;
      for (auto const &[shown, code] : *entries)
        if (!*search || lowercase(shown).find(typed) != std::wstring::npos)
          items.push_back(box_value(hstring(shown)));
      const bool any = !items.empty();
      field.ItemsSource(single_threaded_vector<Inspectable>(std::move(items)));
      field.IsSuggestionListOpen(any);
    };
    auto write = [this](std::wstring const &language) {
      write_credential(L"voice_input.language", language);
      sync_voice();
    };
    input.GotFocus([search, update_menu](Inspectable const &sender, RoutedEventArgs const &) {
      if (auto field = sender.try_as<AutoSuggestBox>()) {
        *search = false;
        update_menu(field);
      }
    });
    input.LostFocus([this](Inspectable const &, RoutedEventArgs const &) { flush_pending(); });
    input.TextChanged([search, update_menu, write](AutoSuggestBox const &sender,
                                                   AutoSuggestBoxTextChangedEventArgs const &args) {
      if (args.Reason() != AutoSuggestionBoxTextChangeReason::UserInput)
        return;
      std::wstring typed(sender.Text().c_str());
      if (typed.size() > 64) {
        typed.resize(64);
        sender.Text(hstring(typed));
      }
      *search = true;
      write(typed);
      update_menu(sender);
    });
    input.QuerySubmitted([entries, search, write](AutoSuggestBox const &sender,
                                                  AutoSuggestBoxQuerySubmittedEventArgs const &args) {
      const auto item = args.ChosenSuggestion();
      if (!item)
        return;
      const std::wstring chosen = unbox_value_or<hstring>(item, L"").c_str();
      for (auto const &[shown, code] : *entries)
        if (shown == chosen) {
          sender.Text(hstring(code));
          *search = false;
          sender.IsSuggestionListOpen(false);
          write(code);
          return;
        }
    });
    return input;
  }

  // 服务商的接入说明（ProviderPresetSection）：一个用默认浏览器打开说明页的按钮，没有说明页时不画。服务商已知的模型由同组的模型下拉列出，见 add_model_picker。语音页和 AI 页共用。
  void add_provider_link(StackPanel const &group, std::wstring const &label,
                         std::wstring_view documentation) {
    if (documentation.empty())
      return;
    const std::wstring url(documentation);
    StackPanel content;
    content.Padding(Thickness{25, 12, 25, 12});
    content.Children().Append(
        button_control(label + L"接入说明与 API Key", [this, url] { open_url(url.c_str()); }));
    auto card = make_card();
    card.Child(content);
    group.Children().Append(card);
  }

  // 模型（共享页的 ModelPickerRows）：一个下拉列出已知的模型（服务商预置的和获取到的，重复的只列一次），末项「自定义…」；选了它，或当前模型不在列表里，下面多一行「自定义模型」输入框。没有已知模型时只有输入框。选「自定义…」不改模型，只重画出输入框；这件事按模型的键记在 custom_model_lists_ 里，连同当时的列表，列表换了（换服务商、获取到新列表）就不再算数。输入框显示着时就算选了「自定义…」，在里面打出列表里有的名字也不收起，与共享页一样。`field` 按给定的无障碍名称造出输入框。
  void add_model_picker(StackPanel const &group, std::wstring const &title,
                        std::wstring const &subtitle, std::wstring const &label,
                        std::wstring const &model_key, std::vector<std::wstring> const &candidates,
                        std::function<void(std::wstring const &)> choose,
                        std::function<FrameworkElement(std::wstring const &)> const &field) {
    std::vector<std::wstring> models;
    for (auto const &entry : candidates)
      if (!entry.empty() && std::find(models.begin(), models.end(), entry) == models.end())
        models.push_back(entry);
    if (models.empty()) {
      add_row(group, 0xE950, title, subtitle, field(label));
      return;
    }
    std::wstring list_key;
    for (auto const &entry : models)
      list_key += entry + L"\n";
    const auto model = document_.String(model_key, L"");
    const auto requested = custom_model_lists_.find(model_key);
    const bool custom =
        (requested != custom_model_lists_.end() && requested->second == list_key) ||
        std::find(models.begin(), models.end(), model) == models.end();
    std::vector<Option> options;
    for (auto const &entry : models)
      options.push_back({entry, entry});
    options.push_back({L"", L"自定义…"});
    auto combo = select_control(label, std::move(options), custom ? L"" : model,
                                [this, model_key, list_key, choose](std::wstring const &value) {
                                  if (value.empty())
                                    custom_model_lists_[model_key] = list_key;
                                  else
                                    custom_model_lists_.erase(model_key);
                                  if (value.empty() || value == document_.String(model_key, L"")) {
                                    queue_refresh();
                                    return;
                                  }
                                  choose(value);
                                });
    combo.Width(240);
    add_row(group, 0xE950, title, subtitle, combo);
    if (!custom)
      return;
    if (!indexing_)
      custom_model_lists_[model_key] = list_key;
    add_row(group, 0xE70F, L"自定义模型", L"", field(label + L"（自定义）"));
  }

  // 语音页的模型：服务预置的模型加上输入框，选中一个模型保存后重画整页，输入框边输入边写进设置。
  void add_voice_model_picker(StackPanel const &group, std::wstring const &title,
                              std::wstring const &subtitle, std::wstring const &label,
                              std::wstring const &model_key, vs::ProviderDefaults const *preset) {
    std::vector<std::wstring> models;
    if (preset)
      for (auto const &entry : preset->models)
        models.emplace_back(entry);
    add_model_picker(
        group, title, subtitle, label, model_key, models,
        [this, model_key](std::wstring const &value) {
          change([&](PreferencesDocument &doc) { doc.SetString(model_key, value); }, true);
        },
        [this, model_key](std::wstring const &name) {
          return voice_text_input(name, model_key, false, L"", 0, nullptr);
        });
  }

  // 云服务的几行（VoiceAsrProviderSettingsSection 的 foldAdvanced 排法和 DoubaoOptionsRows）：接入说明、豆包鉴权方式、旧版鉴权的 App Key 和令牌留在外面；接口地址、识别模型、资源 ID 和豆包的识别选项收进「更多选项」，用户改过接口地址或资源 ID 时一打开就展开。豆包的两个流式预设和自定义地址写的都是 asr_endpoint，合成一个「流式接口」下拉，选「自定义地址」或存的地址不是预设时下面多一行地址输入框。豆包的识别请求不读识别模型（VoiceInputSession 只给 DoubaoAsrClient 接口、凭据和资源 ID），选豆包时不画这一行。
  void build_asr_service_rows(StackPanel const &group, std::wstring const &provider, bool doubao,
                              std::wstring const &auth_mode) {
    const auto *asr_preset = vs::find_defaults(voice_provider_catalog().asr, provider);
    if (asr_preset)
      add_provider_link(group, L"识别服务", asr_preset->documentation);
    if (doubao || indexing_)
      add_row(group, 0xE8D7, L"豆包鉴权方式", L"新版控制台使用单 API Key；旧版使用 App ID + Access Token",
              select_control(L"豆包鉴权方式",
                             {{L"api_key", L"新版 API Key"}, {L"legacy", L"旧版 App ID + Access Token"}},
                             auth_mode, [this](std::wstring const &value) {
                               change([&](PreferencesDocument &doc) {
                                 doc.SetString(L"voice_input.doubao_auth_mode", value);
                               }, true);
                             }));
    if ((doubao && auth_mode == L"legacy") || indexing_)
      voice_text_row(group, 0xE8D7, L"Doubao App Key", L"旧版控制台鉴权使用", L"Doubao App Key",
                     L"voice_input.asr_app_key", true);
    const std::wstring token_label(vs::asr_token_label(provider, auth_mode));
    voice_text_row(group, 0xE72E, token_label, L"仅保存在本机设置中", token_label,
                   L"voice_input.asr_token", true);

    const auto endpoint = document_.String(L"voice_input.asr_endpoint", L"");
    // 搜索结果指向收起的那几行时也要展开，否则滚过去也看不见。
    static constexpr std::array<std::wstring_view, 9> folded{
        L"流式接口", L"自定义接口地址", L"识别接口地址", L"识别模型", L"自定义模型",
        L"Doubao 资源 ID", L"数字格式化", L"语义顺滑", L"热词表 ID"};
    Expander more;
    more.Header(box_value(L"更多选项"));
    more.HorizontalAlignment(HorizontalAlignment::Stretch);
    more.HorizontalContentAlignment(HorizontalAlignment::Stretch);
    more.IsExpanded(
        voice_service_more_open_.value_or(vs::asr_advanced_changed(voice_provider_catalog().asr,
            provider, endpoint, document_.String(L"voice_input.asr_resource_id", L""))) ||
        std::find(folded.begin(), folded.end(), focus_row_) != folded.end());
    more.Expanding([this](Expander const &, ExpanderExpandingEventArgs const &) {
      voice_service_more_open_ = true;
    });
    more.Collapsed([this](Expander const &, ExpanderCollapsedEventArgs const &) {
      voice_service_more_open_ = false;
    });
    StackPanel rows;
    rows.Spacing(4);
    if (doubao || indexing_) {
      // 地址为空时宿主用默认的双向流式地址，下拉照实显示它；选「自定义地址」不改地址，只重画出输入框（共享页 DoubaoStreamEndpointSection 的 customRequested），在输入框里打出预设的地址也不收起。
      const bool custom = voice_custom_endpoint_ || vs::doubao_stream_endpoint_id(endpoint) == L"custom";
      std::vector<Option> streams;
      for (auto const &option : vs::doubao_stream_endpoints)
        streams.push_back({std::wstring(option.id), std::wstring(option.title)});
      streams.push_back({L"custom", L"自定义地址"});
      auto stream = select_control(
          L"流式接口", std::move(streams),
          custom ? std::wstring(L"custom") : std::wstring(vs::doubao_stream_endpoint_id(endpoint)),
          [this](std::wstring const &value) {
            const bool was_custom = voice_custom_endpoint_;
            voice_custom_endpoint_ = value == L"custom";
            for (auto const &option : vs::doubao_stream_endpoints)
              if (option.id == value) {
                const std::wstring chosen(option.endpoint);
                if (chosen == document_.String(L"voice_input.asr_endpoint", L"")) {
                  if (was_custom)
                    queue_refresh();
                  return;
                }
                change([&](PreferencesDocument &doc) {
                  doc.SetString(L"voice_input.asr_endpoint", chosen);
                }, true);
                return;
              }
            // 「自定义地址」不改地址（共享页的 updateDoubaoStreamEndpoint），只重画出地址输入框。
            if (!was_custom)
              queue_refresh();
          });
      stream.Width(200);
      add_row(rows, 0xE895, L"流式接口",
              L"整句流式边录边传、说完返回整句，服务方称准确率更高并推荐用于输入法；双向流式返回增量结果，边说边显示刷新更频繁。",
              stream);
      if (!indexing_)
        voice_sync_.push_back([this, weak = make_weak(stream.as<ComboBox>())] {
          auto shown = weak.get();
          if (!shown)
            return;
          const auto id =
              vs::doubao_stream_endpoint_id(document_.String(L"voice_input.asr_endpoint", L""));
          int32_t index = static_cast<int32_t>(vs::doubao_stream_endpoints.size());
          if (!voice_custom_endpoint_)
            for (size_t i = 0; i < vs::doubao_stream_endpoints.size(); ++i)
              if (vs::doubao_stream_endpoints[i].id == id)
                index = static_cast<int32_t>(i);
          if (shown.SelectedIndex() != index)
            shown.SelectedIndex(index);
        });
      if (custom || indexing_)
        voice_text_row(rows, 0xE71B, L"自定义接口地址", L"留空使用豆包的默认地址", L"识别接口地址",
                       L"voice_input.asr_endpoint", false, L"", 0, [this](std::wstring const &) {
                         // 在输入框里打字就算选了「自定义地址」：打出预设的地址时下拉也停在这一项。
                         voice_custom_endpoint_ = true;
                         return true;
                       });
    }
    if (!doubao || indexing_) {
      voice_text_row(rows, 0xE71B, L"识别接口地址", L"留空使用当前服务的默认地址", L"识别接口地址",
                     L"voice_input.asr_endpoint");
      add_voice_model_picker(rows, L"识别模型", L"由语音服务选择对应模型", L"识别模型",
                             L"voice_input.asr_model", asr_preset);
    }
    if (doubao || indexing_) {
      // 只有豆包的识别请求读资源 ID 和下面这几项；标点在「识别」组里的「自动添加标点」。
      voice_text_row(rows, 0xE8EC, L"Doubao 资源 ID", L"仅豆包识别使用", L"Doubao 资源 ID",
                     L"voice_input.asr_resource_id");
      rows.Children().Append(group_note(L"以下豆包识别选项随识别请求发送给豆包"));
      bool_row(rows, 0xE8EF, L"数字格式化", L"", L"voice_input.doubao_enable_itn", true);
      bool_row(rows, 0xE8FB, L"语义顺滑", L"", L"voice_input.doubao_enable_ddc", false);
      voice_text_row(rows, 0xE8EC, L"热词表 ID", L"", L"热词表 ID",
                     L"voice_input.doubao_boosting_table_id");
    }
    more.Content(rows);
    group.Children().Append(more);
  }

  // 「测试豆包识别配置」「测试语音识别配置」发给 msime_client_test_api_credential 的配置（asrServiceCredentialTestConfig）。
  JsonObject asr_test_config() const {
    const auto provider = document_.String(L"voice_input.asr_provider", L"doubao");
    auto string = [](std::wstring const &value) { return JsonValue::CreateStringValue(hstring(value)); };
    JsonObject config;
    config.SetNamedValue(L"provider", string(provider));
    config.SetNamedValue(L"endpoint", string(vs::provider_setting_value(
                                          document_.String(L"voice_input.asr_endpoint", L""),
                                          provider, voice_provider_catalog().asr, true)));
    config.SetNamedValue(L"model", string(vs::provider_setting_value(
                                       document_.String(L"voice_input.asr_model", L""), provider,
                                       voice_provider_catalog().asr, false)));
    config.SetNamedValue(L"token", string(document_.String(L"voice_input.asr_token", L"")));
    if (provider != L"doubao")
      return config;
    const std::wstring auth_mode(
        vs::doubao_auth_mode(document_.String(L"voice_input.doubao_auth_mode", L"")));
    config.SetNamedValue(L"auth_mode", string(auth_mode));
    config.SetNamedValue(L"app_id", string(auth_mode == L"legacy"
                                               ? document_.String(L"voice_input.asr_app_key", L"")
                                               : std::wstring()));
    config.SetNamedValue(L"resource_id", string(vs::doubao_test_resource_id(
                                             document_.String(L"voice_input.asr_resource_id", L""))));
    config.SetNamedValue(L"doubao_enable_itn", JsonValue::CreateBooleanValue(
                                                   document_.Boolean(L"voice_input.doubao_enable_itn", true)));
    config.SetNamedValue(L"doubao_enable_punc", JsonValue::CreateBooleanValue(
                                                    document_.Boolean(L"voice_input.doubao_enable_punc", true)));
    config.SetNamedValue(L"doubao_enable_ddc", JsonValue::CreateBooleanValue(
                                                   document_.Boolean(L"voice_input.doubao_enable_ddc", false)));
    config.SetNamedValue(L"doubao_boosting_table_id",
                         string(document_.String(L"voice_input.doubao_boosting_table_id", L"")));
    return config;
  }

  // 换服务的字段规则归 Rust；这里仅映射当前编辑值与前缀，不记录请求或返回。
  std::optional<JsonObject> voice_provider_update(std::wstring const &prefix,
                                                 std::wstring const &provider,
                                                 std::wstring const &previous) {
    JsonObject current;
    current.SetNamedValue(L"provider", JsonValue::CreateStringValue(hstring(previous)));
    for (auto field : {L"endpoint", L"model", L"token"})
      current.SetNamedValue(field, JsonValue::CreateStringValue(
          hstring(document_.String(L"voice_input." + prefix + L"_" + field, L""))));
    JsonObject slots;
    const auto stored = document_.Value(L"voice_input." + prefix + L"_tokens");
    if (stored && stored.ValueType() == JsonValueType::Object)
      for (auto const &entry : stored.GetObject())
        if (entry.Value().ValueType() == JsonValueType::String)
          slots.SetNamedValue(entry.Key(), entry.Value());
    current.SetNamedValue(L"tokens", slots);
    JsonObject request;
    request.SetNamedValue(L"service", JsonValue::CreateStringValue(hstring(L"voice." + prefix)));
    request.SetNamedValue(L"provider", JsonValue::CreateStringValue(hstring(provider)));
    request.SetNamedValue(L"current", current);
    const auto body = utf8(request.Stringify());
    const auto response = take_response(msime_client_voice_provider_update(
        reinterpret_cast<const uint8_t *>(body.data()), body.size()));
    if (response.ok) {
      try {
        return JsonObject::Parse(text(response.text)).GetNamedObject(L"value");
      } catch (...) {
      }
    }
    show_notice(L"切换语音服务失败，请重试。");
    return std::nullopt;
  }

  static void apply_provider_update(PreferencesDocument &doc, std::wstring const &prefix,
                                    JsonObject const &update) {
    for (auto const &entry : update)
      doc.SetValue(L"voice_input." + prefix + L"_" + entry.Key().c_str(), entry.Value());
  }

  // 共享页的 onProviderChange（VoiceInputBasicsSection）：换上新服务的令牌、地址和模型，选系统识别时把「自动识别」换成 zh-CN。换到或换走本地模型时，模型列表像共享页的模型管理器重新挂载一样重读。
  void select_asr_provider(std::wstring const &provider) {
    const auto previous = document_.String(L"voice_input.asr_provider", L"doubao");
    if (provider == previous)
      return;
    const auto update = voice_provider_update(L"asr", provider, previous);
    if (!update)
      return;
    const bool locale = provider == L"system" &&
                        document_.String(L"voice_input.language", L"") == L"auto";
    // 换服务像共享页那样重新挂上服务的几行：豆包「流式接口」上次选的「自定义地址」不再算数。
    voice_custom_endpoint_ = false;
    change([&](PreferencesDocument &doc) {
      apply_provider_update(doc, L"asr", *update);
      if (locale)
        doc.SetString(L"voice_input.language", L"zh-CN");
    }, true);
    if (provider == L"local" || previous == L"local")
      reset_local_models();
  }

  // 「文本润色」（VoicePolishSettingsSection 的 Windows 分支）：关着时只有开关；打开后是服务、接入说明、模型、地址、令牌、润色方案和提示词，最后是测试。
  void build_polish_group(StackPanel const &page) {
    auto group = add_group(page, L"文本润色");
    group.Children().Append(group_note(L"识别结果可交给用户管理的服务润色"));
    const bool enabled = document_.Boolean(L"voice_input.polish_text", false) ||
                         document_.Boolean(L"voice_input.polish_enabled", false);
    add_row(group, 0xE70F, L"启用润色", L"",
            toggle_control(L"启用文本润色", enabled, [this](bool on) {
              change([&](PreferencesDocument &doc) {
                doc.SetBoolean(L"voice_input.polish_text", on);
                doc.SetBoolean(L"voice_input.polish_enabled", on);
              }, true);
            }));
    if (!enabled && !indexing_)
      return;
    const auto provider = document_.String(L"voice_input.polish_provider", L"siliconflow");
    std::vector<Option> providers;
    for (auto const &preset : voice_provider_catalog().polish)
      providers.push_back({preset.id, preset.title});
    add_row(group, 0xE753, L"服务提供商", L"",
            select_control(L"文本润色服务提供商", std::move(providers), provider,
                           [this](std::wstring const &value) {
                             const auto previous = document_.String(L"voice_input.polish_provider", L"siliconflow");
                             if (value == previous)
                               return;
                             const auto update = voice_provider_update(L"polish", value, previous);
                             if (!update)
                               return;
                             change([&](PreferencesDocument &doc) {
                               apply_provider_update(doc, L"polish", *update);
                             }, true);
                           }));
    const auto *polish_preset = vs::find_defaults(voice_provider_catalog().polish, provider);
    if (polish_preset)
      add_provider_link(group, L"文本润色", polish_preset->documentation);
    add_voice_model_picker(group, L"模型", L"", L"文本润色模型", L"voice_input.polish_model",
                           polish_preset);
    voice_text_row(group, 0xE71B, L"润色接口地址", L"留空使用当前服务的默认地址", L"润色接口地址",
                   L"voice_input.polish_endpoint");
    voice_text_row(group, 0xE72E, L"润色 API Token", L"仅保存在本机设置中", L"润色 API Token",
                   L"voice_input.polish_token", true);

    // 润色方案（PolishPromptSection）：内置方案只读地显示它的完整提示词，自定义方案就地编辑，空着时发送内置的「精炼整理」。
    const std::wstring slot(
        vs::polish_prompt_slot(document_.String(L"voice_input.polish_prompt_id", L"")));
    std::vector<Option> slots;
    for (auto const &preset : vs::polish_presets)
      slots.push_back({std::wstring(preset.id), std::wstring(preset.name)});
    for (auto const &custom : vs::polish_custom_slots)
      slots.push_back({std::wstring(custom.value), std::wstring(custom.label)});
    add_row(group, 0xE8FD, L"润色方案", L"",
            select_control(L"润色方案", std::move(slots), slot, [this](std::wstring const &value) {
              change([&](PreferencesDocument &doc) {
                doc.SetString(L"voice_input.polish_prompt_id", value);
              }, true);
            }));
    const bool custom = vs::is_polish_custom_slot(slot);
    const std::wstring description =
        custom ? L"这一段会保存到所选的自定义方案；留空时使用内置的「精炼整理」提示词"
               : L"内置方案的完整提示词；要改写请选择自定义方案";
    record(L"润色提示词", description);
    StackPanel block;
    block.Spacing(6);
    block.Padding(Thickness{25, 16, 25, 16});
    block.Children().Append(make_text(L"润色提示词", 14, palette_.text));
    block.Children().Append(make_text(description, 12, palette_.sub));
    TextBox prompt;
    prompt.AcceptsReturn(true);
    prompt.TextWrapping(TextWrapping::Wrap);
    prompt.MinHeight(160);
    prompt.MaxHeight(360);
    prompt.IsSpellCheckEnabled(false);
    const std::wstring prompt_key = L"voice_input.polish_prompt_" + slot;
    prompt.Text(hstring(custom ? document_.String(prompt_key, L"")
                               : std::wstring(vs::polish_preset_prompt(slot))));
    prompt.IsReadOnly(!custom);
    prompt.IsEnabled(loaded_);
    A11y::SetName(prompt, L"润色提示词");
    if (custom)
      prompt.TextChanged([this, prompt_key](Inspectable const &sender, TextChangedEventArgs const &) {
        write_credential(prompt_key, textarea_value(sender.as<TextBox>()));
      });
    block.Children().Append(prompt);
    auto card = make_card();
    card.Child(block);
    group.Children().Append(card);
    mark_search_target(L"润色提示词", card);
    add_credential_test(group, L"voice.polish", L"测试语音润色配置",
                        [this] {
                          const auto current = document_.String(L"voice_input.polish_provider", L"siliconflow");
                          JsonObject config;
                          auto string = [](std::wstring const &value) {
                            return JsonValue::CreateStringValue(hstring(value));
                          };
                          config.SetNamedValue(L"provider", string(current));
                          config.SetNamedValue(L"endpoint", string(vs::provider_setting_value(
                                                                document_.String(L"voice_input.polish_endpoint", L""),
                                                                current, voice_provider_catalog().polish, true)));
                          config.SetNamedValue(L"model", string(vs::provider_setting_value(
                                                             document_.String(L"voice_input.polish_model", L""),
                                                             current, voice_provider_catalog().polish, false)));
                          config.SetNamedValue(L"token", string(document_.String(L"voice_input.polish_token", L"")));
                          return config;
                        },
                        [this] {
                          return vs::polish_test_disabled(document_.String(L"voice_input.polish_token", L""));
                        });
  }

  // 多行输入框里的文字。TextBox 把换行存成 \r；按共享页的 textarea 一样以 \n 分行。
  static std::wstring textarea_value(TextBox const &box) {
    std::wstring value(box.Text().c_str());
    std::wstring normalized;
    normalized.reserve(value.size());
    for (size_t i = 0; i < value.size(); ++i) {
      if (value[i] == L'\r') {
        normalized.push_back(L'\n');
        if (i + 1 < value.size() && value[i + 1] == L'\n')
          ++i;
      } else {
        normalized.push_back(value[i]);
      }
    }
    return normalized;
  }

  // 「录音设备」（VoiceCaptureDevicesSection 只有一种录音后端时的那一支，VoiceMicrophonePicker）：Windows 只经 WASAPI 录音，空、auto、windows 效果相同，所以只有一个「麦克风」下拉，第一项是系统默认设备，下面是读到的设备。每次进入语音页自动读一次列表，不再有录音后端、「刷新设备」和手填的设备标识。选中的设备拔掉后显示「已选的麦克风（未连接）」，录音会失败，改回系统默认设备即可。存着别的平台的后端时宿主不理会它和它的设备、用系统默认设备录音，这里单列一项标出，选「系统默认设备」才是一次真正的改动，把它清掉。打开页面时不改写存储。
  void build_capture_device_group(StackPanel const &page) {
    if (!indexing_ && capture_stale_) {
      capture_stale_ = false;
      // 读完会重画页面，所以等这一次绘制结束后再开始读。
      DispatcherQueue().TryEnqueue([weak = get_weak()] {
        if (auto self = weak.get(); self && !self->closed_)
          self->refresh_capture_devices();
      });
    }
    auto group = add_group(page, L"录音设备");
    const auto backend = document_.String(L"voice_input.capture_backend", L"");
    const auto device = document_.String(L"voice_input.capture_device", L"");
    static constexpr wchar_t foreign[] = L"foreign";
    static constexpr wchar_t stored[] = L"stored";
    std::wstring selected = !vs::capture_backend_supported(backend) ? foreign
                            : device.empty()                         ? L""
                                                                     : stored;
    std::vector<Option> options;
    options.push_back({L"", L"系统默认设备"});
    for (size_t index = 0; index < capture_devices_.size(); ++index) {
      options.push_back({std::to_wstring(index), capture_devices_[index].label});
      if (selected == stored && capture_devices_[index].id == device)
        selected = std::to_wstring(index);
    }
    if (selected == foreign)
      options.push_back({foreign, L"系统默认设备（已忽略其他平台的录音设置）", false});
    if (selected == stored)
      options.push_back({stored, capture_read_ ? L"已选的麦克风（未连接）" : L"已选的麦克风（读取中…）",
                         false});
    auto picker = select_control(
        L"麦克风", std::move(options), selected, [this](std::wstring const &value) {
          const auto current_backend = document_.String(L"voice_input.capture_backend", L"");
          const auto current_device = document_.String(L"voice_input.capture_device", L"");
          std::wstring next_backend, next_device;
          if (!value.empty()) {
            // 「已忽略其他平台的录音设置」和「已选的麦克风」都是灰的，选不到；其余的值是列表里的一项。
            if (value == foreign || value == stored)
              return;
            const auto index = static_cast<size_t>(std::stoul(value));
            if (index >= capture_devices_.size())
              return;
            next_backend = capture_devices_[index].backend;
            next_device = capture_devices_[index].id;
          }
          if (next_backend == current_backend && next_device == current_device)
            return;
          change([&](PreferencesDocument &doc) {
            doc.SetString(L"voice_input.capture_backend", next_backend);
            doc.SetString(L"voice_input.capture_device", next_device);
          }, true);
        });
    picker.as<ComboBox>().Width(240);
    auto status = make_text(capture_notice_.empty()
                                ? L"从下一次录音生效。选中的麦克风拔掉后无法录音，改回系统默认设备即可。"
                                : capture_notice_,
                            12, palette_.sub);
    A11y::SetLiveSetting(status, Automation::Peers::AutomationLiveSetting::Polite);
    add_row(group, 0xE720, L"麦克风", L"", picker, [&] {
      StackPanel holder;
      holder.Margin(Thickness{0, 0, 0, 14});
      holder.Children().Append(status);
      return holder;
    }());
  }

  // 读取录音设备：在工作线程上经 msime_client_voice_capture_devices 列出 WASAPI 录音设备（与 Tauri 的 list_voice_capture_devices 同一份），读不到时留着上次的列表，说明里写读取失败。
  void refresh_capture_devices() {
    if (capture_busy_)
      return;
    capture_busy_ = true;
    run_host([] { return take_response(msime_client_voice_capture_devices()); },
             [this](Response const &response) {
               capture_busy_ = false;
               bool read = false;
               if (response.ok) {
                 try {
                   std::vector<CaptureDevice> devices;
                   for (auto const &item :
                        JsonObject::Parse(text(response.text)).GetNamedArray(L"value")) {
                     const auto object = item.GetObject();
                     devices.push_back({named_string(object, L"backend"), named_string(object, L"id"),
                                        named_string(object, L"label")});
                   }
                   capture_devices_ = std::move(devices);
                   read = true;
                 } catch (...) {
                 }
               }
               // 读取失败也算读完了：失败原因写在说明里，不能一直显示「读取中」。
               capture_read_ = true;
               capture_notice_ = read ? L"" : L"无法读取设备列表，可以先用系统默认设备";
               redraw_voice();
             });
  }

  // ---- 本地识别模型 ----

  // 本地模型存放的目录：共享设置应用（Tauri）的应用数据目录 %APPDATA%\<本版本的 identifier> 下的 voice-models，与 Tauri 的语音页管同一批模型。读不到已知文件夹时为空。
  static std::filesystem::path local_model_root() {
    PWSTR roaming = nullptr;
    if (FAILED(SHGetKnownFolderPath(FOLDERID_RoamingAppData, 0, nullptr, &roaming))) {
      CoTaskMemFree(roaming);
      return {};
    }
    auto root = std::filesystem::path(roaming) / MSIME_EDITION_TAURI_IDENTIFIER / L"voice-models";
    CoTaskMemFree(roaming);
    return root;
  }

  // 本地模型（VoiceLocalModelSettingsSection，带模型管理器的那一支）：模型列表，然后「更多选项」里是下载镜像和手填的模型目录。
  void build_local_model_rows(StackPanel const &group) {
    if (!indexing_ && local_models_stale_)
      refresh_local_models();
    add_local_model_manager(group);
    Expander more;
    more.Header(box_value(L"更多选项"));
    more.HorizontalAlignment(HorizontalAlignment::Stretch);
    more.HorizontalContentAlignment(HorizontalAlignment::Stretch);
    more.IsExpanded(local_more_open_ || focus_row_ == L"模型下载镜像" ||
                    focus_row_ == L"本地模型目录");
    more.Expanding([this](Expander const &, ExpanderExpandingEventArgs const &) {
      local_more_open_ = true;
    });
    more.Collapsed([this](Expander const &, ExpanderCollapsedEventArgs const &) {
      local_more_open_ = false;
    });
    StackPanel rows;
    rows.Spacing(4);
    voice_text_row(rows, 0xE896, L"模型下载镜像",
                   L"可选。以 https:// 开头的加速前缀，下载地址为“镜像/原始地址”；留空直接从 GitHub 下载",
                   L"模型下载镜像", L"voice_input.asr_model_mirror", false,
                   L"https://mirror.example.com", 2048, [](std::wstring const &value) {
                     return voice_model_setting_accepted(L"asr_model_mirror", value);
                   });
    // VoiceModelPathSection：手填目录，或用「选择…」打开系统的选择文件夹对话框（与 Tauri 的 pick_voice_model_path 相同，只选已有的目录，取消时不改）。
    StackPanel path;
    path.Orientation(Orientation::Horizontal);
    path.Spacing(8);
    auto accepted = [](std::wstring const &value) {
      return voice_model_setting_accepted(L"asr_model_path", value);
    };
    path.Children().Append(voice_text_input(L"本地模型目录", L"voice_input.asr_model_path", false,
                                            L"/path/to/voice-models/<model>", 0, accepted));
    path.Children().Append(button_control(L"选择…", [this, accepted] {
      const auto chosen = pick_model_directory();
      if (!chosen || !accepted(*chosen))
        return;
      change([&](PreferencesDocument &doc) {
        doc.SetString(L"voice_input.asr_model_path", *chosen);
      }, true);
    }, loaded_));
    add_row(rows, 0xE8B7, L"本地模型目录", L"已安装模型所在文件夹的绝对路径（包含 msime-model.json）",
            path);
    more.Content(rows);
    group.Children().Append(more);
  }

  // 模型管理器（LocalModelManager）：每个模型一张卡片，写着它是什么、多大、许可，按状态给出下载、从文件导入、取消、使用、删除。
  void add_local_model_manager(StackPanel const &group) {
    static constexpr wchar_t note[] = L"模型下载到本机后完全离线运行，录音不会上传。点击“使用”后生效。";
    record(L"本地识别模型", note);
    StackPanel block;
    block.Spacing(10);
    block.Padding(Thickness{25, 16, 25, 16});
    StackPanel heading;
    heading.Spacing(1);
    heading.Children().Append(make_text(L"本地识别模型", 14, palette_.text));
    heading.Children().Append(make_text(note, 12, palette_.sub));
    block.Children().Append(heading);
    if (!local_models_ && local_models_notice_.empty())
      block.Children().Append(make_text(L"正在读取模型列表…", 13, palette_.sub));
    const auto model_path = document_.String(L"voice_input.asr_model_path", L"");
    if (local_models_)
      for (auto const &model : *local_models_)
        block.Children().Append(local_model_card(model, model_path));
    if (!local_models_notice_.empty()) {
      auto notice = make_text(local_models_notice_, 13, palette_.text);
      A11y::SetLiveSetting(notice, Automation::Peers::AutomationLiveSetting::Polite);
      block.Children().Append(notice);
    }
    A11y::SetName(block, L"本地识别模型");
    auto card = make_card();
    card.Child(block);
    group.Children().Append(card);
    mark_search_target(L"本地识别模型", card);
  }

  // 一段带链接的文字：链接用默认浏览器打开（模型目录里的地址都是 https）。
  void append_link(TextBlock const &block, std::wstring const &label, std::wstring const &url) {
    Documents::Hyperlink link;
    Documents::Run run;
    run.Text(hstring(label));
    link.Inlines().Append(run);
    link.Click([this, url](Documents::Hyperlink const &, Documents::HyperlinkClickEventArgs const &) {
      if (url.rfind(L"https://", 0) == 0)
        open_url(url.c_str());
    });
    block.Inlines().Append(link);
  }

  static void append_run(TextBlock const &block, std::wstring const &value) {
    Documents::Run run;
    run.Text(hstring(value));
    block.Inlines().Append(run);
  }

  FrameworkElement local_model_card(LocalModel const &model, std::wstring const &model_path) {
    const bool in_use = vs::local_model_in_use(model.path, model_path);
    const bool running = local_model_installing_.count(model.id) > 0;
    const bool importing = local_model_importing_.count(model.id) > 0;
    const bool removing = local_model_removing_.count(model.id) > 0;
    const auto progress = local_model_progress_.find(model.id);
    StackPanel lines;
    lines.Spacing(4);
    StackPanel title;
    title.Orientation(Orientation::Horizontal);
    title.Spacing(8);
    auto name = make_text(model.title, 14, palette_.text);
    name.FontWeight(Windows::UI::Text::FontWeight{600});
    title.Children().Append(name);
    auto badge = [&](std::wstring_view label, bool strong) {
      auto text = make_text(label, 12, strong ? palette_.text : palette_.sub);
      text.VerticalAlignment(VerticalAlignment::Center);
      if (strong)
        text.FontWeight(Windows::UI::Text::FontWeight{600});
      title.Children().Append(text);
    };
    if (model.streaming)
      badge(L"流式", false);
    if (model.is_default)
      badge(L"推荐", false);
    if (in_use)
      badge(L"正在使用", true);
    if (model.installed && !in_use)
      badge(L"已下载", false);
    lines.Children().Append(title);
    lines.Children().Append(make_text(model.description, 13, palette_.text));
    lines.Children().Append(make_text(
        L"语言：" + vs::local_model_languages(model.languages) + L" · 下载 " +
            vs::format_model_bytes(model.archive_size) + L" · 占用磁盘 " +
            vs::format_model_bytes(model.installed_size) + L" · 运行内存约 " +
            vs::format_model_bytes(model.memory),
        12, palette_.sub));
    auto license = make_text(L"", 12, palette_.sub);
    append_run(license, L"许可：" + model.license_spdx + L"。" + model.license_notice + L" 来源：");
    append_link(license, model.license_source, model.license_source);
    if (!model.license_terms.empty())
      append_run(license, L"（条款：" + model.license_terms + L"）");
    lines.Children().Append(license);
    if (!model.installed && !model.import_files.empty()) {
      auto offline = make_text(L"", 12, palette_.sub);
      append_run(offline, L"不联网安装：先下载 ");
      for (size_t index = 0; index < model.import_files.size(); ++index) {
        auto const &file = model.import_files[index];
        if (index > 0)
          append_run(offline, L"、");
        append_link(offline, file.name + L"（" + vs::format_model_bytes(file.size) + L"）", file.url);
      }
      append_run(offline, L"，再点“从文件导入”一起选中。文件改过名也能认出。");
      lines.Children().Append(offline);
    }
    if (running || (importing && progress != local_model_progress_.end())) {
      Grid row;
      row.ColumnSpacing(8);
      ColumnDefinition bar_column;
      bar_column.Width(GridLength{1, GridUnitType::Star});
      ColumnDefinition label_column;
      label_column.Width(GridLength{1, GridUnitType::Auto});
      row.ColumnDefinitions().Append(bar_column);
      row.ColumnDefinitions().Append(label_column);
      ProgressBar bar;
      bar.Maximum(100);
      bar.VerticalAlignment(VerticalAlignment::Center);
      A11y::SetName(bar, hstring(model.title + (importing ? L" 导入进度" : L" 下载进度")));
      auto label = make_text(L"", 12, palette_.text);
      label.VerticalAlignment(VerticalAlignment::Center);
      Grid::SetColumn(label, 1);
      row.Children().Append(bar);
      row.Children().Append(label);
      lines.Children().Append(row);
      auto show = [this, id = model.id, weak_bar = make_weak(bar), weak_label = make_weak(label)] {
        auto shown_bar = weak_bar.get();
        auto shown_label = weak_label.get();
        if (!shown_bar || !shown_label)
          return;
        const auto found = local_model_progress_.find(id);
        const std::wstring stage = found == local_model_progress_.end() ? L"" : found->second.stage;
        const int percent = found == local_model_progress_.end()
                                ? 0
                                : vs::local_model_progress_percent(stage, found->second.downloaded,
                                                                   found->second.total);
        shown_bar.Value(percent);
        shown_label.Text(hstring(std::wstring(vs::local_model_stage_label(stage)) + L" " +
                                 std::to_wstring(percent) + L"%"));
      };
      show();
      if (!indexing_)
        local_model_views_[model.id] = std::move(show);
    }
    StackPanel buttons;
    buttons.Orientation(Orientation::Horizontal);
    buttons.Spacing(8);
    if (running || importing) {
      buttons.Children().Append(button_control(importing ? L"取消导入" : L"取消下载", [id = model.id] {
        JsonObject request;
        request.SetNamedValue(L"id", JsonValue::CreateStringValue(hstring(id)));
        const auto body = utf8(request.Stringify());
        take_response(msime_client_voice_local_model_cancel(
            reinterpret_cast<const uint8_t *>(body.data()), body.size()));
      }));
    } else if (model.installed) {
      buttons.Children().Append(button_control(
          in_use ? L"使用中" : L"使用",
          [this, path = model.path] {
            if (!voice_model_setting_accepted(L"asr_model_path", path))
              return;
            change([&](PreferencesDocument &doc) {
              doc.SetString(L"voice_input.asr_model_path", path);
            }, true);
          },
          loaded_ && !in_use && !removing));
      buttons.Children().Append(button_control(L"删除", [this, model] { remove_local_model(model); },
                                               !removing));
    } else {
      buttons.Children().Append(button_control(
          L"下载（" + vs::format_model_bytes(model.archive_size) + L"）",
          [this, id = model.id] { install_local_model(id, false); }));
      buttons.Children().Append(
          button_control(L"从文件导入", [this, id = model.id] { install_local_model(id, true); }));
    }
    lines.Children().Append(buttons);
    Border card;
    card.BorderBrush(brush(palette_.stroke));
    card.BorderThickness(Thickness{1, 1, 1, 1});
    card.CornerRadius(CornerRadius{8, 8, 8, 8});
    card.Padding(Thickness{12, 12, 12, 12});
    card.Child(lines);
    A11y::SetName(card, hstring(model.title));
    return card;
  }

  // 共享页的模型管理器重新挂载：作废进行中的请求，清掉状态，下次绘制时重读列表。
  void reset_local_models() {
    ++local_models_generation_;
    local_models_.reset();
    local_models_notice_.clear();
    local_model_progress_.clear();
    local_model_installing_.clear();
    local_model_importing_.clear();
    local_model_removing_.clear();
    local_models_stale_ = true;
  }

  // 在工作线程上经 msime_client_voice_local_models 读模型目录和已安装的模型，完成后重画。
  void refresh_local_models() {
    local_models_stale_ = false;
    const auto root = local_model_root();
    if (root.empty()) {
      local_models_notice_ = *vs::local_model_error_message(L"local_model_invalid_root");
      return;
    }
    JsonObject request;
    request.SetNamedValue(L"root", JsonValue::CreateStringValue(hstring(root.wstring())));
    const auto generation = local_models_generation_;
    run_host(
        [body = utf8(request.Stringify())] {
          return take_response(msime_client_voice_local_models(
              reinterpret_cast<const uint8_t *>(body.data()), body.size()));
        },
        [this, generation](Response const &response) {
          if (generation != local_models_generation_)
            return;
          if (!response.ok) {
            local_models_notice_ =
                vs::local_model_error_message(vs::local_model_error_code(response_error(response)))
                    .value_or(L"");
          } else {
            try {
              local_models_ = parse_local_models(
                  JsonObject::Parse(text(response.text)).GetNamedObject(L"value"));
            } catch (...) {
              local_models_notice_ = *vs::local_model_error_message(L"");
            }
          }
          redraw_voice();
        });
  }

  static std::vector<LocalModel> parse_local_models(JsonObject const &value) {
    std::vector<LocalModel> models;
    for (auto const &object : named_objects(value, L"models")) {
      LocalModel model;
      model.id = named_string(object, L"id");
      model.title = named_string(object, L"title");
      model.description = named_string(object, L"description");
      model.languages = named_strings(object, L"languages");
      model.streaming = object.GetNamedBoolean(L"streaming", false);
      model.is_default = object.GetNamedBoolean(L"default", false);
      model.installed = object.GetNamedBoolean(L"installed", false);
      model.path = named_string(object, L"path");
      model.installed_size = named_number(object, L"installed_size").value_or(0);
      model.archive_size = named_number(object, L"archive_size").value_or(0);
      model.memory = named_number(object, L"memory").value_or(0);
      model.license_spdx = named_string(object, L"license_spdx");
      model.license_source = named_string(object, L"license_source");
      model.license_terms = named_string(object, L"license_terms");
      model.license_notice = named_string(object, L"license_notice");
      for (auto const &file : named_objects(object, L"import_files"))
        model.import_files.push_back({named_string(file, L"name"), named_string(file, L"url"),
                                      named_number(file, L"size").value_or(0)});
      models.push_back(std::move(model));
    }
    return models;
  }

  // 安装进度的去处：每次进度在工作线程上交给 local_model_progress，百分比或阶段变了才送回 UI 线程。
  struct ModelProgressSink {
    weak_ref<MainWindow> window;
    Microsoft::UI::Dispatching::DispatcherQueue queue{nullptr};
    uint64_t generation = 0;
    std::wstring id;
    int percent = -1;
    std::wstring stage;
  };

  static void local_model_progress(const uint8_t *json, size_t length, void *context) noexcept {
    try {
      auto *sink = static_cast<ModelProgressSink *>(context);
      const auto object = JsonObject::Parse(
          text(std::string_view(reinterpret_cast<const char *>(json), length)));
      std::wstring stage(object.GetNamedString(L"stage", L"").c_str());
      const double downloaded = object.GetNamedNumber(L"downloaded", 0);
      const double total = object.GetNamedNumber(L"total", 0);
      const int percent = vs::local_model_progress_percent(stage, downloaded, total);
      if (percent == sink->percent && stage == sink->stage)
        return;
      sink->percent = percent;
      sink->stage = stage;
      sink->queue.TryEnqueue([window = sink->window, generation = sink->generation, id = sink->id,
                              stage, downloaded, total] {
        auto self = window.get();
        if (!self || self->closed_ || generation != self->local_models_generation_)
          return;
        const bool shown = self->local_model_progress_.count(id) > 0;
        self->local_model_progress_[id] = {stage, downloaded, total};
        if (const auto view = self->local_model_views_.find(id); view != self->local_model_views_.end())
          view->second();
        else if (!shown)
          self->redraw_voice();
      });
    } catch (...) {
    }
  }

  // 「下载」和「从文件导入」（install、importFiles）：导入先让用户在打开对话框里多选下载好的文件，关掉对话框不算失败。下载用已保存的下载镜像。安装在工作线程上进行，完成后若还没有选模型就用上这个，并重读列表。
  void install_local_model(std::wstring const &id, bool import) {
    if (!local_models_)
      return;
    const auto found = std::find_if(local_models_->begin(), local_models_->end(),
                                    [&](LocalModel const &model) { return model.id == id; });
    if (found == local_models_->end())
      return;
    const auto title = found->title;
    const auto archive_size = found->archive_size;
    std::vector<std::wstring> files;
    if (import) {
      auto picked = pick_model_files();
      if (!picked)
        return;
      files = std::move(*picked);
    }
    const auto root = local_model_root();
    if (root.empty()) {
      local_models_notice_ = *vs::local_model_error_message(L"local_model_invalid_root");
      redraw_voice();
      return;
    }
    flush_pending();
    local_models_notice_.clear();
    JsonObject request;
    request.SetNamedValue(L"root", JsonValue::CreateStringValue(hstring(root.wstring())));
    request.SetNamedValue(L"id", JsonValue::CreateStringValue(hstring(id)));
    if (import) {
      local_model_importing_.insert(id);
      JsonArray paths;
      for (auto const &file : files)
        paths.Append(JsonValue::CreateStringValue(hstring(file)));
      request.SetNamedValue(L"files", paths);
    } else {
      local_model_installing_.insert(id);
      local_model_progress_[id] = {L"download", 0, archive_size};
      request.SetNamedValue(L"mirror", JsonValue::CreateStringValue(hstring(
                                           document_.String(L"voice_input.asr_model_mirror", L""))));
    }
    const auto generation = local_models_generation_;
    run_host(
        [body = utf8(request.Stringify()), window = get_weak(), queue = DispatcherQueue(),
         generation, id] {
          ModelProgressSink sink{window, queue, generation, id};
          return take_response(msime_client_voice_local_model_install(
              reinterpret_cast<const uint8_t *>(body.data()), body.size(),
              &MainWindow::local_model_progress, &sink));
        },
        [this, generation, id, title, import](Response const &response) {
          if (generation != local_models_generation_)
            return;
          if (response.ok) {
            local_models_notice_ = L"「" + title + (import ? L"」已导入。" : L"」已下载。");
            std::wstring path;
            try {
              path = JsonObject::Parse(text(response.text))
                         .GetNamedObject(L"value")
                         .GetNamedString(L"path", L"")
                         .c_str();
            } catch (...) {
            }
            // 第一个装上的模型就是为它下载的，直接用上；之后的等用户选。
            if (!path.empty() &&
                msime::settings::js_trim(document_.String(L"voice_input.asr_model_path", L"")).empty() &&
                voice_model_setting_accepted(L"asr_model_path", path))
              change([&](PreferencesDocument &doc) {
                doc.SetString(L"voice_input.asr_model_path", path);
              }, false);
          } else {
            const auto code = vs::local_model_error_code(response_error(response));
            const auto message = import ? vs::local_model_import_error_message(code)
                                        : vs::local_model_error_message(code);
            local_models_notice_ =
                message ? *message
                        : (import ? L"已取消导入「" : L"已取消下载「") + title + L"」。";
          }
          local_model_installing_.erase(id);
          local_model_importing_.erase(id);
          local_model_progress_.erase(id);
          refresh_local_models();
          redraw_voice();
        });
    redraw_voice();
  }

  // 「删除」（remove）：确认后在工作线程上删除；删掉的是正在用的模型时清空 asr_model_path。
  fire_and_forget remove_local_model(LocalModel model) {
    auto strong = get_strong();
    const bool in_use =
        vs::local_model_in_use(model.path, document_.String(L"voice_input.asr_model_path", L""));
    if (!co_await confirm(L"删除本地模型",
                          in_use ? L"「" + model.title +
                                       L"」正在使用。删除后本地识别将不可用，直到选择其他模型。确定删除？"
                                 : L"确定删除「" + model.title + L"」？需要时可以重新下载。",
                          L"删除"))
      co_return;
    const auto root = local_model_root();
    if (root.empty()) {
      local_models_notice_ = *vs::local_model_error_message(L"local_model_invalid_root");
      redraw_voice();
      co_return;
    }
    local_models_notice_.clear();
    local_model_removing_.insert(model.id);
    JsonObject request;
    request.SetNamedValue(L"root", JsonValue::CreateStringValue(hstring(root.wstring())));
    request.SetNamedValue(L"id", JsonValue::CreateStringValue(hstring(model.id)));
    const auto generation = local_models_generation_;
    run_host(
        [body = utf8(request.Stringify())] {
          return take_response(msime_client_voice_local_model_remove(
              reinterpret_cast<const uint8_t *>(body.data()), body.size()));
        },
        [this, generation, model](Response const &response) {
          if (generation != local_models_generation_)
            return;
          if (response.ok) {
            // 按删除完成时的设置判断，与共享页的 onRemoved 相同。
            if (vs::local_model_in_use(model.path,
                                       document_.String(L"voice_input.asr_model_path", L"")))
              change([&](PreferencesDocument &doc) {
                doc.SetString(L"voice_input.asr_model_path", L"");
              }, false);
          } else {
            local_models_notice_ =
                vs::local_model_error_message(vs::local_model_error_code(response_error(response)))
                    .value_or(L"");
          }
          local_model_removing_.erase(model.id);
          refresh_local_models();
          redraw_voice();
        });
    redraw_voice();
  }

  // 「选择…」的系统选择文件夹对话框（与 Tauri 的 pick_voice_model_path 相同的标题）。用户取消或对话框打不开时返回空。
  std::optional<std::wstring> pick_model_directory() {
    com_ptr<IFileDialog> dialog;
    if (FAILED(CoCreateInstance(CLSID_FileOpenDialog, nullptr, CLSCTX_INPROC_SERVER,
                                IID_PPV_ARGS(dialog.put()))))
      return std::nullopt;
    FILEOPENDIALOGOPTIONS options = 0;
    dialog->GetOptions(&options);
    dialog->SetTitle(L"选择本地语音模型目录");
    dialog->SetOptions(options | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST);
    // 点击按钮时本窗口就是这个线程的活动窗口，对话框以它为所有者，模态地挡住设置窗口。
    if (dialog->Show(GetActiveWindow()) != S_OK)
      return std::nullopt;
    com_ptr<IShellItem> item;
    if (FAILED(dialog->GetResult(item.put())))
      return std::nullopt;
    PWSTR raw = nullptr;
    if (FAILED(item->GetDisplayName(SIGDN_FILESYSPATH, &raw)) || !raw)
      return std::nullopt;
    std::wstring chosen(raw);
    CoTaskMemFree(raw);
    return chosen;
  }

  // 「从文件导入」的打开对话框（与 Tauri 的 voice_local_model_import 相同的标题）：可以多选，不按扩展名过滤，下载工具可能改过名，认文件靠内容。用户取消或对话框打不开时返回空。
  std::optional<std::vector<std::wstring>> pick_model_files() {
    com_ptr<IFileOpenDialog> dialog;
    if (FAILED(CoCreateInstance(CLSID_FileOpenDialog, nullptr, CLSCTX_INPROC_SERVER,
                                IID_PPV_ARGS(dialog.put()))))
      return std::nullopt;
    FILEOPENDIALOGOPTIONS options = 0;
    dialog->GetOptions(&options);
    dialog->SetTitle(L"选择下载好的模型文件");
    dialog->SetOptions(options | FOS_ALLOWMULTISELECT | FOS_FORCEFILESYSTEM | FOS_FILEMUSTEXIST);
    if (dialog->Show(GetActiveWindow()) != S_OK)
      return std::nullopt;
    com_ptr<IShellItemArray> items;
    if (FAILED(dialog->GetResults(items.put())))
      return std::nullopt;
    DWORD count = 0;
    if (FAILED(items->GetCount(&count)))
      return std::nullopt;
    std::vector<std::wstring> files;
    for (DWORD index = 0; index < count; ++index) {
      com_ptr<IShellItem> item;
      if (FAILED(items->GetItemAt(index, item.put())))
        continue;
      PWSTR raw = nullptr;
      if (SUCCEEDED(item->GetDisplayName(SIGDN_FILESYSPATH, &raw)) && raw)
        files.emplace_back(raw);
      CoTaskMemFree(raw);
    }
    return files;
  }

  // 共享的「手写输入」在 Windows 上只有「手写识别板」一组：打开面板的一行和一张静态预览，没有可调的项（手写面板主题在「主题」页），所以这里只留打开面板的一行，文案与 handwriting-settings-section.tsx 相同。它不再单独成页，由 build_osk_page 画在「屏幕键盘与手写」页的「外观」之前。
  void build_hand_page(StackPanel const &page) {
    auto group = add_group(page, L"手写识别板");
    shell_row(group, 0xE929, L"打开手写识别板", L"使用鼠标或触控方式手写输入，自动识别候选汉字",
              L"打开", nav::shell_links::handwriting_panel);
  }

  // ---- AI 辅助 ----

  // 接口地址的来源键和说明（msime_client_ai_endpoint_check，不做 I/O，规则与 ai-endpoint-policy.ts 一致）。来源键是 ai_assistant.tokens 里存 Token 的键，地址不能用时为空；说明只在地址不能用时非空。
  struct AiEndpoint {
    std::optional<std::wstring> origin;
    std::wstring hint;
  };

  static AiEndpoint ai_endpoint(std::wstring const &endpoint) {
    const auto bytes = utf8(hstring(endpoint));
    const auto response = take_response(msime_client_ai_endpoint_check(
        reinterpret_cast<const uint8_t *>(bytes.data()), bytes.size()));
    AiEndpoint result;
    // 调用失败只会是地址超过 64 KiB，按 aiEndpointHint 对超长地址的说法处理。
    result.hint = L"请填写完整的接口地址：https://，或本机、局域网的 http://，且不能包含用户名、密码或 # 片段。";
    if (!response.ok)
      return result;
    try {
      const auto value = JsonObject::Parse(text(response.text)).GetNamedObject(L"value");
      const auto origin = value.GetNamedValue(L"origin");
      if (origin.ValueType() == JsonValueType::String)
        result.origin = std::wstring(origin.GetString().c_str());
      result.hint = value.GetNamedString(L"hint", L"").c_str();
    } catch (...) {
    }
    return result;
  }

  AiEndpoint ai_endpoint() const {
    return ai_endpoint(document_.String(L"ai_assistant.endpoint", L""));
  }

  // 当前接口地址对应的 API Token（ai_assistant.tokens[来源键]）；地址不能用时为空。
  std::wstring ai_token(AiEndpoint const &endpoint) const {
    return endpoint.origin ? document_.Entry(L"ai_assistant.tokens", *endpoint.origin, L"")
                           : std::wstring();
  }

  // 共享页 updateAi 除写设置以外做的事：作废进行中的获取模型；改了服务商或接口地址时，已获取的模型列表和它的提示也清掉。
  void ai_edited(bool models) {
    ++ai_generation_;
    ai_models_busy_ = false;
    if (models) {
      ai_models_.reset();
      ai_models_status_.clear();
    }
  }

  // 下拉框和开关的改动：保存后重画整页。
  void ai_change(std::function<void(PreferencesDocument &)> const &edit, bool models) {
    ai_edited(models);
    change(edit, true);
  }

  // 输入框的改动不重画整页，跟着变的控件就地更新：Token 行、地址说明、「获取模型列表」和「测试 AI 辅助配置」。
  void sync_ai() {
    for (auto const &update : ai_sync_)
      update();
    refresh_credential_test(L"ai.assistant");
    refresh_ai_models();
  }

  // add_row 画出的卡片标题下的那行说明；没有说明时补一个收起的，供就地更新。
  TextBlock row_subtitle(Border const &card) {
    auto texts = card.Child()
                     .as<StackPanel>()
                     .Children()
                     .GetAt(0)
                     .as<Grid>()
                     .Children()
                     .GetAt(0)
                     .as<StackPanel>();
    if (texts.Children().Size() < 2) {
      auto line = make_text(L"", 11, palette_.sub);
      line.Visibility(Visibility::Collapsed);
      texts.Children().Append(line);
    }
    return texts.Children().GetAt(1).as<TextBlock>();
  }

  static void set_subtitle(TextBlock const &line, std::wstring const &value) {
    if (std::wstring_view(line.Text()) != value)
      line.Text(hstring(value));
    line.Visibility(value.empty() ? Visibility::Collapsed : Visibility::Visible);
  }

  // 共享 AI 页（`ai-settings-content.tsx`、`ai-settings-page-section.tsx`）在 Windows 上的内容，顺序也相同：只有「服务」一组（开关、服务提供商、API Token、接入说明、模型（预置的和获取到的模型在一个下拉里，末项「自定义…」）、「更多选项」里的接口地址、获取模型列表和当前槽位的自定义提示词，末尾是「测试 AI 辅助配置」）。Windows 固定 AI 候选数量为 3（client-core 的 `HostPlatform::fixed_ai_candidate_limit`，TSF 从 host-api 的在线查询读这个值），所以没有「联想」组；桌面上这份配置只给拼音联想用，没有「AI 润色测试」。Windows 的设置应用没有 AI 对话（宿主不提供 chat），所以最后没有「AI 对话」一组。Token 存在偏好文件里，按接口地址的来源键分开存（ai_assistant.tokens）。服务商表和规则在 AiSettings.h。
  void build_ai_page(StackPanel const &page) {
    if (!indexing_ && !focus_row_.empty())
      ai_mcp_view_ = focus_row_ == L"连接 AI 助手";
    if (!indexing_) {
      auto tabs = segmented_control(
          L"AI 设置分类", {{L"assistance", L"输入辅助"}, {L"connection", L"连接助手"}},
          ai_mcp_view_ ? L"connection" : L"assistance",
          [this](std::wstring const &value) {
            ai_mcp_view_ = value == L"connection";
            queue_refresh();
          }, true, false, 3, L"ai-settings");
      tabs.HorizontalAlignment(HorizontalAlignment::Left);
      tabs.Margin(Thickness{0, 0, 0, 12});
      page.Children().Append(tabs);
    }
    if (indexing_ || !ai_mcp_view_)
      build_ai_assistance_page(page);
    if (indexing_ || ai_mcp_view_)
      build_mcp_page(page);
  }

  void build_ai_assistance_page(StackPanel const &page) {
    if (!indexing_) {
      credential_test_views_.clear();
      ai_sync_.clear();
      ai_models_host_ = nullptr;
    }
    const auto provider = document_.String(L"ai_assistant.provider", L"deepseek");
    const auto endpoint = ai_endpoint();
    if (!indexing_ && !ai_more_open_)
      // MoreOptions 的 defaultOpen：第一次显示时地址不能用就展开，之后由用户自己展开收起。
      ai_more_open_ = !endpoint.origin;

    auto service = add_group(page, L"服务");
    add_row(service, 0xE99A, L"启用 AI 辅助", L"为拼音联想提供共享配置",
            toggle_control(L"启用 AI 辅助", document_.Boolean(L"ai_assistant.enabled", false),
                           [this](bool on) {
                             ai_change([&](PreferencesDocument &doc) {
                               doc.SetBoolean(L"ai_assistant.enabled", on);
                             }, false);
                           }));
    std::vector<Option> providers;
    for (auto const &entry : ais::providers())
      providers.push_back({std::wstring(entry.id), std::wstring(entry.title)});
    auto provider_combo = select_control(
        L"AI 服务提供商", std::move(providers), provider, [this](std::wstring const &value) {
          const ais::ProviderFields current{
              document_.String(L"ai_assistant.provider", L"deepseek"),
              document_.String(L"ai_assistant.endpoint", L""),
              document_.String(L"ai_assistant.model", L"")};
          if (value == current.provider)
            return;
          const auto update = ais::provider_update(value, current);
          ai_change([&](PreferencesDocument &doc) {
            doc.SetString(L"ai_assistant.provider", update.provider);
            doc.SetString(L"ai_assistant.endpoint", update.endpoint);
            doc.SetString(L"ai_assistant.model", update.model);
          }, true);
        });
    provider_combo.Width(200);
    add_row(service, 0xE753, L"服务提供商", L"", provider_combo);
    build_ai_token_row(service, endpoint);

    const auto *preset = ais::find_provider(provider);
    std::vector<std::wstring> known_models;
    if (preset) {
      add_provider_link(service, L"AI ", preset->documentation);
      for (auto const &entry : preset->models)
        known_models.emplace_back(entry);
    }
    if (ai_models_)
      known_models.insert(known_models.end(), ai_models_->begin(), ai_models_->end());
    add_model_picker(
        service, L"模型", L"", L"AI 模型", L"ai_assistant.model", known_models,
        [this](std::wstring const &value) {
          ai_change([&](PreferencesDocument &doc) {
            doc.SetString(L"ai_assistant.model", value);
          }, false);
        },
        [this](std::wstring const &name) {
          return credential_input(name, document_.String(L"ai_assistant.model", L""), false, L"",
                                  true, [this](std::wstring const &value) {
                                    write_credential(L"ai_assistant.model", value);
                                    ai_edited(false);
                                    sync_ai();
                                    return true;
                                  });
        });
    Expander more;
    more.Header(box_value(L"更多选项"));
    more.HorizontalAlignment(HorizontalAlignment::Stretch);
    more.HorizontalContentAlignment(HorizontalAlignment::Stretch);
    // 搜索落到收在里面的行时展开。
    more.IsExpanded(ai_more_open_.value_or(false) || focus_row_ == L"接口地址" ||
                    focus_row_ == L"服务模型" || focus_row_ == L"自定义提示词");
    more.Expanding([this](Expander const &, ExpanderExpandingEventArgs const &) {
      ai_more_open_ = true;
    });
    more.Collapsed([this](Expander const &, ExpanderCollapsedEventArgs const &) {
      ai_more_open_ = false;
    });
    StackPanel rows;
    rows.Spacing(4);
    const auto endpoint_text = document_.String(L"ai_assistant.endpoint", L"");
    auto endpoint_input = credential_input(
        L"AI 接口地址", endpoint_text, false, L"", true, [this](std::wstring const &value) {
          write_credential(L"ai_assistant.endpoint", value);
          ai_edited(true);
          sync_ai();
          return true;
        });
    endpoint_input.Width(320);
    auto endpoint_card = add_row(rows, 0xE71B, L"接口地址",
                                 endpoint_text.empty() ? std::wstring() : endpoint.hint,
                                 endpoint_input);
    if (!indexing_)
      ai_sync_.push_back([this, weak = make_weak(row_subtitle(endpoint_card))] {
        if (auto line = weak.get()) {
          const auto value = document_.String(L"ai_assistant.endpoint", L"");
          set_subtitle(line, value.empty() ? std::wstring() : ai_endpoint(value).hint);
        }
      });
    // 获取模型列表和自定义提示词也是接自定义服务或代理的同一批用户才用，和接口地址一起收在这里。
    StackPanel models;
    models.Spacing(4);
    rows.Children().Append(models);
    if (!indexing_)
      ai_models_host_ = models;
    fill_ai_models(models);

    // 只编辑当前槽位（prompt_id）的提示词，存着的槽位和 prompt_id 都不动；填了就整个替换内置提示词，说明见 ais::prompt_description。
    const auto prompt_id = document_.String(L"ai_assistant.prompt_id", L"");
    auto const &slot = ais::prompt_slot(prompt_id);
    const std::wstring label = L"自定义提示词";
    const std::wstring description = ais::prompt_description(prompt_id);
    record(label, description);
    StackPanel block;
    block.Spacing(6);
    block.Padding(Thickness{25, 16, 25, 16});
    block.Children().Append(make_text(label, 14, palette_.text));
    block.Children().Append(make_text(description, 12, palette_.sub));
    TextBox prompt;
    prompt.AcceptsReturn(true);
    prompt.TextWrapping(TextWrapping::Wrap);
    prompt.MinHeight(120);
    prompt.MaxHeight(360);
    prompt.IsSpellCheckEnabled(false);
    const std::wstring prompt_key = L"ai_assistant.prompt_" + std::wstring(slot.id);
    prompt.Text(hstring(document_.String(prompt_key, L"")));
    prompt.IsEnabled(loaded_);
    A11y::SetName(prompt, L"AI 联想自定义提示词");
    prompt.TextChanged([this, prompt_key](Inspectable const &sender, TextChangedEventArgs const &) {
      const auto value = textarea_value(sender.as<TextBox>());
      if (value == document_.String(prompt_key, L""))
        return;
      write_credential(prompt_key, value);
      ai_edited(false);
      sync_ai();
    });
    block.Children().Append(prompt);
    auto prompt_card = make_card();
    prompt_card.Child(block);
    rows.Children().Append(prompt_card);
    mark_search_target(label, prompt_card);
    more.Content(rows);
    service.Children().Append(more);
    add_credential_test(
        service, L"ai.assistant", L"测试 AI 辅助配置",
        [this] {
          // aiServiceCredentialTestConfig。
          auto string = [](std::wstring const &value) {
            return JsonValue::CreateStringValue(hstring(value));
          };
          JsonObject config;
          config.SetNamedValue(L"provider", string(document_.String(L"ai_assistant.provider", L"deepseek")));
          config.SetNamedValue(L"endpoint", string(document_.String(L"ai_assistant.endpoint", L"")));
          config.SetNamedValue(L"model", string(document_.String(L"ai_assistant.model", L"")));
          config.SetNamedValue(L"token", string(ai_token(ai_endpoint())));
          return config;
        },
        [this] {
          const auto current = ai_endpoint();
          return ais::credential_test_disabled(document_.Boolean(L"ai_assistant.enabled", false),
                                               current.origin.has_value(),
                                               document_.String(L"ai_assistant.model", L""),
                                               ai_token(current));
        });
  }

  // API Token（AiApiTokenSection）：带「显示」「隐藏」的密钥框，接口地址能用时才可填，说明写着 Token 只用于哪个来源；地址不能用时是地址的说明，或请先填写接口地址。改接口地址时就地换成新来源下存的 Token。
  void build_ai_token_row(StackPanel const &group, AiEndpoint const &endpoint) {
    auto description = [](AiEndpoint const &current) {
      if (current.origin)
        return L"只用于 " + *current.origin;
      return current.hint.empty() ? std::wstring(L"请先在「更多选项」中填写有效的接口地址")
                                  : current.hint;
    };
    auto input = credential_input(
        L"AI API Token", ai_token(endpoint), true, L"", endpoint.origin.has_value(),
        [this](std::wstring const &value) {
          // 共享页的 updateToken：只写当前来源下的那一项，并清掉旧版的单个 token。换地址时框里换上的值与存的相同，不算编辑。
          const auto current = ai_endpoint();
          if (!current.origin || ai_token(current) == value)
            return true;
          document_.SetEntry(L"ai_assistant.tokens", *current.origin, value);
          document_.SetString(L"ai_assistant.token", L"");
          schedule_save();
          ai_edited(false);
          sync_ai();
          return true;
        });
    auto card = add_row(group, 0xE72E, L"API Token", description(endpoint), input);
    if (indexing_)
      return;
    ai_sync_.push_back([this, description, weak_line = make_weak(row_subtitle(card)),
                        weak_box = make_weak(input.as<StackPanel>())] {
      const auto current = ai_endpoint();
      if (auto line = weak_line.get())
        set_subtitle(line, description(current));
      auto box = weak_box.get();
      if (!box)
        return;
      const bool usable = loaded_ && current.origin.has_value();
      auto field = box.Children().GetAt(0).as<PasswordBox>();
      const auto token = ai_token(current);
      if (std::wstring_view(field.Password()) != token)
        field.Password(hstring(token));
      field.IsEnabled(usable);
      box.Children().GetAt(1).as<Button>().IsEnabled(usable);
    });
  }

  // 「服务模型」一行（获取模型列表）和获取的结果。按钮在接口地址能用时可按，获取中显示「获取中…」。整块随状态重画，见 refresh_ai_models；获取到的模型列在上面的模型下拉里，所以拿到列表后整页重画。
  void fill_ai_models(StackPanel const &host) {
    host.Children().Clear();
    const auto endpoint = ai_endpoint();
    add_row(host, 0xE895, L"服务模型",
            L"从当前服务的模型目录读取，加进上面的模型列表；服务不支持时选「自定义…」自己填写。",
            button_control(ai_models_busy_ ? L"获取中…" : L"获取模型列表",
                           [this] { fetch_ai_models(); },
                           loaded_ && !ai_models_busy_ && endpoint.origin.has_value()));
    if (!ai_models_status_.empty()) {
      auto status = group_note(ai_models_status_);
      A11y::SetLiveSetting(status, Automation::Peers::AutomationLiveSetting::Polite);
      host.Children().Append(status);
    }
  }

  // 在当前事件处理完后重画这一块：按钮不在自己的 Click 里被拆掉。
  void refresh_ai_models() {
    DispatcherQueue().TryEnqueue([weak = get_weak()] {
      if (auto self = weak.get(); self && !self->closed_ && self->ai_models_host_ &&
                                  self->current_page_ == "ai")
        self->fill_ai_models(self->ai_models_host_);
    });
  }

  // 共享页的 fetchModels：在 UI 线程之外调用 msime_client_ai_models（会联网，最长约 15 秒）。期间改了服务商、地址、模型或 Token，结果作废。宿主给的是错误码，共享页只显示它自己的兜底文字，这里照做。
  void fetch_ai_models() {
    const auto endpoint = ai_endpoint();
    if (!endpoint.origin) {
      ai_models_status_ = endpoint.hint;
      refresh_ai_models();
      return;
    }
    const auto token = ai_token(endpoint);
    if (nav::js_trim(token).empty()) {
      ai_models_status_ = L"请先填写 API Token，或使用已保存的密钥。";
      refresh_ai_models();
      return;
    }
    if (ai_models_busy_)
      return;
    const auto generation = ai_generation_;
    const auto owner = ++ai_models_request_;
    ai_models_busy_ = true;
    ai_models_status_.clear();
    refresh_ai_models();
    JsonObject request;
    request.SetNamedValue(L"endpoint", JsonValue::CreateStringValue(
                                           hstring(document_.String(L"ai_assistant.endpoint", L""))));
    request.SetNamedValue(L"token", JsonValue::CreateStringValue(hstring(token)));
    run_host(
        [body = utf8(request.Stringify())] {
          return take_response(msime_client_ai_models(
              reinterpret_cast<const uint8_t *>(body.data()), body.size()));
        },
        [this, generation, owner](Response const &response) {
          if (generation != ai_generation_ || owner != ai_models_request_)
            return;
          ai_models_busy_ = false;
          std::optional<std::vector<std::wstring>> models;
          if (response.ok) {
            try {
              std::vector<std::wstring> list;
              for (auto const &item : JsonObject::Parse(text(response.text)).GetNamedArray(L"value"))
                list.emplace_back(item.GetString().c_str());
              models = std::move(list);
            } catch (...) {
            }
          }
          if (!models) {
            ai_models_status_ = std::wstring(ais::models_failure);
            refresh_ai_models();
            return;
          }
          ai_models_status_ = ais::models_status(models->size());
          const auto next = ais::fetched_model(*models, document_.String(L"ai_assistant.model", L""));
          ai_models_ = std::move(models);
          if (next.empty()) {
            // 列表进了模型下拉，整页重画。
            queue_refresh();
            return;
          }
          // 当前模型不在列表里时换成第一个（共享页的 updateAi({model})），整页重画。
          ai_change([&](PreferencesDocument &doc) {
            doc.SetString(L"ai_assistant.model", next);
          }, false);
        });
  }

  // ---- 维护与诊断 ----

  void build_dev_page(StackPanel const &page) {
    auto diagnostics = add_group(page, L"诊断");
    bool_row(diagnostics, 0xEBE8, L"Server 端日志",
             L"排查 Server 启动和通信问题时开启。记录 Server 启停原因和各组件是否就绪，限量轮转，不记录按键、输入内容或候选文本。文件是数据目录下的 logs\\server.log，TSF 端日志也写进这个文件，复现后可直接发送。",
             L"diagnostic_log.server", false);
    bool_row(diagnostics, 0xEBE8, L"TSF 端日志",
             L"排查应用内预编辑和输入延迟时开启。日志在内存中限量缓冲，并通过独立管道批量汇总，不记录按键、输入内容或候选文本。",
             L"diagnostic_log.tsf", false);
    const auto directory = state_directory();
    // 数据目录登记在安装器写的 HKLM DataDir 里（common/StateDirectory.h），32 位和 64 位 TSF DLL 与 Server 都按它解析状态根，改它要管理员权限，搬迁时还要先停掉 Server 和看门狗；这些安装器都已经做了（「选择数据位置」一步、MigrateUserDataDir 只复制、安装成功后 FinishDataDirMove 才删旧目录），所以这里指向安装器，而不是在设置窗口里另做一套搬迁。
    add_row(diagnostics, 0xE838, L"数据目录",
            directory.empty()
                ? std::wstring(L"未找到数据目录")
                : directory.wstring() +
                      L"\n要移到其他磁盘，请下载完整安装包重新安装，在「选择数据位置」一步选一个空文件夹；词库、学习记录、皮肤和设置会自动迁移过去，旧目录随后删除。",
            button_control(L"打开", [this, directory] { open_folder(directory); },
                           !directory.empty()));

    auto settings_file = add_group(page, L"设置文件");
    add_row(settings_file, 0xEDE1, L"导出设置",
            L"把当前设置保存为文件，换电脑或重装后可以导入。语音、AI 辅助和翻译服务的配置与密钥、诊断日志、使用统计和剪贴板历史开关只属于本机，不写进文件；词库和学习记录也不在其中。",
            button_control(L"导出…", [this] { export_settings(); }, loaded_));
    add_row(settings_file, 0xE8B5, L"导入设置",
            L"用导出的设置文件替换当前设置，立即生效。本机的服务配置与密钥、诊断日志、使用统计和剪贴板历史开关保持不变。",
            button_control(L"导入…", [this] { import_settings(); }, loaded_));

  }

  // ---- AI 助手 ----

  void build_mcp_page(StackPanel const &page) {
    auto mcp = add_group(page, L"连接 AI 助手");
    record(L"连接 AI 助手", L"MCP msime-mcp Claude Desktop Cursor");
    if (!indexing_) {
      // `add_group` 返回的就是卡片里的行面板，直接放进行面板与其他工具页对齐。
      StackPanel content;
      content.Spacing(10);
      content.Padding(Thickness{0, 4, 0, 4});
      append_mcp_section(content);
      mark_search_target(L"连接 AI 助手", content);
      mcp.Children().Append(content);
    }
  }

  // ---- 关于（含并进来的帮助与反馈） ----

  // 共享设置的 `FeedbackSections`（`packages/ui/src/settings/pages/feedback-page.tsx`）在 Windows 上的样子，画在「关于」页「版本与更新」之后：「帮助」，「提交可复现的问题」的报告，「反馈与交流」的三个渠道。桌面上「帮助与反馈」并进了「关于」（`desktopMergedPages`），没有单独的页，也就没有页首那句说明。共享页的「帮助」是一个子页入口，本窗口没有子页，直接把帮助内容画在这里（`build_help_section`），「关于」因此比共享页长。报告的格式、诊断信息和 GitHub 地址的拼法在 `FeedbackReport.h`。
  void build_feedback_sections(StackPanel const &page) {
    build_help_section(page);

    auto report = add_group(page, L"提交可复现的问题");
    record(L"提交可复现的问题", L"反馈 问题报告 GitHub 复制报告");
    report.Children().Append(group_note(L"报告只在你点击按钮时生成，不会读取或上传输入历史。"));
    ComboBox kinds;
    kinds.Width(156);
    for (size_t i = 0; i < msime::settings::feedback_kinds.size(); ++i) {
      kinds.Items().Append(box_value(hstring(msime::settings::feedback_kinds[i])));
      if (msime::settings::feedback_kinds[i] == feedback_kind_)
        kinds.SelectedIndex(static_cast<int32_t>(i));
    }
    A11y::SetName(kinds, L"反馈类型");
    kinds.SelectionChanged([this](Inspectable const &sender, SelectionChangedEventArgs const &) {
      const auto index = sender.as<ComboBox>().SelectedIndex();
      if (index >= 0 && static_cast<size_t>(index) < msime::settings::feedback_kinds.size())
        feedback_kind_ = msime::settings::feedback_kinds[static_cast<size_t>(index)];
    });
    add_row(report, 0xE8EC, L"类型", L"", kinds);

    // 描述、会一起附上的信息、两个按钮和截断说明在同一张卡片里，对应共享页的 `SettingsManagerBlock`。
    StackPanel block;
    block.Spacing(10);
    block.Padding(Thickness{25, 16, 25, 16});
    TextBox detail;
    detail.Header(box_value(L"描述"));
    detail.PlaceholderText(L"发生了什么？如果和打字有关，写出输入方案、编码和期望结果。");
    detail.AcceptsReturn(true);
    detail.TextWrapping(TextWrapping::Wrap);
    detail.MaxLength(static_cast<int32_t>(msime::settings::feedback_text_limit));
    detail.MinHeight(140);
    detail.Text(hstring(feedback_detail_));
    A11y::SetName(detail, L"反馈描述");
    detail.TextChanged([this](Inspectable const &sender, TextChangedEventArgs const &) {
      feedback_detail_ = std::wstring(sender.as<TextBox>().Text().c_str());
    });
    block.Children().Append(detail);
    StackPanel attached;
    attached.Spacing(2);
    auto attached_title = make_text(L"会一起附上的信息", 12, palette_.sub);
    attached_title.FontWeight(Windows::UI::Text::FontWeight{600});
    attached.Children().Append(attached_title);
    auto attached_text = make_text(feedback_diagnostics_text(), 12, palette_.sub);
    attached_text.IsTextSelectionEnabled(true);
    attached.Children().Append(attached_text);
    block.Children().Append(attached);
    StackPanel actions;
    actions.Orientation(Orientation::Horizontal);
    actions.Spacing(8);
    auto copy_report =
        button_control(feedback_report_copied_ ? L"已复制报告" : L"复制报告", [this] {
          // 共享页只在复制成功后才显示「已复制报告」。
          if (copy_text(current_feedback_report()))
            mark_copied(feedback_report_copied_, feedback_report_timer_);
          else
            show_notice(L"复制失败，请重试");
        });
    actions.Children().Append(copy_report);
    actions.Children().Append(button_control(L"在 GitHub 提交", [this] {
      const auto url = msime::settings::feedback_issue_url(
          msime::settings::feedback_issues_url, feedback_kind_, current_feedback_report());
      open_url(url.c_str());
    }));
    block.Children().Append(actions);
    block.Children().Append(make_text(
        L"提交会打开 GitHub 并预填报告；网址长度有限，过长描述会被截断，完整内容请先复制。", 12,
        palette_.sub));
    auto card = make_card();
    card.Child(block);
    report.Children().Append(card);
    mark_search_target(L"提交可复现的问题", card);
    report.Children().Append(group_note(
        L"提交问题时建议附上系统版本、输入方案、复现步骤、相关截图，以及诊断日志中的关键片段。"));
    add_row(report, 0xEBE8, L"诊断日志", L"在维护与诊断页开启日志并打开日志目录",
            button_control(L"前往", [this] { navigate("dev", true); }));

    // 「反馈与交流」的三张渠道卡片（`feedback-channels.tsx`），每张下面是地址或群号。图标用 Segoe Fluent 的通用字形，本窗口不带品牌标志。
    const std::wstring issues(msime::settings::feedback_issues_url);
    const std::wstring group_number(msime::settings::feedback_qq_group);
    auto channels = add_group(page, L"反馈与交流");
    add_row(channels, 0xE943, L"GitHub Issues", L"适合提交可复现的问题、功能建议和开发讨论。",
            button_control(L"查看 Issues", [this, issues] { open_url(issues.c_str()); }),
            channel_code(issues.substr(std::wstring_view(L"https://").size())));
    auto copy_group =
        button_control(feedback_group_copied_ ? L"已复制" : L"复制群号", [this, group_number] {
          if (copy_text(group_number))
            mark_copied(feedback_group_copied_, feedback_group_timer_);
          else
            show_notice(L"复制失败，请重试");
        });
    add_row(channels, 0xE8BD, L"QQ 交流群", L"适合中文用户进行日常交流、测试反馈和使用讨论。",
            copy_group, channel_code(L"群号：" + group_number));
    add_row(channels, 0xE724, L"Telegram 群组", L"面向国际用户和开发者的即时讨论频道。",
            button_control(L"打开群组",
                           [this] {
                             open_url(std::wstring(msime::settings::feedback_telegram_url).c_str());
                           }),
            channel_code(L"t.me/msimegroup"));
    if (!indexing_) {
      feedback_report_button_ = copy_report.as<Button>();
      feedback_group_button_ = copy_group.as<Button>();
    }
  }

  // 分组里的一段说明文字，对应共享页的 `SettingsGroupNote`。
  TextBlock group_note(std::wstring_view value) {
    auto note = make_text(value, 12, palette_.sub);
    note.Margin(Thickness{4, 0, 4, 4});
    return note;
  }

  // 渠道卡片标题下的地址或群号，对应共享卡片里的 `<code>`，可以选中复制。
  FrameworkElement channel_code(std::wstring const &value) {
    auto code = make_text(value, 12, palette_.sub);
    code.FontFamily(Media::FontFamily(L"Consolas"));
    code.IsTextSelectionEnabled(true);
    code.Padding(Thickness{0, 0, 0, 12});
    return code;
  }

  // 附在报告里的诊断信息：本版本号、系统版本，偏好读到时再加当前的输入方案（共享页没有草稿时同样不写方案）。
  std::wstring feedback_diagnostics_text() {
    const std::wstring scheme =
        loaded_ ? document_.String(L"scheme", MSIME_EDITION_DEFAULT_SCHEME_W) : std::wstring();
    return msime::settings::feedback_diagnostics(
        app_version, windows_os_version(),
        scheme.empty() ? std::wstring_view() : msime::settings::feedback_scheme_title(scheme));
  }

  // 按当前的类型、描述和诊断信息生成报告；描述里 TextBox 的 CR 换成 LF。
  std::wstring current_feedback_report() {
    return msime::settings::feedback_report(
        feedback_kind_, msime::settings::feedback_normalize_newlines(feedback_detail_),
        feedback_diagnostics_text());
  }

  // 复制后按钮显示「已复制」1.6 秒再复原，与共享页相同；再点一次重新计时。标记跨页面重建保留，新画的按钮照样显示。
  void mark_copied(bool &copied, Microsoft::UI::Dispatching::DispatcherQueueTimer &timer) {
    copied = true;
    relabel_feedback_buttons();
    if (!timer) {
      timer = DispatcherQueue().CreateTimer();
      timer.Interval(std::chrono::milliseconds(1600));
      timer.IsRepeating(false);
      timer.Tick([this, &copied](Microsoft::UI::Dispatching::DispatcherQueueTimer const &,
                                 Inspectable const &) {
        copied = false;
        relabel_feedback_buttons();
      });
    }
    timer.Stop();
    timer.Start();
  }

  void relabel_feedback_buttons() {
    if (feedback_report_button_)
      feedback_report_button_.Content(
          box_value(feedback_report_copied_ ? L"已复制报告" : L"复制报告"));
    if (feedback_group_button_)
      feedback_group_button_.Content(box_value(feedback_group_copied_ ? L"已复制" : L"复制群号"));
  }

  // 共享设置的「帮助」页（`packages/ui/src/settings/help-settings-page.tsx`）在 Windows 上的样子：非 macOS、非移动端的那一支，文案取 `settings/platform-copy.ts` 的 Windows 分支。没有系统键盘设置按钮，也没有 iOS 和 Android 的分组。
  void build_help_section(StackPanel const &page) {
    auto help = add_group(page, L"帮助");
    const std::wstring intro =
        L"水杉输入法是一款 Windows 平台的中文输入法。目前支持 Windows 11/Windows 10 平台。";
    record(L"帮助", intro);
    auto note = make_text(intro, 14, palette_.sub);
    note.IsTextSelectionEnabled(true);
    note.Margin(Thickness{4, 0, 4, 4});
    help.Children().Append(note);
    mark_search_target(L"帮助", note);

    add_text_card(page, L"快速上手",
                  {L"安装输入法后，可以使用 Win + Space 快捷键切换到水杉输入法。默认是全拼输入法。"});
    add_text_card(
        page, L"基本功能",
        {L"支持全拼、双拼和五笔，在「输入」页切换。全拼和双拼均支持辅助码，辅助码方案目前支持自然码辅助码、蓝天小雨点、首右 2.0、首右 plus、小鹤、拼音加加和五笔 86。",
         L"语音识别和 AI 联想需要自行填入 API 和 token。云候选目前支持谷歌的云接口，请注意网络问题。",
         L"更多功能欢迎自由探索～"});
    auto more = add_group(page, L"更多");
    url_row(more, 0xE8A5, L"完整文档", L"", L"完整文档（网页）", documentation_url);
  }

  // 一个带标题的分组，里面只有一张写着几段说明文字的卡片，对应共享页的 `GroupList` + `SettingsGroupBlock`。搜索按分组标题找到它。
  void add_text_card(StackPanel const &page, std::wstring const &title,
                     std::initializer_list<std::wstring_view> paragraphs) {
    auto group = add_group(page, title);
    std::wstring detail;
    StackPanel content;
    content.Spacing(10);
    content.Padding(Thickness{25, 20, 25, 20});
    for (const auto paragraph : paragraphs) {
      auto block = make_text(paragraph, 14, palette_.text);
      block.IsTextSelectionEnabled(true);
      content.Children().Append(block);
      detail.append(paragraph);
    }
    record(title, detail);
    auto card = make_card();
    card.Child(content);
    group.Children().Append(card);
    mark_search_target(title, card);
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
      // 产品名按版本取，与窗口标题和侧栏一致；full 仍是「水杉输入法」。
      auto product = make_text(MSIME_EDITION_DISPLAY_NAME, 20, palette_.text);
      product.FontWeight(Windows::UI::Text::FontWeight{600});
      names.Children().Append(product);
      names.Children().Append(make_text(L"© 2026 Metasequoia", 12, palette_.faint));
      Grid::SetColumn(names, 1);
      identity.Children().Append(names);
      card.Child(identity);
      page.Children().Append(card);
    }

    auto updates = add_group(page, L"版本与更新");
    // 检查经由 `msime_client_update_check` 在这里执行，结果显示在该行下方；只有「前往下载」会离开窗口。
    StackPanel update_lines;
    update_lines.Spacing(4);
    update_lines.Padding(Thickness{0, 0, 0, 12});
    auto update_status = make_text(L"", 12, palette_.text);
    auto update_detail = make_text(L"", 12, palette_.sub);
    update_status.IsTextSelectionEnabled(true);
    update_detail.IsTextSelectionEnabled(true);
    update_lines.Children().Append(update_status);
    update_lines.Children().Append(update_detail);
    Button update_button;
    update_button.Height(32);
    update_button.Padding(Thickness{16, 0, 16, 0});
    update_button.Click([this](Inspectable const &, RoutedEventArgs const &) {
      on_update_button();
    });
    add_row(updates, 0xE895, L"检查更新", L"当前版本 " + app_version_label(),
            update_button, update_lines);
    if (!indexing_) {
      update_button_ = update_button;
      update_lines_ = update_lines;
      update_status_ = update_status;
      update_detail_ = update_detail;
    }
    show_update_outcome();
    url_row(updates, 0xE896, L"其他平台下载",
            L"在 macOS、Linux、iOS、Android 和鸿蒙设备上安装水杉输入法。",
            L"打开下载页", download_url);
    add_row(updates, 0xE8C8, L"复制下载链接", download_url,
            button_control(L"复制链接", [this] {
              if (copy_text(download_url))
                show_notice(L"已复制下载链接。", InfoBarSeverity::Success);
              else
                show_notice(L"复制失败，请重试");
            }));
    url_row(updates, 0xE8A5, L"历史版本", L"各版本的更新说明与安装包。", L"查看", releases_url);

    // 并进来的帮助与反馈，顺序与共享的 about-page.tsx 相同：版本与更新 → 帮助与反馈 → 许可与隐私。
    build_feedback_sections(page);

    auto privacy = add_group(page, L"许可与隐私");
    bool_row(privacy, 0xE9D2, L"匿名使用统计",
             L"仅发送匿名使用记录和崩溃摘要，不含输入内容、候选、剪贴板、账号或设备信息；关闭后不再发送，并清空待发记录。",
             L"usage_reporting", true);
    url_row(privacy, 0xEA18, L"隐私政策", L"设置保存到当前输入法数据目录，详细说明见隐私政策。",
            L"查看", privacy_url);
    url_row(privacy, 0xE8A5, L"开源许可协议", L"水杉输入法的开源许可证", L"查看", license_url);
    add_row(privacy, 0xE8A5, L"第三方组件许可", L"随安装包提供的第三方组件许可声明",
            button_control(L"查看", [this] { open_third_party_notices(); }));
  }

  // ---- 连接 AI 助手 ----

  void append_section(StackPanel const &panel, hstring heading,
                      hstring description) {
    auto heading_text = TextBlock();
    heading_text.Text(heading);
    // 13/11 DIP 是卡片内标签和说明的尺寸（`add_row`、README 的「组内使用细分隔线、13 DIP 标签和 11 DIP 说明」）；这里原来用 14 DIP，比组标题还大，卡片内外对不上。
    heading_text.FontSize(13);
    heading_text.FontWeight(Windows::UI::Text::FontWeight{600});
    heading_text.Foreground(brush(palette_.text));
    panel.Children().Append(heading_text);
    auto description_text = TextBlock();
    description_text.Text(description);
    description_text.FontSize(11);
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

  // 「连接 AI 助手」：展示助手运行的 `msime-mcp` 条目，可复制或写入 Claude Desktop、Cursor 的配置；通过 `msime_client_mcp_status` 和 `msime_client_mcp_install` 与共享设置页保持一致。
  void append_mcp_section(StackPanel const &panel) {
    append_text(panel, L"MCP（Model Context Protocol）让 AI 助手通过本机服务调用水杉输入法的功能，除了把皮肤发布到社区之外不联网。连接后，助手可以管理快捷短语和设置、制作、切换和删除候选窗口皮肤、读取打字统计和已安装皮肤，并用你在设置里登录的账号把皮肤发布到社区（助手自己不登录）；遇到卡顿或候选窗口问题时，也能读取诊断日志帮助排查。除开关诊断日志外，修改功能需要在下方明确开启对应权限。");
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
    // AcceptsReturn 必须早于 Text：TextBox 默认是单行，单行状态下写入带换行的文本会被截到第一个换行符——`config_snippet` 的 pretty JSON 第一行正好只有「{」，于是框里就只剩这一个字符，之后再打开多行也找不回来。
    code.AcceptsReturn(true);
    code.Text(snippet);
    code.IsReadOnly(true);
    code.TextWrapping(TextWrapping::NoWrap);
    code.FontFamily(Media::FontFamily(L"Consolas"));
    code.Header(box_value(L"MCP 配置"));
    // 多行 TextBox 的期望高度仍然只有一行，`AcceptsReturn` 不改变这一点：不给高度时整段配置只露出开头的「{」。给够常见片段的高度，更长的在框内滚动；不换行，所以横向也要滚动条。
    code.MinHeight(188);
    code.MaxHeight(280);
    ScrollViewer::SetVerticalScrollBarVisibility(code, ScrollBarVisibility::Auto);
    ScrollViewer::SetHorizontalScrollBarVisibility(code, ScrollBarVisibility::Auto);
    panel.Children().Append(code);
    append_text(panel,
                L"要让助手修改快捷短语和设置、为你制作、切换和删除候选窗口皮肤、把皮肤发布到社区，在 args 中加入 "
                L"--allow-write；要让它读取你的用户词库、查看编码的候选，加入 "
                L"--allow-dictionary-"
                L"read；两项都加才能增删、调整和导入词。这两项只应在你信任该助"
                L"手时开启。");
    auto copy = Button();
    copy.Content(box_value(L"复制配置"));
    // 与 `button_control` 的按钮同尺寸；这里不用它，因为点击后要拿 sender 改自己的文案。
    copy.Height(32);
    copy.Padding(Thickness{16, 0, 16, 0});
    copy.HorizontalAlignment(HorizontalAlignment::Left);
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
        // 每个助手一行，形状和本页其他组一样：左边名称与配置文件路径，右边写入按钮，窄窗口时按钮落到标签下面。原来是「小标题 + 路径 + 下面一个按钮」堆三遍，和同一页的行对不齐。
        add_row(panel, 0, name,
                std::wstring(client.GetNamedString(L"path", L"").c_str()) +
                    (configured ? L"（已连接）" : L""),
                button_control(
                    L"写入 " + name, [this, id] { write_mcp_client(id); },
                    !configured && !mcp_busy_));
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
      const auto server = mcp_server_name();
      dialog.Title(box_value(hstring(L"替换 " + name + L" 中的 " + server + L"？")));
      dialog.Content(box_value(
          hstring(name + L" 的配置里已有另一个名为 " + server +
                  L" 的服务器。替换后，它原来的命令和参数（包括手动加上的"
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
    if (!closed_ && current_page_ == "ai" && ai_mcp_view_)
      render_page(true);
  }

  PreferencesDocument document_;
  bool loaded_ = false;
  std::wstring load_error_;
  std::vector<ThemeEntry> themes_;
  std::string resolved_request_;
  std::optional<msime::windows::CandidatePalette> resolved_theme_;
  std::string unskinned_request_;
  std::optional<msime::windows::CandidatePalette> unskinned_theme_;
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
  std::wstring preview_sample_;
  std::optional<bool> preview_dark_;
  // 正在索引的折叠区（「候选窗口」页的「更多选项」、「主题」页的「自定义配色」），以及索引时记下的每行所在的折叠区，见 fold_holds_focus。
  std::wstring fold_index_;
  std::map<std::wstring, std::wstring> search_folds_;
  // 两个折叠区和「候选字体」的「自定义…」各自的状态，跨重画保留。
  bool candidate_more_open_ = false;
  bool custom_colors_open_ = false;
  bool candidate_font_custom_ = false;
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
  bool ai_mcp_view_ = false;

  // 「标点与翻译」页三个「测试配置」的结果，按服务记，跨页面重建保留（共享页 useProviderCredentials 的 credentialTests）；signature 是测试时的配置。刷新函数属于当前页面上的那一组按钮，测试结束时用它就地更新，不重画整页，免得输入框丢掉焦点和没能写入的内容。
  struct CredentialTest {
    std::wstring signature;
    bool busy = false;
    bool ok = false;
    std::wstring message;
  };
  std::map<std::wstring, CredentialTest> credential_tests_;
  std::map<std::wstring, std::function<void()>> credential_test_views_;
  // 「标点与翻译」页三个「更多选项」是否展开，跨页面重建保留：「标点」组、「候选词翻译」组的第二种语言、腾讯云的地域。地域那一个与共享页 MoreOptions 的 defaultOpen 相同，第一次画出时按已存的地域合不合规决定，之后由用户自己展开收起。
  bool punctuation_more_open_ = false;
  bool translation_more_open_ = false;
  std::optional<bool> tencent_region_open_;
  // 用户这次在「翻译服务」里选了腾讯云（共享页 useTranslationSettings 的 tencentChosen）。腾讯云默认就是 enabled，选它不改文档，只能靠这个窗口内的状态把它和「从没选过」分开；窗口关掉就忘。
  bool tencent_chosen_ = false;

  // 词库: the dictionary and code prefix being looked at, the page of words last read and what the page says about it. Kept across redraws, which every request ends with.
  std::wstring lexicon_kind_ = L"quick_phrase";
  std::wstring lexicon_query_;
  uint32_t lexicon_offset_ = 0;
  std::vector<LexiconEntry> lexicon_entries_;
  bool lexicon_has_more_ = false;
  bool lexicon_listed_ = false;
  bool lexicon_busy_ = false;
  bool lexicon_error_ = false;
  std::wstring lexicon_message_;
  // 导入词库文件的格式，跨重画保留；换到不能用它的词库类型时按标准格式显示。
  std::wstring lexicon_import_format_ = L"standard";
  // 背单词: the last status the host answered with; one request is in flight at a time.
  std::optional<VocabularyStatus> vocabulary_;
  bool vocabulary_busy_ = false;
  bool vocabulary_revealed_ = false;
  std::wstring vocabulary_message_;
  // 五笔查码: the text being looked up and the last answer, kept across redraws.
  std::wstring wubi_lookup_text_;
  bool wubi_lookup_busy_ = false;
  std::optional<WubiCodeLookup> wubi_lookup_;
  std::wstring wubi_lookup_error_;
  // A ContentDialog of this window is showing.
  bool dialog_open_ = false;
  // 辅助码方案下拉框的自定义码表和辅助码表插件（值是插件 id、标签是插件名）。自定义码表只请求一次；插件在进入「输入」页时标为过期，下一次绘制时重读，同一时间只有一次读取。
  std::vector<Option> helpcode_custom_schemas_;
  bool helpcode_schemas_requested_ = false;
  std::vector<Option> helpcode_packs_;
  bool helpcode_packs_stale_ = true;
  bool helpcode_packs_busy_ = false;

  // 屏幕键盘页：预览的拖动面（页面没画出来时为空）、正在进行的拖动和拖动时跟着移动的两条间距滑块，都随页面重建；设计里的照片按内容解码一次，拖动重画时复用。
  struct GeometryDrag {
    enum class Axis { none, key, row };
    uint32_t pointer;
    double x, y;
    double key, row;
    Axis axis;
  };
  Border osk_preview_{nullptr};
  std::optional<GeometryDrag> osk_drag_;
  Slider osk_key_slider_{nullptr};
  Slider osk_row_slider_{nullptr};
  std::wstring keyboard_photo_source_;
  Imaging::BitmapImage keyboard_photo_{nullptr};


  // 「语音输入」页：跟着输入内容就地更新的控件（每次绘制时重建）；读到的录音设备和读取失败时的说明（为空时显示「麦克风」的常规说明），跨页面重建保留（共享页 VoiceMicrophonePicker 的组件状态）；本地模型的列表、说明、各模型的安装、导入、删除状态和进度（共享页 LocalModelManager 的组件状态），local_models_generation_ 在换到或换走本地模型时递增，作废进行中的请求；进度条是当前页面上的那一组，按模型 id 就地更新；「更多选项」是否展开。
  std::vector<std::function<void()>> voice_sync_;
  std::vector<CaptureDevice> capture_devices_;
  std::wstring capture_notice_;
  bool capture_busy_ = false;
  std::optional<std::vector<LocalModel>> local_models_;
  std::wstring local_models_notice_;
  std::map<std::wstring, LocalModelProgress> local_model_progress_;
  std::set<std::wstring> local_model_installing_;
  std::set<std::wstring> local_model_importing_;
  std::set<std::wstring> local_model_removing_;
  std::map<std::wstring, std::function<void()>> local_model_views_;
  uint64_t local_models_generation_ = 0;
  bool local_models_stale_ = true;
  bool local_more_open_ = false;
  // 「输入」页「候选与联想」「输出」「逐个模式」三处「更多选项」是否展开，跨页面重建保留。
  bool candidates_more_open_ = false;
  bool output_more_open_ = false;
  bool local_modes_more_open_ = false;
  // 「语音快捷键」的「更多选项」（Ctrl+F9）是否展开；「识别服务配置」的「更多选项」是否展开，没动过时按用户改没改过接口地址或资源 ID 决定（共享页 MoreOptions 的 defaultOpen）；豆包的「流式接口」选了「自定义地址」，地址输入框照旧显示。都跨页面重建保留。
  bool voice_hotkeys_more_open_ = false;
  std::optional<bool> voice_service_more_open_;
  bool voice_custom_endpoint_ = false;
  // 每次进入语音页自动读一次录音设备列表（共享页 VoiceMicrophonePicker 变为可见时读取）；读完过一次（成功或失败）后，不在列表里的已选设备显示为未连接而不是读取中。
  bool capture_stale_ = true;
  bool capture_read_ = false;

  // 「AI 辅助」页（共享页 useAiAssistant 的状态，跨页面重建保留）：已获取的模型和获取的提示，获取是否在进行。ai_generation_ 在每次改 AI 设置时递增，作废进行中的获取；获取请求的序号作废更早的获取。「更多选项」是否展开在第一次画这一页时按接口地址能不能用决定。随输入就地更新的控件每次绘制时重建。
  std::optional<std::vector<std::wstring>> ai_models_;
  std::wstring ai_models_status_;
  // 选了「自定义…」的模型下拉（add_model_picker）：模型的键对应选的时候列出的模型，跨页面重建保留，列表换了就不算数。
  std::map<std::wstring, std::wstring> custom_model_lists_;
  bool ai_models_busy_ = false;
  uint64_t ai_generation_ = 0;
  uint64_t ai_models_request_ = 0;
  std::optional<bool> ai_more_open_;
  std::vector<std::function<void()>> ai_sync_;
  StackPanel ai_models_host_{nullptr};

  // 「账号与同步」页（共享账号页的组件状态，每次进入页面时重置）：读状态时等着的两份回答、用户和资料里已绑定的登录方式、可用的登录方式、选中的验证码方式、号码、验证码和这次的 challenge 及其过期和可重发的时刻（steady_clock 毫秒）、错误和提示、编辑资料里正在输入的昵称、「已复制」、是否在等浏览器完成 Google 登录。account_generation_ 在进出页面时递增，作废在途的回答；account_busy_ 是进行中的操作（共享页的 busy）。头像按 avatarUrl 缓存解码好的图片。设置同步卡片有自己的状态：卡片读的是哪个账号的、字段表和云端设置是否读到、云端设置文档、忙碌和一行说明；sync_generation_ 在换账号或进出页面时作废在途的回答。随时间变的两个按钮和验证码一栏是该页当前的那一组；对话框开着时 account_dialog_refresh_ 就地更新它。
  bool account_stale_ = true;
  bool account_loading_ = true;
  bool account_busy_ = false;
  uint64_t account_generation_ = 0;
  // 「绑定 QQ 群助手」卡片：已有的绑定（还没读到时为空）、输入框里的绑定码、进行中的操作和操作后的说明，进入账号页时清掉；qq_links_requested_ 记下这次进入已经读过。
  std::optional<std::vector<std::pair<std::wstring, std::wstring>>> qq_links_;
  bool qq_links_requested_ = false;
  std::wstring qq_link_code_;
  bool qq_link_busy_ = false;
  std::wstring qq_link_message_;
  std::optional<Response> account_pending_status_;
  std::optional<Response> account_pending_providers_;
  std::optional<AccountUser> account_user_;
  std::vector<std::wstring> account_providers_linked_;
  bool account_email_ = false;
  bool account_phone_ = false;
  bool account_google_ = false;
  bool account_google_waiting_ = false;
  // 读不出本机保存的登录状态（status 失败、不是取消）。只有这时才给「清除失效登录状态」；进入页面、读出状态、登录成功或清掉会话后复位。
  bool account_session_unreadable_ = false;
  AccountChannel account_channel_ = AccountChannel::none;
  std::wstring account_target_;
  std::wstring account_code_;
  std::optional<std::wstring> account_challenge_;
  int64_t account_expires_at_ = 0;
  int64_t account_resend_at_ = 0;
  std::wstring account_error_;
  std::wstring account_notice_;
  std::wstring account_name_;
  bool account_id_copied_ = false;
  std::wstring account_avatar_url_;
  std::wstring account_avatar_requested_;
  Imaging::BitmapImage account_avatar_{nullptr};
  Button account_resend_button_{nullptr};
  Button account_login_button_{nullptr};
  StackPanel account_code_panel_{nullptr};
  std::function<void()> account_dialog_refresh_;
  Microsoft::UI::Dispatching::DispatcherQueueTimer account_timer_{nullptr};
  Microsoft::UI::Dispatching::DispatcherQueueTimer account_copied_timer_{nullptr};
  std::wstring sync_user_;
  bool sync_ready_ = false;
  std::optional<JsonObject> sync_cloud_;
  bool sync_busy_ = false;
  std::wstring sync_message_;
  uint64_t sync_generation_ = 0;

  // 检查更新：状态跨页面重建保留；控件是「关于」页当前的那一组。
  bool update_checking_ = false;
  std::optional<UpdateOutcome> update_outcome_;
  Button update_button_{nullptr};
  StackPanel update_lines_{nullptr};
  TextBlock update_status_{nullptr};
  TextBlock update_detail_{nullptr};

  // 「关于」页里并进来的帮助与反馈的报告草稿和「已复制」标记：跨页面重建保留，关闭窗口即丢弃，与共享页的组件状态一样。按钮是该页当前的那一组。
  std::wstring feedback_kind_{msime::settings::feedback_kinds[0]};
  std::wstring feedback_detail_;
  bool feedback_report_copied_ = false;
  bool feedback_group_copied_ = false;
  Button feedback_report_button_{nullptr};
  Button feedback_group_button_{nullptr};
  Microsoft::UI::Dispatching::DispatcherQueueTimer feedback_report_timer_{nullptr};
  Microsoft::UI::Dispatching::DispatcherQueueTimer feedback_group_timer_{nullptr};

  // 「剪贴板」页：读到的历史、「确认清空」是否待确认、写入是否在进行，跨页面重建保留。clip_generation_ 在开关变化时递增，作废进行中的读写；clip_read_ 在写入完成时递增，作废更早发出的读取。
  std::vector<ClipboardEntry> clip_entries_;
  bool clip_clear_armed_ = false;
  bool clip_busy_ = false;
  bool clip_stale_ = true;
  uint64_t clip_read_revision_ = 0;
  uint64_t clip_generation_ = 0;
  uint64_t clip_read_ = 0;
  // 发到云剪贴板：这次进入页面查到的状态、最近一次发送的说明和是否正在发送。cloud_generation_ 作废上一次进入页面时发出的请求。
  CloudClipboardState cloud_state_ = CloudClipboardState::checking;
  bool cloud_checked_ = false;
  bool cloud_sending_ = false;
  std::wstring cloud_notice_;
  uint64_t cloud_generation_ = 0;
  std::pair<std::wstring, std::wstring> cloud_configuration_;
  std::optional<CloudClipboardRelayDraft> cloud_relay_draft_;

  // 「打字统计」页：读到的状态、出错说明、选中的日子和标签，跨页面重建保留，每次进入页面时重置（共享页的组件状态）。stats_generation_ 在进入页面时递增，作废上一次留下的请求；stats_request_ 是在途请求的序号，0 表示没有；两个时间点是共享页轮询用的 requestStartedAt 和 lastRequestAt。
  std::optional<StatisticsStatus> stats_;
  std::wstring stats_error_;
  bool stats_busy_ = true;
  std::optional<std::string> stats_selected_;
  std::optional<std::string> stats_day_;
  std::wstring stats_tab_ = L"keys";
  // 日历热力图和按日明细的横向滚动位置，跨重画保留。
  double stats_calendar_offset_ = 0;
  double stats_details_offset_ = 0;
  uint64_t stats_generation_ = 0;
  uint64_t stats_request_ = 0;
  uint64_t stats_request_counter_ = 0;
  std::chrono::steady_clock::time_point stats_request_started_{};
  std::chrono::steady_clock::time_point stats_last_request_{};
  Microsoft::UI::Dispatching::DispatcherQueueTimer stats_timer_{nullptr};
  // 「上传到云端」：读到的状态（还没读到时为空）、进行中的开关操作和操作后的说明，进入页面时清掉。窗口打开时也补传一次，那时的回答同样写进来。
  std::optional<typing::UploadStatus> stats_upload_;
  bool stats_upload_busy_ = false;
  std::wstring stats_upload_message_;

  // 「插件」页：读到的目录和它的状态、打开的视图（包和缺失的选择按类型和 id 记，每次绘制时到目录里找）、打开它的那一行和关上后焦点要落的地方、名单的草稿和已存的样子，跨页面重建保留。plugins_generation_ 在进入页面时递增，作废上一次留下的请求；plugins_busy_ 是进行中的导入、删除或保存。plugin_sound_titles_ 是搜索索引里「声音与效果」视图的标题。名单的提示和保存按钮是该视图当前的那一组。
  pg::Catalog plugin_catalog_;
  PluginCatalogState plugin_catalog_state_ = PluginCatalogState::loading;
  PluginView plugin_view_ = PluginView::list;
  std::wstring plugin_view_kind_;
  std::wstring plugin_view_id_;
  std::wstring plugin_opened_from_;
  std::wstring plugin_pending_focus_;
  bool plugins_stale_ = true;
  bool plugins_busy_ = false;
  uint64_t plugins_generation_ = 0;
  std::wstring plugins_notice_;
  std::vector<pg::Mention> plugin_mentions_;
  std::vector<pg::Mention> plugin_saved_mentions_;
  std::set<std::wstring> plugin_sound_titles_;
  // 「声音与效果」里「更多音效」是否展开，跨页面重建保留；进入插件页时读到的打字统计开关，没读到时为空。
  bool plugin_sound_more_open_ = false;
  std::optional<bool> plugin_statistics_enabled_;
  TextBlock plugin_mentions_warning_{nullptr};
  Button plugin_mentions_save_{nullptr};
  // 脚本插件的授权和输入法记下的停用（`script_grants` 的回答），还没读到时为空，跨页面重建保留。plugin_grant_ 是开着的授权对话框。
  std::optional<pg::ScriptGrants> plugin_scripts_;
  ScriptGrantDialog plugin_grant_;

  // 「主题」页的皮肤目录：上次扫描到的包、被忽略的目录和皮肤目录，跨页面重建保留。skins_pending_ 是在途的扫描，skins_busy_ 是显示「正在读取」的那种（第一次和「刷新皮肤」）；skins_signature_ 是上次落地的目录回答，自动重扫据此判断有没有变化。skins_revision_ 在每次落地时递增，作废还在解码的旧图片。skin_preview_overrides_ 是卡片上选的预览明暗，连同选它时的当前明暗。color_picker_open_ 是有取色器开着，这时不重画页面。
  std::vector<SkinPackage> skin_packages_;
  std::vector<SkinIssue> skin_issues_;
  std::wstring skins_directory_;
  std::string skins_signature_;
  bool skins_scanned_ = false;
  bool skins_stale_ = true;
  bool skins_pending_ = false;
  bool skins_manual_queued_ = false;
  bool skins_busy_ = false;
  bool skins_failed_ = false;
  bool skins_opening_ = false;
  bool skins_open_failed_ = false;
  uint64_t skins_revision_ = 0;
  std::map<std::wstring, ImageSource> skin_images_;
  std::set<std::wstring> skin_image_failures_;
  std::map<std::wstring, std::pair<bool, bool>> skin_preview_overrides_;
  bool color_picker_open_ = false;
  // 从「我的皮肤」的卡片发布后，卡片下面的那一句。
  std::wstring skin_published_notice_;

  // 「社区皮肤」「社区插件」两个标签：停在哪一边（跨页面保留，与共享外壳的 skinView、pluginView 一样）、两个图库、皮肤的分类筛选（正在请求的、上次成功列出的、按钮上显示的）和插件的类型筛选、预览图的缓存和正在显示它们的卡片、皮肤库同步是否在进行或排着下一次，以及两个发布对话框的状态。
  bool skin_community_view_ = false;
  bool plugin_community_view_ = false;
  Gallery<CommunitySkinItem> skin_gallery_;
  Gallery<CommunityPluginItem> plugin_gallery_;
  // 「我的内容」打开的社区词库与回复模板：图库、正在看的分类（dictionary 或 reply）和范围（空、saved 或 mine），以及发布对话框。
  Gallery<CommunityResourceItem> resource_gallery_;
  bool resource_view_ = false;
  std::wstring resource_kind_ = L"dictionary";
  std::wstring resource_scope_;
  ResourceEditor resource_editor_;
  std::optional<std::wstring> skin_category_request_;
  std::optional<std::wstring> skin_category_active_;
  std::optional<std::wstring> skin_category_shown_;
  std::optional<std::wstring> plugin_kind_request_;
  std::optional<std::wstring> plugin_kind_shown_;
  std::map<std::wstring, CommunityPreview> community_previews_;
  std::vector<std::wstring> community_preview_order_;
  std::multimap<std::wstring, weak_ref<Border>> community_preview_views_;
  bool skin_sync_running_ = false;
  bool skin_sync_queued_ = false;
  SkinPublish skin_publish_;
  PluginPublish plugin_publish_;

  // 系统字体列表：读取状态和结果跨页面重建保留，与共享页一样第一次点开字体输入框时才读取。控件是主题页当前的那一组。
  enum class FontCatalogStatus { idle, loading, ready, failed };
  FontCatalogStatus font_catalog_status_ = FontCatalogStatus::idle;
  std::vector<std::wstring> font_catalog_;
  uint64_t font_catalog_generation_ = 0;
  TextBlock font_catalog_text_{nullptr};
  Button font_catalog_button_{nullptr};
  StackPanel font_preset_host_{nullptr};
  std::vector<std::function<void()>> font_menu_updaters_;

  Grid root_{nullptr};
  Grid titlebar_{nullptr};
  Button back_{nullptr};
  AutoSuggestBox search_{nullptr};
  TextBlock toolbar_title_{nullptr};
  NavigationView navigation_{nullptr};
  ScrollViewer scroll_{nullptr};
  Border preview_host_{nullptr};
  Border toolbar_preview_host_{nullptr};
  TextBlock toolbar_preview_size_{nullptr};
  std::optional<JsonObject> dictionary_update_status_;
  std::wstring dictionary_update_message_;
  bool dictionary_update_busy_ = false;
  Microsoft::UI::Dispatching::DispatcherQueueTimer dictionary_update_timer_{nullptr};
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

int WINAPI wWinMain(HINSTANCE, HINSTANCE, PWSTR command, int) {
  const int gameProfileResult = msime::windows::game_profile::run_if_requested(command);
  if (gameProfileResult >= 0) return gameProfileResult;
  init_apartment(apartment_type::single_threaded);
  Application::Start([](auto &&) { make<App>(); });
  return 0;
}

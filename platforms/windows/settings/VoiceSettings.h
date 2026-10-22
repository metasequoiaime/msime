#pragma once
#include "TranslationServiceSettings.h"
#include "../../../shared/voice/PolishPromptWideCatalog.h"
#include "../../../shared/voice/VoiceLanguageCatalog.h"
#include <algorithm>
#include <array>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <map>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

// 「语音输入」页识别服务与润色的展示规则，平台固定为 Windows：共享 Rust 目录的持有模型，豆包流式接口的匹配，识别语言的建议（`voice-language-options.tsx`），内置润色方案（共享 Rust 静态目录），「测试配置」何时可用（`voice-credential-test-config.ts`）和本地模型列表的文字（`local-model-helpers.ts`）。不依赖 Windows 和 WinRT 头文件，方便单独测试。
namespace msime::settings::voice {

struct Choice {
  std::wstring_view value;
  std::wstring_view label;
};

// Rust 的语音服务目录快照。持有字符串，避免 C 接口的 JSON 缓冲区释放后留下悬空视图。
struct ProviderDefaults {
  std::wstring id;
  std::wstring endpoint;
  std::wstring model;
  std::vector<std::wstring> models;
  std::wstring documentation;
  std::wstring title;
};

// 服务顺序和名称来自共享目录；宿主在读取后追加系统识别与本地模型。
inline std::vector<Choice> asr_provider_choices(std::vector<ProviderDefaults> const &table) {
  std::vector<Choice> choices;
  for (auto const &entry : table)
    choices.push_back({entry.id, entry.title});
  return choices;
}

// 系统识别和本地模型不用云服务凭据，其余服务以共享目录为准。
inline bool is_asr_service_provider(std::vector<ProviderDefaults> const &table,
                                    std::wstring_view provider) {
  return provider != L"system" && provider != L"local" &&
         std::any_of(table.begin(), table.end(),
                     [provider](ProviderDefaults const &entry) { return entry.id == provider; });
}

inline ProviderDefaults const *find_defaults(std::vector<ProviderDefaults> const &table,
                                             std::wstring_view provider) {
  for (auto const &entry : table)
    if (entry.id == provider)
      return &entry;
  return nullptr;
}

// 共享页 providerSettingValue：去掉首尾空白后的值，空时取这个服务的默认值，没有默认值时为空。
inline std::wstring provider_setting_value(std::wstring_view value, std::wstring_view provider,
                                           std::vector<ProviderDefaults> const &table,
                                           bool endpoint) {
  const auto trimmed = js_trim(value);
  if (!trimmed.empty())
    return std::wstring(trimmed);
  const auto *defaults = find_defaults(table, provider);
  if (!defaults)
    return {};
  return std::wstring(endpoint ? defaults->endpoint : defaults->model);
}

// 共享 Rust 目录的流式接口快照，同样持有字符串。
struct StreamEndpoint {
  std::wstring id;
  std::wstring endpoint;
  std::wstring title;
};

// 空地址按共享服务默认地址匹配；目录没有该地址时显示「自定义地址」。
inline std::wstring_view doubao_stream_endpoint_id(std::vector<StreamEndpoint> const &table,
                                                   std::wstring_view endpoint,
                                                   std::wstring_view default_endpoint) {
  const auto trimmed = js_trim(endpoint);
  const auto effective = trimmed.empty() ? default_endpoint : trimmed;
  for (auto const &option : table)
    if (option.endpoint == effective)
      return option.id;
  return L"custom";
}

// 豆包资源 ID 的出厂值（defaultVoiceInput.asr_resource_id）。
inline constexpr std::wstring_view doubao_default_resource_id = L"volc.seedasr.sauc.duration";

// 「测试豆包识别配置」发出的资源 ID：去掉首尾空白，留空（缺省或用户清空后存下的 ""）时用出厂值，与 Server 录音时的 `doubao_resource_id` 和共享页的 `asrServiceCredentialTestConfig` 一致，免得检查报配置无效、录音却照常进行。
inline std::wstring doubao_test_resource_id(std::wstring_view stored) {
  const auto trimmed = js_trim(stored);
  return std::wstring(trimmed.empty() ? doubao_default_resource_id : trimmed);
}

// 「服务配置」子视图里「识别服务」的「更多选项」打开页面时就展开（advancedChanged）：用户改过接口地址（不为空，也不是这个服务的默认地址），或豆包的资源 ID 不是出厂值，免得改过的值藏起来看不见。
inline bool asr_advanced_changed(std::vector<ProviderDefaults> const &table,
                                 std::wstring_view provider, std::wstring_view endpoint,
                                 std::wstring_view resource_id) {
  const auto *defaults = find_defaults(table, provider);
  const auto stored_endpoint = js_trim(endpoint);
  const auto stored_resource = js_trim(resource_id);
  return (!stored_endpoint.empty() &&
          stored_endpoint != (defaults ? std::wstring_view(defaults->endpoint) : std::wstring_view())) ||
         (provider == L"doubao" && !stored_resource.empty() &&
          stored_resource != doubao_default_resource_id);
}

// ---- 语音快捷键（voice-hotkeys.ts） ----

// 三个长按录音的组合键，按设置页列出的先后，连同下拉里的名字和存储里缺这个键时宿主用的出厂值（client-core 的 VoiceInputPreferences::default()，只有右 Alt 默认打开）。宿主同一时刻只认一个，所以设置页把它们合成一个「长按录音」选择。
struct HoldKey {
  std::wstring_view key;
  std::wstring_view label;
  bool fallback;
};

inline constexpr std::array<HoldKey, 3> hold_keys{{
    {L"hotkey_ralt", L"右 Alt", true},
    {L"hotkey_rctrl_ralt", L"右 Ctrl+右 Alt", false},
    {L"hotkey_ctrl_win", L"Ctrl+Win", false},
}};

// 「不使用」在下拉里的值；三个组合键的值是各自的偏好名。
inline constexpr std::wstring_view no_hold_key = L"none";

// 当前选中的组合键（selectedHoldKey）：存储里打开的第一个，缺的键按 HoldKey::fallback 算；三个都关时为「不使用」。以前可以同时打开几个，这样的配置在用户改动之前照旧显示第一个，打开页面时不改写存储。`stored` 只放存储里有的键。
inline std::wstring_view selected_hold_key(std::map<std::wstring_view, bool> const &stored) {
  for (auto const &hold : hold_keys) {
    const auto found = stored.find(hold.key);
    if (found == stored.end() ? hold.fallback : found->second)
      return hold.key;
  }
  return no_hold_key;
}

// 选中 `key`（「不使用」或不认识的值时三个都关）时要写入的值（holdKeyPatch）：三个一起写，选中的那个为开，其余为关。
inline std::array<std::pair<std::wstring_view, bool>, 3> hold_key_patch(std::wstring_view key) {
  return {{{hold_keys[0].key, hold_keys[0].key == key},
           {hold_keys[1].key, hold_keys[1].key == key},
           {hold_keys[2].key, hold_keys[2].key == key}}};
}

// 豆包鉴权方式：没有存或存空时是新版 API Key。
inline std::wstring_view doubao_auth_mode(std::wstring_view stored) {
  return stored.empty() ? std::wstring_view(L"api_key") : stored;
}

// 共享页 voiceAsrTokenLabel。
inline std::wstring_view asr_token_label(std::wstring_view provider, std::wstring_view auth_mode) {
  return provider == L"doubao" && auth_mode != L"legacy" ? L"Doubao API Key" : L"识别 API Token";
}

// 「测试豆包识别配置」「测试语音识别配置」不可用：没有令牌，或豆包旧版鉴权没有 App Key（asrServiceCredentialTestDisabled）。
inline bool asr_test_disabled(std::wstring_view provider, std::wstring_view auth_mode,
                              std::wstring_view token, std::wstring_view app_key) {
  return js_trim(token).empty() ||
         (provider == L"doubao" && auth_mode == L"legacy" && js_trim(app_key).empty());
}

// 「测试语音润色配置」不可用：没有令牌（polishServiceCredentialTestDisabled）。
inline bool polish_test_disabled(std::wstring_view token) { return js_trim(token).empty(); }

// 「识别」组里「服务配置 ›」一行右侧的状态，照共享页 `desktopVoiceServiceStatus` 的 Windows 分支：Windows 的豆包没填密钥时拒绝录音，不会改用系统识别，所以照实说没填。本地识别看有没有选模型；网络服务看当前框里的令牌，空时看按服务记下的那一份（`voice_input.asr_tokens[服务]`）；豆包旧版鉴权还要 App Key。系统识别不用配置，返回空，这一行只有箭头。
inline std::wstring_view service_status(std::wstring_view provider, bool service_voice,
                                        bool local_voice, std::wstring_view model_path,
                                        std::wstring_view token, std::wstring_view saved_token,
                                        std::wstring_view app_key, std::wstring_view auth_mode) {
  if (local_voice)
    return model_path.empty() ? L"未选模型" : L"已选模型";
  if (!service_voice)
    return {};
  const bool has_token = !js_trim(token).empty() || !js_trim(saved_token).empty();
  if (provider != L"doubao")
    return has_token ? L"已填写" : L"未填写";
  const bool has_app_key = !js_trim(app_key).empty();
  const bool legacy = auth_mode == L"legacy";
  if (!has_token && (!legacy || !has_app_key))
    return L"未填写";
  if (!has_token || (legacy && !has_app_key))
    return L"未填写完整";
  return L"已填写";
}

// 识别语言的建议（VoiceLanguageOptions）：系统识别用 zh-CN 这样的区域代码，也不能自动识别；其他识别器用短代码。
inline std::vector<Choice> language_choices(bool system_voice) {
  std::vector<Choice> choices;
  for (auto const &language : msime::voice::kLanguages) {
    const auto value = system_voice ? language.system_value : language.provider_value;
    const auto title = system_voice ? language.system_title : language.provider_title;
    if (!value.empty()) choices.push_back({value, title});
  }
  return choices;
}

// 内置名称与正文读取共享 Rust 目录的宽字符串导出，保留静态字符串视图。
using PolishPreset = msime::windows::PolishPromptWideCatalogEntry;
inline constexpr auto &polish_presets = msime::windows::kPolishPromptPresetsWide;

inline constexpr std::array<Choice, 3> polish_custom_slots{{
    {L"custom_1", L"自定义一"},
    {L"custom_2", L"自定义二"},
    {L"custom_3", L"自定义三"},
}};

// 选中的方案：没有存时是「精炼整理」（PolishPromptSection 的 `promptId || "cleanup"`）。
inline std::wstring_view polish_prompt_slot(std::wstring_view stored) {
  return stored.empty() ? std::wstring_view(L"cleanup") : stored;
}

inline bool is_polish_custom_slot(std::wstring_view slot) {
  return std::any_of(polish_custom_slots.begin(), polish_custom_slots.end(),
                     [slot](Choice const &choice) { return choice.value == slot; });
}

// 内置方案的提示词；自定义方案和不认识的 id 为空。
inline std::wstring_view polish_preset_prompt(std::wstring_view slot) {
  for (auto const &preset : polish_presets)
    if (preset.id == slot)
      return preset.prompt;
  return {};
}

// 「录音后端」：Windows 上空值是「系统默认」，然后是 voiceCaptureBackendOptions 给 Windows 的两项。设置页不再画这个下拉（Windows 只有一种录音方式，只给「麦克风」下拉），这张表留作判断存着的后端是不是这台主机的。
inline constexpr std::array<Choice, 3> capture_backend_choices{{
    {L"", L"系统默认"},
    {L"auto", L"自动选择"},
    {L"windows", L"Windows Audio"},
}};

// 存着的录音后端是不是这台主机的：与宿主的 VoiceCaptureSelection 一样不分大小写。不是时宿主不理会它和它的设备、用系统默认设备录音，「麦克风」下拉也照实显示成系统默认设备。
inline bool capture_backend_supported(std::wstring_view backend) {
  std::wstring lowered(backend);
  for (auto &ch : lowered)
    if (ch >= L'A' && ch <= L'Z')
      ch = static_cast<wchar_t>(ch - L'A' + L'a');
  return std::any_of(capture_backend_choices.begin(), capture_backend_choices.end(),
                     [&lowered](Choice const &choice) { return choice.value == lowered; });
}

// ---- 本地识别模型（local-model-helpers.ts） ----

// JavaScript 的 Math.round：.5 向正无穷取整。
inline double js_round(double value) { return std::floor(value + 0.5); }

// formatModelBytes。GB 保留一位小数时与 toFixed(1) 一样，正好在一半时进位（printf 在一半时取偶数）。
inline std::wstring format_model_bytes(double bytes) {
  if (!std::isfinite(bytes) || bytes <= 0)
    return L"未知";
  wchar_t buffer[64] = {};
  if (bytes >= 1e9)
    std::swprintf(buffer, 64, L"%.1f GB", js_round(bytes / 1e9 * 10) / 10);
  else if (bytes >= 1e6)
    std::swprintf(buffer, 64, L"%.0f MB", js_round(bytes / 1e6));
  else
    std::swprintf(buffer, 64, L"%.0f KB", std::max(1.0, js_round(bytes / 1e3)));
  return buffer;
}

// localModelLanguages：认识的语言代码换成中文名，用顿号连起来。
inline std::wstring local_model_languages(std::vector<std::wstring> const &languages) {
  static constexpr std::array<Choice, 5> names{{
      {L"zh", L"中文"}, {L"en", L"英语"}, {L"ja", L"日语"}, {L"ko", L"韩语"}, {L"yue", L"粤语"}}};
  std::wstring joined;
  for (std::size_t index = 0; index < languages.size(); ++index) {
    if (index > 0)
      joined += L"、";
    std::wstring_view shown = languages[index];
    for (auto const &name : names)
      if (name.value == languages[index])
        shown = name.label;
    joined += shown;
  }
  return joined;
}

// localModelInUse：去掉首尾空白和末尾的斜杠后，模型目录就是 asr_model_path。
inline bool local_model_in_use(std::wstring_view model_path, std::wstring_view asr_model_path) {
  auto normalize = [](std::wstring_view path) {
    path = js_trim(path);
    while (!path.empty() && (path.back() == L'\\' || path.back() == L'/'))
      path.remove_suffix(1);
    return path;
  };
  const auto path = normalize(asr_model_path);
  return !path.empty() && path == normalize(model_path);
}

// localModelProgressPercent。
inline int local_model_progress_percent(std::wstring_view stage, double downloaded, double total) {
  if (!(total > 0))
    return 0;
  if (stage == L"done")
    return 100;
  return static_cast<int>(std::clamp(std::floor(downloaded / total * 100), 0.0, 100.0));
}

// localModelStageLabel。
inline std::wstring_view local_model_stage_label(std::wstring_view stage) {
  if (stage == L"download")
    return L"下载中";
  if (stage == L"import")
    return L"导入中";
  if (stage == L"verify")
    return L"校验中";
  if (stage == L"extract")
    return L"解压中";
  if (stage == L"done")
    return L"完成";
  return L"准备中";
}

// host-api 本地模型调用的错误（"local_model_network: <说明>" 这样的串）换成共享页认识的代码，与 Tauri 的 local_model_error_code 相同：冒号后的说明不给页面，长度和摘要不符都算校验不通过，不安全的压缩包和缺文件都算压缩包无效，同一个模型正在安装是 busy。
inline std::wstring local_model_error_code(std::wstring_view error) {
  const auto colon = error.find(L':');
  const auto code = colon == std::wstring_view::npos ? error : error.substr(0, colon);
  if (code == L"local_model_install_running")
    return L"busy";
  if (code == L"local_model_size_mismatch" || code == L"local_model_checksum_mismatch")
    return L"local_model_checksum_mismatch";
  if (code == L"local_model_unsafe_archive" || code == L"local_model_missing_file")
    return L"local_model_invalid_archive";
  return std::wstring(code);
}

// localModelErrorMessage：取消不算失败，没有提示。
inline std::optional<std::wstring> local_model_error_message(std::wstring_view code) {
  if (code == L"local_model_cancelled")
    return std::nullopt;
  if (code == L"local_model_network")
    return L"下载失败：无法连接下载服务器。请检查网络，或设置下载镜像后再试。";
  if (code == L"local_model_http_status")
    return L"下载失败：服务器拒绝了请求。请稍后重试，或更换下载镜像。";
  if (code == L"local_model_checksum_mismatch")
    return L"下载的文件校验不通过，已丢弃。请重试；若使用了镜像，请确认镜像内容完整。";
  if (code == L"local_model_invalid_archive")
    return L"模型文件内容不完整或不安全，已丢弃。请重试。";
  if (code == L"local_model_invalid_mirror")
    return L"下载镜像地址无效，必须以 https:// 开头。";
  if (code == L"local_model_io")
    return L"无法写入模型文件，请确认磁盘空间充足。";
  if (code == L"busy")
    return L"该模型正在下载，请等待完成或先取消。";
  if (code == L"local_model_unknown")
    return L"未知的模型。";
  if (code == L"local_model_invalid_root" || code == L"unavailable")
    return L"无法访问模型存放目录。";
  return L"操作失败，请重试。";
}

// localModelImportErrorMessage：文件对不上的几种换成针对导入的说法，其余和下载相同。
inline std::optional<std::wstring> local_model_import_error_message(std::wstring_view code) {
  if (code == L"local_model_import_missing")
    return L"所选文件里缺少这个模型需要的文件，请对照上面列出的文件全部下载后一起选中。";
  if (code == L"local_model_import_unreadable")
    return L"读取所选文件失败，请确认文件仍在原处、已完整下载到本机后重新选择。";
  if (code == L"local_model_checksum_mismatch")
    return L"所选文件和模型目录里的校验值不符，可能没下载完整或版本不对，请重新下载后再导入。";
  if (code == L"local_model_invalid_archive")
    return L"所选的模型压缩包内容不完整或不安全，已丢弃。请重新下载后再导入。";
  if (code == L"busy")
    return L"该模型正在安装，请等待完成或先取消。";
  return local_model_error_message(code);
}

} // namespace msime::settings::voice

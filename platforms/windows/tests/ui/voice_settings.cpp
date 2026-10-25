#include "../../settings/VoiceSettings.h"
#include <cstdio>
#include <string>

using namespace msime::settings::voice;
namespace {
int failures = 0;
void check(bool condition, const char *what) {
  if (!condition) {
    std::fprintf(stderr, "FAIL: %s\n", what);
    ++failures;
  }
}
// 目录由 Rust 提供；这里用固定输入验证原生控件规则，不在测试程序里调用宿主 DLL。
const std::vector<ProviderDefaults> &asr_provider_defaults() {
  static const std::vector<ProviderDefaults> table{
      {L"doubao", L"wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async", L"", {}, L"", L"豆包"},
      {L"siliconflow", L"https://api.siliconflow.cn/v1/audio/transcriptions", L"FunAudioLLM/SenseVoiceSmall", {}, L"", L"SiliconFlow"},
      {L"openai", L"https://api.openai.com/v1/audio/transcriptions", L"whisper-1", {}, L"", L"OpenAI"},
      {L"groq", L"https://api.groq.com/openai/v1/audio/transcriptions", L"whisper-large-v3-turbo", {}, L"", L"Groq"},
      {L"everyapi", L"https://api.everyapi.ai/v1/audio/transcriptions", L"volc.seedasr.sauc.duration", {}, L"", L"EveryAPI"},
      {L"mistral", L"https://api.mistral.ai/v1/audio/transcriptions", L"voxtral-mini-latest", {}, L"", L"Mistral · Voxtral"},
      {L"bailian", L"https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions", L"qwen3-asr-flash", {}, L"", L"阿里云百炼 · 千问"},
      {L"system", L"", L"", {}, L"", L"Windows 系统识别"},
      {L"local", L"", L"", {}, L"", L"本地模型（离线）"},
  };
  return table;
}
const std::vector<ProviderDefaults> &polish_provider_defaults() {
  static const std::vector<ProviderDefaults> table{
      {L"siliconflow", L"https://api.siliconflow.cn/v1/chat/completions", L"Qwen/Qwen3-8B", {}, L"", L"SiliconFlow"},
      {L"openai", L"https://api.openai.com/v1/chat/completions", L"gpt-4o-mini", {}, L"", L"OpenAI"},
      {L"deepseek", L"https://api.deepseek.com/chat/completions", L"deepseek-v4-flash", {}, L"", L"DeepSeek"},
      {L"groq", L"https://api.groq.com/openai/v1/chat/completions", L"llama-3.3-70b-versatile", {}, L"", L"Groq"},
  };
  return table;
}
std::wstring_view asr_default(std::wstring_view provider, bool endpoint) {
  const auto *defaults = find_defaults(asr_provider_defaults(), provider);
  return defaults ? (endpoint ? defaults->endpoint : defaults->model) : std::wstring_view();
}
} // namespace

int main() {
  // 共享页的服务列表：七个云服务，Windows 再加系统识别和本地模型。
  const auto choices = asr_provider_choices(asr_provider_defaults());
  check(choices.size() == 9 && choices.front().value == L"doubao" &&
            choices[6].value == L"bailian" && choices[7].value == L"system" &&
            choices[7].label == L"Windows 系统识别" && choices[8].value == L"local",
        "the Windows recognition list");
  check(is_asr_service_provider(asr_provider_defaults(), L"bailian") && is_asr_service_provider(asr_provider_defaults(), L"doubao") &&
            !is_asr_service_provider(asr_provider_defaults(), L"system") && !is_asr_service_provider(asr_provider_defaults(), L"local"),
        "service providers are the seven cloud ones");

  // 新目录的值直接影响选项和回退，不依赖编译进界面的服务名单。
  const std::vector<ProviderDefaults> catalog{
      {L"synthetic", L"https://example.invalid/new", L"fixture-model", {L"fixture-model"},
       L"https://example.invalid/docs", L"合成服务"}};
  check(asr_provider_choices(catalog)[0].label == L"合成服务" &&
            is_asr_service_provider(catalog, L"synthetic"), "injected catalog controls choices");
  // providerSettingValue：编辑过的值去掉空白，空时退回默认值，不认识的服务为空。
  check(provider_setting_value(L"  https://asr.internal.example/v1  ", L"openai",
                               asr_provider_defaults(), true) == L"https://asr.internal.example/v1",
        "an edited endpoint is trimmed");
  check(provider_setting_value(L"  ", L"openai", asr_provider_defaults(), false) == L"whisper-1",
        "a blank model falls back to the default");
  check(provider_setting_value(L"", L"unknown", polish_provider_defaults(), false).empty(),
        "an unknown provider has no default");

  // 换服务的补丁规则由共享 Rust 及 C ABI 测试验证。

  // 合成目录改变数量、顺序、标识与默认地址，原生规则必须照目录匹配。
  const std::vector<StreamEndpoint> streams{
      {L"third", L"wss://example.invalid/third", L"第三种接口"},
      {L"first", L"wss://example.invalid/first", L"第一种接口"},
      {L"second", L"wss://example.invalid/second", L"第二种接口"}};
  const auto default_endpoint = streams[2].endpoint;
  check(doubao_stream_endpoint_id(streams, L"", default_endpoint) == L"second" &&
            doubao_stream_endpoint_id(streams, L" \t\u3000", default_endpoint) == L"second",
        "blank endpoints follow the catalog default");
  check(doubao_stream_endpoint_id(streams, L" wss://example.invalid/first\u3000", default_endpoint) == L"first" &&
            doubao_stream_endpoint_id(streams, streams[0].endpoint, default_endpoint) == L"third",
        "all catalog entries match after JavaScript trimming");
  check(doubao_stream_endpoint_id(streams, L"", streams[0].endpoint) == L"third",
        "changing the catalog default changes the blank selection");
  check(doubao_stream_endpoint_id(streams, L"wss://example.invalid/custom", default_endpoint) == L"custom" &&
            doubao_stream_endpoint_id(streams, L"", L"wss://example.invalid/unlisted") == L"custom" &&
            doubao_stream_endpoint_id({}, L"", default_endpoint) == L"custom",
        "unknown or absent presets remain custom");
  auto owned_streams = streams;
  owned_streams[0].id = L"owned";
  owned_streams[0].endpoint = L"wss://example.invalid/owned";
  owned_streams[0].title = L"独立快照";
  check(streams[0].id == L"third" && streams[0].title == L"第三种接口" &&
            doubao_stream_endpoint_id(owned_streams, owned_streams[0].endpoint, default_endpoint) == L"owned",
        "stream catalog snapshots own their strings");

  // 「更多选项」在用户改过接口地址或资源 ID 时打开就展开。
  check(!asr_advanced_changed(asr_provider_defaults(), L"doubao", L"", L"") &&
            !asr_advanced_changed(asr_provider_defaults(), L"doubao", asr_default(L"doubao", true), L"volc.seedasr.sauc.duration"),
        "factory Doubao settings stay folded");
  check(asr_advanced_changed(asr_provider_defaults(), L"doubao", L"wss://example.invalid", L""), "a custom address unfolds");
  check(asr_advanced_changed(asr_provider_defaults(), L"doubao", L"", L"volc.bigasr.sauc.duration"),
        "a changed resource ID unfolds");
  check(!asr_advanced_changed(asr_provider_defaults(), L"openai", L"", L"volc.bigasr.sauc.duration") &&
            !asr_advanced_changed(asr_provider_defaults(), L"openai", asr_default(L"openai", true), L"") &&
            asr_advanced_changed(asr_provider_defaults(), L"openai", L"https://example.invalid/v1", L""),
        "only Doubao reads the resource ID; other services compare their own default address");

  // 「测试豆包识别配置」的资源 ID 与 Server 录音时一样回退到出厂值。
  check(doubao_test_resource_id(L"") == L"volc.seedasr.sauc.duration" &&
            doubao_test_resource_id(L" \t") == L"volc.seedasr.sauc.duration",
        "an empty resource ID is checked as the seedasr default");
  check(doubao_test_resource_id(L" volc.bigasr.sauc.duration ") == L"volc.bigasr.sauc.duration",
        "a stored resource ID is checked trimmed");

  // 长按录音：三个组合键合成一个选择。
  check(selected_hold_key({}) == L"hotkey_ralt", "nothing stored is the factory right Alt");
  check(selected_hold_key({{L"hotkey_ralt", false}}) == no_hold_key,
        "right Alt off with the others missing is 不使用, since the others default to off");
  check(selected_hold_key({{L"hotkey_ralt", true}, {L"hotkey_ctrl_win", true}}) == L"hotkey_ralt",
        "several on show the first, without rewriting the others");
  check(selected_hold_key({{L"hotkey_ralt", false}, {L"hotkey_rctrl_ralt", false},
                           {L"hotkey_ctrl_win", true}}) == L"hotkey_ctrl_win",
        "Ctrl+Win alone");
  const auto patch = hold_key_patch(L"hotkey_rctrl_ralt");
  check(patch[0] == std::pair<std::wstring_view, bool>{L"hotkey_ralt", false} &&
            patch[1] == std::pair<std::wstring_view, bool>{L"hotkey_rctrl_ralt", true} &&
            patch[2] == std::pair<std::wstring_view, bool>{L"hotkey_ctrl_win", false},
        "choosing a chord writes all three keys");
  const auto none = hold_key_patch(no_hold_key);
  check(!none[0].second && !none[1].second && !none[2].second, "不使用 turns all three off");

  // 缺键、显式关闭和开启的 27 种组合沿用同一优先级；查询不修改旧文档。
  const std::array<std::wstring_view, 3> hold_names{{L"hotkey_ralt", L"hotkey_rctrl_ralt", L"hotkey_ctrl_win"}};
  for (int profile = 0; profile < 27; ++profile) {
    auto remaining = profile;
    std::map<std::wstring_view, bool> stored;
    auto expected = no_hold_key;
    for (size_t index = 0; index < hold_names.size(); ++index) {
      const auto state = remaining % 3;
      remaining /= 3;
      if (state) stored[hold_names[index]] = state == 2;
      const auto enabled = state ? state == 2 : index == 0;
      if (expected == no_hold_key && enabled) expected = hold_names[index];
    }
    const auto before = stored;
    check(selected_hold_key(stored) == expected && stored == before,
          "every missing and explicit hold key profile preserves priority without writes");
  }
  check(hold_keys[0].label == L"右 Alt" && hold_keys[1].label == L"右 Ctrl+右 Alt" &&
            hold_keys[2].label == L"Ctrl+Win", "hold choices preserve Windows labels");

  // 「麦克风」下拉：空、auto、windows 是这台主机的后端，不分大小写；别的平台的后端显示成系统默认设备。
  check(capture_backend_supported(L"") && capture_backend_supported(L"auto") &&
            capture_backend_supported(L"Windows") && capture_backend_supported(L"AUTO"),
        "this host's backends ignore case");
  check(!capture_backend_supported(L"pulse") && !capture_backend_supported(L"macos") &&
            !capture_backend_supported(L"harmony"),
        "foreign backends are not this host's");

  // 令牌标签与测试按钮。
  check(asr_token_label(L"doubao", doubao_auth_mode(L"")) == L"Doubao API Key" &&
            asr_token_label(L"doubao", L"legacy") == L"识别 API Token" &&
            asr_token_label(L"openai", L"api_key") == L"识别 API Token",
        "token labels");
  check(asr_test_disabled(L"openai", L"api_key", L"  ", L""), "no token, no test");
  check(!asr_test_disabled(L"openai", L"api_key", L"t", L""), "a token is enough");
  check(asr_test_disabled(L"doubao", L"legacy", L"t", L" "), "legacy Doubao needs an App Key");
  check(!asr_test_disabled(L"doubao", L"api_key", L"t", L""), "the new Doubao key is enough");
  check(polish_test_disabled(L"\u3000") && !polish_test_disabled(L"t"), "polish token");

  // 识别语言建议与润色方案。
  check(language_choices(true).size() == 3 && language_choices(true)[0].value == L"zh-CN",
        "system recognition takes locales");
  check(language_choices(false).size() == 4 && language_choices(false)[3].value == L"auto",
        "providers take short codes and auto");
  check(polish_prompt_slot(L"") == L"cleanup" && polish_prompt_slot(L"custom_2") == L"custom_2",
        "the default polish preset");
  check(is_polish_custom_slot(L"custom_3") && !is_polish_custom_slot(L"cleanup"), "custom slots");
  check(polish_preset_prompt(L"zh2en").substr(0, 4) == L"你是中文" &&
            polish_preset_prompt(L"custom_1").empty(),
        "preset prompts");

  // 本地模型。
  check(format_model_bytes(0) == L"未知" && format_model_bytes(1.25e9) == L"1.3 GB" &&
            format_model_bytes(239.5e6) == L"240 MB" && format_model_bytes(200) == L"1 KB" &&
            format_model_bytes(1500) == L"2 KB",
        "model sizes");
  check(local_model_languages({L"zh", L"en", L"xx"}) == L"中文、英语、xx", "model languages");
  check(local_model_in_use(L"C:\\m\\a", L" C:\\m\\a\\ ") && !local_model_in_use(L"C:\\m\\a", L"") &&
            !local_model_in_use(L"C:\\m\\a", L"C:\\m\\b"),
        "the model in use");
  check(local_model_progress_percent(L"download", 50, 200) == 25 &&
            local_model_progress_percent(L"done", 0, 10) == 100 &&
            local_model_progress_percent(L"download", 5, 0) == 0 &&
            local_model_progress_percent(L"extract", 300, 200) == 100,
        "progress");
  check(local_model_stage_label(L"verify") == L"校验中" && local_model_stage_label(L"x") == L"准备中",
        "stage labels");
  check(local_model_error_code(L"local_model_size_mismatch: a.bin") ==
                L"local_model_checksum_mismatch" &&
            local_model_error_code(L"local_model_missing_file: x") == L"local_model_invalid_archive" &&
            local_model_error_code(L"local_model_install_running") == L"busy" &&
            local_model_error_code(L"local_model_network: timed out") == L"local_model_network",
        "host error codes become page codes");
  check(!local_model_error_message(L"local_model_cancelled") &&
            local_model_error_message(L"invalid local model root") == std::wstring(L"操作失败，请重试。"),
        "error messages");
  check(local_model_import_error_message(L"busy") == std::wstring(L"该模型正在安装，请等待完成或先取消。") &&
            local_model_import_error_message(L"local_model_io") ==
                std::wstring(L"无法写入模型文件，请确认磁盘空间充足。"),
        "import error messages");

  // 「服务配置 ›」的状态（desktopVoiceServiceStatus 的 Windows 分支）：豆包没填时照实说没填，不说「暂用系统识别」。
  check(service_status(L"local", false, true, L"", L"", L"", L"", L"api_key") == L"未选模型" &&
            service_status(L"local", false, true, L"C:\\models\\a", L"", L"", L"", L"api_key") ==
                L"已选模型",
        "local recognition reports the chosen model");
  check(service_status(L"system", false, false, L"", L"t", L"", L"", L"api_key").empty(),
        "system recognition has no status");
  check(service_status(L"openai", true, false, L"", L"", L"", L"", L"api_key") == L"未填写" &&
            service_status(L"openai", true, false, L"", L" \t", L"", L"", L"api_key") == L"未填写" &&
            service_status(L"openai", true, false, L"", L"", L"saved", L"", L"api_key") == L"已填写" &&
            service_status(L"openai", true, false, L"", L"t", L"", L"", L"api_key") == L"已填写",
        "network services look at the current or the saved token");
  check(service_status(L"doubao", true, false, L"", L"", L"", L"", L"api_key") == L"未填写" &&
            service_status(L"doubao", true, false, L"", L"", L"", L"key", L"api_key") == L"未填写" &&
            service_status(L"doubao", true, false, L"", L"t", L"", L"", L"api_key") == L"已填写",
        "doubao with an API key");
  check(service_status(L"doubao", true, false, L"", L"", L"", L"", L"legacy") == L"未填写" &&
            service_status(L"doubao", true, false, L"", L"t", L"", L"", L"legacy") == L"未填写完整" &&
            service_status(L"doubao", true, false, L"", L"", L"", L"key", L"legacy") ==
                L"未填写完整" &&
            service_status(L"doubao", true, false, L"", L"t", L"", L"key", L"legacy") == L"已填写",
        "doubao legacy needs both the token and the app key");

  if (failures)
    return 1;
  std::puts("voice settings: ok");
  return 0;
}

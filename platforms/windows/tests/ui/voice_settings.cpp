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
std::wstring_view asr_default(std::wstring_view provider, bool endpoint) {
  const auto *defaults = find_defaults(asr_provider_defaults(), provider);
  return defaults ? (endpoint ? defaults->endpoint : defaults->model) : std::wstring_view();
}
} // namespace

int main() {
  // 共享页的服务列表：七个云服务，Windows 再加系统识别和本地模型。
  const auto choices = asr_provider_choices();
  check(choices.size() == 9 && choices.front().value == L"doubao" &&
            choices[6].value == L"bailian" && choices[7].value == L"system" &&
            choices[7].label == L"Windows 系统识别" && choices[8].value == L"local",
        "the Windows recognition list");
  check(is_asr_service_provider(L"bailian") && is_asr_service_provider(L"doubao") &&
            !is_asr_service_provider(L"system") && !is_asr_service_provider(L"local"),
        "service providers are the seven cloud ones");

  // providerSettingValue：编辑过的值去掉空白，空时退回默认值，不认识的服务为空。
  check(provider_setting_value(L"  https://asr.internal.example/v1  ", L"openai",
                               asr_provider_defaults(), true) == L"https://asr.internal.example/v1",
        "an edited endpoint is trimmed");
  check(provider_setting_value(L"  ", L"openai", asr_provider_defaults(), false) == L"whisper-1",
        "a blank model falls back to the default");
  check(provider_setting_value(L"", L"unknown", polish_provider_defaults(), false).empty(),
        "an unknown provider has no default");

  // 选系统识别：清掉令牌，原服务的令牌记下来，换回时还原。
  ProviderFields openai{L"openai", std::wstring(asr_default(L"openai", true)), L"whisper-1",
                        L"synthetic-openai", {{L"system", L"unused-synthetic"}}};
  const auto system = asr_provider_update(L"system", openai);
  check(system.provider == L"system" && system.token.empty(), "system recognition has no token");
  check(system.endpoint && system.endpoint->empty() && system.model && system.model->empty(),
        "system recognition clears the shipped endpoint and model");
  check(system.tokens == TokenMap{{L"openai", L"synthetic-openai"}},
        "the OpenAI token is kept in its slot");
  const auto restored = asr_provider_update(
      L"openai", {L"system", *system.endpoint, *system.model, system.token, system.tokens});
  check(restored.token == L"synthetic-openai", "the OpenAI token comes back");
  check(restored.endpoint == std::wstring(asr_default(L"openai", true)) &&
            restored.model == std::wstring(L"whisper-1"),
        "the OpenAI defaults come back");
  const auto custom = asr_provider_update(
      L"system", {L"", L"https://example.invalid/custom", L"synthetic-model", L"", {}});
  check(!custom.endpoint && !custom.model, "a custom endpoint and model are kept");

  // 豆包默认的 wss 地址换成 OpenAI 的，令牌不会发给豆包。
  const auto rewritten = asr_provider_update(
      L"openai", {L"doubao", std::wstring(asr_default(L"doubao", true)), L"", L"", {}});
  check(rewritten.endpoint == std::wstring(asr_default(L"openai", true)) &&
            rewritten.model == std::wstring(L"whisper-1"),
        "the shipped Doubao endpoint is replaced");
  // 空令牌框去掉原服务那一项。
  const auto emptied = asr_provider_update(
      L"groq", {L"openai", L"", L"", L"", {{L"openai", L"old"}, {L"groq", L"g"}}});
  check(emptied.tokens == TokenMap{{L"groq", L"g"}} && emptied.token == L"g",
        "an empty box removes the old slot");
  // 不认识的服务只换令牌。
  const auto unknown = asr_provider_update(L"nobody", {L"openai", L"x", L"y", L"t", {}});
  check(!unknown.endpoint && !unknown.model && unknown.token.empty() &&
            unknown.tokens == TokenMap{{L"openai", L"t"}},
        "an unknown provider only swaps the token");

  // 润色换服务：默认模型跟着换，自己填的保留。
  const auto polish = polish_provider_update(
      L"deepseek", {L"siliconflow", L"https://api.siliconflow.cn/v1/chat/completions",
                    L"Qwen/Qwen3-8B", L"sf", {}});
  check(polish.endpoint == std::wstring(L"https://api.deepseek.com/chat/completions") &&
            polish.model == std::wstring(L"deepseek-v4-flash") && polish.token.empty() &&
            polish.tokens == TokenMap{{L"siliconflow", L"sf"}},
        "polish defaults follow the provider");
  const auto kept = polish_provider_update(L"groq", {L"openai", L"", L"my-model", L"", {}});
  check(kept.endpoint && !kept.model, "a typed polish model is kept");

  // 豆包流式接口。
  check(doubao_stream_endpoint_id(L"wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_nostream") ==
            L"nostream",
        "whole-sentence streaming");
  check(doubao_stream_endpoint_id(asr_default(L"doubao", true)) == L"async",
        "the default is bidirectional");
  check(doubao_stream_endpoint_id(L"wss://example.invalid") == L"custom", "anything else is custom");
  // 地址为空时宿主用默认的双向流式地址，下拉照实显示它，不显示成自定义。
  check(doubao_stream_endpoint_id(L"") == L"async" && doubao_stream_endpoint_id(L"  ") == L"async",
        "an empty address is the default bidirectional stream");

  // 「更多选项」在用户改过接口地址或资源 ID 时打开就展开。
  check(!asr_advanced_changed(L"doubao", L"", L"") &&
            !asr_advanced_changed(L"doubao", asr_default(L"doubao", true), L"volc.seedasr.sauc.duration"),
        "factory Doubao settings stay folded");
  check(asr_advanced_changed(L"doubao", L"wss://example.invalid", L""), "a custom address unfolds");
  check(asr_advanced_changed(L"doubao", L"", L"volc.bigasr.sauc.duration"),
        "a changed resource ID unfolds");
  check(!asr_advanced_changed(L"openai", L"", L"volc.bigasr.sauc.duration") &&
            !asr_advanced_changed(L"openai", asr_default(L"openai", true), L"") &&
            asr_advanced_changed(L"openai", L"https://example.invalid/v1", L""),
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

  if (failures)
    return 1;
  std::puts("voice settings: ok");
  return 0;
}

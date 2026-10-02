#include "VoiceProviders.h"
#include <cassert>
#include <type_traits>

int main() {
    for (const auto *provider : {"openai", "groq"}) {
        assert(msime::windows::transcription_language(provider, "zh-CN") == "zh");
        assert(msime::windows::transcription_language(provider, "zh-cn") == "zh");
        assert(msime::windows::transcription_language(provider, "en-US") == "en");
        assert(msime::windows::transcription_language(provider, "ja_JP") == "ja");
        assert(msime::windows::transcription_language(provider, "zh-Hant-TW") == "zh");
        assert(msime::windows::transcription_language(provider, "en") == "en");
        assert(msime::windows::transcription_language(provider, "AUTO").empty());
        assert(msime::windows::transcription_language(provider, "").empty());
    }
    assert(msime::windows::transcription_language("siliconflow", "zh-CN").empty());
    assert(msime::windows::transcription_language("SILICONFLOW", "en-US").empty());
    using namespace msime::voice;
    // What the language field of a request carries is checked on the wire by tests/transport.py.
    static_assert(std::is_same_v<decltype(&recognize_cloud_asr), decltype(&msime::windows::recognize_cloud_asr)>);
    assert(normalize_voice_provider("SILICONFLOW") == "siliconflow");
    // 旧的 `cloud` 别名已删除（#2830），它只是一个未知的服务商名，不再当作硅基流动。
    assert(normalize_voice_provider("CLOUD") == "cloud");
    assert(is_doubao_asr_provider("DOUBAO"));
    assert(!is_doubao_asr_provider("openai"));
    assert(resolved_asr_endpoint("openai", default_asr_endpoint("doubao")) == default_asr_endpoint("openai"));
    assert(resolved_asr_endpoint("doubao", default_asr_endpoint("groq")) == default_asr_endpoint("doubao"));
    assert(!voice_endpoint_is_websocket(default_asr_endpoint("legacy-unknown")));
    assert(default_asr_model("legacy-unknown") == "FunAudioLLM/SenseVoiceSmall");
    assert(default_asr_model("groq") == "whisper-large-v3-turbo");
    // The two transcription services carried over from the Apple provider catalogue. Both are
    // batch HTTPS, so a websocket default here would route them through the Doubao client.
    assert(default_asr_endpoint("everyapi") == "https://api.everyapi.ai/v1/audio/transcriptions");
    assert(default_asr_model("everyapi") == "openai/whisper-large-v3-turbo");
    assert(default_asr_endpoint("mistral") == "https://api.mistral.ai/v1/audio/transcriptions");
    assert(default_asr_model("mistral") == "voxtral-mini-latest");
    for (const auto *provider : {"everyapi", "mistral"}) {
        assert(!is_doubao_asr_provider(provider));
        assert(!voice_endpoint_is_websocket(default_asr_endpoint(provider)));
        assert(resolved_asr_endpoint(provider, default_asr_endpoint("doubao")) ==
               default_asr_endpoint(provider));
        assert(transcription_language(provider, "zh-CN") == "zh");
    }
    assert(default_polish_model("openai") == "gpt-4o-mini");
    auto cancelled = std::make_shared<std::atomic_bool>(true);
    bool rejected = false;
    try { recognize_cloud_asr({0.0f}, "openai", "https://example.invalid/asr", "fixture", "fixture", "en", cancelled); }
    catch (const std::exception &) { rejected = true; }
    assert(rejected);
}

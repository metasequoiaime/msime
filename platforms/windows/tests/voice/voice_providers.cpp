#include "../../../../shared/voice/VoiceProviders.h"
#include <iostream>
#include <stdexcept>
#include <string>

using namespace msime::windows;
namespace {
[[noreturn]] void require_failed(int line) {
  throw std::runtime_error("Voice provider routing failed at line " +
                           std::to_string(line));
}
#define require(value)                                                         \
  do {                                                                         \
    if (!(value))                                                              \
      require_failed(__LINE__);                                                \
  } while (false)
} // namespace
int main() {
  try {
    // The provider id decides, and only the provider id.
    require(is_doubao_asr_provider("doubao"));
    require(is_doubao_asr_provider("DOUBAO"));
    // An unset provider still means Doubao, which is why dropping the endpoint
    // check costs nothing: normalize_voice_provider already covers this.
    require(is_doubao_asr_provider(""));
    require(!is_doubao_asr_provider("openai"));
    require(!is_doubao_asr_provider("siliconflow"));
    require(!is_doubao_asr_provider("groq"));
    // A provider id this host does not know is not Doubao either.
    require(!is_doubao_asr_provider("cloud"));

    require(voice_endpoint_is_websocket("wss://openspeech.bytedance.com/api"));
    require(voice_endpoint_is_websocket("ws://localhost:9000"));
    require(!voice_endpoint_is_websocket("https://api.openai.com/v1/audio/transcriptions"));
    require(!voice_endpoint_is_websocket(""));

    require(secure_voice_endpoint("https://api.openai.com/v1/audio/transcriptions", false));
    require(secure_voice_endpoint("wss://openspeech.bytedance.com/api", true));
    require(!secure_voice_endpoint("http://api.example.invalid/asr", false));
    require(!secure_voice_endpoint("ws://api.example.invalid/asr", true));
    require(!secure_voice_endpoint("https://user:pass@example.invalid/asr", false));
    require(!secure_voice_endpoint("https:///asr", false));
    require(!secure_voice_endpoint("https://example.invalid/a b", false));
    require(!secure_voice_endpoint("https://example.invalid/asr#fragment", false));

    // Each provider's default endpoint must match its own transport, or the
    // mismatch check in VoiceInputSession would reject its own defaults.
    for (const char *provider : {"openai", "siliconflow", "groq"}) {
      const auto endpoint = default_asr_endpoint(provider);
      require(!endpoint.empty());
      require(!voice_endpoint_is_websocket(endpoint));
      require(!is_doubao_asr_provider(provider));
    }
    require(voice_endpoint_is_websocket(default_asr_endpoint("doubao")));
    require(voice_endpoint_is_websocket(default_asr_endpoint("")));

    // The defect this guards: the shipped endpoint default is Doubao's
    // websocket URL, so a stored endpoint left over from Doubao must not make
    // an OpenAI user look like a Doubao user - that sent their token to
    // ByteDance. Provider and endpoint now disagree, and the caller detects it.
    const auto stale = default_asr_endpoint("doubao");
    require(resolved_asr_endpoint("openai", stale) == default_asr_endpoint("openai"));
    require(resolved_asr_endpoint("doubao", default_asr_endpoint("openai")) ==
            default_asr_endpoint("doubao"));
    require(resolved_asr_endpoint("openai", "https://example.invalid/asr") ==
            "https://example.invalid/asr");
    require(voice_endpoint_is_websocket(stale) != is_doubao_asr_provider("openai"));
    // And the reverse: Doubao selected with an HTTPS endpoint left behind.
    require(voice_endpoint_is_websocket(default_asr_endpoint("openai")) !=
            is_doubao_asr_provider("doubao"));
    // A matching pair does not trip the check.
    require(voice_endpoint_is_websocket(default_asr_endpoint("groq")) ==
            is_doubao_asr_provider("groq"));

    // Every provider offers a usable model default except Doubao, which does
    // not take one; VoiceInputSession only requires a model for the others.
    for (const char *provider : {"openai", "siliconflow", "groq"})
      require(!default_asr_model(provider).empty());

    std::cout << "Voice providers: routing follows the provider id\n";
  } catch (const std::exception &failure) {
    std::cerr << failure.what() << '\n';
    return 1;
  } catch (...) {
    std::cerr << "Voice provider routing failed with an unknown error\n";
    return 1;
  }
}

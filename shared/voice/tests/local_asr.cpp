#include "LocalAsr.h"
#include <msime/voice/stt_service.h>
#include <cassert>
#include <filesystem>
#include <fstream>
#include <string>

namespace fs = std::filesystem;

int main() {
    using namespace msime::voice;
    // What the transducer prints for "review 一下这个 PR，然后把 CI 修好" and what a person would have typed.
    assert(tidy_local_transcript("我们今天下午要 review 一下这个 P R， 然后把 C I 的 pipeline 修好") ==
           "我们今天下午要 review 一下这个 PR，然后把 CI 的 pipeline 修好");
    assert(tidy_local_transcript("  hello  world ") == "hello world");
    assert(tidy_local_transcript("I am a B student") == "I am a B student");
    assert(tidy_local_transcript("A B C") == "ABC");
    assert(tidy_local_transcript("最后 deploy 到 staging 环境 。") == "最后 deploy 到 staging 环境。");
    assert(tidy_local_transcript("").empty());

    const auto root = fs::temp_directory_path() / "msime-local-asr-test";
    fs::remove_all(root);
    fs::create_directories(root / "model");
    assert(!is_local_model_dir(""));
    assert(!is_local_model_dir((root / "missing").u8string()));
    // A directory without the manifest is what an interrupted install leaves behind; it must not look usable.
    assert(!is_local_model_dir((root / "model").u8string()));
    std::ofstream(root / "model" / std::string(local_model_manifest))
        << R"({"kind":"offline_sense_voice","files":{},"padding":")"
        << std::string(256 * 1024, 'x') << R"("})";
    assert(!is_local_model_dir((root / "model").u8string()));
    std::ofstream(root / "model" / std::string(local_model_manifest)) << R"({"kind":"offline_sense_voice","files":{}})";
    assert(is_local_model_dir((root / "model").u8string()));

    // No runtime: the session reports it rather than crashing, and names why.
    set_sherpa_library_path((root / "no-such-runtime").u8string());
    if (!sherpa_runtime_available()) {
        assert(!sherpa_runtime_error().empty());
        bool threw = false;
        try {
            LocalAsrOptions options;
            options.model_dir = (root / "model").u8string();
            recognize_local_model({0.0f, 0.0f}, options, nullptr);
        } catch (const metasequoia::voice::VoiceError &) {
            threw = true;
        }
        assert(threw);
    }
    assert(release_local_models() == 0);
    fs::remove_all(root);
    return 0;
}

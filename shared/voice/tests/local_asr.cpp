#include "LocalAsr.h"
#include <algorithm>
#include <cassert>
#include <cmath>
#include <cstdint>
#include <filesystem>
#include <fstream>
#include <string>
#include <utility>
#include <vector>

#if !defined(_WIN32)
#include <sys/stat.h>
#endif

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
    assert(tidy_local_transcript("昨天是 monday.Today is 礼拜二。The day after tomorrow 是星期三.") ==
           "昨天是 monday. Today is 礼拜二。The day after tomorrow 是星期三。");
    assert(tidy_local_transcript("真的吗?好的!那就这样,走吧") == "真的吗？好的！那就这样，走吧");
    assert(tidy_local_transcript("版本 3.5 和 U.S. 还有 e.g. 不变") == "版本 3.5 和 U.S. 还有 e.g. 不变");
    assert(tidy_local_transcript("Done.Next step") == "Done. Next step");
    // 只有汉字后面的半角标点换成全角；其他非 ASCII 字母后面照旧。
    assert(tidy_local_transcript("我们去 café.") == "我们去 café.");
    assert(tidy_local_transcript("𠮷野家.") == "𠮷野家。");

    // X-ASR 的 token 表：汉字只以带词首标记的独立词出现。热词按 bpe 编码，字间隔空格；token 表里没有的字让整个词作废。
    {
        const std::vector<std::string> xasr = {"<blk>", "▁写", "▁上", "▁去", "▁现", "▁在", "▁时", "▁the", "▁，"};
        std::string unit;
        assert(transducer_hotword_lines({"写上去", "现在时", "GitHub 写", "写A", "鹅", "C++"}, xasr, "cjkchar+bpe", unit) ==
               "写 上 去\n现 在 时\nGitHub 写\n写 A\n");
        assert(unit == "bpe");
    }
    // 有不带前缀的汉字 token 时照清单编码：汉字连写，要求 token 表里有这个字本身。
    {
        const std::vector<std::string> chars = {"<blk>", "写", "上", "去", "▁写"};
        std::string unit;
        assert(transducer_hotword_lines({"写上去", "现在"}, chars, "cjkchar+bpe", unit) == "写上去\n");
        assert(unit == "cjkchar+bpe");
    }
    // 只有英文的词首 token（▁the）不算汉字写成独立的词。
    {
        std::string unit;
        assert(transducer_hotword_lines({"hello"}, {"▁the", "▁and"}, "bpe", unit) == "hello\n");
        assert(unit == "bpe");
        transducer_hotword_lines({}, {"▁the"}, "cjkchar+bpe", unit);
        assert(unit == "cjkchar+bpe");
    }

    // 低电平说话时 VAD 的段首比起音晚：从 VAD 段首往前找回起音，再留 0.3 秒。录音是合成的：-70 dBFS 左右的确定性噪声本底，加上约 -50 dBFS、音节之间停 0.1 秒的「语音」。
    {
        constexpr std::int64_t second = 16000;
        const auto quiet_speech = [&](std::int64_t length, std::vector<std::pair<double, double>> spans) {
            std::vector<float> audio(static_cast<std::size_t>(length));
            std::uint32_t state = 7;
            for (std::size_t i = 0; i < audio.size(); ++i) {
                state = state * 1664525u + 1013904223u;
                audio[i] = (static_cast<float>(state >> 8) / 16777216.0f - 0.5f) * 1e-3f;
                const double at = static_cast<double>(i) / second;
                for (const auto &[from, to] : spans) {
                    // 每 0.4 秒停 0.1 秒，像音节之间的间隙。
                    if (at >= from && at < to && std::fmod(at - from, 0.4) < 0.3)
                        audio[i] += 0.0045f * static_cast<float>(std::sin(at * 2 * 3.14159265 * 220));
                }
            }
            return audio;
        };
        // 说话从 1.0 秒开始，VAD 到 2.0 秒才认出来：补回 1.0 秒并再往前留 0.3 秒。
        auto audio = quiet_speech(6 * second, {{1.0, 4.0}});
        assert(local_vad_segment_start(audio.data(), audio.size(), 0, 2 * second, 0) == second - second * 3 / 10);
        // 不越过上一段的段尾，也不早于手里还留着的录音。
        assert(local_vad_segment_start(audio.data(), audio.size(), 0, 2 * second, second * 3 / 2) == second * 3 / 2);
        assert(local_vad_segment_start(audio.data() + second * 9 / 10, audio.size() - second * 9 / 10, second * 9 / 10, 2 * second, 0) == second * 9 / 10);
        // 段首之前的录音已经丢掉，或者段首在录音之外：原样返回 VAD 的段首。
        assert(local_vad_segment_start(audio.data() + 3 * second, audio.size() - 3 * second, 3 * second, 2 * second, 0) == 2 * second);
        assert(local_vad_segment_start(audio.data(), 3 * second, 0, 4 * second, 0) == 4 * second);
        // 最多往前 3 秒。
        audio = quiet_speech(8 * second, {{0.5, 6.0}});
        assert(local_vad_segment_start(audio.data(), audio.size(), 0, 5 * second, 0) == 2 * second);
        // 段首前面隔着超过 0.25 秒静音的别的声音不算进来。
        audio = quiet_speech(6 * second, {{0.2, 0.5}, {1.1, 4.0}});
        assert(local_vad_segment_start(audio.data(), audio.size(), 0, 2 * second, 0) == second * 11 / 10 - second * 3 / 10);
        // 段首前只有噪声本底：只留固定的 0.3 秒。
        audio = quiet_speech(6 * second, {{2.0, 4.0}});
        assert(local_vad_segment_start(audio.data(), audio.size(), 0, 2 * second, 0) == 2 * second - second * 3 / 10);
        // 开头是数字静音（还没有任何本底）时同样找得到起音。
        audio = quiet_speech(6 * second, {{1.0, 4.0}});
        std::fill(audio.begin(), audio.begin() + second, 0.0f);
        assert(local_vad_segment_start(audio.data(), audio.size(), 0, second * 3 / 2, 0) == second - second * 3 / 10);
    }

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
    assert(!local_model_uses_pinyin_hotwords((root / "model").u8string()));
    std::ofstream(root / "model" / std::string(local_model_manifest))
        << R"({"kind":"offline_sense_voice","files":{},"hotwords":"pinyin"})";
    assert(local_model_uses_pinyin_hotwords((root / "model").u8string()));
    std::ofstream(root / "model" / std::string(local_model_manifest))
        << R"({"kind":"offline_sense_voice","files":{},"hotwords":")"
        << std::string(256 * 1024, 'x') << R"("})";
    assert(!local_model_uses_pinyin_hotwords((root / "model").u8string()));

#if defined(__linux__)
    // Linux distributions may keep `/bin` as a root-owned system link (for
    // example to `/usr/bin`).  The Rust installer trusts that link, so the
    // native recognizer must accept the same path to the already installed
    // model instead of making the install unusable.
    struct stat bin_link {};
    struct stat bin_parent {};
    if (::lstat("/bin", &bin_link) == 0 && S_ISLNK(bin_link.st_mode) &&
        bin_link.st_uid == 0 && ::stat("/", &bin_parent) == 0 &&
        S_ISDIR(bin_parent.st_mode) && bin_parent.st_uid == 0 &&
        (bin_parent.st_mode & 022) == 0) {
      const auto through_system_link = fs::path("/bin") / ".." / root.relative_path() / "model";
      assert(is_local_model_dir(through_system_link.u8string()));
    }
#endif

    assert(local_asr_thread_count(5) == 4);
    // 灵敏度不是录音增益：只反向映射实际 VAD 阈值，默认与非法值保持原值。
    assert(LocalAsrOptions{}.vad_sensitivity_percent == 0);
    assert(local_asr_vad_threshold(0) == 0.5f);
    assert(local_asr_vad_threshold(-1) == 0.5f);
    assert(local_asr_vad_threshold(9) == 0.5f);
    assert(local_asr_vad_threshold(91) == 0.5f);
    assert(local_asr_vad_threshold(50) == 0.5f);
    assert(std::abs(local_asr_vad_threshold(10) - 0.9f) < 0.00001f);
    assert(std::abs(local_asr_vad_threshold(90) - 0.1f) < 0.00001f);
    for (int sensitivity = 11; sensitivity <= 90; ++sensitivity)
        assert(local_asr_vad_threshold(sensitivity) < local_asr_vad_threshold(sensitivity - 1));

    // 会话 API 自身必须执行已安装模型边界；独立 helper 直接构造会话，不经过 provider 封装层。
    set_sherpa_library_path((root / "no-such-runtime").u8string());
    bool rejected_uninstalled = false;
    try {
        LocalAsrOptions options;
        options.model_dir = (root / "missing").u8string();
        LocalAsrSession session(options, nullptr);
    } catch (const VoiceError &error) {
        rejected_uninstalled = std::string(error.what()).find("Not an installed local speech model") != std::string::npos;
    }
    assert(rejected_uninstalled);

#if !defined(_WIN32)
    const auto external = root / "external";
    fs::create_directories(external);
    std::ofstream(external / std::string(local_model_manifest)) << "{}";
    fs::create_symlink(external, root / "linked-model");
    assert(!is_local_model_dir((root / "linked-model").u8string()));
    bool rejected_linked = false;
    try {
        LocalAsrOptions options;
        options.model_dir = (root / "linked-model").u8string();
        LocalAsrSession session(options, nullptr);
    } catch (const VoiceError &error) {
        rejected_linked = std::string(error.what()).find("Not an installed local speech model") != std::string::npos;
    }
    assert(rejected_linked);
    fs::remove(root / "linked-model");

    // A symlinked ancestor is just as much an escape from the configured model tree as a symlinked model directory.
    const auto nested_model = external / "nested-model";
    fs::create_directories(nested_model);
    std::ofstream(nested_model / std::string(local_model_manifest)) << "{}";
    fs::create_symlink(external, root / "linked-parent");
    assert(!is_local_model_dir((root / "linked-parent" / "nested-model").u8string()));
    fs::remove(root / "linked-parent");

    const auto external_manifest = external / "external-manifest.json";
    std::ofstream(external_manifest) << "{}";
    fs::remove(root / "model" / std::string(local_model_manifest));
    fs::create_symlink(external_manifest, root / "model" / std::string(local_model_manifest));
    assert(!is_local_model_dir((root / "model").u8string()));
    fs::remove(root / "model" / std::string(local_model_manifest));

    // 模型清单属于私有安装状态，硬链接可能让识别器通过模型目录读取无关 inode。
    fs::create_hard_link(external_manifest,
                         root / "model" / std::string(local_model_manifest));
    assert(!is_local_model_dir((root / "model").u8string()));
    fs::remove(root / "model" / std::string(local_model_manifest));

    // A replaced manifest must not block the recognizer on a FIFO.
    const auto manifest_fifo = root / "model" / std::string(local_model_manifest);
    assert(::mkfifo(manifest_fifo.c_str(), 0600) == 0);
    assert(!is_local_model_dir((root / "model").u8string()));
    assert(!local_model_uses_pinyin_hotwords((root / "model").u8string()));
    fs::remove(manifest_fifo);
#endif

    // No runtime: the session reports it rather than crashing, and names why.
    set_sherpa_library_path((root / "no-such-runtime").u8string());
    if (!sherpa_runtime_available()) {
        assert(!sherpa_runtime_error().empty());
        bool threw = false;
        try {
            LocalAsrOptions options;
            options.model_dir = (root / "model").u8string();
            recognize_local_model({0.0f, 0.0f}, options, nullptr);
        } catch (const VoiceError &) {
            threw = true;
        }
        assert(threw);
    }
    assert(release_local_models() == 0);
    fs::remove_all(root);
    return 0;
}

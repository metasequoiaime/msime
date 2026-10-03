#include "../../src/core/TypingStatistics.h"
#include "msime_client.h"

#include <cassert>
#include <cstring>
#include <filesystem>
#include <string>

using msime::mac::ResolveTypingSource;
using msime::mac::TypingSource;

static std::string call(const std::filesystem::path &directory, const char *action) {
    const std::string request = "{\"directory\":\"" + directory.string() + "\",\"action\":" + action + "}";
    char *raw = msime_client_typing_statistics(reinterpret_cast<const uint8_t *>(request.data()), request.size());
    assert(raw);
    std::string result(raw);
    msime_client_string_free(raw);
    return result;
}

int main() {
    assert(ResolveTypingSource(0, false, false, "none", "xiaohe") == TypingSource::Quanpin);
    assert(ResolveTypingSource(0, true, false, "none", "xiaohe") == TypingSource::NineKey);
    assert(ResolveTypingSource(1, false, false, "none", "ziranma") == TypingSource::Ziranma);
    assert(ResolveTypingSource(1, false, false, "none", "microsoft") == TypingSource::Microsoft);
    assert(ResolveTypingSource(1, false, false, "none", "shoudao") == TypingSource::Shoudao);
    assert(ResolveTypingSource(2, false, false, "none", "xiaohe") == TypingSource::Wubi);
    assert(ResolveTypingSource(3, false, false, "none", "xiaohe") == TypingSource::Japanese);
    assert(ResolveTypingSource(4, false, false, "none", "xiaohe") == TypingSource::Korean);
    assert(msime::mac::TypingSourceId(TypingSource::Korean) == "korean");
    assert(ResolveTypingSource(5, false, false, "none", "xiaohe") == TypingSource::Cantonese);
    assert(ResolveTypingSource(6, false, false, "none", "xiaohe") == TypingSource::Zhuyin);
    assert(ResolveTypingSource(7, false, false, "none", "xiaohe") == TypingSource::Vietnamese);
    assert(ResolveTypingSource(8, false, false, "none", "xiaohe") == TypingSource::Tibetan);
    assert(msime::mac::TypingSourceId(TypingSource::Cantonese) == "cantonese");
    assert(msime::mac::TypingSourceId(TypingSource::Zhuyin) == "zhuyin");
    assert(msime::mac::TypingSourceId(TypingSource::Vietnamese) == "vietnamese");
    assert(msime::mac::TypingSourceId(TypingSource::Tibetan) == "tibetan");
    // A local mode still wins inside the new schemes, as it does inside the others.
    assert(ResolveTypingSource(7, false, false, "emoji", "xiaohe") == TypingSource::Local);
    assert(ResolveTypingSource(8, false, true, "none", "xiaohe") == TypingSource::English);
    assert(ResolveTypingSource(5, false, true, "none", "xiaohe") == TypingSource::English);
    assert(ResolveTypingSource(0, false, true, "none", "xiaohe") == TypingSource::English);
    assert(ResolveTypingSource(0, false, false, "temporary_japanese", "xiaohe") == TypingSource::Japanese);
    assert(ResolveTypingSource(0, false, false, "emoji", "xiaohe") == TypingSource::Local);
    assert(ResolveTypingSource(99, false, false, "none", "xiaohe") == TypingSource::Unknown);
    assert(msime::mac::TypingSourceId(TypingSource::NineKey) == "nineKey");

    // Keys handed back to the application: printable characters count, Option characters included because Option is the macOS character layer; control characters, DEL, lone surrogates, AppKit function keys and Command/Control chords do not.
    using msime::mac::ShouldCountPassthroughCharacter;
    for (const char16_t counted : {u'a', u'A', u'1', u'@', u' ', u'\u20AC'})
        assert(ShouldCountPassthroughCharacter(counted, false, false));
    for (const char16_t rejected : {char16_t{0x1F}, u'\r', u'\t', char16_t{0x7F}, char16_t{0xD800}, char16_t{0xDFFF},
                                    char16_t{0xF700}, char16_t{0xF704}, char16_t{0xF8FF}})
        assert(!ShouldCountPassthroughCharacter(rejected, false, false));
    assert(!ShouldCountPassthroughCharacter(u'c', true, false));
    assert(!ShouldCountPassthroughCharacter(u'v', false, true));
    static_assert(ShouldCountPassthroughCharacter(u'x', false, false));

    const auto directory = std::filesystem::temp_directory_path() / "msime-macos-typing-statistics-test";
    std::filesystem::remove_all(directory);
    const std::string directoryString = directory.string();
    assert(msime_client_typing_statistics_enabled(
               reinterpret_cast<const uint8_t *>(directoryString.data()), directoryString.size()) == 0);
    // Statistics are off until the user turns them on, as the baseline ships them, and a record into
    // a store that is off counts nothing. Turning them on here is what the settings page does before
    // any of this is reachable.
    assert(call(directory, "{\"operation\":\"set_enabled\",\"enabled\":true}").find("\"enabled\":true") !=
           std::string::npos);
    assert(msime_client_typing_statistics_enabled(
               reinterpret_cast<const uint8_t *>(directoryString.data()), directoryString.size()) == 1);
    const auto result = call(directory, "{\"operation\":\"record\",\"text\":\"合成🌲\",\"source\":\"japanese\",\"day\":\"2026-09-15\"}");
    assert(result.find("\"recorded\":3") != std::string::npos);
    // The id ResolveTypingSource gives Korean commits is one the shared store accepts.
    const auto korean = call(directory, "{\"operation\":\"record\",\"text\":\"한글\",\"source\":\"korean\",\"day\":\"2026-09-15\"}");
    assert(korean.find("\"recorded\":2") != std::string::npos);
    // 它给粤拼、注音、越南文和藏文上屏的标识，共享存储同样接受。
    for (const char *source : {"cantonese", "zhuyin", "vietnamese", "tibetan"}) {
        const std::string action = std::string("{\"operation\":\"record\",\"text\":\"字\",\"source\":\"") + source + "\",\"day\":\"2026-09-15\"}";
        const auto recorded = call(directory, action.c_str());
        assert(recorded.find("\"recorded\":1") != std::string::npos);
    }
    const auto loaded = call(directory, "{\"operation\":\"load\"}");
    assert(loaded.find("\"total\":9") != std::string::npos);
    for (const char *source : {"\"cantonese\":1", "\"zhuyin\":1", "\"vietnamese\":1", "\"tibetan\":1"})
        assert(loaded.find(source) != std::string::npos);
    assert(loaded.find("合成🌲") == std::string::npos);
    std::filesystem::remove_all(directory);
    return 0;
}

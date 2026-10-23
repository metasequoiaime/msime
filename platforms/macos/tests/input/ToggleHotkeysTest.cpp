// 可重绑切换热键的认键逻辑：ToggleHotkeys.h 的规范名解码、chord 命中和全半角两
// 条触发臂，以及它和 msime_client_toggle_hotkey_chords 回答之间的一致性。
#include "../../src/input/ToggleHotkeys.h"
#include "msime_client.h"
#include <cassert>
#include <cstring>
#include <string>

using msime::mac::ToggleChord;
using msime::mac::ToggleFullWidthArm;

// 回答 JSON 里 "<field>" 一节（紧凑对象，到第一个 } 为止），取不到是空串。
static std::string ChordSection(const std::string &response, const char *field) {
    const std::string key = std::string("\"") + field + "\":";
    const size_t at = response.find(key);
    if (at == std::string::npos) return "";
    const size_t end = response.find('}', at + key.size());
    return end == std::string::npos ? "" : response.substr(at, end - at);
}

static std::string Chords(const char *keybindingsJson) {
    const std::string request = std::string("{\"keybindings\":") + keybindingsJson + "}";
    char *raw = msime_client_toggle_hotkey_chords(
        reinterpret_cast<const uint8_t *>(request.data()), request.size());
    assert(raw != nullptr);
    std::string response(raw);
    msime_client_string_free(raw);
    return response;
}

int main() {
    // 规范名解码保留 ANSI 位置；匹配时可打印主键使用无修饰布局字符。
    {
        const ToggleChord e = msime::mac::ToggleChordForKeyName("KeyE", true, false, true, false);
        assert(e.bound && e.keyCode == 0 && e.letter == 'e' && e.control && e.shift && !e.option && !e.command);
        const ToggleChord comma = msime::mac::ToggleChordForKeyName("Comma", true, false, false, false);
        assert(comma.bound && comma.keyCode == 43 && comma.letter == '\0');
        const ToggleChord space = msime::mac::ToggleChordForKeyName("Space", true, false, true, false);
        assert(space.keyCode == 49);
        const ToggleChord f9 = msime::mac::ToggleChordForKeyName("F9", true, false, false, false);
        assert(f9.keyCode == 101);
        const ToggleChord d3 = msime::mac::ToggleChordForKeyName("Digit3", false, false, false, false);
        assert(d3.keyCode == 20 && !d3.control && !d3.option && !d3.shift && !d3.command);
    }
    // 规范名区分大小写、只认契约写法：别名、大小写变体、未知名字一律不绑定（它们
    // 在 Rust 解析层就已归一或拒绝，到不了这里）。
    for (const char *name : {"", "keye", "KeyEE", "Key1", "ctrl", "Period."})
        assert(!msime::mac::ToggleChordForKeyName(name, true, false, false, false).bound);
    assert(!msime::mac::ToggleChordForKeyName(nullptr, true, false, false, false).bound);
    // chord 命中：主键按认法，修饰键四旗精确相等。
    {
        const ToggleChord period = msime::mac::FactoryPunctuationToggleChord();
        assert(msime::mac::ToggleChordMatches(period, 47, '\0', true, false, false, false));
        // 旧行为同样排除小键盘句号与任何外加修饰键。
        assert(!msime::mac::ToggleChordMatches(period, 65, '\0', true, false, false, false));
        for (int mask = 0; mask < 16; ++mask)
            assert(msime::mac::ToggleChordMatches(period, 47, '\0', mask & 1, mask & 2, mask & 4, mask & 8) == (mask == 1));
        assert(!msime::mac::ToggleChordMatches(period, 43, '\0', true, false, false, false));
        // 字母主键按布局字母认：同一个 keyCode 打不出这个字母时不命中，别的键打出它命中。
        const ToggleChord eng = msime::mac::FactoryEnglishCandidatesToggleChord();
        assert(msime::mac::ToggleChordMatches(eng, 14, 'e', true, false, true, false));
        assert(msime::mac::ToggleChordMatches(eng, 2, 'e', true, false, true, false)); // Dvorak 的 E
        assert(msime::mac::ToggleChordMatches(eng, 14, '\0', true, false, true, false));
        assert(!msime::mac::ToggleChordMatches(eng, 14, 'e', true, false, false, false));
        // 不绑定的 chord 什么都不认。
        assert(!msime::mac::ToggleChordMatches(msime::mac::UnboundToggleChord(), 47, '\0', true, false, false, false));
    }
    // 全半角两条触发臂：chord 命中优先，Option+Shift+H 是独立的一臂。
    {
        const ToggleChord space = msime::mac::FactoryFullWidthToggleChord();
        assert(msime::mac::FullWidthToggleArmFor(space, 49, '\0', true, false, true, false) == ToggleFullWidthArm::chord);
        assert(msime::mac::FullWidthToggleArmFor(space, 4, 'h', false, true, true, false) == ToggleFullWidthArm::optionShiftH);
        assert(msime::mac::FullWidthToggleArmFor(space, 49, ' ', false, false, false, false) == ToggleFullWidthArm::none);
        // chord 绑在别的键上时 Option+Shift+H 仍命中自己的一臂；绑到同一个组合时以绑定为准（chord 臂）。
        const ToggleChord h = msime::mac::ToggleChordForKeyName("KeyH", false, true, true, false);
        assert(msime::mac::FullWidthToggleArmFor(h, 4, 'h', false, true, true, false) == ToggleFullWidthArm::chord);
        const ToggleChord g = msime::mac::ToggleChordForKeyName("KeyG", true, false, true, false);
        assert(msime::mac::FullWidthToggleArmFor(g, 49, '\0', true, false, true, false) == ToggleFullWidthArm::none);
        assert(msime::mac::FullWidthToggleArmFor(g, 4, 'h', false, true, true, false) == ToggleFullWidthArm::optionShiftH);
    }
    // FFI 回答与本机解码一致：出厂值、重绑、不绑定三种形态。
    {
        const std::string defaults = Chords("{}");
        assert(ChordSection(defaults, "punctuation").find("\"key\":\"Period\"") != std::string::npos);
        assert(ChordSection(defaults, "punctuation").find("\"ctrl\":true") != std::string::npos);
        assert(ChordSection(defaults, "full_width").find("\"key\":\"Space\"") != std::string::npos);
        assert(ChordSection(defaults, "full_width").find("\"shift\":true") != std::string::npos);
        assert(ChordSection(defaults, "english_candidates").find("\"key\":\"KeyE\"") != std::string::npos);
        // null keybindings 与缺键一样按出厂值。
        assert(Chords("null") == defaults);
        const std::string rebound = Chords("{\"toggle_punctuation\":\"Ctrl+Comma\",\"toggle_full_width\":\"\"}");
        const std::string comma = ChordSection(rebound, "punctuation");
        assert(comma.find("\"key\":\"Comma\"") != std::string::npos && comma.find("\"ctrl\":true") != std::string::npos &&
               comma.find("\"shift\":false") != std::string::npos);
        assert(rebound.find("\"full_width\":null") != std::string::npos);
        // 非法写法与非法表都不影响其它键：前者按不绑定，后者整个请求报错。
        assert(Chords("{\"toggle_punctuation\":\"Ctrl+Bogus\"}").find("\"punctuation\":null") != std::string::npos);
        assert(Chords("\"invalid\"").find("\"ok\":false") != std::string::npos);
    }
    return 0;
}

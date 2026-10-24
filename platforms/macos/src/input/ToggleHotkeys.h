#pragma once

// 三个可重绑切换热键（中英 `keybindings.toggle_english_candidates`、全半角
// `toggle_full_width`、标点 `toggle_punctuation`）在宿主侧的认键逻辑。
//
// 写法（"Ctrl+Shift+KeyE" 这类串）的解析只在 Rust 一侧：共享契约是 client-core
// 的 `KeyChord`，偏好文档经 msime_client_toggle_hotkey_chords 给出每个键的主键
// 规范名与修饰键集合，宿主把规范名解成 ToggleChord 后按事件认键——可打印主键按
// 当前布局的无修饰字符认，空格和功能键按 ANSI keyCode；修饰键四旗
// 精确相等才算命中，与各快捷键分支一直以来的掩码语义一致。

namespace msime::mac
{

struct ToggleChord
{
    bool bound;
    unsigned short keyCode; // 非字母主键的 ANSI keyCode；主键是字母时为 0。
    char letter;            // 主键是字母时的小写布局字母；否则为 0。
    bool control;
    bool option;
    bool shift;
    bool command;
};

constexpr ToggleChord UnboundToggleChord()
{
    return {false, 0, '\0', false, false, false, false};
}

constexpr bool ToggleChordNameEquals(const char *a, const char *b)
{
    while (*a && *a == *b) { ++a; ++b; }
    return *a == *b;
}

namespace detail
{
struct ToggleChordKeyCode { const char *name; unsigned short keyCode; char character; };

// 规范主键、ANSI 位置与美式字符。物理位置只用于 DOM 录入解码、空格/功能键，以及
// 非拉丁或无字符事件的回退；可打印主键的匹配与录入共用无修饰布局字符。
constexpr ToggleChordKeyCode kToggleChordKeyCodes[] = {
    {"KeyA", 0, 'a'}, {"KeyB", 11, 'b'}, {"KeyC", 8, 'c'}, {"KeyD", 2, 'd'},
    {"KeyE", 14, 'e'}, {"KeyF", 3, 'f'}, {"KeyG", 5, 'g'}, {"KeyH", 4, 'h'},
    {"KeyI", 34, 'i'}, {"KeyJ", 38, 'j'}, {"KeyK", 40, 'k'}, {"KeyL", 37, 'l'},
    {"KeyM", 46, 'm'}, {"KeyN", 45, 'n'}, {"KeyO", 31, 'o'}, {"KeyP", 35, 'p'},
    {"KeyQ", 12, 'q'}, {"KeyR", 15, 'r'}, {"KeyS", 1, 's'}, {"KeyT", 17, 't'},
    {"KeyU", 32, 'u'}, {"KeyV", 9, 'v'}, {"KeyW", 13, 'w'}, {"KeyX", 7, 'x'},
    {"KeyY", 16, 'y'}, {"KeyZ", 6, 'z'},
    {"Digit1", 18, '1'}, {"Digit2", 19, '2'}, {"Digit3", 20, '3'}, {"Digit4", 21, '4'}, {"Digit5", 23, '5'},
    {"Digit6", 22, '6'}, {"Digit7", 26, '7'}, {"Digit8", 28, '8'}, {"Digit9", 25, '9'}, {"Digit0", 29, '0'},
    {"F1", 122, 0}, {"F2", 120, 0}, {"F3", 99, 0}, {"F4", 118, 0}, {"F5", 96, 0}, {"F6", 97, 0},
    {"F7", 98, 0}, {"F8", 100, 0}, {"F9", 101, 0}, {"F10", 109, 0}, {"F11", 103, 0}, {"F12", 111, 0},
    {"Space", 49, 0}, {"Semicolon", 41, ';'}, {"Equal", 24, '='}, {"Comma", 43, ','}, {"Minus", 27, '-'},
    {"Period", 47, '.'}, {"Slash", 44, '/'}, {"Backquote", 50, '`'}, {"BracketLeft", 33, '['},
    {"Backslash", 42, '\\'}, {"BracketRight", 30, ']'}, {"Quote", 39, '\''},
};
} // namespace detail

// 只接受主键表中的物理键，防止小键盘字符冒充主键盘的绑定。
constexpr const char *ToggleKeyNameForEvent(unsigned short keyCode, unsigned short character)
{
    const detail::ToggleChordKeyCode *physical = nullptr;
    for (const auto &entry : detail::kToggleChordKeyCodes)
        if (entry.keyCode == keyCode) { physical = &entry; break; }
    if (!physical) return nullptr;
    if (!physical->character) return physical->name;
    if (character >= 'A' && character <= 'Z') character += 'a' - 'A';
    if (character > ' ' && character < 0x7f) {
        for (const auto &entry : detail::kToggleChordKeyCodes)
            if (entry.character == character) return entry.name;
        return nullptr;
    }
    return physical->name;
}

// chord 主键的规范名（client-core key_chord.rs 的 Display 输出）→ 本机认法。
// 认不出的名字给出 unbound；规范名区分大小写，"keye" 不是 "KeyE"。
constexpr ToggleChord ToggleChordForKeyName(const char *name, bool control, bool option,
                                            bool shift, bool command)
{
    if (!name || !*name) return UnboundToggleChord();
    ToggleChord chord = {true, 0, '\0', control, option, shift, command};
    if (name[0] == 'K' && name[1] == 'e' && name[2] == 'y' &&
        name[3] >= 'A' && name[3] <= 'Z' && name[4] == '\0') {
        chord.letter = static_cast<char>(name[3] - 'A' + 'a');
        return chord;
    }
    for (const detail::ToggleChordKeyCode &entry : detail::kToggleChordKeyCodes)
        if (ToggleChordNameEquals(name, entry.name)) {
            chord.keyCode = entry.keyCode;
            return chord;
        }
    return UnboundToggleChord();
}

// 录入和匹配共用规范主键；`character` 必须是去掉全部修饰键后的布局字符。
constexpr bool ToggleChordMatches(ToggleChord chord, unsigned short keyCode, unsigned short character,
                                  bool control, bool option, bool shift, bool command)
{
    if (!chord.bound || chord.control != control || chord.option != option ||
        chord.shift != shift || chord.command != command) return false;
    const auto event = ToggleChordForKeyName(ToggleKeyNameForEvent(keyCode, character), control, option, shift, command);
    return event.bound && chord.keyCode == event.keyCode && chord.letter == event.letter;
}

// 全半角的两条触发臂：绑定的 chord，以及 macOS 既有的 Option+Shift+H。后者不算在
// chord 里，由 `keybindings.toggle_fullwidth_option_shift_h` 单独门控。
enum class ToggleFullWidthArm { none, chord, optionShiftH };

constexpr ToggleFullWidthArm FullWidthToggleArmFor(ToggleChord chord, unsigned short keyCode,
    unsigned short letter, bool control, bool option, bool shift, bool command)
{
    if (ToggleChordMatches(chord, keyCode, letter, control, option, shift, command))
        return ToggleFullWidthArm::chord;
    const char *name = ToggleKeyNameForEvent(keyCode, letter);
    if (name && ToggleChordNameEquals(name, "KeyH") && option && shift && !control && !command)
        return ToggleFullWidthArm::optionShiftH;
    return ToggleFullWidthArm::none;
}

// 出厂绑定：client-core `TOGGLE_*_DEFAULT`（preferences.rs）的解码形，是共享文档
// 还没载入、或 keybindings 里没有这个键时的认法。改出厂值要两边一起改。
constexpr ToggleChord FactoryEnglishCandidatesToggleChord()
{
    return ToggleChordForKeyName("KeyE", true, false, true, false);
}
constexpr ToggleChord FactoryFullWidthToggleChord()
{
    return ToggleChordForKeyName("Space", true, false, true, false);
}
constexpr ToggleChord FactoryPunctuationToggleChord()
{
    return ToggleChordForKeyName("Period", true, false, false, false);
}

} // namespace msime::mac

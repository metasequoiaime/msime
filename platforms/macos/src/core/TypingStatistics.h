#pragma once

#include <cstdint>
#include <map>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

namespace msime::mac {

enum class TypingSource {
    Quanpin,
    NineKey,
    Shuangpin,
    Ziranma,
    Microsoft,
    Shoudao,
    Wubi,
    Japanese,
    Korean,
    Cantonese,
    Zhuyin,
    Vietnamese,
    Tibetan,
    Stroke,
    Handwriting,
    English,
    Local,
    Ai,
    Reply,
    Voice,
    Unknown,
};

constexpr std::string_view TypingSourceId(TypingSource source) {
    switch (source) {
    case TypingSource::Quanpin: return "quanpin";
    case TypingSource::NineKey: return "nineKey";
    case TypingSource::Shuangpin: return "shuangpin";
    case TypingSource::Ziranma: return "ziranma";
    case TypingSource::Microsoft: return "microsoft";
    case TypingSource::Shoudao: return "shoudao";
    case TypingSource::Wubi: return "wubi";
    case TypingSource::Japanese: return "japanese";
    case TypingSource::Korean: return "korean";
    case TypingSource::Cantonese: return "cantonese";
    case TypingSource::Zhuyin: return "zhuyin";
    case TypingSource::Vietnamese: return "vietnamese";
    case TypingSource::Tibetan: return "tibetan";
    case TypingSource::Stroke: return "stroke";
    case TypingSource::Handwriting: return "handwriting";
    case TypingSource::English: return "english";
    case TypingSource::Local: return "local";
    case TypingSource::Ai: return "ai";
    case TypingSource::Reply: return "reply";
    case TypingSource::Voice: return "voice";
    case TypingSource::Unknown: return "unknown";
    }
    return "unknown";
}

// 视图给出实际生效的引擎方案：0 全拼、1 双拼、2 五笔、3 日文、4 韩文、5 粤拼、6 注音、7 越南文、8 藏文、9 笔画。本地模式优先，与其他原生宿主一致。
constexpr TypingSource ResolveTypingSource(int scheme, bool nineKey,
                                            bool dedicatedEnglish,
                                            std::string_view localMode,
                                            std::string_view shuangpinProfile) {
    if (localMode == "temporary_japanese") return TypingSource::Japanese;
    if (!localMode.empty() && localMode != "none") return TypingSource::Local;
    if (dedicatedEnglish) return TypingSource::English;
    switch (scheme) {
    case 0: return nineKey ? TypingSource::NineKey : TypingSource::Quanpin;
    case 1:
        if (shuangpinProfile == "ziranma") return TypingSource::Ziranma;
        if (shuangpinProfile == "microsoft") return TypingSource::Microsoft;
        if (shuangpinProfile == "shoudao") return TypingSource::Shoudao;
        return TypingSource::Shuangpin;
    case 2: return TypingSource::Wubi;
    case 3: return TypingSource::Japanese;
    case 4: return TypingSource::Korean;
    case 5: return TypingSource::Cantonese;
    case 6: return TypingSource::Zhuyin;
    case 7: return TypingSource::Vietnamese;
    case 8: return TypingSource::Tibetan;
    case 9: return TypingSource::Stroke;
    default: return TypingSource::Unknown;
    }
}

// A key the input method hands back to the application is typed by the application itself, so it never reaches a commit path; this decides whether such a key counts, mirroring `ShouldCountPassthroughChar` in MSIME-Windows windows/src/Statistics/stats_passthrough.h. Like the source it is a key-time prediction, not an edit confirmation: a key the application treats as a shortcut or drops in a read-only field is still counted.
//
// Command and Control are the shortcut modifiers, the role Ctrl, Alt and Win play in the source. Option is allowed on purpose: on macOS it is the character layer, like AltGr on Windows, and produces characters such as the euro sign, the em dash and, on German or French layouts, @ [ { |. Control characters, DEL and lone surrogates are rejected as in the source, and so is the AppKit function-key range 0xF700-0xF8FF, which is where arrows, F-keys, Home/End and forward delete land in `NSEvent.characters`.
constexpr bool ShouldCountPassthroughCharacter(char16_t ch, bool control, bool command) {
    if (control || command) return false;
    if (ch < 0x20 || ch == 0x7F) return false;
    if (ch >= 0xD800 && ch <= 0xDFFF) return false;
    if (ch >= 0xF700 && ch <= 0xF8FF) return false;
    return true;
}

// The key heatmap id for a macOS virtual key code (`NSEvent.keyCode`, the Carbon kVK_* constants), or an empty view for a key the heatmap has no id for. kVK codes name physical positions, so a key counts in the same place whatever the active layout (AZERTY, Dvorak, Shuangpin keymaps) makes it type. The ids are the W3C `KeyboardEvent.code` names that crates/client-core typing_statistics.rs KEY_IDS accepts; one id outside that list rejects the whole batch, so keys without a whitelisted name (F13-F20, keypad =, the JIS keypad comma, volume keys) return empty rather than a guess. Help sits where Insert is on PC keyboards and is reported as Insert, the way browsers do. The JIS かな and 英数 keys are Lang1 and Lang2.
constexpr std::string_view KeyIdForVirtualKeyCode(unsigned short keyCode) {
    switch (keyCode) {
    case 0x00: return "KeyA";
    case 0x01: return "KeyS";
    case 0x02: return "KeyD";
    case 0x03: return "KeyF";
    case 0x04: return "KeyH";
    case 0x05: return "KeyG";
    case 0x06: return "KeyZ";
    case 0x07: return "KeyX";
    case 0x08: return "KeyC";
    case 0x09: return "KeyV";
    case 0x0A: return "IntlBackslash";
    case 0x0B: return "KeyB";
    case 0x0C: return "KeyQ";
    case 0x0D: return "KeyW";
    case 0x0E: return "KeyE";
    case 0x0F: return "KeyR";
    case 0x10: return "KeyY";
    case 0x11: return "KeyT";
    case 0x12: return "Digit1";
    case 0x13: return "Digit2";
    case 0x14: return "Digit3";
    case 0x15: return "Digit4";
    case 0x16: return "Digit6";
    case 0x17: return "Digit5";
    case 0x18: return "Equal";
    case 0x19: return "Digit9";
    case 0x1A: return "Digit7";
    case 0x1B: return "Minus";
    case 0x1C: return "Digit8";
    case 0x1D: return "Digit0";
    case 0x1E: return "BracketRight";
    case 0x1F: return "KeyO";
    case 0x20: return "KeyU";
    case 0x21: return "BracketLeft";
    case 0x22: return "KeyI";
    case 0x23: return "KeyP";
    case 0x24: return "Enter";
    case 0x25: return "KeyL";
    case 0x26: return "KeyJ";
    case 0x27: return "Quote";
    case 0x28: return "KeyK";
    case 0x29: return "Semicolon";
    case 0x2A: return "Backslash";
    case 0x2B: return "Comma";
    case 0x2C: return "Slash";
    case 0x2D: return "KeyN";
    case 0x2E: return "KeyM";
    case 0x2F: return "Period";
    case 0x30: return "Tab";
    case 0x31: return "Space";
    case 0x32: return "Backquote";
    case 0x33: return "Backspace";
    case 0x35: return "Escape";
    case 0x36: return "MetaRight";
    case 0x37: return "MetaLeft";
    case 0x38: return "ShiftLeft";
    case 0x39: return "CapsLock";
    case 0x3A: return "AltLeft";
    case 0x3B: return "ControlLeft";
    case 0x3C: return "ShiftRight";
    case 0x3D: return "AltRight";
    case 0x3E: return "ControlRight";
    case 0x3F: return "Fn";
    case 0x41: return "NumpadDecimal";
    case 0x43: return "NumpadMultiply";
    case 0x45: return "NumpadAdd";
    case 0x47: return "NumLock";
    case 0x4B: return "NumpadDivide";
    case 0x4C: return "NumpadEnter";
    case 0x4E: return "NumpadSubtract";
    case 0x52: return "Numpad0";
    case 0x53: return "Numpad1";
    case 0x54: return "Numpad2";
    case 0x55: return "Numpad3";
    case 0x56: return "Numpad4";
    case 0x57: return "Numpad5";
    case 0x58: return "Numpad6";
    case 0x59: return "Numpad7";
    case 0x5B: return "Numpad8";
    case 0x5C: return "Numpad9";
    case 0x5D: return "IntlYen";
    case 0x5E: return "IntlRo";
    case 0x60: return "F5";
    case 0x61: return "F6";
    case 0x62: return "F7";
    case 0x63: return "F3";
    case 0x64: return "F8";
    case 0x65: return "F9";
    case 0x66: return "Lang2";
    case 0x67: return "F11";
    case 0x68: return "Lang1";
    case 0x6D: return "F10";
    case 0x6E: return "ContextMenu";
    case 0x6F: return "F12";
    case 0x72: return "Insert";
    case 0x73: return "Home";
    case 0x74: return "PageUp";
    case 0x75: return "Delete";
    case 0x76: return "F4";
    case 0x77: return "End";
    case 0x78: return "F2";
    case 0x79: return "PageDown";
    case 0x7A: return "F1";
    case 0x7B: return "ArrowLeft";
    case 0x7C: return "ArrowRight";
    case 0x7D: return "ArrowDown";
    case 0x7E: return "ArrowUp";
    default: return {};
    }
}

// Whether a FlagsChanged event for `keyCode` is the key going down, from the event's raw `modifierFlags`. A modifier reports both edges as FlagsChanged, so only the edge that leaves its flag set is a press. The side-specific device bits (IOKit IOLLEvent.h NX_DEVICE*KEYMASK) decide when the event carries any for that modifier, so releasing the left Shift while the right one is held is not read as a press; an event with none, such as one a tool synthesised, falls back to the shared flag. Caps Lock reports only the press that toggles it, so every one counts. Returns false for a key code that is not a modifier.
constexpr bool IsModifierPress(unsigned short keyCode, uint64_t modifierFlags) {
    constexpr uint64_t shift = 1ull << 17, control = 1ull << 18,
        option = 1ull << 19, command = 1ull << 20, function = 1ull << 23;
    constexpr uint64_t leftControl = 0x1, leftShift = 0x2, rightShift = 0x4, leftCommand = 0x8,
        rightCommand = 0x10, leftOption = 0x20, rightOption = 0x40, rightControl = 0x2000;
    const auto sided = [modifierFlags](uint64_t family, uint64_t left, uint64_t right, bool isLeft) {
        if (modifierFlags & (left | right)) return (modifierFlags & (isLeft ? left : right)) != 0;
        return (modifierFlags & family) != 0;
    };
    switch (keyCode) {
    case 0x38: return sided(shift, leftShift, rightShift, true);
    case 0x3C: return sided(shift, leftShift, rightShift, false);
    case 0x3B: return sided(control, leftControl, rightControl, true);
    case 0x3E: return sided(control, leftControl, rightControl, false);
    case 0x3A: return sided(option, leftOption, rightOption, true);
    case 0x3D: return sided(option, leftOption, rightOption, false);
    case 0x37: return sided(command, leftCommand, rightCommand, true);
    case 0x36: return sided(command, leftCommand, rightCommand, false);
    case 0x39: return true;
    case 0x3F: return (modifierFlags & function) != 0;
    default: return false;
    }
}

// One batch of key heatmap counts: the presses of a single local day, by key id, as the `keys` of a `record_keys` request.
struct KeyPressFlush {
    std::string day;
    std::map<std::string, uint64_t> keys;
};

// A session's in-memory key heatmap counts, so the store is written once per batch and never per key. Counts belong to the day the key was pressed on: a press on a new day hands back the old day's counts first, so a batch that crosses midnight is never filed under the day it happens to be flushed on. The host also drains it on focus loss, on destruction and on a timer.
class KeyPressBatch {
public:
    // The press total at which a batch is due without waiting for the timer.
    static constexpr uint64_t kFlushThreshold = 256;

    // Counts one press of `keyId` on `day` and returns the batches now due, oldest first: the previous day's counts when `day` differs from theirs, then this day's once they reach kFlushThreshold.
    std::vector<KeyPressFlush> record(std::string_view day, std::string_view keyId) {
        std::vector<KeyPressFlush> due;
        due.reserve(2);
        if (presses_ != 0 && day != day_) due.push_back(*drain());
        if (presses_ == 0) day_.assign(day);
        ++keys_[std::string(keyId)];
        if (++presses_ >= kFlushThreshold) due.push_back(*drain());
        return due;
    }

    // Hands back everything collected and empties the batch, or nothing when it is already empty.
    std::optional<KeyPressFlush> drain() {
        if (presses_ == 0) return std::nullopt;
        KeyPressFlush flush{std::move(day_), std::move(keys_)};
        day_.clear();
        keys_.clear();
        presses_ = 0;
        return flush;
    }

    // Drops the counts without writing them, for when statistics are switched off.
    void clear() { drain(); }

    bool empty() const { return presses_ == 0; }

private:
    std::string day_;
    std::map<std::string, uint64_t> keys_;
    uint64_t presses_ = 0;
};

} // namespace msime::mac

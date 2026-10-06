#include "../../src/core/TypingStatistics.h"
#include "msime_client.h"

#include <Carbon/Carbon.h>
#include <IOKit/hidsystem/IOLLEvent.h>

#include <cassert>
#include <filesystem>
#include <set>
#include <string>

using msime::mac::IsModifierPress;
using msime::mac::KeyIdForVirtualKeyCode;
using msime::mac::KeyPressBatch;
using msime::mac::KeyPressFlush;

static std::string call(const std::filesystem::path &directory, const std::string &action) {
    const std::string request = "{\"directory\":\"" + directory.string() + "\",\"action\":" + action + "}";
    char *raw = msime_client_typing_statistics(reinterpret_cast<const uint8_t *>(request.data()), request.size());
    assert(raw);
    std::string result(raw);
    msime_client_string_free(raw);
    return result;
}

static void testKeyCodeTable() {
    // Physical positions by their Carbon names, so a slip in the hexadecimal table shows up here.
    static_assert(KeyIdForVirtualKeyCode(kVK_ANSI_A) == "KeyA");
    static_assert(KeyIdForVirtualKeyCode(kVK_ANSI_Q) == "KeyQ");
    static_assert(KeyIdForVirtualKeyCode(kVK_ANSI_Z) == "KeyZ");
    static_assert(KeyIdForVirtualKeyCode(kVK_ANSI_5) == "Digit5");
    static_assert(KeyIdForVirtualKeyCode(kVK_ANSI_6) == "Digit6");
    static_assert(KeyIdForVirtualKeyCode(kVK_ANSI_0) == "Digit0");
    static_assert(KeyIdForVirtualKeyCode(kVK_ANSI_Grave) == "Backquote");
    static_assert(KeyIdForVirtualKeyCode(kVK_ANSI_LeftBracket) == "BracketLeft");
    static_assert(KeyIdForVirtualKeyCode(kVK_ANSI_RightBracket) == "BracketRight");
    static_assert(KeyIdForVirtualKeyCode(kVK_ANSI_Quote) == "Quote");
    static_assert(KeyIdForVirtualKeyCode(kVK_ANSI_Slash) == "Slash");
    static_assert(KeyIdForVirtualKeyCode(kVK_ISO_Section) == "IntlBackslash");
    static_assert(KeyIdForVirtualKeyCode(kVK_JIS_Yen) == "IntlYen");
    static_assert(KeyIdForVirtualKeyCode(kVK_JIS_Underscore) == "IntlRo");
    static_assert(KeyIdForVirtualKeyCode(kVK_JIS_Kana) == "Lang1");
    static_assert(KeyIdForVirtualKeyCode(kVK_JIS_Eisu) == "Lang2");
    static_assert(KeyIdForVirtualKeyCode(kVK_Return) == "Enter");
    static_assert(KeyIdForVirtualKeyCode(kVK_Delete) == "Backspace");
    static_assert(KeyIdForVirtualKeyCode(kVK_ForwardDelete) == "Delete");
    static_assert(KeyIdForVirtualKeyCode(kVK_Help) == "Insert");
    static_assert(KeyIdForVirtualKeyCode(kVK_Shift) == "ShiftLeft");
    static_assert(KeyIdForVirtualKeyCode(kVK_RightShift) == "ShiftRight");
    static_assert(KeyIdForVirtualKeyCode(kVK_Control) == "ControlLeft");
    static_assert(KeyIdForVirtualKeyCode(kVK_RightControl) == "ControlRight");
    static_assert(KeyIdForVirtualKeyCode(kVK_Option) == "AltLeft");
    static_assert(KeyIdForVirtualKeyCode(kVK_RightOption) == "AltRight");
    static_assert(KeyIdForVirtualKeyCode(kVK_Command) == "MetaLeft");
    static_assert(KeyIdForVirtualKeyCode(kVK_RightCommand) == "MetaRight");
    static_assert(KeyIdForVirtualKeyCode(kVK_Function) == "Fn");
    static_assert(KeyIdForVirtualKeyCode(kVK_CapsLock) == "CapsLock");
    static_assert(KeyIdForVirtualKeyCode(kVK_ContextualMenu) == "ContextMenu");
    static_assert(KeyIdForVirtualKeyCode(kVK_F1) == "F1");
    static_assert(KeyIdForVirtualKeyCode(kVK_F4) == "F4");
    static_assert(KeyIdForVirtualKeyCode(kVK_F12) == "F12");
    static_assert(KeyIdForVirtualKeyCode(kVK_ANSI_KeypadClear) == "NumLock");
    static_assert(KeyIdForVirtualKeyCode(kVK_ANSI_KeypadEnter) == "NumpadEnter");
    static_assert(KeyIdForVirtualKeyCode(kVK_ANSI_Keypad8) == "Numpad8");
    static_assert(KeyIdForVirtualKeyCode(kVK_LeftArrow) == "ArrowLeft");
    static_assert(KeyIdForVirtualKeyCode(kVK_UpArrow) == "ArrowUp");
    // Keys with no whitelisted id are not counted rather than guessed.
    static_assert(KeyIdForVirtualKeyCode(kVK_F13).empty());
    static_assert(KeyIdForVirtualKeyCode(kVK_F20).empty());
    static_assert(KeyIdForVirtualKeyCode(kVK_ANSI_KeypadEquals).empty());
    static_assert(KeyIdForVirtualKeyCode(kVK_JIS_KeypadComma).empty());
    static_assert(KeyIdForVirtualKeyCode(kVK_VolumeUp).empty());
    static_assert(KeyIdForVirtualKeyCode(0xFFFF).empty());

    // No two codes share an id, so one key never collects another's presses.
    std::set<std::string_view> seen;
    for (unsigned code = 0; code <= 0xFFFF; ++code) {
        const std::string_view id = KeyIdForVirtualKeyCode(static_cast<unsigned short>(code));
        if (!id.empty()) assert(seen.insert(id).second);
    }
    assert(seen.size() == 107);
}

static void testModifierEdges() {
    // The raw NSEvent modifier flags are the IOKit event masks: the shared flags above the device bits for each side.
    const uint64_t shift = NX_SHIFTMASK, control = NX_CONTROLMASK, option = NX_ALTERNATEMASK,
        command = NX_COMMANDMASK, function = NX_SECONDARYFNMASK;
    // Press and release of a lone left Shift, as the hardware reports it.
    assert(IsModifierPress(kVK_Shift, shift | NX_DEVICELSHIFTKEYMASK));
    assert(!IsModifierPress(kVK_Shift, 0));
    // Right Shift held while the left one is released: the shared flag stays set, the left bit does not.
    assert(!IsModifierPress(kVK_Shift, shift | NX_DEVICERSHIFTKEYMASK));
    assert(IsModifierPress(kVK_RightShift, shift | NX_DEVICELSHIFTKEYMASK | NX_DEVICERSHIFTKEYMASK));
    assert(!IsModifierPress(kVK_RightShift, shift | NX_DEVICELSHIFTKEYMASK));
    assert(IsModifierPress(kVK_Control, control | NX_DEVICELCTLKEYMASK));
    assert(IsModifierPress(kVK_RightControl, control | NX_DEVICERCTLKEYMASK));
    assert(!IsModifierPress(kVK_RightControl, control | NX_DEVICELCTLKEYMASK));
    assert(IsModifierPress(kVK_Option, option | NX_DEVICELALTKEYMASK));
    assert(IsModifierPress(kVK_RightOption, option | NX_DEVICERALTKEYMASK));
    assert(IsModifierPress(kVK_Command, command | NX_DEVICELCMDKEYMASK));
    assert(IsModifierPress(kVK_RightCommand, command | NX_DEVICERCMDKEYMASK));
    assert(!IsModifierPress(kVK_RightCommand, 0));
    // A synthesised event without side bits falls back to the shared flag.
    assert(IsModifierPress(kVK_Option, option));
    assert(!IsModifierPress(kVK_Option, 0));
    assert(IsModifierPress(kVK_Function, function));
    assert(!IsModifierPress(kVK_Function, 0));
    // Caps Lock reports only the press that toggles it, on and off alike.
    assert(IsModifierPress(kVK_CapsLock, NX_ALPHASHIFTMASK));
    assert(IsModifierPress(kVK_CapsLock, 0));
    assert(!IsModifierPress(kVK_ANSI_A, shift | NX_DEVICELSHIFTKEYMASK));
}

static void testBatch() {
    KeyPressBatch batch;
    assert(batch.empty());
    assert(!batch.drain());
    auto due = batch.record("2026-09-30", "KeyA");
    assert(due.empty());
    assert(due.capacity() >= 2);
    assert(batch.record("2026-09-30", "KeyA").empty());
    assert(batch.record("2026-09-30", "Space").empty());
    // Past midnight: the presses before it are handed back under their own day before the new day collects.
    due = batch.record("2026-10-01", "KeyB");
    assert(due.size() == 1);
    assert(due[0].day == "2026-09-30");
    assert(due[0].keys.size() == 2 && due[0].keys.at("KeyA") == 2 && due[0].keys.at("Space") == 1);
    std::optional<KeyPressFlush> rest = batch.drain();
    assert(rest && rest->day == "2026-10-01" && rest->keys.size() == 1 && rest->keys.at("KeyB") == 1);
    assert(batch.empty() && !batch.drain());

    // A full batch is due on the press that fills it, and the next press starts a new one.
    for (uint64_t i = 1; i < KeyPressBatch::kFlushThreshold; ++i) assert(batch.record("2026-10-01", "KeyC").empty());
    due = batch.record("2026-10-01", "KeyD");
    assert(due.size() == 1 && due[0].day == "2026-10-01");
    assert(due[0].keys.at("KeyC") == KeyPressBatch::kFlushThreshold - 1 && due[0].keys.at("KeyD") == 1);
    assert(batch.empty());

    // Switching statistics off drops the counts without handing them back.
    assert(batch.record("2026-10-01", "KeyE").empty());
    batch.clear();
    assert(batch.empty() && !batch.drain());
}

static void testStoreAcceptsEveryId() {
    // Every id the table can produce is one the shared store accepts: a single unknown id would reject a whole batch.
    std::string keys;
    uint64_t total = 0;
    for (unsigned code = 0; code <= 0xFFFF; ++code) {
        const std::string_view id = KeyIdForVirtualKeyCode(static_cast<unsigned short>(code));
        if (id.empty()) continue;
        keys += (keys.empty() ? "" : ",") + std::string("\"") + std::string(id) + "\":2";
        total += 2;
    }
    const auto directory = std::filesystem::temp_directory_path() / "msime-macos-key-press-statistics-test";
    std::filesystem::remove_all(directory);
    const std::string action = "{\"operation\":\"record_keys\",\"day\":\"2026-09-30\",\"keys\":{" + keys + "}}";
    // Off by default, so nothing is written until the user turns statistics on.
    assert(call(directory, action).find("\"recorded\":0") != std::string::npos);
    assert(call(directory, "{\"operation\":\"set_enabled\",\"enabled\":true}").find("\"enabled\":true") != std::string::npos);
    assert(call(directory, action).find("\"recorded\":" + std::to_string(total)) != std::string::npos);
    const auto loaded = call(directory, "{\"operation\":\"load\"}");
    assert(loaded.find("\"dailyKeys\":{\"2026-09-30\":{") != std::string::npos);
    assert(loaded.find("\"KeyA\":2") != std::string::npos);
    std::filesystem::remove_all(directory);
}

int main() {
    testKeyCodeTable();
    testModifierEdges();
    testBatch();
    testStoreAcceptsEveryId();
    return 0;
}

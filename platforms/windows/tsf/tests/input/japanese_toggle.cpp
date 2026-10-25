// Ctrl+Shift+J 在中文方案和日语之间切换。组合键的判定和协商位在 shared/contracts/ipc_negotiation.h，这里逐条钉住；TSF 里的接线要整个 TIP 才编得起来，所以和 caps_lock_tap 一样检查源码。从仓库根目录运行，或传入 TSF 源码目录。
#include "../../../../../shared/contracts/ipc_negotiation.h"

#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <sstream>
#include <string>

namespace {
int failures = 0;

void check(bool condition, const char *what) {
    if (!condition) {
        std::fprintf(stderr, "FAIL: %s\n", what);
        ++failures;
    }
}

std::string read(const std::string &path) {
    std::ifstream file(path);
    if (!file) {
        std::fprintf(stderr, "FAIL: cannot open %s\n", path.c_str());
        std::exit(EXIT_FAILURE);
    }
    std::ostringstream text;
    text << file.rdbuf();
    return text.str();
}

void expect(const std::string &text, const char *needle, bool present, const char *what) {
    check((text.find(needle) != std::string::npos) == present, what);
}

size_t count(const std::string &text, const std::string &needle) {
    size_t found = 0;
    for (auto at = text.find(needle); at != std::string::npos; at = text.find(needle, at + needle.size()))
        ++found;
    return found;
}

void policy() {
    check(FanyImeProtocol::IsJapaneseToggleShortcut('J', 3), "Ctrl+Shift+J is the shortcut");
    check(!FanyImeProtocol::IsJapaneseToggleShortcut('J', 1) && !FanyImeProtocol::IsJapaneseToggleShortcut('J', 2),
          "Ctrl+J and Shift+J are not");
    check(!FanyImeProtocol::IsJapaneseToggleShortcut('J', 7), "Ctrl+Alt+Shift+J is not");
    check(!FanyImeProtocol::IsJapaneseToggleShortcut('F', 3), "Ctrl+Shift+F stays the character-set shortcut");
    check((FanyImeProtocol::Capabilities & FanyImeProtocol::JapaneseToggleShortcut) == 0 &&
              (FanyImeProtocol::RequiredCapabilities & FanyImeProtocol::JapaneseToggleShortcut) == 0,
          "the capability is optional");
}

void wiring(const std::string &root) {
    const std::string sink = read(root + "/Key/KeyEventSink.cpp");
    // 分类：和简繁一样排除 Win 键，并且要协商到这一位、本版本提供两头、Engine 不在英文模式、开关打开。
    const auto classifier = sink.find("bool IsJapaneseInputModeToggle(UINT code, UINT modifiers)");
    check(classifier != std::string::npos, "the TIP has its own classifier");
    const auto body = sink.substr(classifier, sink.find("\n}\n", classifier) - classifier);
    expect(body, "FanyImeProtocol::IsJapaneseToggleShortcut(code, modifiers)", true, "the chord comes from the shared contract");
    expect(body, "(GetAsyncKeyState(VK_LWIN) & 0x8000) == 0", true, "the left Win key excludes it");
    expect(body, "(GetAsyncKeyState(VK_RWIN) & 0x8000) == 0", true, "the right Win key excludes it");
    expect(body, "SupportsJapaneseToggleShortcut()", true, "an old Server that did not negotiate it gets the key as before");
    expect(body, "msime::windows::scheme::offers_japanese_toggle()", true, "an edition without both ends hands the key on");
    expect(body, "!Global::DedicatedEnglish.active(GetTickCount64())", true, "the Engine's English mode hands the key on");
    expect(body, ".japanese_toggle_ctrl_shift_j", true, "the switch gates it");

    // 中文分支和延迟分类都把它分成 FUNCTION_TOGGLE_JAPANESE。
    check(count(sink, "IsJapaneseInputModeToggle(") == 3, "both the immediate and the deferred classification use it");
    check(count(sink, "Function = FUNCTION_TOGGLE_JAPANESE;") == 2, "both classifications own the key as the new function");

    // 简繁快捷键的四处特例它都要有。
    expect(sink,
           "if ((keyState.Function == FUNCTION_TOGGLE_CHARACTER_SET || keyState.Function == FUNCTION_TOGGLE_JAPANESE) &&\n"
           "        (lParam & 0x40000000) != 0)",
           true, "auto-repeat does not queue another toggle");
    expect(sink,
           "if (offlineState.Function == FUNCTION_TOGGLE_CHARACTER_SET || offlineState.Function == FUNCTION_TOGGLE_JAPANESE)",
           true, "with the Server unavailable the key completes without a document edit");
    expect(sink, "if (KeystrokeState.Function == FUNCTION_TOGGLE_JAPANESE && !SupportsJapaneseToggleShortcut())", true,
           "a Server replaced by an older one never gets it as ordinary input");
    expect(sink,
           "if (KeystrokeState.Function == FUNCTION_TOGGLE_CHARACTER_SET || KeystrokeState.Function == FUNCTION_TOGGLE_JAPANESE)",
           true, "TIP 不读主回复帧，后续键由偏好修订屏障放行");

    const std::string ipc = read(root + "/IPC/Ipc.cpp");
    expect(ipc, "FanyImeProtocol::JapaneseToggleShortcut);", true, "the TIP advertises the capability in its hello");
    expect(ipc, "(negotiatedServerCapabilities & FanyImeProtocol::JapaneseToggleShortcut) != 0", true,
           "support follows the negotiated capabilities");
    expect(read(root + "/IPC/Ipc.h"), "bool SupportsJapaneseToggleShortcut();", true, "the query is declared");

    // 枚举只能追加，序号钉在 38。
    const std::string base = read(root + "/IME/MetasequoiaIMEBaseStructure.h");
    expect(base, "FUNCTION_KOREAN_HANJA_KEY,\n", true, "the new function follows the last existing one");
    expect(base, "FUNCTION_TOGGLE_JAPANESE == 38", true, "its ordinal is pinned");

    const std::string utils = read(root + "/Utils/FanyUtils.cpp");
    expect(utils, "keybindings.value(\"toggle_japanese_ctrl_shift_j\", false)", true,
           "the switch is read from the shared keybindings and defaults off");
    expect(read(root + "/Utils/FanyUtils.h"), "bool japanese_toggle_ctrl_shift_j = false;", true,
           "an unreadable store leaves it off");

    // Server 侧：生产环境宣告这一位，并把按键交给工作线程写偏好。
    const std::string server = read(root + "/../src/entrypoints/server_main.cpp");
    expect(server, "FanyImeProtocol::JapaneseToggleShortcut)", true, "the production Server advertises the capability");
    expect(server, "japanese_toggles.submit(JapaneseToggleRequest{completion})", true, "the key hands the write to a worker");
    expect(server, "msime::windows::scheme::japanese_toggle_write(", true,
           "the worker writes what windows-input-scheme-traits tests");
    expect(server, "japanese.request_stop();", true, "the worker stops with the other click workers");
}
} // namespace

int main(int argc, char **argv) {
    policy();
    wiring(argc > 1 ? argv[1] : "platforms/windows/tsf");
    if (failures != 0) return EXIT_FAILURE;
    std::puts("japanese toggle: ok");
    return EXIT_SUCCESS;
}

// Vim 模式的按键判定（common/VimMode.h）由 windows-vim-mode 验证；它接进 TIP 的两个按键入口需要整个 TIP 才能构建，所以这里核对源码里的接线：OnTestKeyDown 和 OnKeyDown 都在 Esc 的去向定下之后判定，OnKeyDown 在派发前记下有没有在飞的按键，名单从共享偏好快照读。从仓库根目录运行，或传入 TSF 源码目录。
#include <algorithm>
#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <sstream>
#include <string>

namespace {
int failures = 0;

std::string read(const std::string &path) {
    std::ifstream file(path, std::ios::binary);
    if (!file) {
        std::fprintf(stderr, "FAIL: cannot open %s\n", path.c_str());
        std::exit(EXIT_FAILURE);
    }
    std::ostringstream text;
    text << file.rdbuf();
    // Git for Windows 默认 autocrlf 检出成 CRLF，读进来时去掉回车，换行一律按 LF 比较。
    std::string content = text.str();
    content.erase(std::remove(content.begin(), content.end(), '\r'), content.end());
    return content;
}

void expect(const std::string &text, const char *needle, bool present, const char *what) {
    if ((text.find(needle) != std::string::npos) != present) {
        std::fprintf(stderr, "FAIL: %s\n", what);
        ++failures;
    }
}

// `first` 出现在 `second` 之前。
void expect_before(const std::string &text, const char *first, const char *second, const char *what) {
    const auto a = text.find(first);
    const auto b = text.find(second);
    if (a == std::string::npos || b == std::string::npos || a > b) {
        std::fprintf(stderr, "FAIL: %s\n", what);
        ++failures;
    }
}

// 从 `from` 到它之后第一个 `to` 之间的文字；任一个找不到时为空。
std::string between(const std::string &text, const char *from, const char *to) {
    const auto begin = text.find(from);
    if (begin == std::string::npos) {
        return {};
    }
    const auto end = text.find(to, begin);
    return end == std::string::npos ? std::string{} : text.substr(begin, end - begin);
}
} // namespace

int main(int argc, char **argv) {
    const std::string root = argc > 1 ? argv[1] : "platforms/windows/tsf";

    const std::string sink = read(root + "/Key/KeyEventSink.cpp");
    const std::string apply = between(sink, "void CMetasequoiaIME::_ApplyVimModeEscape(", "\n}\n");
    expect(apply, "msime::windows::VimModeEscapeSwitchesToEnglish(key)", true, "the TIP asks the shared policy");
    expect(apply, "FanyUtils::ReadConfiguredVimModeApp(Global::current_process_name)", true,
           "the list is matched against the host process basename");
    expect(apply, "key.eaten = isEaten != FALSE;", true, "an Esc the TIP eats never switches");
    expect(apply, "_IsCompositionActiveForKeyGuard()", true, "an open composition blocks the switch");
    expect(apply, "VK_LWIN", true, "Win+Esc keeps its old behavior");
    expect(apply, "SetIMEMode(_pThreadMgr, _tfClientId, FALSE)", true, "the switch goes through the open/close compartment");

    // OnTestKeyDown：排队分支先返回，立即分类得出去向后才判定。
    const std::string test = between(sink, "STDAPI CMetasequoiaIME::OnTestKeyDown(", "\n}\n");
    expect_before(test, "*pIsEaten = _IsKeyEaten(pContext, (UINT)wParam, &code, &wch, &KeystrokeState);",
                  "_ApplyVimModeEscape(wParam, lParam, *pIsEaten, false);", "OnTestKeyDown decides after classifying the key");
    expect_before(test, "if (_HasDeferredKeyBarrier())\n    {\n        _KEYSTROKE_STATE deferredState",
                  "_ApplyVimModeEscape(wParam, lParam, *pIsEaten, false);", "a queued Esc returns before the switch");

    // OnKeyDown：派发前记下在飞的按键，派发后按去向判定。
    const std::string down = between(sink, "STDAPI CMetasequoiaIME::OnKeyDown(", "\n}\n");
    expect_before(down, "const bool deferredKeysPending = _HasDeferredKeyBarrier();", "(void)_DispatchKeyDown(pContext, wParam, lParam, pIsEaten",
                  "OnKeyDown samples pending keys before dispatching");
    expect_before(down, "(void)_DispatchKeyDown(pContext, wParam, lParam, pIsEaten",
                  "_ApplyVimModeEscape(wParam, lParam, *pIsEaten, deferredKeysPending);", "OnKeyDown decides after dispatching");

    const std::string utils = read(root + "/Utils/FanyUtils.cpp");
    const std::string reader = between(utils, "BOOL ReadConfiguredVimModeApp(", "\n}\n");
    expect(reader, "ReadSharedPreferences()", true, "the list comes from the shared preferences snapshot");
    expect(reader, "msime::windows::ReadVimModeApps(*preferences)", true, "the snapshot is read with the shared reader");
    return failures == 0 ? EXIT_SUCCESS : EXIT_FAILURE;
}

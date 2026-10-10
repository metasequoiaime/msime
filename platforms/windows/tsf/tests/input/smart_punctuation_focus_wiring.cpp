// The focus sinks need the whole TSF to build, so this checks their source: like the reference, losing thread focus, a genuine focus-session handover and a top-context change must each clear the smart-punctuation action, so a queued repeated-punctuation rewrite cannot backspace into whatever gains focus next. Run from the repository root, or pass the TSF source directory.
#include <algorithm>
#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <sstream>
#include <string>

namespace {
int failures = 0;
const char *const clear_call = "_ClearSmartPunctuationAction();";

std::string read(const std::string &path) {
    std::ifstream file(path, std::ios::binary);
    if (!file) {
        std::fprintf(stderr, "FAIL: cannot open %s\n", path.c_str());
        std::exit(EXIT_FAILURE);
    }
    std::ostringstream text;
    text << file.rdbuf();
    // 下面有跨行的锚点：Git for Windows 默认 autocrlf 检出成 CRLF，读进来时去掉回车，换行一律按 LF 比较。
    std::string content = text.str();
    content.erase(std::remove(content.begin(), content.end(), '\r'), content.end());
    return content;
}

// The text from `from` up to the next `to` after it, or empty when either is missing.
std::string between(const std::string &text, const char *from, const char *to) {
    const auto begin = text.find(from);
    if (begin == std::string::npos) {
        return {};
    }
    const auto end = text.find(to, begin);
    return end == std::string::npos ? std::string{} : text.substr(begin, end - begin);
}

void expect(const std::string &text, const char *needle, const char *what) {
    if (text.find(needle) == std::string::npos) {
        std::fprintf(stderr, "FAIL: %s\n", what);
        ++failures;
    }
}
} // namespace

int main(int argc, char **argv) {
    const std::string root = argc > 1 ? argv[1] : "platforms/windows/tsf";

    const std::string focus = read(root + "/Thread/ThreadFocusSink.cpp");
    const std::string kill = between(focus, "STDAPI CMetasequoiaIME::OnKillThreadFocus()", "\n}\n");
    expect(between(kill, "return S_OK;", "_CaptureWindowsTextInputHostFocusLoss"), clear_call,
           "losing thread focus clears the smart-punctuation action after the owner guard");

    const std::string manager = read(root + "/Thread/ThreadMgrEventSink.cpp");
    expect(between(manager, "if (pDocMgrFocus && (!Global::g_connected || windowsTextInputHostTransition))",
                   "Global::g_connected = true;"),
           clear_call, "a genuine focus-session handover clears the smart-punctuation action");
    expect(between(manager, "_ClearDeferredKeyDowns();", "MarkNamedpipeSessionDirtyForOwner(this);"), clear_call,
           "a top-context change clears the smart-punctuation action");

    const std::string composition = read(root + "/Composition/Composition.cpp");
    const std::string helper = between(composition, "void CMetasequoiaIME::_ClearSmartPunctuationAction()", "\n}\n");
    expect(helper, "_ResetSmartPunctuationHistory();", "clearing the action forgets the armed space and revert");
    expect(helper, "_pendingSmartPunctuationReplacement = 0;", "clearing the action drops the queued rewrite");
    return failures == 0 ? EXIT_SUCCESS : EXIT_FAILURE;
}

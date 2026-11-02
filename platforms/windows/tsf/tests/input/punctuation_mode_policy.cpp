// 激活和切换中英文时 TIP 把标点 compartment 定成什么（#10646）：规则在 PunctuationModePolicy.h 里单独验证；激活、中英文切换、OPENCLOSE 回调和延迟投影四处要整个 TIP 才能构建，所以这里核对源码里的接线，确认它们都走「跟随时中文模式用已存起始标点」的那条规则，而 Ctrl+. 这类显式请求仍照请求。从仓库根目录运行，或传入 TSF 源码目录。
#include "../../Global/PunctuationModePolicy.h"
#include <algorithm>
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
    check((text.find(needle) != std::string::npos) == present, what);
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
    using namespace Global;

    // 跟随：中文模式回到已存的起始标点，英文模式总是英文标点。
    check(PunctuationOpenForImeMode(PunctuationLock::Follow, true, true), "a Chinese start gives Chinese punctuation in Chinese mode");
    check(!PunctuationOpenForImeMode(PunctuationLock::Follow, true, false), "an English start gives English punctuation in Chinese mode");
    check(!PunctuationOpenForImeMode(PunctuationLock::Follow, false, true), "English mode gives English punctuation under a Chinese start");
    check(!PunctuationOpenForImeMode(PunctuationLock::Follow, false, false), "English mode gives English punctuation under an English start");
    // 锁定压过起始标点和中英文模式。
    check(PunctuationOpenForImeMode(PunctuationLock::AlwaysChinese, false, false), "the Chinese lock wins in English mode with an English start");
    check(!PunctuationOpenForImeMode(PunctuationLock::AlwaysEnglish, true, true), "the English lock wins in Chinese mode with a Chinese start");
    // 显式请求在跟随时照请求，起始标点不参与：Ctrl+. 在英文起始下仍能打开中文标点。
    check(PunctuationOpenForRequest(PunctuationLock::Follow, true), "a request for Chinese punctuation is honoured while following");
    check(!PunctuationOpenForRequest(PunctuationLock::Follow, false), "a request for English punctuation is honoured while following");
    check(PunctuationOpenForRequest(PunctuationLock::AlwaysChinese, false), "the Chinese lock wins over a request");
    check(!PunctuationOpenForRequest(PunctuationLock::AlwaysEnglish, true), "the English lock wins over a request");

    const std::string root = argc > 1 ? argv[1] : "platforms/windows/tsf";
    const std::string engine = read(root + "/Composition/CompositionProcessorEngine.cpp");
    const std::string sink = read(root + "/Key/KeyEventSink.cpp");
    const std::string utils = read(root + "/Utils/FanyUtils.cpp");

    // 中英文切换（Shift、ToggleIMEMode、组字后补切）都经 SyncPunctuationWithImeMode，它先刷新快照再按模式规则定标点。
    const std::string sync = between(engine, "void CCompositionProcessorEngine::SyncPunctuationWithImeMode", "\n}\n");
    expect(sync, "FanyUtils::RefreshPunctuationLockFromConfig();", true, "the mode sync refreshes the saved start before resolving");
    expect(sync, "Global::ResolveImeModePunctuationOpen(isOpen)", true, "the mode sync resolves through the saved start");
    // 激活时与切换走同一条路。
    const std::string activation = between(engine, "void CCompositionProcessorEngine::InitializeMetasequoiaIMECompartment", "\n}\n");
    expect(activation, "SyncPunctuationWithImeMode(pThreadMgr, tfClientId, openChinese);", true, "activation starts from the saved start");
    expect(activation, "SetPunctuationMode(pThreadMgr, tfClientId, openChinese);", false, "activation no longer opens Chinese punctuation with the Chinese mode");
    // 语言栏、应用或 Server 改 OPENCLOSE 时的回调。
    const std::string openClose = between(engine, "else if (IsEqualGUID(guidCompartment, GUID_COMPARTMENT_KEYBOARD_OPENCLOSE))", "fakeThis->KeyboardOpenCompartmentUpdated");
    expect(openClose, "Global::ResolveImeModePunctuationOpen(isOpen)", true, "an OPENCLOSE change resolves through the saved start");
    // 延迟按键投影要和真正落下的 compartment 一致。
    const std::string projection = between(sink, "case CCompositionProcessorEngine::PreservedKeyAction::ToggleImeMode:", "break;");
    expect(projection, "Global::ResolveImeModePunctuationOpen(_deferredProjectedImeOpen)", true, "the deferred projection of a mode toggle uses the saved start");
    const std::string toggle = between(sink, "case CCompositionProcessorEngine::PreservedKeyAction::TogglePunctuationMode:", "break;");
    expect(toggle, "Global::ResolvePunctuationOpen(!_deferredProjectedPunctuationOpen)", true, "the deferred projection of Ctrl+. still follows the request");
    // 快照刷新同时带上起始标点。
    const std::string refresh = between(utils, "void RefreshPunctuationLockFromConfig()", "\n}\n");
    expect(refresh, "Global::SavedChinesePunctuation.store(", true, "the snapshot refresh stores the saved start");

    if (failures == 0)
        std::puts("punctuation mode policy: activation and mode switches start from the saved punctuation");
    return failures == 0 ? 0 : 1;
}

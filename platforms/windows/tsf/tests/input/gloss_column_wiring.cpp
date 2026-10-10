// 释义列快捷键、Ctrl+Enter 和它的释义页横跨 TIP 和 Server：TIP 在候选列表开着时吃掉这些键并带上 CandidateActive，Server 回 CommitExactText 时 TIP 原样上屏。普通组字的候选列表是 CANDIDATE_INCREMENTAL，以前只认 CANDIDATE_ORIGINAL，这些键在普通组字里从没到达 Server 的释义路径；宿主会话拥有组字时数字选词也不读 Server 的回复。这几处都要整个 TIP 才能构建，所以这里核对源码里的接线；哪些键带这一位由 candidate_active_key_policy 验证。从仓库根目录运行，或传入 TSF 源码目录。
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
    // 普通路径：Alt/Ctrl+数字在任何候选列表开着时都吃掉，和 Ctrl+Enter 一样。
    expect(sink, "_candidateMode != CANDIDATE_NONE &&\n            !_serverUnavailableFallbackActive && !Global::IsUiLessMode() &&\n            IsGlossColumnShortcut(*pCodeOut, shortcutModifiers)",
           true, "Alt/Ctrl+digit is taken while incremental candidates are open");
    expect(sink, "_candidateMode == CANDIDATE_ORIGINAL &&\n            !_serverUnavailableFallbackActive", false,
           "the gloss shortcut no longer waits for the wildcard candidate list");
    // 排队路径：看投影里的组字，不只看 CANDIDATE_ORIGINAL。
    expect(sink, "_deferredKeyProjectionValid ? (_deferredProjectedInputLength > 0 || _deferredProjectedCandidateActive)\n                                    : (_candidateMode != CANDIDATE_NONE);",
           true, "a queued gloss shortcut sees the projected composition");
    expect_before(sink, "projectedCandidateListOpen &&", "IsGlossColumnShortcut(*classifiedCode, capturedModifiers)",
                  "a queued Alt/Ctrl+digit is gated on the projected candidate list");
    expect_before(sink, "projectedCandidateListOpen &&\n        IsTranslationCommitShortcut(*classifiedCode", "WriteDataToNamedPipe(Global::Keycode",
                  "a queued Ctrl+Enter is gated on the projected candidate list");
    // 发给 Server 的包：增量候选里释义和释义页认的键带 CandidateActive。
    expect_before(sink, "Global::CandidateKeyReportsActiveList(code, Global::ModifiersDown)", "WriteDataToNamedPipe(Global::Keycode",
                  "gloss and sense-page keys carry CandidateActive with incremental candidates");
    expect(sink, "(candidateActive ? msime::windows::PipeMetadata::CandidateActive : 0u)", true,
           "the computed bit is what goes on the wire");

    const std::string presenter = read(root + "/UI/CandidateListUIPresenter.cpp");
    // 宿主会话拥有组字时数字在 _HandleCandidateWorker 里选词：先读 Server 的回复，CommitExactText 时上屏释义，不向宿主会话选词。
    const std::string worker = between(presenter, "HRESULT CMetasequoiaIME::_HandleCandidateWorker(", "\n}\n");
    expect_before(worker, "TryReadDataFromServerPipeWithTimeout(requestId, /*abortTransportOnTimeout=*/false)",
                  "host->select(displayed._EngineGeneration", "the host digit path reads the Server's reply before selecting");
    expect_before(worker, "Global::DataFromServerMsgType::CommitExactText", "host->select(displayed._EngineGeneration",
                  "an exact-text reply is committed instead of the host candidate");
    expect(worker, "return _CommitServerExactText(ec, pContext, exactText);", true, "the host digit path commits the gloss");
    // 空格、回车和非宿主会话的数字走 _HandleCandidateFinalize，同一个上屏函数。
    const std::string finalize = between(presenter, "HRESULT CMetasequoiaIME::_HandleCandidateFinalize(", "\n}\n");
    expect(finalize, "return _CommitServerExactText(ec, pContext, serverCandidateString);", true, "the finalize path commits the gloss");
    // Server 已经取消了组字，宿主会话也要丢掉，否则下一个字母接着旧拼音组字。
    const std::string commit = between(presenter, "HRESULT CMetasequoiaIME::_CommitServerExactText(", "\n}\n");
    expect_before(commit, "_CancelHostComposition();", "_HandleCompleteCommitFirst(ec, pContext);",
                  "the host session drops its composition with the Server's");
    return failures == 0 ? EXIT_SUCCESS : EXIT_FAILURE;
}

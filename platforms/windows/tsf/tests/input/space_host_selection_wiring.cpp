// 宿主会话拥有组字时，空格和数字一样在 TIP 自己的宿主会话里选中高亮候选，点选上屏也丢掉宿主会话的组字。以前空格只写 Server 回的文字，宿主会话从不前进，空格上屏过的拼音在里面越积越长，Shift 切英文时整串上屏（打 nihao 空格、shijie 空格再按 Shift 得到「你好世界nihaoshijie」），分段选词后的组字也显示成整段拼音。这几处都要整个 TIP 才能构建，所以这里核对源码里的接线。从仓库根目录运行，或传入 TSF 源码目录。
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
    const std::string presenter = read(root + "/UI/CandidateListUIPresenter.cpp");
    const std::string finalize = between(presenter, "HRESULT CMetasequoiaIME::_HandleCandidateFinalize(", "\n}\n");
    if (finalize.empty()) {
        std::fprintf(stderr, "FAIL: _HandleCandidateFinalize not found\n");
        return EXIT_FAILURE;
    }

    // 空格上屏（Normal）：先在宿主会话里选，选不了才写 Server 的文字，写完也要丢掉宿主会话的组字。
    const std::string normal = between(finalize, "serverMsgType == Global::DataFromServerMsgType::Normal", "DataFromServerMsgType::CommitExactText");
    expect_before(normal, "_SelectHighlightedHostCandidate(ec, pContext)", "_InsertTextToComposition(ec, pContext, &candidateString)",
                  "Space selects the highlighted candidate in the host session before writing the Server's text");
    expect_before(normal, "_InsertTextToComposition(ec, pContext, &candidateString)", "_CancelHostComposition()",
                  "a Space commit that falls back to the Server's text drops the host session's composition");

    // 空格只选中一段（NeedToCreateWord）：宿主会话同样只选这一段，组字按宿主会话剩下的原文继续。
    const std::string partial = between(finalize, "serverMsgType == Global::DataFromServerMsgType::NeedToCreateWord", "NoPresenter:");
    expect_before(partial, "_SelectHighlightedHostCandidate(ec, pContext)", "AddVirtualKey(remainingRawInput[i])",
                  "a partial Space selection advances the host session before the legacy key buffer is rebuilt");

    // 点选上屏：任何宿主会话拥有的组字都要丢掉，不只韩文这类内联方案。
    const std::string clicked = between(finalize, "if (!pendingCommitCandidate.empty())", "UINT serverMsgType");
    expect_before(clicked, "if (hostOwnsComposition)\n            {\n                (void)_CancelHostComposition();", "_HandleCompleteCommitFirst(ec, pContext);",
                  "a clicked commit drops the host session's composition for every scheme");
    expect(clicked, "AlwaysInlinePreedit", false, "dropping the host composition after a click is not limited to inline schemes");

    // 数字键和空格用同一个宿主选词函数。
    const std::string worker = between(presenter, "HRESULT CMetasequoiaIME::_HandleCandidateWorker(", "\n}\n");
    expect(worker, "return _SelectHighlightedHostCandidate(ec, pContext);", true, "digits select through the shared host selection");
    const std::string helper = between(presenter, "HRESULT CMetasequoiaIME::_SelectHighlightedHostCandidate(", "\n}\n");
    expect_before(helper, "displayed.MatchesEngineView(view.view.session, view.view.generation)", "host->select(displayed._EngineGeneration",
                  "the host selection refuses a candidate window that no longer matches the host session");
    return failures == 0 ? EXIT_SUCCESS : EXIT_FAILURE;
}

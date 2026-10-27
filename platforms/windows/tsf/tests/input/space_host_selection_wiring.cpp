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
    expect_before(normal, "_SelectHighlightedHostCandidate(ec, pContext, serverMsgType, &serverCandidateString)", "_InsertTextToComposition(ec, pContext, &candidateString)",
                  "Space selects the highlighted candidate in the host session before writing the Server's text");
    expect_before(normal, "_InsertTextToComposition(ec, pContext, &candidateString)", "_CancelHostComposition()",
                  "a Space commit that falls back to the Server's text drops the host session's composition");

    // 空格只选中一段（NeedToCreateWord）：宿主会话同样只选这一段，组字按宿主会话剩下的原文继续。
    const std::string partial = between(finalize, "serverMsgType == Global::DataFromServerMsgType::NeedToCreateWord", "NoPresenter:");
    expect_before(partial, "_SelectHighlightedHostCandidate(ec, pContext, serverMsgType, &serverCandidateString)", "AddVirtualKey(remainingRawInput[i])",
                  "a partial Space selection advances the host session before the legacy key buffer is rebuilt");

    // 点选上屏：任何宿主会话拥有的组字都要丢掉，不只韩文这类内联方案。
    const std::string clicked = between(finalize, "if (!pendingCommitCandidate.empty())", "UINT serverMsgType");
    expect_before(clicked, "if (hostOwnsComposition)\n            {\n                (void)_CancelHostComposition();", "_HandleCompleteCommitFirst(ec, pContext);",
                  "a clicked commit drops the host session's composition for every scheme");
    expect(clicked, "AlwaysInlinePreedit", false, "dropping the host composition after a click is not limited to inline schemes");

    // 数字键和空格用同一个宿主选词函数。
    const std::string worker = between(presenter, "HRESULT CMetasequoiaIME::_HandleCandidateWorker(", "\n}\n");
    expect(worker, "_SelectHighlightedHostCandidate(ec, pContext, reply->msg_type, &serverReply)", true, "digits select through the shared host selection");
    // 数字只选一段（NeedToCreateWord）或什么都没选时也照 Server 的回复，不再退回宿主会话这一页上按键位置的那一项：滚轮只翻 Server 那一页，同一个位置上是另一个词。
    expect(worker, "msime::tsf::ServerReplyDecidesSelection(reply->msg_type)", true,
           "every selection reply, not only a whole commit, decides the digit's word");
    expect(worker, "if (reply->msg_type == Global::DataFromServerMsgType::Normal)\n                serverChoice", false,
           "a partial selection no longer falls back to the host page slot");
    expect_before(worker, "ServerReplyDecidesSelection(reply->msg_type)", "if (!localSlotSelected) return S_FALSE;",
                  "the local slot is used only when no Server reply decided");
    // 宿主会话这一页比 Server 那一页短时，数字照样去读回复，不在宿主会话这一页上越界就返回。
    const std::string byNumber = between(presenter, "HRESULT CMetasequoiaIME::_HandleCandidateSelectByNumber(", "\n}\n");
    expect(byNumber, "_HandleCandidateWorker(ec, pContext, requestId, prefetchedText, /*localSlotSelected=*/false)", true,
           "a digit past the host page still follows the Server's reply");
    const std::string helper = between(presenter, "HRESULT CMetasequoiaIME::_SelectHighlightedHostCandidate(", "\n}\n");
    expect_before(helper, "displayed.MatchesEngineView(view.view.session, view.view.generation)", "SelectHostCandidateByText(*host, view.view, target",
                  "the host selection refuses a candidate window that no longer matches the host session");
    // 候选身份按文字匹配，不套用显示项的下标：候选窗归 Server，宿主会话是另一个引擎会话，同一个下标会指向不同的词。
    expect(helper, "displayed._EngineIndex", false,
           "the host selection does not reuse the displayed candidate's index");
    // 上屏哪个词以 Server 的回复为准（候选窗归它画），再按文字到宿主会话里选中。
    expect(helper, "serverReply ? *serverReply : displayed._ItemString.ToWString()", true,
           "the host selection prefers the Server's reply as the text to commit");
    // 回复里这次选的那段由 ServerSelectedPiece 取出：减掉已暂存的词段，只选一段的载荷取第二段。以前空格把整条带制表符的载荷当成候选文字。
    expect_before(helper, "msime::tsf::ServerSelectedPiece({serverMsgType, text}, view.view.phrase_prefix)",
                  "SelectHostCandidateByText(*host, view.view, target",
                  "the host selection extracts the selected piece before locating it");
    expect(presenter, "_SelectHighlightedHostCandidate(ec, pContext, serverMsgType, &serverCandidateString)", true,
           "Space passes the Server's reply to the host selection");
    // 宿主会话里找不到这个词时，数字和空格一样写 Server 已经上屏的文字，不让文档少一个词。
    expect_before(worker, "_SelectHighlightedHostCandidate(ec, pContext, reply->msg_type, &serverReply)",
                  "return _CommitServerExactText(ec, pContext, serverReply);",
                  "a digit whose word the host session cannot find commits the Server's text");
    // 占位席位要把按键吃掉（S_OK），不能退回 S_FALSE——那会被当成选择失败，丢掉组字并把拼音原文写进文档。
    expect_before(helper, "case msime::tsf::HostSelectionOutcome::PendingSeat:\n        return S_OK;",
                  "case msime::tsf::HostSelectionOutcome::Refused:\n        return S_FALSE;",
                  "a pending seat swallows the key instead of falling through to the Server's text");
    // 按文字查找：当前页上唯一的一个，没有就到完整候选列表里找唯一的一个；同形的不止一个时不猜。
    const std::string lookup = read(root + "/HostServerSelection.h");
    expect_before(lookup, "if (found) return HostSelectionOutcome::Refused;", "host.select(view.generation, found->index",
                  "an ambiguous match on the page refuses instead of guessing");
    expect_before(lookup, "host.all_candidates(raw, error)", "host.select_any(all.at(\"generation\")",
                  "a word off the host page is selected from the full list by its own generation");
    expect(lookup, "if (matched) return HostSelectionOutcome::Refused;", true,
           "an ambiguous match in the full list refuses instead of guessing");
    return failures == 0 ? EXIT_SUCCESS : EXIT_FAILURE;
}

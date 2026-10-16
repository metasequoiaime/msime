// The paired-punctuation completion lives in KeyHandler.cpp, which needs the whole TSF to build, so this checks the wiring in its source: the auto-close must go through the pair stack and the focus-token-guarded caret move, never a bare PostMessage the handler would drop. Run from the repository root, or pass the TSF source directory.
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
    return text.str();
}

void expect(const std::string &text, const char *needle, bool present, const char *what) {
    if ((text.find(needle) != std::string::npos) != present) {
        std::fprintf(stderr, "FAIL: %s\n", what);
        ++failures;
    }
}
} // namespace

int main(int argc, char **argv) {
    const std::string root = argc > 1 ? argv[1] : "platforms/windows/tsf";
    const std::string handler = read(root + "/Key/KeyHandler.cpp");
    expect(handler, "_TryStepOverPairedPunctuation(ec, pContext,", true, "a typed closing half steps over the auto-completed one");
    expect(handler, "BalanceNestPairAfterAutoClose(", true, "the auto-close balances the nest-pair count");
    expect(handler, "SendPairedPunctuationAutoClosedToServerViaNamedPipe(wch)", true, "the auto-close also balances the count in the Server's Engine");
    expect(handler, "_PushPairedPunctuation(", true, "the auto-close records the pair");
    expect(handler, "_QueuePairedPunctuationCaretMove(-1)", true, "the caret move carries the focus token");
    expect(handler, "PostMessage(_msgWndHandle, WM_PairedPunctuationCaretMove", false, "no untokened caret move that the handler drops");

    // 候选上屏的符号恰好是一个左半边时同样补全：服务端回复与点选两条上屏路径都要先补右半边，上屏后再入栈并移回光标。
    const std::string presenter = read(root + "/UI/CandidateListUIPresenter.cpp");
    expect(presenter, "_CandidateCommitPairedClosing(pendingCommitCandidate)", true, "a clicked opening mark is paired");
    expect(presenter, "_CandidateCommitPairedClosing(serverCandidateString)", true, "a selected opening mark is paired");
    expect(presenter, "_OpenCandidateCommitPair(committed.front(), pairedClosing)", true, "a clicked pair is tracked and the caret moved");
    expect(presenter, "_OpenCandidateCommitPair(candidatePairedOpening, candidatePairedClosing)", true, "a selected pair is tracked and the caret moved");
    // 宿主会话拥有组字时，数字和空格都经过 _SelectHighlightedHostCandidate 选词，这条共享路径也要补全。
    expect(presenter, "_CandidateCommitPairedClosing(commit)", true, "a digit-selected opening mark from the host session is paired");
    expect(presenter, "_OpenCandidateCommitPair(pairedOpening, pairedClosing)", true, "a digit-selected pair is tracked and the caret moved");
    // 数字选词上屏后必须结束组字，不只是在补成对标点时：组字留着的话，下一个字母开始的新组字会把刚上屏的词整段替换掉。
    {
        const auto commit = presenter.find("_AddCharAndFinalize(ec, pContext, &selectedText);");
        const auto complete = commit == std::string::npos ? commit : presenter.find("_HandleCompleteCommitFirst(ec, pContext);", commit);
        const auto pair = commit == std::string::npos ? commit : presenter.find("_OpenCandidateCommitPair(pairedOpening, pairedClosing);", commit);
        if (commit == std::string::npos || complete == std::string::npos || pair == std::string::npos || complete > pair) {
            std::fprintf(stderr, "FAIL: a digit selection from the host session ends the composition before tracking a pair\n");
            ++failures;
        }
        expect(presenter, "&& pairedClosing != 0)", false, "the digit selection does not end the composition only when a pair is completed");
    }
    const std::string composition = read(root + "/Composition/Composition.cpp");
    expect(composition, "_QueuePairedPunctuationCaretMove(-1)", true, "the candidate pair moves the caret through the focus token");

    const std::string globals = read(root + "/Global/Globals.cpp");
    const auto table = globals.find("CommitWithHighlightedCandPunc");
    const auto end = table == std::string::npos ? table : globals.find("};", table);
    if (table == std::string::npos || end == std::string::npos) {
        std::fprintf(stderr, "FAIL: CommitWithHighlightedCandPunc not found\n");
        ++failures;
    } else {
        expect(globals.substr(table, end - table), "L'/'", true, "'/' commits the highlighted candidate");
    }
    return failures == 0 ? EXIT_SUCCESS : EXIT_FAILURE;
}

// 日语空格「変換」横跨 TIP 和 Server：Server 的 ReplyComposer 执行转换并回导航回执，TIP 读到回执时保留组字并记下转换开始，回车时改走上屏高亮候选的路径。这几处都要整个 TIP 或 Server 才能构建，所以这里核对源码里的接线；决定本身由 japanese_conversion_policy 和 Server 的 windows-japanese-space-policy 验证。从仓库根目录运行，或传入 TSF 源码目录。
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
    // 下面有跨行的锚点：Git for Windows 默认 autocrlf 检出成 CRLF，读进来时去掉回车，换行一律按 LF 比较。
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
} // namespace

int main(int argc, char **argv) {
    const std::string root = argc > 1 ? argv[1] : "platforms/windows/tsf";
    const std::string presenter = read(root + "/UI/CandidateListUIPresenter.cpp");
    expect(presenter, "Global::JapaneseSpaceReplyKeepsComposition(", true, "a navigation reply to a Japanese Space keeps the composition");
    expect(presenter, "_NoteJapaneseConversionStarted();", true, "the TIP records that the conversion started");
    expect_before(presenter, "Global::JapaneseSpaceReplyKeepsComposition(", "_HandleCompleteCommitFirst(ec, pContext);\n    _OpenCandidateCommitPair(candidatePairedOpening",
                  "the navigation reply returns before the fallback commit");

    const std::string sink = read(root + "/Key/KeyEventSink.cpp");
    expect(sink, "_JapaneseEnterCommitsCandidate(capturedModifiers)", true, "Enter asks whether the conversion is running");
    expect(sink, "_candidateMode == CANDIDATE_ORIGINAL || japaneseCandidateEnter", true, "the candidate Enter carries CandidateActive to the Server");
    expect_before(sink, "const bool japaneseCandidateEnter", "WriteDataToNamedPipe(Global::Keycode", "the Enter route is decided before the key is sent");
    // 排队中的空格不能让投影以为组字已经结束，否则紧跟着的空格或回车会被漏给应用。
    expect_before(sink, "msime::windows::scheme::Japanese)\n        {\n            break;\n        }",
                  "commits and ends the composition rather than merely opening a list.\n        clearComposition();",
                  "a queued Japanese Space keeps the projected composition instead of clearing it");

    const std::string composer = read(root + "/../src/ipc/ReplyComposer.cpp");
    expect(composer, "japanese_space_applies(", true, "the Server routes a Japanese Space through the conversion");
    expect_before(composer, "japanese_space_applies(", "dispatch(session, packet, epoch, ReplyPath::Selection, uiless)",
                  "the conversion is asked before Space commits the highlighted candidate");
    expect(composer, "japanese_conversion_.reset();", true, "the conversion ends with the composition");
    return failures == 0 ? EXIT_SUCCESS : EXIT_FAILURE;
}

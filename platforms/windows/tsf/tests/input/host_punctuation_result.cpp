#include "../../HostPunctuationResult.h"
#include "../../HostCharacterResult.h"
#include <cstdio>
#include <vector>

namespace {
int failures = 0;
void check(bool condition, const char *what) {
    if (!condition) {
        std::fprintf(stderr, "FAIL: %s\n", what);
        ++failures;
    }
}
} // 命名空间

int main() {
    using namespace msime::tsf;
    EngineResult result;
    std::vector<std::string> events;
    bool refresh_ok = true;
    auto apply = [&] {
        return ApplyHostPunctuationResult(result, [&](const EngineResult &transition) {
            const auto status = ApplyHostCharacterResult(transition,
                [&](const std::string &text) { events.push_back(text); return true; },
                [&] { events.push_back("cleanup"); return true; },
                [&] { events.push_back("refresh " + transition.view.preedit); return refresh_ok; });
            return status == CharacterResultStatus::Applied ? 0 : -1;
        }, [&](const std::string &text) { events.push_back("plain " + text); return 0; });
    };

    check(!apply() && events.empty(), "unhandled punctuation leaves legacy routing available");
    // `/` 转超级简拼的实际答复形状：没有提交，仍在组字。
    result.handled = true;
    result.view.local_mode = "super_jianpin";
    result.view.editing_text = "Jnuu";
    result.view.preedit = "Jnuu";
    check(apply() == 0 && events == std::vector<std::string>{"refresh Jnuu"},
          "handled mode transition refreshes once without inserting or finalizing");
    events.clear();
    refresh_ok = false;
    check(apply() == -1 && events == std::vector<std::string>{"refresh Jnuu"},
          "refresh failure remains consumed and propagates without legacy fallback");
    events.clear();
    refresh_ok = true;

    // 第二个 `/` 的普通提交保留原直接写入路径，不再次刷新旧的预编辑。
    result.has_commit = true;
    result.commit = "合成词/";
    result.view = {};
    check(apply() == 0 && events == std::vector<std::string>{"plain 合成词/"},
          "plain punctuation commit retains the existing insertion path");
    events.clear();
    // 光标后缀提交仍交给能处理光标的组合路径。
    result.commit = "()";
    result.commit_cursor_suffix = ")";
    check(apply() == 0 && events == std::vector<std::string>{"()", "cleanup"},
          "cursor suffix uses composition application instead of plain insertion");
    events.clear();
    result.commit_cursor_suffix.reset();
    result.commit.clear();
    check(apply() == 0 && events == std::vector<std::string>{"cleanup"},
          "explicit empty commit is handled without generating a literal mark");
    events.clear();
    result.has_commit = false;
    check(apply() == 0 && events == std::vector<std::string>{"cleanup"},
          "handled idle transition consumes the key without inventing output");
    events.clear();
    result.handled = false;
    result.has_commit = true;
    result.commit = "ignored";
    check(!apply() && events.empty(), "unhandled result preserves the existing fallback rule");
    return failures ? 1 : 0;
}

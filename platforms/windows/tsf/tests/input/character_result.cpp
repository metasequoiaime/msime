#include "../../HostCharacterResult.h"
#include <cstdlib>
#include <vector>
using msime::tsf::CharacterResultStatus;
int main() {
    msime::tsf::EngineResult result;
    std::vector<std::string> events;
    bool writeOK = true, cleanupOK = true, refreshOK = true;
    auto apply = [&] {
        return msime::tsf::ApplyHostCharacterResult(result,
            [&](const std::string &text) { events.push_back(text); return writeOK; },
            [&] { events.push_back("cleanup"); return cleanupOK; },
            [&] { events.push_back("refresh"); return refreshOK; });
    };
    if (apply() != CharacterResultStatus::Unhandled || !events.empty()) return EXIT_FAILURE;
    result.handled = true;
    result.has_commit = true;
    result.commit = "synthetic-commit";
    if (apply() != CharacterResultStatus::Applied ||
        events != std::vector<std::string>{"synthetic-commit", "cleanup"}) return EXIT_FAILURE;
    events.clear();
    result.view.editing_text = "remaining";
    if (apply() != CharacterResultStatus::Applied ||
        events != std::vector<std::string>{"synthetic-commit", "cleanup", "refresh"}) return EXIT_FAILURE;
    events.clear();
    writeOK = false;
    if (apply() != CharacterResultStatus::Failed || events != std::vector<std::string>{"synthetic-commit"}) return EXIT_FAILURE;
    events.clear();
    writeOK = true;
    cleanupOK = false;
    if (apply() != CharacterResultStatus::Failed ||
        events != std::vector<std::string>{"synthetic-commit", "cleanup"}) return EXIT_FAILURE;
    events.clear();
    cleanupOK = true;
    result.has_commit = false;
    if (apply() != CharacterResultStatus::Applied || events != std::vector<std::string>{"refresh"}) return EXIT_FAILURE;
    events.clear();
    refreshOK = false;
    if (apply() != CharacterResultStatus::Failed || events != std::vector<std::string>{"refresh"}) return EXIT_FAILURE;
    events.clear();
    result.view.editing_text.clear();
    if (apply() != CharacterResultStatus::Applied || events != std::vector<std::string>{"cleanup"}) return EXIT_FAILURE;
    events.clear();
    refreshOK = true;
    result.view.phrase_prefix = "合成词";
    if (apply() != CharacterResultStatus::Applied || events != std::vector<std::string>{"refresh"}) return EXIT_FAILURE;

    // 没有组字时宿主会话不收的可打印字符（五笔下的 Shift+A）：TIP 已经吃下这个键，按上屏这个字符处理，不再答「未处理」让回放队列重试。
    msime::tsf::EngineResult declined, written;
    if (!msime::tsf::DeclinedIdleCharacterCommit(declined, 'A', &written) || !written.handled ||
        !written.has_commit || written.commit != "A") return EXIT_FAILURE;
    result = written;
    events.clear();
    if (apply() != CharacterResultStatus::Applied || events != std::vector<std::string>{"A", "cleanup"})
        return EXIT_FAILURE;
    // 其余答复不在此列：已处理、带提交、还在组字或本地模式里、不可打印的键。
    const auto declines = [&](const msime::tsf::EngineResult &value, char key) {
        msime::tsf::EngineResult out;
        return msime::tsf::DeclinedIdleCharacterCommit(value, key, &out);
    };
    if (declines(declined, '\t') || declines(declined, '\x7f') || declines(declined, '\0')) return EXIT_FAILURE;
    msime::tsf::EngineResult other;
    other.handled = true;
    if (declines(other, 'A')) return EXIT_FAILURE;
    other = {};
    other.has_commit = true;
    other.commit = "合成乙";
    if (declines(other, 'A')) return EXIT_FAILURE;
    other = {};
    other.view.editing_text = "wqab";
    if (declines(other, 'A')) return EXIT_FAILURE;
    other = {};
    other.view.phrase_prefix = "合成词";
    if (declines(other, 'A')) return EXIT_FAILURE;
    other = {};
    other.view.local_mode = "temporary_english";
    if (declines(other, '1')) return EXIT_FAILURE;
    return EXIT_SUCCESS;
}

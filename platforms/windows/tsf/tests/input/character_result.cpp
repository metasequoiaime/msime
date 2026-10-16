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
    return EXIT_SUCCESS;
}

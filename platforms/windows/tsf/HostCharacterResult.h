#pragma once
#include "EngineSessionAdapter.h"

namespace msime::tsf {
enum class CharacterResultStatus { Applied, Unhandled, Failed };
template<class Insert, class Cleanup, class Refresh>
CharacterResultStatus ApplyHostCharacterResult(const EngineResult &result,
                                               Insert insert, Cleanup cleanup, Refresh refresh) {
    if (!result.handled && !result.has_commit) return CharacterResultStatus::Unhandled;
    const bool composing = !result.view.phrase_prefix.empty() || !result.view.preedit.empty() || !result.view.editing_text.empty() ||
                           !result.view.candidates.empty();
    if (result.has_commit) {
        // 先替换旧组字再结束；先结束会留下拼写，并把提交文本重复追加。
        if (!result.commit.empty() && !insert(result.commit)) return CharacterResultStatus::Failed;
        if (!cleanup()) return CharacterResultStatus::Failed;
    }
    if (composing) {
        if (!refresh()) return CharacterResultStatus::Failed;
    } else if (!result.has_commit && !cleanup()) return CharacterResultStatus::Failed;
    return CharacterResultStatus::Applied;
}
}

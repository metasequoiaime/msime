#pragma once
#include "EngineSessionAdapter.h"
#include "HostPrediction.h"

namespace msime::tsf {
enum class CharacterResultStatus { Applied, Unhandled, Failed };

// 没有组字时宿主会话不收的可打印字符：五笔下 Engine 不收的 Shift+字母（A，以及按拼音拼写查的 E、M、J 和 V），「Shift+字母进入快捷模式」总开关关掉时拼音的大写字母。TIP 在按下时就分类（排在前面、还没应用的键决定这个键遇到的会话状态，而且 TIP 把五笔记成全拼），只有到编辑会话里问过宿主会话才知道 Engine 收不收，这时键已经当输入吃下，应用收不到它；Engine 的 handled=false 是最终答复，重放同一个键得到的还是它。所以把答复改写成上屏这个字符，和 macOS、Linux 把 handled=false 的键交还应用打出的字符相同。组字中不收的键（它们要先结束组字）、带提交的答复和不可打印的键不在此列，照旧算没处理。
inline bool DeclinedIdleCharacterCommit(const EngineResult &result, char key, EngineResult *commit) {
    const auto &view = result.view;
    if (result.handled || result.has_commit || key < 0x20 || key > 0x7e || HostViewComposes(view) ||
        view.prediction || view.local_mode != "none")
        return false;
    *commit = result;
    commit->handled = true;
    commit->has_commit = true;
    commit->commit.assign(1, key);
    return true;
}

template<class Insert, class Cleanup, class Refresh>
CharacterResultStatus ApplyHostCharacterResult(const EngineResult &result,
                                               Insert insert, Cleanup cleanup, Refresh refresh) {
    if (!result.handled && !result.has_commit) return CharacterResultStatus::Unhandled;
    // 上屏后联想不算组字：它在上屏之后由 TIP 显示在光标处（_RefreshPrediction），这里只管上屏和结束组字。
    const bool composing = HostViewComposes(result.view);
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

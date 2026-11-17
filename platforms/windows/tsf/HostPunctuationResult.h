#pragma once
#include <optional>

namespace msime::tsf {

// 已处理的符号也可能只切换本地模式；必须刷新组字并消费结果，不能继续执行旧标点路径。
template<class Result, class ApplyComposition, class InsertCommit>
auto ApplyHostPunctuationResult(const Result &result,
                                 ApplyComposition applyComposition,
                                 InsertCommit insertCommit)
    -> std::optional<decltype(applyComposition(result))> {
    if (!result.handled) return std::nullopt;
    if (!result.has_commit || result.commit.empty() || result.commit_cursor_suffix)
        return applyComposition(result);
    return insertCommit(result.commit);
}

} // 命名空间 msime::tsf

#pragma once

namespace msime::windows::tsf
{
// 焦点文档的 context 栈变化先恢复语言栏状态；只有顶层 context 真正变化时才重绑编辑 sink。
enum class FocusedContextStackChangeAction
{
    Ignore,
    RefreshLanguageBar,
    RefreshLanguageBarAndRebindTextEditSink,
};

struct FocusedContextStackChangeState
{
    bool ownsFocusState = false;
    bool hasChangedContext = false;
    bool hasChangedDocument = false;
    bool hasFocusedDocument = false;
    bool documentsMatch = false;
    bool hasTopContext = false;
    bool topContextChanged = false;
};

constexpr FocusedContextStackChangeAction DecideFocusedContextStackChange(
    FocusedContextStackChangeState state) noexcept
{
    if (!state.ownsFocusState || !state.hasChangedContext || !state.hasChangedDocument || !state.hasFocusedDocument ||
        !state.documentsMatch || !state.hasTopContext)
    {
        return FocusedContextStackChangeAction::Ignore;
    }
    return state.topContextChanged ? FocusedContextStackChangeAction::RefreshLanguageBarAndRebindTextEditSink
                                   : FocusedContextStackChangeAction::RefreshLanguageBar;
}
} // namespace msime::windows::tsf

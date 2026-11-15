#include <stdexcept>

#include "../../Thread/ContextStackChangePolicy.h"

namespace
{
using Action = msime::windows::tsf::FocusedContextStackChangeAction;
using State = msime::windows::tsf::FocusedContextStackChangeState;

void require(bool condition, const char *message)
{
    if (!condition)
    {
        throw std::runtime_error(message);
    }
}
} // namespace

int main()
{
    using msime::windows::tsf::DecideFocusedContextStackChange;

    // QQ 的时序是先收到文档焦点、后压入第一个 context；压入后必须恢复语言栏按钮并绑定编辑 sink。
    require(DecideFocusedContextStackChange({true, true, true, true, true, true, true}) ==
                Action::RefreshLanguageBarAndRebindTextEditSink,
            "a focused context pushed after focus must restore the language bar and bind the edit sink");

    // GetDocumentMgr 与 GetFocus 可以返回同一 COM 对象的不同接口包装，判断必须使用 COM identity 后的结果。
    require(DecideFocusedContextStackChange({true, true, true, true, true, true, true}) != Action::Ignore,
            "different COM wrappers for one focused document must still be handled");

    // 非当前焦点文档的 context 栈变化不能抢走当前会话的语言栏状态。
    require(DecideFocusedContextStackChange({true, true, true, true, false, true, true}) == Action::Ignore,
            "a background document context change must be ignored");

    // 栈变化但顶层 context 未变时也要刷新语言栏，覆盖焦点回调早于 context 压入的窗口。
    require(DecideFocusedContextStackChange({true, true, true, true, true, true, false}) ==
                Action::RefreshLanguageBar,
            "a focused stack notification must refresh the language bar even without a top-context rebind");

    require(DecideFocusedContextStackChange({true, true, true, true, true, false, true}) == Action::Ignore,
            "an empty focused document cannot refresh the language bar");
    return 0;
}

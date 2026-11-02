#pragma once

// 延迟键屏障里的智能标点。
//
// Server 回得慢、或者有偏好事务在排队时，TIP 会先吃掉按键、排进队列，等屏障落下再回放
// （KeyEventSink 的 `_ClassifyDeferredKeyDown`）。这条路径把「没有功能」的可打印键当成
// 应用文本排队，由 TIP 自己按顺序插进文档。两件事要在这里定清楚：
//
// 1. 这样插进文档的字符，和直通给宿主的字符一样，就是用户刚刚打下的那个字。智能标点的
//    前文影子（`_smartPunctuationShadowChar`）必须记下它；当成「吃进组字的键」而让它失效
//    的话，下一个标点就只能去问文档，而 QQ 这类 Chromium/IMM32 宿主的文本存储读不回前文，
//    于是数字后的 `,` `.` `:` 变成中文标点。表现就是时好时坏：取决于那一瞬间有没有屏障。
// 2. 空格转换和重复标点转中文只看本进程记住的上一次上屏和当前焦点，不需要 Server 的分类
//    结果。它们必须排在「可打印键排进应用文本队列」那条兜底规则之前认领，否则空格会被
//    当成普通空格插进文档，刚打出的中文标点再也换不回 ASCII。
namespace Global
{
// 「没有功能」的按键不是组字也不是命令，只是要打的字：队列按它分应用文本，智能标点按它
// 决定要不要把字符记进前文影子。`_QueueDeferredKeyDown` 与 `_NoteKeyForSmartPunctuation`
// 共用这一条规则，两边判得不一样就会出现「排进去了却不算打进去了」。
constexpr bool DeferredKeyIsApplicationText(bool hasCategory, bool hasFunction)
{
    return !hasCategory && !hasFunction;
}

// 屏障里要由 TIP 自己认领的动作。
enum class DeferredSmartPunctuationClaim
{
    None,
    SpaceConvert,
    Revert,
};

constexpr DeferredSmartPunctuationClaim DeferredSmartPunctuationClaimFor(bool spacePressed, bool canConvertSpace,
                                                                        bool canRevert)
{
    if (spacePressed && canConvertSpace)
    {
        return DeferredSmartPunctuationClaim::SpaceConvert;
    }
    return canRevert ? DeferredSmartPunctuationClaim::Revert : DeferredSmartPunctuationClaim::None;
}

// 已经武装的空格转换还属不属于这篇文档。
//
// `_CanConvertSmartPunctuationSpace()` 还要等 Server 会话说「就绪」，而那个状态在焦点会话
// 协商、偏好事务期间会短暂为假并自己恢复。用它在每个中间按键上判一次就把手势丢掉了：
// 标点先上屏、紧接着一个键落到协商窗口里，空格就再也不会转换。这里只丢真正回不来的
// 情况——焦点会话换了、前台窗口换了、过期；是否就绪留到空格那一刻再判一次。
//
// 「重复标点转中文」不走这一条：它靠盲删一个字符兑现，读不回前文时放宽窗口只会多出误删
// 的机会，仍按每个按键判。
constexpr bool SmartPunctuationArmStillBelongsToDocument(unsigned long long armedToken,
                                                         unsigned long long currentToken, bool sameForeground,
                                                         bool withinWindow)
{
    return withinWindow && sameForeground && armedToken != 0 && armedToken == currentToken;
}
} // namespace Global

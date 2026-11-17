#pragma once

namespace Global
{
// UI-less 宿主在 BeginUIElement 中返回 TRUE 时，表示允许输入法显示自己的候选窗。
// 沉浸式宿主必须先按运行时激活标志决定是否把这条协商结果交给自己的候选窗。
inline bool ShouldShowImeCandidateWindow(bool hostUiLess, bool immersiveMode, bool beginShow, bool forceOverlay)
{
    if (forceOverlay)
        return true;
    if (!hostUiLess)
        return beginShow;
    return immersiveMode && beginShow;
}

// Server 在首次 ClientActivated 时要知道宿主是否要求 UI-less。沉浸式宿主的候选呈现要等
// BeginUIElement 的 pbShow 协商，因此先不锁存宿主 UI-less，后续按 CandidateUiLessMode 发送。
inline bool ShouldReportHostUiLess(bool hostUiLess, bool immersiveMode, bool forceOverlay)
{
    return hostUiLess && !immersiveMode && !forceOverlay;
}
} // namespace Global

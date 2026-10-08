#pragma once
#include <Windows.h>
#include <string>
#include <string_view>
#include <vector>

namespace Global
{
// 从完整路径取可执行文件基名，`\` 和 `/` 都当作分隔符。
inline std::wstring_view ProcessBaseName(std::wstring_view path)
{
    const size_t separator = path.find_last_of(L"\\/");
    return separator == std::wstring_view::npos ? path : path.substr(separator + 1);
}

// 判定要看的进程现场：宿主是否以 UILess 激活，SDL2、Source 2 的模块是否已加载，本线程是否有 SDL 的顶层窗口。
struct CandidateOverlayFacts
{
    bool hostUiLess = false;
    bool sdl2Loaded = false;
    bool sdlWindow = false;
    bool source2Loaded = false;
};

// 一次激活里的判定材料，留给 BeginUIElement 复判和延迟写的 `[game]` 诊断日志用：诊断日志的开关要等 Server 推来配置帧才打开，晚于 ActivateEx。
struct CandidateOverlayDecision
{
    // 偏好读取成功。读取失败时一律不强制，见 ReadConfiguredGameCompatibility。
    bool preferencesRead = false;
    bool enabled = false;
    std::vector<std::wstring> overlayProcesses;
    std::vector<std::wstring> excludedProcesses;
    CandidateOverlayFacts facts;
    // BeginUIElement 复判时 GetActiveFlags 报告了 UILess，而 ActivateEx 时的标志没有。
    bool activeFlagsUiLess = false;
    bool logged = false;
    bool sdl2HiddenLogged = false;
};

// 进程名按序数比较、不分大小写，与用户的区域设置无关。
inline bool IsSameProcessName(std::wstring_view left, std::wstring_view right)
{
    return CompareStringOrdinal(left.data(), static_cast<int>(left.size()), right.data(),
                                static_cast<int>(right.size()), TRUE) == CSTR_EQUAL;
}

inline bool ContainsProcessName(const std::vector<std::wstring> &names, std::wstring_view process)
{
    for (const auto &name : names)
    {
        if (IsSameProcessName(name, process))
            return true;
    }
    return false;
}

// 游戏只声明 UILess、自己却不画候选时，由水杉的候选窗来画。从上到下取第一个命中的结果：开关关闭或进程名为空、用户「从不显示」、用户「总是显示」、内置排除表、内置表和模块规则。
inline bool ShouldForceCandidateOverlay(bool enabled, const CandidateOverlayFacts &facts, std::wstring_view process,
                                        const std::vector<std::wstring> &overlay,
                                        const std::vector<std::wstring> &excluded)
{
    if (!enabled || process.empty())
        return false;
    if (ContainsProcessName(excluded, process))
        return false;
    if (ContainsProcessName(overlay, process))
        return true;
    // 英雄联盟自己画候选，排除它以免被模块规则误判。文件名没有实机核实过。
    constexpr std::wstring_view builtinExcluded[] = {L"league of legends.exe"};
    for (const auto name : builtinExcluded)
    {
        if (IsSameProcessName(name, process))
            return false;
    }
    // CS2 和 Dota 2 声明 UILess，候选只给游戏自己白名单里的输入法画。CS2 的 Trusted Mode 只放行系统目录里签了名的 DLL，安装器因此把 64 位 TIP 装进 System32\IME。
    constexpr std::wstring_view builtinOverlay[] = {L"cs2.exe", L"dota2.exe"};
    for (const auto name : builtinOverlay)
    {
        if (IsSameProcessName(name, process))
            return true;
    }
    // Source 2 引擎，模块名来自 SteamDB GameTracking，没有实机核实过。
    if (facts.source2Loaded)
        return true;
    // SDL2 在 UILess 下一律回 pbShow=FALSE，自己又不画候选；要求本线程有 SDL 窗口，是为了不把只用 SDL2 读手柄的游戏算进来。
    return facts.hostUiLess && facts.sdl2Loaded && facts.sdlWindow;
}

inline BOOL CALLBACK FindSdlAppWindow(HWND window, LPARAM found)
{
    wchar_t className[16] = {};
    const int length = GetClassNameW(window, className, ARRAYSIZE(className));
    // SDL2 默认的窗口类名是 SDL_app，这是凭对 SDL 源码的记忆写的，实测以诊断日志为准。
    if (length > 0 && CompareStringOrdinal(className, length, L"SDL_app", -1, TRUE) == CSTR_EQUAL)
    {
        *reinterpret_cast<bool *>(found) = true;
        return FALSE;
    }
    return TRUE;
}

// 收集当前进程和线程的现场。模块只用 GetModuleHandleW 查已加载的，不会加载 DLL；只有 UILess 宿主并且已加载 SDL2 时，才枚举本线程的顶层窗口。
inline CandidateOverlayFacts ReadCandidateOverlayFacts(bool hostUiLess)
{
    CandidateOverlayFacts facts;
    facts.hostUiLess = hostUiLess;
    facts.sdl2Loaded = GetModuleHandleW(L"SDL2.dll") != nullptr;
    facts.source2Loaded =
        GetModuleHandleW(L"engine2.dll") != nullptr && GetModuleHandleW(L"imemanager.dll") != nullptr;
    if (hostUiLess && facts.sdl2Loaded)
    {
        EnumThreadWindows(GetCurrentThreadId(), FindSdlAppWindow, reinterpret_cast<LPARAM>(&facts.sdlWindow));
    }
    return facts;
}
} // namespace Global

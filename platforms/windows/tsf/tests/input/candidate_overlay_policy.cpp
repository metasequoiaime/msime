#include "../../Global/CandidateOverlayHostPolicy.h"

#include <stdexcept>
#include <string>
#include <vector>

namespace
{
void require(bool condition, const char *message)
{
    if (!condition)
    {
        throw std::runtime_error(message);
    }
}

using Names = std::vector<std::wstring>;

Global::CandidateOverlayFacts Facts(bool hostUiLess, bool sdl2Loaded, bool sdlWindow, bool source2Loaded)
{
    Global::CandidateOverlayFacts facts;
    facts.hostUiLess = hostUiLess;
    facts.sdl2Loaded = sdl2Loaded;
    facts.sdlWindow = sdlWindow;
    facts.source2Loaded = source2Loaded;
    return facts;
}

bool Force(std::wstring_view process, const Global::CandidateOverlayFacts &facts = {}, const Names &overlay = {},
           const Names &excluded = {}, bool enabled = true)
{
    return Global::ShouldForceCandidateOverlay(enabled, facts, process, overlay, excluded);
}
} // namespace

int main()
{
    require(Global::ProcessBaseName(L"C:\\Games\\dota 2 beta\\game\\bin\\win64\\dota2.exe") == L"dota2.exe",
            "the basename follows the last backslash");
    require(Global::ProcessBaseName(L"D:/Games/Terraria/Terraria.exe") == L"Terraria.exe",
            "a forward slash also separates the basename");
    require(Global::ProcessBaseName(L"game.exe") == L"game.exe", "a bare name is its own basename");
    require(Global::ProcessBaseName(L"C:\\Games\\").empty(), "a trailing separator leaves no basename");

    // 用户列表：大小写不敏感
    require(Force(L"Game.EXE", {}, {L"game.exe"}), "the user overlay list matches case-insensitively");
    require(!Force(L"notepad.exe", {}, {L"game.exe"}), "a process outside every list is not forced");
    require(!Force(L"GAME.exe", {}, {L"game.exe"}, {L"Game.exe"}), "the user exclusion wins over the user overlay list");

    // 内置排除表在用户「总是显示」之后、内置表和模块规则之前
    require(!Force(L"League of Legends.exe", Facts(true, true, true, true)),
            "the built-in exclusion wins over the module rules");
    require(Force(L"league of legends.exe", {}, {L"League of Legends.exe"}),
            "the user overlay list wins over the built-in exclusion");

    // 内置表
    require(Force(L"dota2.exe"), "Dota 2 is in the built-in table");
    require(Force(L"CS2.EXE"), "CS2 is in the built-in table, case-insensitively");
    require(!Force(L"dota2.exe", {}, {}, {L"DOTA2.EXE"}), "the user exclusion wins over the built-in table");

    // Source 2 模块规则不看 UILess
    require(Force(L"deadlock.exe", Facts(false, false, false, true)), "a loaded Source 2 engine forces the overlay");
    require(!Force(L"deadlock.exe", Facts(true, false, false, true), {}, {L"deadlock.exe"}),
            "the user exclusion wins over the Source 2 rule");

    // SDL2 规则要三者同时满足
    require(Force(L"terraria.exe", Facts(true, true, true, false)), "a UILess SDL2 host with an SDL window is forced");
    require(!Force(L"terraria.exe", Facts(false, true, true, false)), "SDL2 without UILess is not forced");
    require(!Force(L"terraria.exe", Facts(true, false, true, false)), "UILess without SDL2 is not forced");
    require(!Force(L"terraria.exe", Facts(true, true, false, false)),
            "SDL2 without an SDL window (a controller-only user) is not forced");
    require(!Force(L"terraria.exe", Facts(true, false, false, false)), "a plain UILess host is not forced");

    // 开关关闭、进程名为空、偏好读取失败（调用方传 enabled=false）一律不强制
    require(!Force(L"dota2.exe", Facts(true, true, true, true), {L"dota2.exe"}, {}, false),
            "a disabled switch, or unreadable preferences, never forces");
    require(!Force(L"", Facts(true, true, true, true), {L""}), "an unknown process is never forced");
    return 0;
}

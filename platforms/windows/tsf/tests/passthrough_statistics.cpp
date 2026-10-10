#include "PassthroughStatistics.h"

#include <cstdio>
#include <string>

namespace
{
int failures = 0;

void Require(bool value, const char *what)
{
    if (!value)
    {
        std::fprintf(stderr, "passthrough statistics: %s\n", what);
        ++failures;
    }
}
} // namespace

int main()
{
    Require(ShouldCountPassthroughChar(L'1', false, false, false, false), "a digit counts");
    Require(ShouldCountPassthroughChar(L'A', false, false, false, false), "a shifted letter counts");
    Require(ShouldCountPassthroughChar(L' ', false, false, false, false), "space reaches the Server, which skips whitespace itself");
    Require(!ShouldCountPassthroughChar(L'a', true, false, false, false), "nothing counts without a focused edit context");
    Require(!ShouldCountPassthroughChar(L'c', false, true, false, false), "Ctrl combinations are shortcuts");
    Require(!ShouldCountPassthroughChar(L'c', false, false, true, false), "Alt combinations are shortcuts");
    Require(!ShouldCountPassthroughChar(L'c', false, false, false, true), "Win combinations are shortcuts");
    Require(!ShouldCountPassthroughChar(L'\r', false, false, false, false), "Enter is not a character");
    Require(!ShouldCountPassthroughChar(L'\b', false, false, false, false), "Backspace is not a character");
    Require(!ShouldCountPassthroughChar(0x7F, false, false, false, false), "DEL is not a character");
    Require(!ShouldCountPassthroughChar(L'\0', false, false, false, false), "a key without a character does not count");
    Require(!ShouldCountPassthroughChar(static_cast<wchar_t>(0xD83D), false, false, false, false), "a lone surrogate does not count");
    Require(IsPanelTextSendInput(0x4D535053u), "text the settings app's panels inject is counted there, not here");
    Require(!IsPanelTextSendInput(0), "ordinary keys carry no panel marker");
    Require(!IsPanelTextSendInput(0x4D535050u), "smart punctuation's own marker is not panel text");

    if (failures == 0)
    {
        std::puts("passthrough statistics: filter passed");
    }
    return failures == 0 ? 0 : 1;
}

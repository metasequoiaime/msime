#include "../../Composition/PreeditCaret.h"
#include <cstdlib>
#include <limits>
using msime::tsf::MapPreeditCaret;
int main() {
    if (MapPreeditCaret(L"nihao", 3, L"ni'hao", 0) != 4 ||
        MapPreeditCaret(L"ni'hao", 3, L"ni'hao", 0) != 3 ||
        MapPreeditCaret(L"ni'hao", 2, L"ni'hao", 0) != 2 ||
        MapPreeditCaret(L"ni'hao", 4, L"ni'hao", 0) != 4 ||
        MapPreeditCaret(L"hao", 1, L"你hao", 1) != 2 ||
        MapPreeditCaret(L"hao", 0, L"你hao", 1) != 1 ||
        MapPreeditCaret(L"hao", 2, L"", 0) != 0 ||
        MapPreeditCaret(L"", 9, L"", 0) != 0 ||
        MapPreeditCaret(L"a", 99, L"a", 99) != 1 ||
        MapPreeditCaret(L"abc", (std::numeric_limits<std::size_t>::max)(), L"ab", 0) != 2)
        return EXIT_FAILURE;
    // Explicit surrogate code units exercise Windows offsets even when this
    // test runs on a host whose wchar_t is 32 bits.
    const std::wstring_view prefixed = L"\xD83C\xDF32hao";
    if (MapPreeditCaret(L"hao", 1, prefixed, 2) != 3) return EXIT_FAILURE;
    // A host caret must not be replaced by the stale legacy buffer's caret.
    if (MapPreeditCaret(L"nihao", 1, L"ni'hao", 0) != 1 ||
        MapPreeditCaret(L"nihao", 4, L"ni'hao", 0) != 5) return EXIT_FAILURE;
    // #10632：双拼「拼音分词」的 preedit 是展开的全拼（uiui 是 shi'shi），按字母数会落在 shi's|hi。画的就是引擎的 preedit 时用引擎给的位置，已选词段照样往后移。
    if (MapPreeditCaret(L"uiui", 4, L"shi'shi", 0) == 7 ||
        MapPreeditCaret(L"uiui", 4, L"shi'shi", 0, L"shi'shi", 7) != 7 ||
        MapPreeditCaret(L"uiui", 3, L"shi'shi", 0, L"shi'shi", 3) != 3 ||
        MapPreeditCaret(L"uiui", 4, L"你shi'shi", 1, L"shi'shi", 7) != 8 ||
        MapPreeditCaret(L"uiui", 4, L"shi'shi", 0, L"shi'shi", 99) != 7)
        return EXIT_FAILURE;
    // 画的不是引擎的 preedit（原始按键、Server 送来的另一份），或者视图里没有这一项时，照旧按字母数换。
    if (MapPreeditCaret(L"uiui", 2, L"uiui", 0, L"shi'shi", 3) != 2 ||
        MapPreeditCaret(L"nihao", 2, L"ni'hao", 0, L"ni'hao", std::nullopt) != 2)
        return EXIT_FAILURE;
    return EXIT_SUCCESS;
}

#include "../../Global/LocalModeKeyPolicy.h"
#include <cstdio>
#include <string>

namespace {
int failures = 0;

void check(bool condition, const char *what) {
    if (!condition) {
        std::fprintf(stderr, "FAIL: %s\n", what);
        ++failures;
    }
}
} // namespace

int main() {
    using namespace Global;

    // The V mode is a buffer that starts with the V that opened it, and only while the Server reports the mode on.
    const std::wstring expression = L"V1+2";
    check(IsExpressionModeComposition(expression.c_str(), expression.size(), true), "V buffer is the mode");
    check(!IsExpressionModeComposition(expression.c_str(), expression.size(), false), "switched off, V is a capital");
    check(!IsExpressionModeComposition(L"U4e2d", 5, true), "U buffer is not V");
    check(!IsExpressionModeComposition(L"nihao", 5, true), "pinyin is not V");
    check(!IsExpressionModeComposition(L"", 0, true), "empty buffer is no mode");
    check(!IsExpressionModeComposition(nullptr, 3, true), "no buffer is no mode");

    // Its digits and operators are input, whichever key typed them.
    for (wchar_t symbol : std::wstring(L"0123456789+-*/.()%^"))
        check(IsSpellingSymbol(ExpressionSpellingSymbols, symbol), "every V symbol is spelled");
    for (wchar_t other : std::wstring(L"=!@#$&,;'aV "))
        check(!IsSpellingSymbol(ExpressionSpellingSymbols, other), "other text is not spelled");
    check(!IsSpellingSymbol(ExpressionSpellingSymbols, L'\0'), "no text is not spelled");
    check(ClassifyModeKey(ExpressionSpellingSymbols, L'4', L'4') == ExpressionKey::Input, "digit composes");
    check(ClassifyModeKey(ExpressionSpellingSymbols, 0x64, L'4') == ExpressionKey::Input, "numpad digit composes");
    check(ClassifyModeKey(ExpressionSpellingSymbols, 0xBD, L'-') == ExpressionKey::Input, "minus composes instead of paging");
    check(ClassifyModeKey(ExpressionSpellingSymbols, 0xBB, L'+') == ExpressionKey::Input, "Shift+= plus composes");
    check(ClassifyModeKey(ExpressionSpellingSymbols, L'8', L'*') == ExpressionKey::Input, "Shift+8 star composes, not the eighth row");
    check(ClassifyModeKey(ExpressionSpellingSymbols, L'9', L'(') == ExpressionKey::Input, "Shift+9 paren composes");
    check(ClassifyModeKey(ExpressionSpellingSymbols, L'5', L'%') == ExpressionKey::Input, "Shift+5 percent composes");
    check(ClassifyModeKey(ExpressionSpellingSymbols, 0xBE, L'.') == ExpressionKey::Input, "period composes instead of paging");
    check(ClassifyModeKey(ExpressionSpellingSymbols, 0xBF, L'/') == ExpressionKey::Input, "slash composes instead of punctuation");
    // A digit key printing anything else selects its row, with Shift on a US layout or bare on one whose digit row needs Shift.
    check(ClassifyModeKey(ExpressionSpellingSymbols, L'1', L'!') == ExpressionKey::SelectByNumber, "Shift+1 selects");
    check(ClassifyModeKey(ExpressionSpellingSymbols, L'2', L'@') == ExpressionKey::SelectByNumber, "Shift+2 selects");
    check(ClassifyModeKey(ExpressionSpellingSymbols, L'1', L'&') == ExpressionKey::SelectByNumber, "AZERTY bare 1 selects");
    // Everything else keeps its usual route.
    check(ClassifyModeKey(ExpressionSpellingSymbols, L'0', L')') == ExpressionKey::Input, "Shift+0 paren composes");
    check(ClassifyModeKey(ExpressionSpellingSymbols, L'0', 0x00E0) == ExpressionKey::Unclaimed, "zero key printing a letter is not claimed");
    check(ClassifyModeKey(ExpressionSpellingSymbols, 0xBB, L'=') == ExpressionKey::Unclaimed, "equals keeps paging");
    check(ClassifyModeKey(ExpressionSpellingSymbols, 0xBC, L',') == ExpressionKey::Unclaimed, "comma keeps paging");
    check(ClassifyModeKey(ExpressionSpellingSymbols, L'A', L'a') == ExpressionKey::Unclaimed, "letters keep their route");

    // 网址模式：触发词恰好是整个缓冲、光标在末尾、方案检测网址且不在专用英文模式时，触发键打开它。
    const auto opens = [](const wchar_t *buffer, wchar_t key) {
        const std::wstring raw = buffer;
        return OpensUrlMode(raw.c_str(), raw.size(), raw.size(), key, true, false);
    };
    check(opens(L"www", L'.'), "www dot opens the URL mode");
    check(opens(L"http", L':'), "http colon opens the URL mode");
    check(opens(L"https", L':'), "https colon opens the URL mode");
    check(opens(L"ftp", L'.') && opens(L"ftp", L':'), "ftp opens on dot and colon");
    check(!opens(L"www", L':') && !opens(L"http", L'.') && !opens(L"www", L'@'), "only the trigger's own key opens it");
    for (const wchar_t *other : {L"ww", L"wwww", L"Www", L"WWW", L"w'ww", L"ni", L"httpss", L"mailto"})
        check(!opens(other, L'.') && !opens(other, L':'), "only the exact lowercase trigger words open it");
    const std::wstring www = L"www";
    check(!OpensUrlMode(www.c_str(), www.size(), 2, L'.', true, false), "not with the caret inside the trigger");
    check(!OpensUrlMode(www.c_str(), www.size(), www.size(), L'.', false, false), "not in a scheme that does not detect URLs");
    check(!OpensUrlMode(www.c_str(), www.size(), www.size(), L'.', true, true), "not in the dedicated English mode");
    check(!OpensUrlMode(L"", 0, 0, L'.', true, false), "not on an empty composition");
    check(!OpensUrlMode(nullptr, 3, 3, L'.', true, false), "no buffer opens nothing");

    // 删掉一个字符后：只有删掉的恰好是触发键（剩下的正好是触发词）或缓冲删空时才退出网址模式，光标处的编辑不会因为缓冲开头变了就退出。
    const auto staysAfter = [](const wchar_t *remaining, wchar_t removed) {
        const std::wstring raw = remaining;
        return UrlModeAfterDeletion(true, raw.c_str(), raw.size(), removed);
    };
    check(!staysAfter(L"www", L'.'), "deleting the dot after www leaves the URL mode");
    check(!staysAfter(L"http", L':') && !staysAfter(L"https", L':'), "deleting the colon after http or https leaves it");
    check(!staysAfter(L"ftp", L'.') && !staysAfter(L"ftp", L':'), "deleting either ftp key leaves it");
    check(!staysAfter(L"", L'.'), "an emptied buffer leaves it");
    check(staysAfter(L"ww.", L'w'), "deleting a trigger letter at the caret keeps it");
    check(staysAfter(L"xwww.", L'a'), "a buffer that no longer starts with the trigger keeps it");
    check(staysAfter(L"www.", L'a'), "deleting a later character keeps it");
    check(staysAfter(L"www", L':'), "a removed character that is not the trigger's key keeps it");
    check(staysAfter(L"https", L'.'), "https with a removed dot keeps it");
    check(staysAfter(L"http", L's'), "the TIP keys Wubi as quanpin, so http with a removed s keeps it");
    check(!UrlModeAfterDeletion(false, L"www.a", 5, L'b'), "deleting never enters the URL mode");
    check(!UrlModeAfterDeletion(true, nullptr, 3, L'.'), "no buffer is no URL");

    // 网址的数字和符号是输入，包括 Shift 打出的符号和在别处翻页的键。
    for (wchar_t symbol : std::wstring(L"0123456789-._~:/?#[]@!$&'()*+,;=%^"))
        check(IsSpellingSymbol(UrlSpellingSymbols, symbol), "every URL symbol is spelled");
    for (wchar_t other : std::wstring(L"\"<>\\{}|` a"))
        check(!IsSpellingSymbol(UrlSpellingSymbols, other), "URL enders are not spelled");
    check(!IsSpellingSymbol(UrlSpellingSymbols, L'\0'), "no text is not spelled in a URL");
    check(ClassifyModeKey(UrlSpellingSymbols, L'1', L'1') == ExpressionKey::Input, "digit is URL input, not a pick");
    check(ClassifyModeKey(UrlSpellingSymbols, L'0', L'0') == ExpressionKey::Input, "zero is URL input");
    check(ClassifyModeKey(UrlSpellingSymbols, 0x61, L'1') == ExpressionKey::Input, "numpad digit is URL input");
    check(ClassifyModeKey(UrlSpellingSymbols, L'1', L'!') == ExpressionKey::Input, "Shift+1 bang is URL input");
    check(ClassifyModeKey(UrlSpellingSymbols, L'2', L'@') == ExpressionKey::Input, "Shift+2 at is URL input");
    check(ClassifyModeKey(UrlSpellingSymbols, L'3', L'#') == ExpressionKey::Input, "Shift+3 hash is URL input");
    check(ClassifyModeKey(UrlSpellingSymbols, 0xBE, L'.') == ExpressionKey::Input, "period is URL input instead of paging");
    check(ClassifyModeKey(UrlSpellingSymbols, 0xBC, L',') == ExpressionKey::Input, "comma is URL input instead of paging");
    check(ClassifyModeKey(UrlSpellingSymbols, 0xBD, L'-') == ExpressionKey::Input, "minus is URL input instead of paging");
    check(ClassifyModeKey(UrlSpellingSymbols, 0xBB, L'=') == ExpressionKey::Input, "equals is URL input instead of paging");
    check(ClassifyModeKey(UrlSpellingSymbols, 0xDB, L'[') == ExpressionKey::Input, "bracket is URL input instead of paging");
    check(ClassifyModeKey(UrlSpellingSymbols, 0xBF, L'/') == ExpressionKey::Input, "slash is URL input instead of punctuation");
    check(ClassifyModeKey(UrlSpellingSymbols, 0xBA, L':') == ExpressionKey::Input, "colon is URL input");
    check(ClassifyModeKey(UrlSpellingSymbols, L'2', 0x00E9) == ExpressionKey::SelectByNumber, "AZERTY bare 2 printing e-acute selects");
    check(ClassifyModeKey(UrlSpellingSymbols, 0xBC, L'<') == ExpressionKey::Unclaimed, "Shift+comma keeps the punctuation route");
    check(ClassifyModeKey(UrlSpellingSymbols, 0xDC, L'\\') == ExpressionKey::Unclaimed, "backslash keeps the punctuation route");
    check(ClassifyModeKey(UrlSpellingSymbols, 0xDB, L'{') == ExpressionKey::Unclaimed, "brace keeps the punctuation route");
    check(ClassifyModeKey(UrlSpellingSymbols, L'A', L'a') == ExpressionKey::Unclaimed, "letters keep their route");
    check(ClassifyModeKey(UrlSpellingSymbols, 0x20, L' ') == ExpressionKey::Unclaimed, "space keeps selecting");

    // "/" and "@" open their modes only on an empty composition, with Chinese punctuation, and for a mode that is on.
    check(OpensLocalMode(L'/', false, true, true, false), "slash opens commands");
    check(OpensLocalMode(L'@', false, true, false, true), "at opens mentions");
    check(!OpensLocalMode(L'/', false, true, false, true), "slash needs the command mode");
    check(!OpensLocalMode(L'@', false, true, true, false), "at needs the mention mode");
    check(!OpensLocalMode(L'/', true, true, true, true), "not inside a composition");
    check(!OpensLocalMode(L'/', false, false, true, true), "not with ASCII punctuation");
    check(!OpensLocalMode(L'#', false, true, true, true), "no other mark opens a mode");

    // The Server's frame: three '0'/'1' flags; anything else turns every mode off.
    const auto parse = [](const wchar_t *payload) {
        wchar_t buffer[8] = {};
        for (size_t index = 0; payload[index] && index < 7; ++index)
            buffer[index] = payload[index];
        return ParseLocalModeTriggers(buffer, 8);
    };
    auto triggers = parse(L"101");
    check(triggers.expression && !triggers.command && triggers.mention, "flags in V, /, @ order");
    triggers = parse(L"010");
    check(!triggers.expression && triggers.command && !triggers.mention, "command alone");
    triggers = parse(L"111");
    check(triggers.expression && triggers.command && triggers.mention, "all on");
    // The Server sends V off while the focused Engine is in its own English mode, so an English word starting with V is not the V mode and its digits still select.
    triggers = parse(L"011");
    const std::wstring english = L"Very";
    check(!IsExpressionModeComposition(english.c_str(), english.size(), triggers.expression), "V is a letter in English mode");
    for (const wchar_t *invalid : {L"", L"1", L"11", L"1111", L"1x1", L"abc"}) {
        triggers = parse(invalid);
        check(!triggers.expression && !triggers.command && !triggers.mention, "malformed frame is all off");
    }
    const wchar_t short_buffer[3] = {L'1', L'1', L'1'};
    triggers = ParseLocalModeTriggers(short_buffer, 3);
    check(!triggers.expression && !triggers.command && !triggers.mention, "unterminated frame is all off");
    triggers = ParseLocalModeTriggers(nullptr, 8);
    check(!triggers.expression && !triggers.command && !triggers.mention, "no frame is all off");

    if (failures != 0) {
        std::fprintf(stderr, "%d local mode key policy check(s) failed\n", failures);
        return 1;
    }
    std::puts("TSF local mode key policy checks passed");
    return 0;
}

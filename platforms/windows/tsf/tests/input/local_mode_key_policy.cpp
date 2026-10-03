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
        check(IsExpressionSpellingSymbol(symbol), "every V symbol is spelled");
    for (wchar_t other : std::wstring(L"=!@#$&,;'aV "))
        check(!IsExpressionSpellingSymbol(other), "other text is not spelled");
    check(!IsExpressionSpellingSymbol(L'\0'), "no text is not spelled");
    check(ClassifyExpressionKey(L'4', L'4') == ExpressionKey::Input, "digit composes");
    check(ClassifyExpressionKey(0x64, L'4') == ExpressionKey::Input, "numpad digit composes");
    check(ClassifyExpressionKey(0xBD, L'-') == ExpressionKey::Input, "minus composes instead of paging");
    check(ClassifyExpressionKey(0xBB, L'+') == ExpressionKey::Input, "Shift+= plus composes");
    check(ClassifyExpressionKey(L'8', L'*') == ExpressionKey::Input, "Shift+8 star composes, not the eighth row");
    check(ClassifyExpressionKey(L'9', L'(') == ExpressionKey::Input, "Shift+9 paren composes");
    check(ClassifyExpressionKey(L'5', L'%') == ExpressionKey::Input, "Shift+5 percent composes");
    check(ClassifyExpressionKey(0xBE, L'.') == ExpressionKey::Input, "period composes instead of paging");
    check(ClassifyExpressionKey(0xBF, L'/') == ExpressionKey::Input, "slash composes instead of punctuation");
    // A digit key printing anything else selects its row, with Shift on a US layout or bare on one whose digit row needs Shift.
    check(ClassifyExpressionKey(L'1', L'!') == ExpressionKey::SelectByNumber, "Shift+1 selects");
    check(ClassifyExpressionKey(L'2', L'@') == ExpressionKey::SelectByNumber, "Shift+2 selects");
    check(ClassifyExpressionKey(L'1', L'&') == ExpressionKey::SelectByNumber, "AZERTY bare 1 selects");
    // Everything else keeps its usual route.
    check(ClassifyExpressionKey(L'0', L')') == ExpressionKey::Input, "Shift+0 paren composes");
    check(ClassifyExpressionKey(L'0', 0x00E0) == ExpressionKey::Unclaimed, "zero key printing a letter is not claimed");
    check(ClassifyExpressionKey(0xBB, L'=') == ExpressionKey::Unclaimed, "equals keeps paging");
    check(ClassifyExpressionKey(0xBC, L',') == ExpressionKey::Unclaimed, "comma keeps paging");
    check(ClassifyExpressionKey(L'A', L'a') == ExpressionKey::Unclaimed, "letters keep their route");

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

    // 网址组字：缓冲以触发词加触发键开头。
    for (const wchar_t *url : {L"www.", L"www.example.com", L"http:", L"https://a.b/c?d=1", L"ftp.x", L"ftp://x"}) {
        const std::wstring raw = url;
        check(IsUrlModeComposition(raw.c_str(), raw.size(), true), "a buffer that starts with a trigger and its key is a URL");
        check(!IsUrlModeComposition(raw.c_str(), raw.size(), false), "not in a scheme that does not detect URLs");
    }
    for (const wchar_t *other : {L"www", L"https", L"wwwbaidu", L"ww.", L"Www.", L"www:", L"http.", L"nihao", L"V1+2", L"U4e2d", L""}) {
        const std::wstring raw = other;
        check(!IsUrlModeComposition(raw.c_str(), raw.size(), true), "other buffers are not URLs");
    }
    check(!IsUrlModeComposition(nullptr, 4, true), "no buffer is no URL");

    // 网址的数字和符号是输入，包括 Shift 打出的符号和在别处翻页的键。
    for (wchar_t symbol : std::wstring(L"0123456789-._~:/?#[]@!$&'()*+,;=%^"))
        check(IsUrlSpellingSymbol(symbol), "every URL symbol is spelled");
    for (wchar_t other : std::wstring(L"\"<>\\{}|` a"))
        check(!IsUrlSpellingSymbol(other), "URL enders are not spelled");
    check(!IsUrlSpellingSymbol(L'\0'), "no text is not spelled in a URL");
    check(ClassifyUrlKey(L'1', L'1') == ExpressionKey::Input, "digit is URL input, not a pick");
    check(ClassifyUrlKey(L'0', L'0') == ExpressionKey::Input, "zero is URL input");
    check(ClassifyUrlKey(0x61, L'1') == ExpressionKey::Input, "numpad digit is URL input");
    check(ClassifyUrlKey(L'1', L'!') == ExpressionKey::Input, "Shift+1 bang is URL input");
    check(ClassifyUrlKey(L'2', L'@') == ExpressionKey::Input, "Shift+2 at is URL input");
    check(ClassifyUrlKey(L'3', L'#') == ExpressionKey::Input, "Shift+3 hash is URL input");
    check(ClassifyUrlKey(0xBE, L'.') == ExpressionKey::Input, "period is URL input instead of paging");
    check(ClassifyUrlKey(0xBC, L',') == ExpressionKey::Input, "comma is URL input instead of paging");
    check(ClassifyUrlKey(0xBD, L'-') == ExpressionKey::Input, "minus is URL input instead of paging");
    check(ClassifyUrlKey(0xBB, L'=') == ExpressionKey::Input, "equals is URL input instead of paging");
    check(ClassifyUrlKey(0xDB, L'[') == ExpressionKey::Input, "bracket is URL input instead of paging");
    check(ClassifyUrlKey(0xBF, L'/') == ExpressionKey::Input, "slash is URL input instead of punctuation");
    check(ClassifyUrlKey(0xBA, L':') == ExpressionKey::Input, "colon is URL input");
    check(ClassifyUrlKey(L'2', 0x00E9) == ExpressionKey::SelectByNumber, "AZERTY bare 2 printing e-acute selects");
    check(ClassifyUrlKey(0xBC, L'<') == ExpressionKey::Unclaimed, "Shift+comma keeps the punctuation route");
    check(ClassifyUrlKey(0xDC, L'\\') == ExpressionKey::Unclaimed, "backslash keeps the punctuation route");
    check(ClassifyUrlKey(0xDB, L'{') == ExpressionKey::Unclaimed, "brace keeps the punctuation route");
    check(ClassifyUrlKey(L'A', L'a') == ExpressionKey::Unclaimed, "letters keep their route");
    check(ClassifyUrlKey(0x20, L' ') == ExpressionKey::Unclaimed, "space keeps selecting");

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

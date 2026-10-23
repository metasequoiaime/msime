#include "../punctuation/policy.h"
#include <iostream>

int main()
{
    using namespace metasequoia::punctuation_contract;
    if (version != 4 || simple.size() != 15 || alternating.size() != 4 || corner_quotes.size() != 4 ||
        hollow_brackets.size() != 2 || enumeration_comma.size() != 2)
        return 1;
    if (!is_supported(',') || !is_supported('"') || !is_supported('<') || !is_supported('_') || is_supported('a'))
        return 1;
    // 第 4 版：`/` 只在 enumerationComma 段里，它的无条件映射由偏好开关决定，is_supported 与 simple 都不含它。
    if (is_supported('/') || simple_output('/') != nullptr)
        return 1;
    const auto *yen = simple_output('$');
    if (!yen || std::string_view(yen) != "￥" || simple_output('a') != nullptr)
        return 1;
    const auto *quotes = alternating_mapping('"');
    if (!quotes || std::string_view(quotes->opening) != "“" || std::string_view(quotes->closing) != "”")
        return 1;
    // 第 3 版起 `{` `}` 出直角引号：弯引号之外「」『』也能直接打，两组字两种样式下相同。
    const auto *brace_single = alternating_mapping('{');
    if (!brace_single || std::string_view(brace_single->opening) != "「" ||
        std::string_view(brace_single->closing) != "」")
        return 1;
    const auto *brace_double = alternating_mapping('}');
    if (!brace_double || std::string_view(brace_double->opening) != "『" ||
        std::string_view(brace_double->closing) != "』")
        return 1;
    // 直角引号和空心括号是同一个键的另一组字：键都在原表里，原表的字不变。
    const auto *corner = corner_quote_mapping('\'');
    if (!corner || std::string_view(corner->opening) != "「" || std::string_view(corner->closing) != "」")
        return 1;
    const auto *corner_double = corner_quote_mapping('"');
    if (!corner_double || std::string_view(corner_double->opening) != "『" || corner_quote_mapping(',') != nullptr)
        return 1;
    const auto *hollow = hollow_bracket_output('[');
    if (!hollow || std::string_view(hollow) != "〖" || hollow_bracket_output('(') != nullptr ||
        std::string_view(simple_output('[')) != "【")
        return 1;
    for (const auto &mapping : corner_quotes)
        if (!alternating_mapping(mapping.input))
            return 1;
    for (const auto &mapping : hollow_brackets)
        if (!simple_output(mapping.input))
            return 1;
    // 顿号键：`\` 同时在 simple（无条件）和 enumerationComma（可抑制）里，`/` 只在 enumerationComma 里。
    const auto *comma_backslash = enumeration_comma_output('\\');
    const auto *comma_slash = enumeration_comma_output('/');
    if (!comma_backslash || std::string_view(comma_backslash) != "、" || !comma_slash ||
        std::string_view(comma_slash) != "、" || enumeration_comma_output('a') != nullptr ||
        std::string_view(simple_output('\\')) != "、")
        return 1;
    if (nested_opening_input != '<' || nested_closing_input != '>' || std::string_view(nested_opening_inner) != "〈")
        return 1;
    std::cout << "Punctuation contract passed\n";
    return 0;
}

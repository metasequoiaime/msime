#pragma once

#include <array>
#include <string_view>

namespace msime::linux_host {

// What a helpcode scheme is called, wherever the user can read it.
//
// These are the schemes' own names, not descriptions, so each has exactly one right spelling and
// it is the reference's: 蓝天小雨点, 自然码, 首右2.0, 首右plus, 小鹤, 加加. Both front ends here had
// written 首右 as 搜狗. The identifiers are pinyin (`shouyou`), and read as one word rather than as
// the two syllables it is, that is what it turns into - but 搜狗 is a different company's input
// method, so the label named a product that has nothing to do with the scheme it switched to.
//
// The status bar and the IBus property list are two menus over one setting and were carrying two
// copies of this table; the settings page, which is shared with every other host, carried a third
// and correct one. This is the Linux copy, and `scripts/test-helpcode-schema-labels.py` is what
// keeps every copy saying what the reference says.
struct HelpcodeSchemaName
{
    const char *value;
    const char *label;
};

inline constexpr std::array<HelpcodeSchemaName, 7> kHelpcodeSchemaNames{{
    {"lantian", "蓝天小雨点"},
    {"ziranma", "自然码"},
    {"shouyou2_0", "首右2.0"},
    {"shouyouplus", "首右plus"},
    {"xiaohe", "小鹤"},
    {"jiajia", "加加"},
    // 五笔 86 是本项目自己加的方案，参考实现没有它，名字以共享设置页为准。
    {"wubi86", "五笔 86"},
}};

// The label for one scheme. An identifier this does not know is the default scheme's, which is what
// the preference falls back to when it reads one it does not know.
inline std::string_view helpcode_schema_label(std::string_view schema)
{
    for (const auto &name : kHelpcodeSchemaNames)
        if (schema == name.value)
            return name.label;
    return kHelpcodeSchemaNames.front().label;
}

} // namespace msime::linux_host

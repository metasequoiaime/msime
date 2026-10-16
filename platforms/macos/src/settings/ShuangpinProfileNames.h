#pragma once
// Display names from MSIME-Apple b637828e15eafcb5e459edd270a962dd14517285.
#include <array>
#include <string_view>
namespace msime::mac {
inline constexpr std::array<const char *, 5> kShuangpinSchemaIdentifiers = {"xiaohe", "ziranma", "shoudao",
                                                                            "microsoft", "smart_abc"};

inline const char *NormalizeShuangpinSchema(std::string_view name)
{
    for (const char *identifier : kShuangpinSchemaIdentifiers)
    {
        if (name == identifier)
        {
            return identifier;
        }
    }
    return kShuangpinSchemaIdentifiers.front();
}

inline const char *ShuangpinSchemaTitle(std::string_view name)
{
    const std::string_view identifier = NormalizeShuangpinSchema(name);
    if (identifier == "ziranma")
    {
        return "自然码双拼";
    }
    if (identifier == "shoudao")
    {
        return "首道双拼";
    }
    if (identifier == "microsoft")
    {
        return "微软双拼";
    }
    if (identifier == "smart_abc")
    {
        return "智能 ABC 双拼";
    }
    return "小鹤双拼";
}

}

#pragma once

#include <nlohmann/json.hpp>

namespace msime::linux_host {

// 托盘、状态栏菜单里切换「智能标点」，与共享设置页的 smartPunctuationPatch（packages/ui/src/settings/punctuation-preferences.ts）同一约定：总开关连同数字后直出、字母后直出一起写成同一个值。两半才是智能标点真正起作用的部分（client-core 的 punctuation::route 要其中之一为真才走 ASCII），设置页关掉总开关时会把两半一起写成假，菜单若只把总开关打开，就成了「总开关开、两半都关」，看着开着却不起作用。只在用户切换总开关时调用，单独调过的一半在别处不会被冲掉。
inline void apply_smart_punctuation_choice(nlohmann::json &preferences, bool enabled)
{
    preferences["smart_punctuation"] = enabled;
    preferences["smart_punctuation_direct_digit"] = enabled;
    preferences["smart_punctuation_direct_letter"] = enabled;
}

} // namespace msime::linux_host

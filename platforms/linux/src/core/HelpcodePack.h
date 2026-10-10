#pragma once

#include "HelpcodeDefaults.h"
#include "HelpcodeSchemaNames.h"

#include <nlohmann/json.hpp>

#include <string>
#include <string_view>

namespace msime::linux_host {

// 全拼和双拼各自可以选一个已安装的辅助码表包（偏好里的 `plugins.helpcode_pack_<方案>`）。选了包时 Engine 按包的码表出辅助码，`<方案>_helpcode.schema` 只在包载入失败时才作为回退（crates/host-api/src/plugin_tables.rs），所以 IBus 属性菜单和 Fcitx5 状态栏都要先看这个键：此前两处都只读 schema，选了包之后菜单仍显示一个并不生效的内置方案，在菜单里换方案也改变不了候选。
inline std::string helpcode_pack_key(std::string_view scheme)
{
    return "helpcode_pack_" + std::string(scheme);
}

// 这个方案当前选中的辅助码表包，没选时为空。只有全拼和双拼有辅助码表包。
inline std::string helpcode_pack(const nlohmann::json &preferences, std::string_view scheme)
{
    if ((scheme != "quanpin" && scheme != "shuangpin") || !preferences.is_object())
        return {};
    const auto plugins = preferences.find("plugins");
    if (plugins == preferences.end() || !plugins->is_object())
        return {};
    const auto pack = plugins->find(helpcode_pack_key(scheme));
    return pack != plugins->end() && pack->is_string() ? pack->get<std::string>() : std::string{};
}

// 偏好文档里空字符串与缺省同义（序列化时省略空值，没有任何插件设置时连 `plugins` 也省略），所以清除时直接删键，不为此新建一个 `plugins` 对象。
inline void set_helpcode_pack(nlohmann::json &preferences, std::string_view scheme, const std::string &pack)
{
    if (!pack.empty())
    {
        preferences["plugins"][helpcode_pack_key(scheme)] = pack;
        return;
    }
    const auto plugins = preferences.find("plugins");
    if (plugins != preferences.end() && plugins->is_object())
        plugins->erase(helpcode_pack_key(scheme));
}

// 在菜单里选内置方案，与共享设置页（packages/ui/src/settings/pages/helpcode-page.tsx）同一约定：写入方案的同时停用这个方案的辅助码表插件。只写方案的话，插件继续盖过刚选的方案，菜单换了而候选不变，也就没有办法从托盘切回内置方案。
inline void apply_helpcode_schema_choice(nlohmann::json &preferences, std::string_view scheme, const std::string &schema)
{
    preferences[std::string(scheme) + "_helpcode"]["schema"] = schema;
    set_helpcode_pack(preferences, scheme, {});
}

// 状态栏上辅助码方案那一项的文字：选了辅助码表包时写明是插件在生效，否则是内置方案的名字。
inline std::string helpcode_status_label(const nlohmann::json &preferences, std::string_view scheme)
{
    if (const auto pack = helpcode_pack(preferences, scheme); !pack.empty())
        return "辅助码：插件 " + pack;
    const auto schema = preferences.value(std::string(scheme) + "_helpcode", nlohmann::json::object())
                            .value("schema", std::string(default_helpcode_schema(scheme)));
    return "辅助码：" + std::string(helpcode_schema_label(schema));
}

} // namespace msime::linux_host

#include "../../src/core/HelpcodeSchemaNames.h"

#include <cassert>
#include <set>
#include <string_view>

using msime::linux_host::helpcode_schema_label;
using msime::linux_host::kHelpcodeSchemaNames;

int main()
{
    // The names are the schemes' own. This is the whole reason the table exists in one place: both
    // menus used to spell 首右 as 搜狗, which is a different company's input method.
    assert(helpcode_schema_label("shouyou2_0") == "首右2.0");
    assert(helpcode_schema_label("shouyouplus") == "首右plus");
    assert(helpcode_schema_label("lantian") == "蓝天小雨点");
    assert(helpcode_schema_label("ziranma") == "自然码");
    assert(helpcode_schema_label("xiaohe") == "小鹤");
    assert(helpcode_schema_label("jiajia") == "加加");
    assert(helpcode_schema_label("wubi86") == "五笔 86");

    // An identifier the table does not know is the default scheme's, which is where the preference
    // lands when it reads one it does not know. A menu entry with no text would be worse.
    assert(helpcode_schema_label("") == "蓝天小雨点");
    assert(helpcode_schema_label("sogou") == "蓝天小雨点");

    // 七套方案，名字和标识都不能重复：IBus 的属性列表按标识区分单选项、显示的是名字，任一列重复都会出现两项分不清的条目。
    static_assert(kHelpcodeSchemaNames.size() == 7);
    std::set<std::string_view> values;
    std::set<std::string_view> labels;
    for (const auto &name : kHelpcodeSchemaNames)
    {
        values.insert(name.value);
        labels.insert(name.label);
    }
    assert(values.size() == kHelpcodeSchemaNames.size());
    assert(labels.size() == kHelpcodeSchemaNames.size());

    return 0;
}

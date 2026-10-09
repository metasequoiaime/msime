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

    // Seven schemes, and no two of them share a name or an identifier - the IBus property list keys
    // its radio items by identifier and shows the label, so a duplicate in either column would make
    // two entries that cannot be told apart.
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

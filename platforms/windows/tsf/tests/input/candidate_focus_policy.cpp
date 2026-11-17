#include <stdexcept>

#include "../../Thread/CandidateFocusPolicy.h"

namespace
{
void require(bool condition, const char *message)
{
    if (!condition)
    {
        throw std::runtime_error(message);
    }
}
} // namespace

int main()
{
    using msime::windows::tsf::candidate_belongs_to_focus;

    require(candidate_belongs_to_focus(true, true, true), "同一焦点文档身份应保留候选");
    require(!candidate_belongs_to_focus(true, true, false),
            "不同文档身份必须隐藏候选");
    require(!candidate_belongs_to_focus(false, true, true), "缺少候选文档不能声明拥有焦点");
    require(!candidate_belongs_to_focus(true, false, true), "缺少焦点文档不能保留候选");
    return 0;
}

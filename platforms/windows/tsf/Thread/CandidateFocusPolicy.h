#pragma once

namespace msime::windows::tsf {

// 候选所属文档只有在两边都存在、且 COM 身份相同才算仍然聚焦。
constexpr bool candidate_belongs_to_focus(bool has_candidate_document,
                                           bool has_focused_document,
                                           bool same_com_object) noexcept {
    return has_candidate_document && has_focused_document && same_com_object;
}

} // namespace msime::windows::tsf

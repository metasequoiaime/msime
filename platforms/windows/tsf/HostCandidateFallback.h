#pragma once

#include "EngineSessionAdapter.h"
#include "../common/InputSchemeTraits.h"
#include <string_view>

namespace msime::tsf
{
// 普通候选方案没有候选时仍保留原文回退项；韩文、注音、越南文和藏文由行内组字，不显示候选窗。
inline std::string_view HostCandidateFallback(const EngineView &view)
{
    if (!view.candidates.empty() || view.editing_text.empty() || view.local_mode != "none" || view.prediction ||
        msime::windows::scheme::AlwaysInlinePreedit(static_cast<int>(view.scheme)))
        return {};
    return view.editing_text;
}
} // namespace msime::tsf

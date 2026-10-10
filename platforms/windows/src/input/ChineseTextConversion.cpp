#include "ChineseTextConversion.h"

#include "../../../common/ChineseTextConversion.h"

#include <utility>

namespace msime::windows {

std::string simplified_to_traditional(std::string_view text,
                                      bool traditional_output) {
  if (!traditional_output || text.empty())
    return std::string(text);
  // The shared OpenCC s2t tables, the same phrase-level conversion the reference server ships. A
  // character table cannot tell 头发 (頭髮) from 发展 (發展); LCMapStringEx was one.
  auto converted = msime::host_api::simplified_to_traditional(text);
  return converted ? std::move(*converted) : std::string(text);
}

} // namespace msime::windows

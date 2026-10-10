#include "ChineseTextConversion.h"

#include "../../../common/ChineseTextConversion.h"

#include <utility>

std::string msime_linux_simplified_to_traditional(const std::string &text) {
  if (text.empty())
    return text;
  // The shared OpenCC s2t tables, the same conversion Windows uses; NULL means invalid UTF-8 or an embedded NUL, where the original text is kept.
  auto converted = msime::host_api::simplified_to_traditional(text);
  return converted ? std::move(*converted) : text;
}

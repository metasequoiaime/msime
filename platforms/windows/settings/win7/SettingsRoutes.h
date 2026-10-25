#pragma once

#include <optional>
#include <string_view>

namespace msime::settings::win7 {
std::optional<int> settings_page_for_route(std::string_view route);
} // namespace msime::settings::win7

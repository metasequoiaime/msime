#pragma once

#include "DataUrlImage.h"

namespace msime::settings::win7::community_data {

inline theme_data::Image preview(const Json& response) {
  const auto value = theme_data::value(response);
  if (!value || !value->is_object() || !value->contains("dataUrl") || !value->at("dataUrl").is_string()) return {};
  return image_data::decode(value->at("dataUrl").get<std::string>(), 256 * 1024);
}

inline bool draw(const DRAWITEMSTRUCT& draw, const theme_data::Image& image) {
  return image_data::draw(draw, image, L"选择皮肤后读取预览图");
}
} // namespace msime::settings::win7::community_data

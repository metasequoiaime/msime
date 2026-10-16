#pragma once

#include <nlohmann/json.hpp>

#include <cstddef>
#include <cstdint>
#include <string>
#include <string_view>

namespace msime::linux_host {

// 脚本插件注释的长度上限（UTF-16 码元），与 runtime 的 `candidates_annotate` 相同。
inline constexpr std::size_t kScriptAnnotationMaxUtf16 = 32;

// 会把候选行折行、把周围文字的方向翻过来或插进看不见的宽度的字符：Cc、行和段分隔符、双向控制符、零宽字符和 BOM。
inline bool script_annotation_breaks_layout(std::uint32_t code_point) {
  return code_point < 0x20 || (code_point >= 0x7f && code_point <= 0x9f) ||
         code_point == 0x061c || code_point == 0x180e ||
         (code_point >= 0x200b && code_point <= 0x200f) ||
         (code_point >= 0x2028 && code_point <= 0x202e) ||
         (code_point >= 0x2060 && code_point <= 0x2064) ||
         (code_point >= 0x2066 && code_point <= 0x206f) || code_point == 0xfeff ||
         (code_point >= 0xfff9 && code_point <= 0xfffb);
}

// 宿主这一侧对插件文本的检查：合法 UTF-8、不超过 32 个 UTF-16 码元、不含上面那些字符。runtime 已经按完整的 Cf 表拦过一遍（`abi::is_forbidden_plugin_char`），这里防的是契约走样时插件文本把候选行折断或把整行的方向翻过来；不合规就整条不画，不截断。
inline bool script_annotation_text_allowed(std::string_view text) {
  std::size_t units = 0;
  for (std::size_t offset = 0; offset < text.size();) {
    const auto first = static_cast<std::uint8_t>(text[offset]);
    std::size_t width = 0;
    std::uint32_t code_point = 0;
    if (first < 0x80) {
      width = 1;
      code_point = first;
    } else if (first >= 0xc2 && first <= 0xdf) {
      width = 2;
      code_point = first & 0x1f;
    } else if (first >= 0xe0 && first <= 0xef) {
      width = 3;
      code_point = first & 0x0f;
    } else if (first >= 0xf0 && first <= 0xf4) {
      width = 4;
      code_point = first & 0x07;
    } else {
      return false;
    }
    if (offset + width > text.size()) return false;
    for (std::size_t index = 1; index < width; ++index) {
      const auto byte = static_cast<std::uint8_t>(text[offset + index]);
      if ((byte & 0xc0) != 0x80) return false;
      code_point = (code_point << 6) | (byte & 0x3f);
    }
    if ((width == 2 && code_point < 0x80) || (width == 3 && code_point < 0x800) ||
        (width == 4 && code_point < 0x10000) || code_point > 0x10ffff ||
        (code_point >= 0xd800 && code_point <= 0xdfff))
      return false;
    if (script_annotation_breaks_layout(code_point)) return false;
    units += code_point >= 0x10000 ? 2 : 1;
    if (units > kScriptAnnotationMaxUtf16) return false;
    offset += width;
  }
  return true;
}

// 候选行上脚本插件的注释，即视图里候选的 `script_annotation`（只有授权过的脚本插件给这一行加了注释时才出现）；没有、不是字符串、为空或不合规时返回空串。它和引擎的 `annotation` 是两回事：引擎注释按辅助码开关和本地模式决定画不画，韩文汉字列表还把它当训音读，插件注释不走那条路，两个宿主都在行尾另画一段，不替换引擎注释。AI 占位行（来源 12，共享内核已经不再产出）不是候选，不画。
inline std::string candidate_script_annotation(const nlohmann::json &candidate) {
  if (!candidate.is_object()) return {};
  const auto source = candidate.find("source");
  if (source != candidate.end() && source->is_number_integer() && source->get<std::int64_t>() == 12)
    return {};
  const auto annotation = candidate.find("script_annotation");
  if (annotation == candidate.end() || !annotation->is_string()) return {};
  const auto &text = annotation->get_ref<const std::string &>();
  if (text.empty() || !script_annotation_text_allowed(text)) return {};
  return text;
}

// 追加在候选行最后的一段：两个空格隔开，再用六角括号括起。引擎注释是两个空格后直接跟文本，括号让插件注释在不认颜色和字形属性的面板上也分得出来。
inline std::string candidate_script_annotation_suffix(const std::string &annotation) {
  if (annotation.empty()) return {};
  return "  〔" + annotation + "〕";
}

} // namespace msime::linux_host

#pragma once

#include "PhrasePreedit.h"

#include <nlohmann/json.hpp>

#include <cstddef>
#include <cstdint>
#include <string>
#include <string_view>

namespace msime::linux_host {

// 光标后编辑（偏好 `caret_edit_suffix`）生效时，两个宿主在候选窗的辅助行上显示的提示。只是提示，不能点，也不提供切换。
inline constexpr std::string_view kCaretEditSuffixHint = "候选编辑：光标后";

// 视图里光标后编辑的两个字段（crates/input-runtime/src/types.rs 的 `View`）：`caret_edit_suffix` 只在生效时出现且为真，`caret_edit_prefix` 只在生效且非空时出现，是首选（`id.index` 为 0 的那一行）开头固定的那段文字。字段缺失、类型不对或视图不是对象都按没有生效处理，不抛异常；没有生效时 `prefix` 一律为空，宿主不会按一份过时的前缀去画淡。
struct CaretEditSuffix {
  bool active = false;
  std::string prefix;
};

inline CaretEditSuffix caret_edit_suffix(const nlohmann::json &view) {
  CaretEditSuffix result;
  if (!view.is_object()) return result;
  const auto active = view.find("caret_edit_suffix");
  if (active == view.end() || !active->is_boolean() || !active->get<bool>()) return result;
  result.active = true;
  const auto prefix = view.find("caret_edit_prefix");
  if (prefix != view.end() && prefix->is_string()) result.prefix = prefix->get<std::string>();
  return result;
}

// 把提示作为辅助行的一段接上去，分隔符用宿主自己辅助行的那一种（IBus 是「  · 」，Fcitx5 是「 · 」）。两个宿主都在打字连击之前调它：提示在生效期间一直在，连击数随每次按键变宽变窄，排在最后才不会把提示推来推去。没有生效时什么也不加。
inline void append_caret_edit_suffix_hint(std::string &aux, const CaretEditSuffix &mode, std::string_view separator) {
  if (!mode.active) return;
  if (!aux.empty()) aux += separator;
  aux += kCaretEditSuffixHint;
}

// 这一行候选开头要画淡的字数，按 Unicode 标量计（IBus 的属性区间就是这个单位）。只有生效、前缀非空、这一行是首选（`id.index` 为 0，第 0 页第一行），并且它的原文确实以前缀开头时才画淡；其余一律为 0。核对原文是因为宿主会把别的行冒充成候选画进同一个面板，例如 IBus 的临时译文页沿用保存的视图、把第一条译文编成 0 号，那一行开头并不是这段前缀。
inline std::size_t caret_edit_dim_scalars(const CaretEditSuffix &mode, const nlohmann::json &candidate) {
  if (!mode.active || mode.prefix.empty() || !candidate.is_object()) return 0;
  const auto id = candidate.find("id");
  if (id == candidate.end() || !id->is_object()) return 0;
  const auto index = id->find("index");
  if (index == id->end() || !index->is_number_integer()) return 0;
  // 解析出来的非负整数是无符号的，宿主自己拼的候选里是有符号的，两种都认。
  if (index->is_number_unsigned() ? index->get<std::uint64_t>() != 0 : index->get<std::int64_t>() != 0) return 0;
  const auto text = candidate.find("text");
  if (text == candidate.end() || !text->is_string()) return 0;
  if (text->get_ref<const std::string &>().compare(0, mode.prefix.size(), mode.prefix) != 0) return 0;
  return utf8_scalar_count(mode.prefix);
}

// 显示文字开头 `scalars` 个字占多少字节，超过文字长度时取整段（Fcitx5 和 Pango 的区间按字节计）。宿主画的是简繁转换以后的文字，字数通常不变；万一变短了，画淡的范围也不会越出这一行。
inline std::size_t utf8_scalar_prefix_bytes(std::string_view text, std::size_t scalars) {
  std::size_t bytes = 0;
  for (std::size_t counted = 0; bytes < text.size() && counted < scalars; ++counted) {
    ++bytes;
    while (bytes < text.size() && (static_cast<unsigned char>(text[bytes]) & 0xC0u) == 0x80u) ++bytes;
  }
  return bytes;
}

} // namespace msime::linux_host

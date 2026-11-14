#pragma once

#include "SentencePackModel.h"
#include <array>
#include <string_view>

namespace msime::settings::scheme_resource {

struct Pack {
  std::wstring_view id;
  std::wstring_view title;
  std::array<std::wstring_view, 3> files;
};

inline constexpr std::array<Pack, 3> packs{{
    {L"wubi", L"五笔码表", {L"msime-wubi.db", L"", L""}},
    {L"japanese", L"日文词典", {L"msime-japanese.dat", L"msime-mozc_dictionary_oss_README.txt", L"msime-mozc_LICENSE.txt"}},
    {L"language-dictionaries", L"语言词库", {L"msime-cantonese.db", L"msime-zhuyin.db", L"msime-stroke.db"}},
}};

constexpr Pack const *find(std::wstring_view id) {
  for (auto const &pack : packs)
    if (pack.id == id) return &pack;
  return nullptr;
}

constexpr Pack const *for_scheme(std::wstring_view scheme) {
  if (scheme == L"wubi" || scheme == L"japanese") return find(scheme);
  if (scheme == L"cantonese" || scheme == L"zhuyin" || scheme == L"stroke")
    return find(L"language-dictionaries");
  return nullptr;
}

// 只把完整随包的一组视为内置；不把一个旧文件当成整组可用。
template <typename Exists>
bool bundled(Pack const &pack, Exists exists) {
  for (auto const name : pack.files)
    if (!name.empty() && !exists(name)) return false;
  return true;
}

constexpr sentence_pack::RowView row_view(
    std::wstring_view pack, sentence_pack::Knowledge knowledge, std::wstring_view state,
    bool installing, bool importing, bool removing, bool update_available) {
  auto view = sentence_pack::row_view(knowledge, state, installing, importing, removing, update_available);
  if (!find(pack)) return view;
  if (installing) view.subtitle = importing ? L"正在导入和校验词库…" : L"正在下载和校验词库…";
  else if (removing) view.subtitle = L"正在删除词库…";
  else if (knowledge == sentence_pack::Knowledge::unknown) view.subtitle = L"正在读取词库状态…";
  else if (knowledge == sentence_pack::Knowledge::failed) view.subtitle = L"无法读取词库状态，请重试。";
  else if (state == L"bundled") {
    view = {};
    view.subtitle = L"随安装包提供。";
  } else if (state == L"missing") view.subtitle = L"词库尚未安装，下载后切换一次输入框生效。";
  return view;
}

} // namespace msime::settings::scheme_resource

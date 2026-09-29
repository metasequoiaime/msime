#pragma once
#include <cctype>
#include <cmath>
#include <cstddef>
#include <nlohmann/json.hpp>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

namespace msime::linux_host {

// 一款已安装的外部候选皮肤：id 是身份，title 只用于展示，base 是它的清单声明的底色主题（system 或某个内置主题）。选中它就是选中自定义主题，并把 custom_theme.base 换成这个 base，和设置页的皮肤卡片一样。
struct CandidateSkin {
  std::string id;
  std::string title;
  std::string base;
};

// 外部皮肤 id 的可用字符，与共享层 skin::catalog 的 safe_id 同一条规则。宿主不放宽
// 它：目录里的 id 会被拼进属性名和菜单项标识。
inline bool safe_skin_id(std::string_view id) {
  if (id.empty() || id.size() > 64) return false;
  const auto first = static_cast<unsigned char>(id.front());
  if (!std::isalnum(first)) return false;
  for (const char value : id) {
    const auto byte = static_cast<unsigned char>(value);
    const bool allowed = (byte >= 'a' && byte <= 'z') || (byte >= '0' && byte <= '9') ||
                         byte == '.' || byte == '_' || byte == '-';
    if (!allowed) return false;
  }
  return true;
}

// 运行配置里 candidate_skin_catalog.packages 的那些外部皮肤。条目原样交给 msime_client_resolve_theme 的 package，而共享层按清单严格读取它（ThemePackage::from_host_catalog_entry），所以这里只收它会接受的条目：安全的 id、非空的 title、字符串 base、只含 horizontal / vertical 的 layouts。base 是否为 system 或某个内置主题由 theme_choices 对照共享层的主题目录再筛一次，宿主不另存主题 id 表。调色板不在这里读——只声明了模式而没有任何颜色的 `{}` 同样是合法条目，由共享层决定画什么。
inline std::vector<CandidateSkin> parse_configured_skins(const nlohmann::json &options) {
  std::vector<CandidateSkin> skins;
  const auto catalog = options.find("candidate_skin_catalog");
  if (catalog == options.end() || !catalog->is_object()) return skins;
  const auto packages = catalog->find("packages");
  if (packages == catalog->end() || !packages->is_array()) return skins;
  for (const auto &package : *packages) {
    if (!package.is_object()) continue;
    const auto id_value = package.find("id");
    const auto title_value = package.find("title");
    const auto base_value = package.find("base");
    const auto layouts = package.find("layouts");
    if (id_value == package.end() || !id_value->is_string() || title_value == package.end() ||
        !title_value->is_string() || base_value == package.end() || !base_value->is_string() ||
        layouts == package.end() || !layouts->is_array())
      continue;
    auto id = id_value->get<std::string>();
    if (!safe_skin_id(id)) continue;
    auto title = title_value->get<std::string>();
    if (title.empty() || title.size() > 128) continue;
    auto base = base_value->get<std::string>();
    if (base.empty() || base == "custom") continue;
    bool layouts_known = true;
    for (const auto &layout : *layouts)
      layouts_known = layouts_known && layout.is_string() &&
                      (layout.get<std::string>() == "horizontal" || layout.get<std::string>() == "vertical");
    if (!layouts_known) continue;
    bool listed = false;
    for (const auto &existing : skins) listed = listed || existing.id == id;
    if (listed) continue;
    skins.push_back({std::move(id), std::move(title), std::move(base)});
  }
  return skins;
}

// The catalogue entry for one installed skin, unchanged, as msime_client_resolve_theme takes it for `package`. Null when the catalogue does not list it.
inline const nlohmann::json *candidate_skin_package(const nlohmann::json &catalog, std::string_view id) {
  if (id.empty() || !catalog.is_object()) return nullptr;
  const auto packages = catalog.find("packages");
  if (packages == catalog.end() || !packages->is_array()) return nullptr;
  for (const auto &package : *packages) {
    if (!package.is_object()) continue;
    const auto value = package.find("id");
    if (value != package.end() && value->is_string() && value->get<std::string>() == id) return &package;
  }
  return nullptr;
}

// The decoration an installed skin draws above its candidate list: an image, trailing-aligned in a band top_dip tall and width_dip wide on Windows (candidate_presenter.cpp). The shared host catalog (skin::catalog::host_candidate_catalog) publishes it only for a package that declares one, with the image as an absolute path inside that package. The bounds are the manifest's (0 < top <= 500, 0 < width <= 1000) and are checked again here, because the document is read as untrusted input. Only Fcitx5 draws it; IBus text attributes have no way to show an image, and IBus never reads these keys.
struct CandidateSkinDecoration {
  std::string image;
  double top_dip = 0;
  double width_dip = 0;
};

// One package's decoration. A package without all three keys, or with any of them out of bounds, has none; that costs the skin its decoration, not its place in the catalogue.
inline std::optional<CandidateSkinDecoration> parse_skin_decoration(const nlohmann::json &package) {
  if (!package.is_object()) return std::nullopt;
  const auto top = package.find("decoration_top_dip");
  const auto width = package.find("decoration_width_dip");
  const auto image = package.find("decoration_image");
  if (top == package.end() || width == package.end() || image == package.end()) return std::nullopt;
  if (!top->is_number() || !width->is_number() || !image->is_string()) return std::nullopt;
  const auto top_dip = top->get<double>();
  const auto width_dip = width->get<double>();
  if (!std::isfinite(top_dip) || !std::isfinite(width_dip) || !(top_dip > 0 && top_dip <= 500) ||
      !(width_dip > 0 && width_dip <= 1000))
    return std::nullopt;
  auto path = image->get<std::string>();
  if (path.empty() || path.size() > 4096 || path.front() != '/' || path.find('\0') != std::string::npos)
    return std::nullopt;
  return CandidateSkinDecoration{std::move(path), top_dip, width_dip};
}

// The decoration of the skin the resolved theme draws: its `candidate_skin`, which the shared layer sets only when a custom theme names an installed package whose manifest declares the layout and the mode being drawn. That is the only gate; the host keeps no layout or mode check of its own. An empty id (nothing drawn) has none.
inline std::optional<CandidateSkinDecoration> candidate_skin_decoration(const nlohmann::json &catalog,
                                                                        std::string_view drawn) {
  const auto *package = candidate_skin_package(catalog, drawn);
  return package ? parse_skin_decoration(*package) : std::nullopt;
}

}  // namespace msime::linux_host

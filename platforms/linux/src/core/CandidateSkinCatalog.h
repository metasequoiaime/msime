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

inline constexpr std::size_t kMaxCandidateSkins = 32;

// 一款已安装的外部候选皮肤：id 是身份，title 只用于展示，base 是它的清单声明的底色主题（system 或某个内置主题）。选中它就是选中自定义主题，并把 custom_theme.base 换成这个 base，和设置页的皮肤卡片一样。
struct CandidateSkin {
  std::string id;
  std::string title;
  std::string base;
};

// 外部皮肤 id 的可用字符，与共享层 text 模块的 safe_id 同一条规则。宿主不放宽它：目录里的 id 会被拼进属性名和菜单项标识。
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
  skins.reserve(kMaxCandidateSkins);
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
    if (skins.size() >= kMaxCandidateSkins) break;
    skins.push_back({std::move(id), std::move(title), std::move(base)});
  }
  return skins;
}

// 自定义主题在 `dark` 模式下取哪个槽位的皮肤包 id（共享层 `CustomTheme::candidate_skin_for`）：深色模式先取 `candidate_skin_dark`，没设时与浅色模式一样取 `candidate_skin`。空串表示这个模式没有皮肤包；空字符串和非字符串的值都按没设处理。
inline std::string candidate_skin_for(const nlohmann::json &custom_theme, bool dark) {
  if (!custom_theme.is_object()) return {};
  const auto slot = [&](const char *key) {
    const auto value = custom_theme.find(key);
    return value != custom_theme.end() && value->is_string() ? value->get<std::string>() : std::string{};
  };
  if (dark)
    if (auto skin = slot("candidate_skin_dark"); !skin.empty()) return skin;
  return slot("candidate_skin");
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

// The decoration an installed skin draws above its candidate list: an image in a band top_dip tall and width_dip wide, placed along the card's top edge as `decoration_align` says (trailing when it says nothing, as Windows' candidate_presenter.cpp always drew it). The shared host catalog (skin::catalog::host_candidate_catalog) publishes it only for a package that declares one, with the image as an absolute path inside that package. The bounds are the manifest's (0 < top <= 500, 0 < width <= 1000) and are checked again here, because the document is read as untrusted input. Only Fcitx5 draws it; IBus text attributes have no way to show an image, and IBus never reads these keys.
enum class CandidateSkinAlign { left, center, right };
struct CandidateSkinDecoration {
  std::string image;
  double top_dip = 0;
  double width_dip = 0;
  CandidateSkinAlign align = CandidateSkinAlign::right;
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
  auto align = CandidateSkinAlign::right;
  if (const auto value = package.find("decoration_align"); value != package.end() && value->is_string()) {
    if (*value == "left") align = CandidateSkinAlign::left;
    else if (*value == "center") align = CandidateSkinAlign::center;
  }
  return CandidateSkinDecoration{std::move(path), top_dip, width_dip, align};
}

// The decoration of the skin the resolved theme draws: its `candidate_skin`, which the shared layer sets only when a custom theme names an installed package whose manifest declares the layout and the mode being drawn. That is the only gate; the host keeps no layout or mode check of its own. An empty id (nothing drawn) has none.
inline std::optional<CandidateSkinDecoration> candidate_skin_decoration(const nlohmann::json &catalog,
                                                                        std::string_view drawn) {
  const auto *package = candidate_skin_package(catalog, drawn);
  return package ? parse_skin_decoration(*package) : std::nullopt;
}

// The card radius the drawn skin asks for (`corner_radius_dip`, 0-32), or none to keep the host's own. Checked again here like the decoration, as the document is untrusted. Only Fcitx5 draws a card; the IBus panel belongs to the desktop.
inline std::optional<double> candidate_skin_corner_radius(const nlohmann::json &catalog, std::string_view drawn) {
  const auto *package = candidate_skin_package(catalog, drawn);
  if (!package) return std::nullopt;
  const auto value = package->find("corner_radius_dip");
  if (value == package->end() || !value->is_number()) return std::nullopt;
  const auto radius = value->get<double>();
  if (!std::isfinite(radius) || radius < 0 || radius > 32) return std::nullopt;
  return radius;
}

// 皮肤包画在卡片底色之上、候选之下的背景图（清单的 `[candidate_window.background]`）：共享目录给出包内图片的绝对路径、`fit`（cover 铺满裁切、contain 完整放进卡片、stretch 两个方向各自拉伸）和 0–1 的不透明度。与装饰图同样按不可信输入再查一遍，任何一项不合规就当作没有背景图。只有 Fcitx5 画它；IBus 的面板画不了图片。
enum class CandidateSkinBackgroundFit { cover, contain, stretch };
struct CandidateSkinBackground {
  std::string image;
  CandidateSkinBackgroundFit fit = CandidateSkinBackgroundFit::cover;
  double opacity = 1.0;
};

inline std::optional<CandidateSkinBackground> candidate_skin_background(const nlohmann::json &catalog,
                                                                        std::string_view drawn) {
  const auto *package = candidate_skin_package(catalog, drawn);
  if (!package) return std::nullopt;
  const auto image = package->find("background_image");
  if (image == package->end() || !image->is_string()) return std::nullopt;
  auto path = image->get<std::string>();
  if (path.empty() || path.size() > 4096 || path.front() != '/' || path.find('\0') != std::string::npos)
    return std::nullopt;
  CandidateSkinBackground background{std::move(path)};
  if (const auto fit = package->find("background_fit"); fit != package->end()) {
    if (*fit == "contain") background.fit = CandidateSkinBackgroundFit::contain;
    else if (*fit == "stretch") background.fit = CandidateSkinBackgroundFit::stretch;
    else if (*fit != "cover") return std::nullopt;
  }
  if (const auto opacity = package->find("background_opacity"); opacity != package->end()) {
    if (!opacity->is_number()) return std::nullopt;
    background.opacity = opacity->get<double>();
    if (!std::isfinite(background.opacity) || background.opacity < 0 || background.opacity > 1) return std::nullopt;
  }
  return background;
}

// 所画皮肤包的行圆角（清单的 `item_corner_radius_dip`，0–16），没有时交给宿主自己的 6 px。共享目录对声明了它的包直接给出这个键；Windows 外观和 canvas 底的包还在 `resolvedLayout.itemCornerRadiusDip` 里带着按出厂外观继承后的值，前者没有时取后者。文档按不可信输入再查一遍范围。macOS 同样按包的行圆角画（SkinManifestBridge.mm）。
inline std::optional<double> candidate_skin_item_radius(const nlohmann::json &catalog, std::string_view drawn) {
  const auto *package = candidate_skin_package(catalog, drawn);
  if (!package) return std::nullopt;
  const auto read = [](const nlohmann::json &object, const char *key) -> std::optional<double> {
    if (!object.is_object()) return std::nullopt;
    const auto value = object.find(key);
    if (value == object.end() || !value->is_number()) return std::nullopt;
    const auto radius = value->get<double>();
    if (!std::isfinite(radius) || radius < 0 || radius > 16) return std::nullopt;
    return radius;
  };
  if (const auto radius = read(*package, "item_corner_radius_dip")) return radius;
  const auto layout = package->find("resolvedLayout");
  return layout != package->end() ? read(*layout, "itemCornerRadiusDip") : std::nullopt;
}

// The card radius the user set in the settings (`candidate_corner_radius`, 0-32), or none to leave it to the skin and the host. The shared layer omits the key when it is unset and rejects values out of range; anything else in the file is ignored here rather than trusted.
inline std::optional<double> candidate_corner_radius_preference(const nlohmann::json &preferences) {
  if (!preferences.is_object()) return std::nullopt;
  const auto value = preferences.find("candidate_corner_radius");
  if (value == preferences.end() || !value->is_number()) return std::nullopt;
  const auto radius = value->get<double>();
  if (!std::isfinite(radius) || radius < 0 || radius > 32) return std::nullopt;
  return radius;
}

// The card radius Fcitx5 draws: the user's own value wins over the drawn skin's, and with neither the host keeps its design radius.
inline std::optional<double> candidate_corner_radius(const nlohmann::json &preferences, const nlohmann::json &catalog,
                                                     std::string_view drawn) {
  if (const auto radius = candidate_corner_radius_preference(preferences)) return radius;
  return candidate_skin_corner_radius(catalog, drawn);
}

}  // namespace msime::linux_host

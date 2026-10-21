#pragma once

#include <cmath>
#include <cstdlib>
#include <optional>
#include <string>
#include <string_view>

namespace msime::linux_host {

// KDE 把「动画速度」拖到最左（即时）时在 kdeglobals 的 [KDE] 组写 AnimationDurationFactor=0；这就是 KDE 的减弱动态效果。返回这份文件是否这样要求，没写这个键时没有结论。
inline std::optional<bool> kde_reduce_motion(std::string_view kdeglobals) {
  bool in_kde = false;
  std::optional<bool> result;
  while (!kdeglobals.empty()) {
    const auto end = kdeglobals.find('\n');
    auto line = kdeglobals.substr(0, end);
    kdeglobals.remove_prefix(end == std::string_view::npos ? kdeglobals.size() : end + 1);
    while (!line.empty() && (line.back() == '\r' || line.back() == ' ' || line.back() == '\t')) line.remove_suffix(1);
    while (!line.empty() && (line.front() == ' ' || line.front() == '\t')) line.remove_prefix(1);
    if (line.empty() || line.front() == '#') continue;
    if (line.front() == '[') {
      in_kde = line == "[KDE]";
      continue;
    }
    if (!in_kde) continue;
    const auto equals = line.find('=');
    if (equals == std::string_view::npos) continue;
    auto key = line.substr(0, equals);
    while (!key.empty() && (key.back() == ' ' || key.back() == '\t')) key.remove_suffix(1);
    if (key != "AnimationDurationFactor") continue;
    const std::string value(line.substr(equals + 1));
    char *parsed = nullptr;
    const double factor = std::strtod(value.c_str(), &parsed);
    if (parsed == value.c_str() || !std::isfinite(factor)) continue;
    result = factor <= 0.0;
  }
  return result;
}

// 两个来源合起来：GNOME 的 enable-animations 关掉，或 KDE 的动画速度设成即时，都算减弱动态效果；都没有结论时照常播放。
inline bool reduce_motion_from(std::optional<bool> gnome_animations_enabled, std::optional<bool> kde_reduce) {
  return (gnome_animations_enabled && !*gnome_animations_enabled) || (kde_reduce && *kde_reduce);
}

} // msime::linux_host 命名空间

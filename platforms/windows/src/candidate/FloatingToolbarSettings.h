#pragma once
#include <array>
#include <cstdint>
#include <mutex>
#include <nlohmann/json.hpp>
#include <optional>
#include <vector>

namespace msime::windows {
struct FloatingToolbarSettings {
  unsigned scale_percent = 100;
  unsigned font_size = 24;
  std::array<bool, 6> items{true, true, true, true, false, true};
  // The shared `english_mode` item. The reference always shows its 中/英 button, so this stays on unless the preference turns it off.
  bool language = true;
  // 共享偏好里另外三个可选按钮，默认值与 client-core 的 FloatingToolbarPreferences 一致：切换输入方案默认开，手写和语音默认关。它们不进 `items`，因为 `items` 的六项也是 PreviewConfig 的 floating_toolbar_items 契约，那里要求正好六个键。
  bool input_scheme = true;
  bool handwriting = false;
  bool voice = false;
  // 顶层偏好 `show_app_logo`：关掉时左端画两列三行圆点的握把而不是 logo，仍然可以拖动。读不到时按新装处理，不画 logo，与 macOS 一致。
  bool show_logo = false;
  bool operator==(const FloatingToolbarSettings &other) const {
    return scale_percent == other.scale_percent && font_size == other.font_size &&
           items == other.items && language == other.language &&
           input_scheme == other.input_scheme && handwriting == other.handwriting &&
           voice == other.voice && show_logo == other.show_logo;
  }
  bool valid() const {
    return scale_percent >= 75 && scale_percent <= 150 &&
           font_size >= 16 && font_size <= 28;
  }
};

inline std::optional<FloatingToolbarSettings>
floating_toolbar_settings(const nlohmann::json &preferences) {
  try {
    if (!preferences.is_object()) return std::nullopt;
    const auto toolbar = preferences.value("floating_toolbar", nlohmann::json::object());
    if (!toolbar.is_object()) return std::nullopt;
    FloatingToolbarSettings result;
    for (const auto &[key, target] :
         {std::pair{"scale_percent", &result.scale_percent},
          std::pair{"font_size", &result.font_size}}) {
      if (!toolbar.contains(key)) continue;
      const auto &value = toolbar.at(key);
      if (!value.is_number_integer() || value < 0 || value > 200)
        return std::nullopt;
      *target = value.get<unsigned>();
    }
    constexpr const char *names[] = {"character_set", "punctuation", "fullwidth",
                                     "emoji", "screen_keyboard", "settings"};
    for (size_t i = 0; i < result.items.size(); ++i)
      result.items[i] = toolbar.value(names[i], result.items[i]);
    result.language = toolbar.value("english_mode", result.language);
    result.input_scheme = toolbar.value("input_scheme", result.input_scheme);
    result.handwriting = toolbar.value("handwriting", result.handwriting);
    result.voice = toolbar.value("voice", result.voice);
    result.show_logo = preferences.value("show_app_logo", result.show_logo);
    return result.valid() ? std::optional{result} : std::nullopt;
  } catch (...) {
    return std::nullopt;
  }
}

// 工具栏按顺序画的按钮：0 中/英，11 切换输入方案，1 全角，2 标点，3 简繁，4 表情，7 手写，5 屏幕键盘，8 语音，6 设置。`items` 按偏好的顺序排列：character_set、punctuation、fullwidth、emoji、screen_keyboard、settings。`handwriting_offered` 是这个版本有没有手写：不提供手写的版本（日文、越南文和藏文版）不画手写按钮，从别处同步来的开关也不算，与 macOS 一致。
inline std::vector<int> floating_toolbar_slots(const FloatingToolbarSettings &settings,
                                               bool handwriting_offered = true) {
  const auto &items = settings.items;
  std::vector<int> result;
  result.reserve(10);
  if (settings.language) result.push_back(0);
  if (settings.input_scheme) result.push_back(11);
  if (items[2]) result.push_back(1);
  if (items[1]) result.push_back(2);
  if (items[0]) result.push_back(3);
  if (items[3]) result.push_back(4);
  if (settings.handwriting && handwriting_offered) result.push_back(7);
  if (items[4]) result.push_back(5);
  if (settings.voice) result.push_back(8);
  if (items[5]) result.push_back(6);
  return result;
}

// Only presentation settings cross threads. Keep the newest revision even
// after consumption, so a delayed publisher cannot restore an older layout.
class FloatingToolbarMailbox {
public:
  bool publish(uint64_t revision, FloatingToolbarSettings value) {
    if (!value.valid()) return false;
    std::lock_guard lock(mutex_);
    if (revision_ && revision <= *revision_) return false;
    revision_ = revision;
    pending_ = value;
    return true;
  }
  std::optional<FloatingToolbarSettings> take() {
    std::lock_guard lock(mutex_);
    const auto result = pending_;
    pending_.reset();
    return result;
  }
private:
  std::mutex mutex_;
  std::optional<uint64_t> revision_;
  std::optional<FloatingToolbarSettings> pending_;
};
} // namespace msime::windows

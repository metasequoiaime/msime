#pragma once

#include "HostApiString.h"
#include "msime_client.h"
#include <nlohmann/json.hpp>
#include <string>
#include <string_view>

namespace msime::host_api {
// 只消费 Rust 已解码的规范主键和修饰键；别名、语法和冲突都由共享层决定。
struct ToggleHotkeyChord {
  std::string key;
  bool ctrl = false;
  bool alt = false;
  bool shift = false;
  bool meta = false;

  bool matches(std::string_view pressed, bool control, bool option, bool shifted,
               bool logo) const {
    return !key.empty() && key == pressed && ctrl == control && alt == option &&
           shift == shifted && meta == logo;
  }
};

inline ToggleHotkeyChord decode_toggle_hotkey(const nlohmann::json &preferences,
                                               std::string_view slot) {
  const auto request = nlohmann::json{{"keybindings", preferences.value("keybindings", nlohmann::json::object())}}.dump();
  auto raw = own_string(msime_client_toggle_hotkey_chords(
      reinterpret_cast<const uint8_t *>(request.data()), request.size()));
  if (!raw) return {};
  const auto response = nlohmann::json::parse(raw.get(), nullptr, false);
  if (response.is_discarded() || !response.value("ok", false)) return {};
  const auto &value = response.at("value");
  const auto item = value.find(std::string(slot));
  if (item == value.end() || !item->is_object()) return {};
  return {item->value("key", std::string()), item->value("ctrl", false),
          item->value("alt", false), item->value("shift", false),
          item->value("meta", false)};
}
} // namespace msime::host_api

#pragma once

#include <nlohmann/json.hpp>
#include <optional>

namespace msime::linux_host {

// The shared "mouse wheel pages candidates" switch (navigation.mouse_wheel, off by default).
inline bool read_candidate_wheel_paging(const nlohmann::json &preferences) {
  if (!preferences.is_object()) return false;
  const auto navigation = preferences.find("navigation");
  if (navigation == preferences.end() || !navigation->is_object()) return false;
  const auto value = navigation->find("mouse_wheel");
  return value != navigation->end() && value->is_boolean() && value->get<bool>();
}

// Fcitx5's classic UI turns the wheel over its candidate list into page requests itself, through the same prev/next calls as its page buttons, so the host cannot filter them; the switch belongs to the panel's own WheelForPaging option. That option is desktop-wide, so it is written the way the panel font is (CandidateFontSync): an untouched default leaves the desktop's value, after that every change the user makes is written once after the host completes the write call.
class CandidateWheelPagingSync {
public:
  std::optional<bool> next(bool enabled) {
    if (!last_ && !enabled) {
      last_ = enabled;
      pending_.reset();
      return std::nullopt;
    }
    if (last_ == enabled) {
      pending_.reset();
      return std::nullopt;
    }
    pending_ = enabled;
    return enabled;
  }

  void mark_applied(bool enabled) {
    if (pending_ != enabled) return;
    last_ = enabled;
    pending_.reset();
  }
  bool pending() const { return pending_.has_value(); }

private:
  std::optional<bool> last_;
  std::optional<bool> pending_;
};

}  // namespace msime::linux_host

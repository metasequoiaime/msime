#include "ClipboardPresentation.h"
#include "ClipboardHistory.h"
#include <algorithm>

namespace msime::windows {
void ClipboardMailbox::publish(bool enabled, std::vector<std::string> items) {
  if (items.size() > ClipboardHistory::max_items) items.resize(ClipboardHistory::max_items);
  std::lock_guard lock(mutex_);
  if (latest_ && latest_->enabled == enabled && latest_->items == items) return;
  latest_ = ClipboardPresentation{++revision_, enabled, std::move(items)};
}
std::optional<ClipboardPresentation> ClipboardMailbox::snapshot() const {
  std::lock_guard lock(mutex_);
  return latest_;
}
void ClipboardMailbox::clear() {
  std::lock_guard lock(mutex_);
  latest_.reset();
}
} // namespace msime::windows

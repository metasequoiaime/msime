#pragma once

#include <cstddef>
#include <deque>
#include <optional>
#include <utility>

namespace msime::voice {

// A FIFO whose retained input size is bounded independently of the number of
// commands. The caller supplies the serialized size that caused each item to
// be allocated, so parsing overhead cannot turn a line limit into an unbounded
// process queue.
template <typename T> class BoundedCommandQueue {
public:
  explicit BoundedCommandQueue(std::size_t max_bytes) : max_bytes_(max_bytes) {}

  bool try_push(T value, std::size_t bytes) {
    if (bytes > max_bytes_ || bytes > max_bytes_ - bytes_) return false;
    queue_.push_back({std::move(value), bytes});
    bytes_ += bytes;
    return true;
  }

  std::optional<T> pop() {
    if (queue_.empty()) return std::nullopt;
    auto item = std::move(queue_.front());
    queue_.pop_front();
    bytes_ -= item.bytes;
    return std::move(item.value);
  }

  void clear() {
    queue_.clear();
    bytes_ = 0;
  }

  bool empty() const noexcept { return queue_.empty(); }
  std::size_t size() const noexcept { return queue_.size(); }
  std::size_t bytes() const noexcept { return bytes_; }

private:
  struct Item {
    T value;
    std::size_t bytes;
  };

  std::deque<Item> queue_;
  std::size_t max_bytes_;
  std::size_t bytes_ = 0;
};

} // namespace msime::voice

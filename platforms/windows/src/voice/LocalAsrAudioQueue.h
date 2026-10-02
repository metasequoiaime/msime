#pragma once

#include <condition_variable>
#include <cstddef>
#include <mutex>
#include <utility>
#include <vector>

namespace msime::windows {

// The capture callback must remain non-blocking, so a local recognizer gets a
// finite amount of audio to wait through model loading or a slow decode. At
// 16 kHz mono float PCM this is 10 seconds, or about 640 KiB.
class LocalAsrAudioQueue final {
public:
  enum class PushResult { accepted, closed, overflowed };

  static constexpr std::size_t kDefaultMaximumSamples = 16000 * 10;

  explicit LocalAsrAudioQueue(
      std::size_t maximum_samples = kDefaultMaximumSamples)
      : maximum_samples_(maximum_samples) {
    pending_.reserve(maximum_samples_);
  }

  LocalAsrAudioQueue(const LocalAsrAudioQueue &) = delete;
  LocalAsrAudioQueue &operator=(const LocalAsrAudioQueue &) = delete;

  PushResult push(const float *samples, std::size_t count) {
    if (!samples || count == 0)
      return PushResult::accepted;
    bool accepted = false;
    {
      std::lock_guard lock(mutex_);
      if (closed_)
        return overflowed_ ? PushResult::overflowed : PushResult::closed;
      if (count > maximum_samples_ - pending_.size()) {
        overflowed_ = true;
        closed_ = true;
      } else {
        pending_.insert(pending_.end(), samples, samples + count);
        accepted = true;
      }
    }
    if (accepted) {
      wake_.notify_one();
      return PushResult::accepted;
    }
    wake_.notify_all();
    return PushResult::overflowed;
  }

  // Waits until audio is available or the producer closes the queue. The
  // returned batch owns its samples, so the capture callback can immediately
  // reuse its input buffer.
  std::vector<float> wait_and_take(bool &ended) {
    std::unique_lock lock(mutex_);
    wake_.wait(lock, [this] { return closed_ || !pending_.empty(); });
    std::vector<float> batch;
    batch.swap(pending_);
    ended = closed_;
    return batch;
  }

  void finish() {
    {
      std::lock_guard lock(mutex_);
      closed_ = true;
    }
    wake_.notify_all();
  }

  void cancel() {
    {
      std::lock_guard lock(mutex_);
      closed_ = true;
      cancelled_ = true;
      pending_.clear();
    }
    wake_.notify_all();
  }

  std::size_t size() const {
    std::lock_guard lock(mutex_);
    return pending_.size();
  }

  bool closed() const {
    std::lock_guard lock(mutex_);
    return closed_;
  }

  bool cancelled() const {
    std::lock_guard lock(mutex_);
    return cancelled_;
  }

  bool overflowed() const {
    std::lock_guard lock(mutex_);
    return overflowed_;
  }

private:
  const std::size_t maximum_samples_;
  mutable std::mutex mutex_;
  std::condition_variable wake_;
  std::vector<float> pending_;
  bool closed_ = false;
  bool cancelled_ = false;
  bool overflowed_ = false;
};

} // namespace msime::windows

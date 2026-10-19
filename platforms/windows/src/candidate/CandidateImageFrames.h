#pragma once

#include <msimeui/AnimatedImage.h>
#include <d2d1.h>
#include <wrl/client.h>
#include <algorithm>
#include <limits>
#include <iterator>
#include <optional>
#include <string>
#include <vector>

namespace msime::windows {

// 候选窗独占播放状态；设备只持有当前帧，不为每张动画上传全部 GPU 位图。
class CandidateImageFrames {
public:
  void begin_frame(ID2D1RenderTarget *target, uint64_t elapsed, bool enabled) {
    if (target_.Get() != target) {
      reset_target();
      target_ = target;
    }
    elapsed_ = elapsed;
    enabled_ = enabled;
    next_delay_.reset();
  }

  ID2D1Bitmap *frame(const std::wstring &path, D2D1_SIZE_F *size = nullptr) {
    if (!enabled_ || !target_ || path.empty())
      return nullptr;
    auto found = std::find_if(entries_.begin(), entries_.end(),
                              [&](const Entry &entry) { return entry.path == path; });
    if (found == entries_.end()) {
      // 一个皮肤至多 37 张资源；额外余量留给旧背景与传统装饰，失败也缓存。
      if (entries_.size() >= 40)
        return nullptr;
      Entry entry;
      entry.path = path;
      Microsoft::WRL::ComPtr<IWICImagingFactory> factory;
      if (SUCCEEDED(CoCreateInstance(CLSID_WICImagingFactory, nullptr, CLSCTX_INPROC_SERVER,
                                       IID_PPV_ARGS(factory.GetAddressOf()))) &&
          msimeui::AnimatedImage::MayBeAnimated(factory.Get(), path)) {
        msimeui::AnimatedImage::Limits limits;
        limits.maxDecodedBytes = budget_ - decoded_bytes_;
        auto image = msimeui::AnimatedImage::DecodeFile(factory.Get(), path, limits);
        if (image && image->IsAnimated() && image->DecodedBytes() <= limits.maxDecodedBytes) {
          decoded_bytes_ += image->DecodedBytes();
          entry.image = std::move(image);
        }
      }
      entries_.push_back(std::move(entry));
      found = std::prev(entries_.end());
    }
    if (!found->image)
      return nullptr;
    const auto &image = *found->image;
    const auto sample = image.Timeline().SampleAt(elapsed_);
    const auto &pixels = image.FramePixels(sample.frameIndex);
    const auto dimensions = D2D1::SizeU(image.Width(), image.Height());
    if (!found->bitmap) {
      const auto properties = D2D1::BitmapProperties(
          D2D1::PixelFormat(DXGI_FORMAT_B8G8R8A8_UNORM, D2D1_ALPHA_MODE_PREMULTIPLIED), 96, 96);
      if (FAILED(target_->CreateBitmap(dimensions, pixels.data(), image.Width() * 4,
                                        &properties, found->bitmap.GetAddressOf())))
        return nullptr;
      found->frame = sample.frameIndex;
    } else if (found->frame != sample.frameIndex) {
      if (FAILED(found->bitmap->CopyFromMemory(nullptr, pixels.data(), image.Width() * 4))) {
        found->bitmap.Reset();
        return nullptr;
      }
      found->frame = sample.frameIndex;
    }
    if (size)
      *size = D2D1::SizeF(static_cast<float>(image.Width()), static_cast<float>(image.Height()));
    if (sample.nextChangeMs) {
      const uint64_t delay = *sample.nextChangeMs > elapsed_ ? *sample.nextChangeMs - elapsed_ : 1;
      if (!next_delay_ || delay < *next_delay_)
        next_delay_ = delay;
    }
    return found->bitmap.Get();
  }

  std::optional<uint64_t> next_delay() const { return next_delay_; }
  uint64_t decoded_bytes() const { return decoded_bytes_; }

  void stop() { next_delay_.reset(); enabled_ = false; }
  void reset_target() {
    for (auto &entry : entries_) {
      entry.bitmap.Reset();
      entry.frame = (std::numeric_limits<size_t>::max)();
    }
    target_.Reset();
    next_delay_.reset();
  }
  void clear() {
    reset_target();
    entries_.clear();
    decoded_bytes_ = 0;
  }

private:
  struct Entry {
    std::wstring path;
    std::optional<msimeui::AnimatedImage> image;
    Microsoft::WRL::ComPtr<ID2D1Bitmap> bitmap;
    size_t frame = (std::numeric_limits<size_t>::max)();
  };
  static constexpr uint64_t budget_ = 32000000;
  uint64_t decoded_bytes_ = 0;
  uint64_t elapsed_ = 0;
  bool enabled_ = false;
  std::optional<uint64_t> next_delay_;
  Microsoft::WRL::ComPtr<ID2D1RenderTarget> target_;
  std::vector<Entry> entries_;
};
} // namespace msime::windows

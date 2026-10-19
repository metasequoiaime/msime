// 移植自 msime-windows 固定提交 dd018f79；来源、GPL 许可与本地调整见 msimeui/UPSTREAM.md。
#pragma once

#include <wincodec.h>

#include <cstddef>
#include <cstdint>
#include <optional>
#include <string>
#include <vector>

namespace msimeui
{

struct AnimationTimeline
{

    std::vector<uint32_t> frameDelaysMs;

    uint32_t playCount = 0;

    struct Sample
    {
        size_t frameIndex = 0;

        std::optional<uint64_t> nextChangeMs;
    };

    uint64_t LoopDurationMs() const;
    Sample SampleAt(uint64_t elapsedMs) const;
};

uint32_t NormalizeFrameDelayMs(uint32_t delayMs);

struct AnimatedImageLimits
{
    uint32_t maxFrames = 120;

    uint64_t maxDecodedBytes = 32000000;
    uint64_t maxFileBytes = 8ull * 1024 * 1024;
    uint32_t maxDimension = 2048;
};

class AnimatedImage
{
  public:
    enum class Format
    {
        Still,
        Apng,
        Gif,
        MultiFrame,
    };

    using Limits = AnimatedImageLimits;

    static std::optional<AnimatedImage> DecodeFile(IWICImagingFactory *factory, const std::wstring &filePath,
                                                   const Limits &limits = Limits());
    static std::optional<AnimatedImage> DecodeMemory(IWICImagingFactory *factory, const uint8_t *data, size_t size,
                                                     const Limits &limits = Limits());

    static bool MayBeAnimated(IWICImagingFactory *factory, const std::wstring &filePath);

    uint32_t Width() const
    {
        return width_;
    }
    uint32_t Height() const
    {
        return height_;
    }
    size_t FrameCount() const
    {
        return frames_.size();
    }
    bool IsAnimated() const
    {
        return frames_.size() > 1;
    }
    Format GetFormat() const
    {
        return format_;
    }
    const AnimationTimeline &Timeline() const
    {
        return timeline_;
    }

    const std::vector<uint8_t> &FramePixels(size_t index) const
    {
        return frames_[index];
    }
    uint64_t DecodedBytes() const
    {
        return static_cast<uint64_t>(width_) * height_ * 4 * frames_.size();
    }

  private:
    friend struct AnimatedImageDecoder;

    uint32_t width_ = 0;
    uint32_t height_ = 0;
    Format format_ = Format::Still;
    AnimationTimeline timeline_;
    std::vector<std::vector<uint8_t>> frames_;
};
}

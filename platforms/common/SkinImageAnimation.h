#pragma once

#include "HostApiString.h"
#include <algorithm>
#include <cstdint>
#include <filesystem>
#include <fstream>
#include <optional>
#include <vector>

namespace msime::skin {
// 各原生工具栏共用解码和播放规则；图片只在皮肤资源变动时加载。
struct ImageFrame {
  std::vector<uint8_t> rgba;
  uint32_t delay_ms = 0;
};
struct ImageAnimation {
  uint32_t width = 0;
  uint32_t height = 0;
  uint32_t play_count = 0;
  std::vector<ImageFrame> frames;
  // 所有完整动画帧的 alpha 并集，透明处不能截获另一个窗口的点击。
  std::vector<uint8_t> alpha;

  size_t frame_at(uint64_t elapsed, bool reduce_motion = false) const {
    if (reduce_motion || frames.size() < 2) return 0;
    uint64_t duration = 0;
    for (const auto &frame : frames) duration += frame.delay_ms;
    if (!duration) return 0;
    if (play_count && elapsed / duration >= play_count) return frames.size() - 1;
    elapsed %= duration;
    for (size_t at = 0; at < frames.size(); ++at) {
      if (elapsed < frames[at].delay_ms) return at;
      elapsed -= frames[at].delay_ms;
    }
    return frames.size() - 1;
  }

  static std::optional<ImageAnimation> decode(const std::vector<uint8_t> &bytes,
                                             uint64_t &remaining_pixels) {
    struct Context {
      ImageAnimation image;
      uint64_t remaining;
    } context{{}, remaining_pixels};
    const auto callback = [](void *raw, const uint8_t *rgba, size_t length,
                             uint32_t width, uint32_t height, uint32_t,
                             uint32_t, uint32_t delay, uint32_t plays) -> int32_t {
      auto &state = *static_cast<Context *>(raw);
      const uint64_t pixels = uint64_t(width) * height;
      if (!width || !height || pixels > state.remaining || length != pixels * 4)
        return 0;
      try {
        auto &image = state.image;
        if (image.frames.empty()) {
          image.width = width;
          image.height = height;
          image.play_count = plays;
          image.alpha.resize(static_cast<size_t>(pixels));
        } else if (image.width != width || image.height != height) {
          return 0;
        }
        image.frames.push_back({std::vector<uint8_t>(rgba, rgba + length), delay});
        for (size_t at = 0; at < image.alpha.size(); ++at)
          image.alpha[at] = (std::max)(image.alpha[at], rgba[at * 4 + 3]);
        state.remaining -= pixels;
        return 1;
      } catch (...) {
        // C ABI 回调不能让分配异常越过 Rust 栈。
        return 0;
      }
    };
    if (msime_client_skin_animation_decode(bytes.data(), bytes.size(), callback, &context) != 1 ||
        context.image.frames.empty()) return std::nullopt;
    remaining_pixels = context.remaining;
    return std::move(context.image);
  }

  static std::optional<ImageAnimation> load(const std::filesystem::path &directory,
                                           const std::filesystem::path &relative,
                                           uint64_t &remaining_pixels) {
    if (relative.empty() || relative.is_absolute()) return std::nullopt;
    std::error_code error;
    const auto root = std::filesystem::canonical(directory, error);
    if (error) return std::nullopt;
    const auto path = std::filesystem::canonical(root / relative, error);
    if (error) return std::nullopt;
    const auto mismatch = std::mismatch(root.begin(), root.end(), path.begin(), path.end());
    if (mismatch.first != root.end() || mismatch.second == path.end()) return std::nullopt;
    const auto length = std::filesystem::file_size(path, error);
    if (error || !length || length > 8 * 1024 * 1024) return std::nullopt;
    std::ifstream stream(path, std::ios::binary);
    std::vector<uint8_t> bytes(static_cast<size_t>(length));
    if (!stream.read(reinterpret_cast<char *>(bytes.data()), static_cast<std::streamsize>(length)))
      return std::nullopt;
    return decode(bytes, remaining_pixels);
  }
};
} // namespace msime::skin

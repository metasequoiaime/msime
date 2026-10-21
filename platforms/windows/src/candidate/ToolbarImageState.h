#pragma once

#include <windows.h>
#include <d2d1.h>
#include <wrl/client.h>
#include <nlohmann/json.hpp>
#include "ToolbarLayout.h"
#include "ToolbarIcons.h"
#include "../../../../platforms/common/SkinImageAnimation.h"
#include <cmath>
#include <array>
#include <cstring>
#include <map>
#include <set>

namespace msime::windows {
// 布局来自共享桥，窗口的绘制、命中、提示和读屏只消费同一份矩形。
class ToolbarImageState {
public:
  struct Button {
    int id;
    ToolbarRect rect;
    double icon;
    std::string background;
    std::string image;
  };
  bool active() const { return manifest_.is_object(); }
  double width = 0;
  double height = 0;
  std::vector<Button> buttons;

  void set_skin(nlohmann::json manifest, std::filesystem::path directory) {
    manifest_ = manifest.is_object() ? std::move(manifest) : nlohmann::json();
    directory_ = std::move(directory);
    buttons.clear();
    images_.clear();
    anchor_ = GetTickCount64();
    if (!active()) return;
    background_width_ = manifest_.value("imageWidthDip", 0.0);
    background_height_ = manifest_.value("imageHeightDip", 0.0);
    background_ = manifest_.value("image", std::string());
    std::set<std::string> paths;
    const auto add = [&](const nlohmann::json &object, const char *key) {
      if (object.contains(key) && object.at(key).is_string()) paths.insert(object.at(key).get<std::string>());
    };
    paths.insert(background_);
    add(manifest_.at("buttonDefaults"), "background");
    for (const auto &button : manifest_.at("buttons")) {
      add(button, "background");
      add(button, "image");
      for (const auto &path : button.at("images")) if (path.is_string()) paths.insert(path.get<std::string>());
    }
    uint64_t remaining = 8000000;
    for (const auto &path : paths) {
      Entry entry;
      entry.image = skin::ImageAnimation::load(directory_, std::filesystem::u8path(path), remaining);
      // Direct2D 接收预乘 BGRA；alpha 并集在变换前已经计算好。
      if (entry.image) for (auto &frame : entry.image->frames)
        for (size_t at = 0; at < frame.rgba.size(); at += 4) {
          const uint32_t alpha = frame.rgba[at + 3];
          const uint8_t red = frame.rgba[at];
          frame.rgba[at] = static_cast<uint8_t>((frame.rgba[at + 2] * alpha + 127) / 255);
          frame.rgba[at + 1] = static_cast<uint8_t>((frame.rgba[at + 1] * alpha + 127) / 255);
          frame.rgba[at + 2] = static_cast<uint8_t>((red * alpha + 127) / 255);
        }
      images_.emplace(path, std::move(entry));
    }
  }

  void update(const std::vector<int> &shown, const std::array<std::string, 7> &states) {
    buttons.clear();
    width = height = 0;
    if (!active()) return;
    nlohmann::json requests = nlohmann::json::array();
    for (int id : shown) if (id >= 0 && id < 7) requests.push_back({keys_[id], states[id]});
    const auto request = nlohmann::json{{"toolbar", manifest_}, {"shown", requests}}.dump();
    auto owned = host_api::own_string(msime_client_skin_toolbar_image_layout(
        reinterpret_cast<const uint8_t *>(request.data()), request.size()));
    if (!owned) return;
    const auto envelope = nlohmann::json::parse(owned.get(), nullptr, false);
    if (envelope.is_discarded() || !envelope.value("ok", false)) return;
    const auto &layout = envelope.at("value");
    width = layout.at("widthDip").get<double>();
    height = layout.at("heightDip").get<double>();
    for (const auto &slot : layout.at("buttons")) {
      const auto key = slot.at("key").get<std::string>();
      const auto found = std::find(keys_.begin(), keys_.end(), key);
      if (found == keys_.end()) continue;
      const double x = slot.at("xDip").get<double>(), y = slot.at("yDip").get<double>();
      const auto path = [&](const char *name) {
        return slot.at(name).is_string() ? slot.at(name).get<std::string>() : std::string();
      };
      buttons.push_back({static_cast<int>(found - keys_.begin()),
        {x, y, x + slot.at("widthDip").get<double>(), y + slot.at("heightDip").get<double>()},
        slot.at("iconSizeDip").get<double>(), path("background"), path("image")});
    }
  }

  std::optional<size_t> button_at(double x, double y) const {
    for (size_t at = buttons.size(); at > 0; --at) {
      const auto &box = buttons[at - 1].rect;
      if (x >= box.left && x < box.right && y >= box.top && y < box.bottom) return at - 1;
    }
    return std::nullopt;
  }
  bool drag_at(double x, double y) const {
    if (button_at(x, y) || x < 0 || y < 0 || x >= background_width_ || y >= background_height_) return false;
    const auto *background = image(background_);
    if (!background) return true;
    const auto column = static_cast<size_t>(x / background_width_ * background->width);
    const auto row = static_cast<size_t>(y / background_height_ * background->height);
    return background->alpha[row * background->width + column] > 0;
  }

  // 窗口形状包含全部动画帧与完整按钮矩形，真正将透明区交还给其它进程的窗口。
  void apply_region(HWND window, double unit) const {
    HRGN region = nullptr;
    const auto *background = image(background_);
    if (background) {
      std::vector<RECT> runs;
      for (uint32_t y = 0; y < background->height; ++y) {
        uint32_t x = 0;
        while (x < background->width) {
          while (x < background->width && !background->alpha[y * background->width + x]) ++x;
          const uint32_t start = x;
          while (x < background->width && background->alpha[y * background->width + x]) ++x;
          if (start < x) runs.push_back({static_cast<LONG>(start), static_cast<LONG>(y), static_cast<LONG>(x), static_cast<LONG>(y + 1)});
        }
      }
      // 一次创建已经排序、不相交的源像素区间，避免逐像素反复合并 GDI 区域。
      const size_t size = sizeof(RGNDATAHEADER) + runs.size() * sizeof(RECT);
      std::vector<uint32_t> buffer((size + 3) / 4);
      auto *data = reinterpret_cast<RGNDATA *>(buffer.data());
      data->rdh = {sizeof(RGNDATAHEADER), RDH_RECTANGLES, static_cast<DWORD>(runs.size()),
        static_cast<DWORD>(runs.size() * sizeof(RECT)), {0, 0, static_cast<LONG>(background->width), static_cast<LONG>(background->height)}};
      if (!runs.empty()) std::memcpy(data->Buffer, runs.data(), runs.size() * sizeof(RECT));
      XFORM transform{static_cast<FLOAT>(background_width_ / background->width * unit), 0, 0,
        static_cast<FLOAT>(background_height_ / background->height * unit), 0, 0};
      region = ExtCreateRegion(&transform, static_cast<DWORD>(size), data);
    }
    // 区域创建失败时保留可拖动的背景矩形，不能让整张背景失去命中。
    const bool fallback = !region;
    if (!region) region = CreateRectRgn(0, 0, 0, 0);
    if (!region) return;
    const auto add = [&](const ToolbarRect &box) {
      HRGN part = CreateRectRgn(static_cast<int>(std::floor(box.left * unit)), static_cast<int>(std::floor(box.top * unit)),
                               static_cast<int>(std::ceil(box.right * unit)), static_cast<int>(std::ceil(box.bottom * unit)));
      if (part) { CombineRgn(region, region, part, RGN_OR); DeleteObject(part); }
    };
    if (fallback) add({0, 0, background_width_, background_height_});
    for (const auto &button : buttons) add(button.rect);
    if (!SetWindowRgn(window, region, TRUE)) DeleteObject(region);
  }

  void draw_background(ID2D1RenderTarget *target, float scale) {
    draw(target, background_, {0, 0, background_width_, background_height_}, scale);
  }
  bool draw(ID2D1RenderTarget *target, const std::string &path, const ToolbarRect &rect, float scale) {
    auto found = images_.find(path);
    if (found == images_.end() || !found->second.image) return false;
    auto &entry = found->second;
    const auto &image = *entry.image;
    BOOL animate = TRUE;
    SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, &animate, 0);
    const size_t frame = image.frame_at(GetTickCount64() - anchor_, !animate);
    if (entry.target != target) { entry.bitmap.Reset(); entry.target = target; }
    const auto &pixels = image.frames[frame].rgba;
    if (!entry.bitmap) {
      const auto properties = D2D1::BitmapProperties(D2D1::PixelFormat(DXGI_FORMAT_B8G8R8A8_UNORM, D2D1_ALPHA_MODE_PREMULTIPLIED), 96, 96);
      if (FAILED(target->CreateBitmap(D2D1::SizeU(image.width, image.height), pixels.data(), image.width * 4, properties, &entry.bitmap))) return false;
    } else if (FAILED(entry.bitmap->CopyFromMemory(nullptr, pixels.data(), image.width * 4))) {
      entry.bitmap.Reset();
      return false;
    }
    target->DrawBitmap(entry.bitmap.Get(), D2D1::RectF(static_cast<float>(rect.left) * scale, static_cast<float>(rect.top) * scale,
        static_cast<float>(rect.right) * scale, static_cast<float>(rect.bottom) * scale), 1, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR);
    return true;
  }
  void reset_target() {
    for (auto &[path, entry] : images_) { (void)path; entry.bitmap.Reset(); entry.target = nullptr; }
  }
private:
  const skin::ImageAnimation *image(const std::string &path) const {
    const auto found = images_.find(path);
    return found != images_.end() && found->second.image ? &*found->second.image : nullptr;
  }
  struct Entry {
    std::optional<skin::ImageAnimation> image;
    Microsoft::WRL::ComPtr<ID2D1Bitmap> bitmap;
    ID2D1RenderTarget *target = nullptr;
  };
  inline static const std::array<std::string, 7> keys_{"status", "fullwidth", "punctuation", "character_set", "emoji", "screen_keyboard", "settings"};
  nlohmann::json manifest_;
  std::filesystem::path directory_;
  std::string background_;
  double background_width_ = 0;
  double background_height_ = 0;
  ULONGLONG anchor_ = 0;
  std::map<std::string, Entry> images_;
};
} // namespace msime::windows

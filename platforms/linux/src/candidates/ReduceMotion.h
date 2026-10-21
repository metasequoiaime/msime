#pragma once

#include "ReduceMotionPolicy.h"

#include <gio/gio.h>
#include <sys/stat.h>

#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <optional>
#include <string>

namespace msime::linux_host {

// 读桌面的「减弱动态效果」，规则见 ReduceMotionPolicy.h。GSettings 每次从 dconf 的内存映射取值，不需要 GLib 主循环也能看到新值；KDE 的 kdeglobals 只在修改时间或大小变了时重读。只有 Fcitx5 的原生画布用它。
class ReduceMotionReader {
public:
  ReduceMotionReader() = default;
  ~ReduceMotionReader() {
    if (settings_) g_object_unref(settings_);
  }
  ReduceMotionReader(const ReduceMotionReader &) = delete;
  ReduceMotionReader &operator=(const ReduceMotionReader &) = delete;

  bool read() {
    if (!looked_up_) {
      looked_up_ = true;
      // 没装这份 schema 时 g_settings_new 会直接中止进程，所以先查。
      if (auto *source = g_settings_schema_source_get_default()) {
        if (auto *schema = g_settings_schema_source_lookup(source, "org.gnome.desktop.interface", TRUE)) {
          if (g_settings_schema_has_key(schema, "enable-animations"))
            settings_ = g_settings_new("org.gnome.desktop.interface");
          g_settings_schema_unref(schema);
        }
      }
    }
    std::optional<bool> gnome;
    if (settings_) gnome = g_settings_get_boolean(settings_, "enable-animations");
    readKde();
    return reduce_motion_from(gnome, kde_);
  }

private:
  void readKde() {
    std::filesystem::path file;
    if (const char *config = std::getenv("XDG_CONFIG_HOME"); config && config[0] == '/') file = std::filesystem::path(config) / "kdeglobals";
    else if (const char *home = std::getenv("HOME"); home && home[0] == '/') file = std::filesystem::path(home) / ".config" / "kdeglobals";
    struct stat metadata {};
    if (file.empty() || ::stat(file.c_str(), &metadata) != 0 || !S_ISREG(metadata.st_mode) || metadata.st_size > 1024 * 1024) {
      kde_.reset();
      kde_stamp_.clear();
      return;
    }
    const auto stamp = std::to_string(metadata.st_mtim.tv_sec) + "." + std::to_string(metadata.st_mtim.tv_nsec) + ":" +
                       std::to_string(metadata.st_size);
    if (stamp == kde_stamp_) return;
    kde_stamp_ = stamp;
    std::ifstream stream(file, std::ios::binary);
    const std::string text((std::istreambuf_iterator<char>(stream)), std::istreambuf_iterator<char>());
    kde_ = kde_reduce_motion(text);
  }

  GSettings *settings_ = nullptr;
  bool looked_up_ = false;
  std::string kde_stamp_;
  std::optional<bool> kde_;
};

} // msime::linux_host 命名空间

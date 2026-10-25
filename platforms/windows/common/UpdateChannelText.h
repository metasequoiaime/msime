#pragma once
#include <cstdint>
#include <optional>
#include <string>
#include <string_view>

// 桌面更新通道（共享偏好 `update_channel`，client-core 的 `UpdateChannel`）在 Windows 界面上的写法。设置窗口「关于」页（settings/AboutPageModel.h）和 Server 的新版本通知（src/system/UpdateNotification.h）共用这一份，同一个构建在两处读起来一样。只依赖标准库，方便在主机上单独测试。
namespace msime::windows::update_channel {

inline constexpr std::wstring_view stable = L"stable";
inline constexpr std::wstring_view beta = L"beta";
inline constexpr std::wstring_view nightly = L"nightly";

// 偏好里存的通道：只认这三种写法，其余（没有这一项、以后的版本加的通道、手改的文件）按稳定版，与 client-core 读偏好文档的宽松规则相同。返回值总是上面三个常量之一，可以原样放进检查更新请求的 `channel`。
inline std::wstring_view normalize(std::wstring_view value) {
  if (value == beta)
    return beta;
  if (value == nightly)
    return nightly;
  return stable;
}

// 窄字符版本，给用 nlohmann::json 读偏好的 Server：规则相同，返回 "stable"、"beta"、"nightly" 之一。
inline std::string_view normalize(std::string_view value) {
  if (value == "beta")
    return "beta";
  if (value == "nightly")
    return "nightly";
  return "stable";
}

// 通道的中文名：稳定版、测试版、每日构建，与共享设置页的选项同名。认不出的通道为空。
inline std::wstring_view title(std::wstring_view channel) {
  if (channel == stable)
    return L"稳定版";
  if (channel == beta)
    return L"测试版";
  if (channel == nightly)
    return L"每日构建";
  return {};
}

// 一个构建的称呼：「稳定版 v0.3.1」「测试版 v0.3.1」「每日构建 v0.3.1（构建 123）」。每日构建同一个版本号每天都有新的一份，只有带上运行 ID 才分得清，所以只有它带括注，没有运行 ID 时也不带；认不出的通道只写「v版本号」。
inline std::wstring build_label(std::wstring_view channel, std::wstring_view version,
                                std::optional<std::uint64_t> build_run) {
  std::wstring label(title(channel));
  if (!label.empty())
    label.append(L" ");
  label.append(L"v");
  label.append(version);
  if (channel == nightly && build_run)
    label.append(L"（构建 " + std::to_wstring(*build_run) + L"）");
  return label;
}

} // namespace msime::windows::update_channel

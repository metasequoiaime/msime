#pragma once
#include <cstddef>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

// 「关于」页里只拼文字的两块，照共享设置 UI 逐字搬来：「系统信息」一组列出的各项和「复制系统信息」放进剪贴板的文本（`packages/ui/src/settings/system-info-section.tsx` 的 `systemInfoEntries`、`systemInfoText`），以及检查更新发现新版本时结果块里的几项（`update-manifest.ts` 的 `mirrorDownloadUrl` 和 `describeInstallerTrust` 的 Windows 一支）。版本号、系统版本、检查更新的回答这些输入由 main.cpp 取得；这里不依赖 Windows 和 WinRT 头文件，方便在主机上单独测试。
namespace msime::settings {

struct SystemInfoEntry {
  std::wstring label;
  std::wstring value;
};

// 本程序编译成的处理器架构，写法与共享页的 `HostCapabilities.arch`（Tauri 外壳填的 Rust `std::env::consts::ARCH`）相同；认不出时为空，「处理器架构」一项就省略。
constexpr std::wstring_view build_architecture() {
#if defined(_M_ARM64) || defined(__aarch64__)
  return L"aarch64";
#elif defined(_M_X64) || defined(__x86_64__)
  return L"x86_64";
#elif defined(_M_IX86) || defined(__i386__)
  return L"x86";
#else
  return {};
#endif
}

// 「系统信息」各项，顺序与共享页相同。Windows 只报得出应用版本、系统、处理器架构和输入方案，内核、桌面环境、输入法框架和设备型号与共享页在宿主不报告时一样直接省略，不写「未知」；空的输入同样省略那一项。应用版本后的括注与「当前版本」一致，非完整版另写版本名：`v0.53.0（水杉五笔，2026-10-19 发布）`。`edition_name` 在完整版时传空（共享页只在非 full 版本时收到版本名）。系统是「Windows」加 `RtlGetVersion` 的版本号，读不到版本号时只写「Windows」。
inline std::vector<SystemInfoEntry> system_info_entries(std::wstring_view app_version,
                                                        std::wstring_view edition_name,
                                                        std::wstring_view release_date,
                                                        std::wstring_view os_version,
                                                        std::wstring_view architecture,
                                                        std::wstring_view scheme) {
  std::vector<SystemInfoEntry> entries;
  if (!app_version.empty()) {
    std::wstring notes(edition_name);
    if (!release_date.empty()) {
      if (!notes.empty())
        notes.append(L"，");
      notes.append(release_date);
      notes.append(L" 发布");
    }
    std::wstring value = L"v";
    value.append(app_version);
    if (!notes.empty())
      value.append(L"（" + notes + L"）");
    entries.push_back({L"应用版本", std::move(value)});
  }
  std::wstring system = L"Windows";
  if (!os_version.empty()) {
    system.append(L" ");
    system.append(os_version);
  }
  entries.push_back({L"系统", std::move(system)});
  if (!architecture.empty())
    entries.push_back({L"处理器架构", std::wstring(architecture)});
  if (!scheme.empty())
    entries.push_back({L"输入方案", std::wstring(scheme)});
  return entries;
}

// 「复制系统信息」放进剪贴板的文本：一行标题，然后每项一行「名称：值」，以 LF 连接，贴进问题报告里直接可读。
inline std::wstring system_info_text(std::vector<SystemInfoEntry> const &entries) {
  std::wstring text = L"水杉 IME 系统信息";
  for (auto const &entry : entries) {
    text.append(L"\n");
    text.append(entry.label);
    text.append(L"：");
    text.append(entry.value);
  }
  return text;
}

// 国内镜像的地址前缀（`app-resources.ts` 的 `downloadMirrorPrefix`）。
inline constexpr std::wstring_view download_mirror_prefix = L"https://dl.msime.app/gh/";

namespace detail {
inline bool ascii_alnum(wchar_t c) {
  return (c >= L'0' && c <= L'9') || (c >= L'A' && c <= L'Z') || (c >= L'a' && c <= L'z');
}
// 正则 `\w` 加上 `extra` 里的字符。
inline bool word_or(wchar_t c, std::wstring_view extra) {
  return ascii_alnum(c) || c == L'_' || extra.find(c) != std::wstring_view::npos;
}
} // namespace detail

// 安装包名的形状，与共享页的 `installerNamePattern`（`^[A-Za-z0-9][\w.+~-]*$`）相同：它会进镜像地址和让用户复制的核对命令，宿主只会报告这种形状的名字，别的一概不用。
inline bool installer_name_valid(std::wstring_view name) {
  if (name.empty() || !detail::ascii_alnum(name.front()))
    return false;
  for (const wchar_t c : name.substr(1))
    if (!detail::word_or(c, L".+~-"))
      return false;
  return true;
}

// 安装包的 SHA256，与共享页的 `sha256Pattern`（`^[0-9a-f]{64}$`）相同。
inline bool installer_sha256_valid(std::wstring_view value) {
  if (value.size() != 64)
    return false;
  for (const wchar_t c : value)
    if (!((c >= L'0' && c <= L'9') || (c >= L'a' && c <= L'f')))
      return false;
  return true;
}

// 国内镜像上这次更新的安装包（共享页的 `mirrorDownloadUrl`）：`<前缀>https://github.com/metasequoiaime/<仓库>/releases/download/<tag>/<安装包名>`。发布页必须是 `https://github.com/metasequoiaime/<仓库>/releases/tag/<tag>`（仓库是 `[\w.-]+`，tag 是 `[\w.+-]+`），安装包名要通过 `installer_name_valid`；否则返回空，结果块里不画「国内镜像下载」。镜像上是同一个文件，页面上让用户核对的 SHA256 照样适用。
inline std::wstring update_mirror_url(std::wstring_view release_url, std::wstring_view installer_name) {
  constexpr std::wstring_view owner = L"https://github.com/metasequoiaime/";
  constexpr std::wstring_view tag_path = L"/releases/tag/";
  if (!installer_name_valid(installer_name) || release_url.substr(0, owner.size()) != owner)
    return {};
  const auto rest = release_url.substr(owner.size());
  const auto split = rest.find(tag_path);
  if (split == std::wstring_view::npos || split == 0)
    return {};
  const auto repository = rest.substr(0, split);
  const auto tag = rest.substr(split + tag_path.size());
  if (tag.empty())
    return {};
  for (const wchar_t c : repository)
    if (!detail::word_or(c, L".-"))
      return {};
  for (const wchar_t c : tag)
    if (!detail::word_or(c, L".+-"))
      return {};
  std::wstring url(download_mirror_prefix);
  url.append(owner);
  url.append(repository);
  url.append(L"/releases/download/");
  url.append(tag);
  url.append(L"/");
  url.append(installer_name);
  return url;
}

// 结果块的第一句：「水杉 IME vX 已发布。」
inline std::wstring update_release_headline(std::wstring_view version) {
  return L"水杉 IME v" + std::wstring(version) + L" 已发布。";
}

// 结果块里关于安装包可信度的几行（共享页 `describeInstallerTrust` 的 Windows 一支）：`warning` 是红字警告，签了名时为空；有 SHA256 时 `sha256` 和 `command` 是「下载后请核对 SHA256：」「核对命令：」后面那两段。
struct InstallerTrust {
  std::wstring warning;
  std::wstring sha256;
  std::wstring command;
};

// `is_signed` 是回答里的 `signed`，没有这个字段时为空；`installer_name` 和 `sha256` 是回答里通过了上面两项检查的值，没有或不合形状时传空。没有安装包名时用 `fallback_name`（本版本安装包名的推导写法 `MetasequoiaIME-<Id>_Setup_v<版本>.exe`）。措辞沿用正式发布的设置页：未签名的安装程序不只会触发 SmartScreen 提示，还会失去 uiAccess。
inline InstallerTrust installer_trust(std::optional<bool> is_signed, std::wstring_view installer_name,
                                      std::wstring_view sha256, std::wstring_view fallback_name) {
  const std::wstring name(installer_name.empty() ? fallback_name : installer_name);
  const std::wstring unsigned_text =
      L"该版本未经代码签名，SmartScreen 会拦截，且 uiAccess 失效（候选窗口无法浮在以管理员身份运行的程序之上）。";
  const bool unsigned_build = is_signed.has_value() && !*is_signed;
  InstallerTrust trust;
  if (unsigned_build && sha256.empty()) {
    trust.warning = unsigned_text + L"请从发行页一并下载 " + name + L".sha256，用 Get-FileHash .\\" +
                    name + L" -Algorithm SHA256 核对。";
    return trust;
  }
  if (unsigned_build)
    trust.warning = unsigned_text + L"请务必核对下面的校验值。";
  if (!sha256.empty()) {
    trust.sha256 = std::wstring(sha256);
    trust.command = L"Get-FileHash .\\" + name + L" -Algorithm SHA256";
  }
  return trust;
}

} // namespace msime::settings

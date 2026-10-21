#include "../../settings/AboutPageModel.h"
#include <cstdio>
#include <stdexcept>
#include <string>

using namespace msime::settings;
void require_at(bool value, int line) {
  if (!value)
    throw std::runtime_error("About page model test failed at line " + std::to_string(line));
}
#define require(...) require_at((__VA_ARGS__), __LINE__)

int main() {
  try {
    // 完整版：版本号带发布日期，系统带 RtlGetVersion 的版本号，然后是架构和输入方案；顺序与共享页的 systemInfoEntries 相同。
    const auto full = system_info_entries(L"0.53.0", L"", L"2026-10-19", L"10.0.26100", L"x86_64", L"全拼");
    require(full.size() == 4);
    require(full[0].label == L"应用版本" && full[0].value == L"v0.53.0（2026-10-19 发布）");
    require(full[1].label == L"系统" && full[1].value == L"Windows 10.0.26100");
    require(full[2].label == L"处理器架构" && full[2].value == L"x86_64");
    require(full[3].label == L"输入方案" && full[3].value == L"全拼");
    require(system_info_text(full) ==
            L"水杉 IME 系统信息\n应用版本：v0.53.0（2026-10-19 发布）\n系统：Windows 10.0.26100\n"
            L"处理器架构：x86_64\n输入方案：全拼");

    // 非完整版另写版本名，与发布日期用「，」隔开；没有发布日期时只写版本名。
    require(system_info_entries(L"0.53.0", L"水杉五笔", L"2026-10-19", L"", L"", L"")[0].value ==
            L"v0.53.0（水杉五笔，2026-10-19 发布）");
    require(system_info_entries(L"0.53.0", L"水杉五笔", L"", L"", L"", L"")[0].value ==
            L"v0.53.0（水杉五笔）");
    require(system_info_entries(L"0.53.0", L"", L"", L"", L"", L"")[0].value == L"v0.53.0");

    // 报不出的项直接省略，不写「未知」；系统一项总在，读不到版本号时只写 Windows。
    const auto bare = system_info_entries(L"", L"", L"", L"", L"", L"");
    require(bare.size() == 1 && bare[0].label == L"系统" && bare[0].value == L"Windows");
    require(system_info_text(bare) == L"水杉 IME 系统信息\n系统：Windows");
    require(!build_architecture().empty());

    // 安装包名与摘要的形状，与共享页的两条正则相同。
    require(installer_name_valid(L"MetasequoiaIME-Full_Setup_v0.53.0.exe"));
    require(!installer_name_valid(L""));
    require(!installer_name_valid(L"-leading.exe"));
    require(!installer_name_valid(L"a b.exe"));
    require(!installer_name_valid(L"a/b.exe"));
    require(!installer_name_valid(L"a&b.exe"));
    const std::wstring digest(64, L'a');
    require(installer_sha256_valid(digest));
    require(!installer_sha256_valid(std::wstring(63, L'a')));
    require(!installer_sha256_valid(std::wstring(64, L'A')));

    // 镜像地址照 mirrorDownloadUrl 拼；发布页不是 tag 页、安装包名不合形状时不给镜像。
    const std::wstring tag_page = L"https://github.com/metasequoiaime/msime/releases/tag/windows-v0.53.0";
    require(update_mirror_url(tag_page, L"MetasequoiaIME-Full_Setup_v0.53.0.exe") ==
            L"https://dl.msime.app/gh/https://github.com/metasequoiaime/msime/releases/download/"
            L"windows-v0.53.0/MetasequoiaIME-Full_Setup_v0.53.0.exe");
    require(update_mirror_url(tag_page, L"").empty());
    require(update_mirror_url(tag_page, L"bad name.exe").empty());
    require(update_mirror_url(L"https://github.com/metasequoiaime/msime/releases", L"a.exe").empty());
    require(update_mirror_url(L"https://github.com/other/msime/releases/tag/v1", L"a.exe").empty());
    require(update_mirror_url(L"https://github.com/metasequoiaime/msime/releases/tag/", L"a.exe").empty());
    require(update_mirror_url(L"https://github.com/metasequoiaime/msime/releases/tag/v1/x", L"a.exe").empty());
    require(update_mirror_url(L"https://github.com/metasequoiaime//releases/tag/v1", L"a.exe").empty());

    require(update_release_headline(L"0.53.0") == L"水杉 IME v0.53.0 已发布。");

    // 安装包可信度，照 describeInstallerTrust 的 Windows 一支。
    const std::wstring unsigned_text =
        L"该版本未经代码签名，SmartScreen 会拦截，且 uiAccess 失效（候选窗口无法浮在以管理员身份运行的程序之上）。";
    const std::wstring name = L"MetasequoiaIME-Full_Setup_v0.53.0.exe";
    const std::wstring fallback = L"MetasequoiaIME-Full_Setup_v<版本>.exe";
    // 签了名、有摘要：没有警告，给出摘要和核对命令。
    auto trust = installer_trust(true, name, digest, fallback);
    require(trust.warning.empty() && trust.sha256 == digest);
    require(trust.command == L"Get-FileHash .\\" + name + L" -Algorithm SHA256");
    // 未签名、有摘要：警告要求核对下面的校验值。
    trust = installer_trust(false, name, digest, fallback);
    require(trust.warning == unsigned_text + L"请务必核对下面的校验值。");
    require(trust.sha256 == digest && !trust.command.empty());
    // 未签名、没有摘要：只有警告，让用户去发行页下载 .sha256；没有安装包名时用推导的名字。
    trust = installer_trust(false, L"", L"", fallback);
    require(trust.warning == unsigned_text + L"请从发行页一并下载 " + fallback + L".sha256，用 Get-FileHash .\\" +
                                 fallback + L" -Algorithm SHA256 核对。");
    require(trust.sha256.empty() && trust.command.empty());
    // 不知道签没签名、没有摘要：什么都不写。
    trust = installer_trust(std::nullopt, name, L"", fallback);
    require(trust.warning.empty() && trust.sha256.empty() && trust.command.empty());
    std::puts("about page model passed");
    return 0;
  } catch (std::exception const &error) {
    std::fprintf(stderr, "%s\n", error.what());
    return 1;
  }
}

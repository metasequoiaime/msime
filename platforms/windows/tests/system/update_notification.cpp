#include "system/UpdateNotification.h"
#include <cstdio>
#include <stdexcept>
#include <string>

using namespace msime::windows;
void require_at(bool value, int line) {
  if (!value)
    throw std::runtime_error("Update notification test failed at line " + std::to_string(line));
}
#define require(...) require_at((__VA_ARGS__), __LINE__)

namespace {
std::string response(const std::string &value) { return R"({"ok":true,"value":)" + value + "}"; }
} // namespace

int main() {
  try {
    // 偏好里的通道：只认三种写法，没有这一项、写错、不是字符串都按稳定版。
    require(update_channel_preference(nlohmann::json::object()) == "stable");
    require(update_channel_preference(nlohmann::json{{"update_channel", "beta"}}) == "beta");
    require(update_channel_preference(nlohmann::json{{"update_channel", "nightly"}}) == "nightly");
    require(update_channel_preference(nlohmann::json{{"update_channel", "stable"}}) == "stable");
    require(update_channel_preference(nlohmann::json{{"update_channel", "canary"}}) == "stable");
    require(update_channel_preference(nlohmann::json{{"update_channel", "Beta"}}) == "stable");
    require(update_channel_preference(nlohmann::json{{"update_channel", 1}}) == "stable");
    require(update_channel_preference(nlohmann::json()) == "stable");
    require(update_channel_preference(nlohmann::json::array({"beta"})) == "stable");

    // 请求：普通构建带通道，不带架构（同一个安装程序同时支持 x64 和 Arm64）。
    const auto modern = update_background_request("C:\\Users\\a\\AppData\\Local\\MSIME\\update", false,
                                                  "1.4.0", "full", "nightly", "x86_64");
    require(modern == nlohmann::json{{"directory", "C:\\Users\\a\\AppData\\Local\\MSIME\\update"},
                                     {"platform", "windows"},
                                     {"current_version", "1.4.0"},
                                     {"edition", "full"},
                                     {"channel", "nightly"}});
    require(update_background_request("C:\\s", false, "1.4.0", "wubi", "unknown", "aarch64").at("channel") ==
            "stable");
    // Windows 7 构建：平台是 windows7，不带通道（共享层对它一律按稳定版），但必须带架构，共享层按它在 x64 和 x86 两个安装包里选，没有架构时找不到任何发布（与 Win7 专用设置的手动检查相同）。
    const auto legacy = update_background_request("C:\\s", true, "1.4.0", "full", "beta", "x86_64");
    require(legacy == nlohmann::json{{"directory", "C:\\s"},
                                     {"platform", "windows7"},
                                     {"current_version", "1.4.0"},
                                     {"edition", "full"},
                                     {"arch", "x86_64"}});
    require(update_background_request("C:\\s", true, "1.4.0", "wubi", "stable", "x86").at("arch") == "x86");
    // 认不出编译架构时不编一个出来：不带 arch，由共享层拒绝，而不是拿到另一个架构的安装包。
    require(!update_background_request("C:\\s", true, "1.4.0", "full", "stable", "").contains("arch"));
    {
      constexpr auto architecture = update_build_architecture();
      require(architecture.empty() || architecture == "x86_64" || architecture == "x86" ||
              architecture == "aarch64");
    }

    // 回答：没到时间、关掉了、连不上时没有提醒。
    require(parse_update_background_response(response(R"({"status":"not_due","notify":null})")).status ==
            UpdateCheckStatus::not_due);
    require(parse_update_background_response(response(R"({"status":"disabled","notify":null})")).status ==
            UpdateCheckStatus::disabled);
    const auto offline = parse_update_background_response(response(R"({"status":"unavailable","notify":null})"));
    require(offline.status == UpdateCheckStatus::unavailable && !offline.notice);
    const auto quiet = parse_update_background_response(response(R"({"status":"checked","notify":null})"));
    require(quiet.status == UpdateCheckStatus::checked && !quiet.notice);

    // 请求被拒、回答不是 JSON、状态认不出：都算失败，下一轮照常再问。
    require(parse_update_background_response(R"({"ok":false,"error":"invalid update check request"})").status ==
            UpdateCheckStatus::failed);
    require(parse_update_background_response("").status == UpdateCheckStatus::failed);
    require(parse_update_background_response("not json").status == UpdateCheckStatus::failed);
    require(parse_update_background_response(response(R"({"status":"later"})")).status ==
            UpdateCheckStatus::failed);

    // 每日构建：版本号、通道、运行 ID 和发布页都取出来；update 里的其他字段（安装包名、摘要、签名）不影响。
    const auto nightly = parse_update_background_response(response(
        R"({"status":"checked","notify":{"version":{"display":"1.4.0","parts":[1,4,0]},)"
        R"("release_url":"https://github.com/metasequoiaime/msime/releases/tag/nightly",)"
        R"("installer_name":"MetasequoiaIME-Full_Setup_v1.4.0-nightly.18800000000.1.exe",)"
        R"("installer_sha256":null,"signed":false,"channel":"nightly","build_run":18800000000}})"));
    require(nightly.status == UpdateCheckStatus::checked && nightly.notice);
    require(nightly.notice->version == L"1.4.0");
    require(nightly.notice->channel == L"nightly");
    require(nightly.notice->build_run && *nightly.notice->build_run == 18800000000ull);
    require(nightly.notice->release_url == L"https://github.com/metasequoiaime/msime/releases/tag/nightly");
    const auto nightly_text = update_notification_text(*nightly.notice, L"水杉输入法");
    require(nightly_text.title == L"水杉输入法有新版本 v1.4.0");
    require(nightly_text.body ==
            L"每日构建 v1.4.0（构建 18800000000）已发布。点这里打开设置的「关于」页查看并下载。");
    require(nightly_text.tip == L"水杉输入法有新版本 v1.4.0\n点击查看，右键移除此提醒");
    // 标题里的产品名按版本取：几个版本同时安装时分得清是哪个。
    require(update_notification_text(*nightly.notice, L"水杉五笔").title == L"水杉五笔有新版本 v1.4.0");
    // 系统托盘的定长字段：标题 64、提示 128 个宽字符（含结尾的 0）。
    require(nightly_text.title.size() < 64 && nightly_text.tip.size() < 128);

    // 测试版；运行 ID 为 null 时没有。
    const auto beta = parse_update_background_response(response(
        R"({"status":"checked","notify":{"version":{"display":"0.3.1","parts":[0,3,1]},)"
        R"("release_url":"https://github.com/metasequoiaime/msime/releases/tag/windows-v0.3.1-beta",)"
        R"("installer_name":null,"installer_sha256":null,"signed":false,"channel":"beta","build_run":null}})"));
    require(beta.notice && beta.notice->channel == L"beta" && !beta.notice->build_run);
    require(update_notification_text(*beta.notice, L"水杉输入法").body ==
            L"测试版 v0.3.1 已发布。点这里打开设置的「关于」页查看并下载。");

    // 以前的共享层不报通道和运行 ID：按稳定版。
    const auto old = parse_update_background_response(response(
        R"({"status":"checked","notify":{"version":{"display":"0.3.0","parts":[0,3,0]},)"
        R"("release_url":"https://github.com/metasequoiaime/msime/releases/tag/windows-v0.3.0",)"
        R"("installer_name":null,"installer_sha256":null,"signed":false}})"));
    require(old.notice && old.notice->channel == L"stable" && !old.notice->build_run);
    require(update_notification_text(*old.notice, L"水杉输入法").title == L"水杉输入法有新版本 v0.3.0");
    require(update_notification_text(*old.notice, L"水杉输入法").body ==
            L"稳定版 v0.3.0 已发布。点这里打开设置的「关于」页查看并下载。");

    // 版本号不合形状、通道认不出：不提醒，但状态照常。
    const auto odd_version = parse_update_background_response(response(
        R"({"status":"checked","notify":{"version":{"display":"1.4.0 <b>"},"channel":"stable"}})"));
    require(odd_version.status == UpdateCheckStatus::checked && !odd_version.notice);
    const auto odd_channel = parse_update_background_response(response(
        R"({"status":"checked","notify":{"version":{"display":"1.4.0"},"channel":"canary"}})"));
    require(odd_channel.status == UpdateCheckStatus::checked && !odd_channel.notice);
    require(!parse_update_background_response(
                 response(R"({"status":"checked","notify":{"version":{"display":""},"channel":"stable"}})"))
                 .notice);
    require(!parse_update_background_response(
                 response(R"({"status":"checked","notify":{"version":{"display":"1.4.0"},"channel":5}})"))
                 .notice);

    // 发布页不在本仓库的 tag 下时不记，点通知又打不开设置窗口时就不开浏览器。
    const auto foreign = parse_update_background_response(response(
        R"({"status":"checked","notify":{"version":{"display":"1.4.0"},"channel":"stable",)"
        R"("release_url":"https://example.com/releases/tag/windows-v1.4.0"}})"));
    require(foreign.notice && foreign.notice->release_url.empty());
    const auto bare_tag = parse_update_background_response(response(
        R"({"status":"checked","notify":{"version":{"display":"1.4.0"},"channel":"stable",)"
        R"("release_url":"https://github.com/metasequoiaime/msime/releases/tag/"}})"));
    require(bare_tag.notice && bare_tag.notice->release_url.empty());
    const auto spaced_tag = parse_update_background_response(response(
        R"({"status":"checked","notify":{"version":{"display":"1.4.0"},"channel":"stable",)"
        R"("release_url":"https://github.com/metasequoiaime/msime/releases/tag/a b"}})"));
    require(spaced_tag.notice && spaced_tag.notice->release_url.empty());

    // 运行 ID 不是非负整数时当作没有。
    const auto negative_run = parse_update_background_response(response(
        R"({"status":"checked","notify":{"version":{"display":"1.4.0"},"channel":"nightly","build_run":-1}})"));
    require(negative_run.notice && !negative_run.notice->build_run);
    require(update_notification_text(*negative_run.notice, L"水杉输入法").body ==
            L"每日构建 v1.4.0 已发布。点这里打开设置的「关于」页查看并下载。");

    // 节奏：首查晚于启动，之后每小时，通道每分钟看一次；换了通道至少等一个检查间隔，不在用户刚在「关于」页换完通道、看着结果时又弹一条同样的通知。
    require(update_first_check_delay >= std::chrono::minutes(1));
    require(update_check_interval == std::chrono::minutes(60));
    require(update_channel_poll < update_check_interval);
    require(update_channel_settle >= update_check_interval);

    // 系统不接受通知时先不弹，最多等两小时；接受时立刻弹。
    require(update_notice_may_show(true, std::chrono::minutes(0)));
    require(!update_notice_may_show(false, std::chrono::minutes(0)));
    require(!update_notice_may_show(false, update_notice_max_defer - std::chrono::minutes(1)));
    require(update_notice_may_show(false, update_notice_max_defer));
    require(update_notice_retry < update_notice_max_defer);
  } catch (const std::exception &error) {
    std::fprintf(stderr, "%s\n", error.what());
    return 1;
  }
  return 0;
}

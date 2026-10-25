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

    // 请求：普通构建带通道，不带架构；Windows 7 构建平台是 windows7，不带通道（共享层对它一律按稳定版）。
    const auto modern = update_background_request("C:\\Users\\a\\AppData\\Local\\metasequoiaime-full", false,
                                                  "1.4.0", "full", "nightly");
    require(modern == nlohmann::json{{"directory", "C:\\Users\\a\\AppData\\Local\\metasequoiaime-full"},
                                     {"platform", "windows"},
                                     {"current_version", "1.4.0"},
                                     {"edition", "full"},
                                     {"channel", "nightly"}});
    require(update_background_request("C:\\s", false, "1.4.0", "wubi", "unknown").at("channel") == "stable");
    const auto legacy = update_background_request("C:\\s", true, "1.4.0", "full", "beta");
    require(legacy.at("platform") == "windows7");
    require(!legacy.contains("channel"));
    require(!legacy.contains("arch"));

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
    const auto nightly_text = update_notification_text(*nightly.notice);
    require(nightly_text.title == L"水杉输入法有新版本 v1.4.0");
    require(nightly_text.body ==
            L"每日构建 v1.4.0（构建 18800000000）已发布。点这里打开设置的「关于」页查看并下载。");

    // 测试版；运行 ID 为 null 时没有。
    const auto beta = parse_update_background_response(response(
        R"({"status":"checked","notify":{"version":{"display":"0.3.1","parts":[0,3,1]},)"
        R"("release_url":"https://github.com/metasequoiaime/msime/releases/tag/windows-v0.3.1-beta",)"
        R"("installer_name":null,"installer_sha256":null,"signed":false,"channel":"beta","build_run":null}})"));
    require(beta.notice && beta.notice->channel == L"beta" && !beta.notice->build_run);
    require(update_notification_text(*beta.notice).body ==
            L"测试版 v0.3.1 已发布。点这里打开设置的「关于」页查看并下载。");

    // 以前的共享层不报通道和运行 ID：按稳定版。
    const auto old = parse_update_background_response(response(
        R"({"status":"checked","notify":{"version":{"display":"0.3.0","parts":[0,3,0]},)"
        R"("release_url":"https://github.com/metasequoiaime/msime/releases/tag/windows-v0.3.0",)"
        R"("installer_name":null,"installer_sha256":null,"signed":false}})"));
    require(old.notice && old.notice->channel == L"stable" && !old.notice->build_run);
    require(update_notification_text(*old.notice).title == L"水杉输入法有新版本 v0.3.0");
    require(update_notification_text(*old.notice).body ==
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
    require(update_notification_text(*negative_run.notice).body ==
            L"每日构建 v1.4.0 已发布。点这里打开设置的「关于」页查看并下载。");

    // 节奏：首查晚于启动，之后每小时，通道每分钟看一次。
    require(update_first_check_delay >= std::chrono::minutes(1));
    require(update_check_interval == std::chrono::minutes(60));
    require(update_channel_poll < update_check_interval);
  } catch (const std::exception &error) {
    std::fprintf(stderr, "%s\n", error.what());
    return 1;
  }
  return 0;
}

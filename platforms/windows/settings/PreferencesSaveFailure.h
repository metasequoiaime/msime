#pragma once
#include <array>
#include <chrono>
#include <cstddef>
#include <optional>
#include <string>
#include <string_view>

// 设置窗口保存偏好（msime_client_save_preferences）失败时怎么处理。保存按修订号比较并交换，失败分三类，处理各不相同：别处先存了新修订号时重读并说明，保留别人的修改；写盘失败时页面上的修改保留、稍后自动重试；这组设置本身不被接受时重读已存的设置。以前三类一律当成「可能已在其他窗口中更新」去重读，设置文件被杀毒软件或备份工具短暂占用时，用户刚做的修改就这样被旧值换掉了。不依赖 Windows 和 WinRT 头文件，方便单独测试。
namespace msime::settings {

enum class PreferencesSaveFailure {
  // 托盘、共享应用、同步或另一个设置窗口先存了新的修订号。
  conflict,
  // 写盘失败：设置文件被别的程序占用、磁盘出错、取不到存储锁。设置本身没有问题，再存一次可能就成功。
  storage,
  // 这组设置不被接受（取值越界、按键冲突、提交的文档无效），或者返回内容读不了。
  rejected,
};

// 按 host-api 返回的错误文字分类。两句文字是 `PreferencesError` 的 `Conflict` 与 `Io` 的说明，`crates/client-core/src/preferences/tests.rs` 的 `error_wordings_the_harmony_host_matches_on` 钉着它们；写盘失败的说明后面跟着具体原因，所以按前缀认。认不出的一律当成不被接受：重读已存的设置不会丢掉别人的修改。
inline PreferencesSaveFailure classify_preferences_save_failure(std::string_view error) {
  if (error == "preferences changed; reload before saving")
    return PreferencesSaveFailure::conflict;
  constexpr std::string_view storage_prefix = "preferences storage failed:";
  if (error.substr(0, storage_prefix.size()) == storage_prefix)
    return PreferencesSaveFailure::storage;
  return PreferencesSaveFailure::rejected;
}

// 写盘连续失败 failures 次（从 1 起）之后，等多久自动再存一次。共享存储自己已经在约 1 秒内重试过几次，到这里还失败说明占用不是几毫秒的事，所以间隔按秒拉长；四次（合计约 15 秒）之后不再自动重试，改为在下一次修改、切到别的窗口或关闭设置窗口时再试。
inline std::optional<std::chrono::milliseconds> preferences_save_retry_delay(std::size_t failures) {
  constexpr std::array<std::chrono::milliseconds, 4> delays{
      std::chrono::milliseconds{1000}, std::chrono::milliseconds{2000},
      std::chrono::milliseconds{4000}, std::chrono::milliseconds{8000}};
  if (failures == 0 || failures > delays.size())
    return std::nullopt;
  return delays[failures - 1];
}

// 写盘失败时页面顶部的说明。retrying 表示还会自动重试。
inline std::wstring preferences_storage_failure_message(bool retrying) {
  if (retrying)
    return L"这项设置暂时没能保存：设置文件可能正被其他程序占用。页面上的修改已保留，稍后会自动重试保存。";
  return L"这项设置仍未保存：设置文件一直无法写入。页面上的修改还在，再次修改设置、切换到其他窗口或关闭设置窗口时会再试一次。";
}

// 自动重试或之后的某次保存终于成功时的说明。
inline constexpr std::wstring_view preferences_saved_after_retry_message = L"刚才没能保存的修改现已保存。";

// 重读已存设置的两类失败各自的说明；写盘失败不重读，见 preferences_storage_failure_message。
inline std::wstring preferences_reload_failure_message(PreferencesSaveFailure failure) {
  if (failure == PreferencesSaveFailure::conflict)
    return L"无法保存这项设置：设置已在其他窗口中更新。已重新读取当前设置。";
  return L"无法保存这项设置：它与其他设置冲突或取值无效。已重新读取当前设置。";
}

} // namespace msime::settings

#include "../../settings/PreferencesSaveFailure.h"
#include <chrono>
#include <cstdio>
#include <initializer_list>
#include <stdexcept>
#include <string>

using namespace msime::settings;
void require_at(bool value, int line) {
  if (!value)
    throw std::runtime_error("Preferences save failure test failed at line " +
                             std::to_string(line));
}
#define require(...) require_at((__VA_ARGS__), __LINE__)

int main() {
  try {
    // 别处先存了新修订号：只认 `PreferencesError::Conflict` 的整句说明。
    require(classify_preferences_save_failure("preferences changed; reload before saving") ==
            PreferencesSaveFailure::conflict);
    // 写盘失败的说明后面跟着具体原因，按前缀认：Windows 上设置文件被别的程序占用时就是这样。
    require(classify_preferences_save_failure(
                "preferences storage failed: 拒绝访问。 (os error 5)") ==
            PreferencesSaveFailure::storage);
    require(classify_preferences_save_failure(
                "preferences storage failed: 另一个程序正在使用此文件，进程无法访问。 (os error 32)") ==
            PreferencesSaveFailure::storage);
    // 设置本身不被接受、文档无效、返回内容读不了或空错误，都当成不被接受，重读已存的设置。
    require(classify_preferences_save_failure("candidate page size must be between 1 and 10") ==
            PreferencesSaveFailure::rejected);
    require(classify_preferences_save_failure("invalid preferences snapshot") ==
            PreferencesSaveFailure::rejected);
    require(classify_preferences_save_failure("empty host response") ==
            PreferencesSaveFailure::rejected);
    require(classify_preferences_save_failure("") == PreferencesSaveFailure::rejected);
    // 前缀要从头对上，句中出现同样的字不算。
    require(classify_preferences_save_failure("not preferences storage failed: x") ==
            PreferencesSaveFailure::rejected);

    // 自动重试按秒拉长，四次之后不再自动重试。
    using std::chrono::milliseconds;
    require(!preferences_save_retry_delay(0));
    require(preferences_save_retry_delay(1) == milliseconds{1000});
    require(preferences_save_retry_delay(2) == milliseconds{2000});
    require(preferences_save_retry_delay(3) == milliseconds{4000});
    require(preferences_save_retry_delay(4) == milliseconds{8000});
    require(!preferences_save_retry_delay(5));
    require(!preferences_save_retry_delay(100));

    // 写盘失败的说明不提「其他窗口」：那是冲突的说明，写盘失败时没有别人改过设置。重试中和不再自动重试时说法不同，都说修改还在。
    const auto retrying = preferences_storage_failure_message(true);
    const auto exhausted = preferences_storage_failure_message(false);
    require(retrying != exhausted);
    for (const auto &message : {retrying, exhausted}) {
      require(message.find(L"其他窗口中更新") == std::wstring::npos);
      require(message.find(L"修改") != std::wstring::npos);
    }
    require(retrying.find(L"自动重试") != std::wstring::npos);
    // 冲突和不被接受各有自己的说明，都说明已重新读取。
    const auto conflict = preferences_reload_failure_message(PreferencesSaveFailure::conflict);
    const auto rejected = preferences_reload_failure_message(PreferencesSaveFailure::rejected);
    require(conflict != rejected);
    require(conflict.find(L"其他窗口中更新") != std::wstring::npos);
    require(rejected.find(L"其他窗口") == std::wstring::npos);
    for (const auto &message : {conflict, rejected})
      require(message.find(L"已重新读取当前设置") != std::wstring::npos);
    require(!preferences_saved_after_retry_message.empty());
  } catch (const std::exception &error) {
    std::fprintf(stderr, "%s\n", error.what());
    return 1;
  }
  std::puts("Preferences save failure classification passed");
  return 0;
}

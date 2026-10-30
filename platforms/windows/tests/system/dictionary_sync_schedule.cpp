#include "system/DictionarySyncSchedule.h"
#include <cstdio>
#include <stdexcept>
#include <string>

using namespace msime::windows;
void require_at(bool value, int line) {
  if (!value)
    throw std::runtime_error("Dictionary sync schedule test failed at line " + std::to_string(line));
}
#define require(...) require_at((__VA_ARGS__), __LINE__)

namespace {
std::string run_answer(std::uint64_t next_check_ms, const char *error = nullptr) {
  nlohmann::json value{{"network", false}, {"next_check_ms", next_check_ms}, {"status", nlohmann::json::object()}};
  if (error)
    value["error"] = error;
  else
    value["report"] = nlohmann::json::object();
  return nlohmann::json{{"ok", true}, {"value", value}}.dump();
}
constexpr std::uint64_t minute = 60 * 1000;
} // namespace

int main() {
  try {
    // 偏好：只看 dictionary_sync_webdav.url 是不是非空字符串。
    require(!dictionary_sync_configured(nlohmann::json::object()));
    require(!dictionary_sync_configured(nlohmann::json{{"dictionary_sync_webdav", {{"url", ""}}}}));
    require(!dictionary_sync_configured(nlohmann::json{{"dictionary_sync_webdav", {{"url", 1}}}}));
    require(!dictionary_sync_configured(nlohmann::json{{"cloud_clipboard_webdav", {{"url", "https://dav.example.test/"}}}}));
    require(dictionary_sync_configured(
        nlohmann::json{{"dictionary_sync_webdav", {{"url", "https://dav.example.test/"}, {"username", "u"}, {"password", "p"}}}}));

    // 请求：Server 自己的 HostOptions、本用户的同步目录和 run 动作；不带设备名和凭据，只用设置窗口生成的设备身份。
    const nlohmann::json options{{"api_version", 1}, {"preferences_directory", "C:\\ProgramData\\MSIME"}};
    const auto request = dictionary_sync_request(options, "C:\\Users\\a\\AppData\\Local\\MSIME\\dictionary-sync", "local_change");
    require(request == nlohmann::json{{"options", options},
                                      {"directory", "C:\\Users\\a\\AppData\\Local\\MSIME\\dictionary-sync"},
                                      {"create_identity", false},
                                      {"action", {{"operation", "run"}, {"trigger", "local_change"}}}});
    // 设置窗口还没生成身份：同步答 clipboard_identity_missing，按它给的间隔再来，不算身份冲突。
    auto missing = parse_dictionary_sync_answer(
        R"({"ok":true,"value":{"network":false,"next_check_ms":1800000,"status":{},"error":"clipboard_identity_missing"}})");
    require(missing.ok && missing.sync_error == "clipboard_identity_missing" && !dictionary_sync_keystore_conflict(missing));
    require(dictionary_sync_delay(missing) == dictionary_sync_max_delay);

    // 回答：调用失败、同步失败和成功三种形状；读不懂的当失败。
    auto answer = parse_dictionary_sync_answer(R"({"ok":false,"error":"dictionary_sync_not_configured"})");
    require(!answer.ok && answer.error == "dictionary_sync_not_configured" && !answer.next_check_ms);
    require(dictionary_sync_delay(answer) == dictionary_sync_failure_delay);
    answer = parse_dictionary_sync_answer("not json");
    require(!answer.ok && answer.error == "invalid_response");
    answer = parse_dictionary_sync_answer(R"({"ok":true,"value":null})");
    require(!answer.ok && answer.error == "invalid_response");
    answer = parse_dictionary_sync_answer(run_answer(90000, "dictionary_sync_unavailable"));
    require(answer.ok && answer.sync_error == "dictionary_sync_unavailable" && answer.next_check_ms == 90000u);
    require(dictionary_sync_delay(answer) == std::chrono::milliseconds(90000));
    answer = parse_dictionary_sync_answer(run_answer(5));
    require(answer.ok && answer.sync_error.empty() && dictionary_sync_delay(answer) == dictionary_sync_min_delay);
    answer = parse_dictionary_sync_answer(run_answer(24 * 60 * minute));
    require(dictionary_sync_delay(answer) == dictionary_sync_max_delay);

    // 身份冲突：调用本身或同步结果里报 clipboard_keystore_conflict 都算，要重试。
    require(dictionary_sync_keystore_conflict(parse_dictionary_sync_answer(R"({"ok":false,"error":"clipboard_keystore_conflict"})")));
    require(dictionary_sync_keystore_conflict(parse_dictionary_sync_answer(run_answer(1000, "clipboard_keystore_conflict"))));
    require(!dictionary_sync_keystore_conflict(parse_dictionary_sync_answer(run_answer(1000, "clipboard_key_conflict"))));
    int calls = 0;
    const auto retried = dictionary_sync::with_keystore_retry(
        [&] {
          ++calls;
          return parse_dictionary_sync_answer(calls == 1 ? run_answer(1000, "clipboard_keystore_conflict") : run_answer(2000));
        },
        dictionary_sync_keystore_conflict);
    require(calls == 2 && retried.sync_error.empty() && retried.next_check_ms == 2000u);

    // 没启动（预览实例）：什么都不调，改动通知也不记。
    DictionarySyncSchedule idle(true);
    idle.note_local_change();
    require(idle.take(10 * minute).step == DictionarySyncStep::idle && !idle.take(10 * minute).wait);

    // 没配网盘：启动了也不调，改动通知丢掉。
    DictionarySyncSchedule schedule(false);
    schedule.start(0);
    schedule.note_local_change();
    require(schedule.take(60 * minute).step == DictionarySyncStep::idle && !schedule.take(60 * minute).wait);

    // 配上网盘：10 秒后第一次 periodic；之前的改动通知不补记（periodic 本来就核对整个词库）。
    schedule.configure(true, 60 * minute);
    auto decision = schedule.take(60 * minute);
    require(decision.step == DictionarySyncStep::idle && decision.wait == dictionary_sync_configured_delay);
    decision = schedule.take(60 * minute + 10000);
    require(decision.step == DictionarySyncStep::periodic);
    // 一步在跑的时候不再选下一步，改动通知留到它跑完。
    schedule.note_local_change();
    require(schedule.take(60 * minute + 10001).step == DictionarySyncStep::idle);
    schedule.finished(parse_dictionary_sync_answer(run_answer(30 * minute)), 61 * minute);
    // 跑完马上记那次改动；记完照回答里的 next_check_ms（防抖一分钟）排下一次。
    decision = schedule.take(61 * minute);
    require(decision.step == DictionarySyncStep::local_change);
    schedule.note_local_change();
    schedule.finished(parse_dictionary_sync_answer(run_answer(minute)), 61 * minute);
    // 连着几次改动只记一次。
    schedule.note_local_change();
    require(schedule.take(61 * minute).step == DictionarySyncStep::local_change);
    schedule.finished(parse_dictionary_sync_answer(run_answer(minute)), 61 * minute);
    decision = schedule.take(61 * minute);
    require(decision.step == DictionarySyncStep::idle && decision.wait == std::chrono::milliseconds(minute));
    require(schedule.take(62 * minute).step == DictionarySyncStep::periodic);
    // 调用失败（没有 next_check_ms）：半小时后再试，不连着重试。
    schedule.finished(parse_dictionary_sync_answer(R"({"ok":false,"error":"dictionary_sync_storage"})"), 62 * minute);
    decision = schedule.take(62 * minute + 1);
    require(decision.step == DictionarySyncStep::idle && decision.wait == std::chrono::milliseconds(30 * minute - 1));

    // 网盘配置被清掉：停掉定时和待记的改动；再配上时重新从 10 秒后开始。
    schedule.note_local_change();
    schedule.configure(false, 63 * minute);
    require(!schedule.configured());
    require(schedule.take(200 * minute).step == DictionarySyncStep::idle && !schedule.take(200 * minute).wait);
    schedule.configure(true, 200 * minute);
    require(schedule.take(200 * minute).step == DictionarySyncStep::idle);
    require(schedule.take(200 * minute + 10000).step == DictionarySyncStep::periodic);

    // 启动时就配着网盘：两分钟后第一次 periodic。
    DictionarySyncSchedule startup(true);
    startup.start(5);
    decision = startup.take(5);
    require(decision.step == DictionarySyncStep::idle && decision.wait == dictionary_sync_first_delay);
    // 启动后还没到第一次 periodic 就删了词：马上记，不必等。
    startup.note_local_change();
    require(startup.take(6).step == DictionarySyncStep::local_change);
  } catch (const std::exception &error) {
    std::fprintf(stderr, "%s\n", error.what());
    return 1;
  }
  std::puts("dictionary sync schedule: configured preference, run request, answers, keystore conflict retry and trigger scheduling passed");
  return 0;
}

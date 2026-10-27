#pragma once
#include "../../common/DictionarySyncRetry.h"
#include <chrono>
#include <cstdint>
#include <nlohmann/json.hpp>
#include <optional>
#include <string>
#include <string_view>

// Server 里「我的词条」自动同步（DictionarySyncWorker）不碰 Windows API 的部分：偏好里配没配网盘、拼 `msime_client_dictionary_sync` 的请求、读它的回答，以及什么时候以哪个触发方式调用。要不要联网、额度、防抖和退避都在共享 Rust（docs/dictionary-webdav-sync.md 的「请求量」），这里只按回答里的 `next_check_ms` 排下一次。方便在主机上单独测试（tests/system/dictionary_sync_schedule.cpp）。
namespace msime::windows {

// 启动后等多久才第一次 periodic：登录时一起启动的程序很多，晚两分钟不挡开机，也不在用户刚开始打字时读整个「我的词条」。
inline constexpr std::chrono::milliseconds dictionary_sync_first_delay{std::chrono::minutes(2)};
// Rust 给的 next_check_ms 不小于 1 秒；这里再给一个上限，与共享设置页的 dictionarySyncDelay 相同。Rust 本来就让宿主活着时每 30 分钟拉一次。
inline constexpr std::chrono::milliseconds dictionary_sync_min_delay{1000};
inline constexpr std::chrono::milliseconds dictionary_sync_max_delay{std::chrono::minutes(30)};
// 调用本身失败（偏好或本机状态读不出来、回答读不懂，没有 next_check_ms）时多久后再试，与共享设置页相同：不在出错时连着重试。
inline constexpr std::chrono::milliseconds dictionary_sync_failure_delay{std::chrono::minutes(30)};
// 偏好里刚配上网盘（设置窗口测试连接通过并保存）之后多久第一次 periodic。设置窗口自己会以 opened 同步一次，这里只是接上定时。
inline constexpr std::chrono::milliseconds dictionary_sync_configured_delay{std::chrono::seconds(10)};

// 偏好里是否配置了词库同步：`dictionary_sync_webdav.url` 不为空（与 Rust 的 `is_configured` 相同；三项是否合规由 Rust 校验）。
inline bool dictionary_sync_configured(const nlohmann::json &preferences) {
  if (!preferences.is_object())
    return false;
  const auto webdav = preferences.find("dictionary_sync_webdav");
  if (webdav == preferences.end() || !webdav->is_object())
    return false;
  const auto url = webdav->find("url");
  return url != webdav->end() && url->is_string() && !url->get_ref<const std::string &>().empty();
}

// `msime_client_dictionary_sync` 的 run 请求：`options` 是 Server 自己跑的 HostOptions 文档（偏好目录和 Engine 词库路径都从它来），`directory` 是本用户的同步目录（UTF-8，common/DictionarySyncDirectory.h），`trigger` 是 local_change 或 periodic。设备名不带，由 Rust 取本机名称，与设置窗口相同。
inline nlohmann::json dictionary_sync_request(const nlohmann::json &options, const std::string &directory,
                                              std::string_view trigger) {
  return nlohmann::json{{"options", options},
                        {"directory", directory},
                        {"action", {{"operation", "run"}, {"trigger", std::string(trigger)}}}};
}

// 一次调用的回答。`ok` 为假时 `error` 是调用本身的错误码（没配置、请求或本机状态不合规）；`ok` 为真时同步本身也可能失败，那时 `sync_error` 是同步的错误码，next_check_ms 照样给出。
struct DictionarySyncAnswer {
  bool ok = false;
  std::string error;
  std::string sync_error;
  std::optional<std::uint64_t> next_check_ms;
};

inline DictionarySyncAnswer parse_dictionary_sync_answer(std::string_view raw) {
  DictionarySyncAnswer answer;
  const auto document = nlohmann::json::parse(raw, nullptr, false);
  if (document.is_discarded() || !document.is_object()) {
    answer.error = "invalid_response";
    return answer;
  }
  const auto ok = document.find("ok");
  if (ok == document.end() || !ok->is_boolean() || !ok->get<bool>()) {
    const auto error = document.find("error");
    answer.error = error != document.end() && error->is_string() ? error->get<std::string>() : "invalid_response";
    return answer;
  }
  const auto value = document.find("value");
  if (value == document.end() || !value->is_object()) {
    answer.error = "invalid_response";
    return answer;
  }
  answer.ok = true;
  const auto next = value->find("next_check_ms");
  if (next != value->end() && next->is_number_unsigned())
    answer.next_check_ms = next->get<std::uint64_t>();
  const auto error = value->find("error");
  if (error != value->end() && error->is_string())
    answer.sync_error = error->get<std::string>();
  return answer;
}

// 密钥串身份冲突（#10032 的约定，common/DictionarySyncRetry.h）：调用本身或同步结果里报 clipboard_keystore_conflict。
inline bool dictionary_sync_keystore_conflict(const DictionarySyncAnswer &answer) {
  return answer.error == dictionary_sync::keystore_conflict || answer.sync_error == dictionary_sync::keystore_conflict;
}

// 下一次 periodic 在多久之后：回答里有 next_check_ms 就照它（夹在 1 秒到 30 分钟之间），没有就按失败等半小时。
inline std::chrono::milliseconds dictionary_sync_delay(const DictionarySyncAnswer &answer) {
  if (!answer.ok || !answer.next_check_ms)
    return dictionary_sync_failure_delay;
  const auto requested = std::chrono::milliseconds(static_cast<std::int64_t>(
      (std::min)(*answer.next_check_ms, static_cast<std::uint64_t>(dictionary_sync_max_delay.count()))));
  return (std::max)(dictionary_sync_min_delay, requested);
}

// 下一步做什么。
enum class DictionarySyncStep {
  // 不调用：等 `wait` 那么久，或者（没有 `wait` 时）等到有本机改动或配置变化。
  idle,
  // 用户刚改过词：只记录、不联网，Rust 按防抖稍后发送。
  local_change,
  // 到了上一次回答说的时间。
  periodic,
};

struct DictionarySyncDecision {
  DictionarySyncStep step = DictionarySyncStep::idle;
  std::optional<std::chrono::milliseconds> wait;
};

// 工作线程的调度：什么时候、以哪个触发方式调用。时间是调用方给的单调毫秒数（steady_clock），这里不读时钟，测试可以直接推时间。没启动（预览实例、拿不到同步目录）或者偏好里没配网盘时什么都不调，本机改动的通知也丢掉：没配置时 Rust 只会答 dictionary_sync_not_configured，下一次配上之后的 periodic 照样会核对整个「我的词条」，不丢改动。
class DictionarySyncSchedule {
public:
  explicit DictionarySyncSchedule(bool configured = false) : configured_(configured) {}

  // 工作线程起来了：第一次 periodic 在 `dictionary_sync_first_delay` 之后。
  void start(std::uint64_t now) {
    started_ = true;
    due_ = now + static_cast<std::uint64_t>(dictionary_sync_first_delay.count());
  }

  // 偏好变了。从没配到配上：`dictionary_sync_configured_delay` 后 periodic；从配上到没配：停掉定时和待记的改动。配着时网盘换了地址或账号由 Rust 处理（新设备身份），这里照旧按定时走。
  void configure(bool configured, std::uint64_t now) {
    if (configured == configured_)
      return;
    configured_ = configured;
    local_change_ = false;
    if (configured && started_)
      due_ = now + static_cast<std::uint64_t>(dictionary_sync_configured_delay.count());
  }

  // 用户在候选窗口删了词（候选栏删除、维护快捷键）：下一步马上记一次 local_change。连着几次只记一次。
  void note_local_change() {
    if (started_ && configured_)
      local_change_ = true;
  }

  // 现在该做什么。返回 local_change 或 periodic 时就算已经开始了这一步：待记的改动清掉（这一步运行期间又来的改动会再记一次），下一次 periodic 在 `finished` 里按回答重排，没回来之前不会再选 periodic。
  DictionarySyncDecision take(std::uint64_t now) {
    if (!started_ || !configured_ || running_)
      return {};
    if (local_change_) {
      local_change_ = false;
      running_ = true;
      return {DictionarySyncStep::local_change, std::nullopt};
    }
    if (now >= due_) {
      running_ = true;
      return {DictionarySyncStep::periodic, std::nullopt};
    }
    return {DictionarySyncStep::idle, std::chrono::milliseconds(static_cast<std::int64_t>(due_ - now))};
  }

  // 一步做完了：按回答排下一次 periodic。local_change 的回答同样带 next_check_ms（防抖到点就该发送）。
  void finished(const DictionarySyncAnswer &answer, std::uint64_t now) {
    running_ = false;
    due_ = now + static_cast<std::uint64_t>(dictionary_sync_delay(answer).count());
  }

  bool configured() const { return configured_; }

private:
  bool started_ = false;
  bool configured_ = false;
  bool local_change_ = false;
  bool running_ = false;
  std::uint64_t due_ = 0;
};

} // namespace msime::windows

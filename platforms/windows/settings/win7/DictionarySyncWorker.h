#pragma once

#include <windows.h>
#include "PreferencesStore.h"
#include "ThemeData.h"
#include "../../common/DictionarySyncRetry.h"
#include <functional>
#include <string>

namespace msime::settings::win7 {

// Win7 窗口调「我的词条」词库同步（`msime_client_dictionary_sync`）的那一层。`sync` 为空时调 C ABI；原生验收注入合成回答，照样走窗口和工作线程。
struct DictionarySyncServices {
  std::function<std::string(const std::string&)> sync;
};

// 回答是不是密钥串身份冲突（#10032 的约定）：调用本身失败，或者 run 的同步结果里报 clipboard_keystore_conflict。
inline bool dictionary_sync_conflicted(const Json& response) {
  const std::string conflict(msime::windows::dictionary_sync::keystore_conflict);
  if (!response.is_object()) return false;
  if (response.value("ok", false) != true) return response.contains("error") && response.at("error") == conflict;
  const auto value = response.find("value");
  return value != response.end() && value->is_object() && value->contains("error") && value->at("error") == conflict;
}

// 一个动作：`options` 是输入法跑的 HostOptions 文档（没有时为 null，只有「测试连接」不读它），`directory` 是本用户的同步目录（common/DictionarySyncDirectory.h，与 Server 和 WinUI 设置同一个，一个安装只有一份设备身份）。身份冲突时重试。除 status 外都阻塞联网，只在工作线程上调用。
inline Json call_dictionary_sync(const Json& action, const Json& options, const std::string& directory,
    const DictionarySyncServices& services) {
  if (directory.empty()) return Json{{"ok", false}, {"error", "dictionary_sync_storage"}};
  const auto body = Json{{"options", options}, {"directory", directory}, {"action", action}}.dump(-1, ' ', false, Json::error_handler_t::replace);
  return msime::windows::dictionary_sync::with_keystore_retry([&] {
    if (services.sync) return Json::parse(services.sync(body), nullptr, false);
    return theme_data::response(msime_client_dictionary_sync(reinterpret_cast<const uint8_t*>(body.data()), body.size()));
  }, dictionary_sync_conflicted);
}

// 窗口的一件操作：{action, devices?: bool, status?: bool}。先跑动作，`devices` 为真时再取设备，`status` 为真时再读不联网的概况（第一次取设备会登记本机，核对码在那之后才有）。结果是 {action, devices?, status?}，每项都是原样的回答。
inline Json run_dictionary_sync(const Json& job, const Json& options, const std::string& directory,
    const DictionarySyncServices& services) {
  Json completion{{"action", call_dictionary_sync(job.at("action"), options, directory, services)}};
  if (job.value("devices", false)) completion["devices"] = call_dictionary_sync(Json{{"operation", "devices"}}, options, directory, services);
  if (job.value("status", false)) completion["status"] = call_dictionary_sync(Json{{"operation", "status"}}, options, directory, services);
  return completion;
}

// 偏好里是否配置了词库同步：dictionary_sync_webdav.url 不为空（三项是否合规由 Rust 校验）。
inline bool dictionary_sync_configured(const Json& preferences) {
  const auto webdav = preferences.find("dictionary_sync_webdav");
  return webdav != preferences.end() && webdav->is_object() && webdav->contains("url") && webdav->at("url").is_string() &&
      !webdav->at("url").get_ref<const std::string&>().empty();
}

// 经设置改了「我的词条」（编辑、导入、清除学习数据）之后以 local_change 记一次本机改动：只记录、不联网，Rust 按防抖稍后发送。在设置窗口的工作线程上、写完词库之后调用；Server 的自动同步正在联网时要等它放开文件锁。
inline void note_dictionary_sync_change(const Json& options, const std::string& directory) {
  if (!options.is_object() || directory.empty()) return;
  (void)call_dictionary_sync(Json{{"operation", "run"}, {"trigger", "local_change"}}, options, directory, {});
}

} // namespace msime::settings::win7

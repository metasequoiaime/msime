#pragma once

#include "PreferencesStore.h"
#include "SettingsWindow.h"
#include "ThemeData.h"
#include <functional>

namespace msime::settings::win7 {

// 一个操作内坚持同一来源与修订。配置变化或中继、网盘失败都不能转发到另一个来源。
inline Json run_cloud_clipboard(const Json& envelope, const std::string& preferences_directory,
    const std::string& account_directory, const std::function<Json()>& migrate_account,
    const SettingsWindow::CloudClipboardServices& services) {
  const auto failure = [](const char* code) { return Json{{"ok", false}, {"error", code}}; };
  const auto read_source = [&] {
    return theme_data::response(msime_client_cloud_clipboard_source(
        reinterpret_cast<const uint8_t*>(preferences_directory.data()), preferences_directory.size()));
  };
  const auto source_response = read_source();
  const auto source = theme_data::value(source_response);
  Json completion{{"response", source_response}};
  if (!source || !source->is_object() || !source->contains("source") || !source->at("source").is_string() ||
      !source->contains("revision") || !source->at("revision").is_number_unsigned() ||
      (source->at("source") != "account" && source->at("source") != "relay")) {
    if (source) completion["response"] = failure("invalid_cloud_clipboard");
    return completion;
  }
  const auto unchanged = [&] { return theme_data::value(read_source()) == source; };
  completion["source"] = source->at("source"); completion["revision"] = source->at("revision");
  // 中继和 WebDAV 网盘都报 source relay，另带 backend；窗口绑定这三项，比较时不看来源回答里多出来的其他字段。
  Json identity{{"source", source->at("source")}, {"revision", source->at("revision")}};
  if (source->contains("backend") && source->at("backend").is_string()) {
    completion["backend"] = source->at("backend"); identity["backend"] = source->at("backend");
  }
  const auto operation = envelope.at("action").value("operation", std::string{});
  if ((envelope.contains("binding") && envelope.at("binding") != identity) ||
      (!envelope.contains("binding") && operation != "list")) {
    completion["response"] = failure("clipboard_source_changed"); return completion;
  }
  const bool relay = source->at("source") == "relay";
  if (!relay) {
    // 先按正式账号布局迁移会话；中继路径完全不读取账号状态。
    const auto migrated = migrate_account();
    if (!theme_data::value(migrated)) { completion["response"] = migrated; return completion; }
  }
  const auto call = [&](const Json& action) {
    if (!unchanged()) return failure("clipboard_source_changed");
    Json request{{"directory", account_directory}, {"preferences_directory", preferences_directory}, {"action", action}};
    if (relay) request["expected_revision"] = source->at("revision");
    const auto& transport = relay ? services.relay : services.account;
    return transport ? Json::parse(transport(request.dump()), nullptr, false) :
        theme_data::call(relay ? msime_client_relay_cloud_clipboard : msime_client_account_cloud_clipboard, request);
  };
  auto response = call(envelope.at("action"));
  completion["response"] = response;
  if (theme_data::value(response)) {
    completion["list"] = operation == "list" ? response : call(Json{{"operation", "list"}, {"search", envelope.value("search", std::string{})}});
    if (theme_data::value(completion.at("list"))) completion["devices"] = call(Json{{"operation", "devices"}});
  }
  // 外部修改发生在传输期间时也不能把旧来源内容留在窗口中。
  if (!unchanged()) {
    completion["response"] = failure("clipboard_source_changed");
    completion.erase("list"); completion.erase("devices");
  }
  return completion;
}

// WebDAV 的「测试连接」：request 是 {url, username, password}，用的是窗口里还没保存的草稿；account_directory 是中继密钥串所在的账号目录，通过时 Rust 顺带清掉那里的日志缓存，找不到时不带。阻塞网络，只在工作线程上调用。
inline Json probe_cloud_clipboard_webdav(Json request, const std::string& account_directory,
    const SettingsWindow::CloudClipboardServices& services) {
  if (!account_directory.empty()) request["directory"] = account_directory;
  return services.webdav_probe ? Json::parse(services.webdav_probe(request.dump()), nullptr, false) :
      theme_data::call(msime_client_cloud_clipboard_webdav_probe, request);
}
} // namespace msime::settings::win7

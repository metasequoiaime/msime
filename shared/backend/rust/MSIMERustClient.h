#ifndef MSIME_RUST_CLIENT_H
#define MSIME_RUST_CLIENT_H
#include "../../../crates/host-api/include/msime_client.h"
#ifdef __cplusplus
extern "C" {
#endif

/* Apple 宿主交给 shared/backend 的 Rust 函数表（#8201 迁移计划 4a）。macOS 的后端 dylib 不链接 Rust 库，输入法可执行文件 dlopen 它以后经 MSIMEInstallRustClient 交来这张表；iOS App 链着 Rust 静态库，直接拿 C 函数填表。Swift 侧只经这张表调用 msime_client_*，所以 shared/backend 的 swift test 和 macOS 的 dylib 都不需要链接 Rust。 */
#define MSIME_RUST_CLIENT_VERSION 1u

/* msime_client_* 里最常见的形状：length 字节的 UTF-8 JSON 请求，返回要用 string_free 释放的 JSON。 */
typedef char *(*msime_rust_client_json_entry)(const uint8_t *request, size_t length);

/* version 必须是 MSIME_RUST_CLIENT_VERSION，size 必须是 sizeof(msime_rust_client)，否则整张表不收。string_free、session_store_register 和 session_store_unregister 不能为 NULL；其余为 NULL 时 Swift 侧对那个入口报 rust_client_unavailable。表按值复制，函数指针要在进程的整个生命周期里有效。 */
typedef struct msime_rust_client {
  uint32_t version;
  uint32_t size;
  void (*string_free)(char *value);
  char *(*session_store_register)(const msime_client_session_store *store);
  char *(*session_store_unregister)(uint64_t session_store);
  msime_rust_client_json_entry client_agent;
  msime_rust_client_json_entry account;
  char *(*account_chat_stream)(const uint8_t *request, size_t length, msime_client_account_chat_delta_callback on_delta, void *context);
  msime_rust_client_json_entry account_chat_cancel;
  msime_rust_client_json_entry account_gloss;
  msime_rust_client_json_entry account_cloud_clipboard;
  msime_rust_client_json_entry cloud_clipboard_e2e;
  msime_rust_client_json_entry community;
  msime_rust_client_json_entry cloud_dictionary;
  char *(*ai_skin_generate)(const uint8_t *request, size_t length, msime_client_ai_skin_progress_callback progress, void *context);
  msime_rust_client_json_entry ai_skin_cancel;
  msime_rust_client_json_entry ai_skin_job;
  msime_rust_client_json_entry cloud_sync;
  msime_rust_client_json_entry account_settings_export;
  msime_rust_client_json_entry account_settings_apply;
  msime_rust_client_json_entry account_ios_settings_export;
  msime_rust_client_json_entry account_ios_settings_apply;
} msime_rust_client;

/* 用本进程链进来的 msime_client_* 填一张完整的表。只在链接了 Rust 宿主库的代码里调用：macOS 输入法的 input_method_main.mm，以及 iOS App 和测试（Swift 经 CMSIMEClient 调这个内联函数）。macOS 的后端 dylib 和 shared/backend 的 swift test 没有这些符号，不能调用它。 */
static inline msime_rust_client msime_rust_client_linked(void) {
  msime_rust_client table = {
      MSIME_RUST_CLIENT_VERSION,
      (uint32_t)sizeof(msime_rust_client),
      msime_client_string_free,
      msime_client_session_store_register,
      msime_client_session_store_unregister,
      msime_client_client_agent,
      msime_client_account,
      msime_client_account_chat_stream,
      msime_client_account_chat_cancel,
      msime_client_account_gloss,
      msime_client_account_cloud_clipboard,
      msime_client_cloud_clipboard_e2e,
      msime_client_community,
      msime_client_cloud_dictionary,
      msime_client_ai_skin_generate,
      msime_client_ai_skin_cancel,
      msime_client_ai_skin_job,
      msime_client_cloud_sync,
      msime_client_account_settings_export,
      msime_client_account_settings_apply,
      msime_client_account_ios_settings_export,
      msime_client_account_ios_settings_apply,
  };
  return table;
}

#ifdef __cplusplus
}
#endif
#endif

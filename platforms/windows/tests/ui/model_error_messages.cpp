#include "../../settings/ModelErrorMessages.h"
#include <cstdio>

namespace models = msime::settings::models;

int main() {
  struct Case {
    const char *name;
    std::wstring_view response;
    std::wstring_view category;
    std::wstring_view message;
  };
  const Case cases[] = {
      {"network", L"local_model_network: https://models.example.invalid/voice.bin",
       L"local_model_network",
       L"下载失败：无法连接下载服务器。请检查网络，或设置下载镜像后再试。"},
      {"http", L"local_model_http_status: 503", L"local_model_http_status",
       L"下载失败：服务器拒绝了请求。请稍后重试，或更换下载镜像。"},
      {"length", L"local_model_size_mismatch: incomplete-model.bin",
       L"local_model_checksum_mismatch",
       L"下载的文件校验不通过，已丢弃。请重试；若使用了镜像，请确认镜像内容完整。"},
      {"digest", L"local_model_checksum_mismatch: altered-model.gram",
       L"local_model_checksum_mismatch",
       L"下载的文件校验不通过，已丢弃。请重试；若使用了镜像，请确认镜像内容完整。"},
      {"format", L"local_model_format_mismatch: unsupported-model.gram",
       L"local_model_format_mismatch", L"模型文件格式无效或与当前版本不兼容，无法安装。"},
      {"archive", L"local_model_unsafe_archive: entry", L"local_model_invalid_archive",
       L"模型文件内容不完整或不安全，已丢弃。请重试。"},
      {"missing", L"local_model_missing_file: weights.bin", L"local_model_invalid_archive",
       L"模型文件内容不完整或不安全，已丢弃。请重试。"},
      {"mirror", L"local_model_invalid_mirror: http://mirror.example.invalid",
       L"local_model_invalid_mirror", L"下载镜像地址无效，必须以 https:// 开头。"},
      {"storage", L"local_model_io: synthetic write failure", L"local_model_io",
       L"无法写入模型文件，请确认磁盘空间充足。"},
      {"voice busy", L"local_model_install_running: voice-model", L"busy",
       L"该模型正在下载，请等待完成或先取消。"},
      {"resource busy", L"resource_pack_busy: grammar-model", L"busy",
       L"该模型正在下载，请等待完成或先取消。"},
      {"normalized busy", L"busy", L"busy", L"该模型正在下载，请等待完成或先取消。"},
      {"root", L"local_model_invalid_root: synthetic-directory", L"local_model_invalid_root",
       L"无法访问模型存放目录。"},
      {"unavailable", L"unavailable", L"unavailable", L"无法访问模型存放目录。"},
      {"unknown model", L"local_model_unknown: example-model", L"local_model_unknown",
       L"未知的模型。"},
      {"unrecognized", L"future_model_failure: diagnostic detail", L"future_model_failure",
       L"操作失败，请重试。"},
      {"empty", L"", L"", L"操作失败，请重试。"},
      {"empty category", L": diagnostic detail", L"", L"操作失败，请重试。"},
      {"similar cancellation", L"local_model_cancelled_late: detail",
       L"local_model_cancelled_late", L"操作失败，请重试。"},
  };
  int failures = 0;
  for (const auto &test : cases) {
    const auto category = models::error_code(test.response);
    const auto message = models::error_message(category);
    if (category != test.category || !message || *message != test.message) {
      std::fprintf(stderr, "FAIL: %s\n", test.name);
      ++failures;
    }
  }
  for (const auto response : {L"local_model_cancelled", L"local_model_cancelled: user action"}) {
    if (models::error_message(models::error_code(response))) {
      std::fprintf(stderr, "FAIL: cancellation must not produce an error message\n");
      ++failures;
    }
  }
  if (failures) return 1;
  std::puts("model error messages: ok");
  return 0;
}

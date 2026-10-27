#pragma once
#include <optional>
#include <string>
#include <string_view>

namespace msime::settings::models {

// 与 apps/desktop/src-tauri/src/voice/local_models.rs 的 local_model_error_code 保持分类一致。
// 仅保留错误标识，地址、路径等底层诊断详情不参与用户提示。
inline std::wstring error_code(std::wstring_view error) {
  const auto colon = error.find(L':');
  const auto code = colon == std::wstring_view::npos ? error : error.substr(0, colon);
  if (code == L"local_model_install_running" || code == L"resource_pack_busy")
    return L"busy";
  if (code == L"local_model_size_mismatch" || code == L"local_model_checksum_mismatch" ||
      code == L"local_model_format_mismatch")
    return L"local_model_checksum_mismatch";
  if (code == L"local_model_unsafe_archive" || code == L"local_model_missing_file")
    return L"local_model_invalid_archive";
  return std::wstring(code);
}

// 接收归一化后的错误码；取消返回空结果，界面可自行展示非错误状态。
inline std::optional<std::wstring> error_message(std::wstring_view code) {
  if (code == L"local_model_cancelled")
    return std::nullopt;
  if (code == L"local_model_network")
    return L"下载失败：无法连接下载服务器。请检查网络，或设置下载镜像后再试。";
  if (code == L"local_model_http_status")
    return L"下载失败：服务器拒绝了请求。请稍后重试，或更换下载镜像。";
  if (code == L"local_model_checksum_mismatch")
    return L"下载的文件校验不通过，已丢弃。请重试；若使用了镜像，请确认镜像内容完整。";
  if (code == L"local_model_invalid_archive")
    return L"模型文件内容不完整或不安全，已丢弃。请重试。";
  if (code == L"local_model_invalid_mirror")
    return L"下载镜像地址无效，必须以 https:// 开头。";
  if (code == L"local_model_io")
    return L"无法写入模型文件，请确认磁盘空间充足。";
  if (code == L"busy")
    return L"该模型正在下载，请等待完成或先取消。";
  if (code == L"local_model_unknown")
    return L"未知的模型。";
  if (code == L"local_model_invalid_root" || code == L"unavailable")
    return L"无法访问模型存放目录。";
  return L"操作失败，请重试。";
}

} // namespace msime::settings::models

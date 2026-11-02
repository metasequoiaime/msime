#pragma once
#include <filesystem>
#include <nlohmann/json.hpp>

namespace msime::windows {
// 在会话选项里写上两档 ONNX 逐键重排模型所在的目录：Server 程序所在目录（安装后是 Program Files 下的 server），安装器把 rerank-model 和 rerank-model-small 从 server_exe 装到那里（installer/Prepare-PackageFiles.ps1）。不写时共享库在随包资源目录的上一级找，而 Windows 会话里的随包资源目录（helpcode_fallback_resources）是放辅助码表的 DataDir，装了在线词库更新后 resources 又换成 DataDir 下的代次，都推不到 Server 旁边。目录不在（开发运行、不带模型的版本）时也照样写上：共享库在那里找不到模型，退回资源目录里随包的句子模型。选项里已经有这一项时以它为准；Server 目录不是绝对路径或文档不是对象时不动它。只在内存里写，不写进 runtime-options.json。
inline void name_rerank_models(nlohmann::json &options,
                               const std::filesystem::path &server_directory) {
  if (!options.is_object() || options.contains("rerank_models") ||
      !server_directory.is_absolute())
    return;
  options["rerank_models"] = server_directory.u8string();
}
} // namespace msime::windows

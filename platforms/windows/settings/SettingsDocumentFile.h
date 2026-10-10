#pragma once
#include <cstddef>
#include <filesystem>
#include <fstream>
#include <optional>
#include <string>
#include <string_view>
#include <system_error>

// 「维护与诊断」页的「导出设置」「导入设置」和「关于」页的「第三方组件许可」用到的文件规则。换算设置文件的规则在 host-api（msime_client_export_settings / msime_client_import_settings，规则本身在 crates/client-core/src/settings_document.rs），这里只有读写文件、把错误码换成说明和定位随包文件；不依赖 Windows 和 WinRT 头文件，方便单独测试。
namespace msime::settings {

// 设置文件的大小上限，与 crates/client-core/src/settings_document.rs 的 MAX_DOCUMENT_BYTES 相同。
inline constexpr std::size_t settings_document_limit = std::size_t{1} << 20;

// msime_client_import_settings 失败时的错误码换成给用户看的说明。认不出的错误码（存储错误）一律说成写入失败：导入失败时 host-api 什么也没写，原有设置不变。
inline std::wstring settings_import_error_message(std::string_view code) {
  if (code == "settings_document_invalid")
    return L"无法导入这个文件：它不是水杉输入法导出的设置文件。";
  if (code == "settings_document_macos")
    return L"无法导入这个文件：它是 macOS 版设置窗口导出的文件，其中的设置 Windows 版无法读取。";
  if (code == "settings_document_unsupported")
    return L"无法导入这个文件：文件里的设置无法识别，可能来自更新版本的水杉输入法，或者已经被改动过。";
  if (code == "settings_conflict")
    return L"设置没有导入：设置刚在其他窗口中更新过，已重新读取当前设置，请再导入一次。";
  return L"设置没有导入：无法写入设置，请稍后重试。原有设置保持不变。";
}

// 安装器把 THIRD_PARTY_NOTICES.txt 装在安装目录（{commonpf64}\<安装目录>）下，设置窗口和 MSIME.exe 都在它的 server 子目录里，所以从 server 目录里任何一个可执行文件往上一级就是。不是绝对路径时返回空。
inline std::filesystem::path third_party_notices_path(std::filesystem::path const &executable) {
  if (!executable.is_absolute())
    return {};
  return executable.parent_path().parent_path() / L"THIRD_PARTY_NOTICES.txt";
}

// 读出用户选的设置文件。打不开、读失败或超过 limit 时返回空，调用方据此说明这不是能导入的文件，不把截断的内容交给 host-api。
inline std::optional<std::string> read_settings_document(std::filesystem::path const &path,
                                                         std::size_t limit = settings_document_limit) {
  std::ifstream input(path, std::ios::binary);
  if (!input)
    return std::nullopt;
  std::string contents(limit + 1, '\0');
  input.read(contents.data(), static_cast<std::streamsize>(contents.size()));
  if (input.bad())
    return std::nullopt;
  const auto count = static_cast<std::size_t>(input.gcount());
  if (count > limit)
    return std::nullopt;
  contents.resize(count);
  return contents;
}

// 写出导出的设置文件：先写同目录下的临时文件，写完再改名替换目标，中途失败时用户原来的那个文件（覆盖导出时）保持完整。失败时删掉临时文件并返回假。
inline bool write_settings_document(std::filesystem::path const &target, std::string_view contents) {
  if (!target.is_absolute() || !target.has_filename())
    return false;
  auto temporary = target;
  temporary += L".msime-export.tmp";
  std::error_code error;
  {
    std::ofstream output(temporary, std::ios::binary | std::ios::trunc);
    if (!output)
      return false;
    output.write(contents.data(), static_cast<std::streamsize>(contents.size()));
    output.flush();
    if (!output) {
      output.close();
      std::filesystem::remove(temporary, error);
      return false;
    }
  }
  std::filesystem::rename(temporary, target, error);
  if (error) {
    std::filesystem::remove(temporary, error);
    return false;
  }
  return true;
}

} // namespace msime::settings

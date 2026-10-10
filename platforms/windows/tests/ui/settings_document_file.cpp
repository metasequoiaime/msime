#include "../../settings/SettingsDocumentFile.h"
#include <cstdio>
#include <filesystem>
#include <fstream>
#include <set>
#include <stdexcept>
#include <string>

using namespace msime::settings;
void require_at(bool value, int line) {
  if (!value)
    throw std::runtime_error("Settings document file test failed at line " +
                             std::to_string(line));
}
#define require(...) require_at((__VA_ARGS__), __LINE__)

int main() {
  try {
    // 每个错误码各有自己的说明，认不出的错误码（存储错误）说成写入失败且原有设置不变。
    const std::set<std::wstring> messages{
        settings_import_error_message("settings_document_invalid"),
        settings_import_error_message("settings_document_macos"),
        settings_import_error_message("settings_document_unsupported"),
        settings_import_error_message("settings_conflict"),
        settings_import_error_message("preferences document is not a regular file"),
    };
    require(messages.size() == 5);
    require(settings_import_error_message("") ==
            settings_import_error_message("something the store said"));
    require(settings_import_error_message("settings_document_macos").find(L"macOS") !=
            std::wstring::npos);

    // 随包的许可声明在 server 目录的上一级，与安装器的 DestDir 一致。
    const auto base = std::filesystem::temp_directory_path() / "msime-settings-document-test";
    std::error_code ignored;
    std::filesystem::remove_all(base, ignored);
    std::filesystem::create_directories(base / "server");
    const auto executable = base / "server" / "msime-client-settings.exe";
    require(third_party_notices_path(executable) == base / "THIRD_PARTY_NOTICES.txt");
    require(third_party_notices_path(std::filesystem::path("server") / "MSIME.exe").empty());

    // 写出后原样读回；覆盖已有文件，不留临时文件。
    const auto target = base / "settings.json";
    {
      std::ofstream existing(target, std::ios::binary);
      existing << "older export";
    }
    const std::string document = "{\"format\":\"app.msime.client.preferences\"}";
    require(write_settings_document(target, document));
    require(read_settings_document(target) == document);
    std::size_t entries = 0;
    for ([[maybe_unused]] auto const &entry : std::filesystem::directory_iterator(base))
      ++entries;
    require(entries == 2);

    // 超过上限的文件不读，读不到的文件也不读，不把截断的内容交出去。
    require(read_settings_document(target, document.size()) == document);
    require(!read_settings_document(target, document.size() - 1));
    require(!read_settings_document(base / "missing.json"));
    require(!write_settings_document(std::filesystem::path("relative.json"), document));
    require(!write_settings_document(base / "no-such-directory" / "settings.json", document));

    std::filesystem::remove_all(base, ignored);
    std::puts("Settings document file tests passed");
    return 0;
  } catch (std::exception const &error) {
    std::fprintf(stderr, "%s\n", error.what());
    return 1;
  }
}

#include "../../HostOptionsPaths.h"
#include <windows.h>
#include <chrono>
#include <cstdlib>
#include <fstream>

int main() {
    const auto root = std::filesystem::temp_directory_path() /
        ("msime-prepared-options-" + std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
    if (!std::filesystem::create_directory(root)) return EXIT_FAILURE;
    const auto path = root / "runtime-options.json";
    const auto write = [&](const std::string &text) {
        std::ofstream stream(path, std::ios::binary | std::ios::trunc);
        stream.write(text.data(), static_cast<std::streamsize>(text.size()));
        if (!stream) std::abort();
    };
    bool ok = msime::tsf::read_prepared_host_options(path).empty();
    // Synthetic fixture: schema/Engine validation belongs to the C ABI.
    const std::string document = R"({"api_version":1,"preferences":{"scheme":"shuangpin","shuangpin_profile":"microsoft","candidate_page_size":7,"learning":false,"chinese_punctuation":false},"preferences_directory":"synthetic-state"})";
    write(document);
    ok = ok && msime::tsf::read_prepared_host_options(path) == document;
    // 五笔版本与双拼方案一样是 `preferences` 里的同级字段，读取时原样保留，交给 C ABI 校验。
    const std::string wubi_document = R"({"api_version":1,"preferences":{"scheme":"wubi","wubi_profile":"wubi98"},"preferences_directory":"synthetic-state"})";
    write(wubi_document);
    ok = ok && msime::tsf::read_prepared_host_options(path) == wubi_document;
    write("");
    ok = ok && msime::tsf::read_prepared_host_options(path).empty();
    write(std::string(16384, ' '));
    ok = ok && msime::tsf::read_prepared_host_options(path).size() == 16384;
    write(std::string(16385, ' '));
    ok = ok && msime::tsf::read_prepared_host_options(path).empty();

    const auto outside = root.parent_path() / (root.filename().wstring() + L"-outside");
    std::filesystem::create_directory(outside);
    const auto outside_file = outside / L"runtime-options.json";
    {
        std::ofstream stream(outside_file, std::ios::binary);
        stream << document;
    }
    const auto linked_leaf = root / L"linked-runtime-options.json";
    if (CreateSymbolicLinkW(linked_leaf.c_str(), outside_file.c_str(), 0)) {
        ok = ok && msime::tsf::read_prepared_host_options(linked_leaf).empty();
        std::filesystem::remove(linked_leaf);
    }
    const auto linked_directory = root / L"linked-directory";
    if (CreateSymbolicLinkW(linked_directory.c_str(), outside.c_str(),
                            SYMBOLIC_LINK_FLAG_DIRECTORY)) {
        ok = ok && msime::tsf::read_prepared_host_options(
                         linked_directory / L"runtime-options.json")
                         .empty();
        std::filesystem::remove(linked_directory);
    }
    std::filesystem::remove_all(outside);
    std::filesystem::remove(path);
    std::filesystem::remove(root);
    return ok ? EXIT_SUCCESS : EXIT_FAILURE;
}

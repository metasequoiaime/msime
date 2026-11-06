#pragma once
#include <windows.h>
#include <filesystem>

namespace msime::settings {
void show_local_translation(HWND owner, const std::filesystem::path &directory);
}

#include <algorithm>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <memory>
#include <optional>
#include <string>
#include "Define.h"
#include "Globals.h"
#include "FanyUtils.h"
#include "Ipc.h"
#include "../HostOptionsPaths.h"
#include "../../common/InputSchemeTraits.h"
#include <msime_client.h>
#include <nlohmann/json.hpp>
#include <utf8cpp/utf8.h>
#include <fmt/xchar.h>

using namespace std;

namespace FanyUtils
{
std::string GetIMEDataDirPath()
{
    return msime::tsf::default_state_directory();
}

namespace
{
std::optional<nlohmann::json> ReadSharedPreferences()
{
    const std::string directory = msime::tsf::default_state_directory();
    if (directory.empty())
    {
        return std::nullopt;
    }
    std::unique_ptr<char, decltype(&msime_client_string_free)> raw(
        msime_client_load_preferences(reinterpret_cast<const uint8_t *>(directory.data()), directory.size()),
        msime_client_string_free);
    if (!raw)
    {
        return std::nullopt;
    }
    try
    {
        const auto envelope = nlohmann::json::parse(raw.get());
        if (!envelope.value("ok", false) || !envelope.contains("value") || !envelope["value"].is_object())
        {
            return std::nullopt;
        }
        const auto &value = envelope["value"];
        if (!value.contains("preferences") || !value["preferences"].is_object())
        {
            return std::nullopt;
        }
        return value["preferences"];
    }
    catch (...)
    {
        return std::nullopt;
    }
}

std::string TrimAscii(const std::string &value)
{
    size_t begin = 0;
    while (begin < value.size() && (value[begin] == ' ' || value[begin] == '\t' || value[begin] == '\r'))
    {
        ++begin;
    }
    size_t end = value.size();
    while (end > begin && (value[end - 1] == ' ' || value[end - 1] == '\t' || value[end - 1] == '\r'))
    {
        --end;
    }
    return value.substr(begin, end - begin);
}

std::string UnquoteTomlBasicString(const std::string &value)
{
    if (value.size() >= 2 && value.front() == '"' && value.back() == '"')
    {
        return value.substr(1, value.size() - 2);
    }
    return value;
}

bool ParseTomlBool(const std::string &raw, bool fallback)
{
    const std::string value = to_lower_copy(UnquoteTomlBasicString(TrimAscii(raw)));
    if (value == "true" || value == "1")
    {
        return true;
    }
    if (value == "false" || value == "0")
    {
        return false;
    }
    return fallback;
}

std::filesystem::path SharedConfigPath()
{
    // Build a wide path and open it as such. A narrow std::string path would be opened through the
    // ANSI code page, which cannot round-trip a non-ASCII (e.g. Chinese) user profile path on a
    // non-UTF-8 system, so the TSF would read the wrong file or fail to find the config.
    const auto state = msime::tsf::default_state_directory();
    return state.empty() ? std::filesystem::path{} :
                           std::filesystem::u8path(state) / L"config.toml";
}
} // namespace

BOOL ReadConfiguredDefaultImeModeChinese()
{
    if (const auto preferences = ReadSharedPreferences())
    {
        return preferences->value("default_ime_mode", std::string{"chinese"}) != "english";
    }
    const std::filesystem::path configPath = SharedConfigPath();
    if (configPath.empty())
    {
        return TRUE;
    }

    std::ifstream input(configPath);
    if (!input)
    {
        return TRUE;
    }

    bool inInputSection = false;
    std::string line;
    while (std::getline(input, line))
    {
        const size_t comment = line.find('#');
        if (comment != std::string::npos)
        {
            line = line.substr(0, comment);
        }
        line = TrimAscii(line);
        if (line.empty())
        {
            continue;
        }
        if (line.front() == '[' && line.back() == ']')
        {
            inInputSection = (line == "[input]");
            continue;
        }
        if (!inInputSection)
        {
            continue;
        }
        const size_t eq = line.find('=');
        if (eq == std::string::npos)
        {
            continue;
        }
        const std::string key = TrimAscii(line.substr(0, eq));
        if (key != "default_ime_mode")
        {
            continue;
        }
        const std::string value = to_lower_copy(UnquoteTomlBasicString(TrimAscii(line.substr(eq + 1))));
        return value != "english";
    }
    return TRUE;
}

int ReadConfiguredPunctuationLock()
{
    if (const auto preferences = ReadSharedPreferences())
    {
        const auto value = preferences->value("punctuation_lock", std::string{"follow"});
        if (value == "chinese")
        {
            return Global::PunctuationLock::AlwaysChinese;
        }
        if (value == "english")
        {
            return Global::PunctuationLock::AlwaysEnglish;
        }
        return Global::PunctuationLock::Follow;
    }
    const std::filesystem::path configPath = SharedConfigPath();
    if (configPath.empty())
    {
        return Global::PunctuationLock::Follow;
    }

    std::ifstream input(configPath);
    if (!input)
    {
        return Global::PunctuationLock::Follow;
    }

    bool inInputSection = false;
    std::string line;
    while (std::getline(input, line))
    {
        const size_t comment = line.find('#');
        if (comment != std::string::npos)
        {
            line = line.substr(0, comment);
        }
        line = TrimAscii(line);
        if (line.empty())
        {
            continue;
        }
        if (line.front() == '[' && line.back() == ']')
        {
            inInputSection = (line == "[input]");
            continue;
        }
        if (!inInputSection)
        {
            continue;
        }
        const size_t eq = line.find('=');
        if (eq == std::string::npos)
        {
            continue;
        }
        const std::string key = TrimAscii(line.substr(0, eq));
        if (key != "punctuation_lock")
        {
            continue;
        }
        const std::string value = to_lower_copy(UnquoteTomlBasicString(TrimAscii(line.substr(eq + 1))));
        if (value == "chinese")
        {
            return Global::PunctuationLock::AlwaysChinese;
        }
        if (value == "english")
        {
            return Global::PunctuationLock::AlwaysEnglish;
        }
        return Global::PunctuationLock::Follow;
    }
    return Global::PunctuationLock::Follow;
}

void RefreshPunctuationLockFromConfig()
{
    Global::PunctuationLockMode.store(ReadConfiguredPunctuationLock(), std::memory_order_relaxed);
}

namespace
{
// The legacy config.toml `[input] mode`, lower-cased, or empty when the file or the key is missing.
std::string ReadLegacyConfiguredInputMode()
{
    const std::filesystem::path configPath = SharedConfigPath();
    if (configPath.empty())
    {
        return {};
    }

    std::ifstream input(configPath);
    if (!input)
    {
        return {};
    }

    bool inInputSection = false;
    std::string line;
    while (std::getline(input, line))
    {
        const size_t comment = line.find('#');
        if (comment != std::string::npos)
        {
            line = line.substr(0, comment);
        }
        line = TrimAscii(line);
        if (line.empty())
        {
            continue;
        }
        if (line.front() == '[' && line.back() == ']')
        {
            inInputSection = (line == "[input]");
            continue;
        }
        if (!inInputSection)
        {
            continue;
        }
        const size_t eq = line.find('=');
        if (eq == std::string::npos)
        {
            continue;
        }
        const std::string key = TrimAscii(line.substr(0, eq));
        if (key != "mode")
        {
            continue;
        }
        return to_lower_copy(UnquoteTomlBasicString(TrimAscii(line.substr(eq + 1))));
    }
    return {};
}
} // namespace

std::string ReadConfiguredInputScheme()
{
    if (const auto preferences = ReadSharedPreferences())
    {
        return preferences->value("scheme", std::string{"quanpin"});
    }
    return ReadLegacyConfiguredInputMode();
}

int ReadConfiguredRunningScheme()
{
    const std::string configured = ReadConfiguredInputScheme();
    std::string lastChinese;
    if (const auto preferences = ReadSharedPreferences())
    {
        const auto last = preferences->find("last_chinese_scheme");
        if (last != preferences->end() && last->is_string())
        {
            lastChinese = last->get<std::string>();
        }
    }
    msime::windows::scheme::LanguageDictionaryPresence installed;
    const auto options = nlohmann::json::parse(msime::tsf::default_host_options_json(), nullptr, false);
    if (options.is_object())
    {
        const auto directory = options.find("language_dictionaries");
        if (directory != options.end() && directory->is_string())
        {
            const auto path = std::filesystem::u8path(directory->get<std::string>());
            std::error_code ec;
            installed.cantonese = std::filesystem::is_regular_file(path / "cantonese.db", ec);
            installed.zhuyin = std::filesystem::is_regular_file(path / "zhuyin.db", ec);
        }
    }
    return msime::windows::scheme::effective_scheme(configured, lastChinese, installed);
}

BOOL ReadConfiguredJapaneseInputMode()
{
    return ReadConfiguredInputScheme() == "japanese";
}

SwitchLanguageHotkeys ReadConfiguredSwitchLanguageHotkeys()
{
    SwitchLanguageHotkeys result;
    if (const auto preferences = ReadSharedPreferences())
    {
        const auto keybindings = preferences->value("keybindings", nlohmann::json::object());
        if (keybindings.is_object())
        {
            result.shift = keybindings.value("switch_language_shift", true);
            result.ctrl = keybindings.value("switch_language_ctrl", false);
            result.ctrl_alt_space = keybindings.value("switch_language_ctrl_alt_space", true);
            result.character_set_ctrl_shift_f = keybindings.value("toggle_character_set_ctrl_shift_f", true);
            return result;
        }
    }
    const std::filesystem::path configPath = SharedConfigPath();
    if (configPath.empty())
    {
        return result;
    }

    std::ifstream input(configPath);
    if (!input)
    {
        return result;
    }

    bool inKeybindings = false;
    bool sawShift = false;
    bool sawCtrl = false;
    bool sawCtrlAltSpace = false;
    std::string line;
    while (std::getline(input, line))
    {
        const size_t comment = line.find('#');
        if (comment != std::string::npos)
        {
            line = line.substr(0, comment);
        }
        line = TrimAscii(line);
        if (line.empty())
        {
            continue;
        }
        if (line.front() == '[' && line.back() == ']')
        {
            inKeybindings = (line == "[keybindings]");
            continue;
        }
        if (!inKeybindings)
        {
            continue;
        }
        const size_t eq = line.find('=');
        if (eq == std::string::npos)
        {
            continue;
        }
        const std::string key = TrimAscii(line.substr(0, eq));
        const std::string raw = line.substr(eq + 1);
        if (key == "switch_language_shift")
        {
            result.shift = ParseTomlBool(raw, true);
            sawShift = true;
        }
        else if (key == "switch_language_ctrl")
        {
            result.ctrl = ParseTomlBool(raw, false);
            sawCtrl = true;
        }
        else if (key == "switch_language_ctrl_alt_space")
        {
            result.ctrl_alt_space = ParseTomlBool(raw, true);
            sawCtrlAltSpace = true;
        }
        else if (key == "toggle_character_set_ctrl_shift_f")
        {
            result.character_set_ctrl_shift_f = ParseTomlBool(raw, true);
        }
        else if (key == "switch_language" && !sawShift && !sawCtrlAltSpace)
        {
            // Legacy array: switch_language = ["Ctrl+Space", "Shift"]
            result.shift = raw.find("Shift") != std::string::npos;
            result.ctrl_alt_space =
                raw.find("Ctrl+Alt+Space") != std::string::npos || raw.find("Ctrl+Space") != std::string::npos;
        }
    }
    (void)sawCtrl;
    return result;
}

void SendKeys(std::wstring pinyin)
{
    for (wchar_t ch : pinyin)
    {
        INPUT in[2]{};

        in[0].type = INPUT_KEYBOARD;
        in[0].ki.wScan = ch;
        in[0].ki.dwFlags = KEYEVENTF_UNICODE;

        in[1] = in[0];
        in[1].ki.dwFlags |= KEYEVENTF_KEYUP;

        UINT sent = SendInput(2, in, sizeof(INPUT));
        if (sent != 2)
        {
        }
    }
}

std::wstring string_to_wstring(const std::string &str)
{
    std::u16string utf16result;
    utf8::utf8to16(str.begin(), str.end(), std::back_inserter(utf16result));
    return std::wstring(utf16result.begin(), utf16result.end());
}

std::string wstring_to_string(const std::wstring &wstr)
{
    std::string result;
    utf8::utf16to8(wstr.begin(), wstr.end(), std::back_inserter(result));
    return result;
}

std::string to_lower_copy(const std::string &str)
{
    std::string result = str;
    std::transform(result.begin(), result.end(), result.begin(), [](unsigned char c) { return std::tolower(c); });
    return result;
}

std::wstring GetCurrentProcessName()
{
    TCHAR fullPath[MAX_PATH] = {0};
    if (GetModuleFileName(NULL, fullPath, MAX_PATH) == 0)
        return L"";

    std::wstring wfullPath(fullPath);
    size_t pos = wfullPath.find_last_of(L"\\/");
    std::wstring wname = (pos != std::wstring::npos) ? wfullPath.substr(pos + 1) : wfullPath;
    return wname;
}

/**
 * @brief Count UTF-8 chars
 *
 * @param str
 * @return string::size_type
 */
string::size_type count_utf8_chars(const string &str)
{
    return utf8::distance(str.begin(), str.end());
}
} // namespace FanyUtils

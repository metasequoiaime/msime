#include <algorithm>
#include <cstdlib>
#include <filesystem>
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
} // namespace

BOOL ReadConfiguredDefaultImeModeChinese()
{
    if (const auto preferences = ReadSharedPreferences())
    {
        return preferences->value("default_ime_mode", std::string{"chinese"}) != "english";
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
    return Global::PunctuationLock::Follow;
}

void RefreshPunctuationLockFromConfig()
{
    Global::PunctuationLockMode.store(ReadConfiguredPunctuationLock(), std::memory_order_relaxed);
}

std::string ReadConfiguredInputScheme()
{
    if (const auto preferences = ReadSharedPreferences())
    {
        return preferences->value("scheme", std::string{"quanpin"});
    }
    return "quanpin";
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
            installed.cantonese = std::filesystem::is_regular_file(path / "msime-cantonese.db", ec);
            installed.zhuyin = std::filesystem::is_regular_file(path / "msime-zhuyin.db", ec);
            installed.stroke = std::filesystem::is_regular_file(path / "msime-stroke.db", ec);
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

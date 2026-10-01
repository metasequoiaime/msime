#pragma once
#include <Windows.h>
#include <string>

namespace FanyUtils
{
std::string GetIMEDataDirPath();
void SendKeys(std::wstring pinyin);
std::wstring string_to_wstring(const std::string &str);
std::string wstring_to_string(const std::wstring &wstr);
std::string to_lower_copy(const std::string &str);
std::wstring GetCurrentProcessName();
std::string::size_type count_utf8_chars(const std::string &str);
// Read default_ime_mode from the shared MSIME-Client PreferencesStore.
// A legacy metasequoiaime/config.toml fallback is retained for upgrades.
// Returns TRUE for Chinese (default), FALSE for English.
BOOL ReadConfiguredDefaultImeModeChinese();
// Read the active scheme from shared PreferencesStore. TRUE when Japanese input
// is active. The legacy TOML file remains a compatibility fallback.
BOOL ReadConfiguredJapaneseInputMode();
// The active scheme id from the shared PreferencesStore ("quanpin" when it names none), or the lower-cased legacy TOML `[input] mode` when the store is unreadable. One read answers both the Japanese and the Korean input mode.
std::string ReadConfiguredInputScheme();
// The scheme number the Engine runs for the configured scheme, as host-api's effective_scheme picks it: Cantonese and Zhuyin need their dictionary in the `language_dictionaries` directory the prepared HostOptions names, and fall back to `last_chinese_scheme` or quanpin without it.
int ReadConfiguredRunningScheme();
// Read punctuation_lock from shared PreferencesStore (with legacy TOML fallback).
// 0 = follow IME, 1 = always Chinese punctuation, 2 = always English punctuation.
int ReadConfiguredPunctuationLock();
// Refresh the in-process lock cache from config.toml.
void RefreshPunctuationLockFromConfig();

struct SwitchLanguageHotkeys
{
    bool shift = true;
    bool ctrl = false;
    bool ctrl_alt_space = true;
    bool character_set_ctrl_shift_f = true;
};
// Read keybindings.switch_language_* from shared PreferencesStore (with legacy
// TOML fallback).
SwitchLanguageHotkeys ReadConfiguredSwitchLanguageHotkeys();
} // namespace FanyUtils

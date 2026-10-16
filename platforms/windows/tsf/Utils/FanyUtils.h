#pragma once
#include <Windows.h>
#include <optional>
#include <string>
#include <vector>

namespace FanyUtils
{
std::string GetIMEDataDirPath();
void SendKeys(std::wstring pinyin);
std::wstring string_to_wstring(const std::string &str);
std::string wstring_to_string(const std::wstring &wstr);
std::string to_lower_copy(const std::string &str);
std::wstring GetCurrentProcessName();
std::string::size_type count_utf8_chars(const std::string &str);
// 从已发布快照读取 default_ime_mode；中文或尚无有效快照时返回 TRUE，英文返回 FALSE。
BOOL ReadConfiguredDefaultImeModeChinese();
// 从内存快照取 app_input_mode_rules 中这个进程基名的起始模式：TRUE 为中文、FALSE 为英文；没有规则或有效快照时为空。
std::optional<BOOL> ReadConfiguredAppInputModeChinese(const std::wstring &process);
// 从内存快照读取当前方案，日语输入时返回 TRUE。
BOOL ReadConfiguredJapaneseInputMode();
// 内存快照中的当前方案；尚无有效快照或未指定方案时用 quanpin。
std::string ReadConfiguredInputScheme();
// 与 host-api 的 effective_scheme 一致；粤拼、注音和笔画依赖后台探测的词库可用性，缺词库时回退到 last_chinese_scheme 或 quanpin。
int ReadConfiguredRunningScheme();
// 从内存快照读取 punctuation_lock：0 跟随输入法（也用于尚无有效快照），1 固定中文，2 固定英文。
int ReadConfiguredPunctuationLock();
// 使用已发布快照更新进程内的标点锁定模式，不读取文件。
void RefreshPunctuationLockFromConfig();

struct SwitchLanguageHotkeys
{
    bool shift = true;
    bool ctrl = false;
    bool ctrl_alt_space = true;
    bool character_set_ctrl_shift_f = true;
    // keybindings.toggle_fullwidth_option_shift_h：中文模式下 Alt+Shift+H 切换全半角。
    bool fullwidth_alt_shift_h = true;
    // 单按 Caps Lock 切换中英文，缺省关。
    bool caps_lock = false;
    // keybindings.toggle_japanese_ctrl_shift_j：中文状态下 Ctrl+Shift+J 在中文方案和日语之间切换，缺省关。
    bool japanese_toggle_ctrl_shift_j = false;
};
// 从内存快照读取 keybindings.switch_language_*；尚无有效快照时使用默认值。
SwitchLanguageHotkeys ReadConfiguredSwitchLanguageHotkeys();

struct GameCompatibility
{
    bool overlay = true;
    std::vector<std::wstring> overlay_processes;
    std::vector<std::wstring> excluded_processes;
};
// 激活时同步读取最新 game_compatibility；键不存在时用默认值（开关开、两张表为空）。读取失败时返回 nullopt，调用方不强制显示，避免忽略用户排除规则。
std::optional<GameCompatibility> ReadConfiguredGameCompatibility();
} // namespace FanyUtils

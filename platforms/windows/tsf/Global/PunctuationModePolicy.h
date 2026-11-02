#pragma once

// TIP 标点 compartment 的取值规则，不依赖 Windows 头文件，方便单独测试。`punctuation_lock` 的三个取值见 PunctuationLock；`chinese_punctuation` 是用户在「标点」里选的中文标点或英文标点，即中文模式下的起始标点，出厂默认为中文标点。
namespace Global
{
namespace PunctuationLock
{
constexpr int Follow = 0;
constexpr int AlwaysChinese = 1;
constexpr int AlwaysEnglish = 2;
} // namespace PunctuationLock

// 显式请求（Ctrl+.、工具栏、托盘、应用改 conversion mode）要的标点：锁定时取锁定值，跟随时照请求。
inline bool PunctuationOpenForRequest(int lock, bool requested)
{
    switch (lock)
    {
    case PunctuationLock::AlwaysChinese:
        return true;
    case PunctuationLock::AlwaysEnglish:
        return false;
    default:
        return requested;
    }
}

// 激活输入法和切换中英文时标点回到的状态：锁定时取锁定值；跟随时英文模式用英文标点，中文模式用已存的起始标点。以前跟随时中文模式一律打开中文标点，用户选的英文标点只在设置页里存着，切走输入法再切回、或中英文往返一次就回到中文标点（#10646）。
inline bool PunctuationOpenForImeMode(int lock, bool imeOpen, bool savedChinesePunctuation)
{
    return PunctuationOpenForRequest(lock, imeOpen && savedChinesePunctuation);
}
} // namespace Global

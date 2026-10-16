#pragma once
#include <array>
#include <optional>
#include <string_view>

// 「标点与翻译」页「标点」组的规则，与共享设置页相同：「标点」选择怎样由 `chinese_punctuation` 和 `punctuation_lock` 两个键合起来表达（`packages/ui/src/settings/punctuation-preferences.ts` 的 punctuationMode、punctuationModePatch，选项文字见 `punctuation-options.ts`），智能标点总开关连同两半一起写（smartPunctuationPatch），以及「总开关开、两半都关」时说明里多出的那一句（`punctuation-section.tsx` 的 SmartPunctuationRow）。不依赖 Windows 和 WinRT 头文件，方便单独测试。
namespace msime::settings::punctuation {

struct Choice {
  std::wstring_view value;
  std::wstring_view label;
};

// 共享页 punctuationModeOptions 的四项，顺序和文字相同：前两项是跟随中英文状态时的起始标点，后两项是锁定。
inline constexpr std::array<Choice, 4> mode_choices{{
    {L"chinese", L"中文标点"},
    {L"english", L"英文标点"},
    {L"always_chinese", L"始终中文标点（英文模式也用）"},
    {L"always_english", L"始终英文标点"},
}};

// 「标点」一行的说明。前半句与共享页 PunctuationModeRow 相同；后半句是 Windows 的实情：TIP 每次激活和每次切换中英文时都按中英文模式重定标点（`CompositionProcessorEngine.cpp` 的 SetPunctuationMode、`Ipc.h` 的 ResolvePunctuationOpen），从不读已存的 `chinese_punctuation`，所以不锁定时这台电脑上的标点总是跟随中英文模式，「中文标点」「英文标点」只作为会上云的起始标点，在 macOS、Linux 和手机上生效。
inline constexpr std::wstring_view mode_description =
    L"中文模式下使用的标点。选「始终」时切到英文模式也不变，不能用 Ctrl+. 或工具栏临时切换。"
    L"Windows 上不锁定时，标点在切换中英文时随模式改变，中文标点和英文标点只决定同一账号其他设备上的起始标点";

// 从两个已存的键推出「标点」选择显示的值，只读不写：锁定优先，因为锁定时宿主不看起始标点（TIP 的 ResolvePunctuationOpen、共享层的 engine_chinese_punctuation）。不认得的锁定值按跟随处理，与偏好库读不懂时退回默认相同。
inline std::wstring_view mode(bool chinese_punctuation, std::wstring_view lock) {
  if (lock == L"chinese")
    return L"always_chinese";
  if (lock == L"english")
    return L"always_english";
  return chinese_punctuation ? L"chinese" : L"english";
}

// 设置窗口工具栏预览里的标点按钮画成中文还是英文，与 TIP 的 ResolvePunctuationOpen 相同：锁定时取锁定值；跟随时随中英文模式（预览里语言按钮画的 `default_ime_mode`），不看已存的 `chinese_punctuation`，因为 TIP 从不读它，预览照它画会和这台电脑上真正的工具栏对不上。
inline bool toolbar_chinese_punctuation(bool chinese_mode, std::wstring_view lock) {
  if (lock == L"chinese")
    return true;
  if (lock == L"english")
    return false;
  return chinese_mode;
}

// 用户选定一项时写入的值；没有值的键不写。
struct ModePatch {
  std::optional<bool> chinese_punctuation;
  std::wstring_view punctuation_lock;
};

// 前两项写起始标点并解除锁定；「始终」两项只写锁定，不碰 `chinese_punctuation`：起始标点会上云（`input.chinese_punctuation`），锁定不上云，只写锁定才不会顺带改掉其他设备的起始标点。不认得的值什么都不写。
inline std::optional<ModePatch> mode_patch(std::wstring_view mode) {
  if (mode == L"chinese")
    return ModePatch{true, L"follow"};
  if (mode == L"english")
    return ModePatch{false, L"follow"};
  if (mode == L"always_chinese")
    return ModePatch{std::nullopt, L"chinese"};
  if (mode == L"always_english")
    return ModePatch{std::nullopt, L"english"};
  return std::nullopt;
}

// 用户切换「智能标点」时一起写成同一个值的三个键。数字后直出和字母后直出是智能标点真正起作用的两半（TIP 的 ResolvePunctuation 要总开关开着、且所在一侧的直出也开着才输出 ASCII），三者默认都是开；用户单独关过两半之后，只写总开关时再打开也不起作用。打开页面、切换别的开关都不碰两半。
inline constexpr std::array<std::wstring_view, 3> smart_punctuation_keys{
    L"smart_punctuation", L"smart_punctuation_direct_digit", L"smart_punctuation_direct_letter"};

inline constexpr std::wstring_view smart_punctuation_description =
    L"中文标点模式下，字母或数字后的 , . : 自动使用英文标点";

// 以前只写总开关的版本留下了「总开关开、两半都关」的文档，这时说明里的承诺并不成立；只按已存的值多显示一句，不改写存储。
inline bool smart_punctuation_inert(bool smart, bool direct_digit, bool direct_letter) {
  return smart && !direct_digit && !direct_letter;
}

inline constexpr std::wstring_view smart_punctuation_inert_description =
    L"中文标点模式下，字母或数字后的 , . : 自动使用英文标点。数字后直出和字母后直出都已关闭，这一条暂不生效，打开下面其中一项，或关掉再打开智能标点";

} // namespace msime::settings::punctuation

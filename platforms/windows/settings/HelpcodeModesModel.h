#pragma once
#include <array>
#include <string_view>

// 双拼辅助码触发方式（client-core `ShuangpinHelpcodeModes`，文档里是 `shuangpin_helpcode.modes`）的各行，现代设置窗口和 Win7 设置窗口共用：顺序、标题和说明与共享页 `helpcode-page.tsx` 的 modesRows 逐字相同，标题前由调用处加上「双拼」。`fallback` 是 `ShuangpinHelpcodeModes::default()` 的值：这一段等于出厂值时 client-core 不写进文档，缺这一段或缺某个字段时都按它读。Tab 手动辅助码（`manual_tab`）只在 macOS 生效（host-api 用 cfg 锁住），不列。不依赖 Windows 和 WinRT 头文件，方便单独测试。
namespace msime::settings::helpcode {

// 一行受哪个总开关管：末尾间接的三枚触发键要「末尾直接」和「逐音节直接」都关着，句中三枚触发键随「句中间接辅助码」，两种结束方式随「逐音节直接辅助码」。
enum class ModeParent { none, end_trigger, mid_sentence, direct_helpcode };

struct Mode {
  std::wstring_view field;
  std::wstring_view title;
  std::wstring_view description;
  bool fallback;
  ModeParent parent;
};

inline constexpr std::array<Mode, 11> shuangpin_modes{{
    {L"direct", L"末尾直接辅助码", L"完整音节后直接接辅助码；打开时末尾间接触发不生效", true, ModeParent::none},
    {L"trigger_semicolon", L"末尾间接触发（分号）",
     L"完整音节后按分号，再输入一到两个辅助码字母；微软双拼的分号优先补全韵母", true, ModeParent::end_trigger},
    {L"trigger_backtick", L"末尾间接触发（反引号）", L"完整音节后按反引号，再输入一到两个辅助码字母",
     false, ModeParent::end_trigger},
    {L"trigger_uppercase", L"末尾间接触发（大写字母）", L"完整音节后输入大写字母作为辅助码", false,
     ModeParent::end_trigger},
    {L"mid_sentence", L"句中间接辅助码", L"触发键把它前面的音节标上辅助码，约束带进整句，后面还能接着打",
     false, ModeParent::none},
    {L"mid_sentence_backtick", L"句中触发键（反引号）", L"用反引号在句中标记辅助码", true,
     ModeParent::mid_sentence},
    {L"mid_sentence_semicolon", L"句中触发键（分号）", L"用分号在句中标记辅助码；微软双拼的分号优先补全韵母",
     false, ModeParent::mid_sentence},
    {L"mid_sentence_uppercase", L"句中触发键（大写字母）", L"完整音节后输入大写字母在句中标记辅助码", false,
     ModeParent::mid_sentence},
    {L"direct_helpcode", L"逐音节直接辅助码", L"不要引导键，逐音节接辅码；三码自动识别，四码单独敲时直接定字",
     false, ModeParent::none},
    {L"direct_helpcode_slash", L"逐音节直接（/ 结束）", L"用 / 结束一位或两位辅码", true,
     ModeParent::direct_helpcode},
    {L"direct_helpcode_uppercase", L"逐音节直接（第二码大写结束）", L"第二位辅码敲大写来结束一段", false,
     ModeParent::direct_helpcode},
}};

// 这一行能不能改：双拼辅助码关着时全部置灰，有总开关的行还要总开关开着，与共享页的 disabled 相同。末尾间接三行照引擎的判据（`apply_request_options` 只在末尾直接和逐音节直接都关着时保留末尾段），只置灰、不改存的值。
constexpr bool mode_enabled(Mode const &mode, bool helpcode_on, bool direct, bool mid_sentence, bool direct_helpcode) {
  if (!helpcode_on)
    return false;
  switch (mode.parent) {
  case ModeParent::end_trigger:
    return !direct && !direct_helpcode;
  case ModeParent::mid_sentence:
    return mid_sentence;
  case ModeParent::direct_helpcode:
    return direct_helpcode;
  case ModeParent::none:
    break;
  }
  return true;
}

// 「一码严格筛选」（`<族>_helpcode.strict_filter`，缺字段时为关）的说明，与共享页 strictFilterRow 相同。
inline constexpr std::wstring_view strict_filter_description =
    L"关闭时一码只把匹配项前移；打开后排除辅助码不匹配的候选。两码本来就是严格筛选。";

} // namespace msime::settings::helpcode

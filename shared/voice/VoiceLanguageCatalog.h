#pragma once
// 由共享 Rust 语音语言目录导出；通过 export_voice_language_catalog 更新。
#include <array>
#include <string_view>

namespace msime::voice {
struct VoiceLanguageCatalogEntry {
  std::wstring_view id;
  std::wstring_view provider_value;
  std::wstring_view provider_title;
  std::wstring_view system_value;
  std::wstring_view system_title;
  std::wstring_view mobile_value;
  std::wstring_view mobile_title;
};
inline constexpr std::array<VoiceLanguageCatalogEntry, 5> kLanguages = {{
  {L"zh-cn", L"zh-cn", L"中文（普通话）", L"zh-CN", L"中文（普通话）", L"zh-cn", L"普通话"},
  {L"yue", L"", L"", L"", L"", L"yue", L"粤语"},
  {L"en", L"en", L"English", L"en-US", L"English", L"en", L"英语"},
  {L"ja", L"ja", L"日本語", L"ja-JP", L"日本語", L"", L"日语"},
  {L"auto", L"auto", L"自动识别", L"", L"", L"auto", L"普通话 + 英语"},
}};
} // 命名空间 msime::voice

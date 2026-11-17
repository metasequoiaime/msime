#pragma once

#include <string_view>

namespace msime::windows {
// 宿主按键和上屏统计共用来源标识；语言归组由共享 Rust 决定。
enum class TypingSource {
  Quanpin,
  NineKey,
  Shuangpin,
  Ziranma,
  Microsoft,
  Shoudao,
  Wubi,
  Japanese,
  Korean,
  Cantonese,
  Zhuyin,
  Vietnamese,
  Tibetan,
  Stroke,
  Handwriting,
  English,
  Local,
  Ai,
  Reply,
  Voice,
  Unknown,
  SmartAbc,
};

constexpr std::string_view typing_source_id(TypingSource source) {
  switch (source) {
  case TypingSource::Quanpin:
    return "quanpin";
  case TypingSource::NineKey:
    return "nineKey";
  case TypingSource::Shuangpin:
    return "shuangpin";
  case TypingSource::Ziranma:
    return "ziranma";
  case TypingSource::Microsoft:
    return "microsoft";
  case TypingSource::Shoudao:
    return "shoudao";
  case TypingSource::SmartAbc:
    return "smart_abc";
  case TypingSource::Wubi:
    return "wubi";
  case TypingSource::Japanese:
    return "japanese";
  case TypingSource::Korean:
    return "korean";
  case TypingSource::Cantonese:
    return "cantonese";
  case TypingSource::Zhuyin:
    return "zhuyin";
  case TypingSource::Vietnamese:
    return "vietnamese";
  case TypingSource::Tibetan:
    return "tibetan";
  case TypingSource::Stroke:
    return "stroke";
  case TypingSource::Handwriting:
    return "handwriting";
  case TypingSource::English:
    return "english";
  case TypingSource::Local:
    return "local";
  case TypingSource::Ai:
    return "ai";
  case TypingSource::Reply:
    return "reply";
  case TypingSource::Voice:
    return "voice";
  case TypingSource::Unknown:
    return "unknown";
  }
  return "unknown";
}

// 共享引擎在视图里用数字表示方案：0 全拼、1 双拼、2 五笔、3 日文、4 韩文、5
// 粤拼、6 注音、7 越南文、8 藏文、9 笔画。本地模式优先于键盘方案，与
// Android、Apple 和 Linux 宿主一致。
constexpr TypingSource
resolve_typing_source(int scheme, bool nine_key, bool dedicated_english,
                      std::string_view local_mode,
                      std::string_view shuangpin_profile) {
  if (local_mode == "temporary_japanese")
    return TypingSource::Japanese;
  if (!local_mode.empty() && local_mode != "none")
    return TypingSource::Local;
  if (dedicated_english)
    return TypingSource::English;
  switch (scheme) {
  case 0:
    return nine_key ? TypingSource::NineKey : TypingSource::Quanpin;
  case 1:
    if (shuangpin_profile == "ziranma")
      return TypingSource::Ziranma;
    if (shuangpin_profile == "microsoft")
      return TypingSource::Microsoft;
    if (shuangpin_profile == "shoudao")
      return TypingSource::Shoudao;
    if (shuangpin_profile == "smart_abc")
      return TypingSource::SmartAbc;
    return TypingSource::Shuangpin;
  case 2:
    return TypingSource::Wubi;
  case 3:
    return TypingSource::Japanese;
  case 4:
    return TypingSource::Korean;
  case 5:
    return TypingSource::Cantonese;
  case 6:
    return TypingSource::Zhuyin;
  case 7:
    return TypingSource::Vietnamese;
  case 8:
    return TypingSource::Tibetan;
  case 9:
    return TypingSource::Stroke;
  default:
    return TypingSource::Unknown;
  }
}

} // namespace msime::windows

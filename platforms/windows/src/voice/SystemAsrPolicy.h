#pragma once
// 「Windows 系统识别」（asr_provider = "system"）里不需要 Win32 的判定：识别语言对应哪个 SAPI 识别器语言属性、识别器缺失时给人看哪句话、逐句结果怎样拼成整段文字、浮点采样怎样换成识别器读的 16 位 PCM。放在这里是为了能在主机上测试。

#include <algorithm>
#include <array>
#include <chrono>
#include <cstdint>
#include <optional>
#include <stdexcept>
#include <string>
#include <string_view>

namespace msime::windows {
// 识别失败时带给人看的那句话；what() 就是这句中文，不含录音内容。
class SystemAsrError : public std::runtime_error {
public:
  using std::runtime_error::runtime_error;
};

// 这台 Windows 没有 SAPI 语音识别组件（sapi.dll 未注册或无法创建进程内识别器）。
inline constexpr std::string_view system_asr_unavailable_message =
    "Windows 系统语音识别不可用，请改用其他识别服务。";

// 识别语言转成 SAPI 识别器令牌的 Language 属性（十六进制 LANGID，与注册表里识别器令牌写的格式相同，例如 804）。空值和 auto 按 zh-CN 处理，和 macOS 没给语言时用 zh-CN 一样；Windows 没有对应听写识别器的语言返回空。粤语也返回空：yue、zh-yue 以及地区是香港或澳门的 zh 标签（共享设置把 zh-HK 显示为粤语），不能悄悄交给普通话识别器。
inline std::optional<std::string_view> system_asr_language_id(std::string_view language) {
  std::string tag;
  tag.reserve(language.size());
  for (const char ch : language) {
    if (ch == ' ' || ch == '\t')
      continue;
    tag.push_back(ch == '_' ? '-' : static_cast<char>((ch >= 'A' && ch <= 'Z') ? ch - 'A' + 'a' : ch));
  }
  if (tag.empty() || tag == "auto")
    return std::string_view{"804"};
  // 粤语：主语言 zh 的任一子标签是 yue、hk 或 mo 时，说的多半是粤语，Windows 没有粤语听写识别器。
  if (tag == "zh" || tag.rfind("zh-", 0) == 0) {
    for (std::size_t start = 3; start <= tag.size();) {
      const auto end = std::min(tag.find('-', start), tag.size());
      const auto subtag = std::string_view(tag).substr(start, end - start);
      if (subtag == "yue" || subtag == "hk" || subtag == "mo")
        return std::nullopt;
      start = end + 1;
    }
  }
  struct Entry {
    std::string_view tag;
    std::string_view id;
  };
  // 先按完整标签找，再按主语言找。表里只有 Windows 语音识别提供过听写识别器的语言。
  static constexpr std::array<Entry, 12> exact{{
      {"zh-cn", "804"},
      {"zh-sg", "804"},
      {"zh-hans", "804"},
      {"zh-hans-cn", "804"},
      {"zh-tw", "404"},
      {"zh-hant", "404"},
      {"zh-hant-tw", "404"},
      {"en-us", "409"},
      {"en-gb", "809"},
      {"es-es", "c0a"},
      {"es-mx", "80a"},
      {"fr-fr", "40c"},
  }};
  for (const auto &entry : exact)
    if (entry.tag == tag)
      return entry.id;
  const auto primary = std::string_view(tag).substr(0, tag.find('-'));
  static constexpr std::array<Entry, 6> primaries{{
      {"zh", "804"},
      {"en", "409"},
      {"ja", "411"},
      {"de", "407"},
      {"fr", "40c"},
      {"es", "c0a"},
  }};
  for (const auto &entry : primaries)
    if (entry.tag == primary)
      return entry.id;
  return std::nullopt;
}

// 识别语言 Windows 系统识别器根本不支持。
inline std::string system_asr_unsupported_language_message(std::string_view language) {
  return "Windows 系统识别不支持识别语言“" + std::string(language) +
         "”，请在“语音输入”设置中改用 zh-CN、en-US 等语言代码。";
}

// 支持这种语言，但这台电脑没有装它的语音识别。语言为空时按实际使用的 zh-CN 说明。
inline std::string system_asr_missing_language_message(std::string_view language) {
  // 与 system_asr_language_id 相同：忽略大小写和空白，空值和 auto 都是实际使用的 zh-CN。
  std::string tag;
  for (const char ch : language)
    if (ch != ' ' && ch != '\t')
      tag.push_back((ch >= 'A' && ch <= 'Z') ? static_cast<char>(ch - 'A' + 'a') : ch);
  const bool unnamed = tag.empty() || tag == "auto";
  return "这台电脑没有安装 " + std::string(unnamed ? std::string_view{"zh-CN"} : language) +
         " 的 Windows 语音识别，请在 Windows 设置 › 时间和语言 › 语言和区域 中为该语言安装“语音识别”，或改用其他识别服务。";
}

// 系统识别失败时给人看的那句话：识别器自己说明了原因就用它，其余情况用通用的一句（与其他识别服务的「语音识别失败」相同）。
inline std::string system_asr_failure(const std::exception &error) {
  if (const auto *system = dynamic_cast<const SystemAsrError *>(&error))
    return system->what();
  return "语音识别失败";
}

// 把识别器新给出的一句接到已有文字后面。中文等不用空格分词的文字直接相连；两边都是 ASCII 字母或数字时（英文听写）补一个空格，否则「hello」「world」会连成一个词。
inline std::string system_asr_join(std::string_view committed, std::string_view phrase) {
  std::string result(committed);
  if (phrase.empty())
    return result;
  const auto word = [](unsigned char ch) {
    return (ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z') || (ch >= '0' && ch <= '9') ||
           ch == ',' || ch == '.' || ch == '!' || ch == '?' || ch == ';' || ch == ':';
  };
  const auto starts_word = [](unsigned char ch) {
    return (ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z') || (ch >= '0' && ch <= '9');
  };
  if (!result.empty() && word(static_cast<unsigned char>(result.back())) &&
      starts_word(static_cast<unsigned char>(phrase.front())))
    result.push_back(' ');
  result.append(phrase);
  return result;
}

// 采集回调给的是 [-1, 1] 的浮点采样，识别器读 16 位有符号 PCM。超出范围的值截到两端，不让它回绕成反相的噪声。
constexpr std::int16_t system_asr_pcm16(float sample) {
  const float clamped = sample > 1.0f ? 1.0f : (sample < -1.0f ? -1.0f : sample);
  return static_cast<std::int16_t>(clamped * 32767.0f);
}

// 录音结束后等识别器把剩下的音频识别完、发出流结束事件的最长时间。识别器卡住时不让识别任务一直挂着，已经识别出的文字照常上屏。
inline constexpr std::chrono::seconds system_asr_drain_timeout{20};
} // namespace msime::windows

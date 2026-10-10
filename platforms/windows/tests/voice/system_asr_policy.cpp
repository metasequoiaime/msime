#include "SystemAsrPolicy.h"

#include <iostream>
#include <stdexcept>
#include <string>

using namespace msime::windows;
namespace {
void require(bool value, int line) {
  if (!value)
    throw std::runtime_error("system asr policy failed at line " + std::to_string(line));
}
#define REQUIRE(value) require((value), __LINE__)

bool id_is(std::string_view language, std::string_view expected) {
  const auto id = system_asr_language_id(language);
  return id && *id == expected;
}
} // namespace

int main() {
  try {
    // 设置页给系统识别写的是 zh-CN、en-US、ja-JP，其他识别服务写的是 zh-cn、en、ja；两种写法都要找到同一个识别器。
    REQUIRE(id_is("zh-CN", "804"));
    REQUIRE(id_is("zh-cn", "804"));
    REQUIRE(id_is("zh_CN", "804"));
    REQUIRE(id_is("zh", "804"));
    REQUIRE(id_is("zh-Hans", "804"));
    REQUIRE(id_is("zh-TW", "404"));
    REQUIRE(id_is("zh-Hant", "404"));
    REQUIRE(id_is("en-US", "409"));
    REQUIRE(id_is("en", "409"));
    REQUIRE(id_is("en-GB", "809"));
    REQUIRE(id_is("en-AU", "409"));
    REQUIRE(id_is("ja-JP", "411"));
    REQUIRE(id_is("ja", "411"));
    REQUIRE(id_is("de-DE", "407"));
    REQUIRE(id_is("fr-FR", "40c"));
    REQUIRE(id_is("es-MX", "80a"));
    // 没给语言或自动识别时和 macOS 一样按普通话。
    REQUIRE(id_is("", "804"));
    REQUIRE(id_is("auto", "804"));
    REQUIRE(id_is(" AUTO ", "804"));
    // Windows 没有粤语或韩语的听写识别器：开始录音前就说清楚，而不是悄悄换成普通话。
    REQUIRE(!system_asr_language_id("yue"));
    REQUIRE(!system_asr_language_id("ko-KR"));
    // 共享设置把 zh-HK 显示为粤语；香港、澳门的 zh 标签和 zh-yue 都不交给台湾普通话或大陆普通话识别器。
    REQUIRE(!system_asr_language_id("zh-HK"));
    REQUIRE(!system_asr_language_id("zh_hk"));
    REQUIRE(!system_asr_language_id("zh-MO"));
    REQUIRE(!system_asr_language_id("zh-Hant-HK"));
    REQUIRE(!system_asr_language_id("zh-yue"));
    REQUIRE(!system_asr_language_id("yue-HK"));
    // 只看完整子标签，不误伤别的地区。
    REQUIRE(id_is("zh-Hant-TW", "404"));
    REQUIRE(id_is("zh-SG", "804"));
    REQUIRE(system_asr_unsupported_language_message("yue").find("“yue”") != std::string::npos);

    // 缺识别器的提示写出语言和去哪里装；没给语言时写实际使用的 zh-CN。
    const auto missing = system_asr_missing_language_message("en-US");
    REQUIRE(missing.find("en-US") != std::string::npos);
    REQUIRE(missing.find("语言和区域") != std::string::npos);
    REQUIRE(system_asr_missing_language_message("").find("zh-CN") != std::string::npos);
    REQUIRE(system_asr_missing_language_message("auto").find("zh-CN") != std::string::npos);
    REQUIRE(system_asr_missing_language_message(" Auto ").find("zh-CN") != std::string::npos);

    // 识别器说明了原因就照用，其他异常只给通用的一句，不把英文诊断给人看。
    REQUIRE(system_asr_failure(SystemAsrError("synthetic reason")) == "synthetic reason");
    REQUIRE(system_asr_failure(std::runtime_error("0x80045509")) == "语音识别失败");

    // 中文逐句直接相连，英文两词之间补空格，已有空格或标点开头时不重复。
    REQUIRE(system_asr_join("今天天气", "很好") == "今天天气很好");
    REQUIRE(system_asr_join("", "hello") == "hello");
    REQUIRE(system_asr_join("hello", "world") == "hello world");
    REQUIRE(system_asr_join("hello.", "World") == "hello. World");
    REQUIRE(system_asr_join("hello ", "world") == "hello world");
    REQUIRE(system_asr_join("hello", ", world") == "hello, world");
    REQUIRE(system_asr_join("你好", "hello") == "你好hello");
    REQUIRE(system_asr_join("hello", "") == "hello");

    // PCM 转换在两端截断，不回绕。
    REQUIRE(system_asr_pcm16(0.0f) == 0);
    REQUIRE(system_asr_pcm16(1.0f) == 32767);
    REQUIRE(system_asr_pcm16(-1.0f) == -32767);
    REQUIRE(system_asr_pcm16(3.0f) == 32767);
    REQUIRE(system_asr_pcm16(-3.0f) == -32767);
    REQUIRE(system_asr_pcm16(0.5f) == 16383);
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
  return 0;
}

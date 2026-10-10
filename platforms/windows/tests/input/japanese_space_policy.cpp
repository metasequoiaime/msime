#include "JapaneseSpacePolicy.h"
#include <iostream>
#include <stdexcept>
#include <string>

using namespace msime::windows;
using Action = msime::input::JapaneseConversion::Action;
namespace {
[[noreturn]] void require_failed(int line) {
  throw std::runtime_error("Japanese space policy test failed at line " + std::to_string(line));
}
#define require(value)                                                         \
  do {                                                                         \
    if (!(value))                                                              \
      require_failed(__LINE__);                                                \
  } while (false)

void applies() {
  // 只有日语方案组字时的裸空格归状态机。
  require(japanese_space_applies(scheme::Japanese, 0x20, 0, false, true));
  require(!japanese_space_applies(scheme::Quanpin, 0x20, 0, false, true));
  require(!japanese_space_applies(scheme::Japanese, 0x0D, 0, false, true));
  require(!japanese_space_applies(scheme::Japanese, 0x20, 1, false, true));
  require(!japanese_space_applies(scheme::Japanese, 0x20, 0, false, false));
  // 游戏自己画候选（UILess）时空格照旧上屏。
  require(!japanese_space_applies(scheme::Japanese, 0x20, 0, true, true));
}

void steps() {
  // 开始转换不动会话，步进和回到首条各是一条命令；其余回答不归空格管。
  const auto start = japanese_space_step(Action::Start);
  require(start && !start->command);
  const auto next = japanese_space_step(Action::StepNext);
  require(next && next->command == static_cast<uint32_t>(MSIME_NEXT_CANDIDATE));
  const auto first = japanese_space_step(Action::StepFirst);
  require(first && first->command == static_cast<uint32_t>(MSIME_FIRST_CANDIDATE));
  require(!japanese_space_step(Action::None));
  require(!japanese_space_step(Action::CommitCandidate));
  require(!japanese_space_step(Action::CommitReading));
}

void sequence() {
  // Server 上一段组字里连按空格：开始、下一个、下一个、过了末尾回到第一个。
  msime::input::JapaneseConversion conversion;
  require(japanese_space_step(conversion.space("nihon", 3, 0))->command == std::nullopt);
  require(japanese_space_step(conversion.space("nihon", 3, 0))->command ==
          static_cast<uint32_t>(MSIME_NEXT_CANDIDATE));
  require(japanese_space_step(conversion.space("nihon", 3, 0))->command ==
          static_cast<uint32_t>(MSIME_NEXT_CANDIDATE));
  require(japanese_space_step(conversion.space("nihon", 3, 0))->command ==
          static_cast<uint32_t>(MSIME_FIRST_CANDIDATE));
  // 改了读音就重新开始。
  require(japanese_space_step(conversion.space("nihong", 3, 0))->command == std::nullopt);
  // 组字结束后（ReplyComposer 在回复的 editing_text 为空时 reset），同一段读音再打一遍也从开始算起。
  conversion.reset();
  require(japanese_space_step(conversion.space("nihong", 3, 0))->command == std::nullopt);
  // 唯一的候选是 Fallback 时不接管，空格照常上屏原文。
  msime::input::JapaneseConversion fallback;
  require(!japanese_space_step(fallback.space("x", 1, msime::input::kCandidateSourceFallback)));
  // 没有候选时也不接管。
  require(!japanese_space_step(fallback.space("x", 0, -1)));
}
} // namespace

int main() {
  try {
    applies();
    steps();
    sequence();
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
  return 0;
}

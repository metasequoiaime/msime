#include "ReplyCodec.h"
#include "../../tsf/Global/PairedPunctuationHostPolicy.h"
#include "windows_ipc.h"
#include <iostream>
#include <stdexcept>
#include <string>
#include <utility>

using namespace msime::windows;
namespace {
[[noreturn]] void require_failed(int line) {
  throw std::runtime_error("TSF config frame test failed at line " +
                           std::to_string(line));
}
#define require(value)                                                         \
  do {                                                                         \
    if (!(value))                                                              \
      require_failed(__LINE__);                                                \
  } while (false)
uint32_t frame_type(const std::vector<uint8_t> &frame) {
  uint32_t type = 0;
  for (size_t i = 0; i < sizeof(type); ++i)
    type |= static_cast<uint32_t>(frame[i]) << (8 * i);
  return type;
}
// The payload as the TIP reads it: a NUL-terminated wide string.
std::wstring frame_text(const std::vector<uint8_t> &frame) {
  const size_t offset = offsetof(FanyImeNamedpipeDataToTsfWorkerThread, data);
  std::wstring text;
  for (size_t i = offset; i + 1 < frame.size(); i += 2) {
    const auto unit = static_cast<wchar_t>(frame[i] | (frame[i + 1] << 8));
    if (!unit)
      break;
    text.push_back(unit);
  }
  return text;
}
} // namespace
int main() {
  try {
    TsfLocalConfig config;
    auto frames = tsf_config_frames(config);
    // Every setting the TIP consumes gets a frame; it kept compiled defaults
    // because the Server encoded none of them.
    require(frames.size() == 11);
    for (const auto &frame : frames)
      require(frame.size() == sizeof(FanyImeNamedpipeDataToTsfWorkerThread));

    // Each frame is the message type the TIP dispatches on, once each.
    const uint32_t expected[] = {
        FanyImeWorkerReplyType::PagingCommaPeriodChanged,
        FanyImeWorkerReplyType::SmartPunctuationChanged,
        FanyImeWorkerReplyType::SmartPunctuationRepeatToChineseChanged,
        FanyImeWorkerReplyType::PairedPunctuationChanged,
        FanyImeWorkerReplyType::MicrosoftShuangpinChanged,
        FanyImeWorkerReplyType::InputModeChanged,
        FanyImeWorkerReplyType::TsfDiagnosticLogChanged,
        FanyImeWorkerReplyType::PunctuationLockChanged,
        FanyImeWorkerReplyType::LocalModeTriggersChanged,
        FanyImeWorkerReplyType::DedicatedEnglishChanged,
        FanyImeWorkerReplyType::SecondThirdCandidateChanged};
    for (size_t i = 0; i < frames.size(); ++i)
      require(frame_type(frames[i]) == expected[i]);

    // Booleans travel as "1"/"0", which is what the TIP compares against.
    config.smart_punctuation = false;
    config.paired_punctuation = true;
    config.microsoft_shuangpin = true;
    frames = tsf_config_frames(config);
    require(frame_text(frames[1]) == L"0"); // smart punctuation off
    config.smart_punctuation_repeat_to_chinese = false;
    frames = tsf_config_frames(config);
    require(frame_text(frames[2]) == L"0"); // repeated punctuation conversion off
    config.smart_punctuation_repeat_to_chinese = true;
    require(frame_text(tsf_config_frames(config)[2]) == L"1");
    require(frame_text(frames[3]) == L"1"); // paired punctuation on
    require(frame_text(frames[4]) == L"1"); // Microsoft shuangpin on

    // 输入模式帧给出方案所属的族："0" 全拼、双拼或五笔，"1" 日文，"2" 韩文，"3" 粤拼，"4" 注音，"5" 越南文，"6" 藏文，"7" 笔画。
    require(frame_text(frames[5]) == L"0");
    const std::pair<msime::windows::scheme::InputMode, const wchar_t *> modes[] = {
        {msime::windows::scheme::InputMode::Japanese, L"1"},
        {msime::windows::scheme::InputMode::Korean, L"2"},
        {msime::windows::scheme::InputMode::Cantonese, L"3"},
        {msime::windows::scheme::InputMode::Zhuyin, L"4"},
        {msime::windows::scheme::InputMode::Vietnamese, L"5"},
        {msime::windows::scheme::InputMode::Tibetan, L"6"},
        {msime::windows::scheme::InputMode::Stroke, L"7"},
        {msime::windows::scheme::InputMode::Chinese, L"0"}};
    for (const auto &[mode, code] : modes) {
      config.input_mode = mode;
      require(frame_text(tsf_config_frames(config)[5]) == code);
    }

    // The preedit style rides along with the paging frame after a '|'; there is
    // no separate message type for it, which is why it stayed stuck at raw.
    config.paging_comma_period = true;
    config.preedit_style = "pinyin";
    frames = tsf_config_frames(config);
    require(frame_text(frames[0]) == L"1|pinyin");
    config.paging_comma_period = false;
    config.preedit_style = "empty";
    require(frame_text(tsf_config_frames(config)[0]) == L"0|empty");
    require(Global::IsPairedPunctuationExcludedProcess(L"EXCEL.EXE"));
    require(Global::IsPairedPunctuationExcludedProcess(L"excel.exe"));
    require(!Global::IsPairedPunctuationExcludedProcess(L"WINWORD.EXE"));
    // An unknown style is omitted rather than forwarded, so the TIP keeps its
    // own value instead of being handed something it cannot parse.
    config.preedit_style = "nonsense";
    require(frame_text(tsf_config_frames(config)[0]) == L"0");

    // Punctuation lock starts with a digit and carries the fine-grained direct
    // punctuation policy as a bounded extension.
    config.smart_punctuation_space_convert = true;
    config.smart_punctuation_direct_digit = true;
    config.smart_punctuation_direct_letter = false;
    require(frame_text(tsf_config_frames(config)[7]) == L"0|s1d1l0");
    config.smart_punctuation_space_convert = false;
    config.smart_punctuation_direct_digit = false;
    config.smart_punctuation_direct_letter = true;
    require(frame_text(tsf_config_frames(config)[7]) == L"0|s0d0l1");
    for (uint8_t lock = 0; lock < 3; ++lock) {
      config.punctuation_lock = lock;
      const auto text = frame_text(tsf_config_frames(config)[7]);
      require(text.size() == 8 && text[0] == static_cast<wchar_t>(L'0' + lock) &&
              text[1] == L'|' && text[2] == L's' && text[3] == L'0' &&
              text[4] == L'd' && text[5] == L'0' && text[6] == L'l' && text[7] == L'1');
    }
    // Defensive normalization keeps malformed persisted values within the
    // three-state TIP contract instead of emitting an invalid compartment.
    for (uint8_t lock : {static_cast<uint8_t>(3), static_cast<uint8_t>(255)}) {
      config.punctuation_lock = lock;
      require(frame_text(tsf_config_frames(config)[7]) == L"0|s0d0l1");
    }

    // The V, "/" and "@" switches travel as one frame of three flags in that order, all off until the Server says otherwise, so a TIP never routes a mode's keys the Engine will not open.
    require(frame_text(tsf_config_frames(TsfLocalConfig{})[8]) == L"000");
    config.expression_mode = true;
    require(frame_text(tsf_config_frames(config)[8]) == L"100");
    config.expression_mode = false;
    config.command_mode = true;
    require(frame_text(tsf_config_frames(config)[8]) == L"010");
    config.command_mode = false;
    config.mention_mode = true;
    require(frame_text(tsf_config_frames(config)[8]) == L"001");
    config.expression_mode = config.command_mode = true;
    require(frame_text(tsf_config_frames(config)[8]) == L"111");
    // In the Engine's own English mode V is a letter, so the TIP is told the V mode is off and a word starting with V keeps digit selection and paging. "/" and "@" go off too: the Engine opens neither there, so a "/" the TIP composed would never reach it.
    config.dedicated_english = true;
    require(frame_text(tsf_config_frames(config)[8]) == L"000");
    config.dedicated_english = false;
    require(frame_text(tsf_config_frames(config)[8]) == L"111");
    // The switches reach the flags only where the Engine opens the mode: V in the pinyin schemes (`opens_local_modes`), "/" and "@" in the pinyin schemes and Wubi too (`opens_table_modes`). A Wubi user with "/" and "@" on gets them; the TIP would otherwise send "/" as punctuation while the Engine waits to open the command mode.
    {
      const auto switched = [](std::string_view name, bool expression, bool command, bool mention) {
        TsfLocalConfig local;
        apply_local_mode_switches(local, scheme::scheme_from_name(name), expression, command, mention);
        return frame_text(tsf_config_frames(local)[8]);
      };
      require(switched("quanpin", true, true, true) == L"111");
      require(switched("shuangpin", true, true, true) == L"111");
      require(switched("wubi", true, true, true) == L"011");
      require(switched("wubi", false, true, false) == L"010");
      require(switched("wubi", false, false, true) == L"001");
      require(switched("wubi", false, false, false) == L"000");
      require(switched("quanpin", false, false, false) == L"000");
      for (const auto name : {"japanese", "korean", "cantonese", "zhuyin", "vietnamese", "tibetan", "stroke", "unknown"})
        require(switched(name, true, true, true) == L"000");
      // A switch the scheme does not open never leaves a stale flag on from an earlier scheme.
      TsfLocalConfig reused;
      apply_local_mode_switches(reused, scheme::Quanpin, true, true, true);
      apply_local_mode_switches(reused, scheme::Korean, true, true, true);
      require(!reused.expression_mode && !reused.command_mode && !reused.mention_mode);
    }
    // The TIP drops every type above MaxKnown, so the new type has to be inside it.
    require(FanyImeWorkerReplyType::LocalModeTriggersChanged <=
            FanyImeWorkerReplyType::MaxKnown);
    // The Engine's own English mode travels on its own frame too, after every older one: the TIP keeps reporting Chinese there, and under Stroke it would otherwise hand an idle non-stroke letter (the 'a' of "apple") to the application instead of the Engine.
    require(frame_text(tsf_config_frames(TsfLocalConfig{})[9]) == L"0");
    config.dedicated_english = true;
    require(frame_text(tsf_config_frames(config)[9]) == L"1");
    config.dedicated_english = false;
    require(FanyImeWorkerReplyType::DedicatedEnglishChanged < FanyImeWorkerReplyType::MaxKnown);
    // 二三候选单独一帧，排在所有旧帧之后：关着时是 "0"，TIP 据此把 ';' 和 '\'' 留作标点。
    require(frame_text(tsf_config_frames(TsfLocalConfig{})[10]) == L"0");
    config.second_third_candidate = true;
    require(frame_text(tsf_config_frames(config)[10]) == L"1");
    config.second_third_candidate = false;
    require(FanyImeWorkerReplyType::SecondThirdCandidateChanged == FanyImeWorkerReplyType::MaxKnown);

    // Caps Lock travels on its own frame rather than in the configuration set:
    // the Server owns the indicator because the TIP only sampled GetKeyState at
    // activation, so pressing Caps mid-session left it stale. Nothing covered
    // the frame itself, only the state behind it.
    for (const bool enabled : {false, true}) {
      const auto frame = caps_lock_frame(enabled);
      require(frame.size() == sizeof(FanyImeNamedpipeDataToTsfWorkerThread));
      require(frame_type(frame) == FanyImeWorkerReplyType::CapsLockChanged);
      // "0"/"1", the payload the TIP parses - not a raw byte, and not the
      // configuration frame's key=value shape.
      require(frame_text(frame) == (enabled ? L"1" : L"0"));
    }
    // It is its own message type, so it can never be mistaken for one of the
    // configuration frames above.
    for (const auto &frame : tsf_config_frames(TsfLocalConfig{}))
      require(frame_type(frame) != FanyImeWorkerReplyType::CapsLockChanged);

    std::cout << "TSF config frames: every setting the TIP reads is encoded\n";
  } catch (const std::exception &failure) {
    std::cerr << failure.what() << '\n';
    return 1;
  } catch (...) {
    std::cerr << "TSF config frame test failed with an unknown error\n";
    return 1;
  }
}

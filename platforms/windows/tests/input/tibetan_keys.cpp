#include "../../src/ipc/ReplyComposer.h"
#include "../core/TestHostOptions.h"
#include <cassert>
#include <chrono>
using namespace msime::windows;

namespace {
FanyImeNamedpipeData key(uint64_t request, uint32_t code, char16_t text, uint32_t modifiers = 0) {
  FanyImeNamedpipeData packet{};
  packet.client_id = 42;
  packet.event_type = FanyImePipeEventType::KeyEvent;
  packet.request_id = request;
  packet.keycode = code;
  packet.wch = text;
  packet.modifiers_down = modifiers;
  return packet;
}

// 美式键盘上威利转写用到的非字母键的虚拟键码。
uint32_t virtual_key(char text) {
  switch (text) {
  case '\'':
    return 0xDE;
  case '+':
    return 0xBB;
  case '-':
    return 0xBD;
  case '.':
    return 0xBE;
  case '/':
    return 0xBF;
  case ' ':
    return 0x20;
  default:
    return static_cast<uint32_t>(text);
  }
}

// 藏文方案的 Server 一侧，对着真实的引擎会话运行。和越南文一样，TIP 在自己的宿主会话里组字并自己写出文字；Server 要和它保持同步，不能因为按键带了上屏就让回复失败，并且要把写出的文字计入统计。
struct Fixture {
  ServerSession session;
  ReplyComposer composer{42, 1};
  uint64_t request = 0;
  // 打开减号/等号和逗号/句号翻页，用来说明藏文不理会它们：`-` 和 `.` 是威利拼写符号。
  NavigationBindings paging{true, true, true, true, true, true, false};

  explicit Fixture(const std::string &options) : session(42, options) { session.activate(1); }

  std::optional<PendingReply> press(uint32_t code, char16_t text, uint32_t modifiers = 0,
                                    TsfPreeditStyle style = TsfPreeditStyle::Local) {
    const auto packet = key(++request, code, text, modifiers);
    auto reply =
        composer.configured_key(session, packet, 1, style, paging, std::nullopt, WordCharacterBinding::Disabled);
    if (reply)
      composer.confirm_delivery(42, 1, packet.request_id);
    return reply;
  }
  // 按美式键盘输入的字母和威利拼写符号；`+` 带 Shift，大写字母带 Shift。
  void type(const char *keys) {
    for (const char *text = keys; *text; ++text) {
      const bool upper = *text >= 'A' && *text <= 'Z';
      const bool lower = *text >= 'a' && *text <= 'z';
      const auto code = lower ? static_cast<uint32_t>(*text - 'a' + 'A') : virtual_key(*text);
      assert(press(code, static_cast<char16_t>(*text), upper || *text == '+' ? 1u : 0u));
    }
  }
  std::optional<PendingReply> press_char(char text) {
    return press(virtual_key(text), static_cast<char16_t>(text), text == '+' ? 1u : 0u);
  }
  std::string editing() const { return session.view().at("editing_text").get<std::string>(); }
  std::string preedit() const { return session.view().at("preedit").get<std::string>(); }
};
} // namespace

int main() {
  const auto root = std::filesystem::temp_directory_path() /
                    ("msime-tibetan-keys-" +
                     std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
  std::filesystem::create_directory(root);
  struct Cleanup {
    std::filesystem::path root;
    ~Cleanup() {
      std::error_code ec;
      std::filesystem::remove_all(root, ec);
    }
  } cleanup{root};
  auto options = test_host_options(root);
  options["preferences"]["scheme"] = "tibetan";
  options["preferences"]["traditional_chinese_output"] = true;
  const auto serialized = options.dump();

  // 字母组成威利原文，显示的是转换出的藏文，没有候选。繁体输出是中文投影：藏文从不转换，开关原样保留。
  {
    Fixture tibetan(serialized);
    assert(tibetan.session.view().value("scheme", 0u) == 8u);
    assert(tibetan.session.view().value("spelling_symbols", std::string{}) == "'/");
    tibetan.type("bkra");
    assert(tibetan.preedit() == "བཀྲ");
    assert(tibetan.session.view().at("candidates").empty());
    assert(tibetan.session.view().value("spelling_symbols", std::string{}) == "'+-./");
    assert(tibetan.session.traditional_output());
  }

  // 空格带音节点、`/` 带垂符上屏音节串；两者都只计数，TIP 已经从自己的宿主会话写出了同样的文字。
  {
    Fixture tibetan(serialized);
    tibetan.type("bkra");
    const auto space = tibetan.press(0x20, u' ');
    assert(space && space->committed_text && *space->committed_text == "བཀྲ་");
    assert(!space->traditional_output && tibetan.editing().empty());
    // 空格不是选词：不回上屏帧，否则 TIP 会把同样的文字再写一次。
    assert(!space->encoded || (*space->encoded && space->encoded->packet.msg_type == FanyImeReplyType::NavigationIgnored));
    tibetan.type("shis");
    const auto shad = tibetan.press_char('/');
    assert(shad && shad->committed_text && *shad->committed_text == "ཤིས།");
    assert(tibetan.editing().empty());
    // 没有组字时 `/` 单独上屏垂符。
    const auto lone = tibetan.press_char('/');
    assert(lone && lone->committed_text && *lone->committed_text == "།");
    assert(tibetan.editing().empty());
  }

  // 回车只上屏藏文，不加音节点，也不回帧：TIP 已经上屏并吃掉了这个键。
  {
    Fixture tibetan(serialized);
    tibetan.type("rgyas");
    const auto enter = tibetan.press(0x0D, 0);
    assert(enter && enter->committed_text && *enter->committed_text == "རྒྱས");
    assert(!enter->encoded && tibetan.editing().empty());
  }

  // `'` 随时参与拼写，`+` `.` `-` 在组字时参与拼写，即使减号和句号绑定了翻页；大写字母是不同的威利字母。
  for (const auto &[keys, expected] : {std::pair<const char *, const char *>{"'od", "འོད"},
                                       {"g.yag", "གཡག"},
                                       {"pad+ma", "པདྨ"},
                                       {"Ta", "ཊ"}}) {
    Fixture tibetan(serialized);
    tibetan.type(keys);
    assert(tibetan.preedit() == expected);
    const auto ended = tibetan.press_char('/');
    assert(ended && ended->committed_text && *ended->committed_text == std::string(expected) + "།");
  }
  {
    Fixture tibetan(serialized);
    tibetan.type("k-i");
    assert(tibetan.preedit() == "ཀྀ");
  }

  // 其他标点让音节串以半角跟上标点；数字结束音节串，数字本身由 TIP 插入，空闲时的数字不是选词。
  for (const auto &[code, text, expected] : {std::tuple<uint32_t, char16_t, const char *>{0xBC, u',', "ཀ,"},
                                             {0xBA, u';', "ཀ;"},
                                             {'1', u'1', "ཀ"}}) {
    Fixture tibetan(serialized);
    tibetan.type("ka");
    const auto ended = tibetan.press(code, text);
    assert(ended && ended->committed_text && *ended->committed_text == expected);
    assert(tibetan.editing().empty());
  }
  {
    Fixture tibetan(serialized);
    const auto idle = tibetan.press('3', u'3');
    assert(!idle || !idle->committed_text);
    assert(tibetan.editing().empty());
  }

  // 光标和编辑键只在 TIP 把它们排在前面的键后面时才到达 Server；它们按显示上屏，不回帧。
  for (const uint32_t code : {0x09u, 0x25u, 0x27u, 0x26u, 0x28u, 0x24u, 0x23u, 0x21u, 0x22u, 0x2Eu}) {
    Fixture tibetan(serialized);
    tibetan.type("ka");
    const auto ended = tibetan.press(code, 0);
    assert(ended && ended->committed_text && *ended->committed_text == "ཀ");
    assert(!ended->encoded && tibetan.editing().empty());
  }

  // 退格删掉一个威利按键。
  {
    Fixture tibetan(serialized);
    tibetan.type("sangs");
    assert(tibetan.press(0x08, u'\b'));
    assert(tibetan.preedit() == "སང");
  }

  // 第一次 Esc 把音节串显示回威利原文并继续组字，和 TIP 在自己的宿主会话里一样，不回帧；这时空格只上屏原文。第二次 Esc 丢弃音节串；终止组字时发出的路由清空一次就丢弃。
  {
    Fixture tibetan(serialized);
    tibetan.type("bkra");
    const auto restored = tibetan.press(0x1B, 0);
    assert(restored && !restored->committed_text && !restored->encoded);
    assert(tibetan.preedit() == "bkra");
    const auto raw = tibetan.press(0x20, u' ');
    assert(raw && raw->committed_text && *raw->committed_text == "bkra");
    assert(!raw->encoded);
    assert(tibetan.editing().empty());

    tibetan.type("bkra");
    assert(tibetan.press(0x1B, 0));
    const auto cancelled = tibetan.press(0x1B, 0);
    assert(cancelled && !cancelled->committed_text);
    assert(tibetan.editing().empty());
    tibetan.type("bkra");
    tibetan.session.cancel_composition(1);
    assert(tibetan.editing().empty());
  }

  // 拼音样式从 Server 读组字，所以字母的回复是到目前为止的音节串。
  {
    Fixture tibetan(serialized);
    const auto reply = tibetan.press('K', u'k', 0, TsfPreeditStyle::Pinyin);
    assert(reply && reply->encoded && *reply->encoded &&
           reply->encoded->packet.msg_type == FanyImeReplyType::Preedit);
  }
  return 0;
}

// 二三候选：组字中 ';' 选第二个候选、'\'' 选第三个。TIP 和 Server 用同一条规则（common/SecondThirdCandidatePolicy.h）归类这两个键，这里先测规则本身，再在真实 Engine 会话上走 Server 的按键路由（ReplyComposer::configured_key）。
#include "../../src/ipc/ReplyComposer.h"
#include "../../common/SecondThirdCandidatePolicy.h"
#include "../core/TestHostOptions.h"
#include <chrono>
#include <filesystem>
#include <iostream>
#include <stdexcept>
#include <string>
#include <vector>

using namespace msime::windows;
using Json = nlohmann::json;

namespace {
void require(bool condition, const char *message) {
  if (!condition)
    throw std::runtime_error(message);
}

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

struct Fixture {
  ServerSession session;
  ReplyComposer composer{42, 1};
  uint64_t request = 0;
  NavigationBindings bindings = preference_navigation(Json::object());

  Fixture(const std::string &options, bool enabled) : session(42, options) {
    session.activate(1);
    bindings.second_third_candidate = enabled;
  }

  std::optional<PendingReply> press(uint32_t code, char16_t text, uint32_t modifiers = 0) {
    const auto packet = key(++request, code, text, modifiers);
    auto reply = composer.configured_key(session, packet, 1, TsfPreeditStyle::Local, bindings);
    if (reply)
      composer.confirm_delivery(42, 1, packet.request_id);
    return reply;
  }
  std::string editing() const { return session.view().at("editing_text").get<std::string>(); }
  // Shift+T 进入日期时间模式，再打 rq：不需要词库就有好几行日期候选。
  std::vector<std::string> open_dates() {
    press('T', u'T', 1);
    press('R', u'r');
    press('Q', u'q');
    const auto view = session.view();
    require(view.at("local_mode") == "date_time", "Shift+T did not open the date and time mode");
    std::vector<std::string> texts;
    for (const auto &candidate : view.at("candidates"))
      texts.push_back(candidate.at("text").get<std::string>());
    require(texts.size() >= 3, "The date mode listed fewer than three rows");
    return texts;
  }
};

void policy_tests() {
  // 虚拟键码和打出的字符都要对上：Shift 打出的 ':' '"'、别的布局在这两个键上打出的字符、别的键打出的 ';' 都不算。
  require(second_third_candidate_slot(0xBA, ';') == std::optional<std::size_t>(1), "';' is not the second slot");
  require(second_third_candidate_slot(0xDE, '\'') == std::optional<std::size_t>(2), "'\\'' is not the third slot");
  require(!second_third_candidate_slot(0xBA, ':'), "Shift+';' selected");
  require(!second_third_candidate_slot(0xDE, '"'), "Shift+'\\'' selected");
  require(!second_third_candidate_slot(0xBA, 0x00FC), "A layout printing u-umlaut on VK_OEM_1 selected");
  require(!second_third_candidate_slot(0xC0, '\''), "Another key printing '\\'' selected");
  require(!second_third_candidate_slot('2', '2'), "A digit went through the second and third candidate rule");

  const auto select = [](int scheme, bool enabled = true, uint32_t modifiers = 0, bool composing = true,
                         bool english = false, bool input = false) {
    return second_third_candidate_selection(enabled, 0xBA, ';', modifiers, composing, scheme, english, input);
  };
  for (const int scheme : {scheme::Quanpin, scheme::Shuangpin, scheme::Wubi, scheme::Cantonese, scheme::Stroke})
    require(select(scheme) == std::optional<std::size_t>(1), "A scheme with a candidate list did not select");
  for (const int scheme :
       {scheme::Japanese, scheme::Korean, scheme::Zhuyin, scheme::Vietnamese, scheme::Tibetan})
    require(!select(scheme), "Japanese or a host-composed scheme selected");
  require(!select(scheme::Quanpin, false), "The switched-off rule selected");
  require(!select(scheme::Quanpin, true, 2), "Ctrl+';' selected");
  require(!select(scheme::Quanpin, true, 1), "A Shift chord selected");
  require(!select(scheme::Quanpin, true, 0, false), "';' selected with nothing composing");
  require(!select(scheme::Quanpin, true, 0, true, true), "';' selected in the Engine's English mode");
  require(!select(scheme::Quanpin, true, 0, true, false, true), "A key the Engine spells selected");

  // 微软双拼：光标前、上一个分隔符之后是奇数个字母时 ';' 是韵母 ing。
  require(microsoft_shuangpin_final_position(std::string("n"), 1), "';' after an initial is not ing");
  require(!microsoft_shuangpin_final_position(std::string("ni"), 2), "';' after a whole syllable is ing");
  require(microsoft_shuangpin_final_position(std::string("ni'h"), 4), "The separator did not restart the count");
  require(!microsoft_shuangpin_final_position(std::string("nih"), 2), "The caret was not where the count stops");
  require(microsoft_shuangpin_final_position(std::wstring(L"nih"), 9), "A caret past the end was not clamped");
  require(!microsoft_shuangpin_final_position(std::wstring(), 0), "An empty composition has an ing key");

  // Engine 视图里的输入：spelling_symbols 列出的符号，或者微软双拼声母后的 ';'。Server 和有宿主会话的 TIP 都用这一个函数读各自的视图。
  require(second_third_candidate_engine_input(";'", ';', false, std::string("www"), 3),
          "A ';' the URL mode spells was not input");
  require(second_third_candidate_engine_input("'", '\'', false, std::string("www"), 3),
          "A '\'' the URL mode spells was not input");
  require(!second_third_candidate_engine_input("", '\'', false, std::string("nihao"), 5),
          "A pinyin '\'' was input");
  require(second_third_candidate_engine_input("", ';', true, std::string("n"), 1),
          "The Microsoft Shuangpin final ing was not input");
  require(!second_third_candidate_engine_input("", ';', true, std::string("ni"), 2),
          "';' after a whole Microsoft Shuangpin syllable was input");
  require(!second_third_candidate_engine_input("", ';', false, std::string("n"), 1),
          "';' after an initial was input without Microsoft Shuangpin");
  require(!second_third_candidate_engine_input("", '\'', true, std::string("n"), 1),
          "'\'' was taken as the Microsoft Shuangpin final");

  // 偏好：整个对象缺省时关闭；缺字段按 client-core 的默认值；不认识的键位拒绝整份偏好。
  require(!preference_second_third_candidate(Json::object()), "A document without the key turned it on");
  require(preference_second_third_candidate(
              Json{{"second_third_candidate", {{"enabled", true}, {"keys", "semicolon_quote"}}}}),
          "The switched-on preference was not read");
  require(preference_second_third_candidate(Json{{"second_third_candidate", {{"enabled", true}}}}),
          "A preference without keys was not read with the default keys");
  require(!preference_second_third_candidate(Json{{"second_third_candidate", {{"keys", "semicolon_quote"}}}}),
          "A preference without enabled turned it on");
  bool rejected = false;
  try {
    preference_second_third_candidate(Json{{"second_third_candidate", {{"enabled", true}, {"keys", "shift"}}}});
  } catch (const std::exception &) {
    rejected = true;
  }
  require(rejected, "Unknown keys were accepted");
  // 随翻页键的快照一起交给按键路由；没有 navigation 时翻页键仍是默认值。
  const auto bindings =
      preference_navigation(Json{{"second_third_candidate", {{"enabled", true}, {"keys", "semicolon_quote"}}}});
  require(bindings.second_third_candidate && bindings.minus_equal && bindings.comma_period && !bindings.brackets,
          "The navigation snapshot lost the paging defaults or the second and third candidate keys");
  require(!preference_navigation(Json::object()).second_third_candidate, "The navigation default turned it on");
}

void route_tests(const std::filesystem::path &root) {
  // 日期时间模式默认开着。
  const auto options = test_host_options(root);
  const auto serialized = options.dump();

  // ';' 上屏第二行，'\'' 上屏第三行，和数字键 2、3 一样。
  {
    Fixture dates(serialized, true);
    const auto texts = dates.open_dates();
    const auto second = dates.press(0xBA, u';');
    require(second && second->committed_text == std::optional<std::string>(texts[1]),
            "';' did not commit the second row");
    require(dates.editing().empty(), "';' left the composition open");
    const auto again = dates.open_dates();
    const auto third = dates.press(0xDE, u'\'');
    require(third && third->committed_text == std::optional<std::string>(again[2]),
            "'\\'' did not commit the third row");
    require(dates.editing().empty(), "'\\'' left the composition open");
  }

  // 关着时 ';' 不选第二行。
  {
    Fixture dates(serialized, false);
    const auto texts = dates.open_dates();
    const auto punctuation = dates.press(0xBA, u';');
    require(!punctuation || punctuation->committed_text != std::optional<std::string>(texts[1]) ||
                texts[0] == texts[1],
            "';' picked the second row while the setting was off");
  }

  // 拼音组字中的 '\''：打开后不再是音节分隔符，关着时照旧分隔。
  {
    Fixture off(serialized, false);
    off.press('X', u'x');
    off.press('I', u'i');
    off.press(0xDE, u'\'');
    require(off.editing() == "xi'", "The switched-off '\\'' no longer separated syllables");
    Fixture on(serialized, true);
    on.press('X', u'x');
    on.press('I', u'i');
    require(on.press(0xDE, u'\'').has_value(), "The switched-on '\\'' was not taken");
    require(on.editing() != "xi'", "The switched-on '\\'' still separated syllables");
  }

  // 微软双拼：声母后的 ';' 是韵母 ing，仍是输入。
  {
    auto microsoft = options;
    microsoft["preferences"]["scheme"] = "shuangpin";
    microsoft["preferences"]["shuangpin_profile"] = "microsoft";
    Fixture shuangpin(microsoft.dump(), true);
    require(shuangpin.session.view().value("microsoft_shuangpin", false), "The fixture is not Microsoft shuangpin");
    shuangpin.press('N', u'n');
    shuangpin.press(0xBA, u';');
    require(shuangpin.editing() == "n;", "';' after an initial was not composed as ing");
    shuangpin.press(0xBA, u';');
    require(shuangpin.editing() != "n;;", "';' after a whole syllable was composed");
  }
}
} // namespace

int main() {
  const auto root = std::filesystem::temp_directory_path() /
                    ("msime-second-third-candidate-" +
                     std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
  std::filesystem::create_directory(root);
  struct Cleanup {
    std::filesystem::path root;
    ~Cleanup() {
      std::error_code ec;
      std::filesystem::remove_all(root, ec);
    }
  } cleanup{root};
  try {
    policy_tests();
    route_tests(root);
  } catch (const std::exception &failure) {
    std::cerr << failure.what() << '\n';
    return 1;
  }
  std::cout << "second and third candidate keys: ok\n";
  return 0;
}

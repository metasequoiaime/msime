// 释义列快捷键在真实 Engine 会话上的行为：Tab/Shift+Tab 预选高亮候选的释义列并把列号带进 view（候选窗据此画下划线），预选后空格和数字上屏那一列，Alt/Ctrl+数字直接上屏第 1/2 列。释义都按精确文本上屏、组字取消，不经过候选选择，引擎不会学到用户没选的词。
#include "ipc/ReplyComposer.h"
#include "TranslationDisplay.h"
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <filesystem>
#include <string>
using namespace msime::windows;

namespace {
void require(bool value, const char *what) {
  if (!value) {
    std::fprintf(stderr, "FAIL: %s\n", what);
    std::exit(EXIT_FAILURE);
  }
}

std::u16string payload(const PendingReply &reply) {
  std::u16string text;
  for (auto c : reply.encoded->packet.candidate_string) {
    if (!c)
      break;
    text += static_cast<char16_t>(c);
  }
  return text;
}
} // namespace

int main() {
  const auto root = std::filesystem::temp_directory_path() /
                    ("msime-gloss-column-" +
                     std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
  std::filesystem::create_directory(root);
  struct Cleanup {
    std::filesystem::path root;
    ~Cleanup() {
      std::error_code ec;
      std::filesystem::remove_all(root, ec);
    }
  } cleanup{root};
  nlohmann::json options{{"api_version", 1},
                         {"preferences",
                          {{"scheme", "quanpin"},
                           {"learning", true},
                           {"candidate_page_size", 5},
                           {"chinese_punctuation", true},
                           {"candidate_translations", true}}}};
  for (const char *name : {"resources", "user_data", "cache", "dictionaries"}) {
    const auto directory = root / name;
    std::filesystem::create_directories(directory);
    options[name] = directory.u8string();
  }

  constexpr uint64_t epoch = 1;
  ServerSession session(42, options.dump());
  session.activate(epoch);
  uint64_t request = 0;
  const std::string glosses =
      std::string("hello") + std::string(translation_line_separator) + "こんにちは";
  // 组出「nihao」，候选「你好」带两种语言的释义。夹具没有词典，候选按 AI 服务的回答插进来。
  const auto compose = [&](const std::string &translation) {
    for (char c : std::string("nihao")) {
      FanyImeNamedpipeData packet{};
      packet.client_id = 42;
      packet.event_type = FanyImePipeEventType::KeyEvent;
      packet.request_id = ++request;
      packet.keycode = static_cast<uint32_t>(c - 'a' + 'A');
      packet.wch = static_cast<FanyImeWireChar>(c);
      session.key(packet, epoch);
    }
    const auto query = session.online_query(epoch);
    require(query.has_value(), "no online query for the composition");
    require(session.apply_ai_candidates(epoch, *query, R"(["你好"])").has_value(),
            "the fixture candidate was not applied");
    const auto view = session.view();
    require(!view.at("candidates").empty(), "the fixture produced no candidate");
    if (!translation.empty())
      require(session
                  .apply_translations(epoch, view.at("generation").get<uint64_t>(),
                                      nlohmann::json::array({{{"text", "你好"}, {"translation", translation}}})
                                          .dump())
                  .has_value(),
              "the translation was not applied");
  };
  const auto key = [&](uint32_t code, uint32_t modifiers, char16_t wch = 0) {
    FanyImeNamedpipeData packet{};
    packet.client_id = 42;
    packet.event_type = FanyImePipeEventType::KeyEvent;
    packet.request_id = ++request;
    packet.keycode = code;
    packet.wch = static_cast<FanyImeWireChar>(wch);
    packet.modifiers_down = PipeMetadata::CandidateActive | modifiers;
    return packet;
  };

  ReplyComposer composer(42, epoch);
  compose(glosses);
  // Tab 预选第 1 列，回执不上屏，view 带着列号。
  auto tab = key(0x09, 0);
  auto reply = composer.configured_key(session, tab, epoch, TsfPreeditStyle::Pinyin, {});
  require(reply.has_value() && reply->encoded && *reply->encoded, "Tab did not arm the gloss column");
  require(reply->encoded->packet.msg_type == FanyImeReplyType::NavigationIgnored,
          "arming a column committed or paged");
  require(reply->source.transition.at("view").value("armed_gloss_column", 0) == 1,
          "the armed column did not reach the view");
  require(composer.armed_gloss_column() == 1, "Tab did not arm column 1");
  composer.confirm_delivery(42, epoch, tab.request_id);
  // 再按 Tab 到第 2 列，Shift+Tab 回到第 1 列。
  tab = key(0x09, 0);
  reply = composer.configured_key(session, tab, epoch, TsfPreeditStyle::Pinyin, {});
  require(reply.has_value() && composer.armed_gloss_column() == 2, "Tab did not move to column 2");
  composer.confirm_delivery(42, epoch, tab.request_id);
  tab = key(0x09, 1u);
  reply = composer.configured_key(session, tab, epoch, TsfPreeditStyle::Pinyin, {});
  require(reply.has_value() && composer.armed_gloss_column() == 1, "Shift+Tab did not move back to column 1");
  composer.confirm_delivery(42, epoch, tab.request_id);
  // 空格上屏预选的那一列。
  auto space = key(0x20, 0, u' ');
  reply = composer.configured_key(session, space, epoch, TsfPreeditStyle::Pinyin, {});
  require(reply.has_value() && reply->encoded && *reply->encoded &&
              reply->encoded->packet.msg_type == FanyImeReplyType::CommitExactText,
          "Space with an armed column did not commit exact text");
  require(payload(*reply) == u"hello", "Space committed something other than column 1");
  require(!reply->ui_selection && reply->source.transition.at("commit").is_null(),
          "the gloss went through candidate selection");
  // 送达后候选窗按 transition 的 view 收起（candidate_presentation），缺了 view 会在输入队列里抛出并停掉整个队列。
  require(reply->source.transition.at("view").at("editing_text") == "",
          "the gloss commit's transition did not carry the cancelled view");
  require(session.view().at("editing_text") == "", "the gloss commit left the composition open");
  require(composer.armed_gloss_column() == 0, "the armed column outlived its commit");
  composer.confirm_delivery(42, epoch, space.request_id);

  // Tab 预选第 1 列之后，数字上屏那个候选的第 1 列。
  compose(glosses);
  tab = key(0x09, 0);
  reply = composer.configured_key(session, tab, epoch, TsfPreeditStyle::Pinyin, {});
  composer.confirm_delivery(42, epoch, tab.request_id);
  auto digit = key('1', 0, u'1');
  reply = composer.configured_key(session, digit, epoch, TsfPreeditStyle::Pinyin, {});
  require(reply.has_value() && reply->encoded && payload(*reply) == u"hello",
          "a digit with an armed column did not commit that column");
  composer.confirm_delivery(42, epoch, digit.request_id);

  // Ctrl+1 直接上屏第 2 列，Alt+1 上屏第 1 列。
  compose(glosses);
  auto chord = key('1', 2u);
  reply = composer.configured_key(session, chord, epoch, TsfPreeditStyle::Pinyin, {});
  require(reply.has_value() && reply->encoded && *reply->encoded &&
              reply->encoded->packet.msg_type == FanyImeReplyType::CommitExactText &&
              payload(*reply) == u"こんにちは",
          "Ctrl+1 did not commit the second-language gloss");
  require(session.view().at("editing_text") == "", "Ctrl+1 left the composition open");
  composer.confirm_delivery(42, epoch, chord.request_id);
  compose(glosses);
  chord = key('1', 4u);
  reply = composer.configured_key(session, chord, epoch, TsfPreeditStyle::Pinyin, {});
  require(reply.has_value() && reply->encoded && payload(*reply) == u"hello",
          "Alt+1 did not commit the first-language gloss");
  composer.confirm_delivery(42, epoch, chord.request_id);

  // 只有一种语言时 Ctrl+数字没有可上屏的：TIP 已经吃掉了这个键，回执什么也不做，组字留着。
  compose("hello");
  chord = key('1', 2u);
  reply = composer.configured_key(session, chord, epoch, TsfPreeditStyle::Pinyin, {});
  require(reply.has_value() && reply->encoded && *reply->encoded &&
              reply->encoded->packet.msg_type == FanyImeReplyType::NavigationIgnored,
          "Ctrl+1 without a second language was not acknowledged");
  require(session.view().at("editing_text") != "", "Ctrl+1 without a gloss dropped the composition");
  composer.confirm_delivery(42, epoch, chord.request_id);
  // 预选之后按别的键（这里是一个字母）就不再预选。
  tab = key(0x09, 0);
  reply = composer.configured_key(session, tab, epoch, TsfPreeditStyle::Pinyin, {});
  require(composer.armed_gloss_column() == 1, "Tab did not arm the only column");
  composer.confirm_delivery(42, epoch, tab.request_id);
  auto letter = key('A', 0, u'a');
  reply = composer.configured_key(session, letter, epoch, TsfPreeditStyle::Pinyin, {});
  require(composer.armed_gloss_column() == 0, "another key left the column armed");
  if (reply)
    composer.confirm_delivery(42, epoch, letter.request_id);

  // 鼠标点选候选之后不再预选：选中后的 view 不带列号，留着它会让下一个空格上屏看不见的那一列。
  composer.cancel();
  session.cancel_composition(epoch);
  compose(glosses);
  tab = key(0x09, 0);
  reply = composer.configured_key(session, tab, epoch, TsfPreeditStyle::Pinyin, {});
  require(composer.armed_gloss_column() == 1, "Tab did not arm before the click");
  composer.confirm_delivery(42, epoch, tab.request_id);
  {
    const auto view = session.view();
    const auto &first = view.at("candidates").at(0).at("id");
    const auto clicked = composer.select_candidate(session, first.at("session").get<uint64_t>(),
                                                   first.at("generation").get<uint64_t>(),
                                                   first.at("index").get<size_t>());
    require(clicked.has_value(), "the click was not taken");
    require(composer.armed_gloss_column() == 0, "a click left the column armed");
    composer.confirm_ui_delivery(42, epoch,
                                 clicked->source.transition.at("view").at("generation").get<uint64_t>());
  }

  // TIP 在候选列表打开时总会吃掉 Alt/Ctrl+数字，Server 不接的场合（这里是 UILess 宿主）也必须回执，否则 SessionPump 会断开客户端。
  composer.cancel();
  session.cancel_composition(epoch);
  compose(glosses);
  chord = key('1', 4u);
  chord.modifiers_down |= FanyImePipeFlags::UiLess;
  reply = composer.configured_key(session, chord, epoch, TsfPreeditStyle::Pinyin, {});
  require(reply.has_value() && reply->encoded && *reply->encoded && !reply->committed_text,
          "a UILess Alt+1 was not acknowledged without a commit");
  require(session.view().at("editing_text") != "", "a UILess Alt+1 dropped the composition");
  composer.confirm_delivery(42, epoch, chord.request_id);

  // Ctrl+Enter 打开释义页之后按了不带 CandidateActive 的键：普通组字里 TIP 只给释义页认的键带这一位，字母、退格、回车都不带。释义页随之关闭，之后的空格不再上屏旧义项。
  composer.cancel();
  session.cancel_composition(epoch);
  compose("hello; hi");
  auto translation = key(0x0D, 2u);
  reply = composer.configured_key(session, translation, epoch, TsfPreeditStyle::Pinyin, {});
  require(reply.has_value() && reply->encoded && *reply->encoded &&
              reply->encoded->packet.msg_type == FanyImeReplyType::NavigationIgnored &&
              reply->source.transition.at("view").at("candidates").size() == 2u,
          "Ctrl+Enter did not open the sense page");
  composer.confirm_delivery(42, epoch, translation.request_id);
  auto plain_letter = key('A', 0, u'a');
  plain_letter.modifiers_down = 0;
  require(!composer.translation_page_key(session, plain_letter, epoch), "the sense page took a letter");
  require(!composer.translation_page_key(session, key(0x20, 0, u' '), epoch),
          "a key without CandidateActive left the sense page open");
  return EXIT_SUCCESS;
}

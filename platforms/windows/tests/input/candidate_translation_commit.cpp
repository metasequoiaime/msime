// Ctrl+Enter on a candidate with a single translation sense, run against a real Engine session. The reference (HandleTranslationCommitKey) commits the sense as exact text and clears the composition; it never selects the candidate, so the Engine does not learn a word the user did not pick.
#include "ipc/ReplyComposer.h"
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
                    ("msime-translation-commit-" +
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
  for (char c : std::string("nihao")) {
    FanyImeNamedpipeData packet{};
    packet.client_id = 42;
    packet.event_type = FanyImePipeEventType::KeyEvent;
    packet.request_id = ++request;
    packet.keycode = static_cast<uint32_t>(c - 'a' + 'A');
    packet.wch = static_cast<FanyImeWireChar>(c);
    session.key(packet, epoch);
  }
  // The fixture has no dictionary, so the candidate comes in the way an AI provider's answer would.
  const auto query = session.online_query(epoch);
  require(query.has_value(), "no online query for the composition");
  require(session.apply_ai_candidates(epoch, *query, R"(["你好"])").has_value(), "the fixture candidate was not applied");
  auto view = session.view();
  require(!view.at("candidates").empty(), "the fixture produced no candidate");
  auto disable_ai = nlohmann::json{{"format_version", 1},
                                   {"revision", 1},
                                   {"preferences", options["preferences"]}};
  disable_ai["preferences"]["ai_assistant"]["enabled"] = false;
  const auto after_ai_disable = session.update_preferences(epoch, disable_ai.dump());
  require(after_ai_disable.at("view").at("candidates").empty(),
          "disabling AI candidates left the displayed row");
  disable_ai["revision"] = 2;
  disable_ai["preferences"]["ai_assistant"]["enabled"] = true;
  session.update_preferences(epoch, disable_ai.dump());
  view = session.view();
  const auto ai_query = session.online_query(epoch);
  require(ai_query.has_value(), "AI query disappeared after re-enabling candidates");
  require(session.apply_ai_candidates(epoch, *ai_query, R"(["你好"])").has_value(),
          "the AI candidate could not be restored after the preference test");
  view = session.view();
  require(session.apply_translations(epoch, view.at("generation").get<uint64_t>(),
                                     R"([{"text":"你好","translation":"hello"}])")
              .has_value(),
          "the translation was not applied");
  auto disabled = nlohmann::json{{"format_version", 1},
                                 {"revision", 3},
                                 {"preferences", disable_ai["preferences"]}};
  disabled["preferences"]["candidate_translations"] = false;
  const auto after_disable = session.update_preferences(epoch, disabled.dump());
  require(after_disable.at("view").at("candidates").at(0).value("translation", "") == "",
          "disabling translations clears the displayed gloss");
  disabled["revision"] = 4;
  disabled["preferences"]["candidate_translations"] = true;
  session.update_preferences(epoch, disabled.dump());
  view = session.view();
  const auto restored_query = session.online_query(epoch);
  require(restored_query.has_value(), "AI query disappeared after restoring translations");
  require(session.apply_ai_candidates(epoch, *restored_query, R"(["你好"])").has_value(),
          "the AI candidate could not be restored after restoring translations");
  view = session.view();
  require(session.apply_translations(epoch, view.at("generation").get<uint64_t>(),
                                     R"([{"text":"你好","translation":"hello"}])")
              .has_value(),
          "the translation could not be restored after the preference test");

  ReplyComposer composer(42, epoch);
  FanyImeNamedpipeData enter{};
  enter.client_id = 42;
  enter.event_type = FanyImePipeEventType::KeyEvent;
  enter.request_id = ++request;
  enter.keycode = 0x0D;
  enter.modifiers_down = PipeMetadata::CandidateActive | 2u;
  const auto reply = composer.configured_key(session, enter, epoch, TsfPreeditStyle::Pinyin, {});
  require(reply.has_value(), "Ctrl+Enter did not route the highlighted translation");
  require(!reply->ui_selection, "Ctrl+Enter went through candidate selection");
  require(reply->encoded && *reply->encoded && reply->encoded->packet.msg_type == FanyImeReplyType::CommitExactText,
          "Ctrl+Enter did not commit exact text");
  require(payload(*reply) == u"hello", "Ctrl+Enter committed something other than the sense");
  require(reply->committed_text == "hello" && reply->next_prefix.empty(), "Ctrl+Enter kept a prefix");
  require(reply->source.transition.at("commit").is_null(), "Ctrl+Enter reported an Engine commit");
  // 送达后候选窗按 transition 的 view 收起（candidate_presentation），缺了 view 会在输入队列里抛出并停掉整个队列。
  require(reply->source.transition.at("view").at("editing_text") == "",
          "Ctrl+Enter's transition did not carry the cancelled view");
  require(session.view().at("editing_text") == "", "Ctrl+Enter left the composition open");
  composer.confirm_delivery(42, epoch, enter.request_id);
  return EXIT_SUCCESS;
}

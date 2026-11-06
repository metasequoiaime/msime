// TIP 新建的宿主会话必须立即带上用户当前的偏好，不能停在 runtime-options.json 里的版本默认值（#10867、#10887）。runtime-options.json 是 Server 首次准备状态目录时写的，里面的方案是全拼；用户改成小鹤双拼后只有偏好快照里是双拼。以前同一个 TIP 对象 Deactivate 再 ActivateEx 时，新会话只按 runtime-options.json 建，偏好定时器又因为修订号没变不再套偏好，于是候选窗（Server，双拼）显示「情况」，TIP 这份全拼会话只消耗一部分按键，上屏「情况kl」。这里用真实引擎会话和合成词库验证 create_host_session（HostSessionStart.h）：同一份全拼宿主选项，只建不套时是全拼、选首选留下余码；带上双拼快照建时是双拼、首选整段上屏。不读用户或发布词库。
#include "../../tsf/HostSessionStart.h"
#include "../../common/InputSchemeTraits.h"
#include <nlohmann/json.hpp>
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <filesystem>
#include <string>

using nlohmann::json;
namespace {
const char *phase = "initialization";
std::filesystem::path fixture;
void require(bool value, const char *message) {
  if (!value) { std::fprintf(stderr, "FAIL (%s): %s\n", phase, message); std::exit(EXIT_FAILURE); }
}

json default_preferences() {
  char *defaults = msime_client_default_preferences();
  require(defaults != nullptr, "shared preference defaults available");
  auto preferences = json::parse(defaults).at("value");
  msime_client_string_free(defaults);
  preferences["default_ime_mode"] = "chinese";
  preferences["learning"] = false;
  preferences["candidate_page_size"] = 5;
  return preferences;
}

// runtime-options.json 的样子：preferences 是准备状态目录时的版本默认值，方案是全拼。
std::string host_options(const std::filesystem::path &root) {
  auto preferences = default_preferences();
  preferences["scheme"] = "quanpin";
  json options{{"api_version", 1}, {"preferences", preferences}};
  for (const char *name : {"resources", "user_data", "cache", "dictionaries"}) {
    std::filesystem::create_directories(root / name); options[name] = (root / name).u8string();
  }
  for (const char *name : {"resources", "dictionaries"})
    std::filesystem::copy_file(fixture, root / name / "msime-pinyin.db");
  return options.dump();
}

// HostConfiguration 的 preferenceSnapshot：用户在设置里选了小鹤双拼之后的偏好。
std::string shuangpin_snapshot() {
  auto preferences = default_preferences();
  preferences["scheme"] = "shuangpin";
  return json{{"format_version", 1}, {"revision", 7}, {"preferences", preferences}}.dump();
}

msime::tsf::EngineView view_of(msime::tsf::EngineSessionAdapter &session) {
  std::string raw, error;
  msime::tsf::EngineResult parsed;
  require(session.view(&raw, &error) && msime::tsf::EngineSessionAdapter::parse_result(raw, &parsed, &error), "session view");
  return parsed.view;
}

struct Typed {
  uint32_t scheme = 0;
  std::string first;
  json selected;
  std::string remaining;
};

// 小鹤双拼 nihc = ni'hao；全拼读作 ni、h、c。打完选第一页首选，和 TIP 跟着 Server 选词一样。
Typed type_and_select_first(const std::string &snapshot) {
  const auto root = std::filesystem::temp_directory_path() /
      ("msime-host-session-preferences-" + std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
  std::filesystem::create_directories(root);
  struct Cleanup { std::filesystem::path root;
    ~Cleanup() { std::error_code error; std::filesystem::remove_all(root, error); }
  } cleanup{root};
  msime::tsf::EngineSessionAdapter session;
  std::string raw, error;
  require(msime::tsf::create_host_session(session, host_options(root), snapshot, &error), "host session created");
  require(session.focus(true, &raw, &error), "host session focused");
  for (const unsigned char letter : std::string("nihc"))
    require(session.character(letter, false, &raw, &error), "host session types");
  Typed typed;
  const auto view = view_of(session);
  typed.scheme = view.scheme;
  require(!view.candidates.empty(), "the reading has candidates");
  typed.first = view.candidates.front().text;
  require(session.select(view.generation, view.candidates.front().index, &raw, &error), "first candidate selected");
  typed.selected = json::parse(raw).at("value");
  typed.remaining = view_of(session).editing_text;
  return typed;
}
} // namespace

int main(int argc, char **argv) {
  require(argc == 2, "synthetic dictionary fixture is required");
  fixture = std::filesystem::path(argv[1]);
  try {
    phase = "options only";
    // 对照：只按 runtime-options.json 建的会话是全拼，就是 #10867 里 TIP 那一份。
    const auto stale = type_and_select_first("");
    require(stale.scheme == static_cast<uint32_t>(msime::windows::scheme::Quanpin), "a session built from the options alone runs the edition default");
    require(stale.selected.at("commit") != "拟好" && !stale.remaining.empty(), "under quanpin the first candidate does not consume every key");

    phase = "options and snapshot";
    const auto fresh = type_and_select_first(shuangpin_snapshot());
    require(fresh.scheme == static_cast<uint32_t>(msime::windows::scheme::Shuangpin), "the new session runs the user's scheme");
    require(fresh.first == "拟好", "shuangpin reads nihc as ni'hao");
    require(fresh.selected.at("commit") == "拟好" && fresh.remaining.empty(), "the first candidate commits whole, with no keys left over");
  } catch (const std::exception &error) {
    std::fprintf(stderr, "FAIL at %s: %s\n", phase, error.what());
    return EXIT_FAILURE;
  }
  std::puts("host session preferences: passed");
}

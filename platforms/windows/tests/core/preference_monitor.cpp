#include "PreferenceMonitor.h"
#include <filesystem>
#include <fstream>
#include <stdexcept>
#include <atomic>

namespace {
void require(bool value, const char *message) {
  if (!value)
    throw std::runtime_error(message);
}
template <class F> void await(F ready) {
  const auto deadline =
      std::chrono::steady_clock::now() + std::chrono::seconds(5);
  while (!ready()) {
    require(std::chrono::steady_clock::now() < deadline,
            "Preference monitor timed out");
    std::this_thread::sleep_for(std::chrono::milliseconds(5));
  }
}
} // namespace
void preference_monitor_tests(const std::string &options,
                              const std::string &directory) {
  using namespace msime::windows;
  auto write = [&](const std::string &value) {
    std::ofstream output(std::filesystem::u8path(directory) /
                         "preferences.json");
    output << value;
    output.close();
    require(static_cast<bool>(output), "Synthetic settings write failed");
  };
  auto document = nlohmann::json{
      {"format_version", 1},
      {"revision", 1},
      {"preferences", nlohmann::json::parse(options).at("preferences")}};
  write(document.dump());
  const auto older = PreferenceSnapshot::load(directory);
  document["revision"] = 2;
  document["preferences"]["navigation"] = {{"minus_equal", false}, {"comma_period", false},
    {"brackets", true}, {"tab", false}, {"page_up_down", false}, {"arrows", false}};
  document["preferences"]["word_character"] = {{"enabled", true}, {"keys", "minus_equal"}};
  document["preferences"]["keybindings"]["toggle_character_set_ctrl_shift_f"] = false;
  document["preferences"]["keybindings"]["toggle_japanese_ctrl_shift_j"] = true;
  write(document.dump());
  FocusGate gate;
  InputQueue input(gate, 1, 1, options);
  std::promise<void> entered, release;
  auto released = release.get_future().share();
  auto blocker = input.submit([&](InputState &) {
    entered.set_value();
    released.wait();
  });
  entered.get_future().wait();
  // Ensure the test never strands its synthetic blocking task on an assertion.
  struct Release {
    std::promise<void> &value;
    ~Release() {
      try {
        value.set_value();
      } catch (...) {
      }
    }
  } guard{release};
  auto filler = input.submit([](InputState &) {});
  require(filler.has_value(), "Synthetic queue fill failed");
  std::atomic<unsigned> callbacks{0};
  std::atomic<uint64_t> published_revision{0};
  std::atomic<bool> callback_saw_applied_preferences{false};
  PreferenceMonitor monitor(input, directory, std::chrono::milliseconds(10),
                            [&](const PreferenceSnapshot &snapshot) {
                              published_revision.store(snapshot.revision());
                              callbacks.fetch_add(1);
                              auto observation = input.submit(
                                  [&](InputState &state) {
                                    const auto navigation =
                                        state.navigation_bindings();
                                    callback_saw_applied_preferences.store(
                                        state.word_character_binding() ==
                                                WordCharacterBinding::MinusEqual &&
                                            !navigation.minus_equal &&
                                            navigation.brackets);
                                  });
                              if (observation)
                                (void)observation->get();
                            });
  // Stop monitor before Release unwinds: its pending tasks capture only values,
  // and stopping never waits for an input publication receipt.
  await([&] { return monitor.status() == PreferenceMonitorStatus::QueueFull; });
  release.set_value();
  require(blocker->get() == InputTaskStatus::Completed &&
              filler->get() == InputTaskStatus::Completed,
          "Synthetic queue unblock failed");
  await([&] { return monitor.status() == PreferenceMonitorStatus::Current; });
  require(callbacks.load() == 1 && published_revision.load() == 2,
          "Published preference callback was not confirmed after apply");
  require(callback_saw_applied_preferences.load(),
          "Published preference callback ran before the queue applied settings");
  const auto completed = input.stats().completed;
  std::this_thread::sleep_for(std::chrono::milliseconds(60));
  require(input.stats().completed == completed,
          "Unchanged settings were repeatedly published");
  auto check = input.submit([&](InputState &state) {
    bool rejected = false;
    try {
      state.publish_preferences(older);
    } catch (const std::invalid_argument &) {
      rejected = true;
    }
    require(rejected, "Monitor did not publish newest settings");
    const auto navigation = state.navigation_bindings();
    require(state.word_character_binding() == WordCharacterBinding::MinusEqual,
            "Live word binding was not published or was reverted by stale settings");
    require(!state.character_set_shortcut_enabled(),
            "Live character-set shortcut setting was not published");
    require(state.japanese_toggle_shortcut_enabled(),
            "Live Japanese toggle shortcut setting was not published");
    // Tab 和 PageUp/PageDown 固定开启（preference_navigation），存着的关不起作用。
    require(!navigation.minus_equal && !navigation.comma_period && navigation.brackets &&
            navigation.tab && navigation.page_up_down && !navigation.arrows,
            "Live navigation settings were not published or were reverted by stale settings");
    bool join_rejected = false;
    try {
      monitor.stop();
    } catch (const std::logic_error &) {
      join_rejected = true;
    }
    require(join_rejected, "Input thread joined dependent monitor");
  });
  require(check && check->get() == InputTaskStatus::Completed,
          "Monitor publication check failed");
  write("broken");
  await(
      [&] { return monitor.status() == PreferenceMonitorStatus::ReadFailed; });
  require(!monitor.failed(), "Read failure killed settings monitoring");
  write(older.serialized());
  std::this_thread::sleep_for(std::chrono::milliseconds(40));
  require(monitor.status() == PreferenceMonitorStatus::ReadFailed,
          "Stale file replaced settings");
  document["revision"] = 3;
  write(document.dump());
  await([&] { return monitor.status() == PreferenceMonitorStatus::Current; });
  require(callbacks.load() == 2 && published_revision.load() == 3,
          "Published preference callback did not observe the new revision");
  document["revision"] = 4;
  document["preferences"]["keybindings"]["toggle_japanese_ctrl_shift_j"] = false;
  write(document.dump());
  monitor.wake();
  await([&] { return published_revision.load() == 4; });
  require(callbacks.load() == 3,
          "Explicit preference wake did not publish the new revision");
  input.stop();
  await([&] { return monitor.failed(); });
  monitor.stop();
  monitor.stop();
  InputQueue idle(gate, 1, 1, options);
  PreferenceMonitor stopping(idle, directory, std::chrono::seconds(60));
  const auto start = std::chrono::steady_clock::now();
  stopping.request_stop();
  stopping.stop();
  require(stopping.status() == PreferenceMonitorStatus::Stopped &&
              std::chrono::steady_clock::now() - start <
                  std::chrono::seconds(2),
          "Monitor stop waited for poll interval");
  idle.stop();

  // 用一分钟空闲间隔区分显式唤醒与正常轮询，不靠短间隔掩盖丢失通知。
  document["revision"] = 5;
  document["preferences"]["keybindings"]["toggle_japanese_ctrl_shift_j"] = true;
  write(document.dump());
  InputQueue awakened(gate, 1, 1, options);
  std::atomic<uint64_t> awakened_revision{0};
  PreferenceMonitor long_poll(
      awakened, directory, std::chrono::seconds(60),
      [&](const PreferenceSnapshot &snapshot) {
        awakened_revision.store(snapshot.revision());
      });
  await([&] { return awakened.stats().completed >= 1; });
  // 输入队列的屏障确认旧发布任务已完成，后续只能由这次写入与 wake 推进。
  auto barrier = awakened.submit([](InputState &) {});
  require(barrier && barrier->get() == InputTaskStatus::Completed,
          "Explicit wake setup did not drain the initial publication");
  document["revision"] = 6;
  document["preferences"]["keybindings"]["toggle_japanese_ctrl_shift_j"] = false;
  write(document.dump());
  long_poll.wake();
  await([&] { return awakened_revision.load() == 6; });
  long_poll.stop();
  awakened.stop();
}

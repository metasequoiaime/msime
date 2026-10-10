#include "../../src/ipc/SessionController.h"
#include "../../src/system/DedicatedEnglishMailbox.h"
#include "../core/TestHostOptions.h"
#include <atomic>
#include <cassert>
#include <chrono>
#include <thread>
using namespace msime::windows;
class ModeTransport final : public MainTransport {
public:
  const PipeTicket ticket{42, {1, 2, 3}};
  bool current(const PipeTicket &value) override {
    return !closed && same_ticket(ticket, value);
  }
  bool try_current(const PipeTicket &value) override { return current(value); }
  std::optional<FanyImeNamedpipeData> read(const PipeTicket &value) override {
    std::unique_lock lock(mutex);
    if (!current(value)) return std::nullopt;
    if (!activated) {
      activated = true;
      FanyImeNamedpipeData packet{};
      packet.client_id = ticket.client;
      packet.event_type = FanyImePipeEventType::ClientActivated;
      packet.request_id = 77;
      return packet;
    }
    ready.wait(lock, [&] { return closed.load(); });
    return std::nullopt;
  }
  KeyEventSendResult send(const PipeTicket &value, uint32_t,
                         const std::vector<uint8_t> &) override {
    if (!current(value)) return KeyEventSendResult::DefinitelyNotSent;
    ++writes;
    return KeyEventSendResult::Sent;
  }
  void close(const PipeTicket &) noexcept override {
    std::lock_guard lock(mutex);
    closed = true;
    ready.notify_all();
  }
  std::atomic<unsigned> writes{0};
private:
  std::atomic<bool> closed{false};
  bool activated = false;
  std::mutex mutex;
  std::condition_variable ready;
};
int main() {
  const auto root = std::filesystem::temp_directory_path() /
      ("msime-english-controller-" + std::to_string(
          std::chrono::steady_clock::now().time_since_epoch().count()));
  std::filesystem::create_directory(root);
  struct Cleanup {
    std::filesystem::path path;
    ~Cleanup() { std::error_code ec; std::filesystem::remove_all(path, ec); }
  } cleanup{root};
  // Both defaults describe the host's own passthrough for a new focus
  // session. Neither may start the Engine in dedicated English, which the
  // CN/EN switch cannot leave.
  for (const char *mode : {"chinese", "english"}) {
    auto options = test_host_options(root / mode);
    options["preferences"]["default_ime_mode"] = mode;
    ModeTransport transport;
    RegistrationInbox inbox(1);
    std::promise<FocusLease> activation;
    auto activated = activation.get_future();
    SessionController *owner = nullptr;
    SessionController controller(transport, inbox, 1, 8, options.dump(),
        [](InputState &, const FocusLease &, const FanyImeNamedpipeData &)
            -> std::optional<PendingReply> { return std::nullopt; },
        [&](const FocusRoute &route, const FanyImeNamedpipeData &) {
          bool rejected = false;
          try { (void)owner->dedicated_english_state(*route.route); }
          catch (const std::logic_error &) { rejected = true; }
          assert(rejected);
          activation.set_value(*route.route);
          return true;
        }, [] { return true; }, [&] { transport.close(transport.ticket); });
    owner = &controller;
    assert(!controller.dedicated_english_state({}));
    assert(inbox.push(transport.ticket));
    assert(activated.wait_for(std::chrono::seconds(10)) == std::future_status::ready);
    const auto lease = activated.get();
    std::optional<bool> result;
    const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(5);
    while (!(result = controller.dedicated_english_state(lease)) &&
           std::chrono::steady_clock::now() < deadline)
      std::this_thread::sleep_for(std::chrono::milliseconds(1));
    assert(result && *result == false);
    assert(transport.writes == 1); // Only the activation fence; reads send nothing.
    assert(controller.reset_cache());
    DedicatedEnglishMailbox mailbox;
    assert(!mailbox.snapshot(lease));
    mailbox.publish(lease, *result);
    assert(mailbox.snapshot(lease) == false);
    // The mailbox reports whatever the queue observed, including a mode the
    // user turns on at runtime; only the lease decides what it will answer.
    mailbox.publish(lease, true);
    assert(mailbox.snapshot(lease) == true);
    auto stale = lease;
    ++stale.token;
    assert(!controller.dedicated_english_state(stale));
    assert(!mailbox.snapshot(stale));
    stale = lease;
    ++stale.transport.generations[0];
    assert(!controller.dedicated_english_state(stale));
    assert(!mailbox.snapshot(stale));
    // 托盘「英文候选模式」在焦点会话上设置 Engine 的英文模式，读回来就是新状态；它不向 TIP 写任何东西，新状态之后由 DedicatedEnglishChanged 推送。过期的租约什么也不动。
    assert(!controller.set_dedicated_english(stale, true));
    assert(controller.set_dedicated_english(lease, true));
    assert(controller.dedicated_english_state(lease) == true);
    assert(controller.set_dedicated_english(lease, false));
    assert(controller.dedicated_english_state(lease) == false);
    // 250 毫秒一次的英文模式读取和托盘点击抢同一把事务锁。读取占着锁时托盘开关等它做完再切，不会点了没反应。
    {
      std::atomic<bool> reading{true};
      std::thread reader([&] {
        while (reading.load()) {
          const auto started = std::chrono::steady_clock::now();
          (void)controller.dedicated_english_state(lease);
          // 每次读完空出和这次读取一样长的时间再读，锁大约一半时间被占着：只试一次的开关二十次里几乎必然撞上，等锁的开关每次都等得到。读取之间不留空隙时锁几乎一直被占着，每 5 毫秒一次的重试能否撞上空档全凭调度，在 Wine 和繁忙的 CI 上会把 2 秒等完。
          const auto now = std::chrono::steady_clock::now();
          const auto until = now + (now - started);
          while (std::chrono::steady_clock::now() < until)
            std::this_thread::yield();
        }
      });
      for (int round = 0; round < 20; ++round)
        assert(controller.set_dedicated_english(lease, round % 2 == 0));
      reading.store(false);
      reader.join();
    }
    assert(controller.dedicated_english_state(lease) == false);
    assert(transport.writes == 1);
    controller.stop();
    assert(!controller.dedicated_english_state(lease));
  }
}

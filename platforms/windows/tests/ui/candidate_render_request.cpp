#include "CandidateMailbox.h"
#include "CandidateRenderSync.h"

#include <atomic>
#include <chrono>
#include <condition_variable>
#include <mutex>
#include <stdexcept>
#include <string>
#include <thread>

using namespace msime::windows;

namespace {
void require(bool condition, const char *what) {
  if (!condition)
    throw std::runtime_error(std::string("candidate render request regression: ") + what);
}

// 模拟候选窗所在的界面线程：平时按很长的周期轮询（远大于按键屏障的 30 毫秒），被请求时立即读快照、画、回执。#9519 里拖住空格的正是这段轮询间隔。
class FakeCandidateUi {
public:
  FakeCandidateUi(CandidateMailbox &mailbox, FocusGate &gate) : mailbox_(mailbox), gate_(gate) {
    thread_ = std::thread([this] { run(); });
  }
  ~FakeCandidateUi() {
    {
      std::lock_guard lock(mutex_);
      stopping_ = true;
    }
    wake_.notify_all();
    thread_.join();
  }
  // 交给 CandidateMailbox 的绘制请求：只做通知，不回调信箱。
  void request() {
    requests_.fetch_add(1);
    {
      std::lock_guard lock(mutex_);
      requested_ = true;
    }
    wake_.notify_all();
  }
  int requests() const { return requests_.load(); }

private:
  void run() {
    std::unique_lock lock(mutex_);
    for (;;) {
      wake_.wait_for(lock, poll_interval, [&] { return stopping_ || requested_; });
      if (stopping_)
        return;
      requested_ = false;
      lock.unlock();
      // 测试只验证请求链路，用阻塞读取避开与屏障线程瞬间争用信箱锁造成的偶发读空；产品的非阻塞读取读空时屏障照旧有界放行。
      if (const auto shown = mailbox_.snapshot(gate_))
        mailbox_.rendered(shown->lease, shown->render_serial);
      lock.lock();
    }
  }
  static constexpr auto poll_interval = std::chrono::milliseconds(500);
  CandidateMailbox &mailbox_;
  FocusGate &gate_;
  std::mutex mutex_;
  std::condition_variable wake_;
  bool requested_ = false;
  bool stopping_ = false;
  std::atomic<int> requests_{0};
  std::thread thread_;
};
} // namespace

int main() {
  FocusGate gate;
  CandidateMailbox mailbox;
  const PipeTicket ticket{42, {1, 2, 3}};
  const auto change = gate.begin(ticket, 7);
  require(change.has_value(), "focus begin");
  require(gate.acknowledge(change->pending, [] { return true; }), "focus acknowledge");
  const FocusLease lease = change->pending;
  uint64_t generation = 0;
  // 与按键回包一样，在活动焦点下发布一页带候选的组字；每发布一次 render_serial 加一。
  const auto publish = [&] {
    ++generation;
    FanyImeNamedpipeData packet{};
    packet.client_id = ticket.client;
    packet.request_id = generation;
    packet.event_type = FanyImePipeEventType::KeyEvent;
    PendingReply reply{};
    reply.source.client_id = packet.client_id;
    reply.source.activation_epoch = lease.epoch;
    reply.source.request_id = packet.request_id;
    reply.source.transition = {{"view",
                                {{"session", 1},
                                 {"generation", generation},
                                 {"focused", true},
                                 {"editing_text", "U4"},
                                 {"preedit", "U4"},
                                 {"candidates", nlohmann::json::array()}}}};
    require(gate.with_active(lease, [&] { mailbox.delivered(lease, reply, packet); }), "publish");
    const auto shown = mailbox.snapshot(gate);
    require(shown && shown->visible, "published page visible");
    return shown->render_serial;
  };
  const auto budget = std::chrono::milliseconds(candidate_render_wait_max_ms);

  {
    FakeCandidateUi ui(mailbox, gate);
    mailbox.set_render_request([&] { ui.request(); });

    // 快速连打：上一键刚发布新页，界面线程还没轮询到，空格就到了。屏障必须叫醒界面线程，在预算内等到回执，而不是空等满 30 毫秒。
    const auto serial = publish();
    const auto started = std::chrono::steady_clock::now();
    require(mailbox.wait_rendered(lease, serial, budget), "barrier receives the receipt it requested");
    const auto waited = std::chrono::steady_clock::now() - started;
    require(waited < budget, "barrier returned before its budget");
    require(ui.requests() == 1, "one paint request per unsatisfied barrier");

    // 已经画过的页不再请求绘制，屏障直接通过。
    require(mailbox.wait_rendered(lease, serial, budget), "painted page passes");
    require(ui.requests() == 1, "painted page asks for no paint");

    // 连续几页都由各自的屏障叫醒，每一页都在预算内拿到回执（重连后重放整段前缀再按空格就是这样）。
    for (int i = 0; i < 8; ++i) {
      const auto next = publish();
      require(mailbox.wait_rendered(lease, next, budget), "replayed page receives its receipt");
    }
    require(ui.requests() == 9, "every replayed page requested its own paint");

    // 撤掉请求之后（候选窗先于 Server 收尾）不再调用旧的回调，屏障回到有界超时。
    mailbox.set_render_request({});
    const auto orphan = publish();
    require(!mailbox.wait_rendered(lease, orphan, std::chrono::milliseconds(5)), "no paint without a requester");
    require(ui.requests() == 9, "cleared requester is never called");
  }

  // 停止后屏障立即放行，不再请求绘制。
  {
    int calls = 0;
    mailbox.set_render_request([&] { ++calls; });
    const auto serial = publish();
    mailbox.stop();
    require(!mailbox.wait_rendered(lease, serial, budget), "stopped mailbox does not report a paint");
    require(calls == 0, "stopped mailbox asks for no paint");
    mailbox.set_render_request({});
  }
  return 0;
}

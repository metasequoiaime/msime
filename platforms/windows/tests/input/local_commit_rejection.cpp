#include "ReplyComposer.h"
#include "KeyEvent.h"
#include "../core/TestHostOptions.h"
#include <chrono>
#include <filesystem>
#include <iostream>
#include <stdexcept>
#include <string>

using namespace msime::windows;

// 本地回车的原文观测是 TIP 交来的数据。#9525、#9706 里它和 Server 的组字对不上，`ReplyComposer::dispatch` 抛出的异常一路冒到输入队列，整个 Server 停机重启。这里对真实 Engine 会话核对：观测缺失或不符时抛出带固定校验名的 RequestRejected（SessionPump 只接住这一种），Engine 和回复合成器都不动，诊断文本里没有输入内容，同一个会话里下一个键照常处理，随后正确的观测照常上屏。
namespace {
void require(bool value, const std::string &what) {
  if (!value)
    throw std::runtime_error("Local commit rejection: " + what);
}

FanyImeNamedpipeData key(uint64_t request, uint32_t code, char16_t text) {
  FanyImeNamedpipeData packet{};
  packet.client_id = 42;
  packet.event_type = FanyImePipeEventType::KeyEvent;
  packet.request_id = request;
  packet.keycode = code;
  packet.wch = text;
  return packet;
}

FanyImeNamedpipeData enter(uint64_t request, const std::u16string &observation) {
  auto packet = key(request, 0x0D, 0);
  packet.pinyin_length = static_cast<int>(observation.size());
  for (size_t index = 0; index < observation.size(); ++index)
    packet.pinyin_string[index] = observation[index];
  return packet;
}

// 拒绝时返回异常的副本；没有拒绝、或者抛出别的异常，都算失败。
RequestRejected rejected(const std::function<void()> &action, const std::string &what) {
  try {
    action();
  } catch (const RequestRejected &error) {
    return error;
  }
  throw std::runtime_error("Local commit rejection: not rejected: " + what);
}

void run(const std::filesystem::path &root, const char *scheme) {
  auto options = test_host_options(root / scheme);
  options["preferences"]["scheme"] = scheme;
  ServerSession session(42, options.dump());
  session.activate(1);
  ReplyComposer composer(42, 1);
  uint64_t request = 0;
  const NavigationBindings bindings{};
  const auto press = [&](const FanyImeNamedpipeData &packet) {
    auto reply = composer.configured_key(session, packet, 1, TsfPreeditStyle::Local, bindings,
                                         local_commit_observation(packet));
    if (reply)
      composer.confirm_delivery(42, 1, packet.request_id);
    return reply;
  };
  const auto type = [&](char letter) {
    require(press(key(++request, static_cast<uint32_t>(letter - 'a' + 'A'), static_cast<char16_t>(letter)))
                .has_value(),
            std::string(scheme) + " letter reply");
  };
  const auto editing = [&] { return session.view().at("editing_text").get<std::string>(); };

  type('a');
  type('b');
  require(editing() == "ab", std::string(scheme) + " composing ab");
  const auto before = session.view();

  // 观测缺失、长度不符、同长不同文三种，都只拒绝这一次请求。
  struct Case {
    std::u16string observation;
    const char *detail;
  };
  for (const auto &item : {Case{u"", "observation=mismatch observed_bytes=0 expected_bytes=2"},
                           Case{u"a", "observation=mismatch observed_bytes=1 expected_bytes=2"},
                           Case{u"ba", "observation=mismatch observed_bytes=2 expected_bytes=2"}}) {
    const auto packet = enter(++request, item.observation);
    const auto error = rejected([&] { (void)press(packet); }, std::string(scheme) + " " + item.detail);
    require(std::string(error.check()) == "local_commit_observation", "check name");
    require(error.detail() == item.detail, std::string(scheme) + " detail: " + error.detail());
    const std::string text = std::string(error.what()) + " " + error.detail();
    require(text.find("ab") == std::string::npos && text.find("ba") == std::string::npos,
            "diagnostic carries no input text");
    require(session.view() == before, std::string(scheme) + " rejected observation preserves the Engine view");
    require(!composer.has_pending() && composer.selected_prefix().empty(),
            std::string(scheme) + " rejected observation leaves no pending reply");
  }
  // 完全没有观测（不是回车载荷）也是同一项校验。
  {
    const auto packet = enter(++request, u"");
    const auto error = rejected(
        [&] {
          (void)composer.configured_key(session, packet, 1, TsfPreeditStyle::Local, bindings, std::nullopt);
        },
        std::string(scheme) + " missing observation");
    require(error.detail() == "observation=missing expected_bytes=2", "missing detail: " + error.detail());
    require(session.view() == before && !composer.has_pending(), "missing observation preserves state");
  }

  // 被拒之后同一个会话里的下一个键照常处理，组字接着往下走。
  type('c');
  require(editing() == "abc", std::string(scheme) + " next key after rejection: " + editing());
  // 和组字一致的观测照常上屏，并清掉组字。
  const auto committed = press(enter(++request, u"abc"));
  require(committed && !committed->encoded && committed->committed_text == std::string("abc"),
          std::string(scheme) + " matching observation commits");
  require(editing().empty() && composer.selected_prefix().empty(), std::string(scheme) + " commit clears composition");

  // 回车路由落在不是原文提交的键上是内部不变量被破坏，不能当作可以放过的单个请求。
  type('d');
  auto letter = key(++request, 'E', u'e');
  bool invariant = false;
  try {
    (void)composer.dispatch(session, letter, 1, ReplyPath::LocalCommit, false, std::string("d"));
  } catch (const RequestRejected &) {
  } catch (const std::logic_error &) {
    invariant = true;
  }
  require(invariant, std::string(scheme) + " non-commit key on the local commit route stays an invariant failure");
}
} // namespace

int main() {
  const auto root = std::filesystem::temp_directory_path() /
                    ("msime-local-commit-rejection-" +
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
    for (const auto *scheme : {"wubi", "quanpin"})
      run(root, scheme);
  } catch (const std::exception &error) {
    std::cerr << error.what() << "\n";
    return 1;
  }
  std::cout << "Windows local commit rejection: Wubi and Quanpin passed\n";
  return 0;
}

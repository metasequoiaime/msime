#define MSIME_WEBSOCKET_TESTING
#include "../../compat/Windows7WebSocket.h"
#include <future>
#include <iostream>
#include <stdexcept>

using msime::windows::compat::Windows7WebSocket;
namespace {
void check(bool value, const char* reason) {
  if (!value) throw std::runtime_error(reason);
}
}
int main(int argc, char** argv) {
  try {
    check(Windows7WebSocket::available(), "curl was built without wss support");
    if (argc == 1) { std::cout << "WebSocket transport capability available\n"; return 0; }
    check(argc == 3, "expected synthetic TLS server URL and CA file");
    const std::string url = argv[1];
    const char* ca = argv[2];
    const std::string headers = "X-MSIME-Synthetic: allowed\r\n";
    {
      Windows7WebSocket untrusted;
      check(!untrusted.connect(url + "/echo", headers), "untrusted TLS certificate accepted");
    }
    {
      Windows7WebSocket redirect;
      check(!redirect.connect(url + "/redirect", headers, ca), "WebSocket redirect accepted");
    }
    {
      Windows7WebSocket socket;
      if (!socket.connect(url + "/echo", headers, ca)) throw std::runtime_error(std::string("synthetic TLS handshake failed: ") + socket.test_error());
      std::vector<std::uint8_t> packet(256 * 1024);
      for (std::size_t i = 0; i < packet.size(); ++i) packet[i] = static_cast<std::uint8_t>(i % 251);
      check(socket.send(packet), "binary send failed");
      std::vector<std::uint8_t> result;
      check(socket.receive(result) && result == packet, "fragmented binary response/ping mismatch");
      auto receiver = std::async(std::launch::async, [&] { std::vector<std::uint8_t> reply; return socket.receive(reply) && reply == packet; });
      check(socket.send(packet), "concurrent binary send failed");
      check(receiver.get(), "concurrent binary receive failed");
      auto waiting = std::async(std::launch::async, [&] { std::vector<std::uint8_t> reply; return socket.receive(reply); });
      std::this_thread::sleep_for(std::chrono::milliseconds(50));
      socket.close();
      check(waiting.wait_for(std::chrono::seconds(2)) == std::future_status::ready && !waiting.get(), "close did not cancel pending receive");
      socket.close();
      check(!socket.send(packet), "send accepted after close");
    }
    for (const auto* path : {"/oversize", "/text", "/close"}) {
      Windows7WebSocket socket;
      check(socket.connect(url + path, headers, ca), "bounded-response handshake failed");
      std::vector<std::uint8_t> result;
      check(!socket.receive(result), "invalid, oversized or closed response accepted");
      socket.close();
    }
    std::cout << "TLS trust, redirect refusal, binary fragments/ping, concurrent send/receive, cancellation and response bounds passed\n";
    return 0;
  } catch (const std::exception& error) {
    std::cerr << error.what() << '\n'; return 1;
  }
}

#include "FocusGate.h"
#include "FocusRouter.h"
#include "PipeHandshake.h"
#include "PipeIntake.h"
#include "PipeIo.h"
#include "PipeListener.h"
#include "PipeMainTransport.h"
#include "PipePeer.h"
#include "PipeRegistry.h"
#include "PipeService.h"
#include "ReplyCodec.h"
#include "windows_ipc.h"
#include <aclapi.h>
#include <atomic>
#include <chrono>
#include <cstring>
#include <future>
#include <iostream>
#include <sddl.h>
#include <stdexcept>
#include <string>
#include <system_error>

using namespace msime::windows;
namespace {
void require(bool value) {
  if (!value)
    throw std::runtime_error("Windows pipe test failed");
}
struct Handle {
  Handle() = default;
  Handle(const Handle &) = delete;
  Handle &operator=(const Handle &) = delete;
  HANDLE value = INVALID_HANDLE_VALUE;
  ~Handle() { close(); }
  void close() {
    if (value && value != INVALID_HANDLE_VALUE)
      CloseHandle(value);
    value = INVALID_HANDLE_VALUE;
  }
};
struct RegistryShutdown {
  PipeRegistry &registry;
  ~RegistryShutdown() { registry.shutdown(); }
};
struct SignalOnExit {
  HANDLE event;
  ~SignalOnExit() { SetEvent(event); }
};
struct Pair {
  std::unique_ptr<PipeListener> listener;
  struct Server {
    std::unique_ptr<PipeConnection> owner;
    HANDLE value = INVALID_HANDLE_VALUE; // Borrowed from owner for test calls.
  } server;
  Handle client;
  Pair() {
    static unsigned serial = 0;
    // Exercise the production listener with a unique test-only name.
    auto name = L"\\\\.\\pipe\\MSIMEClientIoTest-" +
                std::to_wstring(GetCurrentProcessId()) + L"-" +
                std::to_wstring(++serial);
    DWORD error = ERROR_SUCCESS;
    listener = PipeListener::create(name, error);
    require(listener && error == ERROR_SUCCESS);
    require(!PipeListener::create(name, error)); // Never join an owned name.
    client.value =
        CreateFileW(name.c_str(), GENERIC_READ | GENERIC_WRITE, 0, nullptr,
                    OPEN_EXISTING, FILE_FLAG_OVERLAPPED, nullptr);
    require(client.value != INVALID_HANDLE_VALUE);
    DWORD mode = PIPE_READMODE_MESSAGE;
    require(SetNamedPipeHandleState(client.value, &mode, nullptr, nullptr));
    auto accepted = listener->accept(2000);
    require(accepted.io.complete() && accepted.connection);
    server.owner = std::move(accepted.connection);
    server.value = server.owner->handle();
  }
};
template <typename Packet>
std::vector<uint8_t> fixture_bytes(const Packet &packet) {
  // Test-only synthetic packets; production serializers never expose padding.
  std::vector<uint8_t> bytes(sizeof(Packet));
  std::memcpy(bytes.data(), &packet, sizeof(Packet));
  return bytes;
}
void listeners() {
  DWORD error = ERROR_SUCCESS;
  require(!PipeListener::create(L"\\\\remote\\pipe\\test", error));
  require(error == ERROR_INVALID_NAME);
  const auto name = L"\\\\.\\pipe\\MSIMEClientListenerTest-" +
                    std::to_wstring(GetCurrentProcessId());
  auto listener = PipeListener::create(name, error);
  require(listener && error == ERROR_SUCCESS);
  auto timed = listener->accept(20);
  require(timed.io.status == IoStatus::Timeout && !timed.connection);
  require(!PipeListener::create(name, error));
  Handle cancel;
  cancel.value = CreateEventW(nullptr, TRUE, TRUE, nullptr);
  require(cancel.value != nullptr);
  auto cancelled = listener->accept(2000, cancel.value);
  require(cancelled.io.status == IoStatus::Cancelled && !cancelled.connection);
  require(ResetEvent(cancel.value));
  auto canceller = std::async(std::launch::async, [&] {
    Sleep(30);
    require(SetEvent(cancel.value));
  });
  cancelled = listener->accept(2000, cancel.value);
  canceller.get();
  require(cancelled.io.status == IoStatus::Cancelled && !cancelled.connection);
  auto accepting =
      std::async(std::launch::async, [&] { return listener->accept(2000); });
  require(accepting.wait_for(std::chrono::milliseconds(30)) ==
          std::future_status::timeout);
  Handle client;
  client.value =
      CreateFileW(name.c_str(), GENERIC_READ | GENERIC_WRITE, 0, nullptr,
                  OPEN_EXISTING, FILE_FLAG_OVERLAPPED, nullptr);
  require(client.value != INVALID_HANDLE_VALUE);
  auto accepted = accepting.get();
  require(accepted.io.complete() && accepted.connection);
  DWORD flags = 0;
  require(GetHandleInformation(accepted.connection->handle(), &flags));
  require(!(flags & HANDLE_FLAG_INHERIT));
  PACL dacl = nullptr;
  PSECURITY_DESCRIPTOR security = nullptr;
  require(GetSecurityInfo(accepted.connection->handle(), SE_KERNEL_OBJECT,
                          DACL_SECURITY_INFORMATION, nullptr, nullptr, &dacl,
                          nullptr, &security) == ERROR_SUCCESS);
  // Inspect actual kernel ACL, not just the descriptor string.
  bool acl_ok = dacl && dacl->AceCount == 3;
  bool connect_only_appcontainer = false;
  BYTE app_sid[SECURITY_MAX_SID_SIZE];
  DWORD sid_size = sizeof(app_sid);
  acl_ok = acl_ok && CreateWellKnownSid(WinBuiltinAnyPackageSid, nullptr,
                                        app_sid, &sid_size);
  if (acl_ok) {
    for (DWORD i = 0; i < dacl->AceCount; ++i) {
      void *raw = nullptr;
      if (!GetAce(dacl, i, &raw)) {
        acl_ok = false;
        break;
      }
      const auto ace = static_cast<ACCESS_ALLOWED_ACE *>(raw);
      if (ace->Header.AceType != ACCESS_ALLOWED_ACE_TYPE) {
        acl_ok = false;
        break;
      }
      if (IsWellKnownSid(const_cast<DWORD *>(&ace->SidStart), WinWorldSid)) {
        acl_ok = false;
        break;
      }
      if (EqualSid(const_cast<DWORD *>(&ace->SidStart), app_sid))
        connect_only_appcontainer = ace->Mask == 0x12019bu;
    }
  }
  LocalFree(security);
  require(acl_ok && connect_only_appcontainer);
  require(!PipeListener::create(name, error));
  listener.reset();
  require(
      !PipeListener::create(name, error)); // Live connection still owns name.
  accepted.connection.reset();
  client.close();
  listener = PipeListener::create(name, error);
  require(listener && error == ERROR_SUCCESS);
}
void handshakes() {
  const auto id = (static_cast<uint64_t>(GetCurrentProcessId()) << 32) | 11u;
  for (auto role :
       {FanyImePipeRole::ToTsf, FanyImePipeRole::ToTsfWorkerThread}) {
    Pair reverse;
    auto accept = std::async(std::launch::async, [&] {
      return accept_reverse(reverse.server.value, role, 2000);
    });
    FanyImePipeHello hello{};
    hello.client_id = id;
    hello.pipe_role = role;
    require(write_frame(reverse.client.value, fixture_bytes(hello), 2000)
                .complete());
    const DWORD size = role == FanyImePipeRole::ToTsf
                           ? sizeof(FanyImeNamedpipeDataToTsf)
                           : sizeof(FanyImeNamedpipeDataToTsfWorkerThread);
    auto ready = read_frame(reverse.client.value, size, 2000);
    auto registered = accept.get();
    require(registered.status == HandshakeStatus::Ready && registered.peer &&
            registered.client_id == id && ready.complete());
    require(ready.frame[0] == 9);
    for (size_t i = 1; i < ready.frame.size(); ++i)
      require(ready.frame[i] == 0);
    if (role != FanyImePipeRole::ToTsf)
      continue;
    require(accept_main(reverse.server.value, reverse.server.value,
                        *registered.peer, id,
                        FanyImeProtocol::RequiredCapabilities, 2000)
                .status == HandshakeStatus::InvalidArgument);
    for (int mode : {0, 1, 2, 3}) {
      Pair main;
      auto packet = FanyImeProtocol::Hello(id, 71);
      if (mode == 1) // Required but not implemented optional capability.
        packet.point[1] |= FanyImeProtocol::FramedVoice;
      if (mode == 2) { // A hello without version or request id is rejected.
        packet = {};
        packet.event_type = FanyImePipeEventType::ClientHello;
        packet.client_id = id;
      }
      if (mode == 3)
        packet.client_id = id + 1;
      auto negotiate = std::async(std::launch::async, [&] {
        return accept_main(main.server.value, reverse.server.value,
                           *registered.peer, id,
                           FanyImeProtocol::RequiredCapabilities, 2000);
      });
      require(write_frame(main.client.value, fixture_bytes(packet), 2000)
                  .complete());
      if (mode < 2) {
        auto ack = read_frame(reverse.client.value,
                              sizeof(FanyImeNamedpipeDataToTsf), 2000);
        require(ack.complete());
        FanyImeNamedpipeDataToTsf response{};
        std::memcpy(&response, ack.frame.data(), sizeof(response));
        require(response.request_id == 71);
        require((mode == 0 && FanyImeProtocol::AcceptReply(response, 71)) ||
                (mode == 1 &&
                 response.msg_type == FanyImeReplyType::ProtocolMismatch));
        require(!(FanyImeProtocol::ReplyCapabilities(response) &
                  FanyImeProtocol::FramedVoice));
      }
      auto result = negotiate.get();
      require(result.status == (mode == 0 ? HandshakeStatus::Ready
                                          : HandshakeStatus::ProtocolRejected));
      if (result.status == HandshakeStatus::Ready)
        require(result.io.complete());
      if (mode >= 2) {
        DWORD available = 0;
        require(PeekNamedPipe(reverse.client.value, nullptr, 0, nullptr,
                              &available, nullptr));
        require(available == 0); // No ACK for an unversioned or spoofed-client hello.
      }
    }
  }
  {
    Pair pipe;
    FanyImePipeHello hello{};
    hello.client_id = id;
    hello.pipe_role = FanyImePipeRole::ToTsfWorkerThread;
    auto accept = std::async(std::launch::async, [&] {
      return accept_reverse(pipe.server.value, FanyImePipeRole::ToTsf, 2000);
    });
    require(
        write_frame(pipe.client.value, fixture_bytes(hello), 2000).complete());
    auto result = accept.get();
    require(result.status == HandshakeStatus::ProtocolRejected && !result.peer);
    require(
        accept_reverse(pipe.server.value, FanyImePipeRole::Main, 2000).status ==
        HandshakeStatus::InvalidArgument);
  }
  {
    Pair pipe;
    Handle cancel;
    cancel.value = CreateEventW(nullptr, TRUE, TRUE, nullptr);
    require(cancel.value != nullptr);
    auto result = accept_reverse(pipe.server.value, FanyImePipeRole::ToTsf,
                                 2000, cancel.value);
    require(result.status == HandshakeStatus::TransportError &&
            result.io.status == IoStatus::Cancelled && !result.peer);
  }
}
void registries(int malformed = 0) {
  PipeRegistry registry(1);
  const auto id = (static_cast<uint64_t>(GetCurrentProcessId()) << 32) | 33u;
  const auto register_reverse = [&](Pair &pipe, uint32_t role,
                                    uint64_t client) {
    auto task = std::async(std::launch::async, [&] {
      return registry.register_reverse(std::move(pipe.server.owner), role,
                                       2000);
    });
    FanyImePipeHello hello{};
    hello.client_id = client;
    hello.pipe_role = role;
    require(
        write_frame(pipe.client.value, fixture_bytes(hello), 2000).complete());
    auto ack =
        read_frame(pipe.client.value,
                   role == 1 ? sizeof(FanyImeNamedpipeDataToTsf)
                             : sizeof(FanyImeNamedpipeDataToTsfWorkerThread),
                   2000);
    auto result = task.get();
    require((result.status == RegistryStatus::Ready) == ack.complete());
    return result;
  };
  Pair reply, worker, main;
  auto reply_registration = register_reverse(reply, FanyImePipeRole::ToTsf, id);
  require(reply_registration.status == RegistryStatus::Ready);
  auto worker_registration =
      register_reverse(worker, FanyImePipeRole::ToTsfWorkerThread, id);
  require(worker_registration.status == RegistryStatus::Ready);
  const auto bytes = *wire_bytes(preedit_reply(91, "test"));
  const std::vector<uint8_t> frame(bytes.begin(), bytes.end());
  require(
      !registry.send(worker_registration.ticket, 1, frame, 2000).complete());
  auto handshake = std::async(std::launch::async, [&] {
    const auto input =
        read_frame(main.server.value, sizeof(FanyImeNamedpipeData), 2000);
    require(input.complete());
    FanyImeNamedpipeData hello{};
    std::memcpy(&hello, input.frame.data(), sizeof(hello));
    return registry.register_main(std::move(main.server.owner), hello,
                                  FanyImeProtocol::RequiredCapabilities, 2000);
  });
  const auto hello = FanyImeProtocol::Hello(id, 90);
  require(
      write_frame(main.client.value, fixture_bytes(hello), 2000).complete());
  auto ack =
      read_frame(reply.client.value, sizeof(FanyImeNamedpipeDataToTsf), 2000);
  require(ack.complete() && ack.frame[0] == FanyImeReplyType::ProtocolReady);
  auto registered = handshake.get();
  require(registered.status == RegistryStatus::Ready);
  const auto fence = *focus_ready_bytes(7, 8, 90);
  FocusGate focus;
  FocusRouter router(focus, 1);
  require(router.connected(registered.ticket).accepted);
  FanyImeNamedpipeData activated{};
  activated.client_id = id;
  activated.event_type = FanyImePipeEventType::ClientActivated;
  activated.request_id = 90;
  auto routed = router.dispatch(registered.ticket, activated);
  require(routed.activation.has_value());
  const auto activation = *routed.activation;
  auto fencing = std::async(std::launch::async, [&] {
    IoResult result;
    require(focus.acknowledge(activation.pending, [&] {
      result = registry.send(registered.ticket,
                             FanyImePipeRole::ToTsfWorkerThread, fence, 2000);
      return result.complete();
    }));
    return result;
  });
  auto focus_ack = read_frame(
      worker.client.value, sizeof(FanyImeNamedpipeDataToTsfWorkerThread), 2000);
  require(fencing.get().complete() && focus_ack.complete() &&
          focus_ack.frame == fence);
  require(router.confirmed(activation.pending));
  auto wrong_ticket = registered.ticket;
  ++wrong_ticket.generations[0];
  PipeMainTransport transport(registry, 2000);
  require(transport.current(registered.ticket) && !transport.current(wrong_ticket));
  const auto registered_tickets = transport.current_tickets();
  require(registered_tickets.size() == 1 &&
          same_ticket(registered_tickets.front(), registered.ticket));
  require(transport.try_current(registered.ticket) &&
          !transport.try_current(wrong_ticket) &&
          !transport.try_current(PipeTicket{}));
  transport.close(wrong_ticket);
  require(transport.current(registered.ticket));
  require(!registry.send(wrong_ticket, 1, frame, 2000).complete());
  auto sending = std::async(std::launch::async, [&] {
    KeyEventSendResult result = KeyEventSendResult::DefinitelyNotSent;
    require(focus.with_active(activation.pending, [&] {
      result = transport.send(registered.ticket, FanyImePipeRole::ToTsf, frame);
    }));
    return result == KeyEventSendResult::Sent;
  });
  auto response =
      read_frame(reply.client.value, sizeof(FanyImeNamedpipeDataToTsf), 2000);
  require(sending.get() && response.complete() &&
          response.frame == frame);
  auto reading = std::async(std::launch::async, [&] {
    return transport.read(registered.ticket);
  });
  RegistryShutdown reading_guard{registry};
  require(reading.wait_for(std::chrono::milliseconds(60)) ==
          std::future_status::timeout);
  FanyImeNamedpipeData key{};
  key.event_type = FanyImePipeEventType::KeyEvent;
  key.client_id = id;
  key.request_id = 91;
  key.wch = L'a';
  require(write_frame(main.client.value, fixture_bytes(key), 2000).complete());
  const auto received = reading.get();
  require(received && fixture_bytes(*received) == fixture_bytes(key));
  if (malformed) {
    if (malformed == 1)
      ++key.client_id;
    else if (malformed == 2)
      key.pinyin_length = 128;
    else
      key.event_type = FanyImePipeEventType::LangbarRightClick;
    auto invalid = std::async(std::launch::async, [&] {
      return registry.read_main(registered.ticket);
    });
    require(write_frame(main.client.value, fixture_bytes(key), 2000).complete());
    const auto rejected = invalid.get();
    require(rejected.status == IoStatus::MalformedFrame &&
            rejected.frame.empty());
    require(!registry.read_main(registered.ticket, 10).complete());
    require(!registry.send(registered.ticket, 1, frame, 10).complete());
    return;
  }
  auto pending = std::async(std::launch::async, [&] {
    return registry.read_main(registered.ticket);
  });
  RegistryShutdown pending_guard{registry};
  require(pending.wait_for(std::chrono::milliseconds(30)) ==
          std::future_status::timeout);
  Pair replacement;
  auto replaced = register_reverse(replacement, FanyImePipeRole::ToTsf, id);
  require(replaced.status == RegistryStatus::Ready);
  require(pending.get().status == IoStatus::Cancelled);
  require(focus.invalidate(registered.ticket));
  require(!focus.with_active(activation.pending, [] {}));
  require(!registry.send(registered.ticket, 1, frame, 2000).complete());
  require(!registry.remove(registered.ticket, 1));
  require(!registry.remove(registered.ticket, 0));
  Pair excess;
  require(register_reverse(excess, 1, id + 1).status ==
          RegistryStatus::Capacity);
  require(registry.remove(replaced.ticket, 1));
  require(registry.remove(worker_registration.ticket, 2));
  Pair reclaimed;
  require(register_reverse(reclaimed, 1, id + 1).status ==
          RegistryStatus::Ready);
  require(transport.current_tickets().empty());
  registry.shutdown();
  require(transport.current_tickets().empty());
  require(!registry.send(registered.ticket, 1, frame, 2000).complete());
}
// A TSF thread or process that goes away closes its pipes but never calls
// remove(). Its slot must come back without anyone doing so, while a live
// thread that only lost Main keeps its reverse pipes for the reconnect.
void dead_clients() {
  PipeRegistry registry(1);
  RegistryShutdown guard{registry};
  const auto id = (static_cast<uint64_t>(GetCurrentProcessId()) << 32) | 66u;
  const auto reverse = [&](Pair &pipe, uint32_t role, uint64_t client) {
    auto task = std::async(std::launch::async, [&] {
      return registry.register_reverse(std::move(pipe.server.owner), role,
                                       2000);
    });
    FanyImePipeHello hello{};
    hello.client_id = client;
    hello.pipe_role = role;
    require(
        write_frame(pipe.client.value, fixture_bytes(hello), 2000).complete());
    (void)read_frame(pipe.client.value,
                     role == 1
                         ? sizeof(FanyImeNamedpipeDataToTsf)
                         : sizeof(FanyImeNamedpipeDataToTsfWorkerThread),
                     2000);
    return task.get().status;
  };
  const auto establish = [&](Pair &pipe, Pair &reply, uint32_t request) {
    auto handshake = std::async(std::launch::async, [&] {
      const auto input =
          read_frame(pipe.server.value, sizeof(FanyImeNamedpipeData), 2000);
      require(input.complete());
      FanyImeNamedpipeData hello{};
      std::memcpy(&hello, input.frame.data(), sizeof(hello));
      return registry.register_main(std::move(pipe.server.owner), hello,
                                    FanyImeProtocol::RequiredCapabilities,
                                    2000);
    });
    require(write_frame(pipe.client.value,
                        fixture_bytes(FanyImeProtocol::Hello(id, request)),
                        2000)
                .complete());
    require(read_frame(reply.client.value, sizeof(FanyImeNamedpipeDataToTsf),
                       2000)
                .complete());
    return handshake.get();
  };
  Pair reply, worker, first;
  require(reverse(reply, 1, id) == RegistryStatus::Ready);
  require(reverse(worker, 2, id) == RegistryStatus::Ready);
  const auto registered = establish(first, reply, 93);
  require(registered.status == RegistryStatus::Ready);
  // Only Main closes: the thread is alive, so the client keeps its slot and
  // Main comes back over the existing reverse pipes.
  first.client.close();
  require(registry.read_main(registered.ticket).status ==
          IoStatus::Disconnected);
  Pair excess;
  require(reverse(excess, 1, id + 1) == RegistryStatus::Capacity);
  Pair second;
  const auto again = establish(second, reply, 94);
  require(again.status == RegistryStatus::Ready);
  // Now the whole chain closes, as when the host process exits.
  second.client.close();
  reply.client.close();
  worker.client.close();
  require(registry.read_main(again.ticket).status == IoStatus::Disconnected);
  Pair reclaimed;
  require(reverse(reclaimed, 1, id + 1) == RegistryStatus::Ready);
  // A client that registered its reverse pipes and died before Main is
  // reclaimed by the capacity sweep alone.
  reclaimed.client.close();
  Pair late;
  require(reverse(late, 1, id + 2) == RegistryStatus::Ready);
}
template <typename Predicate> void eventually(Predicate predicate) {
  const auto until = GetTickCount64() + 2000;
  while (!predicate()) {
    require(GetTickCount64() < until);
    Sleep(1);
  }
}
void intake_pools() {
  const auto id = (static_cast<uint64_t>(GetCurrentProcessId()) << 32) | 44u;
  {
    PipeRegistry registry(2);
    std::atomic<unsigned> callbacks{0};
    PipeIntake intake(registry, 1, 1, FanyImeProtocol::RequiredCapabilities,
                      2000, [&](uint32_t, const PipeRegistration &) {
                        ++callbacks;
                        return true;
                      });
    Pair silent, queued, excess;
    require(intake.submit(std::move(silent.server.owner), 1));
    eventually([&] { return intake.stats().active == 1; });
    require(intake.submit(std::move(queued.server.owner), 1));
    require(!intake.submit(std::move(excess.server.owner), 1));
    require(intake.stats().queued == 1 && intake.stats().rejected == 1);
    intake.stop();
    require(intake.stats().active == 0 && intake.stats().queued == 0 &&
            callbacks == 0);
    Pair after_stop;
    require(!intake.submit(std::move(after_stop.server.owner), 1));
    registry.shutdown();
  }
  {
    PipeRegistry registry(1);
    std::array<std::promise<PipeRegistration>, 3> completions;
    PipeIntake intake(registry, 2, 3, FanyImeProtocol::RequiredCapabilities,
                      2000, [&](uint32_t role, const PipeRegistration &result) {
                        completions[role].set_value(result);
                        return true;
                      });
    Pair reply, worker, main;
    const auto reverse = [&](Pair &pipe, uint32_t role) {
      auto completion = completions[role].get_future();
      require(intake.submit(std::move(pipe.server.owner), role));
      FanyImePipeHello hello{};
      hello.client_id = id;
      hello.pipe_role = role;
      require(write_frame(pipe.client.value, fixture_bytes(hello), 2000)
                  .complete());
      require(read_frame(pipe.client.value,
                         role == 1
                             ? sizeof(FanyImeNamedpipeDataToTsf)
                             : sizeof(FanyImeNamedpipeDataToTsfWorkerThread),
                         2000)
                  .complete());
      require(completion.wait_for(std::chrono::seconds(2)) ==
              std::future_status::ready);
      require(completion.get().status == RegistryStatus::Ready);
    };
    reverse(reply, 1);
    reverse(worker, 2);
    auto completion = completions[0].get_future();
    require(intake.submit(std::move(main.server.owner), 0));
    require(write_frame(main.client.value,
                        fixture_bytes(FanyImeProtocol::Hello(id, 92)), 2000)
                .complete());
    require(
        read_frame(reply.client.value, sizeof(FanyImeNamedpipeDataToTsf), 2000)
            .complete());
    require(completion.wait_for(std::chrono::seconds(2)) ==
            std::future_status::ready);
    require(completion.get().status == RegistryStatus::Ready);
    intake.stop();
    require(intake.stats().failed == 0);
    registry.shutdown();
  }
  for (bool throwing : {false, true}) {
    PipeRegistry registry(1);
    PipeIntake intake(registry, 1, 1, FanyImeProtocol::RequiredCapabilities,
                      2000,
                      [throwing](uint32_t, const PipeRegistration &) -> bool {
                        if (throwing)
                          throw std::runtime_error("Synthetic rejection");
                        return false;
                      });
    Pair pipe;
    require(intake.submit(std::move(pipe.server.owner), 1));
    FanyImePipeHello hello{};
    hello.client_id = id;
    hello.pipe_role = 1;
    require(
        write_frame(pipe.client.value, fixture_bytes(hello), 2000).complete());
    require(
        read_frame(pipe.client.value, sizeof(FanyImeNamedpipeDataToTsf), 2000)
            .complete());
    eventually([&] { return intake.stats().failed == 1; });
    // The callback did not take responsibility for the registration.
    const auto closed =
        read_frame(pipe.client.value, sizeof(FanyImeNamedpipeDataToTsf), 100);
    require(!closed.complete() && closed.status != IoStatus::Timeout);
    intake.stop();
    registry.shutdown();
  }
}
void services() {
  PipeServiceOptions options;
  options.capabilities = FanyImeProtocol::RequiredCapabilities;
  options.handshake_timeout = 2000;
  for (size_t role = 0; role < 3; ++role)
    options.names[role] = L"\\\\.\\pipe\\MSIMEClientServiceTest-" +
                          std::to_wstring(GetCurrentProcessId()) + L"-" +
                          std::to_wstring(role);
  const auto id = (static_cast<uint64_t>(GetCurrentProcessId()) << 32) | 55u;
  for (int round = 0; round < 2; ++round) {
    std::promise<PipeRegistration> main_ready;
    auto ready = main_ready.get_future();
    PipeService service(options,
                        [&](uint32_t role, const PipeRegistration &result) {
                          if (role == FanyImePipeRole::Main)
                            main_ready.set_value(result);
                          return true;
                        });
    std::array<Handle, 3> clients;
    const auto open = [&](size_t role) {
      clients[role].value =
          CreateFileW(options.names[role].c_str(), GENERIC_READ | GENERIC_WRITE,
                      0, nullptr, OPEN_EXISTING, FILE_FLAG_OVERLAPPED, nullptr);
      require(clients[role].value != INVALID_HANDLE_VALUE);
      DWORD mode = PIPE_READMODE_MESSAGE;
      require(SetNamedPipeHandleState(clients[role].value, &mode, nullptr,
                                      nullptr));
    };
    for (uint32_t role : {1u, 2u}) {
      open(role);
      FanyImePipeHello hello{};
      hello.client_id = id;
      hello.pipe_role = role;
      require(write_frame(clients[role].value, fixture_bytes(hello), 2000)
                  .complete());
      require(read_frame(clients[role].value,
                         role == 1
                             ? sizeof(FanyImeNamedpipeDataToTsf)
                             : sizeof(FanyImeNamedpipeDataToTsfWorkerThread),
                         2000)
                  .complete());
    }
    open(0);
    require(write_frame(clients[0].value,
                        fixture_bytes(FanyImeProtocol::Hello(id, 93)), 2000)
                .complete());
    auto ack =
        read_frame(clients[1].value, sizeof(FanyImeNamedpipeDataToTsf), 2000);
    require(ack.complete() && ack.frame[0] == FanyImeReplyType::ProtocolReady);
    require(ready.wait_for(std::chrono::seconds(2)) ==
            std::future_status::ready);
    require(ready.get().status == RegistryStatus::Ready);
    service.stop();
    service.stop(); // Control-thread shutdown is idempotent.
    require(service.failure() == ERROR_SUCCESS);
    for (auto &client : clients)
      client.close();
  }
  {
    DWORD error = ERROR_SUCCESS;
    auto occupied = PipeListener::create(options.names[2], error);
    require(occupied != nullptr);
    bool rejected = false;
    try {
      PipeService service(
          options, [](uint32_t, const PipeRegistration &) { return true; });
    } catch (const std::system_error &) {
      rejected = true;
    }
    require(rejected);
    // Failed startup released the other names and did not take over the old
    // one.
    auto recovered = PipeListener::create(options.names[0], error);
    require(recovered != nullptr);
    require(!PipeListener::create(options.names[2], error));
  }
}
} // namespace
int main() {
  try {
    {
      Pair pipe;
      require(read_frame_until_cancel(pipe.server.value, 4, nullptr).status ==
              IoStatus::InvalidArgument);
      Handle cancel;
      cancel.value = CreateEventW(nullptr, TRUE, FALSE, nullptr);
      require(cancel.value != nullptr);
      require(read_frame(pipe.server.value, 4, INFINITE, cancel.value).status ==
              IoStatus::InvalidArgument);
      require(
          write_frame(pipe.server.value, {1, 2, 3, 4}, INFINITE, cancel.value)
              .status == IoStatus::InvalidArgument);
      auto reader = std::async(std::launch::async, [&] {
        return read_frame_until_cancel(pipe.server.value, 4, cancel.value);
      });
      SignalOnExit cancel_guard{cancel.value};
      require(reader.wait_for(std::chrono::milliseconds(60)) ==
              std::future_status::timeout);
      require(SetEvent(cancel.value));
      require(reader.wait_for(std::chrono::seconds(2)) ==
              std::future_status::ready);
      require(reader.get().status == IoStatus::Cancelled);
    }
    services();
    intake_pools();
    registries();
    for (int malformed = 1; malformed <= 3; ++malformed)
      registries(malformed);
    dead_clients();
    listeners();
    handshakes();
    {
      Pair main, reverse;
      const auto id = (static_cast<uint64_t>(GetCurrentProcessId()) << 32) | 7u;
      DWORD error = ERROR_SUCCESS;
      auto peer = PipePeer::bind(main.server.value, id, error);
      require(peer && error == ERROR_SUCCESS);
      require(peer->matches(main.server.value, id, error));
      require(peer->matches(reverse.server.value, id, error));
      require(!peer->matches(reverse.server.value, id + 1, error));
      require(error == ERROR_ACCESS_DENIED);
      require(
          !PipePeer::bind(main.server.value, id ^ (uint64_t{1} << 32), error));
      require(error == ERROR_ACCESS_DENIED);
      require(!PipePeer::bind(main.server.value, 0, error));
      require(!PipePeer::bind(main.client.value, id, error));
      require(!PipePeer::bind(INVALID_HANDLE_VALUE, id, error));
      require(DisconnectNamedPipe(main.server.value));
      require(!peer->matches(main.server.value, id, error));
    }
    for (auto size : {sizeof(FanyImePipeHello), sizeof(FanyImeNamedpipeData),
                      sizeof(FanyImeNamedpipeDataToTsfWorkerThread),
                      sizeof(FanyImeNamedpipeDataToTsf)}) {
      Pair pipe;
      std::vector<uint8_t> payload(size, 0x5A);
      auto writer = std::async(std::launch::async, [&] {
        return write_frame(pipe.client.value, payload, 2000);
      });
      auto received =
          read_frame(pipe.server.value, static_cast<DWORD>(size), 2000);
      require(writer.get().complete() && received.complete() &&
              received.frame == payload);
    }
    for (DWORD size : {3u, 5u}) {
      Pair pipe;
      auto writer = std::async(std::launch::async, [&] {
        return write_frame(pipe.client.value, std::vector<uint8_t>(size, 0x51),
                           2000);
      });
      auto received = read_frame(pipe.server.value, 4, 2000);
      require(writer.get().complete());
      require(received.status == IoStatus::MalformedFrame &&
              received.frame.empty());
    }
    {
      Pair pipe;
      auto result = read_frame(pipe.server.value, 4, 20);
      require(result.status == IoStatus::Timeout && result.frame.empty() &&
              !result.delivery_uncertain);
    }
    {
      Pair pipe;
      Handle cancel;
      cancel.value = CreateEventW(nullptr, TRUE, FALSE, nullptr);
      require(cancel.value != nullptr);
      auto canceller = std::async(std::launch::async, [&] {
        Sleep(30);
        require(SetEvent(cancel.value));
      });
      auto result = read_frame(pipe.server.value, 4, 2000, cancel.value);
      canceller.get();
      require(result.status == IoStatus::Cancelled && result.frame.empty());
      auto skipped = write_frame(pipe.server.value, {1, 2}, 2000, cancel.value);
      require(skipped.status == IoStatus::Cancelled &&
              !skipped.delivery_uncertain && skipped.transferred == 0);
    }
    {
      Pair pipe;
      pipe.client.close();
      require(read_frame(pipe.server.value, 4, 1000).status ==
              IoStatus::Disconnected);
    }
    require(read_frame(INVALID_HANDLE_VALUE, 4, 100).status ==
            IoStatus::InvalidArgument);
    std::cout << "Windows named-pipe peer binding, framing, timeout, "
                 "cancellation and "
                 "disconnect tests passed\n";
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}

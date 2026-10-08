#pragma once
#include "PipeHandshake.h"
#include "PipeListener.h"
#include "PipeTicket.h"
#include <array>
#include <mutex>
#include <unordered_map>
#include <vector>

namespace msime::windows {
// IdentityRejected 单独列出：管道对端的进程、会话或用户对不上。反作弊剥掉 Server 打开游戏进程所需的权限时就会落在这里，Server 据此写诊断日志。
enum class RegistryStatus {
  Ready,
  Rejected,
  Capacity,
  Stale,
  TransportError,
  IdentityRejected
};
struct PipeRegistration {
  RegistryStatus status = RegistryStatus::Rejected;
  PipeTicket ticket;
  IoResult io;
};
// Thread-safe endpoint ownership. Native calls run only on I/O workers.
// Bounds registered clients; the caller must separately bound accepted sockets
// and handshake workers. Same-client writes/registration are serialized, while
// independent clients do not hold a global lock across I/O.
class PipeRegistry final {
public:
  explicit PipeRegistry(size_t max_clients) : max_clients_(max_clients) {}
  PipeRegistration register_reverse(std::unique_ptr<PipeConnection> connection,
                                    uint32_t role, DWORD timeout,
                                    HANDLE cancel = nullptr);
  // Called after extracting the client ID from the main hello. The supplied
  // hello is still untrusted; negotiation and peer validation happen here.
  PipeRegistration register_main(std::unique_ptr<PipeConnection> connection,
                                 const FanyImeNamedpipeData &hello,
                                 uint32_t capabilities, DWORD timeout,
                                 HANDLE cancel = nullptr);
  // Established streams wait through user inactivity by default. Endpoint
  // replacement/removal or shutdown cancels the wait. Finite timeouts are for
  // diagnostics; an expired submitted read still invalidates that connection.
  IoResult read_main(const PipeTicket &ticket, DWORD timeout = INFINITE);
  // Registration snapshot only, not focus authorization or process liveness.
  bool is_current(const PipeTicket &ticket);
  // Display-only snapshot; false includes contention, not just invalidation.
  bool try_is_current(const PipeTicket &ticket);
  // Snapshot all clients with a complete Main/ToTsf/Worker transport chain.
  // The returned tickets are registration identities, not focus leases.
  std::vector<PipeTicket> current_tickets();
  // Returned input is still untrusted packet data, and the caller must carry
  // its ticket into the input queue and validate focus/activation there.
  // Transport-only send: caller must ALSO verify input focus/activation before
  // using this for candidate or status output. No automatic retry on failure.
  IoResult send(const PipeTicket &ticket, uint32_t role,
                const std::vector<uint8_t> &frame, DWORD timeout);
  bool remove(const PipeTicket &ticket, uint32_t role);
  void
  shutdown(); // Stop callers and join their I/O workers before destruction.

private:
  struct Endpoint;
  struct Client;
  std::shared_ptr<Client> lookup(uint64_t client, bool create);
  void retire(uint64_t id, const std::shared_ptr<Client> &client);
  // Retire clients whose process or pipes are gone; true if a slot may now
  // be free.
  bool reclaim_dead();
  static PipeTicket ticket(uint64_t id, const Client &client);
  static bool current(const PipeTicket &ticket, const Client &client);
  size_t max_clients_;
  std::mutex mutex_;
  bool stopped_ = false;
  uint64_t next_generation_ = 0;
  std::unordered_map<uint64_t, std::shared_ptr<Client>> clients_;
  uint64_t next_generation();
};
} // namespace msime::windows

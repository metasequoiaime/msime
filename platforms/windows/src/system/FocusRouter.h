#pragma once
#include "FocusGate.h"
#include "MainFrame.h"
#include <stdexcept>
#include <thread>
#include <unordered_map>

namespace msime::windows {
struct FocusRoute {
  bool accepted = false;
  std::optional<FocusChange> activation;
  std::optional<FocusLease> cleanup;
  std::optional<FocusLease> route;
  bool terminal = false;
  bool fence = false;
};

// Serialized control/input-queue policy; pipe workers never call these methods.
// Register only successfully negotiated, current Registry tickets. Process each
// Main stream in order. Returned work still requires gate + Registry
// validation.
class FocusRouter final {
public:
  explicit FocusRouter(FocusGate &gate, size_t capacity)
      : gate_(gate), capacity_(capacity) {
    if (!capacity || capacity > 1024)
      throw std::invalid_argument("Invalid focus routing capacity");
  }
  FocusRouter(const FocusRouter &) = delete;
  FocusRouter &operator=(const FocusRouter &) = delete;
  FocusRoute connected(const PipeTicket &ticket) {
    check_thread();
    if (!ticket.client)
      return {};
    for (auto generation : ticket.generations)
      if (!generation)
        return {};
    auto found = clients_.find(ticket.client);
    if (found == clients_.end()) {
      if (clients_.size() == capacity_)
        return {};
      clients_.emplace(ticket.client, Client{ticket, 0, std::nullopt});
      FocusRoute result;
      result.accepted = true;
      return result;
    }
    auto &client = found->second;
    if (same_ticket(ticket, client.ticket)) {
      FocusRoute result;
      result.accepted = true;
      return result;
    }
    // Registry generations are monotonic; a delayed registration completion
    // must not reinstall an older chain, including a replaced reverse pipe.
    for (size_t i = 0; i < 3; ++i)
      if (ticket.generations[i] < client.ticket.generations[i])
        return {};
    auto result = detach(client);
    client = Client{ticket, 0, std::nullopt};
    return result;
  }
  FocusRoute disconnected(const PipeTicket &ticket) {
    check_thread();
    auto found = clients_.find(ticket.client);
    if (found == clients_.end() || !same_ticket(ticket, found->second.ticket))
      return {};
    auto result = detach(found->second);
    clients_.erase(found);
    return result;
  }
  // Enqueue after the I/O worker's successful gate.acknowledge. This records
  // the token eligible for implicit recovery; failed fences never qualify.
  bool confirmed(const FocusLease &lease) {
    check_thread();
    auto found = clients_.find(lease.transport.client);
    if (!current_ || !same_lease(*current_, lease) || found == clients_.end())
      return false;
    return gate_.with_active(lease, [&] { found->second.token = lease.token; });
  }
  // Call on preparation/fence/delivery failure before accepting more work.
  // No uncertain write is replayed; a new explicit activation is required.
  FocusRoute failed(const FocusLease &lease) {
    check_thread();
    auto found = clients_.find(lease.transport.client);
    if (found == clients_.end() || !found->second.last ||
        !same_lease(*found->second.last, lease))
      return {};
    return detach(found->second);
  }
  FocusRoute dispatch(const PipeTicket &ticket,
                      const FanyImeNamedpipeData &packet) {
    check_thread();
    auto found = clients_.find(ticket.client);
    if (found == clients_.end() || !same_ticket(ticket, found->second.ticket) ||
        !valid_main_frame(packet, ticket.client))
      return {};
    auto &client = found->second;
    using namespace FanyImePipeEventType;
    FocusRoute result;
    if (packet.event_type == ClientHello) {
      result.accepted = true;
      return result;
    }
    // 字段属性属于这个客户端，不属于哪一次焦点：不管它此刻是否持有焦点都收下，但既不认领焦点也不带路由。
    if (packet.event_type == FieldTraits) {
      result.accepted = true;
      return result;
    }
    const bool owns = current_ && same_ticket(current_->transport, ticket);
    if (IsRouteDeactivation(packet.event_type)) {
      if (owns) {
        result = detach(client);
        // Keep exact cleanup identity for terminal notification after suspend.
        if (packet.event_type == ClientSuspended)
          client.last = result.cleanup;
      } else if (packet.event_type == ClientDeactivated && client.last &&
                 client.token == 0) {
        result.accepted = true;
        result.cleanup = client.last;
        client.last.reset();
      }
      result.terminal =
          result.accepted && packet.event_type == ClientDeactivated;
      return result;
    }
    const bool explicit_activation = packet.event_type == ClientActivated;
    const bool implicit_activation =
        packet.event_type == KeyEvent || packet.event_type == FocusRestored;
    if (explicit_activation || implicit_activation) {
      const uint64_t token =
          explicit_activation ? packet.request_id : client.token;
      // Pending explicit activation can queue input, but cannot lend its token
      // to another activation before the worker fence has completed.
      if (!owns || (explicit_activation && current_->token != token)) {
        if (!token)
          return {};
        auto change = gate_.begin(ticket, token);
        if (!change)
          return {};
        // Capture completed old fences atomically with replacement, even when
        // their control-queue receipt has not run yet.
        if (change->previous && change->previous_ready) {
          auto previous = clients_.find(change->previous->transport.client);
          if (previous != clients_.end())
            previous->second.token = change->previous->token;
        }
        // A failed fence may already have cleared the gate; queue-owned
        // Engine state still needs its exact previous lease cleaned up.
        change->previous = current_;
        if (explicit_activation)
          client.token = 0;
        current_ = change->pending;
        client.last = current_;
        result.activation = change;
        result.cleanup = change->previous;
      }
      result.fence = true;
    } else if (!owns) {
      return {}; // StatusSnapshot and UI messages never claim focus.
    }
    if (!current_)
      return {};
    // A failed ACK invalidates the gate before its control-queue completion.
    // Do not resurrect that route while the failure notification is in flight.
    if (!gate_.with_pending(*current_, [] {}) &&
        !gate_.with_active(*current_, [] {}))
      return {};
    result.accepted = true;
    result.route = current_;
    return result;
  }

private:
  struct Client {
    PipeTicket ticket;
    uint64_t token;
    std::optional<FocusLease> last;
  };
  static bool same_lease(const FocusLease &a, const FocusLease &b) {
    return a.epoch == b.epoch && a.token == b.token &&
           same_ticket(a.transport, b.transport);
  }
  void check_thread() const {
    if (std::this_thread::get_id() != thread_)
      throw std::logic_error("Wrong focus routing thread");
  }
  FocusRoute detach(Client &client) {
    FocusRoute result;
    result.accepted = true;
    result.cleanup = client.last;
    gate_.invalidate(client.ticket);
    if (current_ && same_ticket(current_->transport, client.ticket))
      current_.reset();
    client.token = 0;
    client.last.reset();
    return result;
  }
  FocusGate &gate_;
  size_t capacity_;
  const std::thread::id thread_ = std::this_thread::get_id();
  std::unordered_map<uint64_t, Client> clients_;
  std::optional<FocusLease> current_;
};
} // namespace msime::windows

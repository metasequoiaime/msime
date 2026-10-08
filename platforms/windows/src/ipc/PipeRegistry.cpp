#include "PipeRegistry.h"
#include "MainFrame.h"
#include "ReplyCodec.h"
#include <algorithm>
#include <cstring>
#include <limits>

namespace msime::windows {
namespace {
IoResult stale() {
  return {IoStatus::Cancelled, ERROR_OPERATION_ABORTED, 0, false, {}};
}
} // namespace
struct PipeRegistry::Endpoint {
  std::unique_ptr<PipeConnection> connection;
  std::shared_ptr<PipePeer> peer;
  uint64_t generation = 0;
  HANDLE cancel = CreateEventW(nullptr, TRUE, FALSE, nullptr);
  std::mutex read_mutex;
  ~Endpoint() {
    if (cancel)
      CloseHandle(cancel);
  }
  void stop() { SetEvent(cancel); }
  // False once the client process has exited or closed its end. Any other
  // probe failure is not proof of death, so the endpoint is kept.
  bool alive(uint64_t id) const {
    DWORD error = ERROR_SUCCESS;
    if (!peer->matches(connection->handle(), id, error))
      return false;
    if (PeekNamedPipe(connection->handle(), nullptr, 0, nullptr, nullptr,
                      nullptr))
      return true;
    error = GetLastError();
    return error != ERROR_BROKEN_PIPE && error != ERROR_PIPE_NOT_CONNECTED &&
           error != ERROR_NO_DATA;
  }
};
struct PipeRegistry::Client {
  std::mutex mutex;
  bool retired = false;
  std::array<std::shared_ptr<Endpoint>, 3> endpoints;
  void clear(uint32_t role) {
    if (endpoints[role])
      endpoints[role]->stop();
    endpoints[role].reset();
  }
  // Only endpoints that are really gone. A live TSF thread that lost Main
  // keeps its reverse pipes on purpose and re-establishes Main over them.
  void clear_dead(uint64_t id) {
    for (uint32_t role = 0; role < 3; ++role)
      if (endpoints[role] && !endpoints[role]->alive(id))
        clear(role);
  }
};
uint64_t PipeRegistry::next_generation() {
  std::lock_guard lock(mutex_);
  if (stopped_ || next_generation_ == std::numeric_limits<uint64_t>::max())
    return 0;
  return ++next_generation_;
}
std::shared_ptr<PipeRegistry::Client> PipeRegistry::lookup(uint64_t id,
                                                           bool create) {
  std::lock_guard lock(mutex_);
  if (stopped_ || !id)
    return {};
  auto it = clients_.find(id);
  if (it != clients_.end())
    return it->second;
  if (!create || clients_.size() >= max_clients_)
    return {};
  auto client = std::make_shared<Client>();
  clients_.emplace(id, client);
  return client;
}
// Called with Client::mutex held. lookup never waits for a client mutex while
// holding the map mutex, and shutdown releases the map lock before stopping
// I/O.
void PipeRegistry::retire(uint64_t id, const std::shared_ptr<Client> &client) {
  for (const auto &endpoint : client->endpoints)
    if (endpoint)
      return;
  client->retired = true;
  std::lock_guard lock(mutex_);
  auto it = clients_.find(id);
  if (it != clients_.end() && it->second == client)
    clients_.erase(it);
}
PipeTicket PipeRegistry::ticket(uint64_t id, const Client &client) {
  PipeTicket result;
  result.client = id;
  for (size_t i = 0; i < 3; ++i)
    if (client.endpoints[i])
      result.generations[i] = client.endpoints[i]->generation;
  return result;
}
bool PipeRegistry::current(const PipeTicket &value, const Client &client) {
  if (client.retired || !client.endpoints[0] || !client.endpoints[1] ||
      !client.endpoints[2])
    return false;
  return ticket(value.client, client).generations == value.generations;
}
PipeRegistration
PipeRegistry::register_reverse(std::unique_ptr<PipeConnection> connection,
                               uint32_t role, DWORD timeout, HANDLE cancel) {
  PipeRegistration result;
  if (!connection || (role != FanyImePipeRole::ToTsf &&
                      role != FanyImePipeRole::ToTsfWorkerThread))
    return result;
  auto endpoint = std::make_shared<Endpoint>();
  if (!endpoint->cancel)
    return result;
  endpoint->generation = next_generation();
  if (!endpoint->generation)
    return result;
  auto handshake = verify_reverse(connection->handle(), role, timeout, cancel);
  result.io = std::move(handshake.io);
  if (handshake.status != HandshakeStatus::Verified) {
    if (handshake.status == HandshakeStatus::TransportError)
      result.status = RegistryStatus::TransportError;
    else if (handshake.status == HandshakeStatus::IdentityRejected)
      result.status = RegistryStatus::IdentityRejected;
    return result;
  }
  endpoint->connection = std::move(connection);
  endpoint->peer = std::move(handshake.peer);
  auto client = lookup(handshake.client_id, true);
  if (!client && reclaim_dead())
    client = lookup(handshake.client_id, true);
  if (!client) {
    result.status = RegistryStatus::Capacity;
    return result;
  }
  std::lock_guard lock(client->mutex);
  if (client->retired) {
    result.status = RegistryStatus::Stale;
    return result;
  }
  if (client->endpoints[role] &&
      client->endpoints[role]->generation > endpoint->generation) {
    result.status = RegistryStatus::Stale;
    return result;
  }
  // A changed reverse endpoint invalidates the protocol/focus chain. The new
  // client must establish Main again; old queued work cannot target this pipe.
  DWORD error = ERROR_SUCCESS;
  if (!endpoint->peer->matches(endpoint->connection->handle(),
                               handshake.client_id, error)) {
    result.io = {IoStatus::Disconnected, error, 0, false, {}};
    retire(handshake.client_id, client);
    return result;
  }
  result.io = write_frame(endpoint->connection->handle(),
                          *pipe_ready_bytes(role), timeout, cancel);
  if (!result.io.complete()) {
    result.status = RegistryStatus::TransportError;
    retire(handshake.client_id, client);
    return result;
  }
  client->clear(FanyImePipeRole::Main);
  client->clear(role);
  client->endpoints[role] = std::move(endpoint);
  result.ticket = ticket(handshake.client_id, *client);
  result.status = RegistryStatus::Ready;
  return result;
}
PipeRegistration
PipeRegistry::register_main(std::unique_ptr<PipeConnection> connection,
                            const FanyImeNamedpipeData &hello,
                            uint32_t capabilities, DWORD timeout,
                            HANDLE cancel) {
  PipeRegistration result;
  if (!connection)
    return result;
  auto client = lookup(hello.client_id, false);
  if (!client)
    return result;
  auto endpoint = std::make_shared<Endpoint>();
  if (!endpoint->cancel)
    return result;
  endpoint->generation = next_generation();
  if (!endpoint->generation)
    return result;
  std::lock_guard lock(client->mutex);
  if (client->retired || !client->endpoints[1] || !client->endpoints[2])
    return result;
  auto &reply = client->endpoints[1];
  auto &worker = client->endpoints[2];
  DWORD error = ERROR_SUCCESS;
  if (!reply->peer->matches(connection->handle(), hello.client_id, error) ||
      !reply->peer->matches(worker->connection->handle(), hello.client_id,
                            error)) {
    result.status = RegistryStatus::IdentityRejected;
    result.io.system_error = error;
    return result;
  }
  if (client->endpoints[0] &&
      client->endpoints[0]->generation > endpoint->generation) {
    result.status = RegistryStatus::Stale;
    return result;
  }
  auto handshake = negotiate_main(
      connection->handle(), reply->connection->handle(), *reply->peer,
      hello.client_id, hello, capabilities, timeout, cancel);
  result.io = std::move(handshake.io);
  if (handshake.status != HandshakeStatus::Ready) {
    if (handshake.status == HandshakeStatus::TransportError)
      result.status = RegistryStatus::TransportError;
    else if (handshake.status == HandshakeStatus::IdentityRejected)
      result.status = RegistryStatus::IdentityRejected;
    // A protocol frame may already have reached the existing reverse pipe.
    // Fail the whole transport chain; never let old key replies follow it.
    for (uint32_t role = 0; role < 3; ++role)
      client->clear(role);
    retire(hello.client_id, client);
    return result;
  }
  endpoint->connection = std::move(connection);
  endpoint->peer = reply->peer;
  client->clear(0);
  client->endpoints[0] = std::move(endpoint);
  result.ticket = ticket(hello.client_id, *client);
  result.status = RegistryStatus::Ready;
  return result;
}
bool PipeRegistry::try_is_current(const PipeTicket &value) {
  std::shared_ptr<Client> client;
  {
    std::unique_lock lock(mutex_, std::try_to_lock);
    if (!lock || stopped_ || !value.client)
      return false;
    const auto found = clients_.find(value.client);
    if (found == clients_.end())
      return false;
    client = found->second;
  }
  // Never hold the map lock while inspecting an individual client. In
  // particular, registration can hold its mutex across handshake I/O without
  // participating in the controller's focus gate.
  std::unique_lock lock(client->mutex, std::try_to_lock);
  return lock && current(value, *client);
}
bool PipeRegistry::is_current(const PipeTicket &value) {
  auto client = lookup(value.client, false);
  if (!client)
    return false;
  std::lock_guard lock(client->mutex);
  return current(value, *client);
}
std::vector<PipeTicket> PipeRegistry::current_tickets() {
  std::vector<std::pair<uint64_t, std::shared_ptr<Client>>> clients;
  {
    std::lock_guard lock(mutex_);
    if (stopped_)
      return {};
    clients.reserve(clients_.size());
    for (const auto &[id, client] : clients_)
      clients.emplace_back(id, client);
  }
  std::vector<PipeTicket> result;
  result.reserve(clients.size());
  for (const auto &[id, client] : clients) {
    std::lock_guard lock(client->mutex);
    if (!client->retired && client->endpoints[0] && client->endpoints[1] &&
        client->endpoints[2])
      result.push_back(ticket(id, *client));
  }
  std::sort(result.begin(), result.end(),
            [](const PipeTicket &a, const PipeTicket &b) {
              return a.client < b.client;
            });
  return result;
}
IoResult PipeRegistry::read_main(const PipeTicket &value, DWORD timeout) {
  auto client = lookup(value.client, false);
  if (!client)
    return stale();
  std::shared_ptr<Endpoint> endpoint;
  {
    std::lock_guard lock(client->mutex);
    if (!current(value, *client))
      return stale();
    endpoint = client->endpoints[0];
  }
  IoResult result;
  {
    std::lock_guard io_lock(endpoint->read_mutex);
    result = timeout == INFINITE
                 ? read_frame_until_cancel(endpoint->connection->handle(),
                                           sizeof(FanyImeNamedpipeData),
                                           endpoint->cancel)
                 : read_frame(endpoint->connection->handle(),
                              sizeof(FanyImeNamedpipeData), timeout,
                              endpoint->cancel);
  }
  {
    std::lock_guard lock(client->mutex);
    if (!current(value, *client))
      return stale();
    DWORD error = ERROR_SUCCESS;
    if (!endpoint->peer->matches(endpoint->connection->handle(), value.client,
                                 error)) {
      client->clear(0);
      client->clear_dead(value.client);
      retire(value.client, client);
      return {IoStatus::Disconnected, error, 0, false, {}};
    }
  }
  if (result.complete()) {
    FanyImeNamedpipeData packet{};
    std::memcpy(&packet, result.frame.data(), sizeof(packet));
    if (!valid_main_frame(packet, value.client))
      result = {IoStatus::MalformedFrame, ERROR_INVALID_DATA,
                result.transferred, false, {}};
  }
  if (!result.complete()) {
    remove(value, 0);
    // A closed Main may mean the whole TSF thread or process is gone; if so
    // its reverse endpoints are dead too and must not hold a slot for good.
    // Cancelled comes from close or replacement, which is not that.
    if (result.status == IoStatus::Disconnected) {
      std::lock_guard lock(client->mutex);
      if (!client->retired) {
        client->clear_dead(value.client);
        retire(value.client, client);
      }
    }
  }
  return result;
}
IoResult PipeRegistry::send(const PipeTicket &value, uint32_t role,
                            const std::vector<uint8_t> &frame, DWORD timeout) {
  if ((role != 1 && role != 2) ||
      frame.size() != (role == 1
                           ? sizeof(FanyImeNamedpipeDataToTsf)
                           : sizeof(FanyImeNamedpipeDataToTsfWorkerThread)))
    return {IoStatus::InvalidArgument, ERROR_INVALID_PARAMETER, 0, false, {}};
  auto client = lookup(value.client, false);
  if (!client)
    return stale();
  std::lock_guard lock(client->mutex);
  if (!current(value, *client))
    return stale();
  auto endpoint = client->endpoints[role];
  DWORD error = ERROR_SUCCESS;
  if (!endpoint->peer->matches(endpoint->connection->handle(), value.client,
                               error)) {
    client->clear(0);
    client->clear(role);
    client->clear_dead(value.client);
    retire(value.client, client);
    return {IoStatus::Disconnected, error, 0, false, {}};
  }
  auto result = write_frame(endpoint->connection->handle(), frame, timeout,
                            endpoint->cancel);
  if (!result.complete()) {
    client->clear(0);
    client->clear(role);
    client->clear_dead(value.client);
    retire(value.client, client);
  }
  return result;
}
bool PipeRegistry::remove(const PipeTicket &value, uint32_t role) {
  if (role > 2 || !value.generations[role])
    return false;
  auto client = lookup(value.client, false);
  if (!client)
    return false;
  std::lock_guard lock(client->mutex);
  if (client->retired || !client->endpoints[role] ||
      client->endpoints[role]->generation != value.generations[role])
    return false;
  client->clear(0);
  client->clear(role);
  retire(value.client, client);
  return true;
}
// Only when the map is full, so the hot path never probes. Never takes a
// client mutex under the map mutex, and skips a client busy in handshake I/O
// rather than waiting on it.
bool PipeRegistry::reclaim_dead() {
  std::vector<std::pair<uint64_t, std::shared_ptr<Client>>> clients;
  {
    std::lock_guard lock(mutex_);
    if (stopped_)
      return false;
    if (clients_.size() < max_clients_)
      return true;
    clients.reserve(clients_.size());
    for (const auto &[id, client] : clients_)
      clients.emplace_back(id, client);
  }
  bool reclaimed = false;
  for (const auto &[id, client] : clients) {
    std::unique_lock lock(client->mutex, std::try_to_lock);
    if (!lock || client->retired)
      continue;
    client->clear_dead(id);
    retire(id, client);
    reclaimed = reclaimed || client->retired;
  }
  return reclaimed;
}
void PipeRegistry::shutdown() {
  std::unordered_map<uint64_t, std::shared_ptr<Client>> clients;
  {
    std::lock_guard lock(mutex_);
    stopped_ = true;
    clients.swap(clients_);
  }
  for (auto &[id, client] : clients) {
    (void)id;
    std::lock_guard lock(client->mutex);
    client->retired = true;
    for (uint32_t role = 0; role < 3; ++role)
      client->clear(role);
  }
}
} // namespace msime::windows

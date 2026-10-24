#include "../../compat/WindowsVersion.h"
#include "PipePeer.h"
#include <vector>

namespace msime::windows {
namespace {
struct Handle {
  HANDLE value = nullptr;
  Handle() = default;
  Handle(const Handle &) = delete;
  Handle &operator=(const Handle &) = delete;
  ~Handle() {
    if (value)
      CloseHandle(value);
  }
};
bool identity(HANDLE pipe, ULONG &pid, ULONG &session, DWORD &error) {
  DWORD flags = 0;
  if (!GetNamedPipeInfo(pipe, &flags, nullptr, nullptr, nullptr)) {
    error = GetLastError();
    return false;
  }
  if (!(flags & PIPE_SERVER_END)) {
    error = ERROR_INVALID_PARAMETER;
    return false;
  }
  if (!GetNamedPipeClientProcessId(pipe, &pid) ||
      !GetNamedPipeClientSessionId(pipe, &session)) {
    error = GetLastError();
    return false;
  }
  if (!pid) {
    error = ERROR_ACCESS_DENIED;
    return false;
  }
  return true;
}
bool user(HANDLE process, std::vector<unsigned char> &data, DWORD &error) {
  Handle token;
  if (!OpenProcessToken(process, TOKEN_QUERY, &token.value)) {
    error = GetLastError();
    return false;
  }
  DWORD needed = 0;
  GetTokenInformation(token.value, TokenUser, nullptr, 0, &needed);
  if (GetLastError() != ERROR_INSUFFICIENT_BUFFER || !needed) {
    error = ERROR_INVALID_DATA;
    return false;
  }
  data.resize(needed);
  if (!GetTokenInformation(token.value, TokenUser, data.data(), needed,
                           &needed)) {
    error = GetLastError();
    return false;
  }
  return true;
}
bool same_session(ULONG session, DWORD &error) {
  DWORD own_session = 0;
  if (!ProcessIdToSessionId(GetCurrentProcessId(), &own_session)) {
    error = GetLastError();
    return false;
  }
  if (session != own_session) {
    error = ERROR_ACCESS_DENIED;
    return false;
  }
  return true;
}
bool same_user(HANDLE process, DWORD &error) {
  std::vector<unsigned char> peer_user, own_user;
  if (!user(process, peer_user, error) ||
      !user(GetCurrentProcess(), own_user, error))
    return false;
  const auto peer_sid =
      reinterpret_cast<const TOKEN_USER *>(peer_user.data())->User.Sid;
  const auto own_sid =
      reinterpret_cast<const TOKEN_USER *>(own_user.data())->User.Sid;
  if (!IsValidSid(peer_sid) || !IsValidSid(own_sid) ||
      !EqualSid(peer_sid, own_sid)) {
    error = ERROR_ACCESS_DENIED;
    return false;
  }
  return true;
}
// 设置进程必须属于当前账号、会话、非 AppContainer，且至少中完整性。
bool desktop_token(HANDLE process, DWORD &error) {
  Handle token;
  if (!OpenProcessToken(process, TOKEN_QUERY, &token.value)) {
    error = GetLastError();
    return false;
  }
  DWORD app_container = 1, size = 0;
  if (!GetTokenInformation(token.value, TokenIsAppContainer, &app_container,
                           sizeof(app_container), &size)) {
    error = GetLastError();
    if (error != ERROR_INVALID_PARAMETER || !compat::predates_appcontainers())
      return false;
    app_container = 0;
  }
  if (app_container) {
    error = ERROR_ACCESS_DENIED;
    return false;
  }
  DWORD needed = 0;
  GetTokenInformation(token.value, TokenIntegrityLevel, nullptr, 0, &needed);
  if (GetLastError() != ERROR_INSUFFICIENT_BUFFER || !needed) {
    error = ERROR_INVALID_DATA;
    return false;
  }
  std::vector<unsigned char> data(needed);
  if (!GetTokenInformation(token.value, TokenIntegrityLevel, data.data(),
                           needed, &needed)) {
    error = GetLastError();
    return false;
  }
  PSID label = reinterpret_cast<const TOKEN_MANDATORY_LABEL *>(data.data())
                   ->Label.Sid;
  if (!IsValidSid(label) || *GetSidSubAuthorityCount(label) == 0) {
    error = ERROR_INVALID_DATA;
    return false;
  }
  const DWORD rid =
      *GetSidSubAuthority(label, *GetSidSubAuthorityCount(label) - 1);
  if (rid < SECURITY_MANDATORY_MEDIUM_RID) {
    error = ERROR_ACCESS_DENIED;
    return false;
  }
  error = ERROR_SUCCESS;
  return true;
}
} // namespace
bool pipe_client_in_session(HANDLE pipe, ULONG &pid, DWORD &error) {
  error = ERROR_SUCCESS;
  ULONG session = 0;
  return identity(pipe, pid, session, error) && same_session(session, error);
}
bool pipe_client_is_desktop_user(HANDLE pipe, DWORD &error) {
  ULONG pid = 0;
  if (!pipe_client_in_session(pipe, pid, error))
    return false;
  Handle process;
  process.value = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid);
  if (!process.value) {
    error = GetLastError();
    return false;
  }
  return same_user(process.value, error) && desktop_token(process.value, error);
}
std::unique_ptr<PipePeer> PipePeer::bind(HANDLE pipe, uint64_t client_id,
                                         DWORD &error) {
  error = ERROR_SUCCESS;
  ULONG pid = 0, session = 0;
  if (!identity(pipe, pid, session, error))
    return nullptr;
  if (!same_session(session, error))
    return nullptr;
  if (client_id == 0 || static_cast<DWORD>(client_id >> 32) != pid) {
    error = ERROR_ACCESS_DENIED;
    return nullptr;
  }
  Handle process;
  process.value =
      OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE, FALSE, pid);
  if (!process.value) {
    error = GetLastError();
    return nullptr;
  }
  if (!same_user(process.value, error))
    return nullptr;
  // Allocate before handing off ownership, preserving cleanup on exceptions.
  auto peer = std::unique_ptr<PipePeer>(
      new PipePeer(process.value, client_id, session));
  process.value = nullptr;
  if (!peer->matches(pipe, client_id, error))
    return nullptr;
  return peer;
}
PipePeer::~PipePeer() { CloseHandle(process_); }
bool PipePeer::matches(HANDLE pipe, uint64_t client_id, DWORD &error) const {
  error = ERROR_SUCCESS;
  if (client_id != client_id_) {
    error = ERROR_ACCESS_DENIED;
    return false;
  }
  const auto alive = WaitForSingleObject(process_, 0);
  if (alive != WAIT_TIMEOUT) {
    error = alive == WAIT_FAILED ? GetLastError() : ERROR_PROCESS_ABORTED;
    return false;
  }
  ULONG pid = 0, session = 0;
  if (!identity(pipe, pid, session, error))
    return false;
  if (pid != static_cast<DWORD>(client_id_ >> 32) || session != session_) {
    error = ERROR_ACCESS_DENIED;
    return false;
  }
  return true;
}
} // namespace msime::windows

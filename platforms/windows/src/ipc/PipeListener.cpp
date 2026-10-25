#include "PipeListener.h"
#include <sddl.h>

namespace msime::windows {
namespace {
struct Handle {
  HANDLE value = nullptr;
  ~Handle() {
    if (value)
      CloseHandle(value);
  }
};
struct Local {
  HLOCAL value = nullptr;
  ~Local() {
    if (value)
      LocalFree(value);
  }
};
bool descriptor(PSECURITY_DESCRIPTOR &output, DWORD &error) {
  Handle token;
  if (!OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &token.value)) {
    error = GetLastError();
    return false;
  }
  DWORD size = 0;
  GetTokenInformation(token.value, TokenUser, nullptr, 0, &size);
  if (!size || GetLastError() != ERROR_INSUFFICIENT_BUFFER) {
    error = ERROR_INVALID_DATA;
    return false;
  }
  std::vector<unsigned char> data(size);
  if (!GetTokenInformation(token.value, TokenUser, data.data(), size, &size)) {
    error = GetLastError();
    return false;
  }
  auto sid = reinterpret_cast<const TOKEN_USER *>(data.data())->User.Sid;
  LPWSTR text = nullptr;
  if (!ConvertSidToStringSidW(sid, &text)) {
    error = GetLastError();
    return false;
  }
  Local sid_text{text};
  // 保留当前账号、SYSTEM、AppContainer 仅连接权限和低完整性标签，
  // 不允许 Everyone，也不授予 AppContainer 创建管道实例的权限。
  // Win7 的 SDDL 解析器不认识 AC 别名，但接受同一个数值 SID；
  // 使用数值形式保持所有系统上的 ACL 完全一致，对端仍校验账号和会话。
  constexpr DWORD client_access =
      (FILE_GENERIC_READ | FILE_GENERIC_WRITE) & ~FILE_CREATE_PIPE_INSTANCE;
  static_assert(client_access == 0x12019bu);
  const auto sddl = std::wstring(L"S:(ML;;NW;;;LW)D:P(A;;FA;;;SY)(A;;FA;;;") +
                    text + L")(A;;0x12019b;;;S-1-15-2-1)";
  if (!ConvertStringSecurityDescriptorToSecurityDescriptorW(
          sddl.c_str(), SDDL_REVISION_1, &output, nullptr)) {
    error = GetLastError();
    return false;
  }
  return true;
}
IoResult connect(HANDLE pipe, DWORD timeout, HANDLE cancel) {
  if (!timeout || timeout == INFINITE)
    return {IoStatus::InvalidArgument, ERROR_INVALID_PARAMETER, 0, false, {}};
  if (cancel) {
    const auto wait = WaitForSingleObject(cancel, 0);
    if (wait == WAIT_OBJECT_0)
      return {IoStatus::Cancelled, ERROR_OPERATION_ABORTED, 0, false, {}};
    if (wait != WAIT_TIMEOUT)
      return {IoStatus::InvalidArgument, GetLastError(), 0, false, {}};
  }
  Handle event{CreateEventW(nullptr, TRUE, FALSE, nullptr)};
  if (!event.value)
    return {IoStatus::Failed, GetLastError(), 0, false, {}};
  OVERLAPPED operation{};
  operation.hEvent = event.value;
  if (ConnectNamedPipe(pipe, &operation))
    return {IoStatus::Complete, ERROR_SUCCESS, 0, false, {}};
  auto error = GetLastError();
  if (error == ERROR_PIPE_CONNECTED)
    return {IoStatus::Complete, ERROR_SUCCESS, 0, false, {}};
  if (error != ERROR_IO_PENDING)
    return {IoStatus::Failed, error, 0, false, {}};
  HANDLE events[] = {event.value, cancel};
  const auto wait =
      WaitForMultipleObjects(cancel ? 2 : 1, events, FALSE, timeout);
  DWORD ignored = 0; // ConnectNamedPipe has no meaningful byte count.
  if (wait != WAIT_OBJECT_0) {
    error = wait == WAIT_FAILED ? GetLastError() : ERROR_OPERATION_ABORTED;
    CancelIoEx(pipe, &operation);
    GetOverlappedResult(pipe, &operation, &ignored, TRUE);
    if (wait == WAIT_TIMEOUT)
      return {IoStatus::Timeout, WAIT_TIMEOUT, 0, false, {}};
    if (cancel && wait == WAIT_OBJECT_0 + 1)
      return {IoStatus::Cancelled, ERROR_OPERATION_ABORTED, 0, false, {}};
    return {IoStatus::Failed, error, 0, false, {}};
  }
  if (!GetOverlappedResult(pipe, &operation, &ignored, FALSE))
    return {IoStatus::Failed, GetLastError(), 0, false, {}};
  return {IoStatus::Complete, ERROR_SUCCESS, 0, false, {}};
}
} // namespace
std::unique_ptr<PipeListener> PipeListener::create(const std::wstring &name,
                                                   DWORD &error) {
  error = ERROR_SUCCESS;
  const std::wstring prefix = L"\\\\.\\pipe\\";
  if (name.size() <= prefix.size() || name.size() > 256 ||
      name.compare(0, prefix.size(), prefix) != 0 ||
      name.find_first_of(L"\\/:*?", prefix.size()) != std::wstring::npos ||
      name.find(L'\0') != std::wstring::npos) {
    error = ERROR_INVALID_NAME;
    return nullptr;
  }
  auto listener = std::unique_ptr<PipeListener>(new PipeListener(name));
  if (!descriptor(listener->security_, error))
    return nullptr;
  listener->pending_ = listener->instance(true);
  if (listener->pending_ == INVALID_HANDLE_VALUE) {
    error = GetLastError();
    return nullptr;
  }
  return listener;
}
HANDLE PipeListener::instance(bool first) const {
  SECURITY_ATTRIBUTES attributes{sizeof(SECURITY_ATTRIBUTES), security_, FALSE};
  return CreateNamedPipeW(name_.c_str(),
                          PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED |
                              (first ? FILE_FLAG_FIRST_PIPE_INSTANCE : 0),
                          PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE |
                              PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                          PIPE_UNLIMITED_INSTANCES, 32 * 1024, 32 * 1024, 0,
                          &attributes);
}
PipeAccept PipeListener::accept(DWORD timeout, HANDLE cancel) {
  PipeAccept result;
  result.io = connect(pending_, timeout, cancel);
  if (!result.io.complete()) {
    if (result.io.status == IoStatus::InvalidArgument)
      return result;
    // The connect operation has been drained, including cancellation races.
    // Keep the instance handle alive so the pipe name never becomes unowned.
    DisconnectNamedPipe(pending_);
    return result;
  }
  // Allocate before creating another native handle so exceptions cannot leak
  // it.
  auto connection = std::unique_ptr<PipeConnection>(new PipeConnection());
  const auto replacement = instance(false);
  if (replacement == INVALID_HANDLE_VALUE) {
    result.io = {IoStatus::Failed, GetLastError(), 0, false, {}};
    DisconnectNamedPipe(pending_);
    return result;
  }
  connection->handle_ = pending_;
  pending_ = replacement;
  result.connection = std::move(connection);
  return result;
}
PipeListener::~PipeListener() {
  if (pending_ != INVALID_HANDLE_VALUE)
    CloseHandle(pending_);
  if (security_)
    LocalFree(security_);
}
PipeConnection::~PipeConnection() {
  if (handle_ != INVALID_HANDLE_VALUE)
    CloseHandle(handle_);
}
} // namespace msime::windows

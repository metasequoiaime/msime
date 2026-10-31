#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <werapi.h>
#include "CrashCaptureContext.h"
#include "WerApiCompatibility.h"
#include <cstdio>
#include <cstring>
#include <cwchar>

namespace {
using msime::tsf::wer::Context;
using msime::tsf::wer::Module;
static_assert(sizeof(wchar_t) == sizeof(char16_t));

bool read(HANDLE process, const void *address, void *value, SIZE_T size) {
  SIZE_T copied = 0;
  return ReadProcessMemory(process, address, value, size, &copied) && copied == size;
}

bool current(HANDLE process, const void *address, std::uint32_t sequence) {
  std::uint32_t state[2]{};
  return read(process, address, state, sizeof(state)) && state[0] == sequence && state[1] == 1;
}

bool snapshot(HANDLE process, const void *address, Context &result) {
  std::uint32_t sequence = 0;
  return address && read(process, address, &sequence, sizeof(sequence)) && sequence != 0 &&
      (sequence & 1) == 0 && read(process, address, &result, sizeof(result)) &&
      result.magic == msime::tsf::wer::context_magic && result.size == sizeof(result) &&
      result.sequence == sequence && result.armed == 1 && current(process, address, sequence);
}

// WER 可能以较宽权限运行；所有路径检查和写盘都模拟目标令牌，不能替沙箱越权。
class TargetIdentity {
public:
  explicit TargetIdentity(HANDLE process) {
    HANDLE token = nullptr;
    if (!OpenThreadToken(GetCurrentThread(), TOKEN_QUERY | TOKEN_IMPERSONATE, TRUE, &previous_) &&
        GetLastError() != ERROR_NO_TOKEN) return;
    if (!OpenProcessToken(process, TOKEN_QUERY | TOKEN_DUPLICATE, &token)) return;
    HANDLE duplicate = nullptr;
    if (DuplicateTokenEx(token, TOKEN_QUERY | TOKEN_IMPERSONATE, nullptr,
                         SecurityImpersonation, TokenImpersonation, &duplicate)) {
      active_ = SetThreadToken(nullptr, duplicate) != FALSE;
      CloseHandle(duplicate);
    }
    CloseHandle(token);
  }
  ~TargetIdentity() {
    if (active_) SetThreadToken(nullptr, previous_);
    if (previous_) CloseHandle(previous_);
  }
  explicit operator bool() const { return active_; }
  TargetIdentity(const TargetIdentity &) = delete;
  TargetIdentity &operator=(const TargetIdentity &) = delete;
private:
  HANDLE previous_ = nullptr;
  bool active_ = false;
};

// 保持每一级父目录句柄且不共享删除，防止检查后被改名或换成重解析点。
class Parents {
public:
  bool lock(const wchar_t *path) {
    wchar_t prefix[msime::tsf::wer::path_capacity]{};
    wchar_t root[]{path[0], L':', L'\\', 0};
    if (GetDriveTypeW(root) != DRIVE_FIXED) return false;
    std::wcscpy(prefix, path);
    for (std::size_t i = 3; prefix[i]; ++i) {
      if (prefix[i] != L'\\') continue;
      if (count_ == sizeof(handles_) / sizeof(handles_[0])) return false;
      prefix[i] = 0;
      const HANDLE directory = CreateFileW(prefix, FILE_READ_ATTRIBUTES,
          FILE_SHARE_READ | FILE_SHARE_WRITE, nullptr, OPEN_EXISTING,
          FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT, nullptr);
      prefix[i] = L'\\';
      if (directory == INVALID_HANDLE_VALUE) return false;
      handles_[count_++] = directory;
      BY_HANDLE_FILE_INFORMATION info{};
      if (!GetFileInformationByHandle(directory, &info) ||
          !(info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) ||
          (info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT)) return false;
    }
    return count_ != 0;
  }
  ~Parents() { while (count_) CloseHandle(handles_[--count_]); }
  Parents() = default;
  Parents(const Parents &) = delete;
  Parents &operator=(const Parents &) = delete;
private:
  HANDLE handles_[128]{};
  std::size_t count_ = 0;
};

bool terminated(const char *value, std::size_t capacity) { return std::memchr(value, 0, capacity) != nullptr; }
bool valid_name(const Module &module) {
  if (!terminated(module.name, sizeof(module.name)) || module.name[0] == 0) return false;
  for (const char *p = module.name; *p; ++p)
    if (!((*p >= 'a' && *p <= 'z') || (*p >= 'A' && *p <= 'Z') ||
          (*p >= '0' && *p <= '9') || *p == '_' || *p == '.' || *p == '-')) return false;
  return std::strcmp(module.name, "MetasequoiaImeTsf.dll") == 0 ||
      std::strncmp(module.name, "msime_host_api", 14) == 0;
}

const Module *owner(const Context &context, std::uint64_t address) {
  for (const auto &module : context.modules)
    if (module.base && module.size && address >= module.base &&
        address - module.base < module.size && valid_name(module)) return &module;
  return nullptr;
}

bool valid_record(const Context &context) {
  // 输出路径只允许预备的本地绝对路径和 UUID 记录名，不接受设备/网络路径或无界文本。
  const auto *path = reinterpret_cast<const wchar_t *>(context.path);
  std::size_t length = 0;
  while (length < msime::tsf::wer::path_capacity && path[length]) ++length;
  if (length < 45 || length >= msime::tsf::wer::path_capacity || path[1] != L':' ||
      path[2] != L'\\' || !((path[0] >= L'A' && path[0] <= L'Z') ||
                            (path[0] >= L'a' && path[0] <= L'z'))) return false;
  for (std::size_t i = 3, start = 3; i < length; ++i) {
    if (path[i] < L' ' || path[i] == L':' || path[i] == L'/') return false;
    if (path[i] == L'\\') {
      if (i == start || path[i - 1] == L'.' || path[i - 1] == L' ') return false;
      start = i + 1;
    }
  }
  const wchar_t *name = std::wcsrchr(path, L'\\');
  if (!name || std::wcslen(++name) != 42 || std::wcscmp(name + 36, L".crash") != 0) return false;
  for (std::size_t i = 0; i < 36; ++i) {
    if (i == 8 || i == 13 || i == 18 || i == 23) { if (name[i] != L'-') return false; }
    else if (!((name[i] >= L'0' && name[i] <= L'9') || (name[i] >= L'a' && name[i] <= L'f'))) return false;
  }
  if (!terminated(context.header, sizeof(context.header)) ||
      std::strncmp(context.header, "MSIME-CRASH/1 ", 14) != 0) return false;
  const auto headerLength = std::strlen(context.header);
  return headerLength > 14 && context.header[headerLength - 1] == '\n' &&
      std::strchr(context.header, '\n') == context.header + headerLength - 1 &&
      std::strchr(context.header, '\r') == nullptr;
}

const char *exception_name(DWORD code) {
  switch (code) {
  case EXCEPTION_ACCESS_VIOLATION: return "EXCEPTION_ACCESS_VIOLATION";
  case EXCEPTION_STACK_OVERFLOW: return "EXCEPTION_STACK_OVERFLOW";
  case EXCEPTION_ILLEGAL_INSTRUCTION: return "EXCEPTION_ILLEGAL_INSTRUCTION";
  case EXCEPTION_IN_PAGE_ERROR: return "EXCEPTION_IN_PAGE_ERROR";
  case EXCEPTION_INT_DIVIDE_BY_ZERO: return "EXCEPTION_INT_DIVIDE_BY_ZERO";
  case EXCEPTION_PRIV_INSTRUCTION: return "EXCEPTION_PRIV_INSTRUCTION";
  default: return "EXCEPTION";
  }
}

bool write_all(HANDLE file, const char *data, std::size_t size) {
  DWORD written = 0;
  return size <= MAXDWORD && WriteFile(file, data, static_cast<DWORD>(size), &written, nullptr) && written == size;
}

#if defined(_M_IX86) || defined(__i386__)
// 只把返回地址中属于自有模块的帧写入记录；外部宿主帧仍参与链校验，但不泄露模块名。
bool write_owned_frame(HANDLE file, const Context &context, std::uint64_t address) {
  const auto *module = owner(context, address);
  if (!module) return true;
  char frame[160]{};
  const int length = std::snprintf(frame, sizeof(frame), "%s+0x%llx\n", module->name,
      static_cast<unsigned long long>(address - module->base));
  return length > 0 && static_cast<std::size_t>(length) < sizeof(frame) &&
      write_all(file, frame, static_cast<std::size_t>(length));
}

// WER helper 在另一个进程中运行，不能让普通的本地展开器解引用目标栈；x86 仅沿有界 EBP 链逐项远程读取。
bool write_x86_frames(HANDLE file, HANDLE process, const Context &context, const CONTEXT &registers) {
  DWORD pointer = registers.Ebp;
  for (int frame = 0; frame < 64 && pointer; ++frame) {
    struct Link {
      DWORD previous;
      DWORD address;
    } link{};
    if (!read(process, reinterpret_cast<const void *>(static_cast<std::uintptr_t>(pointer)),
              &link, sizeof(link)) || !link.address) break;
    if (!write_owned_frame(file, context, link.address)) return false;
    // 损坏、回环或跨出合理范围的链立即停止，不直接解引用故障栈。
    if (link.previous <= pointer || link.previous - pointer > 1024 * 1024 ||
        link.previous % sizeof(DWORD)) break;
    pointer = link.previous;
  }
  return true;
}
#endif

// STATUS_STACK_BUFFER_OVERRUN（__fastfail、/GS、CFG、CRT 无效参数）、STATUS_HEAP_CORRUPTION 和 STATUS_FAIL_FAST_EXCEPTION 不经过进程内的未处理异常过滤器。
bool bypasses_unhandled_filter(DWORD code) {
  return code == 0xC0000409 || code == 0xC0000374 || code == 0xC0000602;
}

void record(const void *remote, const WER_RUNTIME_EXCEPTION_INFORMATION &event) {
  Context context{};
  if (!event.bIsFatal || !snapshot(event.hProcess, remote, context) ||
      !valid_record(context)) return;
  // 进程内记录器已带完整栈和正确组件记下的故障不再记第二份；绕过它的快速失败只有这里能记。
  if (context.in_process_recorder && !bypasses_unhandled_filter(event.exceptionRecord.ExceptionCode)) return;
  const auto address = static_cast<std::uint64_t>(reinterpret_cast<std::uintptr_t>(event.exceptionRecord.ExceptionAddress));
  const auto *module = owner(context, address);
  if (!module) return;
  TargetIdentity identity(event.hProcess);
  Parents parents;
  if (!identity || !parents.lock(reinterpret_cast<const wchar_t *>(context.path))) return;
  char record[512]{};
  const int length = std::snprintf(record, sizeof(record), "%s (0x%08lx)\n%s+0x%llx\n",
      exception_name(event.exceptionRecord.ExceptionCode), static_cast<unsigned long>(event.exceptionRecord.ExceptionCode),
      module->name, static_cast<unsigned long long>(address - module->base));
  if (length <= 0 || static_cast<std::size_t>(length) >= sizeof(record)) return;
  const auto *path = reinterpret_cast<const wchar_t *>(context.path);
  wchar_t temporary[msime::tsf::wer::path_capacity + 16]{};
  if (std::swprintf(temporary, sizeof(temporary) / sizeof(temporary[0]), L"%ls.werpartial", path) <= 0) return;
  HANDLE file = CreateFileW(temporary, GENERIC_WRITE, 0, nullptr, CREATE_NEW,
                            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT, nullptr);
  if (file == INVALID_HANDLE_VALUE) return;
  bool complete = write_all(file, context.header, std::strlen(context.header)) &&
      write_all(file, record, static_cast<std::size_t>(length));
  if (complete) {
#if defined(_M_IX86) || defined(__i386__)
    complete = write_x86_frames(file, event.hProcess, context, event.context);
#endif
    complete = complete && FlushFileBuffers(file);
  }
  CloseHandle(file);
  // 不覆盖首份；停用或快照换代后，迟到的 helper 不能再发布旧准备。
  if (!complete || !current(event.hProcess, remote, context.sequence) ||
      !MoveFileExW(temporary, path, MOVEFILE_WRITE_THROUGH)) DeleteFileW(temporary);
}
}

extern "C" __declspec(dllexport) HRESULT __cdecl OutOfProcessExceptionEventCallback(
    PVOID context, const PWER_RUNTIME_EXCEPTION_INFORMATION event, PBOOL ownership,
    PWSTR, PDWORD, PDWORD) {
  if (!ownership) return E_POINTER;
  *ownership = FALSE;
  // WER 原生处理继续；不填写事件名、参数数量或宿主的调试器选项。
  if (event && event->dwSize >= sizeof(WER_RUNTIME_EXCEPTION_INFORMATION)) record(context, *event);
  return S_OK;
}
extern "C" __declspec(dllexport) HRESULT __cdecl OutOfProcessExceptionEventSignatureCallback(
    PVOID, const PWER_RUNTIME_EXCEPTION_INFORMATION, DWORD, PWSTR, PDWORD, PWSTR, PDWORD) { return E_NOTIMPL; }
extern "C" __declspec(dllexport) HRESULT __cdecl OutOfProcessExceptionEventDebuggerLaunchCallback(
    PVOID, const PWER_RUNTIME_EXCEPTION_INFORMATION, PBOOL, PWSTR, PDWORD, PBOOL) { return E_NOTIMPL; }
static_assert(std::is_same_v<decltype(&OutOfProcessExceptionEventCallback), msime::tsf::wer::ExceptionEvent>);
static_assert(std::is_same_v<decltype(&OutOfProcessExceptionEventSignatureCallback), msime::tsf::wer::ExceptionSignature>);
static_assert(std::is_same_v<decltype(&OutOfProcessExceptionEventDebuggerLaunchCallback), msime::tsf::wer::DebuggerLaunch>);
BOOL WINAPI DllMain(HINSTANCE, DWORD, LPVOID) { return TRUE; }

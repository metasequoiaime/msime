#include "Telemetry.h"
#include "msime_client.h"
#include <nlohmann/json.hpp>
#include <atomic>
#include <chrono>
#include <cstdlib>
#include <cstring>
#include <exception>
#include <iterator>
#include <memory>
#include <mutex>
#include <sstream>
#include <thread>
#include <typeinfo>
#ifdef _WIN32
#include <vector>
#include <windows.h>
#else
#include <cerrno>
#include <csignal>
#include <fcntl.h>
#include <unistd.h>
#if __has_include(<execinfo.h>)
#include <execinfo.h>
#define MSIME_TELEMETRY_BACKTRACE 1
#endif
#if __has_include(<cxxabi.h>)
#include <cxxabi.h>
#define MSIME_TELEMETRY_DEMANGLE 1
#endif
#endif

namespace msime::telemetry {
namespace {
using Request = char *(*)(const uint8_t *, size_t);

std::mutex host_lock;
Host current;
bool configured = false;

// The crash record path of the running session, read by crash handlers that may not allocate or lock. armed is cleared before the path changes and set after, so a handler sees a whole path or none.
std::atomic<bool> armed{false};
#ifdef _WIN32
wchar_t record_path[32768];
#else
char record_path[4096];
#endif

// Calls one Host API function with a JSON request; the value on success, nothing on any failure. Reporting never takes the host down.
std::optional<nlohmann::json> call(Request function, const nlohmann::json &request) {
  try {
    const auto body = request.dump();
    std::unique_ptr<char, decltype(&msime_client_string_free)> response(
        function(reinterpret_cast<const uint8_t *>(body.data()), body.size()), msime_client_string_free);
    if (!response)
      return std::nullopt;
    auto parsed = nlohmann::json::parse(response.get(), nullptr, false);
    if (parsed.is_discarded() || !parsed.value("ok", false) || !parsed.contains("value"))
      return std::nullopt;
    return std::move(parsed["value"]);
  } catch (...) {
    return std::nullopt;
  }
}

std::string utf8(const std::filesystem::path &path) {
  const auto value = path.u8string();
  return std::string(value.begin(), value.end());
}

nlohmann::json session_request(const Host &host) {
  nlohmann::json request{{"directory", utf8(host.directory)}, {"platform", host.platform}, {"version", host.version}};
  if (host.enabled)
    request["enabled"] = *host.enabled;
  else if (!host.preferences_directory.empty())
    request["preferences_directory"] = utf8(host.preferences_directory);
  else
    request["enabled"] = true;
  return request;
}

void disarm() { armed.store(false, std::memory_order_release); }

void arm(const std::string &path) {
  disarm();
#ifdef _WIN32
  const int length = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, path.data(), static_cast<int>(path.size()), record_path,
                                         static_cast<int>(std::size(record_path) - 1));
  if (length <= 0)
    return;
  record_path[length] = L'\0';
#else
  if (path.empty() || path.size() >= sizeof(record_path))
    return;
  std::memcpy(record_path, path.data(), path.size());
  record_path[path.size()] = '\0';
#endif
  armed.store(true, std::memory_order_release);
}

bool begin_locked(const Host &host) {
  if (host.directory.empty() || !host.directory.is_absolute())
    return false;
  const auto value = call(msime_client_telemetry_begin, session_request(host));
  if (!value || !value->is_object() || !value->value("enabled", false)) {
    disarm();
    return false;
  }
  const auto path = value->find("crash_record_path");
  if (path != value->end() && path->is_string())
    arm(path->get<std::string>());
  return true;
}

// ---- crash records written from signal and exception handlers ----

// Appends text to a fixed buffer; handlers must not allocate.
struct Buffer {
  char data[8192];
  size_t size = 0;
  void text(const char *value) {
    while (*value && size + 1 < sizeof(data))
      data[size++] = *value++;
  }
  void hex(uint64_t value) {
    char digits[17];
    int count = 0;
    do {
      digits[count++] = "0123456789abcdef"[value & 0xf];
      value >>= 4;
    } while (value && count < 16);
    text("0x");
    while (count && size + 1 < sizeof(data))
      data[size++] = digits[--count];
  }
  void decimal(int value) {
    char digits[12];
    int count = 0;
    unsigned magnitude = value < 0 ? 0u - static_cast<unsigned>(value) : static_cast<unsigned>(value);
    do {
      digits[count++] = static_cast<char>('0' + magnitude % 10);
      magnitude /= 10;
    } while (magnitude && count < 11);
    if (value < 0)
      text("-");
    while (count && size + 1 < sizeof(data))
      data[size++] = digits[--count];
  }
};

std::atomic<bool> handling{false};
#ifdef _WIN32
Buffer windows_record;
#endif

#ifdef _WIN32
// module.dll+0x1a2b for an address, the module reduced to its file name; <unknown>+0x... when no module owns it.
void windows_frame(Buffer &out, uint64_t address) {
  HMODULE module = nullptr;
  if (GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                         reinterpret_cast<LPCWSTR>(static_cast<uintptr_t>(address)), &module) &&
      module) {
    static wchar_t name[MAX_PATH];
    static char narrow[MAX_PATH * 3];
    const DWORD length = GetModuleFileNameW(module, name, MAX_PATH);
    if (length > 0 && length < MAX_PATH) {
      const wchar_t *file = name;
      for (const wchar_t *cursor = name; *cursor; ++cursor)
        if (*cursor == L'\\' || *cursor == L'/')
          file = cursor + 1;
      const int bytes = WideCharToMultiByte(CP_UTF8, 0, file, -1, narrow, static_cast<int>(sizeof(narrow)), nullptr, nullptr);
      if (bytes > 0) {
        out.text(narrow);
        out.text("+");
        out.hex(address - reinterpret_cast<uintptr_t>(module));
        return;
      }
    }
  }
  out.text("<unknown>+");
  out.hex(address);
}

const char *exception_name(DWORD code) {
  switch (code) {
  case EXCEPTION_ACCESS_VIOLATION: return "EXCEPTION_ACCESS_VIOLATION";
  case EXCEPTION_STACK_OVERFLOW: return "EXCEPTION_STACK_OVERFLOW";
  case EXCEPTION_ILLEGAL_INSTRUCTION: return "EXCEPTION_ILLEGAL_INSTRUCTION";
  case EXCEPTION_PRIV_INSTRUCTION: return "EXCEPTION_PRIV_INSTRUCTION";
  case EXCEPTION_IN_PAGE_ERROR: return "EXCEPTION_IN_PAGE_ERROR";
  case EXCEPTION_INT_DIVIDE_BY_ZERO: return "EXCEPTION_INT_DIVIDE_BY_ZERO";
  case EXCEPTION_INT_OVERFLOW: return "EXCEPTION_INT_OVERFLOW";
  case EXCEPTION_DATATYPE_MISALIGNMENT: return "EXCEPTION_DATATYPE_MISALIGNMENT";
  case EXCEPTION_ARRAY_BOUNDS_EXCEEDED: return "EXCEPTION_ARRAY_BOUNDS_EXCEEDED";
  case EXCEPTION_FLT_DIVIDE_BY_ZERO: return "EXCEPTION_FLT_DIVIDE_BY_ZERO";
  case EXCEPTION_FLT_INVALID_OPERATION: return "EXCEPTION_FLT_INVALID_OPERATION";
  case EXCEPTION_NONCONTINUABLE_EXCEPTION: return "EXCEPTION_NONCONTINUABLE_EXCEPTION";
  case 0xC0000374: return "STATUS_HEAP_CORRUPTION";
  case 0xE06D7363: return "C++ exception";
  default: return "EXCEPTION";
  }
}

void windows_write(HANDLE file, const Buffer &buffer) {
  DWORD written = 0;
  WriteFile(file, buffer.data, static_cast<DWORD>(buffer.size), &written, nullptr);
}

LPTOP_LEVEL_EXCEPTION_FILTER previous_filter = nullptr;
bool filter_installed = false;

// Writes the crash record and nothing else: no heap, no locks, no network. Buffers are static because a stack overflow leaves this filter little stack of its own.
LONG WINAPI unhandled_exception(EXCEPTION_POINTERS *pointers) {
  if (pointers && pointers->ExceptionRecord && armed.load(std::memory_order_acquire) && !handling.exchange(true)) {
    const HANDLE file = CreateFileW(record_path, GENERIC_WRITE, 0, nullptr, CREATE_NEW, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (file != INVALID_HANDLE_VALUE) {
      const auto *record = pointers->ExceptionRecord;
      const auto address = reinterpret_cast<uintptr_t>(record->ExceptionAddress);
      auto &out = windows_record;
      out.size = 0;
      out.text(exception_name(record->ExceptionCode));
      out.text(" (");
      out.hex(record->ExceptionCode);
      out.text(") in ");
      windows_frame(out, address);
      out.text("\n");
      // The summary and the faulting frame go to disk before the stack walk, which reads a stack that may itself be what broke.
      windows_frame(out, address);
      out.text("\n");
      windows_write(file, out);
      out.size = 0;
#if defined(_M_X64) || defined(__x86_64__)
      static CONTEXT context;
      context = *pointers->ContextRecord;
      for (int frame = 0; frame < 64 && context.Rip; ++frame) {
        DWORD64 image_base = 0;
        const auto function = RtlLookupFunctionEntry(context.Rip, &image_base, nullptr);
        if (!function) {
          // A leaf function: the return address is on top of the stack.
          context.Rip = *reinterpret_cast<const DWORD64 *>(context.Rsp);
          context.Rsp += 8;
        } else {
          PVOID handler_data = nullptr;
          DWORD64 establisher = 0;
          RtlVirtualUnwind(UNW_FLAG_NHANDLER, image_base, context.Rip, function, &context, &handler_data, &establisher, nullptr);
        }
        if (!context.Rip)
          break;
        windows_frame(out, context.Rip);
        out.text("\n");
        windows_write(file, out);
        out.size = 0;
      }
#endif
      FlushFileBuffers(file);
      CloseHandle(file);
    }
  }
  return previous_filter ? previous_filter(pointers) : EXCEPTION_CONTINUE_SEARCH;
}
#else
struct Signal {
  int number;
  const char *summary;
};
constexpr Signal crash_signals[] = {
    {SIGSEGV, "SIGSEGV: segmentation fault"}, {SIGBUS, "SIGBUS: bus error"},
    {SIGILL, "SIGILL: illegal instruction"},  {SIGFPE, "SIGFPE: arithmetic exception"},
    {SIGABRT, "SIGABRT: aborted"},
};
constexpr size_t crash_signal_count = sizeof(crash_signals) / sizeof(crash_signals[0]);
struct sigaction previous_actions[crash_signal_count];
bool actions_installed = false;
// Room for the handler on a thread whose own stack overflowed.
alignas(16) char alternate_stack[64 * 1024];

void write_all(int fd, const char *data, size_t size) {
  while (size) {
    const auto written = ::write(fd, data, size);
    if (written < 0 && errno == EINTR)
      continue;
    if (written <= 0)
      return;
    data += written;
    size -= static_cast<size_t>(written);
  }
}

// Only async-signal-safe calls: open, write, close, sigaction, raise, and glibc's backtrace (preloaded by install_crash_handlers) and backtrace_symbols_fd, which does not allocate.
void crash_signal(int number, siginfo_t *info, void *context) {
  const int saved = errno;
  size_t index = 0;
  while (index < crash_signal_count && crash_signals[index].number != number)
    ++index;
  if (index < crash_signal_count && armed.load(std::memory_order_acquire) && !handling.exchange(true)) {
    const int fd = ::open(record_path, O_WRONLY | O_CREAT | O_EXCL | O_CLOEXEC, 0600);
    if (fd >= 0) {
      Buffer summary;
      summary.text(crash_signals[index].summary);
      if (info && info->si_code > 0) {
        summary.text(" (code ");
        summary.decimal(info->si_code);
        summary.text(")");
      }
      summary.text("\n");
      write_all(fd, summary.data, summary.size);
#ifdef MSIME_TELEMETRY_BACKTRACE
      void *frames[64];
      const int count = backtrace(frames, 64);
      // Frame 0 is this handler; the signal trampoline in libc follows, then the code that faulted.
      if (count > 1)
        backtrace_symbols_fd(frames + 1, count - 1, fd);
#endif
      ::close(fd);
    }
  }
  // Hand the signal on as if this handler had never been there: an earlier handler (Fcitx5's own crash log) runs, and otherwise the default action ends the process once this returns.
  if (index < crash_signal_count) {
    const auto &previous = previous_actions[index];
    sigaction(number, &previous, nullptr);
    if ((previous.sa_flags & SA_SIGINFO) && previous.sa_sigaction) {
      previous.sa_sigaction(number, info, context);
    } else if (previous.sa_handler != SIG_DFL && previous.sa_handler != SIG_IGN && previous.sa_handler) {
      previous.sa_handler(number);
    } else {
      if (previous.sa_handler == SIG_IGN) {
        struct sigaction fallback {};
        fallback.sa_handler = SIG_DFL;
        sigemptyset(&fallback.sa_mask);
        sigaction(number, &fallback, nullptr);
      }
      // Blocked while this handler runs, so it is delivered with the default action right after it returns; a fault would recur on its own, an abort() or a kill would not.
      raise(number);
    }
  }
  errno = saved;
}
#endif

#ifndef _WIN32
#ifdef MSIME_TELEMETRY_DEMANGLE
std::string demangle(const char *name) {
  int status = 0;
  std::unique_ptr<char, decltype(&std::free)> readable(abi::__cxa_demangle(name, nullptr, nullptr, &status), std::free);
  return status == 0 && readable ? std::string(readable.get()) : std::string(name);
}
#else
std::string demangle(const char *name) { return name; }
#endif
#else
std::string demangle(const char *name) { return name; }
#endif
} // namespace

std::filesystem::path default_directory() {
#ifdef _WIN32
  // Read the wide value through Win32: getenv is deprecated under MSVC /WX and would pass the path through the ANSI code page, and MinGW's msvcrt import library has no _wdupenv_s.
  std::vector<wchar_t> base(32768);
  const DWORD length = GetEnvironmentVariableW(L"LOCALAPPDATA", base.data(), static_cast<DWORD>(base.size()));
  if (!length || length >= base.size())
    return {};
  return std::filesystem::path(std::wstring(base.data(), length)) / "MSIME";
#else
  if (const char *state = std::getenv("XDG_STATE_HOME"); state && *state == '/')
    return std::filesystem::path(state) / "msime";
  if (const char *home = std::getenv("HOME"); home && *home == '/')
    return std::filesystem::path(home) / ".local/state/msime";
  return {};
#endif
}

bool begin(const Host &host) {
  std::lock_guard guard(host_lock);
  current = host;
  configured = true;
  return begin_locked(current);
}

void end() {
  std::lock_guard guard(host_lock);
  if (!configured)
    return;
  disarm();
  call(msime_client_telemetry_end, nlohmann::json{{"directory", utf8(current.directory)}});
}

void flush() {
  nlohmann::json request;
  {
    std::lock_guard guard(host_lock);
    if (!configured || current.directory.empty())
      return;
    request = session_request(current);
  }
  const auto value = call(msime_client_telemetry_flush, request);
  // Reporting was turned off in the shared preferences while this host ran: the flush cleared the queue, marker and records, so a crash from now on writes nothing.
  if (value && value->is_object() && !value->value("enabled", true))
    disarm();
}

void start_flushing() {
  std::thread([] {
    for (;;) {
      flush();
      std::this_thread::sleep_for(std::chrono::minutes(30));
    }
  }).detach();
}

void set_enabled(bool enabled) {
  std::lock_guard guard(host_lock);
  if (!configured)
    return;
  const bool was = current.enabled.value_or(true);
  current.enabled = enabled;
  if (was == enabled)
    return;
  if (enabled) {
    begin_locked(current);
  } else {
    disarm();
    call(msime_client_telemetry_clear, nlohmann::json{{"directory", utf8(current.directory)}});
  }
}

std::string current_exception_summary() {
  const auto exception = std::current_exception();
  if (!exception)
    return "std::terminate without an active exception";
  try {
    std::rethrow_exception(exception);
  } catch (const nlohmann::json::exception &error) {
    // A JSON error's text quotes the bytes it failed on, which can come from the user's own files; the type and id say where it failed without them.
    return "std::terminate: " + demangle(typeid(error).name()) + " " + std::to_string(error.id);
  } catch (const std::exception &error) {
    std::string what = error.what();
    if (const auto line = what.find('\n'); line != std::string::npos)
      what.resize(line);
    return "std::terminate: " + demangle(typeid(error).name()) + ": " + what;
  } catch (...) {
    return "std::terminate: unknown exception";
  }
}

std::string current_stack(int skip) {
  std::ostringstream stack;
#ifdef _WIN32
  void *frames[62];
  const USHORT count = CaptureStackBackTrace(static_cast<DWORD>(skip + 1), 62, frames, nullptr);
  for (USHORT index = 0; index < count; ++index) {
    Buffer line;
    windows_frame(line, reinterpret_cast<uintptr_t>(frames[index]));
    stack.write(line.data, static_cast<std::streamsize>(line.size));
    stack << '\n';
  }
#elif defined(MSIME_TELEMETRY_BACKTRACE)
  void *frames[64];
  const int count = backtrace(frames, 64);
  std::unique_ptr<char *, decltype(&std::free)> symbols(backtrace_symbols(frames, count), std::free);
  for (int index = skip + 1; symbols && index < count; ++index)
    stack << symbols.get()[index] << '\n';
#else
  (void)skip;
#endif
  return stack.str();
}

void record_terminate() {
  try {
    if (!armed.load(std::memory_order_acquire))
      return;
    std::filesystem::path directory;
    {
      // try_lock: the thread that holds the lock may be the one terminating.
      std::unique_lock guard(host_lock, std::try_to_lock);
      if (!guard.owns_lock() || !configured)
        return;
      directory = current.directory;
    }
    call(msime_client_telemetry_record_crash,
         nlohmann::json{{"directory", utf8(directory)}, {"message", current_exception_summary()}, {"stack", current_stack(1)}});
  } catch (...) {
  }
}

void install_crash_handlers() {
#ifdef _WIN32
  if (filter_installed)
    return;
  previous_filter = SetUnhandledExceptionFilter(unhandled_exception);
  filter_installed = true;
#else
  if (actions_installed)
    return;
#ifdef MSIME_TELEMETRY_BACKTRACE
  // backtrace() loads libgcc on its first call, which allocates; do it here, never first inside a handler.
  void *warm[1];
  backtrace(warm, 1);
#endif
  stack_t current_stack_info{};
  if (sigaltstack(nullptr, &current_stack_info) == 0 && (current_stack_info.ss_flags & SS_DISABLE)) {
    stack_t alternate{};
    alternate.ss_sp = alternate_stack;
    alternate.ss_size = sizeof(alternate_stack);
    sigaltstack(&alternate, nullptr);
  }
  for (size_t index = 0; index < crash_signal_count; ++index) {
    struct sigaction action {};
    action.sa_sigaction = crash_signal;
    action.sa_flags = SA_SIGINFO | SA_ONSTACK;
    sigemptyset(&action.sa_mask);
    sigaction(crash_signals[index].number, &action, &previous_actions[index]);
  }
  actions_installed = true;
#endif
}

void remove_crash_handlers() {
  disarm();
#ifdef _WIN32
  if (!filter_installed)
    return;
  SetUnhandledExceptionFilter(previous_filter);
  filter_installed = false;
#else
  if (!actions_installed)
    return;
  for (size_t index = 0; index < crash_signal_count; ++index)
    sigaction(crash_signals[index].number, &previous_actions[index], nullptr);
  actions_installed = false;
#endif
}
}

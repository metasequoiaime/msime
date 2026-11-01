#include "CrashRecorder.h"
#include "PosixCrashFile.h"
#include <atomic>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <exception>
#include <iterator>
#include <memory>
#include <sstream>
#include <vector>
#ifdef _WIN32
#include <windows.h>
#else
#include <cerrno>
#include <csignal>
#include <fcntl.h>
#include <unistd.h>
#if defined(__ANDROID__)
#include <dlfcn.h>
#endif
#if defined(__linux__)
#include <ucontext.h>
#include <link.h>
#include <unwind.h>
#endif
#if defined(__APPLE__)
#include <mach-o/dyld.h>
#include <mach-o/loader.h>
#include <sys/ucontext.h>
#endif
#if !defined(__linux__) && __has_include(<execinfo.h>)
#include <execinfo.h>
#define MSIME_TELEMETRY_BACKTRACE 1
#endif
#endif

namespace msime::crash {
namespace {
// 正常路径准备好记录头和路径，处理器只读固定缓冲区；重配期间停用，处理器不等待配置锁。
std::atomic<bool> armed{false};
char record_header[1024];
size_t record_header_size = 0;
std::atomic_flag configuration_lock = ATOMIC_FLAG_INIT;

// 信号处理程序只能尝试一次，不能等待可能正是故障线程持有的配置锁。
struct ConfigurationGuard {
  bool owns = !configuration_lock.test_and_set(std::memory_order_acquire);
  ~ConfigurationGuard() { if (owns) configuration_lock.clear(std::memory_order_release); }
};
#ifdef _WIN32
wchar_t record_path[32768];
#else
char record_path[4096];
char temporary_record_path[4096];
#endif

// 信号与异常处理器使用的固定缓冲区。

// 追加到固定缓冲区，处理器不分配内存。
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

#if defined(__linux__) || defined(__APPLE__)
// 在正常启动时固定模块映射，信号路径不碰动态加载器的锁，也不读取完整路径。
struct Module {
  uintptr_t begin = 0;
  uintptr_t end = 0;
  uintptr_t base = 0;
  char name[128]{};
  char build_id[129]{};
};
// 鸿蒙应用的系统依赖可能超过 600 个；只在持配置锁的正常路径扩容，信号路径只读快照。
std::vector<Module> modules;

#if defined(__linux__)
int snapshot_module(dl_phdr_info *info, size_t, void *data) {
  auto &snapshot = *static_cast<std::vector<Module> *>(data);
  Module module;
  module.base = static_cast<uintptr_t>(info->dlpi_addr);
  const char *name = info->dlpi_name;
  if (!name || !*name) name = "<executable>";
  for (const char *cursor = name; *cursor; ++cursor) if (*cursor == '/') name = cursor + 1;
  std::strncpy(module.name, name, sizeof(module.name) - 1);
  for (size_t index = 0; index < info->dlpi_phnum; ++index) {
    const auto &header = info->dlpi_phdr[index];
    if (header.p_type == PT_LOAD && (header.p_flags & PF_X)) {
      const auto begin = module.base + header.p_vaddr;
      if (!module.begin || begin < module.begin) module.begin = begin;
      if (begin + header.p_memsz > module.end) module.end = begin + header.p_memsz;
    }
    if (header.p_type != PT_NOTE) continue;
    // 某些 Android 系统模块的 NOTE 不在映射里，不能直接解引用文件虚拟地址。
    bool readable = false;
    for (size_t load_index = 0; load_index < info->dlpi_phnum; ++load_index) {
      const auto &load = info->dlpi_phdr[load_index];
      if (load.p_type == PT_LOAD && (load.p_flags & PF_R) && header.p_vaddr >= load.p_vaddr
          && header.p_vaddr - load.p_vaddr <= load.p_memsz
          && header.p_memsz <= load.p_memsz - (header.p_vaddr - load.p_vaddr)) readable = true;
    }
    if (!readable) continue;
    const auto *cursor = reinterpret_cast<const unsigned char *>(module.base + header.p_vaddr);
    size_t remaining = header.p_memsz;
    while (remaining >= sizeof(ElfW(Nhdr))) {
      ElfW(Nhdr) note;
      std::memcpy(&note, cursor, sizeof(note));
      cursor += sizeof(note);
      remaining -= sizeof(note);
      const auto name_size = (static_cast<size_t>(note.n_namesz) + 3) & ~size_t(3);
      const auto data_size = (static_cast<size_t>(note.n_descsz) + 3) & ~size_t(3);
      if (name_size > remaining || data_size > remaining - name_size) break;
      if (note.n_type == NT_GNU_BUILD_ID && note.n_namesz == 4 && std::memcmp(cursor, "GNU\0", 4) == 0) {
        const auto *data = cursor + name_size;
        const size_t bytes = note.n_descsz < 64 ? note.n_descsz : 64;
        for (size_t byte = 0; byte < bytes; ++byte) {
          module.build_id[byte * 2] = "0123456789abcdef"[data[byte] >> 4];
          module.build_id[byte * 2 + 1] = "0123456789abcdef"[data[byte] & 15];
        }
      }
      cursor += name_size + data_size;
      remaining -= name_size + data_size;
    }
  }
  if (module.begin < module.end) {
    // 不能让分配异常穿过动态加载器的 C 回调；枚举不完整时让 arm 保持停用。
    try { snapshot.push_back(module); } catch (...) { return 1; }
  }
  return 0;
}

#else
void snapshot_mach_modules(std::vector<Module> &snapshot) {
  for (uint32_t index = 0; index < _dyld_image_count(); ++index) {
    const auto *image = _dyld_get_image_header(index);
    if (!image || image->magic != MH_MAGIC_64) continue;
    const auto *header = reinterpret_cast<const mach_header_64 *>(image);
    Module module;
    module.base = reinterpret_cast<uintptr_t>(image);
    const char *name = _dyld_get_image_name(index);
    if (!name) name = "<executable>";
    for (const char *cursor = name; *cursor; ++cursor) if (*cursor == '/') name = cursor + 1;
    std::strncpy(module.name, name, sizeof(module.name) - 1);
    const auto slide = _dyld_get_image_vmaddr_slide(index);
    const auto *cursor = reinterpret_cast<const unsigned char *>(header + 1);
    size_t remaining = header->sizeofcmds;
    for (uint32_t command = 0; command < header->ncmds && remaining >= sizeof(load_command); ++command) {
      const auto *load = reinterpret_cast<const load_command *>(cursor);
      if (load->cmdsize < sizeof(load_command) || load->cmdsize > remaining) break;
      if (load->cmd == LC_SEGMENT_64 && load->cmdsize >= sizeof(segment_command_64)) {
        const auto *segment = reinterpret_cast<const segment_command_64 *>(load);
        if (segment->initprot & 4) {
          const auto begin = static_cast<uintptr_t>(segment->vmaddr + slide);
          if (!module.begin || begin < module.begin) module.begin = begin;
          if (begin + segment->vmsize > module.end) module.end = begin + segment->vmsize;
        }
      }
      if (load->cmd == LC_UUID && load->cmdsize >= sizeof(uuid_command)) {
        const auto *uuid = reinterpret_cast<const uuid_command *>(load);
        for (size_t byte = 0; byte < sizeof(uuid->uuid); ++byte) {
          module.build_id[byte * 2] = "0123456789abcdef"[uuid->uuid[byte] >> 4];
          module.build_id[byte * 2 + 1] = "0123456789abcdef"[uuid->uuid[byte] & 15];
        }
      }
      cursor += load->cmdsize;
      remaining -= load->cmdsize;
    }
    if (module.begin < module.end) snapshot.push_back(module);
  }
}
#endif

// 只在持配置锁的正常路径调用；信号处理器拿不到配置锁时不读快照。
bool snapshot_modules(std::vector<Module> &snapshot) {
  snapshot.clear();
#if defined(__linux__)
  return dl_iterate_phdr(snapshot_module, &snapshot) == 0;
#else
  snapshot_mach_modules(snapshot);
  return true;
#endif
}

void posix_frame(Buffer &out, uintptr_t address) {
  for (const auto &module : modules) {
    if (address < module.begin || address >= module.end) continue;
    out.text(module.name);
    out.text("+");
    out.hex(address - module.base);
    if (*module.build_id) {
      out.text(" [id 0x");
      out.text(module.build_id);
      out.text("]");
    }
    out.text("\n");
    return;
  }
  out.text("<unknown>+");
  out.hex(address);
  out.text("\n");
}

#if defined(__linux__)
struct UnwindFrames { uintptr_t frames[64]; size_t count = 0; };
_Unwind_Reason_Code collect_frame(_Unwind_Context *context, void *raw) {
  auto &frames = *static_cast<UnwindFrames *>(raw);
  const auto address = static_cast<uintptr_t>(_Unwind_GetIP(context));
  if (address && frames.count < std::size(frames.frames)) frames.frames[frames.count++] = address;
  return frames.count == std::size(frames.frames) ? _URC_END_OF_STACK : _URC_NO_REASON;
}
#endif

#endif

std::atomic<bool> handling{false};
#ifdef _WIN32
Buffer windows_record;
#endif

#ifdef _WIN32
// 模块只保留文件名和相对偏移，无法定位时标为 unknown。
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

// 只落盘，不分配堆内存、不等待锁、不联网；静态缓冲区为栈溢出保留处理空间。
LONG WINAPI unhandled_exception(EXCEPTION_POINTERS *pointers) {
  ConfigurationGuard configuration;
  if (configuration.owns && pointers && pointers->ExceptionRecord && armed.load(std::memory_order_acquire) && !handling.exchange(true)) {
    const HANDLE file = CreateFileW(record_path, GENERIC_WRITE, 0, nullptr, CREATE_NEW, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (file != INVALID_HANDLE_VALUE) {
      DWORD header_written = 0;
      WriteFile(file, record_header, static_cast<DWORD>(record_header_size), &header_written, nullptr);
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
      // 先保存摘要和故障帧，避免展开损坏栈时二次故障丢失直接位置。
      windows_frame(out, address);
      out.text("\n");
      windows_write(file, out);
      out.size = 0;
#if defined(_M_X64) || defined(__x86_64__)
      static CONTEXT context;
      context = pointers->ContextRecord ? *pointers->ContextRecord : CONTEXT{};
      for (int frame = 0; frame < 64 && context.Rip; ++frame) {
        DWORD64 image_base = 0;
        const auto function = RtlLookupFunctionEntry(context.Rip, &image_base, nullptr);
        if (!function) {
          // 叶函数的返回地址位于栈顶，通过系统 API 读取。
          DWORD64 next = 0;
          SIZE_T read = 0;
          if (!ReadProcessMemory(GetCurrentProcess(), reinterpret_cast<const void *>(context.Rsp), &next, sizeof(next), &read) || read != sizeof(next))
            break;
          context.Rip = next;
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
#elif defined(_M_IX86) || defined(__i386__)
      DWORD pointer = pointers->ContextRecord ? pointers->ContextRecord->Ebp : 0;
      for (int frame = 0; frame < 64 && pointer; ++frame) {
        struct { DWORD previous; DWORD address; } next{};
        SIZE_T read = 0;
        if (!ReadProcessMemory(GetCurrentProcess(), reinterpret_cast<const void *>(static_cast<uintptr_t>(pointer)), &next, sizeof(next), &read) || read != sizeof(next))
          break;
        if (!next.address)
          break;
        windows_frame(out, next.address);
        out.text("\n");
        windows_write(file, out);
        out.size = 0;
        // 损坏、回环或跨出合理范围的链立即停止，不直接解引用故障栈。
        if (next.previous <= pointer || next.previous - pointer > 1024 * 1024 || next.previous % sizeof(DWORD))
          break;
        pointer = next.previous;
      }
#elif defined(_M_ARM64) || defined(__aarch64__)
      static CONTEXT context;
      context = pointers->ContextRecord ? *pointers->ContextRecord : CONTEXT{};
      for (int frame = 0; frame < 64 && context.Pc; ++frame) {
        DWORD64 image_base = 0;
        const auto function = RtlLookupFunctionEntry(context.Pc, &image_base, nullptr);
        const auto previous = context.Pc;
        if (!function) {
          context.Pc = context.Lr;
          context.Lr = 0;
        } else {
          PVOID handler_data = nullptr;
          DWORD64 establisher = 0;
          RtlVirtualUnwind(UNW_FLAG_NHANDLER, image_base, context.Pc, function, &context, &handler_data, &establisher, nullptr);
        }
        if (!context.Pc || context.Pc == previous)
          break;
        windows_frame(out, context.Pc);
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
    {SIGABRT, "SIGABRT: aborted"}, {SIGTRAP, "SIGTRAP: trace trap"},
};
constexpr size_t crash_signal_count = sizeof(crash_signals) / sizeof(crash_signals[0]);
struct sigaction previous_actions[crash_signal_count];
bool actions_installed = false;
#if defined(__ANDROID__)
using SignalActionQuery = int (*)(int, const struct sigaction *, struct sigaction *);
SignalActionQuery kernel_signal_action = nullptr;
void *signal_libc = nullptr;
#endif
// 为安装线程上的栈溢出预留信号栈。
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

// 发信号的进程名（Linux 的 `/proc/<pid>/comm`，至多 15 个字节）追加到 `out`；读不到或不是可打印 ASCII 时什么也不写，返回 false。只用 open、read、close，可以在信号处理函数里调用。
bool append_sender_name(Buffer &out, pid_t pid) {
#ifdef __linux__
  char path[32] = "/proc/";
  size_t length = 6;
  char digits[12];
  int count = 0;
  auto value = static_cast<unsigned long>(pid);
  do {
    digits[count++] = static_cast<char>('0' + value % 10);
    value /= 10;
  } while (value && count < 11);
  while (count)
    path[length++] = digits[--count];
  for (const char *suffix = "/comm"; *suffix;)
    path[length++] = *suffix++;
  path[length] = '\0';
  const int fd = ::open(path, O_RDONLY | O_CLOEXEC);
  if (fd < 0)
    return false;
  char name[17];
  const auto got = ::read(fd, name, 16);
  ::close(fd);
  if (got <= 0)
    return false;
  size_t size = static_cast<size_t>(got);
  while (size && (name[size - 1] == '\n' || name[size - 1] == ' '))
    --size;
  if (!size)
    return false;
  for (size_t index = 0; index < size; ++index) {
    if (name[index] < 0x20 || name[index] > 0x7e)
      return false;
  }
  name[size] = '\0';
  out.text(name);
  return true;
#else
  (void)out;
  (void)pid;
  return false;
#endif
}

// 信号是别的进程用 kill、sigqueue 发来的，或者本进程的某个线程用 raise、tgkill 发出的，而不是执行时出的错（那时 si_code 为正）。
bool sent_by_a_process(const siginfo_t *info) {
  if (!info)
    return false;
#ifdef SI_TKILL
  if (info->si_code == SI_TKILL)
    return true;
#endif
  return info->si_code == SI_USER || info->si_code == SI_QUEUE;
}

bool same_signal_action(const struct sigaction &left, const struct sigaction &right) {
  if (left.sa_flags != right.sa_flags)
    return false;
  if (left.sa_flags & SA_SIGINFO) {
    if (left.sa_sigaction != right.sa_sigaction) return false;
  } else if (left.sa_handler != right.sa_handler) {
    return false;
  }
  for (int number = 1; number < NSIG; ++number) {
    if (sigismember(&left.sa_mask, number) != sigismember(&right.sa_mask, number))
      return false;
  }
  return true;
}

bool default_signal_pending(int number) {
  sigset_t pending{};
  if (sigpending(&pending) != 0 || sigismember(&pending, number) != 1)
    return false;
  struct sigaction action{};
#if defined(__ANDROID__)
  // ART 的逻辑回调可能仍在，debuggerd 却已把内核处置改成默认；只查询预先解析的 libc 入口。
  return kernel_signal_action && kernel_signal_action(number, nullptr, &action) == 0
      && action.sa_handler == SIG_DFL;
#else
  return sigaction(number, nullptr, &action) == 0 && action.sa_handler == SIG_DFL;
#endif
}

// 先用信号安全的写入保存摘要，再尽力展开预热过的运行时栈；展开器仍可能受损坏栈影响。
void crash_signal(int number, siginfo_t *info, void *context) {
  const int saved = errno;
  size_t index = 0;
  while (index < crash_signal_count && crash_signals[index].number != number)
    ++index;
  ConfigurationGuard configuration;
  int fd = -1;
  const bool owns_handling = configuration.owns && index < crash_signal_count
      && armed.load(std::memory_order_acquire) && !handling.exchange(true);
  if (owns_handling) {
    fd = msime_crash_file_create(record_path, temporary_record_path);
    if (fd >= 0) {
      write_all(fd, record_header, record_header_size);
      // 静态缓冲区减少信号栈占用；handling 保证只由一个崩溃处理器使用。
      static Buffer summary;
      summary.size = 0;
      summary.text(crash_signals[index].summary);
      if (info && info->si_code > 0) {
        summary.text(" (code ");
        summary.decimal(info->si_code);
        summary.text(")");
      } else if (sent_by_a_process(info)) {
        // 区分同进程 raise、tgkill 与外部发送的信号；来源信息本身不能排除宿主或依赖库的上游缺陷。只记录进程名，不写 pid，服务端分组会掩去数字。
        if (info->si_pid == getpid()) {
          summary.text(" (raised by this process)");
        } else {
          summary.text(" (sent by ");
          if (!append_sender_name(summary, info->si_pid))
            summary.text("another process");
          summary.text(")");
        }
      }
      summary.text("\n");
      write_all(fd, summary.data, summary.size);
#if defined(__linux__) || defined(__APPLE__)
      // 先保存故障指令，再尽力展开；损坏的栈即使展开失败，也保留直接故障位置。
      auto *machine = static_cast<ucontext_t *>(context);
      uintptr_t pc = 0;
      if (machine) {
#if defined(__APPLE__) && defined(__aarch64__)
        if (machine->uc_mcontext) pc = __darwin_arm_thread_state64_get_pc(machine->uc_mcontext->__ss);
#elif defined(__APPLE__) && defined(__x86_64__)
        if (machine->uc_mcontext) pc = machine->uc_mcontext->__ss.__rip;
#elif defined(__linux__) && defined(__aarch64__)
        pc = machine->uc_mcontext.pc;
#elif defined(__linux__) && defined(__x86_64__)
        pc = machine->uc_mcontext.gregs[REG_RIP];
#elif defined(__linux__) && defined(__i386__)
        pc = machine->uc_mcontext.gregs[REG_EIP];
#elif defined(__linux__) && defined(__arm__)
        pc = machine->uc_mcontext.arm_pc;
#endif
      }
      static Buffer frame;
      frame.size = 0;
      if (pc) { posix_frame(frame, pc); write_all(fd, frame.data, frame.size); }
#endif
#if defined(__linux__)
      static UnwindFrames frames;
      frames.count = 0;
      _Unwind_Backtrace(collect_frame, &frames);
      for (size_t item = 1; item < frames.count; ++item) {
        frame.size = 0;
        posix_frame(frame, frames.frames[item]);
        write_all(fd, frame.data, frame.size);
      }
#elif defined(MSIME_TELEMETRY_BACKTRACE)
      static void *frames[64];
      const int count = backtrace(frames, 64);
      // 使用预先固定的模块偏移和 UUID，不把设备路径写进报告。
      for (int item = 1; item < count; ++item) {
        static Buffer frame;
        frame.size = 0;
        posix_frame(frame, reinterpret_cast<uintptr_t>(frames[item]));
        write_all(fd, frame.data, frame.size);
      }
#endif
      (void)::fsync(fd);
    }
  }
  // 恢复既有处理器（包括 Fcitx5 自带的崩溃日志），否则交给系统默认动作。
  if (index < crash_signal_count) {
    const auto previous = previous_actions[index];
    struct sigaction capture_action {};
    const bool has_capture_action = sigaction(number, nullptr, &capture_action) == 0
        && (capture_action.sa_flags & SA_SIGINFO) && capture_action.sa_sigaction == crash_signal;
    auto disposition = previous;
    if (previous.sa_flags & SA_RESETHAND) {
      // 直接委托不会由内核重置一次性回调，先恢复默认处置再调用原回调。
      disposition = {};
      disposition.sa_handler = SIG_DFL;
      sigemptyset(&disposition.sa_mask);
    }
    sigaction(number, &disposition, nullptr);
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
      // 当前信号在处理期间被屏蔽，返回后按默认动作送达；主动 raise 的信号也需要重新发送。
      raise(number);
      // 默认动作在回调返回后才结束进程；文件锁必须由内核随进程退出释放。
      errno = saved;
      return;
    }
    if (default_signal_pending(number)) {
      // 前任恢复默认并重新投递后也会返回；保留原 inode 和锁，由致命退出释放，不能先删记录。
      errno = saved;
      return;
    }
    // 没有待送达的默认致命动作才视为恢复；持锁撤掉临时记录，不能抢先被另一进程补报。
    if (fd >= 0) {
      // 扫描器可能已打开旧 inode、尚未尝试文件锁；清空后再删名，旧描述符也读不到误报。
      (void)!ftruncate(fd, 0);
      (void)unlink(record_path);
      ::close(fd);
      fd = -1;
    }
    // 前任可能恢复默认处置后等故障指令重试；更新下一次的委托，不能因此丢掉后续采集。
    struct sigaction after {};
    if (has_capture_action && sigaction(number, nullptr, &after) == 0
        && !same_signal_action(capture_action, after)) {
      previous_actions[index] = after;
      sigaction(number, &capture_action, nullptr);
    }
  }
  if (fd >= 0) ::close(fd);
  if (owns_handling) handling.store(false);
  errno = saved;
}
#endif

}

void disarm() { armed.store(false, std::memory_order_release); }

void arm(const std::string &path, const std::string &header) {
  disarm();
  ConfigurationGuard configuration;
  if (!configuration.owns)
    return;
  if (header.size() >= sizeof(record_header))
    return;
  std::memcpy(record_header, header.data(), header.size());
  record_header_size = header.size();
#if defined(__linux__) || defined(__APPLE__)
  if (!snapshot_modules(modules)) return;
#endif
#ifdef _WIN32
  const int length = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, path.data(), static_cast<int>(path.size()), record_path,
                                         static_cast<int>(std::size(record_path) - 1));
  if (length <= 0)
    return;
  record_path[length] = L'\0';
#else
  constexpr char temporary_suffix[] = ".writing";
  if (path.empty() || path.size() + sizeof(temporary_suffix) > sizeof(record_path))
    return;
  std::memcpy(record_path, path.data(), path.size());
  record_path[path.size()] = '\0';
  std::memcpy(temporary_record_path, path.data(), path.size());
  std::memcpy(temporary_record_path + path.size(), temporary_suffix, sizeof(temporary_suffix));
#endif
  armed.store(true, std::memory_order_release);
}

bool is_armed() { return armed.load(std::memory_order_acquire); }

void refresh_modules() {
#if defined(__linux__) || defined(__APPLE__)
  ConfigurationGuard configuration;
  if (!configuration.owns)
    return;
  // 先在旁边建好新快照再交换；枚举失败时保留旧快照，不因此停采。
  std::vector<Module> next;
  if (snapshot_modules(next))
    modules.swap(next);
#endif
}

void record(const char *summary) {
  if (!summary || !is_armed())
    return;
  // 先在普通异常回调里收集栈，再占用配置；信号路径不分配这些字符串。
  const auto stack = current_stack(1);
  ConfigurationGuard configuration;
  if (!configuration.owns || !is_armed())
    return;
#ifdef _WIN32
  const HANDLE file = CreateFileW(record_path, GENERIC_WRITE, 0, nullptr, CREATE_NEW, FILE_ATTRIBUTE_NORMAL, nullptr);
  if (file == INVALID_HANDLE_VALUE)
    return;
  DWORD written = 0;
  for (const auto &part : {std::string(record_header, record_header_size), std::string(summary), std::string("\n"), stack})
    WriteFile(file, part.data(), static_cast<DWORD>(part.size()), &written, nullptr);
  FlushFileBuffers(file);
  CloseHandle(file);
#else
  const int file = msime_crash_file_create(record_path, temporary_record_path);
  if (file < 0)
    return;
  write_all(file, record_header, record_header_size);
  write_all(file, summary, std::strlen(summary));
  write_all(file, "\n", 1);
  write_all(file, stack.data(), stack.size());
  (void)::fsync(file);
  ::close(file);
#endif
}

std::string current_stack(int skip) {
  ConfigurationGuard configuration;
  if (!configuration.owns) return {};
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
#elif defined(__linux__)
  UnwindFrames frames;
  _Unwind_Backtrace(collect_frame, &frames);
  for (size_t index = static_cast<size_t>(skip + 1); index < frames.count; ++index) {
    Buffer line;
    posix_frame(line, frames.frames[index]);
    stack.write(line.data, static_cast<std::streamsize>(line.size));
  }
#elif defined(MSIME_TELEMETRY_BACKTRACE)
  void *frames[64];
  const int count = backtrace(frames, 64);
  for (int index = skip + 1; index < count; ++index) {
    Buffer line;
    posix_frame(line, reinterpret_cast<uintptr_t>(frames[index]));
    stack.write(line.data, static_cast<std::streamsize>(line.size));
  }
#else
  (void)skip;
#endif
  return stack.str();
}

extern "C" void msime_native_crash_arm(const char *path, const char *header) {
  if (!path || !header) { msime::crash::disarm(); return; }
  try { msime::crash::arm(path, header); } catch (...) { msime::crash::disarm(); }
}
extern "C" void msime_native_crash_disarm(void) { msime::crash::disarm(); }
extern "C" void msime_native_crash_install(void) { msime::crash::install_crash_handlers(); }
extern "C" void msime_native_crash_refresh_modules(void) {
  try { msime::crash::refresh_modules(); } catch (...) {}
}
extern "C" void msime_native_crash_record(const char *summary) {
  try { msime::crash::record(summary); } catch (...) {}
}

void install_crash_handlers() {
#ifdef _WIN32
  if (filter_installed)
    return;
  previous_filter = SetUnhandledExceptionFilter(unhandled_exception);
  filter_installed = true;
  SetEnvironmentVariableW(in_process_recorder_variable, std::to_wstring(GetCurrentProcessId()).c_str());
#else
  if (actions_installed)
    return;
#if defined(__ANDROID__)
  // 加载器调用只发生在正常安装路径，保留库引用直到进程结束，信号中不解析符号。
  if (!signal_libc) {
    signal_libc = dlopen("libc.so", RTLD_NOW | RTLD_LOCAL);
    if (!signal_libc)
      return;
    kernel_signal_action = reinterpret_cast<SignalActionQuery>(dlsym(signal_libc, "sigaction"));
    if (!kernel_signal_action) {
      dlclose(signal_libc);
      signal_libc = nullptr;
      return;
    }
  }
#endif
#if defined(__linux__)
  UnwindFrames warm;
  _Unwind_Backtrace(collect_frame, &warm);
#endif
#ifdef MSIME_TELEMETRY_BACKTRACE
  // 首次调用可能加载展开器并分配内存，必须在正常执行路径预热。
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
  SetEnvironmentVariableW(in_process_recorder_variable, nullptr);
#else
  if (!actions_installed)
    return;
  for (size_t index = 0; index < crash_signal_count; ++index)
    sigaction(crash_signals[index].number, &previous_actions[index], nullptr);
  actions_installed = false;
#endif
}
}

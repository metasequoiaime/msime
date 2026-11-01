#pragma once
#include <cstddef>
#include <cstdint>
#include <type_traits>

namespace msime::tsf::wer {
constexpr std::uint32_t context_magic = 0x4d534357;
constexpr std::size_t path_capacity = 1024;
constexpr std::size_t header_capacity = 4096;
struct alignas(8) Module {
  std::uint64_t base = 0;
  std::uint64_t size = 0;
  char name[64]{};
};
// 只含固定宽度字段；pContext 是目标进程地址，helper 必须通过 ReadProcessMemory 读取。
struct alignas(8) Context {
  std::uint32_t sequence = 0;
  std::uint32_t armed = 0;
  std::uint32_t magic = context_magic;
  std::uint32_t size = sizeof(Context);
  char16_t path[path_capacity]{};
  char header[header_capacity]{};
  Module modules[2]{};
  // 1 表示同一进程装了进程内记录器（水杉自己的设置、面板等），能到达未处理异常过滤器的故障由它记下，helper 只补记绕过过滤器的快速失败。
  std::uint32_t in_process_recorder = 0;
};
static_assert(std::is_trivially_copyable_v<Context>);
static_assert(sizeof(Module) == 80 && sizeof(Context) == 6328);
static_assert(offsetof(Context, in_process_recorder) == 6320);
static_assert(offsetof(Context, armed) == 4);
static_assert(offsetof(Context, modules) % 8 == 0);
}

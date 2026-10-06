#include "ComponentFailure.h"
#include <cassert>
#include <string>
#include <vector>

int main() {
  using namespace msime::windows;
  assert(describe_failure_site(failure_at_stage("refresh", 5)) == "refresh, error 5");
  assert(describe_failure_site(failure_in_message(0x000F, 0)) == "window message 0x000F, error 0");
  // 消息号超过四位时照实写出，不截断。
  assert(describe_failure_site(failure_in_message(0xC0DE1, 1400)) == "window message 0xC0DE1, error 1400");
  assert(component_failure("candidate window", failure_in_message(0x000F, 0)) ==
         "candidate window failed (window message 0x000F, error 0)");
  // 工作线程没有失败位置，只报名字。
  assert(component_failure("candidate paging", std::nullopt) == "candidate paging failed");
  assert(server_stop_line({}) == "Server stopping");
  assert(server_stop_line({"candidate paging failed"}) == "Server stopping: candidate paging failed");
  assert(server_stop_line({"session controller failed (input queue)", "candidate paging failed"}) ==
         "Server stopping: session controller failed (input queue); candidate paging failed");
}

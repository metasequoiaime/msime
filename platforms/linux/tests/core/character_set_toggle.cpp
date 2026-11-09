#include "core/CharacterSetTogglePolicy.h"

#include <cstdlib>
#include <iostream>
#include <string_view>

namespace {
void require(bool condition, const char *message) {
  if (condition) return;
  std::cerr << message << '\n';
  std::exit(1);
}
} // 匿名命名空间

int main() {
  using namespace msime::linux_host;
  require(character_set::follows_cantonese(scheme::Cantonese),
          "粤拼应使用专用字符集偏好");
  require(!character_set::follows_cantonese(scheme::Quanpin),
          "全拼不应使用粤拼字符集偏好");
  require(std::string_view(character_set::preference_key(scheme::Cantonese)) ==
              "cantonese_simplified_output",
          "粤拼应写入简体输出偏好");
  require(character_set::checked(scheme::Cantonese, false, true),
          "粤拼简体输出开启时应显示已选中");
  require(!character_set::checked(scheme::Cantonese, true, false),
          "粤拼不应读取繁体输出偏好");
  require(character_set::checked(scheme::Quanpin, true, false),
          "全拼应读取繁体输出偏好");
  return 0;
}

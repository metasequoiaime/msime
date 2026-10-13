#include "../src/candidates/CandidateWheelPaging.h"

#include <cassert>

int main() {
  using msime::linux_host::CandidateWheelPagingSync;
  using msime::linux_host::read_candidate_wheel_paging;
  using Json = nlohmann::json;

  assert(!read_candidate_wheel_paging(Json()));
  assert(!read_candidate_wheel_paging(Json::object()));
  assert(!read_candidate_wheel_paging({{"navigation", {{"mouse_wheel", "yes"}}}}));
  assert(!read_candidate_wheel_paging({{"navigation", {{"mouse_wheel", false}}}}));
  assert(read_candidate_wheel_paging({{"navigation", {{"mouse_wheel", true}}}}));

  // The untouched default leaves the desktop's value; later changes are written once each.
  CandidateWheelPagingSync untouched;
  assert(!untouched.next(false));
  assert(!untouched.next(false));
  assert(untouched.next(true) == true);
  untouched.mark_applied(true);
  assert(!untouched.next(true));
  assert(untouched.next(false) == false);
  untouched.mark_applied(false);
  assert(!untouched.next(false));

  // A preference already enabled when the addon starts is written straight away.
  CandidateWheelPagingSync enabled;
  assert(enabled.next(true) == true);
  // classicui 缺席时没有写入配置，相同偏好必须继续返回待写值。
  assert(enabled.next(true) == true);
  assert(enabled.pending());
  enabled.mark_applied(true);
  assert(!enabled.pending());
  assert(!enabled.next(true));
  // 用户在首次写入失败后改回默认值，不必接管桌面配置。
  CandidateWheelPagingSync cancelled;
  assert(cancelled.next(true) == true);
  assert(!cancelled.next(false));
  assert(!cancelled.pending());
  return 0;
}

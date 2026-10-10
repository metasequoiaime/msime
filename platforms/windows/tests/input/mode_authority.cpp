#include "ModeAuthority.h"
#include <iostream>
#include <stdexcept>
#include <string>

using namespace msime::windows;
namespace {
[[noreturn]] void require_failed(int line) {
  throw std::runtime_error("Mode authority test failed at line " +
                           std::to_string(line));
}
#define require(value)                                                         \
  do {                                                                         \
    if (!(value))                                                              \
      require_failed(__LINE__);                                                \
  } while (false)
// 测试里的客户端号和 TSF 一样是 pid << 32 | tid，进程号就是传给 mode_authority_step 的 app。
constexpr uint64_t client_of(uint32_t app) { return (uint64_t{app} << 32) | 7; }
} // namespace
int main() {
  try {
    // Seeding: the first observation adopts the client's mode rather than
    // fighting it with an authority that was never set.
    ModeAuthorityState state;
    state.seeded = false;
    auto step = mode_authority_step(state, true, true, client_of(0), 1, false);
    require(!step.push);
    require(step.next.seeded && !step.next.chinese && step.next.session == 1);
    state = step.next;

    // Same client changing its own mode: the user meant it, so it becomes the
    // authority and will travel to the next application.
    step = mode_authority_step(state, true, true, client_of(0), 1, true);
    require(!step.push);
    require(step.next.chinese);
    state = step.next;

    // A different client takes focus reporting the other mode: the authority
    // wins and is pushed back to it.
    step = mode_authority_step(state, true, true, client_of(0), 2, false);
    require(step.push && step.push_chinese);
    require(step.next.session == 2);
    // The authority itself does not change on a push.
    require(step.next.chinese);

    // A client that already agrees is left alone: pushing would be a pointless
    // round trip through the pipe on every focus change.
    step = mode_authority_step(step.next, true, true, client_of(0), 3, true);
    require(!step.push);
    require(step.next.session == 3);

    // Per-application scope never pushes, whatever the mismatch. That is
    // exactly what the 按应用记忆 option promises.
    ModeAuthorityState per_app;
    per_app.seeded = true;
    per_app.chinese = true;
    per_app.session = 1;
    auto app_step = mode_authority_step(per_app, false, true, client_of(0), 2, false);
    require(!app_step.push);
    require(app_step.next.session == 2 && !app_step.next.chinese);

    // Nothing focused: the authority survives and nothing is pushed, so a
    // moment with no client does not reset the user's mode.
    auto idle = mode_authority_step(state, true, false, client_of(0), 0, false);
    require(!idle.push);
    require(idle.next.chinese == state.chinese);
    require(idle.next.session == state.session);
    require(idle.next.seeded);

    // 推送落地时客户端在同一会话里报告新模式：那是 Server 推的，不算用户切换，也不显示中英文切换提示。
    ModeAuthorityState pushed;
    pushed.seeded = true;
    pushed.chinese = true;
    pushed.session = 1;
    pushed.reported = true;
    step = mode_authority_step(pushed, true, true, client_of(20), 2, false, 20);
    require(step.push && step.push_chinese && step.next.pushed == true);
    step = mode_authority_step(step.next, true, true, client_of(20), 2, true, 20);
    require(!step.push && !step.user_changed && !step.next.pushed);
    // 之后用户在同一会话里切换，才是用户的选择。
    step = mode_authority_step(step.next, true, true, client_of(20), 2, false, 20);
    require(step.user_changed && !step.next.chinese);
    // 同一会话、模式没变，什么也不发生。
    auto same = mode_authority_step(step.next, true, true, client_of(20), 2, false, 20);
    require(!same.push && !same.user_changed);

    // 应用例外：焦点进入有规则的应用时推规则里的模式，全局状态不变。
    ModeAuthorityState ruled;
    ruled.seeded = true;
    ruled.chinese = true;
    ruled.session = 1;
    ruled.app = 10;
    ruled.reported = true;
    auto rule_step = mode_authority_step(ruled, true, true, client_of(11), 2, true, 11, false);
    require(rule_step.push && !rule_step.push_chinese);
    require(rule_step.next.chinese && rule_step.next.app == 11);
    // 推送落地。
    rule_step = mode_authority_step(rule_step.next, true, true, client_of(11), 2, false, 11, false);
    require(!rule_step.user_changed && !rule_step.push);
    // 同一应用的另一个输入框不是新的停留，规则照样适用。
    rule_step = mode_authority_step(rule_step.next, true, true, client_of(11), 3, true, 11, false);
    require(rule_step.push && !rule_step.push_chinese);
    rule_step = mode_authority_step(rule_step.next, true, true, client_of(11), 3, false, 11, false);
    // 用户在应用里手动切回中文：规则让位，并且成为全局状态。
    rule_step = mode_authority_step(rule_step.next, true, true, client_of(11), 3, true, 11, false);
    require(rule_step.user_changed && rule_step.next.rule_yielded && rule_step.next.chinese);
    // 让位期间在同一应用里换输入框，不再推规则。
    rule_step = mode_authority_step(rule_step.next, true, true, client_of(11), 4, true, 11, false);
    require(!rule_step.push);
    // 离开再回来是一次新的停留，规则重新生效。
    rule_step = mode_authority_step(rule_step.next, true, true, client_of(12), 5, true, 12);
    require(!rule_step.push && !rule_step.next.rule_yielded);
    rule_step = mode_authority_step(rule_step.next, true, true, client_of(11), 6, true, 11, false);
    require(rule_step.push && !rule_step.push_chinese);

    // 按应用记忆时规则同样生效；客户端已经在规则的模式里就不推。
    ModeAuthorityState app_ruled;
    app_ruled.seeded = true;
    app_ruled.session = 1;
    app_ruled.app = 10;
    auto app_rule = mode_authority_step(app_ruled, false, true, client_of(11), 2, true, 11, false);
    require(app_rule.push && !app_rule.push_chinese && !app_rule.next.chinese);
    app_rule = mode_authority_step(app_ruled, false, true, client_of(11), 2, false, 11, false);
    require(!app_rule.push);
    // 没有规则的应用按应用记忆时从不推。
    app_rule = mode_authority_step(app_rule.next, false, true, client_of(12), 3, true, 12);
    require(!app_rule.push && app_rule.next.chinese);

    // 两个应用的 TIP 线程各自数焦点令牌，常常同时拿着同一个数字。按应用记忆时 Alt-Tab 到另一个应用，令牌一样也是换了客户端：不算用户切换，不出提示，也不让位。
    ModeAuthorityState shared;
    shared.seeded = true;
    auto shared_step = mode_authority_step(shared, false, true, client_of(30), 2, true, 30);
    shared_step = mode_authority_step(shared_step.next, false, true, client_of(31), 2, false, 31);
    require(!shared_step.push && !shared_step.user_changed);
    require(shared_step.next.client == client_of(31) && shared_step.next.app == 31);
    require(!shared_step.next.chinese && !shared_step.next.rule_yielded);
    // 回到有规则的应用，令牌还是同一个，规则照样推。
    shared_step = mode_authority_step(shared_step.next, false, true, client_of(30), 2, false, 30, true);
    require(shared_step.push && shared_step.push_chinese && !shared_step.user_changed);
    // 全局作用域下另一个应用的模式不是用户的选择，权威状态把它推回去。
    ModeAuthorityState global_shared;
    global_shared.seeded = true;
    global_shared.chinese = true;
    auto global_step = mode_authority_step(global_shared, true, true, client_of(30), 2, true, 30);
    global_step = mode_authority_step(global_step.next, true, true, client_of(31), 2, false, 31);
    require(global_step.push && global_step.push_chinese);
    require(!global_step.user_changed && global_step.next.chinese);

    // 推送没发出去（事务锁正忙）：忘掉这次推送，同一会话的下一次观察重推，规则不会因此在这次停留里失效。
    ModeAuthorityState busy;
    busy.seeded = true;
    busy.chinese = true;
    busy.client = client_of(10);
    busy.session = 1;
    busy.app = 10;
    busy.reported = true;
    auto busy_step = mode_authority_step(busy, true, true, client_of(40), 2, true, 40, false);
    require(busy_step.push && !busy_step.push_chinese);
    auto failed = mode_authority_push_failed(busy_step.next);
    require(!failed.pushed && failed.retry == false && failed.app == 40);
    busy_step = mode_authority_step(failed, true, true, client_of(40), 2, true, 40, false);
    require(busy_step.push && !busy_step.push_chinese && !busy_step.user_changed);
    require(busy_step.next.pushed == false && !busy_step.next.retry);
    // 重推落地，之后用户切回中文才是用户的选择：出提示，规则让位。
    busy_step = mode_authority_step(busy_step.next, true, true, client_of(40), 2, false, 40, false);
    require(!busy_step.push && !busy_step.user_changed);
    busy_step = mode_authority_step(busy_step.next, true, true, client_of(40), 2, true, 40, false);
    require(busy_step.user_changed && busy_step.next.rule_yielded);
    // 推送又没发出去，用户自己按 Shift 切到了规则的模式：没有残留的「已推送」把它当成推送落地，照样算用户切换，重推作废。
    busy_step = mode_authority_step(busy, true, true, client_of(40), 3, true, 40, false);
    require(busy_step.push && !busy_step.push_chinese);
    failed = mode_authority_push_failed(busy_step.next);
    busy_step = mode_authority_step(failed, true, true, client_of(40), 3, false, 40, false);
    require(!busy_step.push && busy_step.user_changed);
    require(busy_step.next.rule_yielded && !busy_step.next.chinese && !busy_step.next.retry);
    // 全局作用域的推送也一样重推。
    ModeAuthorityState busy_global;
    busy_global.seeded = true;
    busy_global.chinese = false;
    busy_global.client = client_of(10);
    busy_global.session = 1;
    auto busy_global_step = mode_authority_step(busy_global, true, true, client_of(41), 1, true, 41);
    require(busy_global_step.push && !busy_global_step.push_chinese);
    busy_global_step = mode_authority_step(mode_authority_push_failed(busy_global_step.next), true, true,
                                           client_of(41), 1, true, 41);
    require(busy_global_step.push && !busy_global_step.push_chinese);

    std::cout << "Mode authority: one CN/EN state follows the user\n";
  } catch (const std::exception &failure) {
    std::cerr << failure.what() << '\n';
    return 1;
  } catch (...) {
    std::cerr << "Mode authority test failed with an unknown error\n";
    return 1;
  }
}

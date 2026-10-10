#pragma once
#include <cstdint>
#include <optional>

namespace msime::windows {
// Cross-application CN/EN authority.
//
// With input.ime_mode_scope set to "global" the user expects one Chinese or
// English state to follow them between applications. Each TSF client keeps its
// own mode, so the Server has to be the authority: when a different client
// takes focus it reports whatever mode it happens to hold, and the Server
// pushes its own back. Without this the choice was per-application only, which
// is the behaviour the "按应用记忆" option already describes.
//
// 应用例外（`app_input_mode_rules`）排在作用域前面，和 macOS 的 -englishMode 一样：焦点从别的应用进入一个有规则的应用时推规则里的模式，两种作用域下都是；用户在这次停留里手动切换后规则让位，离开再回来重新生效。规则是例外，不改全局状态。
struct ModeAuthorityState {
  bool chinese = true;
  bool seeded = false;
  // 焦点客户端（TSF 的 pid << 32 | tid）和它的焦点令牌，用来区分换了客户端和同一客户端自己换模式。令牌是每个 TIP 线程各自从 1 数起的，两个应用常常拿着同样的小数字，所以单看令牌认不出换了应用，必须连同客户端一起比。
  uint64_t client = 0;
  uint64_t session = 0;
  // 当前这次停留所在应用的进程号。同一应用里换焦点（两个窗口、一个窗口里的几个输入框）不算新的停留。
  uint32_t app = 0;
  // 本次停留里用户已经手动切换过，规则让位到离开这个应用为止。
  bool rule_yielded = false;
  // 当前会话上一次报告的模式，用来认出同一会话里的模式变化。
  std::optional<bool> reported;
  // 推给当前会话、还没看到回报的模式。回报等于它时是推送落地，不是用户切换。
  std::optional<bool> pushed;
  // 要推给当前会话、但上一次没发出去的模式（mode_authority_push_failed）。同一会话的下一次观察重推它；在那之前会话自己换了模式就是用户切换，重推作废。
  std::optional<bool> retry;
};
struct ModeAuthorityDecision {
  // Send a mode switch to the focused client.
  bool push = false;
  bool push_chinese = true;
  // 同一会话里模式变了且不是 Server 自己推的：用户切换了中英文（快捷键、工具栏、托盘或语言栏），中英文切换提示据此出现。
  bool user_changed = false;
  // The authority after this observation.
  ModeAuthorityState next;
};
// 处理对焦点客户端模式的一次观察，决定要不要推送。
//
// `global` 是配置的作用域，`focused` 表示有没有焦点客户端，`client` 和 `session` 是它的客户端号和焦点令牌，`reported` 是它报告的模式（true 为中文）。`app` 是焦点客户端的进程号，`rule` 是这个进程的应用例外（true 为中文），没有规则时为空。
inline ModeAuthorityDecision
mode_authority_step(const ModeAuthorityState &state, bool global, bool focused,
                    uint64_t client, uint64_t session, bool reported,
                    uint32_t app = 0, std::optional<bool> rule = std::nullopt) {
  ModeAuthorityDecision decision;
  decision.next = state;
  if (!focused)
    return decision; // 没有焦点客户端：权威状态不动，什么也不推。
  auto push = [&](bool chinese) {
    decision.next.pushed.reset();
    if (reported == chinese)
      return;
    decision.push = true;
    decision.push_chinese = chinese;
    decision.next.pushed = chinese;
  };
  if (client != state.client || session != state.session) {
    // 换了客户端。先记下它报告的模式；进程号变了就是一次新的停留，上次的手动让位作废。
    decision.next.client = client;
    decision.next.session = session;
    decision.next.reported = reported;
    decision.next.pushed.reset();
    decision.next.retry.reset();
    if (app != state.app) {
      decision.next.app = app;
      decision.next.rule_yielded = false;
    }
    if (rule && !decision.next.rule_yielded) {
      // 有规则的应用从规则里的模式开始，全局状态不动；按应用记忆时这就是它这次的模式。
      push(*rule);
      if (!global) {
        decision.next.chinese = *rule;
        decision.next.seeded = true;
      }
      return decision;
    }
    if (!global) {
      // 按应用记忆。照样记下会话，之后改成全局作用域时不会把当前客户端当成新来的；但从不推送，每个应用保留自己的模式，正如这个选项所说。
      decision.next.chinese = reported;
      decision.next.seeded = true;
      return decision;
    }
    if (!state.seeded) {
      // 第一次观察用客户端的模式做初始的权威状态，不和它对着干。
      decision.next.chinese = reported;
      decision.next.seeded = true;
      return decision;
    }
    // 别的客户端拿到焦点，报告的是它自己的模式；以权威状态为准，只在不一致时推送，已经对的客户端不打扰。
    push(state.chinese);
    return decision;
  }
  decision.next.retry.reset();
  if (state.reported == reported) {
    // 同一会话，模式没变。上一次的推送没发出去时再推一次。
    if (state.retry)
      push(*state.retry);
    return decision;
  }
  decision.next.reported = reported;
  if (state.pushed == reported) {
    // Server 推的模式落地了，不是用户的选择。
    decision.next.pushed.reset();
    return decision;
  }
  decision.next.pushed.reset();
  // 同一客户端换了模式且不是 Server 推的：用户有意切换，它成为权威状态并带到下一个应用。有规则的应用里手动切换，规则在这次停留里让位。
  decision.user_changed = state.reported.has_value();
  if (rule)
    decision.next.rule_yielded = true;
  decision.next.chinese = reported;
  decision.next.seeded = true;
  return decision;
}
// mode_authority_step 要求推送、但推送没发出去（事务锁正忙、租约已失效或写失败）时调用。忘掉这次推送，否则之后用户自己切到同一个模式会被当成推送落地，提示不出现、规则也不让位；同一会话的下一次观察重推，规则不会因为一次锁忙就在这次停留里失效。
inline ModeAuthorityState mode_authority_push_failed(ModeAuthorityState state) {
  state.retry = state.pushed;
  state.pushed.reset();
  return state;
}
} // namespace msime::windows

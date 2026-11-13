# Agent Note: 鸿蒙基础访问模式不再算「已启用」，引导打开完整体验模式

Status: implemented

## Problem

鸿蒙上启用输入法有两种模式：基础访问模式（`BASIC_MODE`）和完整体验模式（`FULL_EXPERIENCE_MODE`）。基础访问模式下系统设置里显示已开启，`ime -s` 也能把水杉设为当前输入法，但编辑器获得焦点时框架打的是 `panel not create`，扩展的 `onCreate` 一行都不运行，用户看到的就是键盘不弹出（`platforms/harmony/README.md` 里「输入法要在 `FULL_EXPERIENCE_MODE` 下才会被框架驱动」那一段）。

`OnboardingStatePolicy.setup` 原来把基础访问模式算作已启用（测试里写着「basic mode counts as enabled」）。结果欢迎流程不打开，首页状态卡片写「已启用」、两项都是绿勾，2in1 的警告条也不出现，用户拿到的是一个处处说「好了」、实际打不了字的键盘。QQ 群里有用户遇到，只能靠人工回复「到 设置 → 系统 → 输入法 → 水杉输入法 打开完整体验模式」。

## Decision

- `SetupState.enabled` 只在完整体验模式下为 true；基础访问模式读作未启用，`required()` 因此打开欢迎流程，`describe()` 记一条 `enabled in basic mode, opening welcome flow`。
- `SetupState` 多一个 `basicMode`：系统报告 `BASIC_MODE` 时为 true，其余（包括读不到）为 false。宿主经 `setupStatus` 和 `msimeHarmonySetupChanged` 原样交给页面，`basicMode` 变化也算一次变化。
- 共享契约 `ImeSetupState` 加可选的 `basicMode`，只有鸿蒙宿主报告；`apps/harmony` 的解析把缺这一项读作 false，旧宿主照旧。
- 页面在 `enabled === false && basicMode` 时：
  - 欢迎流程第一步和首页状态卡片里「已在系统中启用」那一项改叫「开启完整体验模式」，下面加一段说明：当前是基础访问模式，系统里可能显示已启用但键盘不会正常弹出，到「设置 → 系统 → 输入法 → 水杉输入法」打开「完整体验模式」，再回到输入框重新切换到水杉。按钮仍是「去开启」，打开系统的输入法设置。
  - 2in1 警告条写「水杉输入法处于基础访问模式，键盘不会正常弹出。请在『系统 → 输入法 → 水杉输入法』中打开『完整体验模式』」，按钮「去开启」。
- 宿主没有设置客户端时的三步说明，第 2 步补一句「并选择『完整体验模式』」。
- `oobeStatus` 的 `systemEnabled` 来自同一个 `enabled`，所以 OOBE 快照也把基础访问模式当作未完成。

## Alternatives considered

- **加第三项检查「已开启完整体验模式」，`enabled` 保持「不是 DISABLED」。** 最强的理由是两个事实分开报告，和原来「启用」「设为默认」分两行的思路一致。没用：鸿蒙上「开启」和「完整体验模式」是同一个开关页里的同一个选择，用户不会把它当成两步；未启用时两行同时亮橙色感叹号，第二行还没法单独做。而且基础访问模式下键盘根本不工作，`enabled` 为 true 本身就是错的答案，`required()`、`makeDefaultAction`、OOBE 快照都会跟着错。
- **只改文案、不改 `enabled`。** 欢迎流程不会打开，首页卡片仍是两颗绿勾，提示根本没地方出现。

## Consequences

- 现在处于基础访问模式的老用户，下次打开设置应用会进欢迎流程，第一步直接告诉他缺的是完整体验模式。这是想要的结果：他们的键盘本来就弹不出来。
- 基础访问模式下「设为默认」按钮改为打开系统输入法列表（`makeDefaultAction` 在 `enabled === false` 时的既有行为），不再弹 🌐 引导；在那个模式下切过去也用不了，先去开完整体验模式是对的。
- 共享沙箱、账号会话交换等仍按 `getSecurityMode()` 区分两种模式，不受这里影响（见[鸿蒙设置应用与键盘经共享沙箱交换状态](../architecture/2026-10-09-harmony-ime-shared-sandbox.md)）。
- 「去开启」打开的 `input_method_settings` 深链在模拟器上是空白页（见[鸿蒙手机上「设为默认输入法」改为引导长按 🌐](../feature/2026-10-11-harmony-set-default-globe-guide.md)），所以说明里把路径完整写出来，不依赖深链能落到正确页面。

## Testing

- `platforms/harmony/tests/onboarding-state.test.ts`：基础访问模式读作未启用且 `basicMode` 为 true；全部组合下 `required` 与 `requiredFor(setup(...))` 一致，基础访问模式与未启用同样还有一步。
- `platforms/harmony/tests/keyboard-logic.test.ts`：基础访问模式打开欢迎流程，日志原因单独一条。
- `apps/desktop/tests/account/onboarding-page.test.tsx`、`apps/desktop/tests/core/home-page.test.tsx`：欢迎流程、首页状态卡片和 2in1 警告条在基础访问模式下显示「开启完整体验模式」和路径，「去开启」打开系统设置；宿主报告完整体验模式后提示消失。
- 没有在鸿蒙真机或模拟器上验证：本机没有 DevEco 命令行工具，也没有连接的设备。截图来自浏览器里用同一套共享页面、注入基础访问模式状态渲染的结果。

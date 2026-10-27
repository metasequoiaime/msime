# Agent Note: 统计页身份验证默认关闭，认证后五分钟内再进入不用重新验证

Status: implemented

## Problem

[移动端统计页统一系统认证规则](../bug-fix/2026-10-25-mobile-statistics-auth-parity.md)让 Android、iOS、HarmonyOS 的「统计页身份验证」默认开启，授权只存在于本次页面会话：离开统计页或退到后台就撤销，每次点开统计都要过一次系统认证。维护者认为这太打扰，要求默认关闭，并在认证后加一段冷却。

## Decision

- **默认关闭**：三端存储里没有记录时按关闭处理（Android `StatisticsAuthenticationPreference`、iOS `StatisticsAuthentication.enabledByDefault`、HarmonyOS `StatisticsAuthentication.ets` 的 `getSync('enabled', false)`），共享「我的」页的开关在读到值之前也显示为关。用户自己保存过的选择不变：之前从没碰过开关的用户由开变关，这正是要的效果。
- **五分钟冷却**：系统认证成功后记下时刻，处于已认证状态时离开统计页或退到后台再把时刻挪到现在；之后五分钟之内再进入统计页（含从后台回来）直接放行，放行本身也算已认证，所以连续使用时冷却一直顺延。满五分钟重新认证。
- 冷却只记在内存里，跨页面实例共用（Android 是 `StatisticsFragment` 的静态字段，iOS 是 `StatisticsAuthenticationCooldown.shared`，HarmonyOS 是模块级实例），时钟用含睡眠时间的单调时钟：Android `SystemClock.elapsedRealtime()`，iOS `ContinuousClock`，HarmonyOS `systemDateTime.getUptime(STARTUP)`。进程被结束或设备重启后重新认证，改系统时间也绕不过去。
- 只有系统认证成功或冷却期内放行得到的授权才开启和顺延冷却；关闭开关、未设锁屏密码得到的放行不算，免得刚打开开关就因为之前没验证过的浏览而跳过认证。
- 三端开关的说明改为「开启后使用系统认证，5 分钟内再次进入不用重新验证；未配置锁屏凭据时直接进入」。

HarmonyOS 的冷却放在原生 `StatisticsAuthentication`：共享 React 认证门每次进入都调 `statistics_auth`、离开和退后台都调 `cancel_authentication`，原生在冷却期内直接回答成功，共享页不用改。规则写在纯 TS 的 `StatisticsAuthenticationCooldown.ts`，可以在 node 下测。

## Alternatives considered

- **完全去掉统计页认证**：最省事。否定：维护者只要求默认关、加冷却，想保护统计的用户仍要能打开。
- **冷却从认证成功时算起，而不是从离开时**：实现少一处。否定：在统计页里停留超过五分钟后离开、马上回来又要认证，正是要消除的打扰。
- **冷却持久化到磁盘**：App 被系统回收后仍免认证。否定：要用墙钟时间才能跨进程比较，改系统时间就能绕过；被回收后重新认证一次可以接受。

## Consequences

统计页默认直接打开；打开了认证的用户在五分钟内反复进出、切到别的应用再回来都不再弹窗。代价是开启认证时，别人拿到刚解锁过统计的手机，在五分钟内也能直接看统计。

## Verification

- Android：`StatisticsAuthenticationPolicySmoke` 新增冷却边界（从未认证、刚认证、差 1 毫秒满五分钟、满五分钟、时钟回退），`javac` 编译运行通过。
- iOS：`scripts/test-ios-statistics-authentication.py` 运行真实状态机，新增注入时钟的冷却用例（离开后四分钟回来不弹认证、跨实例共用、满五分钟重新认证、关闭开关和未设密码不开启冷却）；原有用例各用独立的冷却实例，通过。
- HarmonyOS：`bash platforms/harmony/tests/run.sh` 通过，`keyboard-logic.test.ts` 新增冷却分组，`run.sh` 钉住 ArkTS 接线（默认值、冷却放行、成功开启冷却、离开顺延、开机时钟）。
- `apps/desktop/tests/settings/statistics-authentication-setting.test.tsx` 通过。
- 未在真机上验证三端的实际弹窗与冷却；iOS 视图和 HarmonyOS ArkTS 的编译由 PR 上的 CI 完成。

## Related notes

检索了活跃笔记里的统计页认证、系统认证与冷却：[移动端统计页统一系统认证规则](../bug-fix/2026-10-25-mobile-statistics-auth-parity.md) 的「开关默认开启」和「授权只存在于本次页面会话」由本篇取代，凭据规则、认证门位置和后台撤销的接线仍然有效；[Android 无凭据放行](../bug-fix/2026-10-25-android-statistics-auth-availability.md) 与 [Android 统计页认证](2026-10-25-android-statistics-auth.md) 的规则不变；剪贴板操作认证与本篇无关。

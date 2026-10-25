# Agent Note: Caps Lock 轻点区分实体按住与锁定

Status: implemented

## Problem

#9094 报告 macOS 27 的短按仍留下大写锁定。`MSIMECapsLockPhysicallyHeld` 用 `CGEventSourceKeyState(kCGEventSourceStateHIDSystemState, 0x39)`，报告人的只读测量显示该位随锁定状态保持为真，松手不清除；`pollCapsLockTap` 因而等到 300 毫秒后当成长按。既有测试替换整个「是否按住」布尔函数，无法发现原生接口把锁定当成按住的问题。

## Decision

保留轮询、CommonModes、300 毫秒阈值及先撤回再切换规则，仅改物理状态读取：从 `CGEventSourceFlagsState(kCGEventSourceStateHIDSystemState)` 读取 `NX_ALPHASHIFT_STATELESS_MASK`。不要用 `NX_ALPHASHIFTMASK` 或键码 0x39 的状态代替物理按住。若实体松手的 `FlagsChanged` 在下一次轮询前抵达，且没有其他修饰键，就用原始按下时刻立即完成同一判定，不取消或重新布防。初始事件和 HID 状态均没有实体按住位时不布防，保留系统大写锁定，避免不支持该位的设备把长按也当作松手。测试在原生 flags API 边界替换合成状态，不再跳过生产判定函数，始终保留逻辑锁定位并分别改变实体按住位。

Apple 固定源码 [IOHIKeyboardMapper.cpp](https://github.com/apple-oss-distributions/IOHIDFamily/blob/777ccd9698845aadf711e32d843c8c9b777431d9/IOHIDSystem/IOHIKeyboardMapper.cpp) 的 `calcModBit` 在键按住时设置 stateless 修饰位、释放时清除；公开 SDK 的 `IOLLEvent.h` 定义该位为 `0x01000000`，与锁定位 `0x00010000` 独立。它能否在报告人的真实 HID/WindowServer 链路中可靠到达查询接口，仍需实体按键验收。

## 历史检索

[Caps Lock 语言切换](../../implemented/feature/2026-10-11-caps-lock-language-switch.md)持有既有规则及 #9096 的运行模式修复；本篇翻转其中「键码查询可表示实体按住」的决定，其他规则沿用，互链记录状态来源的翻转。Windows IMM 与共享键面笔记属于其他路径，不受影响。检索 proposed、implemented、rejected 没有其他 stateless 状态方案。

## Alternatives considered

- IOHIDManager 直接订阅键盘 usage：能明确区分实体按下和松开，也可定位设备；引入设备生命周期与输入监控授权，本次先采用已有系统状态只读查询，避免为一个快捷键新增全键盘监听。
- 不改变接口，只延长时间窗或增加轮询频率：保留原有结构且改动少；锁定状态松手后不变，任何有限阈值仍会误判，不能解决状态语义错误。
- 按下立即切换并撤回锁定：无需知道实体松手；不能保留长按大写锁定，违背既有用户行为。

## Verification

- 原生接口边界先模拟键码查询随锁定持续为真，旧读取在第一次短按的切换断言失败；读取 stateless 位后现有短按、长按、组字、组合键、撤回失败及三种运行模式回归通过。
- 新增先到松手状态事件的回归在只改状态读取时失败；完成事件处理后 `shortcut-test --caps-lock` 通过，覆盖普通键码 0x39 与 Apple 映射使用的 stateless 键码 0xff。
- 最终 macOS 原生构建、Apple 桥接构建、`shortcut-test --caps-lock`、完整快捷键 CTest（79.69 秒）及 `bash scripts/verify-local.sh --quick` 通过；新增缺位不布防回归通过，笔记和 diff 检查通过。
- 实体按键验收尚未完成：本机两分钟只读探针未记录到状态变化，不能证明真实按键链路通过。PR 使用 `Refs #9094`。

## Consequences

收益是控制器按实体状态而非逻辑锁定计算轻点时长，测试覆盖的边界下移到原生 API，能发现过去被布尔替换隐藏的接口语义错误；代价是依赖公开 IOKit stateless 标志在 Quartz HID 状态表中的传递。

不同键盘和系统版本是否都把 stateless 位传递至 Quartz 状态表仍须实测；初始事件和状态均缺位时保留系统行为，不提供轻点切换。不能据 Apple 源码或合成测试就宣称所有键盘实机修复；如果报告人的设备仍无法观察该位，需要重访 IOHID 的实体事件来源与权限边界。

本决定随后因 #9094 外接蓝牙键盘仍缺失 Quartz stateless 位而翻转；HID 监听与回退策略见[用 HID 实体事件补足 Caps Lock 按住状态](2026-10-24-macos-caps-lock-hid-state.md)。

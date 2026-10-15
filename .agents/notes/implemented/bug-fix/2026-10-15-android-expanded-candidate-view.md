# Agent Note: Android 展开候选后同步宿主视图

Status: implemented

## Problem

Android 0.4.1 的首道双拼输入任意字母再连续点候选会退到「输入连接失败：仅直接输入」。合成首道双拼会话复现：`allCandidates` 放出暂扣候选后将代次从 85 推到 86，宿主仍用旧 `view` 的 85 代身份选词，运行时正确拒绝。离线释义在普通绘制中也会请求完整候选，不必手动展开面板就能触发。

## Decision

所有 Android 完整候选请求共用一个入口；快照推进代次时立即从 `NativeClient.view` 读取同一代完整视图，再交给释义任务或面板。释义任务使用展开后的代次。九键面板和离线释义在候选绘制前完成准备；手动打开面板后重画候选条，防止文字与身份来自不同快照。运行时继续拒绝旧身份，不重放按键、不放宽失败分类。

## Related notes audit

- [完整候选 release 代次](../../implemented/bug-fix/2026-10-11-full-list-generation-release-build.md)：部分重叠，保留展开重排必须换代的不变量，本篇修复 Android 消费方。
- [过期候选回调](../../implemented/bug-fix/2026-10-14-android-stale-candidate-callbacks.md)：部分重叠，保留旧 UI 回调的身份围栏，本篇同步宿主未观察到的运行时代次变化。
- [输入连接自动恢复](../../implemented/bug-fix/2026-10-15-android-input-connection-recovery.md)：相关但不取代，恢复不是避免失步的办法。
- 其余完整候选网格与插件笔记保留各自范围，没有重复提案需要吸收。

## Alternatives considered

- 让完整候选展开保持旧代次：宿主不用同步，但尾部释放会重排座位，旧序号可能上屏另一个词，违反运行时身份契约。
- 离线释义只查询当前页：可以消除普通绘制的展开，但会遗漏完整列表释义，手动面板和九键仍有相同缺口；统一同步更适合现有能力。
- 只更新宿主 `generation` 数字：省一次 JNI，但候选顺序、分页和各行 ID 仍是旧值，无法保证选择正确。

## Verification

独立 API 35 arm64 AVD 上，改动前的 `develop` Java 宿主在 `offline gloss expansion synchronizes view` 断言失败。原生调用的独立合成复现确认 `allCandidates` 把代次从 85 推到 86，旧身份选择被拒绝。测试用固定种子的合成字母，不输出用户输入或异常原文。

`CandidateExpansionDeviceSmoke` 通过安装产品的类加载器初始化真实服务，使用原生首道双拼会话，分别覆盖完整组字后查询离线释义、手动展开和每键查询释义；核对宿主与运行时代次、候选身份和顺序一致，再连续点首选候选至组字结束。每轮新建会话以避免前一轮已展开的引擎缓存令换代断言失去覆盖。编辑器使用只记录调用次数的合成 `InputConnection`，不将这项测试称为 Redmi 真机验收。

最终原生宿主 Java 构建、APK 签名和 16 KB 对齐通过。独立 `msime-client-test-api35-fast-typing`（API 35 arm64，`emulator-5584`）通过完整 `smoke.sh --core`，包括中文上屏/繁体/删除、长度限制、失败分类、过期候选回调、新首道双拼回归及剪贴板保存生命周期；额外的 `NineKeyPanelDeviceSmoke`、`NineKeyShuangpinDeviceSmoke` 均通过。设备安装包 SHA-256 与本地最终 APK 一致。

`platforms/android/check-host.sh`、`bash scripts/verify-local.sh --quick`、`pnpm run verify-notes` 与 `git diff --check` 通过。未使用共享模拟器的中途结果作为最终验收，也未验证 Android 16 真机或发布新版本。

## Consequences

完整候选换代后候选条、面板和释义任务同步采用新视图；旧身份围栏继续有效，按键不重放。仅在展开确实换代时增加一次读视图 JNI，不经过 `apply` 重写编辑器，也不在同步入口递归渲染。手动展开重画整条候选；九键预先取快照，避免同一绘制帧混用两代候选。

本地 Java 宿主构建复用已有 JNI 和已暂存资源，没有修改原生库。模拟器结果不能替代 Redmi Android 16 真机复测。

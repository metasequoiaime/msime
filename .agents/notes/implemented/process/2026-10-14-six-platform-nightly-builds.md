# Agent Note: 六个平台共用发布构建链的 nightly 产物

Status: implemented

## Problem

Issue #7767 希望用户在正式版之间能下载新提交的可安装构建。当前六个平台的发布 workflow 都是手动触发，且每个平台各自处理资源、原生依赖、架构矩阵和产物校验；如果另写一套 nightly 构建，很容易和正式发布链漂移。

## Decision

为六个正式平台的 release workflow 增加 `workflow_call` 入口，保留原有 `workflow_dispatch`。新增的 nightly workflow 按定时任务或手动触发并行调用 Android、iOS、macOS、Linux、HarmonyOS、Windows 六条现有发布链，统一传入只构建、不创建平台 release 的参数；六个平台都成功后，下载各自的 workflow artifacts，按平台打成一个滚动的 `nightly` GitHub prerelease。iOS nightly 复用现有的签名与外部 TestFlight 流程，上传到外部测试组并提交 Apple 审核；其余平台不使用正式发布密钥或签名服务。

为支持非发布构建，Android 只在 `publish=true` 时强制发布密钥，Windows 只在发布时调用 SignPath，macOS 只在发布时导入 Developer ID 和公证凭据。三个桌面发布 workflow 的官方插件 artifact 改用平台专属名称，避免六个 reusable workflow 在同一个 nightly run 中互相覆盖。iOS 构建号使用 2020 年起的 UTC 秒数，两条入口共用发布并发组，避免各 workflow 独立的 run_number 产生重复编号。

nightly 的回归修复保留 iOS 词库页原有的发现加载、下拉刷新和个人词库导入 sheet。HarmonyOS 接线门禁要求三条候选路径的六个释义调用完整闭合并传入显示开关，忽略注释且允许换行，防止相同参数文本的注释掩盖真实接线错误。Android 模糊拼音验收以候选内容和关闭规则后的缺席为契约；词库排序或学习改变候选排名时，仍能验证模糊规则本身。macOS DMG 创建输出系统工具错误，最多重试三次，每次前后清理半成品；持续失败继续阻止签名和上传。

## Alternatives considered

- **复制六套 nightly 构建步骤**：拒绝。依赖安装、资源锁、产物验证和版本矩阵会与正式链分叉，之后修复发布问题时容易漏改 nightly。
- **继续要求模糊拼音结果排第一**：可以捕获排序变化，但规则只保证候选存在，排序属于词库和学习行为，因此门禁改为匹配完整候选内容并保留关闭规则的负向断言。
- **macOS 磁盘映像首次失败立即退出**：失败路径最短，但磁盘映像服务的短暂故障会浪费已完成的 universal 构建；采用三次有界重试，持续失败仍返回工具退出码并清理输出。原始 CI 的 macOS 根因没有确认，这项恢复措施不作为根因已定位的证据。
- **nightly 直接调用正式发布并创建六个版本标签**：拒绝。正式发布标签按语义版本工作，nightly 重跑会与正式版本冲突，也会触发应用内更新逻辑；滚动的独立 `nightly` prerelease 更适合试用构建。
- **只上传 Actions artifact，不创建 Release**：拒绝。artifact 的保留期和入口受仓库设置影响，用户还要进入具体运行；滚动 Release 给出稳定下载地址，同时保留原始 artifact 供排查。

## Consequences

nightly 与正式发布共享每个平台的构建脚本和验证步骤，六个平台任一失败都不会更新滚动 Release。Release 同时提供六个按平台拆分的压缩包和一个总压缩包，并附统一校验文件。nightly 产物是测试构建：Android 使用开发签名，iOS 同时提供模拟器 artifact 和外部 TestFlight 构建，macOS 可能是 ad-hoc 未公证包，Windows、HarmonyOS、Linux 按各自 workflow 的非发布路径产出；不能把它们当正式升级包。

每次 nightly 会消耗六个平台的 runner，默认每天一次；手动触发用于临时验证。iOS 外部测试构建还受 Apple 审核和 App Store Connect 处理时间影响。滚动 release 替换时会先等六个平台全部成功，随后删除旧的 `nightly` release 并创建新的版本；替换窗口内下载链接可能短暂失效。

首次完整运行暴露了 PR 编译门禁未覆盖的路径：iOS 设置页的辅助方法被刷新重构误删，HarmonyOS 候选组装的释义调用缺少参数及闭合括号，Android API 35 完整验收没有准备注音词库。这些修复保留现有产品行为与设备门禁，不靠跳过平台或套件发布部分产物。macOS 藏文包阶段以退出码 1 结束，日志没有指出失败命令，因此打包脚本的 ERR 诊断传入函数和子 shell，仅记录行号、版本 id、退出码；不启用可能展开公证凭据的全局 shell 跟踪。

## Verification

`actionlint` 检查新增和修改的 workflow 通过，`git diff --check` 通过，`pnpm run verify-notes` 通过；`scripts/run-checks.sh` 的全部契约检查通过或按环境跳过。本地 `scripts/verify-local.sh --quick` 通过；六个平台产物匹配规则用合成名称验证。首次远端运行是 `38520744229`：Windows 全部打包和安装验收、Linux 两架构的 DEB/RPM、Android 六版 APK 和 API 28 设备验收通过；iOS、HarmonyOS、Android API 35 和 macOS 失败，滚动发布被跳过，没有发布 nightly 或上传 TestFlight。

补充修复在本地通过 iOS 原生模拟器 Release 构建、HarmonyOS 原生库及 HAP 构建、Android 带锁定词库的 APK 构建。独立 API 35 AVD 一次跑完全部 29 个默认 smoke 套件，包含注音九键、模糊拼音与禁用规则、十四键窄屏；测试后停止并移除该 AVD。共享 AVD 上同时有其它任务的 instrumentation，因此只采用独立设备结果。macOS 原生宿主的 bundle 内容和后端动态库加载 ctest 通过，藏文 universal 安装包完整构建并通过 SHA-256 校验；原始静默失败未复现，后续远端结果见下文。原生构建与完整设备验收在原修复工作树完成，最终 quick 在包含 #7838 的新分支完成。

DMG helper 的合成测试覆盖成功、一次失败后恢复、持续失败与半成品清理，HarmonyOS 门禁的缺参数变异在保留匹配注释时仍被拒绝。ERR 诊断的合成测试检查函数和子 shell 的退出码传递，确认不打印合成凭据。macOS 原始失败命令仍未确定，不能把增加重试或本地构建成功说成已定位根因。

修复 PR #7838 合并后，源提交 `401677684cf9f578a495b60ded2936bf0173a20a` 的第二轮运行 `38529860877` 中，macOS、Windows、Linux、HarmonyOS、iOS 均成功；iOS `0.52.0 (214127513)` 上传成功，加入现有外部测试组并提交 beta review。Android 六版 APK 和 API 28 通过，API 35 在高度重启用例失败，滚动发布仍被跳过。macOS 本轮完整打包通过，但不能据此断言首轮失败原因已经确定。运行结束后的记录被删除，结果以本地保存的最终监控日志和删除前取得的作业日志为依据。

后续修复 PR #7850 确认重启后的系统输入法选择稳定，随后在独立 API 35 AVD 上一次通过完整验收；公共模糊拼音排序和设备首选断言保持原样。共享 AVD 曾被并行会话替换测试 APK，因此那台设备上此前的失败与单项通过不充作当前提交证据，方法见[设备门禁笔记](../testing/2026-10-08-android-device-gates.md)。这份修复的远端完整 nightly 尚未验证，不能把本地通过说成六平台滚动 Release 已发布。

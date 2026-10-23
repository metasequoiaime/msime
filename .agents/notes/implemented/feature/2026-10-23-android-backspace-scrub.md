# Agent Note: Android 退格键同次按压左右滑动删除与恢复

Status: implemented

## Problem

#9324 要求按住退格键左滑连续删除，保持接触时右滑逐步恢复，松手后不能再恢复。已有上滑清空和下滑撤回不能逐步控制删除量。

## Decision

保留按下即删、长按加速和上下滑入口。无组字、墨迹、选区、快速删除备份且允许学习的非密码输入框，在按下时只通过一次 `getExtractedText` 建有界快照。最多保存光标前 4096 UTF-16 单元及两侧各 64 单元校验上下文；编辑器返回更多文本也只复制有界片段。未知位置、部分增量、读取失败则保留普通退格。

左移至少 8 dp 且水平位移大于垂直位移的 1.5 倍后锁定水平手势；已经进入上滑阶段时不再切换。以手指到键盘左边的可移动距离映射删除比例，平方曲线兼顾起步微调和大段删除；滑得快时删除变化也快。长按已删部分纳入同次恢复，回到按下位置恢复全部。按 Unicode 码位定位，不拆代理对，不声称覆盖完整字素簇。

每次水平写入前后核对同一连接、绝对光标和前后文本。拒写、声称成功但没有写入、应用改文或移动光标时终止本次恢复，不能重复盲写。恢复直接提交编辑器，不写统计、学习或剪贴板。松手、取消、生命周期结束立即丢弃快照；取消不自动回滚已经完成的删除。密码、不学习和无痕输入框不建立快照。

## Related notes

- [上滑清空](../../implemented/feature/2026-10-08-android-backspace-quick-delete.md) 与 [下滑撤回](../../implemented/feature/2026-10-17-android-quick-delete-undo.md)：部分重叠；保留各自入口，互链说明方向锁。
- [空输入框退格](../../implemented/bug-fix/2026-10-14-android-empty-editor-backspace.md) 与 [编辑器异常边界](../../implemented/bug-fix/2026-10-16-android-editor-connection-guard.md)：部分重叠；保留空框发键和异常保护，不改既有决定。
- 文本编辑面板、剪贴板卡片滑动删除、滑行输入等命中属于其他入口，与这次按压内恢复无关。

## Alternatives considered

- 每次短按延迟到松手：避免备份按下已删的一字，但破坏按下即删及现有设备验收，因此不采用。
- 仅开始水平滑动时读取：普通退格没有额外 IPC，但无法可靠确认按下已删文字的原位置，外部编辑可能造成错位恢复，因此使用按下时的一次快照。
- 用独立重输键：与上下滑完全隔离，但增加键区入口，报告人只是提出备选，方向锁可以保留熟悉的退格键操作。

## Testing

- `check-host.sh` 的 257 项 JVM 冒烟、216 个设备源文件与 JNI/Java 契约通过。纯 Java 合成编辑器覆盖 Unicode 码位、4096 单元容量、屏宽映射、长按已删内容、连接/位置/上下文变动、拒写和虚假成功后的终止。
- 旧 #9244 APK（`7c5ca8de32`）在 API 28 的新增专项明确失败于左滑多码位删除；新原生 APK 在 API 28、35 的同一专项通过，包含真实系统触摸下的分步恢复、回原点、松手、长按、取消、同位置改文、移动光标、拒写和未知位置。
- 原生库、完整原生发布宿主 APK 构建、16 KB 对齐及开发签名通过；设备夹具复用 `QuickDeleteUndoDeviceSmoke`，可用 `-e scrub-only true` 单独运行，也在原套件正常运行中执行。

- 最终设备夹具在 API 28、35 的完整 `QuickDeleteUndoDeviceSmoke` 均通过，覆盖原有上下滑、短按、选区、密码、不学习、切框与普通中文输入回归，并执行新增水平专项；两台的 `QuickDeletePreviewDeviceSmoke` 也通过，保留 26 键/九键长按提示及上滑清空。
- 最终 `scripts/verify-local.sh --quick`（`20261023-035027`）通过；包含仓库契约、Rust workspace、Android 目标、Android Java 宿主与共享 Apple 桥接层，其余平台按范围跳过。远端 Node 缺失的笔记检查由本机 `pnpm run verify-notes` 补齐，`git diff --check` 通过。
- 集成提交 `5a957811ae` 的完整本机 quick 与推送钩子均通过，额外覆盖 Linux 原生宿主的 111 项测试、Linux 设置外壳检查和 macOS 原生宿主编译。该提交的 PR CI 仅因基线 Windows 引导文案契约失败；基线修复 #9389 合入后，以补充验证记录的提交重新验证 PR 合并结果，不把基线失败视为设备回归。

## Consequences

按下新增一次提取 IPC，编辑器可能忽略长度提示返回全文，但本地快照始终有界。Android 没有通用文档修订号，同一位置相同内容的外部替换不可识别；检查与写入间仍存在应用并发编辑的窗口。真机阈值手感需后续反馈。

绝对选区按 `startOffset + selectionStart` 计算，增量提取不用于快照；字段语义参照 [Android ExtractedText](https://developer.android.com/reference/android/view/inputmethod/ExtractedText)。

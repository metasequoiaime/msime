# Agent Note: 云词库快照轮询等待在途状态读取

Status: implemented

## Problem

`useCloudDictionaryApplyPanel.refreshStatus` 在每次两秒轮询时递增 `statusRevision`。若宿主每次读取耗时 2500 ms，回答到达时已被下一次轮询作废，`localVersion` 始终为空，下载按钮一直禁用。`snapshot_status` 还会尝试处理待应用快照并取走终态，因此它并非一定能在轮询间隔内完成的内存读取。

## Decision

同一客户端只保留一个在途状态读取，后续手动刷新与定时 tick 在它结束前跳过。成功和失败都释放读取槽位；客户端切换和 effect 清理释放旧槽位，旧请求结束时仅能清理自己的槽位。现有 `statusRevision` 继续保证下载、入列、取消等动作优先，过期状态不得覆盖操作结果。轮询仍以原 effect 的两秒相位执行。

## Existing notes audit

- [面板状态抽取](../../implemented/simplification/2026-10-19-cloud-dictionary-apply-panel-state.md)部分重叠。抽取时保留的并发读取策略由本篇修正；状态组织和动作代次继续保留，两篇互链。
- [共享间隔订阅](../../implemented/simplification/2026-10-15-shared-interval-hook.md)部分重叠。订阅和相位不变，是否发请求由业务 hook 判断。
- [iOS 后台版本读取](../../implemented/bug-fix/2026-10-19-ios-snapshot-version-background-poll.md)属于原生键盘线程；本项不改宿主。
- 其他平台轮询、账号生命周期及皮肤更新的命中属于独立机制；没有完全吸收的笔记或需要否决的旧提案。

## Alternatives considered

- 延长固定轮询间隔：修改更少，但宿主仍可能慢于新间隔，无法保证结果能够发布，也会降低正常情况下的反馈速度。
- 保持并发读取，只不作废前一个：慢回答可以发布，但可能倒序覆盖较新状态，且增加有副作用的状态请求；改为等待当前读取结束。
- 完成后再等待两秒：自然串行，但会改变客户端 effect 的既有 tick 相位和正常刷新节奏，因此保留固定间隔并跳过在途 tick。

## Verification

新增回归在修复前五项失败：2500 ms 后本机版本仍为空，手动刷新重复启动请求，旧请求结束后的轮询也未等待当前读取。修复后 hook、真实面板消费者、内容视图、确认生命周期、代次复用与桌面云词库套件六文件 49 项通过，覆盖成功及失败释放、客户端切换、旧完成不释放新读取、读取期间允许下载且旧状态不覆盖下载结果、StrictMode 初始化和卸载清理。真实面板断言延迟读取后显示本机版本并启用下载按钮。

桌面 TypeScript 检查和生产构建、HarmonyOS TypeScript 检查、改动文件 lint 与格式检查、notes 和差异检查通过。`bash scripts/verify-local.sh --quick` 通过，包括仓库契约、Rust workspace、Android 目标、Linux 设置外壳和共享 Apple bridge 编译；HarmonyOS 原生打包因缺少 hvigorw 和 staged 依赖跳过，Windows 原生构建因缺少 MinGW 跳过，其他原生宿主按范围跳过。独立静态审查无 Critical、Important 或 Minor 问题。验证层级是 React/jsdom 和编译，不代表原生宿主装机或真实云账号验收。

## Consequences

永久不结束的宿主调用会占用当前客户端的槽位；底层调用的超时由宿主负责，不在 UI 增设超时后启动重复请求。客户端更换能立即发起新读取，旧调用不会被取消。

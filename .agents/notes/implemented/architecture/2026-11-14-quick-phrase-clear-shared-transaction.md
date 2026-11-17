# Agent Note: 本机快捷短语清空使用共享事务

Status: implemented

## Problem

快捷短语清空原先由 React 逐条调用 `edit`，失败后再由界面层逆序补写。每次编辑都是独立事务，批量操作中途失败会短暂或永久留下半个词库，回滚还可能再次遇到存储故障；界面层也会把内置快捷短语列表误当作可删除的用户行。

## Decision

本机清空通过 host-api 的 `clear_quick_phrases` 操作进入共享 Engine。Engine 在一次独占维护事务中读取 `user_dictionary_operations` 里 `dictionary='quick'` 且 `user_inserted=1` 的行，并复用单条删除的日志写入和输入上下文清理逻辑；任意一行失败时整个事务回滚，内置 `quick_parases` 行不参与删除。操作返回实际删除条数，Windows 原生设置负责确认、调用和刷新，不在界面实现逐条删除或回滚。遵守全部界面原生化的迁移约定，共享 React 设置页撤回本次新增的本机清空按钮及其专属管理 hook；macOS 原生词库页与 Linux 独立 GTK 窗口也复用该事务。共享 host-api 与桌面适配器接口保留，供原生迁移调用。

`clear_quick_phrases` 与 `edit`、`import`、`reset` 一样需要设置页先让输入宿主释放词库。移动端个人词库队列没有独占维护入口，继续拒绝该操作；支持该入口的桌面 host-api 通过共享契约调用。

云端清空的快照筛选也下沉到 host-api 的 `snapshot_clear_quick_phrases` 操作。它校验完整快照后，只移除 `quick` 的 `entry`、`overlay`、`position` 和 `selection` 记录，重算正文摘要并返回原 revision 与实际用户词条数；没有用户快捷短语时原文按字节返回。桌面原生设置仍需在确认后使用既有 CAS 恢复接口提交，筛选操作本身不改变云端内容。

Windows 现代 WinUI、Win7、macOS 与 Linux 的原生入口使用 `quick_clear_preview` / `quick_clear_apply` / `quick_clear_cancel`。完整快照下载、校验和过滤留在 Rust；界面只拿到实际条数、导出 revision 和一次性令牌，避免 C ABI 4 MiB 请求上限截断完整快照。预览按账号与登录代次绑定，只保留最新一份，15 分钟后拒绝提交；取消丢弃内存正文，不访问云端。确认时先比对云端当前 revision，已变化就不发送恢复；比对后再发生写入，由服务端 CAS 拒绝。两次联网都经 `authenticated_as` 固定原登录，换号和同账号重新登录不能提交旧快照。令牌在请求发起前消费，失败后重新预览，不重试旧正文。窗口关闭时先结束进行中的动作，再丢弃预览。

原生临时目录按进程区分，避免两个进程互相清理对方尚在使用的下载文件；只使用既有私有文件工具，不让界面解析或改写快照。`SnapshotClearQuickPhrases` 同时补入共享分发分类，避免 C 入口把快照筛选误当作普通账号请求而漏检临时目录。


## Alternatives considered

- 保留 React 逐条删除并补强回滚：改动较小，但无法保证跨条目的原子性，也会把来源筛选和恢复责任留在界面层。
- 由各原生界面串联整份快照导出、过滤、预览和恢复：能复用文本操作，但会复制登录绑定与 CAS 顺序，并受 C ABI 大小限制；改用共享层持有预览正文与一次性令牌。
- 让 Engine 暴露一个不限数量的批量编辑列表：可复用现有编辑协议，但仍需界面先列出用户行，且容易重新引入内置行误删；专用按种类清空操作能在持锁事务内完成筛选。

## Consequences

- 本机清空只影响用户快捷短语，内置短语和其他词库保持不变；事务失败时词库保持原状。
- 新增了共享 host-api 操作和 C 契约文档，桌面适配器保留实际删除数量返回值，供原生宿主接入。
- Windows 现代设置页和 Win7 设置页都通过同一个 host-api 操作清空；按钮只在个人快捷短语词库中出现，成功后显示删除条数并刷新第一页，Win7 入口还沿用现有确认和词库同步流程。
- 移动端暂不提供该独占操作；移动设置页若接入时应复用同一语义，不能在队列层复制逐条回滚逻辑。
- 云端清空仍沿用[快捷短语本机与云端清空边界](../feature/2026-11-12-quick-phrase-clear.md)中的快照 CAS 规则；共享筛选已进入 host-api，Windows、Win7、Linux 原生界面现使用共享预览与确认接口，macOS 同样使用共享预览与确认接口。

## Verification

- `cargo test -p msime-engine user_dictionary::personal::tests::clear_quick_phrases`：事务成功、保留内置行和失败回滚用例通过。
- `cargo check -p msime-host-api --tests`：共享 host-api 及其测试目标编译通过。
- 本机清空的 React 专属 hook 和对应三项测试随新增 Web 入口一并撤回；共享事务、云端快照并发回归与 Windows 原生确认/刷新测试保留。
- `cargo test -p msime-host-api cloud_dictionary::quick_phrase --lib`：共享云端快照筛选的快捷短语记录、无条目原文保留和畸形输入拒绝用例通过。
- 快照中的 `position`/`selection` 记录没有 `kind` 字段，筛选时按快捷短语条目的 `(code, word)` 关联移除；筛选结果再次通过完整快照校验，避免只做文本匹配留下非法快照。
- Windows Win7 原生词库测试补覆盖确认取消、成功刷新和非快捷短语禁用；CI 首次执行发现新增断言只期待最后一条为清空请求，实际成功路径随后会刷新列表。断言改为核对清空、当前快捷短语第一页刷新这两条请求的顺序，保留取消不发请求、实际删除条数和列表内容断言；本机没有 Windows 宿主，现代 WinUI/Win7 的真实窗口截图与设备交互仍交由 Windows CI 和报告者环境验收。

真实 C ABI 明确拒绝文本版 `snapshot_export`、`snapshot_restore_preview` 和 `snapshot_restore`。macOS 云端清空不调用这些入口；Swift 替身按同样规则拒绝文本快照，回归覆盖实际令牌传递、空列表不提交和冲突不重试，避免替身支持产品不支持的操作。

完整 quick 首轮在 i686 的新增 UI 测试编译失败：普通无捕获 lambda 不能转换成 32 位 Win32 的 `WNDENUMPROC`。改为显式 `BOOL CALLBACK` 枚举回调，保持生产窗口与五个场景不变；`target/windows-pipe-cross-i686` 的 `windows-cloud-quick-phrase-clear` 重新编译和链接通过。首轮其余阶段成功不记作整轮 quick 成功，最终仍运行完整门禁。

本轮整合后的完整 quick（`20261117-134840`）40 分 31 秒、退出码 0，包含修复后的 x86_64/i686 pipe-only 编译。API 28 专用模拟器上，四个共享 crate 共 2,777 项测试、维护租约 13 项和新增清空事务 2 项通过；首次装包冒烟因尚未生成 APK 停止，不记作装包成功。

旧版 Linux 包在两个架构均暴露了测试运行器的依赖缺口：源码树 provider 合约已经通过共享 C ABI 校验 AI 候选，但 legacy 的干净运行容器只有已安装的库，源码树没有 debug 库，合约因此把合法候选读作空列表。运行器沿安装包生成的 `msime_voice_paths.py` 相对路径设置 `MSIME_HOST_LIBRARY`，明确使用刚安装的同版本库；保留配置发现、权限变更和候选内容的原断言，不复制 Python 版拼音规则，也不把缺库当作可跳过的测试。

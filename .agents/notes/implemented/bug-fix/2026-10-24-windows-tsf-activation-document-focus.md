# Agent Note: TSF 线程获得焦点后恢复首次文档连接

Status: implemented

## Problem

Win7 SP1 的真实记事本中，TIP 激活时 `ITfThreadMgr::IsThreadFocus` 仍为假，随后收到 `OnSetThreadFocus`，却没有重发激活前已存在文档的 `OnSetFocus`。原线程回调只投递状态刷新，`Global::g_connected` 一直为假。真实虚拟键进入 KeyEventSink、被分类为输入后留在队列中，Server 没有候选，记事本也没有文字。

临时诊断副本确认：按键时系统前台线程就是 TIP 所属线程，`IsThreadFocus` 已为真，`GetFocus/GetTop` 返回该按键所属上下文；实际产品三条管道均可打开并成功握手。因此问题是错过首次文档连接，不是放宽权限或替换协议。诊断注册覆盖只在可丢弃测试账号中使用并在 finally 恢复。

## Decision

`WM_ThreadFocus` 消息处理时，仅在当前 TIP 仍是 IPC 焦点状态 owner 且尚未连接时，重新查询 `IsThreadFocus` 与当前文档。两者成功且确实有焦点文档才交给现有 `OnSetFocus`，复用文档的激活令牌、状态同步、编辑事件订阅和重连逻辑。

不在回调进入时缓存文档，延后的消息必须复核当前事实；旧消息不能激活已经失焦的线程。已经连接的会话继续只刷新状态，不因此换代或取消组字。空文档、失败查询或失效 owner 不建立连接。

### 既有笔记审计

- [应用键回放顺序](../../implemented/bug-fix/2026-10-16-windows-application-key-replay-order.md)仍持有按键所有权和编辑队列，不修改其检查。
- [Server 启动与权限](../../implemented/bug-fix/2026-10-24-windows7-server-startup.md)已经验证启动与真实握手，本篇处理其后发现的首次焦点缺口。
- 按 `IsThreadFocus`、`OnSetThreadFocus` 与首次激活检索活跃笔记，没有需要翻转的决定。文档焦点继续是会话边界，线程事件仅触发读取真实文档。

## Alternatives considered

- **在每次线程获得焦点时直接设为已连接并创建新令牌。** 代码更少，但现有活动会话和系统表情等辅助窗口的短暂切换也会被重置，违背已有文档焦点边界，因此只补尚未建立且真实有文档的连接。
- **遇到 `IsThreadFocus=false` 就用前台窗口代替。** 可以绕过激活时刻的错误时序，但真实按键时原生查询已为真，无需增加系统版本分支或降低已有所有权检查。

## Verification

真实生产 `WM_ThreadFocus` 分支由 C++ mock 编译执行，13 项回归覆盖延后焦点、查询失败、空文档、旧 owner、COM 查询重入、失焦和活动会话不重复激活。`test-windows-application-key-replay.py` 的 183 项检查与 deferred recovery 回归通过；Win7 x64／x86 和现代 Windows TSF 均重新编译通过。

最终受信任签名安装包在 Win7 SP1 x64 上完成升级，x64 与 x86 系统记事本均通过真实虚拟键、可见候选及“你好”提交；设置的两个系统文件对话框打开并取消，Server 进程持续稳定，英文出厂默认值保持。临时 COM 覆盖已移除，最终产品不含诊断跟踪代码。全仓 quick 结果由[完整产品构建](../../implemented/feature/2026-10-24-windows7-product-build.md)记录。

## Consequences

首次文档连接能够在真实线程焦点到达后补齐，现有会话不因此换代。代价是在未连接的线程焦点消息中增加 COM 查询；查询可重入，必须在调用后复核当前 owner、焦点和连接状态。32 位进程与独立 32 位操作系统的证据不能互相替代。

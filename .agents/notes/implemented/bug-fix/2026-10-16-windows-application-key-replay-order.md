# Agent Note: Windows 应用键回放先于队列尾键

Status: implemented

## Problem

韩文、注音、越南文和藏文的行内组字遇到方向或编辑键时，会先上屏，再让该键交给应用。排队路径用 `_QueueKoreanSyllableKeyReplay` 投递消息，在 `_RunKoreanSyllableKeyReplay` 中调用 `SendInput`；原编辑回调随即完成当前槽，并投递下一键 drain。默认 Windows 消息循环先取 posted 消息再取 input，注入只进入输入流，不表示应用已经取到方向键。后续字母可能先开始组字，再收到原本排在前面的方向键。

## Decision

以一个独立的应用键回放槽挡住后续队列，不延长原音节的 in-flight 槽。原编辑回调照常完成并更新恢复前缀；drain 和按键归属屏障额外检查回放槽。注入 keydown 的 Test 或 KeyDown 入口核对标记、键码、上下文、焦点令牌及代次后，允许该键交给应用，并只异步投递下一次 drain；投递失败交给现有 UI 偏好定时器，不能在 key sink 返回前同步调用 drain。

回放消息只带请求标记，实际键码保存在槽里。32 位标记使用固定前缀和进程范围单调 nonce，跨 clear、Deactivate 和新实例不重置；耗尽拒绝复用。已确认的槽保留身份以兼容重复 Test 和后续 KeyDown，不把同一合法事件吞掉。新请求替换旧确认身份；取消身份不再确认新请求，晚到的取消键按标记识别后吞掉。

`SendInput` 返回 0 时只重试导航发送，返回 1 后只补 keyup，返回 2 后等待 keydown 确认；不重试已经提交的音节。已有 UI 定时器负责未投递或发送失败的重试、失效 owner 与有界等待；无确认超时撤销等待，不能再次发送 keydown。失焦、clear、Deactivate 和显式本地 reset 都释放槽引用。部分发送后取消时尽力补 keyup；若系统仍拒绝注入，不能保证释放成功。已确认的身份持有至下次回放或 clear，最多保留一个上下文引用。回放槽保存原前台窗口。`_IsDeferredKeyOwnerCurrent` 核验 TSF 线程焦点、GetFocus/GetTop 的原上下文、焦点代次、本地 reset 与原窗口，并在 COM 返回后复查；在线还核验 `_IsFocusSessionCurrent` 的 worker Ready 令牌。离线无需 worker Ready，但同样必须通过真实本地 owner 核验。

选择结果未定后的应用编辑键也使用这个槽，白名单扩展为 Backspace/Escape（发送与取消时不带扩展键标记）。队列移交后返回成功，原槽正常退休；分配失败走原重试。独立应用键 pending 时仍捕获后续普通字符与编辑键，即使 FIFO 暂时为空也不允许越过屏障，详见[延迟分类规则](../feature/2026-10-15-engine-email-url-completions.md)。

队列令牌从 edit session 经 DTO 传给普通回放及韩文汉字键处理；列表在排队期间关闭后动态决定 `CommitAndPass`，也使用同一令牌创建屏障。

## Alternatives considered

- **再投递一条普通消息后再 drain**：代码最少，也能让 `SendInput` 先调用。但新消息仍属于 posted，默认循环会先处理它，不能证明应用已经取到 input。
- **延长原 in-flight 槽直到方向键确认**：能复用现有屏障和上下文引用。但音节已不可逆地上屏，原恢复前缀却尚未退休；transport 失败若按原动作重试，可能重复音节。独立槽让提交完成和应用键等待分开。
- **固定延迟后恢复队列**：容易实现并能降低常见窗口中的错序概率。但时间过去不代表输入已被应用取走，慢窗口仍会反序；仅将超时用于退出永久等待，不将其当作确认。

## Verification

- `scripts/test-windows-application-key-replay.py` 提取实际 marker、状态、Queue/Run/Observe/Poll/Clear、完成、调度方法和两个 key sink 的入口分支编译执行，以及实际本地 owner 与应用文字 edit session；183 项检查通过。posted 优先模型覆盖仅 Test、仅 KeyDown、双入口、重复 probe、无 keyup 回调、投递失败、发送 0/1/2、超时、失焦、generation、旧消息、COM 查询重入、上下文引用和 nonce 耗尽。
- `scripts/test-windows-deferred-recovery.py` 通过，原音节在应用键确认前完成并清空恢复检查点，避免重连重复上屏。
- 跨文件接线断言核对 Hanja 动态分支的令牌传递，以及 timer、reset、队列 clear、drain 与新屏障的接入。独立代码复核无阻断项。
- 修复前的最小执行模型记录 `XL`（后续字符先于方向键），修复后各入口均为 `LX`。

## Consequences

TSF key sink 的 Test 只表示该输入已经被宿主取来查询，不代表应用已执行方向操作；只异步调度尾键，使普通消息循环先返回应用处理当前键。不同应用的嵌套消息循环和跨进程焦点变化仍需真实 Windows 编辑器验收。注入沿用执行时的物理修饰键，未保存并恢复捕获时的导航修饰语义。`SendInput` 已进入系统的事件不能撤回，未加载当前 TIP 的目标无法由本槽吞掉晚到事件；不将标记视为认证凭据。

## Related notes

检索 `KoreanSyllableKeyReplay`、`WM_ReplayKoreanSyllableKey`、行内组字和韩文方向规则后，[Windows 组字对齐](../../implemented/feature/2026-10-10-windows-input-compose-mac-parity.md) 记录的是方向键归属与其他组字规则，不拥有此回放确认机制；[整句改字](../../implemented/feature/2026-10-10-sentence-conversion-edit.md) 记录引擎内编辑，属于无关机制。两者保留原决定，本篇单独记录应用输入流与队列之间的屏障。

## References

- [GetMessage 顺序](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getmessage)：posted 先于 input。
- [SendInput](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput)：返回已插入事件数，保持输入顺序，但不表示应用完成处理。
- [OnTestKeyDown](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfkeyeventsink-ontestkeydown)：查询服务是否处理该次按下。

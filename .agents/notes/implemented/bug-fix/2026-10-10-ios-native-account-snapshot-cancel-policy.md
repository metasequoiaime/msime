# Agent Note: iOS 原生账号操作按操作区分快照取消失败的处理

Status: implemented

## Problem

[移动端账户退出时取消待激活的原生快照](2026-10-09-mobile-snapshot-logout-native-queue.md) 定下的是「先清存储、再取消队列，取消尽力而为」。#6499 在 iOS 原生账号层（Swift `BackendAccountSession` 与 `SkinCommunityAPI`）把顺序改成先取消、再写入或清除会话，同时让取消失败中止整个账号操作，却没有更新那篇笔记。

改成严格后出现了永久失败：`DictionarySnapshotQueue.read` 遇到损坏或版本不符的 `state.json` 抛 `.invalid`，`cancel` 只对 `.busy` 重试；`update` 即使没有可取消的请求也重写状态文件，磁盘满时同样失败。于是退出登录、注销账号、清除失效登录和切换账号在这些情况下全部无法完成，用户没有自救手段。注销账号更糟：远端 `deleteAccount` 已经不可撤销地执行，随后取消失败让本地留着一个已删除账号的会话。

## Decision

三类操作对取消失败的处理不同，iOS 原生账号层按下面的规则执行：

- **切换（替换）账号严格。** `signIn` 的 `replacingAccount` 回调仍可抛错；它在新会话写入前运行，失败时保留旧会话、不发布新身份。不这样做，键盘可能把旧账号的快照应用进新账号的词库，而登录失败可以直接重试，代价小。
- **退出登录、注销账号、清除失效登录尽力而为。** `forget` / `logout` 的 `removingAccount` 回调签名是不抛错的 `@Sendable (String) -> Void`，由类型保证清理失败不能阻止本地会话清除。回调仍在清除存储之前运行，缩小键盘按旧身份领取的窗口。`SkinCommunityAPI` 在回调里捕获取消错误，写一条 `snapshot_cancel_failed operation=<操作> reason=<失败类别>` 诊断，不含账号 ID、路径或错误原文。退出登录是用户明确要离开这个账号（本地清除先于远端 `logout` 请求），注销账号时远端已删除，清除失效登录时身份已不可用；留下本地会话只会让用户卡在一个无法再用或不想再用的账号上。
- **无效状态等于没有可取消的请求。** `DictionarySnapshotQueue.cancelIfPresent` 把 `.invalid`（损坏或版本不符）当成功返回；键盘的 `claim` 读同一份文件也会得到 `.invalid`，领取不到其中的请求，放行是安全的。`cancel` 在没有匹配的活动请求时不重写状态文件，只有真正改了状态才写。目录不可访问、状态锁持续被占用和真正需要写入时的写入失败仍然抛出，交给上面两条规则处理。

Rust `client-core` 与 Tauri 账号页仍按旧笔记执行（先清存储、再取消，宿主吞掉错误），本篇不改它们；两边的差异见下节。

## 各宿主现状

- Rust `client-core` 的清理回调没有返回值，切换账号时先保存新会话再回调；Tauri iOS 与 Tauri Android 在四种操作里都吞掉取消错误，切换账号因此不是严格的。
- Android 原生 `SignIn.cancelPendingSnapshot` 吞掉全部异常，切换账号在新会话保存后才取消；刷新得到 401/403 时只清会话、不取消快照，依赖键盘 worker 领取时按属主取消。
- HarmonyOS 的 `HarmonyAccountCloudBridge` 对退出、注销、清除失效登录和登录都先严格取消，失败直接返回 `snapshot_unavailable`；Rust 快照队列遇到无效状态报错，没有可取消的请求时也重写文件。

这些差异是否要向本篇对齐由维护者决定，不在本次改动里处理。

## Alternatives considered

- **四种操作都严格（#6499 的做法）** — 最强的理由是绝不让旧账号的快照在会话变化后还留在队列里。但退出、注销、清除失效登录之后已经没有「新账号」可以被污染，取消失败的风险只是旧账号自己的快照可能在退出后被应用；而代价是状态文件一坏或磁盘一满，用户就再也退不出登录，注销账号还会留下已删除账号的本地会话。
- **四种操作都尽力而为（旧笔记与 Tauri/Android 的做法）** — 实现最简单，账号操作永不因队列失败。但切换账号时旧账号的快照可能落进新账号的词库，而这一步失败是可重试的，没有必要承担这个风险。
- **保留 `removingAccount` 可抛错，由会话层吞掉并通过回调上报** — 会话层可以集中保证「清理失败不阻止清除」。但共享的 `shared/backend` 没有日志设施，需要再加一个上报钩子；改成不抛错的签名同样由编译器保证，而且与 Rust `client-core` 的回调形状一致。
- **在 `update` 里比较前后状态决定是否写入** — 所有调用方都能少写一次文件。但这会改变 `claim`、`fail` 等调用的落盘行为，超出这次修复的范围；只给 `cancel` 一个 `persist` 条件就够了。

## Consequences

- **收益**：状态文件损坏、版本不符或磁盘已满时，退出登录、注销账号、清除失效登录都能完成本地清理；注销账号不再留下已删除账号的本地会话。切换账号仍然不会把旧账号的快照带进新账号。
- **代价与已知上限**：退出类操作取消失败时，旧账号排队中的快照可能在退出后仍被键盘应用，这时只有诊断日志里的一行记录；App 进程按共享偏好里的 `diagnostic_log.server` 写入，关闭时不留任何痕迹。状态文件损坏时队列本身仍不可用，需要另行修复。若键盘以后能读取当前视为无效的状态（例如新版本格式），「无效即无请求」这条必须重新评估。

## Verification

`shared/backend/Tests/BackendAccountSessionTests.swift` 断言切换、忘记和 `logout(all: true)` 的回调确实被调用且发生在会话变化之前，切换时回调失败保留旧会话。`platforms/ios/KeyboardTests/candidate/SkinCommunityTests.swift` 用注入的失败取消覆盖退出、全部退出、注销和清除失效登录都完成清除，以及切换账号失败时保留旧账号。`platforms/ios/KeyboardTests/settings/DictionarySnapshotQueueTests.swift` 覆盖损坏和版本不符的状态文件被放行且不被改写、只读目录下没有可取消请求时成功而有请求时报错。

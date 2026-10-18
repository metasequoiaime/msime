# Agent Note: 鸿蒙账号与云端业务整体切到 Rust 目录模式

Status: implemented

## Problem

鸿蒙宿主的账号与云端业务原来全在 ArkTS 里：`account/AccountCloudBridge.ts`（2800 行）自己发 `/v1/auth`、社区、云词库、云剪贴板、对话、反馈、QQ 群助手和打字统计的请求，自己存会话、刷新令牌，用 `files/state/account-session.lock` 的 `fs.File.lock` 与键盘互斥；AI 皮肤（`HarmonyAiSkins` 加 `AiSkinRunPolicy`）、设置同步的映射（`AccountPreferencePlan`）、头像（`AccountAvatarPolicy`）也各有一份。#8201 要求各平台的账号与云端逻辑只有 Rust 一份（C 头文件 `crates/host-api/include/msime_client.h`），鸿蒙这一份还漏了反馈（发不出去，415）、未登录用匿名账号、需要最近登录的注销提示、按 Rust 的规则同步设置等。

难点是刷新令牌：服务端每次刷新都轮换刷新令牌，拿用过的那枚再刷新超过 30 秒宽限就整个会话族被撤销。同一个沙箱里 ArkTS 的锁文件与 Rust 的 `account-refresh.lock` 互相看不见，所以不能逐页切；两个沙箱（设置应用、键盘扩展，API 12 起各有各的 `files/state`）之间的会话又是经共享沙箱整份复制的（[共享沙箱镜像](2026-10-09-harmony-ime-shared-sandbox.md) 第 5 条），复制会把轮换后的令牌换回用过的那枚。

## Decision

- 整个 HAP 一次切到 Rust 的目录模式，目录是各自沙箱的 `files/state`，同一个沙箱里只剩 Rust 一个刷新方。ArkTS 的桥接、它的两个锁文件和上面几份副本一起删掉。
- NAPI 加通用的异步入口（`account`、`community`、`cloudDictionary`、`accountCloudClipboard`、`ensureAnonymousAccountWithAgent`、`harmonySettingsExport`、`harmonySettingsApply`，沿用 `queueRequest`），长任务（对话 `accountThread`、整份快照 `cloudDictionarySnapshot`、AI 皮肤 `aiSkinGenerate`）各起一个线程、经线程安全函数回到 ArkTS 线程，不占 uv 线程池；`accountStatus` 只收目录、自己拼 `{operation:"status"}`，同步入口发不了要联网的操作。
- ArkTS 只剩薄桥（`account/HarmonyAccountCloudBridge.ets`，规则在 `account/HarmonyAccountRequests.ts`）：
  - 页面的操作按名单放行（`PAGE_ACCOUNT_OPERATIONS` 等），带文件路径的操作和字段页面发不了，路径只由宿主经 DocumentViewPicker 选好、复制进私有目录后填；反馈由宿主按字段重建，不带截图，平台总是 `harmony`。
  - 请求带 `client_agent`（`msime_client_client_agent` 拼出 `msime-harmony/...`）；云剪贴板总是带设备名（`marketName`、`productModel`、「HarmonyOS 设备」）。
  - 登录、退出、注销、`forget`、Google 回环登录前后取消旧账号排队中的词库快照。
  - 整份快照和词库导出：Rust 写进私有目录，宿主复制到选择器给的 URI；恢复先复制进私有目录再交给 `snapshot_restore_preview_file`、`snapshot_restore_file`，预览和恢复之间换了账号就作废。
  - 设置同步：宿主核对账号（`status`）、取字段表和云端文档、读写按键反馈文件（总是带 `feedback`），映射在 `msime_client_account_harmony_settings_export`、`_apply`，上传用 `preferences_put` 绑定读文档时的账号。
- 两个沙箱之间的会话不再整份复制：`account-session`、`anonymous-session` 照旧发布，导入时 `beforeFile` 返回 `handled`，改由 Rust 的 `session_adopt` 在本沙箱的锁里决定（`account/AccountSessionAdoption.ets`，规则在 `AccountSessionAdoptionPolicy.ts`）。键盘采用设置应用的（`from:"settings"`），设置应用采用键盘的（`from:"keyboard"`）；规则只看 user.id 和 expires_at（[C 头文件](../../../../crates/host-api/include/msime_client.h) 的 `session_adopt`）。匿名账号的身份 `anonymous-account` 同样不按文件复制：匿名会话的采用带上发布方身份文件的路径（`identity_path`），Rust 采用了匿名会话时在同一把锁里连同身份一起换成发布方的，会话和身份始终属于同一个匿名账号（迁移差异 M40）。
- 顺序不变量：每个沙箱在进程启动时就登记本进程的采用并占住一条串行队列（拿到共享目录之前）；本进程的每个认证请求（包括匿名注册、Google 回环的 challenge 和授权码）都先 `await AccountSessionAdoption.beforeRequest()`，它再排一次登记过的采用并等它做完。不靠生命周期事件排序：设置页的网页在 `visibilitychange` 时就发认证请求（打字统计补传），它与窗口的 ACTIVE 事件谁先到没有保证，2in1 上只换焦点时连生命周期事件都没有。回到前台时（设置应用 ACTIVE、键盘获得焦点）照旧再排一次，只为让本地显示的账号状态尽早跟上。同一份发布物采用成功一次以后按清单指纹跳过，不再拿锁；失败的下一次再试，同一份失败只记一次日志。
- 发布文件的属主检查不放宽（迁移差异 M1）：Rust 要求文件属主等于所在目录的属主，data-group 目录在真机上是否满足还没核对。不满足时每次采用都是 `account_storage`，宿主写一条点名这条规则、提示 `ls -ln` 的错误日志，README 的「账号与云端业务在 Rust 里」写明真机验收要看它。

## Alternatives considered

- **逐页切换，ArkTS 桥接与 Rust 共存一段时间。** 最强的理由是每一步小、能单独回滚。不行：同一个沙箱里 ArkTS 的 `account-session.lock` 和 Rust 的 `account-refresh.lock` 是两个文件，两边各自刷新同一枚令牌就会重放、被撤销，没有能让两者互斥的办法（ArkTS 的 `fs.File.lock` 是 flock，但锁的是另一个文件），所以同一个 HAP 里的全部刷新方只能在一个提交里一起切。
- **保留整份复制会话文件的镜像。** 理由是旧镜像简单、两边总是一致。不行：复制按「谁后改」裁决，键盘留着的旧副本会把设置应用刚轮换的令牌盖掉，或者撤销设置应用里的换号（迁移差异 M5）；Rust 在锁里按 user.id 和 expires_at 决定，才保证不换回用过的刷新令牌。
- **两个沙箱改用注入存储（`msime_client_session_store`）直接读写共享目录。** 理由是只有一份会话、不用采用。不行：基础访问模式下键盘对共享目录只读，刷新后写不回；跨进程锁也要宿主自己做，鸿蒙两个沙箱之间没有共用的锁文件。
- **发布文件的属主检查在宿主侧放宽（例如属主等于本进程 euid 也接受）。** 理由是能先让真机上跑起来。没有采用：检查在 Rust 里，放宽要改共享规则，而现在连目录的真实属主都不知道；先把失败写清楚，真机上看到 `ls -ln` 再决定。

## Consequences

- **收益**：反馈发得出去；未登录时社区「获取」、举报、AI 皮肤和对话用本机匿名账号；注销需要最近登录时会话保留并提示；设置同步按 Rust 的规则（只跳过不认识的那一个键、繁体变体同步、五笔版本总是写入）；「我的设备」显示鸿蒙和机型；同一个沙箱里只有一个刷新方。
- **代价**：已有的账号操作断网时从「网络连接失败」变成「暂不可用」（Rust 对已有操作仍报 `account_unavailable`）；降级装回旧版时旧版读不了 Rust 写的会话布局，会被登出；云剪贴板换号时不再删密钥串（M3）；云剪贴板列表改按 user.id 绑定，同一账号的令牌轮换不再作废列表；整份快照的恢复要多复制一次（宿主一次、Rust 一次，最多 512 MiB）。
- **仍然覆盖不到**：一边在采用另一边之前就拿用过的令牌刷新（M2 写明的那种）；两个沙箱各用各的锁，两边独立刷新同一枚令牌的风险是原来就有的。
- 这些都没有在设备上跑过：HAP 没有打包，ArkTS 只经 `scripts/test-harmony-arkts-subset.py` 这类文本检查；NAPI 在 Studio 上用 OpenHarmony NDK 编译链接通过。

## Verification

- `platforms/harmony/tests/account-requests.test.ts`：名单放行与拒绝、反馈重建、各入口的请求形状、设置同步的三步、`session_adopt` 的请求与回答、指纹跳过与只报一次、认证请求等在采用之后、采用失败不堵队列。它只导入 .ts 里的纯函数：把这些函数改回旧行为（反馈原样转交、不按名单放行、`adoptedByRust` 不认会话、`SerialGate.settled` 不等、`AdoptionTracker.due` 每次都采用）时对应的检查失败，改动 .ets 里的调用点它看不见。
- `scripts/test-harmony-session-adoption-order.py`：盯 .ets 的调用点。`AccountSessionAdoption.ets` 以外每个 `client.account`、`accountThread`、`community`、`cloudDictionary`、`cloudDictionarySnapshot`、`accountCloudClipboard`、`aiSkinGenerate`、`ensureAnonymousAccountWithAgent` 调用（现在 13 处）都排在同一方法里、包住它的代码块里的 `await AccountSessionAdoption.beforeRequest()` 之后，私有方法则要求本文件里它的每个调用点都满足；`AccountSessionAdoption.ets` 的队列是 `AdoptionGate`、`adoptOnStart` 经 `gate.register` 登记、`beforeRequest` 原样转给 `gate.beforeRequest()`（`AdoptionGate` 每次重排采用的语义由 `account-requests.test.ts` 测，改回只等已排进来的那两组就失败）；两个 ability 的 `onCreate` 先 `adoptOnStart`（方向 `keyboard`、`settings`）再 `ensureAnonymous`；`Settings.syncShared`、`KeyboardSession.syncSharedState` 回到前台时各排一次采用；`sessionDecision` 交给 `adoptedByRust`，两个 `beforeFile` 都落到它；`libmsimeclient.so` 只能以 `client` 导入。在临时副本上逐一删掉 13 行等待（当时叫 `settled()`）、把 `sessionDecision` 或设置应用的 `beforeFile` 改回 `'copy'`、删掉或调换两个 `adoptOnStart`、改错采用方向、删掉两处前台采用、把 `settled()` 挪进分支、换个导入名，24 种改法都让它失败；评审后改成每个请求自己排采用时，又在副本上试了队列换回 `SerialGate`、`adoptOnStart` 不登记、`beforeRequest` 不转给 `gate.beforeRequest()`、调用点改回 `settled()` 四种，也都让它失败。它是文本检查，不认识动态调用；HAP 仍然没有打包。
- `bash scripts/run-checks.sh`；`rbuild sh -c 'MSIME_OHOS_NDK=... bash platforms/harmony/build-native.sh arm64-v8a'`。
- 设备上要过的流程见 #8201 迁移计划第 3 节：覆盖安装后仍是登录状态、两个沙箱之间登出和换号都跟上、云剪贴板升级后不出现「等待批准」、hilog 里没有 `session adopt ... account_storage`。

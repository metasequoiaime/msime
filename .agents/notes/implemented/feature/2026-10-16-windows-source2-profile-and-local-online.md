# Agent Note: Source 2 可选登记名称与 TIP 本地异步候选

Status: implemented

## Problem

[UI-less 与 IMM 移植](../../implemented/bug-fix/2026-10-16-windows-game-uiless-and-imm.md)只完成旧管道页面同步。有效的 TIP 引擎会话拥有自己的 session、generation 和候选 index，不能拿 Server 的候选文本覆盖显示后继续按本地身份选词。Source 2 独占全屏也无法显示外部候选窗口，而 msime-windows #925 提供了用户选择兼容登记名称、让游戏原生绘制的路径。用户要求继续补完这两部分。

## Decision

保留本地候选身份，Server 只回送已发起查询的异步操作。新增可选能力 `HostOnlineCandidates`（bit 10）和 worker opcode 36。TIP 在原始按键编辑会话成功后保存请求号与本地 `online_query`；Server 在同一按键确认送达后保存请求号、查询和焦点租约。云响应、AI 候选或 AI 空回答、静止重排只回送给这个租约和原始请求。普通宿主及明确请求旧页面的宿主不走这条路径。

TIP 在 worker 上有界重组，再在拥有焦点和组字 epoch 的编辑会话里应用。查询的语义必须与当时保存的本地查询一致；跨进程的 session_id、generation 和 AI 上文不参与语义比较，实际 C ABI 调用始终使用保存的本地原始查询，绝不在回调到达时从新组字读取查询来重贴身份。引擎自己的会话与查询代次继续拒绝清空、失焦、换方案后的回答，包括重新输入相同拼音。Server 因已缓存相同答案而没有视图变化时仍回送原始回答，由 TIP 的原始查询判定。应用结果通过本地引擎视图重建候选，数字选择继续使用本地 session/generation/index。静止重排还核对保存的本地查询仍是当前查询，再在本地执行同一操作，不导入 Server 排序后的身份。

同样协商成功的本地 UI-less 导航只回导航意图，在本地编辑会话里执行对应的引擎命令并刷新；旧页面路径仍回带代次的整页。不能只改变 UIElement 上看到的一页，而把本地引擎留在上一页。导航开关仍由 Server 按现有共享偏好判定。

分帧复用 `candidate_continuation_pipe.h` 的 ASCII JSON、事务号、帧序和 owner 核对。继续组字保留原来的 32768 字符限制，新异步操作的重组上限为 2 MiB，足以容纳 256 KiB 云响应最坏的 JSON 转义膨胀。每个分帧仍为独立的固定 404 字节管道消息；不把整个事务并成一条消息。写入失败关闭同一租约；重组或 owner 投递失败使用现有 owner 绑定的失败槽触发会话重建。正常异步结果没有按键和上屏副作用，也不启动 TIP 进程内的联网线程。

CS2 / Dota 2 登记名称提供固定的可选值「拼音输入法」，关闭时恢复 `MSIME_EDITION_TEXT_SERVICE_DESCRIPTION`。状态在本 edition 的 HKLM 64 位注册表视图 `GameCompatProfileName`，与共享偏好和其他 edition、msime-windows 分离。WinUI 设置通过固定的启用/关闭命令重启自身提权，不传任意注册表路径、名称或用户文本。用户取消 UAC 时不更改配置。

提权实例先打开本 edition 所有存在的 32/64 位 TSF profile，读到完整原始 Description 后才修改；记录最后写入，任何写入失败都恢复两份 Description，恢复失败单独报告。缺少所有 profile、访问拒绝或原始 Description 读不出都在修改之前返回。注册与升级始终读取该 edition 记录，卸载只删除本 edition 记录。Source 2 在兼容名称开启时默认交回游戏自绘，用户「从不显示」和「总是显示」列表仍优先。设置告知系统输入法列表也会变名、需要管理员权限和重启游戏。

## 来源与旧笔记审计

来源为 msime-windows #925 的固定合并提交 `e46294308df5a93d5f628878543a6a373cde3ccc`，特别是 `server/src/settings/game_compat_name.cpp`、`windows/src/Global/GameCompat.h` 和注册流程。授权 GPL-3.0，沿用根目录 LICENSE。固定值来自来源的游戏实测记录，本仓没有重新验证游戏内部白名单。

- [游戏强制候选窗](./2026-10-08-windows-game-candidate-overlay.md)：部分取代。保留叠加窗与用户规则，翻转其「不提供登记名称兼容」的选择；本篇补可选、可恢复且按 edition 隔离的实现，两篇互链。
- [UI-less 与 IMM](../../implemented/bug-fix/2026-10-16-windows-game-uiless-and-imm.md)：部分重叠。保留旧页面契约与拒绝 Server 页面覆盖本地身份，补齐其记录的本地异步候选和登记名称缺口。
- [外部候选继续组字](../../implemented/bug-fix/2026-10-16-windows-candidate-continuation.md)：复用分帧与 owner 验证，继续组字的语义和大小限制不变。
- [CS2 系统目录部署](./2026-10-08-windows-cs2-trusted-mode-system32-deployment.md)：独立且保留。兼容名称不能代替签名、系统目录安装和反作弊实际放行。
- [非游戏独占抑制](../../proposed/feature/2026-10-08-windows-non-game-exclusive-fullscreen-suppression.md)：无关，不扩大抑制范围。

## Alternatives considered

- **把 Server 页直接覆盖本地 presenter**：可直接显示云/AI 结果，代码最少。但会丢失本地身份，数字选词会拒绝或错位，部分选词还会破坏继续组字，因此仍拒绝。
- **在 TIP 中再建联网 worker**：能直接使用本地查询，不需要跨进程映射。但会把网络、凭据读取、线程停止和 DLL 生命周期带进每个宿主进程，并重复 Server 已做的请求；选择回送操作。
- **结果到达时按拼音匹配当前查询**：不需要保存请求号，能接收更多迟到结果。但清空后重新打相同拼音也会匹配，给旧结果贴上新代次；因此只使用原始按键保存的查询。
- **继续只保留强制叠加窗**：没有系统变名和提权的成本，适合无边框游戏。但独占全屏无法绘制，用户已明确要求移植缺失部分，因此增加默认关闭的兼容名称路径。
- **允许用户任意输入兼容名称**：和来源一致，未来可手动适配新的白名单。但需要额外的名称校验、命令行引用和误配置恢复，目前只需要来源验证过的固定名称；选择固定参数和明确的恢复按钮。

## Consequences

本地异步刷新不借用 Server 身份，不需要复制或重放原文。网络请求仍由 Server 原有的防抖与缓存控制。代价是新能力只在双方升级后启用；旧组合保持原行为。较大的云响应有更多固定分帧，仍受原有传输超时限制。注册表修改是机器范围操作，系统列表会显示兼容名；它不保证游戏能加载 DLL，也不能替代签名发布产物和实际游戏验收。

## Verification

新增 `windows-host-online-candidates` 使用真实 ServerSession 和 EngineSessionAdapter 两份引擎，验证 AI、云响应、本地高亮与翻页、本地身份选择、AI 空回答、重输同拼音、旧请求与跨焦点分帧。`scripts/test-windows-game-profile.py` 抽取实际生产读写函数，在 full 与 pinyin 两个 edition 的合成注册表上覆盖两视图同步、缺注册、权限失败、逐个写入失败、关闭失败回滚和回滚失败报告。注册函数测试验证升级保留与关闭恢复。MinGW x64 的 host DLL、TIP、Server 和原生测试完整链接，401 个源文件通过 i686 语法检查，本机 Windows 源码测试 162 项通过、2 项因主机差异排除、56 项仍需 Windows 构建。SessionPump 的能力/旧协议导航矩阵已加入 Windows 套件并交叉编译，本机真实引擎测试执行了本地高亮、翻页与选词。新增代码没有 Wine 运行证据，Docker 当前未运行；真实 Windows、UAC、游戏全屏和反作弊尚无可用设备验收。完整本地门禁结果随提交记录。

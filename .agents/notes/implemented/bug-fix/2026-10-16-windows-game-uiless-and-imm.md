# Agent Note: Windows 游戏的 UI-less 候选同步与 IMM 状态

Status: implemented

本篇最初保留的 TIP 本地异步候选和登记名称缺口由[后续实现](../../implemented/feature/2026-10-16-windows-source2-profile-and-local-online.md)补齐；这里仍描述旧管道页面、基岩版锚点和 IMM 的契约。

## Problem

msime-windows #925、#932、#933 修复了游戏里的候选页不同步、基岩版位置闪动与 IMM 宿主 Caps Lock 显示反转。本仓已有 CS2/Dota 2 强制候选窗、系统目录部署、线程池加载器引用保护，但 UI-less 翻页回复仍被当作确认丢弃，后台云候选、AI 候选和静止重排也没有通知宿主重新读取。

## Decision

沿用来源的 UI-less worker opcode 35，按键回复与后台推送携带同一种带 generation 的候选页。Server 只向已确认送达的 UI-less 会话推送，校验焦点租约、传输代次、引擎会话与候选代次；TIP 在所属消息线程再校验焦点和组字代次，拒绝更旧的候选页。旧管道路径的翻页与移高亮应用 Server 的新页，不能再使用本地引擎的占位列表。有效的 TIP 引擎会话仍使用自己的候选与身份，拒绝这类 Server 页面，不能只移植显示而让选词仍落在另一份列表上。

`UiLessCandidates` 是可选协商能力，TIP 只有在协商成功、使用旧管道候选路径时才在键包上设置 `ServerUiLessCandidates`；Server 同时核对协商和请求位才记住推送目标。有效 TIP 会话不请求后台页面，也不会因它不使用的页面超长而断开连接。旧组合不发送或不处理新请求位。

基岩版 `Minecraft.Windows.exe` 的文本锚点固定到游戏所在显示器工作区左上角；该坐标已经是物理像素，不重复做 DPI 转换。Caps Lock 延后一轮消息循环读取真实状态，同时用真实状态反推轻点前状态，保留本仓的单按切换语言行为。

## 来源与旧笔记审计

来源仓库 https://github.com/metasequoiaime/msime-windows 的实际默认分支为 `develop`，本次只读检出固定为 `1c3bd7cb838cbbf31ed3a171704b3e56e90f0dad`；逐个 PR 核对合并提交：#925 `e46294308df5a93d5f628878543a6a373cde3ccc`、#932 `a281dbb6e871d39bc3ad61dd47c22a9a5a616789`、#933 `526bf85cc52b0ce1a122063bccf9dcae1814945d`。源代码授权 GPL-3.0，许可证全文沿用根目录 `LICENSE`，移植来源保留在本笔记和新增契约头中，不引入独立第三方运行时。

- [游戏强制候选窗](../../implemented/feature/2026-10-08-windows-game-candidate-overlay.md)：部分重叠。保留原来的 CS2/Dota 2 强制显示、排除英雄联盟及不改登记名称的决定；本篇补真正由宿主绘制的 UI-less 候选同步。
- [CS2 系统目录部署](../../implemented/feature/2026-10-08-windows-cs2-trusted-mode-system32-deployment.md)：部署已落地，不重复改动。
- [按键沿用锚点](../../implemented/bug-fix/2026-10-13-windows-candidate-position-across-keys.md)：部分重叠，解决正常宿主的瞬间默认位置；本篇处理基岩版持续提供互相矛盾的 extent。
- [非游戏会话的独占抑制](../../proposed/feature/2026-10-08-windows-non-game-exclusive-fullscreen-suppression.md)：仍为独立提案，不在缺少 FSO 实测的情况下扩大抑制范围。
- [数字上屏与宿主候选身份](./2026-10-11-windows-digit-commit-and-hud-anchor.md)、[外部候选继续组字](./2026-10-16-windows-candidate-continuation.md)：部分重叠。TIP 与 Server 的候选身份不同，保留本地选词及继续组字的契约；不把来源的整页替换扩大到有效 TIP 会话。

## Alternatives considered

- **原样搬运来源的共享内存回读**：可在部分上屏后直接拿最新页面，改动少。但本仓使用明确的命名管道回复与宿主会话，不以旧 HTML 共享内存存候选。统一应用同步回复与后台推送，避免把展示缓存当作权威列表。
- **搬运 CS2 登记名称开关**：来源已有游戏内打字证据，游戏原生绘制也能覆盖独占全屏。但这会翻转本仓已有的登记身份决定，且需要新增提权设置、按 edition 隔离与升级恢复路径；本次保留既有策略，不把这项描述为已移植。
- **所有全屏窗口都抑制候选**：与来源一致，规则简单。但原有 FSO 提案明确缺少真机误报验证；不借本次修复绕过该边界。
- **所有 UI-less 页面都切成 Server 列表**：可以直接获得 Server 的云候选、AI 候选和重排结果。但本仓连全拼也建立 TIP 会话，数字与非上屏补全使用本地 session/generation/index；覆盖页面会丢失这些身份，单独改显示会造成拒绝选词或选错词。本次只覆盖旧管道路径；有效 TIP 会话的异步候选统一需要连同输入、选择和继续组字一起设计，未完成这项迁移。

## Consequences

- 旧管道路径的 UI-less 翻页和移高亮由 Server 的回复更新宿主页；后台云候选、AI 候选与静止重排走相同载荷，并保留部分上屏的前缀。普通宿主不收到这类推送。有效 TIP 会话拒绝 Server 页面，保留自己的候选身份；因此不能声称英雄联盟、三角洲行动已获得与来源相同的完整兼容效果。
- 新 TIP 可读取旧 Server 没有 generation 的载荷；旧 TIP 的 `_wtoi` 遇到追加的制表符就停止。后台推送需要双方宣告 `UiLessCandidates`，旧组合不启用。
- 只有 Preedit、没有有效候选页的回复会清空旧管道候选缓存，不把上一段组字的页面留给新预编辑；清空列表保留当前候选会话的代次高水位，结束会话才清零，不能让迟到的旧页因清空而重新生效。更旧但格式有效的候选页直接忽略，不清掉已显示的新页。
- 有效的新页超出固定载荷时，Server 断开该连接让 TIP 重建状态，不截断候选、不继续按旧页选词。代价是该轮可能进入已有的断线回退。
- Caps Lock 延后一轮消息循环读取真实状态；投递失败保留原来的预测回退，未改变全局低级钩子。
- Source 2 独占全屏仍保留原有上限，未移植登记名称切换。真实游戏与 Windows 输入法框架的验收不能由交叉链接、合成测试或 Wine 替代。

## Verification

- `scripts/test-windows-game-compat.py` 编译执行实际 `_QueueCapsLockEdge`、`_ApplyCapsLockEdge` 与 `GetPhysicalTextAnchor`，覆盖 TSF 切换前询问、IMM 切换后询问且无 OnKeyDown、投递失败、多屏负坐标、物理坐标不重复缩放与普通宿主定位；也执行实际 `_ApplyUiLessPayload`，核对旧页拒绝、同代次高亮更新、非法载荷、旧协议和有效 TIP 会话拒绝 Server 页面；执行实际 `push_uiless_candidates`，核对正常发送、传输失败/异常、无效新页、过期焦点、停止与无更新。
- `windows-uiless-candidates` 以实际回复编码器验证更新帧、选词前缀、旧代次、旧会话、旧焦点、旧传输、普通宿主、组字结束及过长新页；还覆盖旧格式解析、乱序拒绝、非法数字、新旧能力协商、请求位和键修饰位隔离。它接入 CMake、Wine 批量入口与本机 native runner，不需要词库。
- `windows-reply-composer` 的 UI-less 断言核对 generation 确实随同步回复发出；`caps_lock_tap` 更新为从切换后的状态还原轻点前状态，保留原有轻点阈值及自注入回归。
- macOS 上 `scripts/test-windows-native-run.py` 的 152 项通过，2 项因主机差异排除，65 项需要 Windows 构建；游戏合成宿主回归检查通过。MinGW x64 产物在 Wine 10 下通过 9 个相关程序：UI-less 候选、回复编码与编排、TSF 配置帧、焦点门禁、Main 帧、导航策略、Caps Lock 轻点与游戏候选策略。Wine 的 Caps Lock 接线检查同时挂载 TSF 和 Server 源码，不把缺少挂载造成的读文件失败当成产品失败。
- 完整 quick、交叉链接和 Wine 的最终结果随同提交记录；没有 Windows 实机和真实游戏验收。

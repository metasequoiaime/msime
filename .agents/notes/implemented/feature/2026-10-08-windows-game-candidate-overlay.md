# Agent Note: Windows 游戏里由水杉画候选窗

Status: implemented

## Problem

Windows 上有一类游戏以 `TF_TMF_UIELEMENTENABLEDONLY`（UILess）激活 TSF，等于告诉输入法「候选由我自己画」，实际却什么都不画：SDL2 在 UILess 下对 `ITfUIElementMgr::BeginUIElement` 一律回 `pbShow=FALSE`，自己的 `IME_Present` 只有一行 FIXME；Dota 2 也是这样。TIP 按约定把 UILess 报给 Server（ClientActivated 的 keycode），Server 锁存后抑制候选窗，用户在这些游戏里打中文只能盲选。

另一类游戏不是 UILess，但 `GetTextExt` 失败（TIP 写入 `INVALID_Y`）或给出贴着左上角的垃圾坐标：候选窗要么被隐藏，要么跑到屏幕左上角。全屏游戏上还叠着几件事：候选窗按工作区钳制，停在任务栏原来的位置上方；游戏把自己置顶后压住候选窗；D3D 独占全屏下外部窗口显示不出来，弹出来还可能把游戏挤出全屏。另外，Server 隐藏一个 `INVALID_Y` 的可见快照时不发渲染回执，每个空格或数字选词键都要白等 `candidate_render_wait_max_ms`（30ms）。

不处理的话，水杉在这些游戏里要么看不到候选，要么在全屏下干扰游戏本身。

## Decision

命中策略的游戏进程里，TIP 不向 Server 报 UILess，也不调 `BeginUIElement`，候选改由水杉自己的候选窗画；Server 为这类「游戏会话」补上兜底定位、全屏下的钳制与置顶、独占全屏下的抑制。游戏进程里不注入渲染钩子，不新增低级键盘钩子，不调 `SendInput`。

### 判定：TIP 在 `ActivateEx` 里决定

`ShouldForceCandidateOverlay`（`platforms/windows/tsf/Global/CandidateOverlayHostPolicy.h`）按下面的顺序取第一个命中的结果：

1. 总开关 `game_compatibility.candidate_overlay` 关闭，或进程名为空：不强制。
2. 进程基名在用户的「从不显示」`excluded_processes` 里：不强制。
3. 进程基名在用户的「总是显示」`overlay_processes` 里：强制。
4. 进程基名在内置排除表 `league of legends.exe` 里：不强制（英雄联盟自己画候选）。
5. 进程基名在内置表 `cs2.exe`、`dota2.exe` 里：强制。
6. `engine2.dll` 和 `imemanager.dll` 都已加载（Source 2）：强制。
7. 以 UILess 激活、`SDL2.dll` 已加载、本线程有类名 `SDL_app` 的顶层窗口：强制。
8. 其余不强制。

进程名不分大小写地按序数比较（`CompareStringOrdinal(..., TRUE)`）。模块检查只用 `GetModuleHandleW`，不会加载 DLL；只有 UILess 并且已加载 SDL2 时才 `EnumThreadWindows` 枚举本线程窗口。自动规则（5 到 7）没有编译期开关，和用户列表一起生效。

- 偏好由 `FanyUtils::ReadConfiguredGameCompatibility` 每次激活读一次。文件里没有 `game_compatibility` 键时就是默认值（开关开、两张表空）；读取本身出错时返回 `nullopt`，这次激活一律不强制，因为这时不知道用户有没有关掉开关或把这个游戏放进「从不显示」。
- 判定必须早于把 UILess 报给 Server，所以进程名改由 `ReadModulePath` 包 `GetModuleFileNameW(nullptr, …)` 取基名，`ActivateEx` 不再调 `CommonUtils.cpp` 的 `GetCurrentProcessName`（那个函数对自身开带 `PROCESS_VM_READ` 的句柄，函数本身保留）。
- `BeginUIElement` 里复判一次：`ActivateEx` 拿到的标志不含 UILess、而 `ITfThreadMgrEx::GetActiveFlags` 此刻报告 `TF_TMF_UIELEMENTENABLEDONLY` 时（例如 CUAS 先激活了线程），按 UILess 重新收集现场再判。`ActivateEx` 时已经是 UILess 的不复判：Server 已经按 ClientActivated 锁存了 UILess，改判也显示不出来。
- 偏好改动只在下一次激活时生效：已打开的游戏要切换一次输入法。

### 强制时 TIP 的行为

- `Global::HostUiLessMode = !ForceOverlayCandidate && _IsUiLessMode()`，ClientActivated 的 keycode 因而为 0，Server 不锁存 UILess。`IsUiLessMode()` 本身不变，按键路径上读它的地方随之按普通宿主处理。
- `Global::Point` 置为 `{0, INVALID_Y}`，不发初值 (100,100)；组字真正结束时（`Composition.cpp` 的宿主终止和 `EndComposition.cpp` 的 `_TerminateComposition`）由 `Global::ResetForcedOverlayAnchor` 再复位一次，因为下一次组字的第一个按键先于 `_StartCandidateList` 发出，不复位就会带着上一个输入框的坐标。不在删除 presenter 时复位：韩文 Hanja 和注音的候选列表关闭后组字还在继续，那时复位会把正在组字的锚点冲掉，列表每次重新打开都先跳到兜底点再跳回光标。等待延迟清理的 presenter 的 layout sink 也不再写 `Global::Point`（`CTfTextLayoutSink::_IsDetached`），否则组字结束后的 layout 变化会把复位改回旧位置，或盖掉同一次按键里新组字的锚点；这一条对所有宿主都生效。
- `BeginUIElement` 直接返回：`_isShowMode=TRUE`、`CandidateUiLessMode=false`，不向游戏登记 UIElement，`_uiElementId` 保持 -1；`_UpdateUIElement` 和 `EndUIElement` 对 -1 提前返回。
- `SendToNamedpipe` 在最终的重连与确认检查之后、`WriteFile` 之前改包：协商到 `GameHostCandidate` 时，Key/Show/Move/Hide 四类包加 `PipeMetadata::GameHost`，KeyEvent 另补上当前 `Global::Point`（按键包本身不带坐标，清空后是 (100,100)）。坐标也只在协商到能力位后才补：旧 Server 对 `INVALID_Y` 的可见快照只隐藏、不发渲染回执，每个选词键都会白等回执超时，而 (100,100) 至少能画出来并回执。必须放在最终检查之后：Main 管道在 `EnsureNamedpipeFocusSessionActivated` 里才打开并协商，放在前面的话首次按键和 Server 重启后的第一次按键都拿不到协商结果。

### 协议

- `FanyImeProtocol::GameHostCandidate = 1u << 5`（`shared/contracts/ipc_negotiation.h`）是可选能力，不进 `Capabilities` 和 `RequiredCapabilities`。TIP 在 Main Hello 里宣告；Server 在生产和非生产两套能力里都宣告，否则开发版和预览版永远走不到游戏会话的逻辑。
- `PipeMetadata::GameHost = 0x10000000`（`platforms/windows/common/PipeMetadata.h`）表示「游戏会话，宿主给的锚点可能不可信」，`key_modifiers()` 把它掩掉。它只能出现在 Key/Show/Move/Hide 包上：`ModeMailbox.h` 对 StatusSnapshot/FocusRestored 用裸的 `modifiers_down != 0` 判全角，`MainFrame.h` 要求这两类包 `modifiers_down <= 1`，带上这一位会读错全角或整帧判为非法。

### Server：游戏会话

- `CandidatePresentation::game_host` 由包上的 `GameHost` 位设置。`CandidateMailbox` 在同一租约（epoch、token、ticket 都相同）里只置位不清除：后续某个包漏了这一位、`refresh_view` 按 Engine 视图重建快照、或带位的 Show/Move 晚于不带位的按键到达，都不会把已认出的游戏会话打回普通宿主；换了租约不沿用。鼠标点选合成的按键包同样带上这一位（`SessionController.cpp`）。
- 兜底锚点 `game_candidate_anchor`（`platforms/windows/src/candidate/GameCandidateAnchor.h`）只对游戏会话生效，并且前台窗口必须属于这个客户端进程（`client_pid` 即 `client_id >> 32`，握手时已校验）、客户区不为空。锚点是 `INVALID_Y`，或者落在客户区外、或者离客户区顶边不到 2 像素（锚点是文本 extent 的 `{left, bottom}`，真实文本行不会贴着顶边）时，返回客户区左下部 `{left + max(round(24 × DPI 缩放), width/20), bottom - height/5}`，DPI 缩放取游戏所在显示器的有效 DPI（`monitor_effective_dpi`），因为客户区是每显示器感知下的物理像素，而 DPI 不感知的游戏 `GetDpiForWindow` 固定是 96，之后照常交给 `candidate_card_placement`。替换发生在跟随光标的锁定和快照身份比较之前，同一个兜底点不会反复触发重绘。非游戏会话遇到 `INVALID_Y` 仍然隐藏。

### Server：前台呈现方式、抑制与置顶

- 主循环每轮先算一次前台和 `foreground_presentation()`（`platforms/windows/src/system/FullscreenForeground.h`），交给 `CandidateWindow::set_foreground`，工具栏的全屏判断也用这一份。分类是 `Windowed`、`Fullscreen`（几何铺满）、`ExclusiveFullscreen`（几何铺满且 `SHQueryUserNotificationState == QUNS_RUNNING_D3D_FULL_SCREEN`）。QUNS 只在几何铺满时调用，`QUNS_BUSY` 不作判据，同一前台 HWND 的结果缓存 300ms。
- 独占抑制只作用于游戏会话，并要求前台 pid 等于客户端 pid（QUNS 是系统全局状态，可能是别的进程在独占）。这类宿主本来就没人画候选，隐藏不会比原来差。
- 反应式锁存覆盖 QUNS 看不到的独占模式（Vulkan 独占、OpenGL 改分辨率全屏）：每个客户端连接（记最近 16 个）第一次在自己的几何全屏前台上弹出后，盯住那个窗口 2 秒，期间窗口最小化、前台换到别的进程、窗口矩形变了、或收到 `WM_DISPLAYCHANGE`，就对这个进程锁存抑制。该进程的前台变成窗口化、同一个客户端重连（登记代次变了），或进程退出（锁存时用 `OpenProcess(SYNCHRONIZE)` 留一个句柄等待它）时解除；不认进程退出的话，pid 被系统复用后，下一个拿到这个 pid 的游戏会一直被抑制。
- 前台在候选窗所在的显示器上全屏时，按 `rcMonitor` 而不是 `rcWork` 钳制。
- `keep_on_top()` 在 `refresh()` 之后调用，同时满足这些条件才 `SetWindowPos(HWND_TOPMOST, … SWP_NOACTIVATE …)`：候选窗可见、前台是几何全屏、z 序上方第一个可见且非 `WS_EX_TRANSPARENT` 的窗口属于前台进程、这个 `render_serial` 还没重申过、距上次至少 1 秒。只认前台进程自己的窗口，是为了不和厂商 overlay、录屏工具这类常驻置顶窗口每轮互抢。
- 策略隐藏（`INVALID_Y` 且无兜底、独占抑制、锁存）一个可见快照时，按 `render_serial` 去重发一次渲染回执，选词键不再白等 30ms。`INVALID_Y` 的隐藏也走这条路，非游戏宿主同样受益。

### 诊断

- TIP：诊断日志开关要等 Server 推来配置帧才打开，晚于 `ActivateEx`，所以判定结果存在 thread_local 的 `GameOverlayDecision` 里，这次激活第一次开始组字时补写一行 `[game] process= preferences= overlay= uiless= active_uiless= sdl2= sdl_window= source2= forced=`，任何进程都写，用来经 MCP 的 `read_diagnostic_log` 给未知游戏归类。复判翻成强制时写 `[game] recheck …`；没有强制、`pbShow=FALSE` 而 SDL2 已加载时写一次 `[game] pbShow=0 …`；强制时 `_StartLayout` 失败另写一行。
- Server：前台呈现方式变化、第一次 QUNS 调用的耗时、独占抑制与锁存的进入和解除（带 pid 和触发原因）、候选窗第一次在全屏下显示时 `GetWindowBand` 的值（取不到就写 unavailable），以及管道对端身份被拒（`RegistryStatus::IdentityRejected`）时的 pid 和错误码，同一个 pid 一分钟只记一条，因为回调跑在握手线程上，而反作弊持续拒绝时 TSF 每次重连都会走到这里。最后一项用来区分「DLL 加载了但 Server 拒了连接」和「DLL 根本没加载」。

### 偏好与设置入口

- `Preferences::game_compatibility`（`GameCompatibilityPreferences`，`crates/client-core/src/preferences.rs`）：`candidate_overlay` 默认 true，`overlay_processes` 与 `excluded_processes` 默认空，等于默认值时不写进文档。条目必须是不超过 64 个字符、以 `.exe` 结尾（ASCII 不分大小写）、不含 `\ / : * ? " < > |` 和控制字符的基名；两张表合计不超过 32 条，合在一起按 `to_ascii_lowercase()` 不重复，和 WinUI 设置页 `GameProcessList.h` 的折叠规则一致。违规时 `validate()` 返回 `InvalidGameCompatibility`，Tauri 映射为 `game_compatibility_invalid`。
- WinUI 设置「候选窗口」页的「游戏」组：总开关加「总是显示候选窗口的程序」「从不显示候选窗口的程序」两张列表。写入前按 `platforms/windows/settings/GameProcessList.h` 规范化（去首尾空白、ASCII 小写）并校验，不合法时就地说明原因、不写入文档，因为 `Save` 失败只会显示一句笼统的提示。
- MCP 的偏好白名单有一个布尔 `game_candidate_overlay`，只读写总开关，不碰两张表。React 设置页只加了类型，没有控件。

### 不在这里做的

- 非游戏会话在独占全屏下的抑制、几何全屏下 `INVALID_Y` 超时兜底：见 [非游戏会话的独占抑制与全屏兜底](../../proposed/feature/2026-10-08-windows-non-game-exclusive-fullscreen-suppression.md)，要等全屏优化（FSO）实测。
- CS2 Trusted Mode 只放行系统目录里签了名的外来 DLL，所以安装器把 64 位 TIP 装进 `System32\IME`，见 [CS2 Trusted Mode 的 System32 部署](2026-10-08-windows-cs2-trusted-mode-system32-deployment.md)；内置表里的 `cs2.exe` 靠它才在正常匹配里生效。
- 候选窗出现和消失时 DWM 在直通与合成之间来回切造成的闪烁，没有做缓解，要先用 PresentMon 测。
- IME 被游戏禁用、或 DLL 被反作弊挡掉时的替代输入通道不做，理由见下面最后两条备选。

## Alternatives considered

- **保留 `BeginUIElement`，只忽略 `pbShow`** — 最守 TSF 契约：UILess 宿主照样拿到 UIElement 通知，想画的游戏仍然能画，改动只在 presenter 一处。但 Server 侧的 UILess 锁存在 `ActivateEx` 发 ClientActivated 时就定了，光忽略 `pbShow` 候选窗照样被抑制，判定无论如何都得提前到 `ActivateEx`；而一旦已经决定由水杉画，再向游戏登记 UIElement，自己画候选的游戏就会出现两套候选或闪烁。
- **所有 UILess 宿主默认强制** — 覆盖面最大，长尾游戏不用任何名单或模块规则，实现也最简单。但自己画候选的 UILess 游戏会被我们的候选窗替换；到了独占全屏我们的窗口又不显示，等于把原本能用的游戏变成不能用。只有 SDL2 和 Source 2 有证据表明「本来就没人画」，强制后不会比现在差，所以自动规则只认这两类加内置表。
- **在 Server 侧按 pid 判定** — Server 已有握手校验过的 pid，名单只在一处维护，游戏线程上不用读偏好文件。但 Server 拦不住 TIP 在游戏进程里报 UILess、调 `BeginUIElement`，这两件事都发生在 DLL 里、早于 Server 能插手的任何时刻，判定只能放在 DLL。
- **在 TSF 侧算兜底锚点** — TIP 手里就有游戏窗口和文本上下文，新 TIP 配旧 Server 的混跑窗口里也能兜底。但 Server 已经有校验过的 pid、前台窗口、显示器与 DPI 排版，再在游戏进程里写一套同样的定位逻辑，只为覆盖升级后旧 Server 还没重启的短暂窗口，不值得。
- **兜底点放在客户区水平居中的底部** — 位置对任何游戏都对称、可预期。但调研涉及的几款游戏聊天输入框都在左下部，居中还会挡住射击游戏的准星区域；先用左下部，等实机记录各游戏聊天框的位置后再调。
- **Hook 游戏的 `Present`，把候选画进游戏画面** — 这是唯一能在真独占全屏里显示候选的办法，能覆盖现在明确放弃的独占全屏游戏。但往游戏进程注入渲染钩子正是反作弊要抓的行为，有封号风险；一个输入法 TIP 也不该接管游戏的交换链。
- **`WH_KEYBOARD_LL` 加 `SendInput` 的独立输入条** — 不依赖游戏是否加载我们的 DLL、是否禁用了 IME，能覆盖 DLL 被反作弊挡掉或 IME 被关掉的游戏。但输入条必须先抢焦点，独占全屏下会切出游戏，很多游戏失焦还会关掉聊天框；`SendInput` 回填受 UIPI 限制，反作弊怎么看它也查不到公开说法；用户点名的主流游戏里没有一款确认是「DLL 加载了但 IME 被禁用」。以后真要做，最小安全形态是热键弹出能激活的输入条、回车只写剪贴板并提示粘贴，不调 `SendInput`。
- **冒充 Source 2 白名单里的输入法 profile 描述** — 调研材料称 Source 2 的 `imemanager` 只对一份已知输入法名单自己画候选（未核实）；如果成立，把 TIP 的 profile 描述改成名单里的名字，CS2 和 Dota 2 会自己画候选，连独占全屏都不受影响，也不用违反 UILess 契约。但这是冒用别家产品的名字，用户在语言栏和系统设置里看到的也会是别家的名字；它依赖游戏未公开的内部行为，任何一次游戏更新都可能失效；游戏还可能按那家输入法的私有约定读取我们并不提供的东西。它对 SDL2 游戏也毫无帮助。

## Consequences

- **收益**：
  - SDL2 游戏、Dota 2，以及用户自己加进「总是显示」的游戏里能看到候选；锚点不可用时候选落在游戏客户区左下部，而不是消失或停在屏幕左上角。
  - 几何全屏下候选窗按整块显示器钳制，并在游戏自己置顶后重申置顶；独占全屏的游戏会话里候选窗不弹出，不会把游戏挤出全屏。
  - 策略隐藏照样发渲染回执，包括非游戏宿主在 `INVALID_Y` 期间的隐藏，选词键不再白等 30ms。
  - 游戏进程里新增的调用只有 `GetModuleHandleW`、`GetModuleFileNameW`、本线程的 `EnumThreadWindows`/`GetClassNameW`、每次激活一次偏好读取、`BeginUIElement` 里一次 `GetActiveFlags`；进程名的来源换掉后，游戏进程里不再对自身开带 `PROCESS_VM_READ` 的句柄。
- **代价与已知上限**：
  - 违反 TSF 契约：宿主以 `UIELEMENTENABLEDONLY` 激活时，TIP 不调 `BeginUIElement` 就自己显示 UI。只对命中策略的进程这样做，用户可以把它放进「从不显示」或关掉总开关。
  - 误判自绘游戏：只用 SDL2 读手柄、候选由自己的 UI 层画的游戏可能被 SDL2 规则命中，自己的候选被替换。`SDL_app` 类名条件、内置排除表、「从不显示」列表和 `[game]` 日志是对策。自动规则在实机验证之前就已启用，这是有意的取舍：没有它，SDL2 这一最大的长尾类别要靠用户自己配。
  - 首键跳动：游戏其实能给出有效 extent 时，第一次按键先出现在兜底点，Show 到达后再跳到真实位置。窗口化的非 UILess 游戏锚点是垃圾值时不会自动兜底，用户要把它加进「总是显示」，它才算游戏会话。
  - 兜底点可能盖住 MOBA 的小地图；位置固定，不能拖动，也不按进程记住偏移。
  - 独占检测有盲区：QUNS 对 FSO、Vulkan、OpenGL 的报告都没核实过。锁存兜底的代价是第一次弹出时已经造成了一次破坏；用户在 2 秒窗口里自己 Alt-Tab 也会触发锁存，改成窗口化或重连即可解除。
  - 版本混跑：新 TIP 配旧 Server 时协商不到 `GameHostCandidate`，坐标照样注入，游戏能给出有效 extent 时照样显示；`GetTextExt` 失败时旧 Server 照旧隐藏，并且每个选词键白等最多 30ms。旧 TIP 配新 Server 不受影响。
  - `ActivateEx` 里多一次同步读偏好文件，是游戏线程上的磁盘 I/O；读失败时这次激活不强制。
  - 静态链接 SDL2 的游戏按模块名识别不到，只能靠用户列表。
  - Server 主循环每轮多了前台分类（QUNS 有 300ms 缓存、只在几何全屏时调）、`GetClientRect` 和限频后的 `keep_on_top`。若诊断日志里 QUNS 首次耗时到了毫秒级，就该把分类挪到 `WINEVENT_OUTOFCONTEXT` 回调里。
  - 去重的大小写规则：Rust 和 WinUI 都只折叠 ASCII 字母，两边对重复的判断一致；只有 TIP 匹配进程名时用 `CompareStringOrdinal` 不分大小写，也折叠非 ASCII 字母。这个差别无害：只差非 ASCII 大小写的两项分在两张表里时，TIP 两张都命中，`excluded_processes` 优先，结果确定；在同一张表里时，同一个进程命中两次，结果相同。
  - 开发版和预览版没有 uiAccess，候选窗所在的 z 带没验证，全屏游戏上只能靠 `keep_on_top`。若诊断日志里 `GetWindowBand` 的值不是 `ZBID_UIACCESS`，就该另开提案评估 `CreateWindowInBand`。

## Verification

- 自动化测试覆盖的面：
  - Rust：`crates/client-core/src/preferences/tests.rs` 的 `game_compatibility_defaults_on_and_stays_out_of_the_document` 与 `game_compatibility_process_names_are_validated`；`crates/mcp-server/src/preferences.rs` 的 `the_game_candidate_overlay_switch_leaves_the_process_lists_alone`。`cargo test -p msime-client-core preferences`、`cargo test -p msime-mcp-server`。
  - 偏好门禁：`python3 scripts/test-preferences-field-parity.py`、`python3 platforms/macos/tests/settings/preference_coverage.py .`（`game_compatibility` 记为 macOS 不适用）。
  - 契约：`shared/contracts/tests/windows_ipc_contract.cpp` 断言 `GameHostCandidate` 不在两套必选能力里、不和已有位重叠，并覆盖新旧 TIP 与 Server 的三种协商组合；`platforms/windows/tests/core/main_frame.cpp` 断言 `GameHost` 被 `key_modifiers` 掩掉、不和内部标志重叠，Key/Show/Move/Hide 带它仍是合法帧，StatusSnapshot/FocusRestored 带它被判为非法帧。
  - TIP：`msime-tsf-candidate-overlay-policy`（`platforms/windows/tsf/tests/input/candidate_overlay_policy.cpp`）覆盖基名提取、大小写、完整的优先级顺序、SDL2 三个条件缺一不可、开关关闭和空进程名。
  - Server：`windows-game-candidate-anchor` 覆盖 `INVALID_Y`、左上角垃圾值、贴顶边、客户区外、有效锚点与边界锚点、非游戏会话、前台不属于客户端、空客户区、负坐标副屏、DPI 1.5 和拿不到 DPI；`windows-fullscreen-foreground` 覆盖 `classify_foreground` 的四种组合、300ms 缓存和 QUNS 耗时出参；`windows-candidate-initialization` 覆盖 `GameHost` 位与 UiLess 的组合；`windows-server`（`tests/runtime/server_smoke.cpp`）覆盖 `INVALID_Y` 策略隐藏只发一次回执、游戏会话靠兜底锚点显示、独占全屏时隐藏并发回执、前台不属于客户端时既不抑制也不兜底。
  - `tests/ui/candidate_mailbox.cpp` 的 `game_host` 粘滞用例属于 `windows-session-smoke`，它在 Wine 下是已知失败（`scripts/known-failures.txt`），结果只能在原生 Windows 上看。`GameProcessList.h` 没有接入测试目标。
- 只有实机能回答、尚未确认的事项（签名的 x64 安装版）：
  - SDL2 游戏：诊断日志里出现 `sdl2=1 sdl_window=1 forced=1`，候选可见、位置合理；同时看有没有 `pbShow=0 sdl2=1 forced=0`，那表示 CUAS 先激活、复判也没接住。`SDL_app` 类名凭对 SDL 源码的记忆写成，以日志为准；TSF 在宿主后来以 UILess 再激活时会不会重调 `ActivateEx`、`GetActiveFlags` 会不会变，同样没验证。
  - Dota 2：无边框下候选可见；日志里 `source2=1`，以确认 `engine2.dll`、`imemanager.dll` 这两个模块名；用 PresentMon 记录候选窗出现和消失时 PresentMode 在 Independent Flip 与 Composed 之间的切换次数，据此决定要不要做闪烁缓解；Vulkan 独占（如果有这个选项）下锁存只触发一次就生效；记录聊天框的实际位置，评估兜底点会不会盖住小地图。
  - CS2：用签名的正式安装包、不加任何启动项时，TIP 加载进去、不降级、叠加窗可见，核对项见 System32 部署那篇笔记。
  - 英雄联盟的可执行文件名是否就是 `League of Legends.exe`。
  - PMv2 线程上 `GetClientRect`/`ClientToScreen` 拿到的是不是物理坐标；锁存触发条件在真游戏里的误报率。
  - 观察项：`GetWindowBand` 的值；Server 日志里有没有 `Pipe identity rejected`，即反作弊有没有剥掉 Server 打开游戏进程所需的权限；QUNS 首次调用的耗时。

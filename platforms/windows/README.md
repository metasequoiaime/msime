# Windows Server 会话适配

## 目录结构

Windows 平台的实现源码在 `src/` 下；`tsf/`、`msimeui/`、`tests/`、`installer/` 各自守着协议、UI、测试与打包的边界，`experiments/` 放不进产品的验证工具。平台根目录放构建文件、脚本、清单和文档。

`common/` 只收 TSF DLL 与 Server 两侧都要编译的协议头：`PipeMetadata.h`（主管道元数据位）、`AuxMessage.h`（Aux 管道消息的编码与解析）、`TsfFocusLeaseProtocol.h`（焦点租约帧）、`KeyEventSendResult.h`（按键写入结果的三分类；`tsf/IPC/KeyEventSendResult.h` 只是把它以 DLL 原有的全局名转出来）和 `StateDirectory.h`（状态目录的解析顺序：本版本的数据目录环境变量（full 是 `METASEQUOIA_IME_DATA_DIR`）、本版本 HKLM 键在 64 位视图下的 `DataDir`、`%LOCALAPPDATA%\<本版本的状态目录>`（full 是 `MSIME-Client`），两个进程各自解析，但必须落到同一个根；`crates/host-windows` 的 `server_state_directory` 是它的 Rust 副本，`scripts/test-windows-state-dir-parity.py` 核对两边都从版本表取这些名字）。`tsf/` 只能以相对路径（`../common/`、`../../common/`）引用这里的头文件，不得再伸进 `src/`；Server 侧经全局 include 路径按文件名引用。往这里加头文件等于扩大 DLL ↔ Server 的契约，只放两侧确实共用的定义。

## 产品版本（edition）

同一套源码按 `shared/contracts/editions.json` 打出几个可以同时安装、彼此完全隔离的产品：full（水杉输入法，引入版本之前的产品本身）、pinyin（水杉拼音）、wubi（水杉五笔）、japanese（水杉日语）、vietnamese（水杉越南语）和 tibetan（水杉藏文）。每个版本有自己的 TSF CLSID、profile 和全部 TSF 内部 GUID、Inno AppId、Program Files 下的安装目录、HKLM 键、状态目录、用户目录（匿名账号和使用统计）、数据目录环境变量、看门狗计划任务、host DLL 名、`MSIME.exe` 的 Tauri identifier 和安装包名；命名管道、命名事件、互斥量和窗口类名都带 `.<id>` 后缀。full 的后缀是空串，所有标识与引入版本之前相同。

- `platforms/windows/scripts/edition_windows.py gen` 从版本表生成并提交 `shared/contracts/msime_edition.h`（C++ 读的宏）和 `installer/editions.iss`（Inno Setup 读的 `#define`）。构建必须定义且只定义一个 `MSIME_EDITION_<ID>`：CMake 由缓存变量 `MSIME_EDITION`（`Edition.cmake`，缺省 full）定义，WinUI 设置窗口工程由 `MsimeEdition` 属性定义；少了它头文件以 `#error` 停下，不会悄悄编成 full。
- TSF DLL、Server、看门狗、prepare 工具和设置窗口在编译期绑定一个版本，所以每个版本各编一次：`Build-Client.ps1 -Edition <id>` 和 `build-cross.sh <arch> <id>` 的输出在 `target/windows-<id>`（full 仍是 `target/windows-full`）。host DLL 改成版本表里的名字（例如 `msime_host_api_wubi.dll`），再按原 DLL 的导出表生成同名导入库（MSVC 用 `lib /DEF`，MinGW 用 `dlltool`）：两个版本的 TIP 被同一个应用加载时，按导入表找 DLL 会拿到先加载的那一个。
- `MSIME.exe` 和 `msime-mcp.exe` 所有版本共用一份构建，运行时读 Server 目录里的 `edition.json`（只有不是 full 的包才有，由 `Prepare-PackageFiles.ps1 -Edition` 写入）决定管道后缀、状态目录和 Tauri identifier。
- 每个版本注册在它的默认方案所属的语言下（版本表 `langid`，经 `msime_edition.h` 的 `MSIME_EDITION_LANGID` 进入 TSF 的 `RegisterProfile`、看门狗和设置窗口的「添加到键盘列表」）：中文版本是简体中文 0x0804，日文版 0x0411（日语），越南文版 0x042A（越南语），藏文版 0x0451（藏语），于是在 Windows 设置里分别列在这几种语言下。TIP 的行为不按语言分支：保留键、开关和标点 compartment、转换模式和语言栏按钮在各版本都一样，提交的文字按注册语言标上 `GUID_PROP_LANGID`；唯一按语言取的是触摸键盘布局（中文版本是优化的简体拼音布局，日文版是优化的日文布局，越南文和藏文版没有优化布局，用经典布局）。未在真机核实：注册在日语下时，系统的输入指示器按 TIP 写的转换模式位（`TF_CONVERSIONMODE_NATIVE`、`FULLSHAPE`）显示成什么样子，以及触摸键盘是否按上面的布局弹出。
- 日文、越南文和藏文版不带中文主词库、n-gram 和整句模型（资源锁见 `resources/editions/<id>.lock.json`），`Prepare-PackageFiles.ps1` 也不给它们装落定重排模型、手写模型和非英文离线释义（版本表 `features.handwriting`、`features.offline_glosses` 为 false），托盘菜单和原生设置窗口也没有手写；越南文和藏文版只带 core，日文版另带日文词典。
- Server 把版本 id 交给宿主库准备状态根，宿主库按版本选资源锁、收窄方案；托盘和设置窗口只列出本版本提供的方案和本版本带的快捷模式（五笔版没有临时日语，也没有全拼、双拼的辅助码）。几个版本的 Server 同时运行时，维护快捷键由焦点所在版本的 Server 处理（每个生产 Server 用命名事件 `MetasequoiaImeServer_ModeActive<后缀>` 发布本版本的模式是否活动）；没有任何版本的模式活动时，由先收到按键的 Server 处理，不会谁都不管。
- 数据目录的所有权标记文件名也按版本取：full 是 `.metasequoiaime-data`，其他版本接上名字后缀（例如 `.metasequoiaime-data.wubi`）。每个版本的安装器（包括 full）只认本版本的标记，目录里只要有别的版本的标记就不认，即使那是它自己的默认数据目录，所以不会接管、清理或删除别的版本的数据目录。full 的标记文件名和内容不变，以前的 full 写下的标记照样认。
- 标记只看目录顶层，看不到嵌在子目录里的别的版本，所以安装器还按 `editions.iss` 里别的版本的注册表键和安装目录名（由 `edition_windows.py gen` 从版本表生成）找出别的版本的数据目录：它们登记的 `DataDir` 和默认目录 `%LOCALAPPDATA%\<安装目录>`。本版本的数据目录不能和这些目录重叠或互相包含，向导和 `/DATADIR` 都会拒绝；卸载和更换数据目录时，嵌在本版本目录里的别的版本的数据目录原样留下，迁移也不把它当作用户数据复制。
- 升级和卸载前，每个版本的安装器（包括 full）只结束可执行文件在本安装 `server` 目录里的进程，不按映像名结束：几个版本的 Server、看门狗、设置窗口、`MSIME.exe` 和 `msime-mcp.exe` 同名，`taskkill /IM` 会把同时安装的其他版本一起停掉。已经发出去的旧版 full 仍按映像名结束进程，所以卸载旧版 full、或运行旧版 full 的安装包时，同时安装的其他版本的进程会被停一次；数据和安装不受影响，Server 在下次需要时由 TSF 重新拉起，看门狗在下次登录时由计划任务拉起。
- `scripts/test-editions.py` 检查版本表（GUID 两两不同、名字不撞、目录不嵌套），`scripts/test-windows-editions.py` 检查生成文件没有漂移、安装脚本按版本展开后互不越界、每个版本的 `langid` 是它默认方案所属的语言、`release-windows.yml` 的发布矩阵恰好是有 Windows 段的全部版本，并且 Windows 源码不再自己写 full 的 CLSID 和注册表键。

`src/` 按职责分目录，每个目录一句话说清它收什么：

| 目录 | 收什么 |
| --- | --- |
| `entrypoints/` | Server 与 prepare-host 两个 `main`，与 `platforms/linux/src/entrypoints/` 同一约定 |
| `ipc/` | TSF DLL ↔ Server 的命名管道：监听、握手、会话泵、回复编排与编码 |
| `input/` | 输入队列、焦点会话、按键与编辑策略，以及打字统计的记录点 |
| `candidate/` | 候选窗、右键菜单、悬浮工具栏、托盘菜单，以及云/AI/翻译的候选工作线程 |
| `voice/` | 语音输入：热键、采集、识别客户端、控制协议，以及录音时的波形浮层 |
| `clipboard/` | 剪贴板历史的采集与存储 |
| `panels/` | 原生 emoji 与手写面板的源码，不参与构建；产品里这两个面板走共享桌面面板宿主，详见下节 |
| `system/` | 以上都不属于的系统层：看门狗、首次运行、偏好监视、焦点网关、shell 启动、进程与管道身份 |

`settings/` 是独立的 Windows App SDK / WinUI 3 设置宿主。它只负责设置窗口和共享偏好文件的读写；TSF、Server 和共享面板仍由各自的宿主负责。

### `panels/` 是什么

`EmojiPanel.h`、`HandwritingPanel.h` 和 `EmojiPanelIcons.{h,cpp}` 不在 `CMakeLists.txt` 里，也没有任何文件 include 它们——它们不参与构建。产品里的表情/符号面板和手写识别板由共享桌面面板宿主（Tauri）提供，托盘菜单通过 `--route` 拉起，见下文「托盘菜单与共享界面」。`msimeui/demos/` 下另有一份**在构建的** `EmojiPanel`，但那是 demo，比这里这份短（163 行对 210 行），且不接 `ClipboardHistory` 与 `NativeTextInput`。这里这份是原生面板的另一条实现路线，单独成目录保留，让它的状态一眼可见，而不是混在 `system/` 里。

本目录以 MSIME-Windows 的完整功能和既有 TSF DLL / Server 协议为基线：Rust/C++ 共享会话、管道、焦点和回复编排，以及 TSF 注册、Server/Host DLL、原生候选窗口、语音与安装打包都在这里落地。

固定 Engine 的 `FanyImeNamedpipeData` 键包通过 `msime-host-api` 接入。共享库只进入独立 Server，不加载到注入应用的 TSF DLL 中；TSF DLL / Server 进程隔离、版本化 Named Pipe 契约和 UI 原生窗口所有权是这条边界上不变的三条约束。

## 包管理器（winget、Scoop、Chocolatey）

`packaging/` 下是 winget（`Metasequoia.MetasequoiaIME`）、Scoop（`msime`）与 Chocolatey（`msime`）的包定义模板和渲染脚本 `packaging/render.py`。三个包都只静默运行发布页上 full 的 Inno Setup 安装包，不另编二进制；包描述和主页 `https://github.com/metasequoiaime/msime` 与 Linux 各发行版的定义一致。包管理器只能指向签过名的安装包：`release-windows.yml` 先行发布的安装包未签名，其 uiAccess Server 无法启动，所以 `render.py` 拒绝没有有效 Authenticode 签名的安装包；维护者把 SimplySign 签名的安装包替换到发布上之后，手动触发 `package-definitions-windows.yml` 渲染并上传构建产物 `msime-package-definitions-windows-<版本>`（含 `.nupkg`），不向任何外部仓库推送。`scripts/test-windows-package-managers.py`（由 `scripts/run-checks.sh` 自动运行）核对模板里的安装包事实与 `installer/msime_setup.iss`、`installer/editions.iss` 和发布流程一致。

上架之后的安装方式：`winget install Metasequoia.MetasequoiaIME`；`scoop bucket add msime https://github.com/metasequoiaime/scoop-bucket` 后 `scoop install msime`；`choco install msime`。各仓库的发布步骤（winget-pkgs PR、Scoop bucket、`choco push`）见 [packaging/README.md](packaging/README.md#发布步骤)。

## 原生界面渲染与皮肤

候选窗、悬浮工具条、托盘菜单和 `msimeui` 的面板 demo 都通过 `msimeui` 的 Direct2D 设备资源绘制，本仓库不引入第二套 D2D 路径。几何与配色从已发布 Windows 呈现器移植：`CandidateCardSize.h` 提供卡片尺寸、行矩形与命中测试的唯一来源，`CandidatePalette.h` 解析皮肤清单里的 CSS 颜色子集（三位/六位/八位十六进制、`rgb()`/`rgba()`、`transparent`），无法表示的写法保留内置 token 而不是渲染出不可见窗口。候选卡片用 DirectWrite 实测预编辑与每个候选的宽度后合成尺寸，工作区一半封顶两个轴；绘制和命中读同一份 metrics，点击不会落到渲染器没画的行上。固定候选只在未高亮时使用 accent；进入高亮行后，候选正文、辅助码和翻译整体改用选中行正文颜色。

皮肤经 `msime_client_skin_catalog` 从共享目录进入原生宿主，Server 解析可选的 `appearance`（`skin_directory` 绝对路径、`skin`、`dark_theme`）后下发给候选窗和悬浮工具条。兼容性以清单为准：没有声明当前布局和主题的包保留内置 token，不做半套应用；目录不可读、id 未知或条目畸形都不改变主题，也不让运行中的 Server 失败。独立面板以各自的内置 token 作为缺省值，同时接收解析后的共享调色板。

Direct2D 的成像工厂是 COM 服务器，这些窗口各自进入套间（`S_FALSE` 仍需配对释放，`RPC_E_CHANGED_MODE` 不动其他模式）；面板在 `--help` 返回之后才进入套间，冒烟运行器不需要额外条件。

## 候选与模式展示接口

WindowsServer::mode_view() 提供独立于组合的当前宿主模式视图，包含精确焦点 lease、中英、标点和全半角可选状态。激活未报告的字段保持 unknown；StatusSnapshot/FocusRestored 填充全部字段，单项通知仅更新对应项。新焦点不继承旧状态，断开/停机清空；读取尝试获取锁并校验连接，忙碌返回空，不等待管道。request_mode 的 Sent 不更新该视图，必须等宿主通知；悬浮工具条按钮的状态就取自这个视图。

windows-server-smoke 将真实隔离 Named Pipe、KeyHandler、确认候选快照、原生窗口合成鼠标消息及单任务后台选词连在同一测试中，检查 worker 提交帧、内部确认、提交后隐藏和前台窗口不变；空格键提交测试一并保留。该程序在 Windows 与 Wine 下运行（`run-tests-wine.sh`）；合成鼠标消息覆盖的是窗口消息路径，真实鼠标与编辑器交互由 `experiments/tsf-edit-control/` 的受控编辑器承担。

SessionPump 按已登记 Main 连接保存 ClientActivated.keycode 表达的 UILess 状态；仅接受路由后更新，接受失活时清除。后续键/显示/移动/隐藏包的有效 UILess 为激活状态与单包标志的并集，并传给输入处理和展示回调，不修改协议布局。激活重复通知切入 UILess 时立即抑制已有窗口；切回普通模式等待新输入或有效显示，不复活已经取消的组合。不同连接不共享该状态。

MoveCandidateWnd 带 UILess 时立即抑制 Server 候选展示并更新坐标，隐藏快照不携带文本，点击被拒绝；这表示宿主接管绘制，不取消 Engine 组合。后续普通移动不自行恢复窗口，非 UILess 显示事件可恢复尚有效的同代快照。已取消或英文模式清空的内容仍不可恢复。这里处理的是事件自身携带的 UILess 标志，宿主生命周期的其余部分由焦点路由和会话泵负责。

收到当前焦点的 IMESwitch / StatusSnapshot / FocusRestored 英文模式通知后，输入队列先同步 Engine 模式，候选缓冲随即清空并隐藏。切回中文或显示事件不能恢复旧组合，必须等待新输入结果；标点模式通知不清空候选。request_mode 发出请求本身不等于宿主已应用模式，不据此提前改写展示。

窗口读取候选时尝试获取焦点锁和候选缓冲锁；忙碌即返回空值，暂时隐藏并在下一次刷新重新读取，不复用旧焦点内容、不等待持有焦点锁的慢管道写入。连接校验保留在焦点锁内，使用专用 try_current 同时尝试获取注册表和连接锁，避免等待不经过焦点锁的握手 I/O。该接口的 false 也可能只是忙碌，只供展示使用，不据此断线或拒绝输入事务；正常输入仍使用 current。这是一条只读的展示快捷路径，不是整个 UI 的无锁或实时延迟保证。

当前 lease 的 HideCandidateWnd 先进入约 24ms 的短暂 grace；若期间没有新的确认投递或 Show/Move 事件取消它，则在读取快照时取消 Engine 组合并清除已选前缀，将候选缓冲变为无文本、不可见。这样可吸收拥塞 worker 晚到的旧 Hide，避免新一词候选显示前闪过空窗口；保留焦点及连接，不发送伪造提交或从 TSF 包重放输入。ShowCandidateWnd / MoveCandidateWnd 更新锚点，但不能复活已经应用的隐藏；新确认键回复才能发布新组合。显示仍尊重 UILess，移动不解除隐藏。取消拒绝过期 lease，也不能越过待确认回复；Main/UI 事务锁保证事件按序处理。该语义对齐固定 Windows 的 candidate-hide grace 路径。

候选窗口支持左键按下/抬起同一候选后提交选择。命中按已绘制快照与 DPI 行布局计算，不用未绘制的新候选替换用户点击的内容；页/代次变化、隐藏、取消或 DPI 重排会使旧按下失效。CandidateClickWorker 使用一个后台线程、最多一个未完成请求；忙碌/停止时拒绝新点击，不积压、不自动重试。窗口回调仅复制身份并更新短锁保护的任务状态，不等待管道；后台调用控制器再次验证可见候选与焦点。正常或异常退出均先停止接收并请求 Server 停机，再等待任务退出。

输入队列提供 select_candidate / ui_delivered，会话侧在有效 lease 内核对 session、generation 和当前页全局 index，再调用共享 Engine 选择。忙碌或过期点击返回空值且不推进状态；已准备的 UI 回复保留在 PendingReply::ui_selection，完整提交清空前缀、部分组词累积前缀都只在确认后生效。UI 确认使用选择后的视图 generation，不能用线上 request_id=0 作为可重复确认标识；普通 confirm_delivery 拒绝 UI 项。未确认时不得调度下一次 Engine 操作，编码异常由队列失败停机处理，不能捕获后重试选择。控制器提供选择事务入口，窗口事件与后台调用层接在它上面。

WindowsServer::request_selection 只能从外部 I/O 线程调用，串行完成视图核对、队列内准备、管道发送、队列确认及候选发布；Sent 仅证明完整写入和内部确认，不证明 TSF 已插入文本。共享事务锁覆盖 Main 包处理的整个准备/发送/确认阶段，但不覆盖阻塞读管道；点击抢锁失败返回 Busy 且不排队、不重试，模式请求忙时返回 Rejected。确认后的展示回调中，UI 结果带 ui_selection，packet 是保留屏幕锚点的合成零请求号包，不代表真实按键。写入或队列失败关闭连接并停止控制器，防止留下半完成选择。控制器串行化所有客户端事务，因此一次慢写会延迟其他 Main 包的处理。

点击回复线格式提供 ui_complete_selection / ui_partial_selection / ui_rejected_selection：完整文本仅进 worker CommitCurCandidate；部分组词或越界先发普通管道 id=0 的专用回复，再发空 worker 触发帧。UiSelectionDelivery 在有效焦点锁内按序发送，首帧失败不发触发，任一写入返回失败或抛异常都使连接失效且不重试。它是投递机制，不执行 Engine 选择、不校验候选代次，也不确认 TSF 已上屏；调用方必须先在输入队列准备持有待确认状态的选择，再投递并确认。键回复编码仍拒绝零请求号；候选窗口的点击经控制器走这条投递路径。该顺序来自固定 Windows 6e03f5774777e40c921930fd90a76e5425c66d89 的 UiCommitCandidate / _HandleCandidateFinalize 路径。

Server 主线程创建 `CandidateWindow`，显示预编辑、当前页序号与高亮；通过 NOACTIVATE/TOOLWINDOW 样式及 WM_MOUSEACTIVATE 拒绝鼠标激活，不抢输入焦点。字体族、字号与配色来自皮肤和候选设置，缺省为按 DPI 缩放的 Segoe UI，长文本省略，位置限制到最近显示器工作区。主线程以最长 50ms 空闲等待轮询状态，消息批次有界，快照身份未变时不重复触发绘制；每次 WM_PAINT 重新读取候选，失焦/UILess/断开/停机隐藏，绘制错误关闭窗口。模式状态与工具入口由悬浮工具条和托盘菜单提供，不挤进候选窗。

窗口使用临时线程 Per-Monitor V2 上下文，创建/布局/绘制结束后恢复调用者设置，不修改进程 DPI 默认值；该上下文要求 Windows 10 1703 或更新版本。坐标按固定 TSF 上游的物理屏幕锚点消费；换屏时先隐藏迁移，再按 GetDpiForWindow 重新计算宽度、行高、间距和字体。WM_DPICHANGED 使布局失效，下一次刷新按当前锚点重新定位，避免在 SetWindowPos 回调里递归布局。`windows-candidate-card-size` 独立验证卡片尺寸、行矩形与命中测试，覆盖多候选页、负屏幕原点、极小工作区和整数极值。

控制器内置 CandidateMailbox，默认接收确认后的快照，同时保留外部展示回调。WindowsServer::candidate_view() 供外部窗口线程读取最新值；单槽覆盖更新，不积压逐键消息。读取复核焦点、连接和输入队列状态，空值表示应隐藏；UILess 等也可能返回 visible=false 的隐藏快照。旧 ticket 断开不清除新连接，request_stop 清空并永久关闭缓冲区，迟到投递不能重开。输入/控制器回调禁止调用读取接口，避免焦点锁重入；锁内不调用 UI。此接口只保证读取时的有效性，窗口每次绘制重新读取，不能把缓存快照当作稍后点击的授权；点击命令由后台提交并在控制器重新校验。

WindowsServer 可注入 SessionPump::Presentation，经控制器和连接 worker 传入会话泵。delivered 在回复完整发送并确认后运行，本地预编辑无回复帧时也在确认后运行；回调位于输入队列并持有有效焦点锁，只允许复制有界快照，不允许窗口操作、I/O、Engine 调用或焦点门禁重入。写入失败不发布候选，回调异常停止输入队列且不重放按键。

CandidatePresentation.h 将回复投影为带焦点 lease、会话/代次、坐标、已选前缀和候选身份的值快照；预编辑最多 4096 字节、候选最多 9 项且每项最多 4096 字节，核对候选代次与高亮。UILess、失焦或组合结束仅输出隐藏快照，不携带候选文本。disconnected 在输入队列完成清理后通知匹配 ticket；队列失败时不保证通知，消费者还必须处理 Server 停机和其他焦点事件，显示及点击前重新验证 lease，不能让旧连接清空新窗口。原生候选窗口与点击消费者就挂在这个快照上。

`ServerSession` 由 Server 输入队列线程创建和销毁，不可复制或移动；所有操作检查线程。上层路由器须在构造前完成客户端认证与协议握手，并分配递增的 activation epoch；适配器拒绝错误客户端、未聚焦、过期代次、非法请求 ID 和长度。新的激活代次先取消旧组合。键结果携带 client/epoch/request 元数据，供后续回复队列在发送前再次验证所有权，不能绕过路由检查直接发给当前任意客户端。

输入文本使用 TSF 已按当前键盘布局转换的 wch，不在 Server 重跑 ToUnicode，不用 TSF 的 pinyin_string 覆盖 Engine 组合状态。数字小键盘交给共享数字选词，UiLess 标志不当作修饰键；既有 TSF 的 Shift/Escape 本地消费通知只取消后端，不生成按键回复。共享候选代次和配置延迟应用接口直接复用，不复制其规则。

## 语音输入与本地验证

`VoiceInputSession::start_review` 为面板提供独立结果对象与录音电平、识别/润色阶段；该模式不显示原生浮层、不发送行内组合、不执行 TSF/SendInput/剪贴板自动上屏或失败回退。结果限制为 v2 允许的 UTF-8 大小，取消后晚到结果不能恢复，`stop_review` / `cancel_review` 只作用于匹配对象，不能取消后续录音。`windows-voice-review-result` 覆盖结果状态、竞态与提交隔离。

识别服务选 `local` 时，Server 不联网，模型为 `voice_input.asr_model_path` 指向的已安装目录。录音开始时即在识别任务上加载模型，采集线程只把音频放进队列，由该任务边录边用共享的 `msime::voice::LocalAsrSession`（`shared/voice/LocalAsr.h`）解码；识别中的文字与豆包流式识别走同一条路径显示（允许内嵌预编辑时写入组合串，否则显示在语音浮窗上），录音结束后取最终文本。`asr_model_path` 若是旧的 Whisper 模型文件而非已安装目录，录音结束后走批量的 `recognize_local_asr`，识别会失败并提示模型不可用（Windows 从未带 Whisper 识别器）。热词取自 `msime_client_voice_hotwords`（用户自己的拼音词条）；模型清单 `msime-model.json` 写明 `"hotwords": "pinyin"` 时，最终文本再经 `msime_client_voice_hotword_correct` 校正。未选模型时开始录音即提示去设置下载；运行时或模型不可用时给出对应提示。已加载的模型空闲 120 秒后由 `maintain()` 交给工作线程卸载。运行时 `sherpa-onnx-c-api.dll`、`onnxruntime.dll`、`onnxruntime_providers_shared.dll` 由 `Build-Client.ps1` 取来并放在 Server 同目录，安装包随 `server_exe` 一起安装。

`VoiceControllerProtocol.h` 直接消费 `shared/contracts/voice_controller.h` 的 v2 布局，处理有界消息、UTF-8 和语言标识，不复制 opcode。`VoiceControllerConnection.h` 在专用连接上执行 Hello、OS 对端认证、请求顺序和有界读写，拒绝重放；它不接受客户端提供的 TSF 目标身份。`windows-voice-controller-protocol` 可在非 Windows 运行，`windows-voice-controller-connection` 使用真实 Windows Named Pipe。

Server 主循环创建 `VoiceControllerListener`：专用 I/O 线程通过单槽 `VoiceControllerMailbox` 向控制线程派发，`VoiceControllerDispatch` 分配不可跨连接复用的会话 ID，保存并复核 Server 内部焦点租约。断线、焦点失效、派发超时使对应会话失效，旧队列任务不能启动新录音；关闭时先中止并等待 I/O，再在控制线程取消匹配结果。监听失败不影响原生输入。当前一个认证连接独占端点；Hello/写回复限时 2 秒，空闲读取 5 秒，等待控制线程回复 10 秒，因此控制端应小于 5 秒轮询一次。I/O 超时后会取消并排空操作，不承诺硬实时截止。`windows-voice-controller-dispatch` 为可移植的所有权/焦点/队列测试；`windows-voice-controller-listener` 在 Windows 通过真实管道和模拟录音后端检查 Hello→Start→Stop→Poll、断线清理与未派发请求的关闭。

共享 `client-core::voice_controller` 注入消息传输，校验精确 v2 帧、请求/会话 ID、阶段顺序和有界 UTF-8 结果；Windows `host-windows::voice_controller` 使用消息管道与 overlapped I/O，校验并保留 Server 的进程句柄，核对同账户/同 Windows 会话。每次 I/O 限时 12 秒并在取消后排空，消息不自动重试。Tauri Windows `recognize_voice` 接线到该控制端，每 100ms 轮询，复用现有 request_id、停止/取消标志、电平及 `voice-update` 事件；结果仅返回面板，不自动提交。原生 `Processing` 映射到面板的 `polishing` 阶段。

依赖 CMake 3.25+、C++17、nlohmann-json 3.11+ 以及为运行平台构建的 msime-host-api。下面这组测试在 macOS/Linux 本机驱动真实的共享 Rust/C++ 库，用于在没有 Windows 机器时检查跨平台的会话与编码边界：

```sh
cargo build -p msime-host-api --locked
cmake -S platforms/windows -B target/windows-boundary -DMSIME_HOST_LIBRARY=/absolute/path/to/libmsime_host_api.dylib
cmake --build target/windows-boundary
ctest --test-dir target/windows-boundary --output-on-failure
target/windows-boundary/windows-session-smoke /absolute/verified-resources
```

Windows 构建时 MSIME_HOST_LIBRARY 应指定同架构 Rust DLL 的导入库，运行时需可找到对应 DLL；Linux 本机边界测试使用 .so。Windows MSVC 的完整构建入口是 `Build-Client.ps1`（见 `Build-Client.md`），MinGW 交叉构建不替代它；两者产出同一组目标。

`bash tests/tools/check-cross.sh /absolute/nlohmann-include-root` 使用 x86_64/i686 MinGW 分别编译适配器、测试源和 `shared/contracts` 中的 IPC 契约，验证 32/64 位 COFF 和 Windows SDK 键码断言；另将不依赖 Rust 的编码测试链接为 Windows PE。不会链接 Windows Rust 库，也不执行 Windows 二进制。会话测试覆盖真实 Unicode 输入、锁定词库第二页数字选词、配置延迟、客户端/焦点/线程拒绝及本地取消不回复。

## 旧协议回复编码

`ReplyCodec` 直接构造上游定义的 Normal、CommitExactText、Preedit、NavigationIgnored、NeedToCreateWord 和 UiLessComposition。Normal 用于既有候选完成路径，CommitExactText 表示无需 DLL 再补标点的完整提交，不能将两者无条件互换。分段格式为 remaining-raw、整个已选前缀、显示预编辑，以 tab 分隔；UILess 为显示预编辑、逗号分隔当前候选页、页内高亮序号，以 tab 分隔。候选包含协议分隔符时明确报错，不伪造转义或静默删除候选。

UTF-8 严格转换为 UTF-16，拒绝过长编码、孤立代理、超范围标量和嵌入 NUL；按既有 199 个 UTF-16 单元容量检查，非 BMP 字符计两个单元，不截断。`wire_bytes` 显式按小端写入协议成员和零填充字节，发送方不直接发送 C++ 结构体内存。失败结果不能生成可发送字节。macOS 本地编码测试覆盖精确布局、字节序、代理对、尾部清零、长度边界、坏编码和字段歧义；真实共享会话的完整提交也接到编码器测试。

编码器只负责已选定语义的表示，不自动决定每个 TSF 输入路径应读取何种回复，也不维护分段输入状态。

## 分段回复编排与发送确认

`ReplyComposer` 按已认证 client/activation 创建，将共享运行时的增量选词结果累积为旧 DLL 需要的整个已选前缀。该前缀只是旧协议尚未上屏的展示状态；输入字符、剩余拼音和候选选择仍归 Engine。Selection 路径在有剩余输入时生成 NeedToCreateWord，全部完成后生成含完整前缀的 Normal；Punctuation 路径发送完整 CommitExactText，预编辑/UILess 路径在显示文本前保留已选前缀。

PreviousCandidate/NextCandidate/PreviousPage/NextPage 路径消费共享导航结果：普通模式发送 Engine 契约中的对应导航 opcode 和空文本，即使已在边界也保留导航意图；UILess 返回共享视图的完整候选页及高亮。导航不清除已选前缀、不允许夹带 commit，确认前仍禁止下一次 Engine 操作。配置驱动的导航键分类仍由后续原生策略接入，本层不复制分页算法。

正常按键通过 `dispatch` 进入共享会话，它在推进 Engine 前检查上一条回复是否仍待确认。`stage` 保存原始 KeyResult、待发送帧和下一前缀；完整写入后才调用 `confirm_delivery` 更新前缀。写入失败或结果不确定时保留 pending，不得再次执行原始输入，也不能在未确认是否已送达时盲目重发。编码失败保留原始提交，并禁止成功确认；失焦或明确的传输取消才调用 cancel 丢弃旧路由状态。发送端仍须在实际写入前检查最新路由所有者，局部的确认元数据匹配不能替代该检查。

旧 TSF 的 Enter 本地完成不应收到第二次上屏帧。LocalCommit 要求调用方提供实际本地完成文本，与前缀加共享提交匹配后才允许无帧确认；缺失或不匹配保留错误结果。LocalCancel 清除前缀而不回复，NoReply 保留未完成前缀。每个 TSF 路径选择何种 ReplyPath 由原生分发层决定，不能单凭 VK 数字判断选词，例如 Unicode 模式数字仍是编辑。

本机三项 CTest 和真实固定词库回归通过。真实 nihao 会话先选“你”产生剩余输入和 NeedToCreateWord，再选“好”产生唯一完整 Normal“你好”；测试还验证待确认时再次 dispatch 不推进 Engine。独立编排测试覆盖多段前缀、标点、UILess、Enter 文本一致性、取消、错误确认与超长结果保留。

## 测试与 CI

`platforms/windows/CMakeLists.txt` 注册 96 个 CTest，`tsf/CMakeLists.txt` 另有 19 个，`tests/native-pipe/`（仅 Windows）2 个，`msimeui/tests/` 1 个；`tsf/tests/exports/`、`tsf/tests/registration_profiles/`、`tsf/tests/registration_categories/` 和 `tests/server-manifest/` 是各自 configure 的独立子工程。PowerShell 侧另有 `tests/tools/*.ps1` 与 `installer/tests/*.ps1` 覆盖构建编排、PE 门禁、通知收集、运行器控制和安装器编排。

`bash platforms/windows/run-tests-wine.sh x64` 把交叉构建出的 C++ 套件（`windows-*.exe`、`msime-tsf-*.exe`、`bin/msimeui-tests.exe`）和 `cargo test --no-run` 产出的 Rust 套件一起放在 `xvfb-run -a wine` 下执行，每个 120 秒超时，结果与 `scripts/known-failures.txt` 比对；不带 `--quick` 的 `scripts/verify-local.sh` 会自动调用它。

CI（`.github/workflows/ci-platforms.yml` 的 windows job）在 `debian:trixie-slim` 容器里跑 `build-cross.sh x64`——Ubuntu 24.04 的 MinGW 头文件缺 `d2d1_3.h`。`release-windows.yml` 是手动 `workflow_dispatch`，在 Windows runner 上按版本矩阵（full、wubi、pinyin）各跑一遍 `Build-Client.ps1 -Edition`、打包和装卸冒烟，再用 `installer/tests/coexistence-smoke.ps1` 把几个版本装到同一台机器上，检查它们并存、卸掉一个版本不碰 full，最后一起发布。安装器由 Windows 上的 `installer/Package-SimplySign.ps1` 编译和签名，不在 CI 里产出。

## 管道 I/O 与进程身份绑定

`PipeIo` 在工作线程执行固定帧消息读写，连接须由调用者保证为 overlapped、消息读取和 PIPE_WAIT 模式，且不能并发关闭或重新连接。短帧、超长帧、断连、超时和取消均不产出可用输入帧；取消后等待 I/O 完成再释放缓冲区。未成功的已提交写入可能已经影响对端，不得自动重发；失败连接须关闭，避免残余消息污染下一帧。

`PipePeer::bind` 仅接收服务端管道句柄，读取系统提供的客户端 PID 和登录会话，验证 client_id 的高 32 位、当前 Server 登录会话及进程 TokenUser 的账户 SID。查询或进程访问失败直接拒绝，不降级放行、不请求调试权限。成功后保留不可继承的进程句柄；`matches` 检查完整 client_id、绑定进程是否仍存活以及主/反向管道 PID 和会话是否相符，避免已经建立的绑定在原进程退出后转到复用 PID 的其他进程。

调用方必须先配置正确的生产 DACL 与 PIPE_REJECT_REMOTE_CLIENTS，在客户端存活时完成绑定，并在路由/端点生命周期保护下进行复核。进程绑定不证明同进程中的具体线程，不检查程序签名，不替代协议版本协商、管道角色、registration/activation epoch 或输入焦点授权，也不消除复核后进程立即退出的可能。受保护进程、UAC 提升进程、AppContainer 与跨会话客户端各有自己的访问规则，不能为了兼容直接跳过拒绝检查。

管道子集的独立测试入口：`cmake -S platforms/windows -B target/windows-pipe -DMSIME_WINDOWS_PIPE_ONLY=ON`，随后 `cmake --build target/windows-pipe --config Debug` 与 `ctest --test-dir target/windows-pipe -C Debug --output-on-failure`。它不需要 Rust 库，也不注册输入法，因此适合只验证帧 I/O 与身份绑定；`verify-local.sh` 在有 MinGW 的非 Windows 主机上对 x86_64 和 i686 两个架构分别配置这一子集，用来锁住 `windows_ipc.h` 的帧大小与字段偏移 `static_assert`。

键码依据 [Microsoft Virtual-Key Codes](https://learn.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes)，线格式直接包含 shared/contracts，不另造 opcode 或改协议能力声明。

## 主连接与反向端点握手

`accept_reverse` 按固定大小读取 FanyImePipeHello、检查监听端预期角色并绑定真实进程，发送该角色专属的 PipeReady 后才返回 Ready 和进程绑定。回复端点是 416 字节，worker 端点是 404 字节；确认帧显式写字段并清零填充，不发送 C++ 内存填充区。此函数不修改全局注册表，调用者只能在成功后发布端点，失败需关闭端点。

`accept_main` 依赖已经 Ready 的 ToTsf 回复端点及其进程绑定，读取主连接 ClientHello、复用固定 Engine 的 Negotiate，再次复核两条端点身份后，经回复端点发送 ProtocolReady 或 ProtocolMismatch。不带版本号的 hello 被拒绝；非法客户端或无法表示的请求 ID 不发送确认。只有 HandshakeStatus::Ready 可进入下一注册步骤，单独的 negotiation.accepted 或 io.complete() 不代表握手成功。能力位必须由已实现的分发层显式提供，没有默认启用语音或字符集快捷键。

两函数仅用于 I/O 工作线程，超时按每次 I/O 计算；调用方须保证借用句柄有效，在握手期间排除同端点的其他写入、关闭与替换，并把返回结果绑定到同一 registration generation。这里不建立路由注册表，也不赋予焦点所有权。主握手传入的回复管道必须确实是已注册 ToTsf 而不是 worker，不能只因 PID 相同就任意替换；生产入口由 `PipeService` 的 intake 回调把成功登记交给 `RegistrationInbox`，再由 `SessionController` 消费。测试覆盖编码字节、两类 PipeReady、版本协商、未实现的必需能力拒绝、不带版本号的握手被拒且无 ACK、错误客户端/角色及预取消；编码部分可在任意主机跑，管道握手部分需要真实 Named Pipe，因此在 Windows 或 Wine 下执行。

## 安全监听与连接所有权

`PipeListener::create` 仅接受本机管道名，从当前进程 TokenUser 构造受保护 DACL：当前账户与 LocalSystem 完全访问，AppContainer 只有上游定义的 0x12019b 连接权限，不包含创建管道实例权限；保留低完整性标签，并设置 PIPE_REJECT_REMOTE_CLIENTS。账户获取失败不降级为默认 ACL。当前账户是这条边界的信任范围：它不隔离同账户下的恶意进程，也不提供跨会话访问。

首次创建带 FILE_FLAG_FIRST_PIPE_INSTANCE，名字已占用时明确失败；不终止旧 Server 或夺取既有产品管道。监听器保留待连接实例，accept 成功后先创建替代实例，再返回独占 RAII PipeConnection，避免全部业务连接关闭导致名字失去持有者。返回连接保持 overlapped、消息读取和 PIPE_WAIT 模式，32 KiB 双向缓冲区为内核提示值，不是无限队列；Server 后续还必须设置连接数量和工作队列上限。

accept 只在单一监听工作线程调用，使用手动事件等待连接，处理客户端先连入的 ERROR_PIPE_CONNECTED。超时/取消会取消并等待操作完成，再断开可能竞态连入的客户端，保留句柄供下次监听；它不是硬性返回时限。析构前须停止 accept，并保证连接没有未完成的借用 I/O。连接成功不等于可信客户端，仍必须经过握手、进程绑定与注册代次检查。

原生测试使用唯一测试名称调用实际监听器，覆盖预连接和异步连接、超时/取消后的恢复、重复名字拒绝、连接保留名字、句柄不可继承及内核 DACL 的 AppContainer 权限和无 Everyone ACE。帧、身份与握手用例都用同一监听器；它们使用唯一测试名，不创建生产管道名，也不安装或注册 TSF。

## 有界端点注册与传输票据

`PipeRegistry` 接管 PipeConnection 所有权，按客户端保存主、回复和 worker 三条连接。反向端点握手后才能登记；主入口接收 intake 工作线程已按精确帧读取的 ClientHello，核对主/回复/worker 的进程身份，并在该客户端锁内执行协议确认和发布。握手前不允许普通发送，失败不会保留可用于输入的主注册。独立客户端不共用长时间 I/O 锁；同客户端写入与端点替换串行化。

PipeTicket 保存三条端点的不可复用注册代次；发送须匹配完整票据，清理只删除指定角色的匹配代次。反向端点变化使主注册失效，必须重新主握手；旧任务不能发往替代连接，也不能通过过期清理关闭替代连接。主读取持有共享端点引用但不占用客户端锁，移除/重连发出取消信号；I/O 完成后再验证注册与身份，旧帧不返回给调用方。写入失败停用该发送端点与主注册，不自动重试。

容量限制针对已登记客户端数量；握手尚未识别 client_id 时由外部 intake 保有连接，因此 Server 仍须限制待握手连接和工作线程。verify_reverse 只验证身份并返回 Verified，不发送成功确认；注册器先检查容量和代次，再在客户端锁内发送 PipeReady 并发布端点，避免客户端收到确认后主连接却查不到登记的竞态。容量不足或过旧的并发注册直接关闭，不发成功确认。通用 `PipeRegistry` 没有默认后台线程或全局 Server；生产 `WindowsServer` 由 `PipeService`、有界收件箱和 `SessionController` 提供这层装配。shutdown 先停止新登记并取消已登记读取；外部握手使用调用方取消事件，调用方必须停止 intake、取消握手、等待所有 I/O 工作线程退出后再析构注册器。

这里的 Ready 和票据只证明传输登记，不授予输入焦点，也不确认客户端已经处理上屏。进入 Engine 队列和发送前的 activation/focus 校验、FocusSessionReady 与 ReplyComposer 确认由上层的焦点门禁和会话泵负责，见下文。原生测试连通监听→反向注册→主协商→注册器读写，覆盖未主握手禁止发送、代次错配、重连取消待读取、旧发送/旧清理拒绝、容量耗尽与回收。

## 有界握手工作池

`PipeIntake` 取得监听连接的所有权，使用固定 1–32 个线程及 1–1024 个等待槽位，由调用方选定。队满、角色非法或停止后直接拒绝并关闭新连接；池内未完成连接最多为等待容量加线程数，监听实例和 Registry 已登记连接另计。不为每个连接新建线程，也不在握手线程长期运行主管道输入循环。

主 hello 精确读取后交给 Registry，反向端点按角色登记。完成回调必须短且非阻塞，将结果放入下游有界队列；返回 false 或抛错时，池按代次撤销未交付登记，不误删替代端点。统计只含数量，不记录输入、帧或异常正文。控制线程 stop 会停止接收、清空队列、取消握手并 join；stop 返回后没有回调，已开始的回调可能在 join 期间结束，因此依赖须保持存活，禁止在回调内 stop 或析构池。

停止 intake 不删除已成功交给下游的登记。生产 `WindowsServer` 先停止监听和 intake，再由 `SessionController` 关闭 Registry I/O、等待输入线程和连接 worker 退出；通用池仍要求调用方保证这一顺序。超时/取消需等待实际 I/O 完成，不承诺硬性返回时限。原生测试覆盖静默客户端、队列饱和、停止清理、三角色投递及回调拒收/异常。

## 三角色传输服务装配

`PipeService` 拥有 Registry、一个有界 PipeIntake 和三条独立监听线程。调用方显式提供 Main/ToTsf/worker 名称与已实现能力位，不会默认发布产品管道名。启动时先验证工作池配置并建立监听器，任一步失败都会取消并等待已启动的线程、释放已取得的管道，不接管占用名。回调依赖必须在构造前准备好，监听线程开始工作后即可触发回调。

各监听循环把连接所有权交给同一有界池，队满连接由池关闭。空闲超时和客户端在 accept 前断开可恢复，不可恢复错误仅保存首个 Win32 错误码并 request_stop 全部监听和握手；控制线程观察 failure 后调用 stop 完成 join 和回收。request_stop 不等待，可从完成回调请求停止；stop/析构只能由控制线程调用，不能在完成回调内等待自身退出。

stop 幂等地停止监听、等待握手池、shutdown Registry 并释放监听器。若外部线程使用 registry() 读取输入，stop 会取消其注册 I/O，但外部线程仍须 join 后才能析构整个服务。客户端仍持有旧句柄时，旧管道名可能仍占用，重启仍按首次创建规则失败，不绕过占用保护。

原生测试不手动调用 accept/submit，而用三条实际监听连接完成反向确认和主协商；另外覆盖重复启停、幂等 stop、第三个名称占用导致启动失败及前两个名称回收。这个类本身只是传输服务库；焦点状态机、Engine 输入队列和会话 worker 由 `WindowsServer` 装在它外面。

## 主管道空闲等待

已登记主管道的 `read_main(ticket)` 默认通过 `read_frame_until_cancel` 等待输入，不因用户空闲而按固定时间注销主注册。这个入口必须带有效取消事件，重连、移除及 Registry shutdown 会发出取消并等待 I/O 完成；握手 `read_frame` 和所有写入仍拒绝无限超时。显式指定有限 read_main 超时只用于诊断，超时后仍弃用该连接，避免取消竞态下丢失的消息被当成下一帧。

原生测试覆盖空闲等待后正常读取、重连取消长期待读取、直接取消，以及禁止无取消事件长期读取和无限握手/写入；测试清理也主动取消，避免断言失败时 future 析构无限等待。

## Worker 焦点确认编码

`focus_ready_bytes` 生成固定上游 FocusSessionReady worker 帧，将已认证的非零 client、activation epoch 和 focus token 中的 token 以无区域设置影响的十进制 UTF-16 编码；支持完整 uint64 范围，拒绝任一零标记，保留终止符并清零全部剩余字节。activation epoch 不是传输注册代次。编码函数不授予焦点，也不自动激活 Engine。

本机编码测试验证零拒绝、跨 32 位值和 uint64 最大值的精确字节；原生注册器测试另做真实 worker 路由发送与完整帧读取。焦点激活状态机见下一节。

## 激活确认门禁

`FocusGate` 保存一个当前 FocusLease，分别携带完整传输票据、Server 单调 activation epoch 和 TSF focus token。begin 返回旧 lease 与新 pending lease，供输入队列取消旧组合并激活新会话；它本身不操作 Engine。只有 acknowledge 的 worker 写入回调完整成功才进入 ready，失败或异常清除该激活；过期确认和重复确认不会执行写入回调。

with_active 在同一焦点锁内验证并执行动作，避免检查后焦点已切换却继续发送；动作须短或有界，不得递归调用该门禁。涉及管道时锁顺序固定为 FocusGate → PipeRegistry，注册器仍在写入时核对传输代次；不能只凭 focus lease 绕过注册验证。旧 lease 的 deactivate 和旧票据的 invalidate 不影响新激活。已确认激活仍不证明应用实际插入了文本，ReplyComposer 投递确认保持独立。

这只是状态与同步机制，不会从任意状态通知推断系统焦点。`SessionPump`/`FocusRouter` 消费可信的 ClientActivated、KeyEvent、FocusRestored、挂起与停用事件，以及旧/新 ServerSession 的输入队列切换；`begin` 单独不构成系统焦点授权。

`windows-focus-gate` 验证 pending/ready、坏票据、旧确认、失败/异常、失焦失效与并发动作互斥。原生管道测试把门禁组合到 worker 焦点确认和回复发送，并在重连后失效旧 lease。

## 焦点约束的输入队列会话

`FocusedSession` 为一个已登记客户端组合 ServerSession、ReplyComposer 与 FocusGate，全部方法必须在创建它的输入队列线程执行。控制器先 begin 新 lease，再在队列 prepare：只有当前 pending 激活能准备 Engine，新的 epoch 会取消旧组合并替换旧回复编排；准备成功后才把 worker 焦点确认交给 I/O 线程。适配器不执行管道 I/O，也不自行判断 OS 焦点。

key 必须匹配已准备的完整 lease，并在 with_active 内调用真实 Engine；确认前或过期任务返回空结果，不推进 Engine。ReplyComposer 的待回复门禁继续阻止重复执行。返回的 PendingReply 是独立副本，供 I/O 队列处理；复制/传输失败时可通过 pending 取回暂存结果而不重跑输入，但这不是不确定投递的重发许可。成功写入或已核实本地处理后，confirm 回到输入队列并重新检查 lease；旧确认不能推进新会话前缀。

cancel 只清理匹配 lease 的 Engine/编排状态，即使全局焦点已切走也可清理旧客户端，不会清掉新激活。控制器仍负责将 FocusChange.previous 的取消发送给旧客户端队列；不能只准备新客户端而遗漏旧组合。ReplyPath 与本地完成文本由实际 TSF 路径显式提供；候选点击、配置文件监听和生命周期事件策略装在这一适配器外面，分别由 `CandidateClickWorker`、`PreferenceMonitor` 和 `FocusRouter` 承担。

本机测试调用真实 Rust/C++ 会话验证 Unicode 提交结果与编码、确认前阻止输入、待回复时不重跑 Engine、暂存结果读取、过期输入/确认拒绝、新激活清空旧组合、过期取消与线程拒绝。这一层的焦点写入完成使用测试回调，真实管道写入由独立的原生管道测试覆盖。

FocusedSession 的 update_preferences 同样检查完整 lease 与队列线程；回复待确认时返回空结果，不修改 Engine，调用方应在确认后重试最新配置快照。没有待确认回复时直接复用共享层的版本与组词延迟规则，不在 Windows 重写它们。本机实际会话测试覆盖待回复拒绝、组词中 deferred、提交后应用和旧焦点拒绝；文件监控与队列重试调度由 `SessionController` 的 `PreferenceMonitor` 提供，见下文「配置投递后的自动重试」。

### Main 消息进入控制器前的校验

PipeRegistry::read_main 在返回完整帧前复核 packet.client_id 与登记客户端一致，并通过 MainFrame 校验已知 Main 事件、pinyin 长度及终止符、状态快照字段和非零激活 token。Aux 专用事件和未知 opcode 不进入分发；key 同时拒绝零与 NO_REQUEST_ID，激活 token 则允许完整 uint64 范围。失败返回 MalformedFrame 且不携带原始帧，注销匹配主注册，旧票据不可继续发送。重复 ClientHello 保留为后续控制器忽略的兼容事件，不重协商；这一校验不证明前台焦点，也不取代 lease 检查。

字段规则依据本仓共享 contracts 与 Main 接收路径，key 的 NO_REQUEST_ID 限制与本仓 ServerSession 一致。纯校验部分可在任意主机执行；真实管道测试另覆盖同进程伪造 client_id、越界长度与 Aux 事件导致连接弃用。

### 生命周期路由策略

FocusRouter 在单一控制/输入队列上管理有界登记表，并作为 FocusGate 唯一的激活策略写入方。connected 只接收 Registry 已协商且仍有效的完整 ticket；消费登记通知时仍须复核 Registry，不能重新加入已注销后迟到的登记。重复登记无操作，旧代次不能覆盖现存新链。disconnected/failed 返回精确 cleanup lease，旧连接或旧激活不能清掉新链；关闭时先断开登记、处理清理，再销毁队列和会话。

dispatch 按各 Main 流原始顺序处理消息。显式激活引入非零 token；同一当前 token 不重置 Engine，新 token 或其他客户端激活产生新 epoch。被挤走客户端只有已完成 worker 确认的 token 可用于 KeyEvent/FocusRestored 恢复；key request_id 从不冒充 token。StatusSnapshot 和候选窗/模式消息不抢焦点。当前客户端挂起清除恢复 token，之后必须显式激活；终止事件可获取挂起时的精确清理身份。后台挂起不改变前台归属，后台终止清理也不是隐藏新客户端工具栏的授权。策略依据固定上游 6e03f5774777e40c921930fd90a76e5425c66d89 的激活与失活路径。

FocusRoute.activation 表示新激活：先将 cleanup 交给旧 FocusedSession.cancel，再 prepare 新会话，I/O worker 经 gate.acknowledge 写焦点 fence；成功通知回到控制队列后调用 confirmed，再执行输入。fence 表示上游要求确认标记：pending 路由等待已安排的确认，不重复 acknowledge；ready 路由重发标记通过 gate.with_active 和 Registry 票据检查。准备、确认或发送失败调用 failed 并处理 cleanup，不能重放不确定写入。gate 切换时原子记录 previous_ready，避免遗漏已成功确认但通知尚未处理的 token。route 可能仍为 pending，不是立即执行 Engine 的授权，实际执行与发送仍复核 gate。

本机测试覆盖策略状态和真实 FocusedSession 跨客户端组合清理；Windows 管道测试串入路由、worker 确认与回复发送。生产控制器由 `WindowsServer` 装配并接管安装时注册的生产管道名。

### 专用输入线程与有界任务队列

InputQueue 的单一 worker 拥有 InputState、FocusRouter 和各客户端 FocusedSession，包含创建和销毁；从不把 Rust 线程局部会话迁移给管道 worker。InputState.connected 创建或更新登记会话，dispatch 自动执行旧 lease 清理和新 lease 准备，disconnected/failed 自动清理匹配会话。后续输入、回复确认和配置快照通过 key、delivered、update_preferences 进入同一线程；confirmed 只记录外部 I/O worker 已成功写出的焦点确认。key 前仍需 dispatch 得到正确路由；ReplyPath 由原生控制器提供，不能仅靠 VK 推断。

submit 非阻塞接纳任务，容量 1–4096 限制等待任务数，客户端容量 1–1024；捕获数据大小仍由控制器限制。满队列、停机或空任务返回空结果，调用方必须撤销相应传输/焦点，不能静默丢键或在 I/O worker 直接执行 Engine。每个已接纳任务都有 Completed/Failed/Cancelled future；Completed 只表示回调正常返回，不表示消息被接受、输入已上屏或网络发送成功。实际结果必须按任务返回的数据和会话接口判断。回调不得做管道 I/O、等待队列 future、保留 InputState/会话引用，或捕获可能过期的管道缓冲区；控制器应复制已验证的有界数据。

request_stop 可从回调调用，不 join；stop 由外部控制线程调用，等待当前短任务结束，取消未执行任务并结算 future，在原 worker 撤销路由和销毁全部会话。任务异常先清理全部会话、停止接纳，再返回 Failed，其余任务得到 Cancelled，不传播原始诊断。禁止在 worker 调用 stop 或销毁队列；对同一队列的并发外部 stop 会串行 join。Gate 及回调依赖必须活到 stop 返回，产品停机还应先停止外部发送/接纳，不能在队列结束后继续调度 I/O。

本机测试执行专用线程真实 Unicode 提交、焦点切换和停机销毁，另验证多生产者顺序、满队列、取消、异常和自 join 拒绝。`SessionController` 把 PipeService 接纳、读取、焦点确认、回复发送及失败回执串接到此队列；生产入口的控制循环和偏好监视路径负责配置发布与重试。

### 已登记 Main 连接处理循环

SessionPump 在外部 I/O worker 运行一个完整 ticket 的消息循环，通过 MainTransport 接口连接实际 PipeMainTransport/PipeRegistry。登记通知消费时检查 current，再在 InputQueue 创建会话；每条 Main 消息经过路由与 prepare、worker 焦点确认、队列确认、Engine 输入、回复发送、队列回执之后才读下一条。队列任务复制单帧数据，管道读取和写入都在外部 worker；禁止从输入队列调用 run，入口会拒绝自等待。

PipeMainTransport 的 read 复用 Registry 的可取消空闲等待与消息校验；send 要求显式有限超时，仅完整成功返回 true。is_current 只是登记快照，不证明进程存活或焦点；实际读写继续复核身份/代次，发送同时经过 FocusGate。正常焦点切换导致的过期任务和输出被丢弃，不视为连接故障；确实尝试但失败的写入、无法编码的回复、身份/请求不匹配的分发结果和队列故障终止循环，不重放输入或不确定写入。重复 hello 无操作。退出撤销匹配焦点与主注册，并在队列清理会话；若连清理也无法入队，停止共享输入队列，宿主还必须停止其他管道循环。

KeyHandler 在输入队列上使用真实 TSF 模式/消费路径选择 ReplyPath，并且只调用一次 state.key；测试里的 Unicode 专用分发不是生产 VK 推断器。EventHandler 必须显式提供，用于发布携带 lease 的模式/候选 UI 工作；有活动 route 时回调处于焦点锁内，不得重入 gate、调用 Engine 或执行 I/O。清理类通知没有新前台授权，不得隐藏其他客户端 UI；异步消费者仍须检查 lease。无编码帧的返回只能表示 ReplyComposer 已验证的本地完成或无回复路径，不能拿它绕过待回复门禁。

本机确定性传输测试运行同一个 SessionPump 和真实 Rust/C++ 输入线程，验证 Unicode 完整回复、确认顺序、失败不重放、错误路由拒绝、旧焦点输出丢弃、重复 hello 和自等待拒绝。Windows 原生管道测试使用实际 PipeMainTransport 检查读写与旧 ticket 关闭不影响新登记。`WindowsServer` 提供有界 worker 生命周期、接纳回调、取消/join、实际 KeyHandler/EventHandler、模式输出和配置重试。

### 有界连接工作线程

SessionWorkers 为 1–64 个固定连接槽位预建 I/O worker，每槽最多一个活动 SessionPump 和一个待替换 ticket。重复完整票据不会启动第二个读取者；同客户端重连取消旧读取并覆盖待替换票据，旧循环完成队列清理后原线程接替新连接。不同客户端超过槽位容量、旧代次或停机后提交会被拒绝并关闭匹配主注册，不按每次重连无限创建线程。

submit 供外部控制线程消费已协商的登记通知，先检查 MainTransport.current；取消可能等待有限时间的在途写入，不得直接放进要求非阻塞的 PipeIntake 回调或输入/gate 回调。有界登记通知入口由 `RegistrationInbox` 提供。request_stop 标记停止、关闭活动和待替换票据以取消读取；stop 从外部线程串行 join，不能从输入队列或自身 worker 调用。依赖的传输、Gate、输入队列和处理器须活到 stop 返回；正常退出顺序是停止接纳、取消/join 连接循环、停止输入队列，再释放传输服务。输入队列故障被循环观察到时会停止其余槽位；全部连接空闲时仍需宿主监控输入队列/服务状态并主动取消，不能依靠空闲读取自行发现故障。

本机测试组合实际 SessionWorkers、SessionPump、InputQueue 和真实会话，使用可取消的空闲传输验证并发读取上限、重复登记、容量拒绝、重连线程复用、连续替换合并、旧关闭隔离与并发 stop。`src/entrypoints/server_main.cpp` 用 `WindowsServer` 装配生产/隔离预览两套管道、`production_key_handler()`、偏好发布回调及 voice control listener。

### 原生服务装配与故障监控

WindowsServer 组合 RegistrationInbox、PipeService、PipeMainTransport 和 SessionController。构造需要显式管道名、能力掩码、共享宿主选项及 KeyHandler/EventHandler，会真正启动指定名称的管道；`--production` 入口传入安装时注册的生产管道名称。登记收件箱先于监听器构造，Main 握手通知只复制 ticket 入有界队列，满队列/关闭返回 false，由 PipeIntake 注销匹配登记；反向管道就绪不启动 Main 读取。构造失败时按成员依赖顺序释放已启动资源。

SessionController 持有输入队列、连接 worker 和独立控制线程，消费登记通知时由管理器再次验证票据。无通知时默认每 100ms 检查服务、输入队列和连接管理器状态，因此全部客户端空闲也能触发故障退出。request_stop 只置位并关闭/唤醒收件箱，可从输入回调调用；实际取消与 join 留给控制线程：停止服务接纳和握手、关闭 Registry 端点、join 连接循环，再停止输入队列。stop 由外部调用并等待整个顺序结束，禁止从自身控制线程或输入线程 join。故障仅暴露分类，不记录原始异常、输入或路径。

WindowsServer 的回调可能在构造返回前运行，捕获依赖须事先初始化，不能访问尚未构造完成的 server。通用 SessionController 的 healthy/stop_service 回调在控制线程运行；stop_service 必须关闭所有登记端点（包括尚未消费的收件箱票据）并等待服务线程结束，服务和收件箱须活到 controller 停止之后。

本机测试增加有界登记队列、空闲时服务故障、输入异常全局停机，以及持有焦点锁的事件回调请求退出。原生 windows-server-smoke 使用唯一测试名称运行实际 WindowsServer，发送反向握手、Main 协商、激活及 Unicode 输入，并验证完整提交和空闲停机。它需要完整的同架构 Rust 导入库构建（不能用 MSIME_WINDOWS_PIPE_ONLY），运行方式是 `ctest --test-dir target/windows-boundary -C Debug -R windows-server --output-on-failure`。测试里用的是固定 Unicode 处理器，产品的模式/候选 UI 走的是 `production_key_handler()` 和原生窗口。

### Windows GNU 完整链接构建

`bash platforms/windows/build-cross.sh x64` 在已具备 Git、MinGW、Rust、CMake 的主机上准备固定 vcpkg、安装锁定依赖，构建真实 Rust/C++ 宿主 DLL，再链接全部 Windows 原生测试（包含 windows-server-smoke.exe）。vcpkg 固定 ef7dbf94b9198bc58f45951adcf1f041fcbc5ea0；默认使用 target/tooling/vcpkg，缺失时由 bootstrap-vcpkg.sh 从官方仓库获取固定提交并关闭指标收集进行 bootstrap。会访问网络下载工具与依赖。也可设置绝对 MSIME_VCPKG_ROOT，显式提供的目录必须已准备好，脚本不重置或自动修复它。脚本会安装相应 Rust 标准库和清单依赖，依赖安装根按架构隔离，避免 vcpkg 切换 triplet 时移除另一架构的库；同一 vcpkg checkout 不并发执行。宿主库不需要 vcpkg 前缀，其中的 C 部分（rusqlite 自带的 SQLite）用同一套 MinGW 工具链编译；同架构依赖前缀只交给 platforms/windows 的 CMake（`CMAKE_PREFIX_PATH`）。

bootstrap 只管理默认工具缓存，已有错误版本、跟踪文件改动、符号链接或非预期目录均拒绝，不覆盖用户内容。目录锁拒绝并发准备；失败的独立 staging 目录保留供检查，不递归删除。若遗留锁，先确认原进程已结束再清理空锁目录。可单独执行 `bash platforms/windows/bootstrap-vcpkg.sh`，已有固定版本且可执行时复用；离线拒绝路径测试为 `bash tests/tools/bootstrap-vcpkg.sh`，测试只操作新建的隔离目录。本机 x86 SJLJ 工具链会在网络准备前被拒绝。

x64 宿主 DLL、会话测试及完整原生管道集成测试链接为 PE32+，产物位于 target/windows-full/x64。脚本只复制宿主 DLL，不打包 MinGW 运行时 DLL，运行前还需同工具链的 libstdc++、libgcc 和 libwinpthread 及系统运行时——交叉构建的产物是测试目录，不是安装包，发行安装包由 `installer/Package-SimplySign.ps1` 产出。

x86 的 Rust GNU 目标要求 DWARF 展开，而 Homebrew 的 i686 MinGW 用 SJLJ，`build-cross.sh` 在准备依赖之前就拒绝这个组合，不通过 panic=abort 改变既有错误隔离契约。在这类主机上用 `bash platforms/windows/build-cross-container.sh x86`：容器里的 Debian i686 MinGW 以 DWARF 构建，脚本内容不变。Windows 上的 x86 由 `Build-Client.ps1` 以 MSVC 构建。

### 本地原生测试目录

完整 x64 构建后运行 `bash platforms/windows/stage-runtime.sh x64`，脚本从同一 MinGW 工具链定位 libstdc++、libgcc、libwinpthread，复制到 target/windows-full/x64，并逐项检查测试 EXE、宿主 DLL 和递归运行时导入的架构。未分类依赖或缺失运行时立即失败，不从网络或任意系统目录猜 DLL；工具链的额外运行时目录可显式通过 MSIME_MINGW_RUNTIME_DIR 提供。该目录用于本地验证，不带完整的许可证与源码交付，不要拿它当发行包——发行走 `installer/Package-SimplySign.ps1`。

可将验证目录复制到匹配的 Windows 测试机，在其中运行 `powershell -File .\run-smoke.ps1`。脚本预检十一个固定测试程序和 Server EXE，逐个执行并限制超时，失败立即停止；Server EXE 只执行 --help，不启动常驻服务。不要求 CMake，也不注册 TSF 或修改输入源。

显式运行真实词库回归：`powershell -File .\run-smoke.ps1 -ResourcesDirectory 'C:\IME Test\resources\固定代目录' -TimeoutSeconds 120`。目录必须是已下载的锁定词库代；脚本仅解析目录，数据完整性由共享 prepare_host 校验，不下载或修改词库，工作状态仍在测试临时目录。会话测试在 Windows 使用宽字符入口，路径转 UTF-8 后传给共享宿主；脚本为原生进程引用路径，不经过 shell。未提供目录时明确输出 SKIP，不冒充词库回归通过。这套隔离回归检查的是宿主与管道边界，编辑器内的交互由 `experiments/tsf-edit-control/` 的受控编辑器和第三方编辑器实测负责。

脚本进程控制已另用 macOS 原生探针和 PowerShell 7.6.6 执行验证：默认 12 项及词库跳过、指定含空格目录追加第 13 项、空参数、缺少程序、非零退出和超时终止。探针不链接 Engine，不验证 Windows 二进制、词库内容或 Windows 中文路径。非交叉 CMake 配置找到 pwsh/powershell 时自动登记 windows-runner-control，也可通过 MSIME_POWERSHELL 指定路径；交叉构建不会尝试在本机执行 Windows 探针。探针与临时副本仅用于测试，超时 Kill 后等待进程退出再释放对象。

### Server 命令行入口

完整构建产出 `MetasequoiaImeServer.exe`。生产启动使用 `--production`（Watchdog 使用等价的 `--watchdog-managed`），从安装状态目录读取配置并监听生产 TSF 管道；隔离预览实例使用 `--config <绝对配置路径>`，用来在不碰系统输入源的前提下跑一个独立 Server。`--help` 只显示模式说明，不读写状态。TSF 注册由安装器负责，Server 不在启动时修改系统输入法注册。预览配置是最多 16 KiB 的 JSON，以下五个字段必需，另可提供 key_bindings；其余字段拒绝：

```json
{"format_version":1,"resources":"C:\\MSIME-Preview\\resources","state_root":"C:\\MSIME-Preview\\state","pipe_namespace":"dev-01","preedit_style":"pinyin"}
```

resources 指向已下载的锁定词库代目录；state_root 必须是本预览实例独享的独立目录，两者不能互相包含。路径需绝对；preedit_style 仅 local/pinyin，它决定该实例的宿主收到哪些组字帧，始终覆盖共享 preferences.json 的 tsf_preedit_style，修改启动文件需重启。命名空间只允许 1–48 个 ASCII 字母、数字或连字符，三条管道固定生成为 `\\.\pipe\msime-client-preview-<命名空间>-0/1/2`，不接受旧产品管道名。词库准备前独占打开稳定锁文件，持有到 Server/worker/输入会话全部停止；退出释放句柄，锁文件不删除，其存在不代表实例仍活着。其他直接调用 Engine 的进程不遵守此锁，必须由调用方保证不共享此状态目录。

入口调用共享 prepare_host 校验词库并准备隔离工作数据，启用同目录配置监听，再启动 WindowsServer。Ctrl+C/Ctrl+Break 请求顺序停机；准备阶段的磁盘操作不能即时中断，系统强制终止不保证清理。初始化错误只输出通用信息，不输出配置路径或输入内容。

key_bindings 可选对象示例：

```json
{"minus_equal":false,"comma_period":true,"brackets":false,"tab":false,"page_up_down":true,"arrows":true,"word_character":"brackets"}
```

对象提供时七个字段必须完整且无未知字段，前六项仅接受布尔值，分别控制减号/等号、逗号/句号、方括号、Tab、PageUp/Down、上下箭头；word_character 仅 disabled/brackets/minus_equal。显式对象覆盖共享设置，修改启动文件需重启。未提供对象时使用共享 preferences.json 的 navigation 和 word_character，并通过现有监听器动态发布；旧设置缺省采用 Windows 默认值（除方括号外全部开启，以词定字关闭、键组为方括号）。共享设置禁止以词定字与同键翻页同时开启；UI 切换会同时关闭冲突项。绑定在输入队列更新，不读盘、不解析每次按键；它独立于延迟中的 Engine 方案。显式启动配置仍按以词定字优先处理同键绑定，Microsoft 分号及 Unicode 编辑优先级不变。这份配置只决定这一个 Server 实例怎么分流按键，不改变 TSF 同步契约：接在它上面的客户端必须使用匹配的吃键配置。

#### 托盘菜单与共享界面

托盘菜单七项与成品一致：悬浮工具栏开关由 Server 自己处理；设置和关于在独立的 WinUI 3 `msime-client-settings.exe` 中打开，表情/符号面板、手写识别板和屏幕键盘仍在共享桌面面板宿主（Tauri）中打开。两类窗口与 Linux 的 IBus 属性菜单共用同一套路由契约——用 `--route=<面板>` 指定面板，`--route=settings:<分类>` 指定设置分类（关于用 `settings:about`），只接受小写 ASCII 标识符；同一路由也写进子进程的 `MSIME_CLIENT_ROUTE`，进程自身继承到的同名变量会被丢弃，不会盖过实际点击的那一行。Windows 语音输入由 Server 内置的 VoiceInputSession 和波形浮层负责录音、识别及 TSF 提交；共享外壳的语音入口通过固定 Aux 管道发送 `ToggleVoiceInput`，由 Server 主线程消费，避免让 Tauri 伪造一个无法录音的面板。

设置外壳按 `MSIME_CLIENT_SETTINGS_COMMAND`（须为绝对路径且存在）、Server 同目录的 `msime-client-settings.exe` 查找；面板外壳使用同目录的 `MSIME.exe`。找不到时这些行保持可见但禁用，点击不会做任何事，也不会声称已打开；启动失败同样按未处理返回，菜单不会因为一个没发生的动作而关闭。两个外壳都用 `CreateProcessW` 启动并继承本进程令牌，因此打包时它们与 Server 的完整性级别一致。

隔离预览实例不注册 TSF，也不接管系统输入源：它挂上候选窗口、后台点击选词和悬浮工具条，走 configured_key 的同一套路径；未支持的路由会断开当前连接。生产模式复用同一份 Server 会话/窗口实现，只是换成安装器注册的生产管道名，TSF 注册和 DLL/Server 部署由安装器完成。Enter 缺少宿主实际本地提交观察时明确拒绝，不从 Engine 伪造观察。预览实例不连接旧产品管道，也不替代生产 KeyHandler。运行时检查包含此 EXE 的依赖，PowerShell 合成测试不启动常驻 Server 进程；CMake 另登记无副作用的 `windows-preview-help` --help 测试。

#### TSF 适配器交接契约

TSF DLL 与 WindowsServer 保持独立进程，通过 Engine contracts 定义的 Main、ToTsf、Worker 管道交换协议帧。DLL 不调用 `configured_key`、`basic_key`、`navigate`，也不缓存 Server 内部 `FocusLease` 或 `PendingReply`。DLL 使用自身的 focus token、composition epoch 和 request_id 验证异步回复，再在所属线程的 TSF edit session 中修改文本；这些值不能冒充 Server 的连接代次或焦点 epoch。

Server 的 SessionPump 接收已登记连接的键和通知，在输入队列与焦点门禁内调用处理器。`configured_key` 已按顺序包含以词定字、`basic_key`、标点及 `navigate`，不能在返回后重复调用这些路径。PendingReply 的编码、发送与确认由 Server 完成；完整写入不等于 TSF 已上屏。原生候选窗口和悬浮工具条归 Server，使用确认快照中的 lease、代次和索引发起请求，不把窗口回调或 Engine 对象注入 DLL。

现有 DLL 的 `KeyEventSendResult` 区分 Sent、DefinitelyNotSent、DeliveryAmbiguous。只有确定未发送的键才可进入既有本地原始输入回退策略；发送结果不确定时禁止重新交给另一套路由，否则可能重复上屏。不能用一个 bool 代替该分类。本地 Enter 完成必须来自宿主的实际观察，不得从 Engine 文本或原始键包推断已经完成的 TSF 文档写入。

该分类由 `windows-key-event-send-result` 在 Debug/Release 两种配置下锁定，并以 x86/x64 MinGW 交叉编译检查 PE32/PE32+；它验证的是本地回退策略本身。

升级两端时，先固定 Engine 契约提交、DLL 与 Client 产物，再按隔离管道握手、激活 fence、请求配对和断连，普通/UILess、密码字段、焦点切换、候选与模式请求的顺序过一遍。这类接线检查在隔离管道上做，不注册 TSF、不接管生产管道、不替换正在运行的 Server。

### 配置投递后的自动重试

`InputState::queue_preferences(lease, snapshot)` 供设置通知在输入队列中提交最新快照。无待回复时直接交给共享宿主；有待回复时每个活动会话最多保存一份 16 KiB 以内的快照，较新 revision 替换旧值，相同内容幂等，较旧或同版本冲突拒绝。只有当前焦点的成功投递确认才会自动交付保存的快照，错误确认不能触发；焦点取消和新激活清除尚未交付的旧快照。

返回 true 表示已接纳，不代表设置已生效：平台仅验证大小、格式版本、revision 和对象外形，完整偏好校验及组合期间延迟应用仍归共享宿主。交付失败抛错，输入队列按既有失败路径撤回授权，不能重放已投递的键。调用方应使用共享设置存储产出的有效快照；已有 update_preferences 保留手动接口。

`PreferenceSnapshot::load` 直接调用共享 PreferencesStore 读取/完整校验，可能等待磁盘或文件锁，必须在设置工作线程执行，不得放进输入任务。不可由任意 JSON 直接构造该发布值；加载失败只返回通用异常，不暴露共享错误原文。`InputState::publish_preferences` 接受其副本，保留一份全局最新值，拒绝旧版本/同版本冲突，向当前已确认焦点交付；无焦点时仍保留。新激活及断开重连的新会话在 confirmed 后、按键前自动接收最新值，不因其初始 options 较旧而回退。尚未确认的焦点只等待，不被配置发布授予输入授权。低层单会话更新接口不应与另一套 revision 来源混用。

WindowsServerOptions::preferences_directory 显式启用共享配置轮询，空值表示关闭。SessionController 持有 PreferenceMonitor，默认每 250ms 在单独线程尝试读取；最多一个待完成发布任务，未变化快照不重复入队。锁忙、坏文件、旧版本/同版本冲突与队列满保留旧配置并稍后重试；发布失败或输入队列不可用成为终止故障，不重放输入。preferences_status 暴露分类状态，不包含路径/原始错误；Current 仅表示最新快照已交给队列，不保证正在组合的会话已经应用。

轮询停机使用条件变量唤醒，不等待整个间隔；已提交任务只捕获快照值，不引用监听器，监听器停机不等待输入 future。控制器先请求监听器停止，再停止服务/连接 worker/输入队列，最后 join 设置线程；对象仍保留输入队列到 join 完成。设置线程禁止从输入任务 join，request_stop 可请求退出。初次轮询异步执行，启动须提供经过共享准备的 host_options。这里用的是轮询而不是系统级文件变化通知：配置文件由共享设置存储以原子替换的方式写入，轮询读到的永远是完整文档，而变化通知还要处理替换产生的删除/重建事件。

监听器使用 `PreferenceSnapshot::try_load`：调用共享 C ABI msime_client_try_load_preferences，锁竞争返回空值（ok:true,value:null），成功返回完整已校验快照，坏文件/权限错误仍报错。空值只表示忙，保留之前发布值并稍后重试，不能恢复默认配置。底层仍使用同一稳定锁文件及共享验证，没有旁路读 JSON；不会等待写者释放锁，但磁盘 I/O 仍可能阻塞，因此不能在输入队列调用，停机时限也不因此有保证。宿主库与 Windows 适配器需成套更新到含该符号的版本。

### 显式导航绑定入口

`InputState::navigate(lease, packet, bindings)` 在同一个输入队列/焦点门禁下贯通 FocusedSession、ReplyComposer 和 ServerSession。NavigationBindings 是调用方提供的值快照，分别启用减号等号、逗号句号、方括号、Tab、PageUp/PageDown、上下候选；全部默认关闭，不暗设产品偏好。Tab 根据 Shift 判定方向，UiLess 从原始包读取；Unicode 的 + 仍交给共享字符输入。回复未确认时禁止再导航或输入，失效焦点不会推进 Engine。

该入口必须在 TSF 上下文已排除标点提交、以词定字等优先路径后调用。已消费的导航键即使绑定关闭也返回 PendingReply：普通模式使用 NavigationIgnored，UILess 返回未变的候选页；不调用 Engine command，不改变组合、代次、页码或高亮，仍等待投递确认，禁止再回退旧 VK 映射。返回空只表示非导航键、快捷键、Unicode +、空组合或失效焦点等不适用情况。生产入口通过 `production_key_handler()` 接入 `SessionPump`，设置监听发布的导航快照也由 `InputState` 消费；隔离预览实例则使用启动配置里的显式绑定。

### TSF 中文开关同步

SessionPump 在输入队列内自动处理当前焦点的 IMESwitch、StatusSnapshot 与 FocusRestored，先同步 keycode 表示的中文开关，再调用外部事件回调。关闭时经共享宿主清空 Engine 组合与旧回复前缀；关闭期间迟到按键不再推进 Engine，重新开启从空组合输入。相同状态通知不重复改变 Engine 代次，每个客户端会话保留开关至再次通知；过期焦点不能修改状态，未确认回复阻止切换。外部事件回调仍在焦点锁内，不得重入 Engine/焦点门禁。

这一段只负责每客户端的键盘开关。全局模式作用域由共享设置的 `ime_mode_scope` 决定，全半角等 UI 状态走 `mode_view()` 与悬浮工具条；出站模式请求见下文。

### 配置相关按键分流

InputState::configured_key 接受末尾可选 WordCharacterBinding（默认 Disabled，另有 Brackets/MinusEqual），先处理以词定字，再执行 basic_key 和候选标点／导航。仅无修饰键（UILess 除外）且 VK 与实际字符匹配时命中；以词定字优先于同键翻页配置。逗号/句号和方括号按 NavigationBindings 决定是否保留为翻页；数字键盘加减键不属于以词定字。返回空表示该包未由共享 Server 消费；生产 `production_key_handler()` 将这个结果交给 `SessionPump` 的统一回复/确认路径，隔离预览实例则用同一编排器加启动配置里的显式绑定。调用方仍须先完成 TSF 配置专属快捷键等原生优先规则；空组合标点仍由 TSF 本地处理。

Microsoft 双拼分号在 edit/basic_key 中先于标点处理：读取 View.microsoft_shuangpin（当前已应用 Engine 配置）、local_mode、editing_text 与 caret_position，只有普通模式且光标所在分隔块为奇数长度时作为 ing 韵母编辑；已完成双键、其他方案或局部模式继续标点路径。不是从最新持久化设置推断，延迟应用期间仍遵循旧会话方案。新 View 字段与宿主库应成套更新；缺失时不启用特殊分号。

以词定字依据共享视图中的高亮候选 ID 调用 Engine 首／尾汉字选择，成功发送 CommitExactText；无汉字或没有候选时清理组合并发送 Normal 高亮文本（可为空），交给 TSF 补本地智能标点，不在 Server 再转换一次，也不完成剩余分段。两条路径都带已有已选前缀，保留焦点、待回复及投递确认门禁。配置持久化监听和生产 KeyHandler 分别由 `PreferenceMonitor` 和 `production_key_handler()` 提供。

ABI 1 的附加符号 msime_client_punctuation，适配器与宿主库必须成套更新。它显式复用共享运行时的高亮候选完成与 Engine 标点转换，避免 Unicode 等局部模式把普通 character 调用标记为已处理却未完成标点提交。非 ASCII 标点参数在状态推进前拒绝；普通/UILess 标点完成均发送 CommitExactText，保留已有焦点、待回复与投递确认门禁。

### TSF 标点开关同步

PuncSwitch 的 keycode 与 StatusSnapshot/FocusRestored 的 pinyin_length 同步到当前会话的中文标点开关。经共享 C ABI 调用固定 Engine 的运行时 setter，不重建会话，不改变组合、光标、候选代次或引号配对。待回复和过期焦点仍不能修改状态。TSF 开关是每会话临时覆盖，不写入配置文件，并在共享偏好替换 Engine 时重新应用；未收到覆盖的会话继续使用持久化偏好默认值。

msime_client_set_chinese_punctuation 是 ABI 1 的附加符号，适配器和宿主库须成套更新。其返回值为未变化的 View；关闭时空组合 ASCII 标点由宿主透传，组合中标点仍按共享运行时的完成策略处理。这个接口只管每会话的标点开关，全局作用域由 `ModeAuthority` 和共享的 `ime_mode_scope` 决定。

### 书名号自动补全后的嵌套回退

候选打开时 `<` 由 Server 的 Engine 转换，Engine 的书名号嵌套计数随之加一；开启配对补全后右半边由 TSF 自己插入，之后的 `>` 只跨过它，Server 收不到能回退计数的按键，于是后面每个书名号都变成〈〉。所以 TSF 自动补全 `<` 后发 Main 事件 PairedPunctuationAutoClosed（16），keycode 为开口键，只能是 `<`，其他值在 `valid_main_frame` 被拒。它是没有回复的通知，和 PuncSwitch 一样经 FocusRouter 限定当前焦点，在输入队列里调用 `msime_client_balance_paired_punctuation_after_auto_close`；管道按序投递，所以回退先于下一个按键生效。开口由 TSF 本地解析时也照发：两边计数都在零处截止，多回退一次无害。事件编号由 `scripts/apply_engine_paired_punctuation_ipc.py` 叠加进 Engine 契约头。

### 发往 TSF 的模式请求

WindowsServer/SessionController::request_mode(lease, mode) 提供中英文、中文/ASCII 标点、全/半角六种命令，线格式使用固定 Engine worker opcode 与清零的 404 字节帧，不支持任意 opcode 或文本。调用方从已确认焦点事件获取 lease，并在外部线程调用；输入/事件/控制回调内调用会拒绝，避免递归焦点锁。有限时写入可能阻塞，不宜直接占用 UI 线程。

发送在焦点锁内核对 lease 与当前连接；失效、未就绪、停机或非法模式返回 Rejected。Sent 只证明完整投递，不证明 TSF 已应用；实际中英文/标点状态经 TSF 回报再进入共享会话，不提前改变 Engine。写入失败或异常返回 WriteFailed，撤销连接焦点并关闭连接，不重发不确定命令。这条线格式不携带服务端 epoch，所以 Sent 之后的确认只能靠 TSF 的回报，模式请求与对象析构由调用者管理生命周期。

固定上游 TSF 的 _HandleCompositionDoubleSingleByte 在编辑会话内转换并上屏全角字符，Server 不重复转换。Server 拥有一个非激活的 Win32 悬浮工具栏，显示中英、标点、全半角状态，并把这三个按钮通过当前焦点 lease 路由回 TSF；位置按当前工作区定位，客户区支持拖动，隔离预览实例还会把拖动后的位置写回启动配置。`floating_toolbar_enabled` 控制显示，缺省开启；简繁输出、图标菜单和缩放通过共享设置与 `FloatingToolbarSettings` 下发。工具栏本身不注册也不修改本机输入源，那归安装器。

### 编辑键与 TSF 预编辑回复

InputState::basic_key(lease, packet, style, local_text) 统一基础分发：复用 edit 的预编辑规则，按 Engine 模式识别空格/数字选词，处理 Shift/Esc 本地清理及忽略的修饰键，并将 Enter 交给 LocalCommit 前置校验。原生配置相关优先路径必须先处理；返回空表示尚未处理的快捷键、标点或导航，不清空组合，生产 `configured_key()` 会继续候选标点与导航分支，不能把空结果直接当作最终处理结果。Enter 的 local_text 必须来自宿主实际完成观察，不能从 Engine 返回结果拼造。生产与隔离预览两个 KeyHandler 使用同一套编排。

普通模式的无修饰 1..9 与 Unicode 模式的 Shift+1..9 均按 VK 槽位选择当前共享候选 ID，不依赖 wch；例如非美式布局数字键产生 & 时仍选词，Unicode 裸数字仍编辑。小键盘按同样规则归一化。空候选返回未提交状态，不把布局字符作为替代文本上屏。

LocalCommit 分发先验证该包确为原始文本提交键，且调用方提供的本地完成文本等于已选前缀加 Engine 当前 editing_text，再执行共享提交；缺少文本、内容不符或错误键型抛错，并保留组合和待回复状态。stage 的提交后校验仍保留，调用方不能用 Engine 返回值反过来伪造本地已上屏的证明。拒绝并不能撤回 TSF 已经错误插入的文本，仍需宿主处理协议分歧。

InputState::edit(lease, packet, style) 按 Engine 当前模式判定字母、组合中的手动分隔符、Unicode 裸数字/加号、Backspace/Delete 和左右光标移动，并在同一焦点/待回复门禁中推进会话。非编辑键、快捷键、空组合删除、关闭输入或 unknown 模式返回空，不调用 Engine；调用方继续其余原生分发，不能将空结果当作完成了一次输入。

调用方显式提供与 TSF 相同的 Local/Pinyin 预编辑样式，UILess 从包标志读取。Local 编辑不回包；普通 Pinyin 的字符与未清空组合的删除返回 Preedit，左右移动和删除到空组合不回包；UILess 编辑统一返回候选页，包括清空状态。非空 PendingReply 即使没有 encoded 帧也必须通过 SessionPump 确认，不能重跑 Engine。`edit()` 仍只负责编辑子路径；Enter、取消、候选选择、标点和配置优先导航由 `configured_key()` 在同一编排器中按优先级衔接。

### Engine 模式与 Unicode 数字选词

数字候选判定前将 VK_NUMPAD0..9 归一化为 0..9，与固定上游 Server 边界一致；不修改原始请求包。Unicode 模式的 Shift+小键盘 1..9 因此与主键盘一致，越界选择保持原组合；无 Shift 的小键盘数字（包括 0）仍用于 Unicode 编码输入，Ctrl/Alt 快捷键不会被误当作选词。

共享 View 的 local_mode 由固定 Engine 的 SessionSnapshot.local_mode 显式映射并透传，不从 editing_text/preedit 猜测；快照失败时的 unknown 不能当作普通输入模式使用。模式变化不会沿用旧模式候选高亮。宿主与共享库应成套构建，Windows 不对缺失模式字段做前缀回退。

依据固定上游 TSF 消费规则，ServerSession 在 Unicode 模式下把 Shift+1..9 解释为当前页候选选择，即使 wch 已被键盘布局翻译为 ! 等标点；通过视图提供的 generation/global index 调用共享 select，越界槽位不改变组合。非 Unicode 模式仍使用原始 wch 和共享标点处理，不将所有 Shift+数字都强制选词。UiLess 标志不干扰修饰键判定，模式快照只在该数字分支读取，不给普通字符路径增加一次完整视图查询。生产与隔离预览两个分发器都通过 `configured_key()` 选择 Selection 回复路径。

### V、/、@ 模式的按键分流

V（计算与数字）、/（指令）和 @（名字与地点）三个局部模式默认关闭，只在拼音方案下由 Engine 打开：空组合时 Shift+V 进入 V，`/` 与 `@` 在中文标点下进入另外两个。Engine 在 View.spelling_symbols 里列出它当前当作输入的字符：V 模式是 `0123456789+-*/.()%^`，Unicode 模式是十个数字，空组合的拼音会话是已开启的 `/`、`@`。Server 只按这份数据分流，不自己猜模式：`src/input/EditPolicy.h` 的 `edit_kind` 把文本在其中的键交给 Engine 作输入，`digit_selects_candidate` 决定数字键是否选词。Unicode 保留原来按 VK 的规则（Shift+数字选词、裸数字编码）；V 模式里数字与运算符是输入（包括 Shift+8 的 `*`、在别处翻页的 `-`），打出其他字符的数字键（美式布局的 Shift+1 即 `!`，或数字行要按 Shift 的布局上的裸数字键）按槽位选词；其余模式仍是裸数字选词。

TSF 在收到 Server 回复之前就要决定一个键是组合输入还是选词，所以同一条规则在 `tsf/Global/LocalModeKeyPolicy.h` 里还有一份：键击缓冲以 V 开头且 V 模式开启时按上面的规则分类，空组合的 `/`、`@` 在中文标点且对应模式开启时作为组合的第一个字符，而不是标点。网址模式没有开关，在全拼、双拼、五笔（TIP 记作 quanpin，`scheme::DetectsUrls`）且不在专用英文模式时生效：键击缓冲恰好是 `www`、`http`、`https` 或 `ftp`、光标在末尾时，紧跟的 `.` 或 `:` 作为输入进入缓冲，而不是翻页或标点，TIP 同时记下已进入网址模式。之后 TIP 不再从缓冲前缀推断（光标处的编辑会让缓冲开头不再是触发词加触发键，而 Engine 仍在网址模式），与 Engine 一样只在删掉的恰好是触发键（剩下的正好是触发词）、缓冲删空或组字结束时退出。网址模式里网址的数字和符号 `0123456789-._~:/?#[]@!$&'()*+,;=%^`（包括 Shift+数字行的符号和别处翻页的 `-` `=` `,` `.` `[` `]`）都是输入，打出其他字符的数字键按槽位选词，规则与 V 模式相同。Server 按 Engine 列出的 spelling_symbols 做同样的判断。五笔在 `http` 后的 `s` 上已进入网址模式、在网址模式里把 `https` 删成 `http` 时退出，TIP 分不出五笔和全拼，要等触发键之后才按网址分类，删成 `http` 后也仍按网址分类；TIP 的键击缓冲最长 64 个字符，网址在 Windows 上超过这个长度后的按键输入不进去。V 模式和网址模式的符号表、网址触发词，两份拷贝都由 `scripts/test-windows-expression-symbols-parity.py` 核对。三个开关由 Server 经 Worker 帧 LocalModeTriggersChanged（28，载荷为 V、/、@ 三个 `0`/`1`）随其余 TSF 本地设置一起推送，只在拼音方案下为真；格式不对时 TIP 三个全关，旧版 TIP 把未知类型直接丢弃。这些模式上屏的是生成文本：commit_context.typing_statistics 为 false 时 Server 不记打字统计，也就不触发成就音。

### 韩语的汉字转换

韩语（Dubeolsik）组字时按汉字键（VK_HANJA，0x19），或者单独轻按一下右 Ctrl（按下到松开之间没有别的键，且在 500 ms 内松开），发送 `MSIME_CONVERT_HANJA`，列出正在组字的那一个音节的汉字，再按一次关闭。右 Ctrl 只在韩语有音节在组字时这样解释，这时它优先于“单击 Ctrl 切换语言”；没有组字时两个键都照旧交给应用或切换语言。纯辅音（ㄱ）没有汉字，按键被吞掉，音节继续组字。已经上屏的音节不转换。

列表打开时的按键规则只有一份，在 `common/KoreanHanjaKey.h`：TIP 用它驱动自己的 host session，Server 用它驱动自己的会话（`ReplyComposer::korean_hanja`），两边对同一个键做同一件事，不看中文候选的翻页绑定。数字 1-9（主键盘或小键盘）选本页，空格和回车选高亮项，方向键移动高亮，PageUp/PageDown 翻页，Home/End 到首尾，Esc 和退格只关闭列表、保留音节。其余键照没有列表时的规则：字母关闭列表并继续组字；标点（包括 `-` `=` `[` `]` `,` `.`）、`0`、Tab、Insert、Delete 关闭列表并提交韩文，与 macOS 和 Linux 一致。TIP 上屏的是自己 host session 选出的汉字，Server 对这些键不回帧，只计入打字统计；在候选窗口里点选时由 Server 选出，TIP 写入后丢弃 host session 里的音节。TIP 先决定吃不吃键、再执行：排在别的键后面的键，按“前面的键执行完以后列表开没开”的推算来分类。推算从 host session 的实际状态出发，汉字键打开或关闭列表，Esc、退格、选字和字母关闭列表。推算列表关着时，每个键照没有列表时的规则分类，所以从不按汉字键的韩文输入和以前完全一样；推算列表开着时，列表的键在执行时再按 host session 的实际状态决定，Server 也按自己会话的状态决定，两边一致。纯辅音按汉字键不会打开列表，推算会暂时偏开，但键在执行时仍按实际状态处理，只是排在后面的键的推算要等队列排空才校正。Ctrl+Enter 在韩语下始终交给应用，Server 的 `commit_candidate_translation` 遇到韩语也不提交，汉字候选的훈음和译文都只用于显示。列表在音节仍在组字时关闭，TIP 的候选 presenter 会安静地撤掉，不发 HideCandidateWnd，否则 Server 会取消它仍在组的音节；Server 的 `cancel_composition` 遇到列表打开时会连发两次 `MSIME_CANCEL`。汉字候选不提供置顶、固定排位和删除菜单。

汉字候选的훈음（音训，例如 `나라 이름 한, 한나라 한`）由 Engine 放在候选的 annotation 里。`CandidatePresentation.h` 在韩语汉字列表（scheme 4，不在专用英文模式和任何局部模式下）里把它从 annotation 挪到 `PresentationCandidate::gloss`，所以候选正文只有汉字本身，훈음画在翻译那一段：字号是候选的 0.78 倍（`translation_font`），颜色是翻译的颜色，摆放规则也和翻译一样（竖排放得下时跟在汉字后面，横排或放不下时在汉字下面），不看候选翻译和英文释义开关。翻译查询对韩语和中文一样发出（共享库的 `msime_client_translation_query` 不再对 scheme 4 返回 null，繁体汉字查不到时按简体字再查一次），有译文时훈음在上、译文另起一行在下：`candidate_secondary_text` 用换行连接两者，`CandidateItemWidths::translation_lines` 为 2，这样的一段总是放到汉字下面，高度按两行实测。훈음只存在于这份显示投影里，从不写进 Engine 的 translation，Ctrl+Enter 等提交译文的路径读到的始终是 Engine 自己的 translation。

未在真机核实：TIP 只注册在 zh-CN 配置下，韩国键盘的汉字键在这种情况下能否送到 0x19，以及右 Ctrl 在各种键盘驱动下是否作为右 Ctrl 而不是汉字键上报。

### 粤拼、注音、越南语与藏语

这四个方案和全拼、双拼、五笔、日语、韩语一样在托盘菜单、工具栏和设置里可选，仍只注册一个 zh-CN TSF 配置，Watchdog 不变。按方案决定的行为集中在 `common/InputSchemeTraits.h`，Server 与 TIP 共用；其中镜像 Engine `SchemeType` 谓词的部分由 `scripts/test-scheme-traits-parity.py` 与 `crates/engine/src/types.rs` 核对。Server 经 InputModeChanged 把输入模式告诉 TIP，载荷是一个字符：`0` 中文（全拼、双拼、五笔）、`1` 日语、`2` 韩语、`3` 粤拼、`4` 注音、`5` 越南语、`6` 藏语、`7` 笔画（见下文）。旧版 TIP 只认 `1`、`2`，把新代码当作中文。粤拼和注音是中文方案，切换到它们会更新 `last_chinese_scheme`；越南语、藏语与日语、韩语一样是单独的语言，切过去时记住被替换的中文方案。

粤拼（scheme 5）按中文方案走候选窗，标点、翻页和以词定字与全拼相同；它的词库本身是繁体，繁体输出开关不再转换。注音（scheme 6，只有大千键位）、越南语（scheme 7）和藏语（scheme 8）像韩语一样由 TIP 在自己的 host session 里组字，组合始终内嵌显示在文档里，光标锁在组合末尾，失去焦点、切换方案或模式时提交而不是丢弃；Server 对这些键不回帧，只计入打字统计（`ReplyPath::SyllableCommit`）。TIP 的分类规则在 `tsf/HostKoreanKey.h` 的 `host_composed_key_action`，立即路径和排队路径共用：

- 注音：小写字母和大千符号键拼注音；空组合时 `1`、`2`、`5`、`8`、`9`、`0`、`,`、`.`、`/`、`;`、`-` 起一个音节，组字中十个数字、这些标点和空格都参与拼写（空格是一声，或在音节完整时打开列表）。下方向键打开候选列表；列表打开时数字、空格、回车、方向键和翻页键属于列表，`0` 和标点仍在拼写，选字只固定一个读音、继续组字。回车提交，Tab、方向键、Home/End、PageUp/PageDown、Delete 提交后交给应用，Shift 标点经中文标点表随转换一起提交，Shift 加字母提交转换并跟上该字母。执行时以 View.spelling_symbols 为准再判断一次，排队时用相同的静态表推算。在候选窗口里点选注音候选会被拒绝，列表只用键盘操作。
- 越南语：Telex 或 VNI（设置里选择），字母按原样大小写组字，Caps Lock 下的大写字母照样组字，与 macOS 一致；VNI 下组字中的数字是声调。没有候选列表。空格、数字（Telex）、标点提交单词并跟上该字符，标点是半角；方向键等提交后交给应用；Esc 丢弃整个单词。关闭中文时越南语与韩语一样直接输出半角 ASCII。
- 藏语：威利转写（EWTS），没有可选项。字母按原样大小写组字（`T`、`D`、`N`、`Sh`、`A`、`I`、`U`、`M`、`H` 都是不同的字母），Caps Lock 下的大写字母照样送进宿主会话；威利读不了的字母（`A`、`D`、`H`、`I`、`M`、`N`、`R`、`S`、`T`、`U`、`W`、`X`、`Y` 以外的大写字母，以及小写 `q`、`x`）不进组合，引擎先上屏已有的藏文再原样写出该字母，拉丁字母不会混进转换结果；`'` 随时参与拼写（可以开头 achung 音节），`+`、`.`、`-` 在组字中参与拼写。组合里保存当前音节串的威利原文，内嵌显示的是转换出的藏文，没有候选列表。组字中空格上屏藏文并加音节点 `་`，`/` 上屏藏文并加垂符 `།`（以 ང 结尾时垂符前保留音节点，`ང་།`），空组合时 `/` 单独输出垂符；回车只上屏藏文，按键被吃掉；数字和其他标点上屏藏文并跟上该字符（半角，数字保持原样）；方向键等提交后交给应用。第一次 Esc 把组合切回拉丁原文并继续组字，第二次 Esc 丢弃；原文显示时空格上屏原文并跟上空格。排队时用引擎的静态拼写表（`kTibetanIdleSymbols`、`kTibetanComposingSymbols`）推算，执行时以 View.spelling_symbols 为准再判断一次。

粤拼和注音需要安装包附带的词库：`Prepare-PackageFiles.ps1` 从 `target/language-dictionaries`（`scripts/fetch_language_dictionaries.py` 按 `resources/language-dictionaries.lock.json` 下载）把 `msime-cantonese.db`、`msime-zhuyin.db` 连同各自的授权声明放进 `server_exe/language-dictionaries`，安装后位于 `server\language-dictionaries`，与 `server\resources` 同级，宿主库在那里找到它们并写进运行时配置。缺少某个词库时，托盘里对应的项不可选，已选的方案按宿主库的 `effective_scheme` 退回上次的中文方案（再不行就是全拼），TIP 和托盘都按实际运行的方案处理。越南语和藏语不需要数据。设置 `MSIME_REQUIRE_LANGUAGE_DICTIONARIES=1` 时缺少任一词库会让打包失败。

未在真机核实：注音、越南语与藏语的 TIP 行为只在 macOS 上用交叉编译和单元测试验证过，没有在 Windows 上实际打字。语言栏图标没有新增，这几个方案显示中文图标。

### 笔画

笔画（scheme 9）是中文方案，在托盘菜单「输入方案」里排在藏文之后，设置页的分段控件里同样可选，工具栏语言按钮显示「笔」。它有自己的输入模式代码 `7`：它的 trait 与全拼不同（不做繁简转换、不显示译文），TIP 的按键分类也不同。Engine 谓词逐项照抄粤拼，所以它和粤拼一样走 Server 的候选窗，标点、翻页、数字选词与全拼相同，候选来自只读的 `msime-stroke.db`，不提供置顶、固定和删除，繁体输出开关不再转换，打字统计记在 `stroke` 名下。

键位：`h` 横、`s` 竖、`p` 撇、`n` 点、`z` 折，`x` 是匹配任意一笔的通配符。空组合时只有 `hspnz` 开始组合，`x`、其他字母和大写字母交给应用（`InputSchemeTraits.h` 的 `LetterPassesWhileIdle`，立即路径和排队路径共用）。Engine 自己的英文模式（Ctrl+Shift+E、工具栏或托盘的英文）例外：那时 TSF 仍报告中文，Engine 先于方案判断英文模式、组合每一个字母，所以 Server 用 Worker 帧 DedicatedEnglishChanged（29，载荷 `0`/`1`）把这个状态推给 TIP，打开时所有字母都交给 Engine，否则 "apple" 的 a 会直接进应用。这个状态随 Server 每 250 毫秒读一次焦点会话的英文模式推送；TIP 吞下 Ctrl+Shift+E 时先在本地翻转（`common/DedicatedEnglishMirror.h`），所以切换后立刻键入的字母已按新状态分类。这个先行值 1 秒内没有被推送确认（按键在发出前被丢弃，或 Server 没有切换），就退回 Server 推送过的值。组合中 TIP 收下所有字母，Engine 在两边都吞掉笔画以外的字母，组合不变。数字 1-9 选词，空格提交高亮候选，回车提交键入的字母，Backspace 删最后一笔，Esc 清空。组合中的 `'` 不是音节分隔符（Engine 的 `accepts_apostrophe` 对笔画为假）：TIP 和 Server 都按 `ApostropheIsPunctuationWhileComposing` 把它当标点，和逗号一样先上屏高亮候选再上屏标点，与 Linux、macOS 一致。预编辑显示笔画字形 一丨丿丶乛＊：候选窗的预编辑行和 TIP 的内嵌组合都取 View 的 `preedit`（TIP 读自己 host session 的 View），`editing_text` 仍是 ASCII 字母，只用来对齐光标和校验回车提交的文本。

词库：`Prepare-PackageFiles.ps1` 把 `msime-stroke.db` 连同 `msime-rime_stroke_LICENSE.txt` 与粤拼、注音词库一起放进 `server_exe/language-dictionaries`；Server、TIP（`FanyUtils.cpp` 的 `ReadConfiguredRunningScheme`）和托盘都按这个文件是否存在决定笔画能否运行，缺少时托盘里的「笔画」不可选，已选的笔画按 `effective_scheme` 退回上次的中文方案。`MSIME_REQUIRE_LANGUAGE_DICTIONARIES=1` 要求 `resources/language-dictionaries.lock.json` 固定的每一份词库都在；锁固定 `msime-stroke.db` 之前笔画词库存在就装入，缺少也不让打包失败。`tests/input/stroke_keys.cpp` 用入库的合成词库 `tests/input/fixtures/msime-stroke.db` 对真实 Engine 会话跑这些键。

未在真机核实：笔画的 TIP 行为和十个方案的设置页分段控件宽度只在 macOS 上用 MinGW 语法检查、本机运行的测试验证过，没有在 Windows 上实际打字或查看。

### 按键音、上屏音与背景音乐

播放由共享库完成（host-api 的 kira 播放器，设置来自会话的 `preferences.plugins`），Server 只在自己的输入队列上报事件，TSF DLL 从不调用任何音频接口：它把同一个 `msime_host_api.dll` 加载进每个宿主进程，而播放器要等第一次有开关打开的调用才启动。`FocusedSession::configured_key` 在 Engine 处理完一个它接受的键之后调用 `msime_client_key_sound`，类别由 `src/input/KeySoundPolicy.h` 决定（空格 1、回车 2、退格 3、其他 0；Ctrl/Alt 组合键和单独的修饰键不出声），英文模式下不出声。TSF 只把输入法接手的键转给 Server，所以没有组合时的空格、回车等交给应用的键不会出声。确认送达的上屏在 `record_commit` 里调用 `msime_client_commit_sound`。获得焦点时调用 `msime_client_music_set_active(true)`，失去焦点、会话销毁时置为 false，偏好更新后在仍持有焦点时再报一次，让中途打开的背景音乐立即开始。前台是全屏应用（`FullscreenForeground.h`）时按键音、上屏音都不出，获得焦点时也不开音乐。

密码框：TSF 不读取输入范围（InputScope），靠的是键盘上下文。经典 Edit 的 ES_PASSWORD 控件会停用输入法；Chromium 与 Firefox 的密码框按它们的实现也挂在停用的上下文上（这一点没有在真机上逐个验证）。`_IsKeyboardDisabled()` 为真时 TSF 不接手任何键，Server 也就收不到，按键音不会泄露密码节奏。背景音乐只随 Server 的焦点租约开关，并不知道字段是不是密码框，所以不会因为密码框而暂停。

内置音效包由安装器放在 DataDir 的 `sound-packs`（与提示音 `audios` 同级），Server 通过 `src/system/SoundPackRoot.h` 在会话选项里写入 `sound_packs`；开发运行的状态目录里没有它时不写，由共享库按 resources 旁边的默认位置查找。

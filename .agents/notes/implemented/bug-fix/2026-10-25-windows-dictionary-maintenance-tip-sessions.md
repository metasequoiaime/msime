# Agent Note: Windows 词库维护让 TIP 宿主会话放开，设置窗口写进输入法读的代次

Status: implemented

## Problem

#9434：Windows 10 LTSC 上的 0.3.0，在设置窗口「词库」里给快捷短语或 86 五笔新增词条，点保存后「新增词条」灰一下又恢复，词条没有加上；偶尔提示「词库正在被输入法占用」，更多时候什么也不显示。[维护期间延迟新连接的会话创建](2026-10-24-windows-maintenance-new-connection.md)修掉了 Server 内部的一条竞争，但那篇自己说明不能解释全部失败。沿新增的整条路径追下来，有三处各自都能让一次新增失败或白做，而且都不会让用户看到原因。

一、维护的独占锁在真实桌面上拿不到。设置窗口经 `msime_client_settings_dictionary` 新增词条，`edit_personal_dictionary` 要在 `user_data` 和代次目录上取 `DictionaryAccess` 独占锁。持锁者不只有 Server：TSF 的 TIP 在每个激活了水杉输入法的应用线程里，用 `runtime-options.json` 调 `msime_client_create` 建了一个宿主会话（`CCompositionProcessorEngine::SetupLanguageProfile` → `InitializeHostSession`），这个会话在 `ffi/session.rs` 里以共享方式拿着同一把锁，直到 TIP 停用。[升级时保留内容相同的已占用词库](2026-10-24-windows-upgrade-locked-dictionaries.md)里 Restart Manager 查到的占用者正是加载着 TIP 的记事本。Windows 上的 `QuiescedHosts` 只经辅助管道发 `DictionaryQuiesce`，Server 只放开它自己进程里的会话，各应用里的 TIP 会话（包括设置窗口自己：用户刚在对话框里用水杉打了短语）一个也放不开。2.5 秒重试用完，答 `dictionary_busy`。清除学习数据和导入走同一条路，结局相同。

二、设置窗口写的不是输入法读的代次。Windows Server 每次启动都调 `msime_client_prepare_host` 重新准备宿主，会话用的是准备出来的文档（`server_main.cpp` 的 `session_options = prepared.at("value")`）；`runtime-options.json` 却只在首次运行时由 `FirstRun.h` 写下，此后 Windows 上没有任何一方刷新它（macOS、Linux 和 Android 的输入法启动时调 `msime_client_refresh_host`，Windows 从来没有）。代次目录按词库锁的摘要命名，0.2.1 到 0.3.0 之间锁变了，于是升级上来的用户，Server 读 `user/dictionaries/<新代次>`，设置窗口、`msime-mcp` 和 TIP 还照文件读写 `<旧代次>`。即使锁拿到了，新增的短语也只写进旧代次和日志，Server 要等下次启动回放日志才看得到，打字时就是「加了没用」。`tests.rs` 的 `the_published_options_follow_the_generation_the_input_method_prepared` 先复现这一步：按首次运行的文件新增一条，Server 那份代次里列不出来。

三、失败说了，但用户看不见。设置页把操作结果画在页内标签下面（照共享页的 `phraseError`），「新增词条」按钮却在查找结果表格下面。初始 1000 × 780 的窗口要往下滚才点得到按钮，结果条正好在视野上方；成功时也什么都不说，而列表按个人词条、精确匹配、权重排序，权重 10 的新词条不一定在第一页。

## Decision

Windows 的维护放开是 `server::WindowsRelease`：照旧经辅助管道请 Server 放开，同时写下 Linux 和 macOS 宿主看的那份租约（`user_data/.msime-dictionary-quiesce`，格式和上限不变）。租约写不下时退回只问 Server，不因此连 Server 也不放开。每个请求之前续期两者；重试间隙只再问 Server，不每 50 毫秒重写租约。放下时先撤租约，再请 Server 恢复。

TIP 在已有的 500 毫秒偏好定时器（`TIMER_REFRESH_HOST_PREFERENCES`）上检查租约，读法在 `platforms/windows/common/DictionaryQuiesceLease.h`：先一次属性查询排除没有租约的常见情况，有文件时经 `read_private_file` 读（拒绝重解析点和多链接，带 `FILE_SHARE_DELETE`，写者随时能改名盖上新租约），过期判定与 Linux、macOS 共用 `platforms/common/DictionaryQuiesceLeaseFormat.h`。去留由 `tsf/HostSessionMaintenance.h` 决定：租约有效、会话开着且不在组字时放开，正在组字时等下一轮；租约撤掉后只重建为维护放开过的会话，而且同样等不在组字时才建：会话放开期间 TIP 走自己的旧组字流程（`KeyHandler` 只在宿主会话无效时用它），组合在 `_keystrokeBuffer` 里，组字途中换上一个空的宿主会话，按键状态和后续按键就改读宿主那边，已输入的字母被丢下，退格和空格按空组合判定、可能漏进文档。重建后补发偏好快照，并按线程此刻的焦点文档重新告诉新会话焦点（放开期间的焦点变化没有记下来）。激活时正赶上租约，或者 `msime_client_create` 答 `dictionary maintenance busy`，也记作「在等维护结束」，由定时器补建；别的创建失败照旧不重试。

`msime_client_prepare_host` 的启动请求多一个可选的 `runtime_options`：给了时，`publish_prepared_dictionaries` 把这次准备的 `dictionaries` 写进那份文件，其余键原样保留；文件已是这个代次时只读不写，两份文档不是准备布局或状态目录（按规范化路径比较）不同时不动。两份文档可以把同一个状态目录拼成不同的字面路径（Server 传的是 `weakly_canonical` 后的状态根，首次运行写文件时用的是原样拼写），所以只取准备出的代次目录名，接在文件自己的 `user_data/dictionaries` 下；照搬整条路径会让 `dictionaries` 和文件里的 `user_data` 拼写对不上，文件从此不是准备布局，之后的发布、刷新和 `bundled_resource_root` 都会跳过它。发布失败只记诊断，不挡住宿主启动。Windows Server 在生产模式下把它读的 `runtime-options.json` 传进去。只写 `dictionaries`、不写 `resources`：在线词库更新的资源目录在状态目录下，而 Server 启动时从这份文件读 `resources` 并要求它与状态目录不相交，写进去会让 Server 下次起不来；新增、编辑和查找只经过代次里的工作副本，`resources` 留在安装包目录不影响它们。

设置窗口每有新的操作结果，结果条画出来后滚进视野一次（等同一轮里 `render_page_at` 恢复滚动位置之后，换下来的旧结果条不滚）；新增成功时说「已添加到<词库>，编码 <编码>」，编辑成功说「已保存」。共享页没有成功提示，这是 Windows 页面高、按钮离结果条远而补的，不改变共享页。

## Alternatives considered

- **让 Server 经已有的管道通知各 TIP 放开宿主会话。** 每个 TIP 本来就连着 Server，通知不必碰文件系统，Server 还能等所有 TIP 答复再回「OK」，语义最精确。但这要给 TSF 协议加一种帧和一次跨进程的确认往返，Server 还得处理断线、停用和挂住不答的 TIP；租约是 Linux、macOS 宿主已经在用、Rust 写者已经会写的同一份契约，TIP 只需要读，维护者照旧靠 2.5 秒重试吸收各 TIP 的定时器间隔。
- **TIP 不建宿主会话，或只在需要本地组字的方案（韩文、注音、越南文、藏文）里建。** 能从源头少掉大部分持锁者。但 TIP 的第二三候选、UI-less 宿主的本地候选和导航也依赖宿主会话（`HostOnlineCandidateSync`、`KeyHandler` 里的多处 `host->valid()`），收窄要重审这些路径，而且留下的那些方案仍会挡住维护。
- **Windows Server 启动时调 `msime_client_refresh_host`，与其他平台一致。** 现成接口，也会准备代次、回放日志。但它连 `resources` 一起换成在线词库更新的目录，而 Windows Server 从这份文件读 `resources` 并拒绝落在状态目录里的资源目录，装过在线词库更新的用户下次启动 Server 会直接失败；它还可能往仍被旧版 TIP 读取的文件里加新键。只写 `dictionaries` 避开这两点。
- **结果条改画到「新增词条」按钮旁边。** 离操作最近。但结果条的位置与共享页一致（标签下面、对所有标签可见），导入、导出和清除学习数据的结果也在那里；滚进视野对所有操作一样有效，不必为一个按钮另起一处。

## Consequences

- **收益**：一次维护里，Server 和各应用里的 TIP 都会让出词库锁，设置窗口和 `msime-mcp` 的新增、导入、清除学习数据在真实桌面上能拿到独占锁。升级或装上在线词库更新后，设置窗口写进的就是 Server 读的代次，新增的词条维护一结束就能打出来，不必等下次登录。失败和成功都会出现在视野里。
- **代价与已知上限**：每个 TIP 线程每 500 毫秒多一次文件属性查询。每次维护后，各应用的 TIP 都要重建一次宿主会话。TIP 正在组字或线程不处理消息（应用挂住）时不会放开，维护者仍会在 2.5 秒后答 `dictionary_busy`。`runtime-options.json` 的 `resources` 仍停在安装包目录：装了在线词库更新时，设置窗口、TIP 用安装包里的资源文件加更新的代次，和 Server 的会话不完全一致；Server 自己的 `resources` 读法改成不依赖这份文件之后，才能把 `resources` 一起发布。Win7 兼容设置窗口的词库页没有改结果条的显示。

## Verification

`cargo test -p msime-host-api --lib published`：`the_published_options_follow_the_generation_the_input_method_prepared` 用合成词库锁准备两次（首次运行，再换锁模拟升级），先证明按旧文件新增的短语在 Server 的代次里列不出来，发布后再新增的能列出来，且文件只变了 `dictionaries`、已是当前代次时不再改写；`prepared_dictionaries_are_not_published_into_a_foreign_document` 覆盖别的状态目录和不是准备布局的文档；`a_differently_spelled_state_directory_keeps_the_published_layout` 用符号链接把同一个状态目录拼成两种路径，断言发布后文件沿用自己的拼写、仍是准备布局，同一代次不再改写、换代次照样发布（修之前这条失败在写入的 `dictionaries` 用了准备文档的拼写）。`cargo test -p msime-client-services --lib quiesce` 的既有用例（`busy_is_retried_under_a_lease_that_is_removed_afterwards` 等）断言重试期间租约在、结束后撤掉，现在对 Windows 的 `WindowsRelease` 同样成立；CI 不在 Windows 上跑这组测试。`platforms/windows/tsf/tests/runtime/host_session_maintenance.cpp` 覆盖 TIP 的去留决定，包括租约撤掉时正在组字不重建；`platforms/common/tests/dictionary_quiesce_lease.cpp` 照旧覆盖共用的过期判定。TIP、Server 和设置窗口的原生代码在 PR CI 的 MinGW 交叉构建和 MSVC 设置窗口构建里编译；真实 Windows 上在记事本和设置窗口都激活着输入法时新增快捷短语与五笔词条、升级后新增并立即打出，还没有验收。

# Agent Note: Linux 打开账号云功能，并补上快捷短语候选、大写锁定标点和应用例外

Status: implemented

## Problem

对照 macOS 的界面审计（Linux UI 差距清单的 4.1、4.2、4.4、4.5 和评审补充的 8.5、8.6、8.7）发现，Linux 上有一批功能看得见入口却用不了，或者共享设置页因为能力位为假把它藏了起来：

- 云词库面板从 IBus 菜单、Fcitx5 状态栏、`.desktop` 动作和设置页「云词库」分类都能打开，但没有 provider socket 时 `cloud_dictionary_request` 在 Linux 上直接回 `unavailable`。没有任何随装服务提供 `cloud-dictionary.sock`，所以对普通用户这是一个死面板；而 Linux 设置应用早就登记了与 Windows、macOS 相同的账号会话（`linux_account.rs`）。
- 完整云词库备份、恢复和「应用到本机」被 `cloudDictionaryCapabilities` 只给移动端、macOS 和 Windows。
- 设置同步卡片和社区「词包与回复模板」只在 Windows 的设置应用里接上。
- 快捷短语编码只能先按 Shift+K 进 K 模式，macOS 已经让它直接排进普通候选。
- 两个宿主都跟踪大写锁定（「⇪」指示），但从不报告给会话，「大写锁定时使用英文标点」因此在 Linux 上不显示。
- 应用例外（`app_input_mode_rules`）只在 macOS 和 Windows 执行，Linux 的宿主不读这张表，设置页也不显示。

## Decision

**账号云功能复用 Windows 那一套桌面代码，不另写 Linux 版本。**

- `linux_account::setup` 和 Windows、macOS 一样登记 `CloudDictionaryState`（设置应用本地数据目录下的 `dictionary-snapshots`）。`cloud_dictionary_request` 的账号回退不再按平台分支：配置的或自动发现的 provider socket 仍然优先，provider 失败时照旧报错不改走账号；只有没有 provider 时才经 `desktop_cloud_dictionary::request` 走账号会话。`platform::cloud_dictionary` 和 `desktop_cloud_dictionary` 的目标条件加上 Linux。
- 前端 `cloudDictionaryCapabilities` 在 Linux 上给出 `snapshot: true`、`snapshotAppliesInPanel: true`：IBus 和 Fcitx5 宿主都不处理快照队列，和 Windows 的 Server 一样由面板当场应用、开着时重试。「应用到本机」靠 `QuiescedHosts` 挂的词库维护租约让两个宿主放开词库，这是它们本来就认的协议（`linux_dictionary_quiesce.rs` 和 `DictionaryQuiesceLease.h`）。恢复走页面的文件选择，不走 macOS 的原生选文件。
- Linux 的云词库面板（设置页里的和独立面板窗口）经 `save_export` 把导出和完整备份写进「下载」目录：WebKitGTK 和 macOS 的 WKWebView 一样没有下载处理器，设置页其他导出在 Linux 上本来就走这条路。`createDesktopCloudDictionaryClient` 多一个可选的宿主保存函数，只在 Linux 上用。
- `windows_settings_sync.rs` 原样搬到 `platform/desktop/desktop_settings_sync.rs`，在 Windows 和 Linux 上注册四个 `account_preferences_*` 命令。只同步 `input.*`：账号文档里没有 Linux 自己的键，桌面专属设置（候选窗口、快捷键、皮肤）不随设置同步，和 Windows 一样。应用经 `save_preferences_impl` 写进共享偏好并同步 `runtime-options.json`，两个宿主当场读到，所以沿用 Windows 的「已应用云端设置。」。
- `community_resources` 的命令和 `CommunityResourceState` 加上 Linux，`main.tsx` 在 Linux 上注入 `settingsSync` 和 `communityResources`。

**快捷短语候选和大写锁定标点只在宿主侧接线，规则仍在共享层。**

- 两个宿主建会话时和 `phrase_preedit` 一起请求 `quick_phrase_candidates`；`HostCapabilities::quick_phrase_candidates` 对 Linux 为真，设置页的快捷短语说明随之改成「直接输入编码即可调用」。
- 两个宿主把每个按键事件带的锁定状态（与「⇪」指示同一来源）报告给会话：IBus 的 `sync_caps_lock` 在建会话时和每个键分发前比较会话上次收到的值，Fcitx5 的 `syncSessionCapsLock` 在 `ensure()` 之后、按键分发前做同样的比较；`HostCapabilities::caps_lock_punctuation` 对 Linux 为真。

**应用例外按程序名执行，规则和「停留」放在共享头文件里。**

- `src/core/AppInputModeRules.h` 读规则表、按 ASCII 不分大小写匹配程序名，并用 `AppInputModeRuleStay` 记住「这次停留」：进入另一个程序才算新的一次，用户在一次停留里手动切换后规则让位，和 macOS、Windows（`ModeAuthority.h`）同样的语义。没有程序名的焦点不结束停留。
- Fcitx5 取 `InputContext::program()`。任何上下文获得焦点（不论用哪个输入法）都更新停留；新会话恢复模式时（`restoreInputMode` 和首次按 `default_ime_mode` 的分支）规则排在作用域和记忆前面；`toggleInputMode` 让位；全局作用域下规则定下的模式不写进全局状态。
- IBus 只能尽力而为：1.5.27 起 `focus_in_id` 的客户端名只有 GTK 输入法模块带程序名（`gtk3-im:<g_get_prgname()>`，见 ibus 的 `client/gtk2/ibusimcontext.c`），`ibus_app_rule_program` 只认这种格式。`app_rule_active` 让全局作用域的 `open()`、偏好重载和 `sync_global_input_mode` 不拿全局状态盖过规则；`toggle_input_mode` 和属性菜单的中英文切换让位。身份协商时名字带来了另一个模式，`focus_in` 会照切换中英文时那样重新聚焦会话并刷新属性。
- `HostCapabilities::app_input_mode_rules` 改为三个桌面平台都为真。Linux 没有选应用的对话框，设置页手填程序名（粘贴路径时只取文件名），不调 `pick_application`、`application_names`（这两个命令在 Linux 上没有注册），并在说明里写明哪些程序会报告程序名。

## Alternatives considered

- **Linux 云词库继续只认 provider socket，再随包装一个提供 `cloud-dictionary.sock` 的用户服务。** 和在线候选、翻译那几个 provider 的结构对称，输入法进程也能直接用。但账号令牌、刷新锁、快照校验和词库激活在设置应用里都现成，Windows 和 macOS 的设置进程已经这样服务同一个面板；多一个常驻服务只是把同一份代码搬进另一个进程，还要多一条凭据交接的路。[macOS 云词库账号回退](../bug-fix/2026-10-18-macos-cloud-dictionary-account-fallback.md)当时写的「保持 Linux 的 provider-only 行为」由本篇翻转，provider 优先、provider 失败不重发这两条照旧。
- **完整备份的导出在 Linux 上沿用下载链接。** 少一个参数。但 WebKitGTK 没有注册下载处理器时下载去向不可靠，设置页的其他导出早已为 Linux 改走 `save_export`，面板单独走下载链接会让同一个应用里两种导出表现不同。
- **设置同步给 Linux 加自己的 `platform.linux.*` 键，同步候选窗口和快捷键。** 那需要服务端字段表收录这些键，并让 Linux 宿主和 macOS 原生偏好的映射互不冲突；这次只追平 Windows 已有的范围，不在一个对齐 PR 里扩大账号文档契约。
- **应用例外只在 Fcitx5 上提供，按 `input_method_framework` 收窄能力位。** 能力位就能如实描述「IBus 下大多不生效」。但 GTK 程序在 IBus 上确实能报告程序名，规则对它们生效；能力位是平台级的，按框架收窄要让设置应用每次启动读宿主状态文件，框架换了还要失效。设置页的说明写清两个框架的差别，比整组藏起来更有用。
- **IBus 下用 `_NET_WM_PID` 反查焦点窗口的进程。** 能覆盖 X11 上的 Qt 和 XIM 程序。但 Wayland 下没有对应的查询，引擎进程还得自己连 X 服务器、跟踪焦点窗口，和输入上下文的焦点并不总是同一个窗口；对一个尽力而为的路径来说代价过大。
- **Linux 设置页加一个选应用的对话框（选 `.desktop` 文件或可执行文件）。** 用户不用记程序名。但 Fcitx5 的 `program()` 和 IBus 的程序名来自 `g_get_prgname()` 或可执行文件名，不总等于 `.desktop` 的 `Exec`，选出来的名字可能根本不命中；解析 `.desktop` 也要再引一层库或手写解析。手填加上「粘贴路径只取文件名」更可预期。

## Consequences

- **收益**：Linux 用户登录后就能用云词库面板的全部功能（含完整备份、恢复和把云端快照替换本机词库）、设置同步和社区词包，不用自己架 provider；快捷短语编码直接出候选；大写锁定时可以打英文标点；可以给终端、编辑器这类程序固定起始中英文模式。
- **代价与已知上限**：
  - 「应用到本机」激活期间两个宿主的输入会暂停；面板关着时没人处理队列，下次打开云词库面板才继续。在真实 Linux 桌面上的大词库准备耗时和激活期间 IBus、Fcitx5 是否都按时放开词库，还没有在真机上量过。
  - `save_export` 单个文件上限 64 MiB，超过的完整备份在 Linux（和 macOS 设置页）上导出失败。
  - 设置同步只覆盖 `input.*`。
  - 应用例外在 IBus 上只对 GTK 程序生效；Qt、XIM 和 IBus 1.5.27 以前都匿名。Fcitx5 下也取决于前端是否报告程序名，部分 Wayland text-input 客户端没有程序名。没有程序名的焦点不结束停留，所以在有规则的程序和匿名程序之间来回切，规则不会重新生效。
  - IBus 身份协商时，如果名字带来的规则模式和协商前不同，模式会在协商完成的那一刻切换，协商期间已经打出的组字按切换中英文的规则处理。

## Verification

- Rust：`apps/desktop` 在 Linux 容器里 `cargo clippy -p msime-desktop --all-targets` 无新告警；`client-core` 的 `host_surface` 测试断言 Linux 声明 `caps_lock_punctuation`、`quick_phrase_candidates` 和 `app_input_mode_rules`。
- 前端：`mobile-host-capabilities.test.ts`、`desktop-cloud-dictionary-client.test.ts`（Linux 快照能力、只有 Linux 用宿主保存导出）、`app-input-mode-rules.test.tsx`（Linux 手填程序名、路径只取文件名、不调用选应用和名称查询）。
- 宿主：`linux-app-input-mode-rules`（规则匹配、IBus 客户端名解析、停留与让位）；`fcitx5-native-test --app-rules`；`fcitx5-native-test --caps-lock` 和 `ibus-engine-smoke --caps-lock` 末尾的大写锁定标点用例；`fcitx5-native-test --caps-lock` 同时断言会话请求了快捷短语候选；`ibus-engine-smoke` 默认流程里的应用例外用例。资源相关的用例需要已验证的词库目录，各自的运行结果写在 PR 里。
- 没有在真实 Linux 桌面上用真实账号做联网验收，也没有在真实 GTK、Qt 程序上核对报告的程序名。

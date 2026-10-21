# Agent Note: Windows 设置的语音页、AI 输入辅助与连接助手对齐 macOS

Status: implemented

## Problem

macOS 设置（共享设置 UI 的桌面分支）的「语音输入」页顶上是「识别与输入」「录音行为」两个页内标签，服务的接入（识别服务、本地模型、测试、文本润色）收在页内的「服务配置」子视图里，从「识别」组紧跟识别服务的「服务配置 ›」一行打开，行尾写着配置状态（`voice-settings-content.tsx` 的 `desktopVoiceServiceStatus`）。Windows 原生设置窗口的语音页是一整页：页首一组、识别、识别结果、语音快捷键、录音行为、录音设备、识别服务配置、文本润色依次排下来，没有「服务配置」那一行，也没有状态。当时的理由是原生窗口没有子视图控件（见 [精简语音输入设置页](../simplification/2026-10-16-macos-voice-settings-prune.md) 的「各平台跟进」）。之后设置窗口有了页内标签框架（[页内标签框架](2026-10-20-windows-settings-page-tabs.md)），插件页也有原地换视图的写法，这个理由不再成立；用户要求 Windows 设置以 macOS 为准迁移。

同一批差距里还有 AI 页的「输入辅助」标签：macOS 的「服务」组按依赖排成服务提供商 → 接口地址 → API Token → 接入说明 → 模型 → 服务模型 → 折叠的「自定义提示词」 → 测试（[AI 服务配置按依赖顺序展示](../bug-fix/2026-10-19-ai-service-setup-order.md)），API Token 的说明写「服务地址：…」，服务商下拉每项带品牌图标（[AI 服务商选择菜单显示图标](2026-10-19-ai-provider-icons.md)）。Windows 把接口地址、服务模型和提示词一起收在「更多选项」里，Token 说明写「只用于 <来源>」，服务商下拉是纯文字。

「连接助手」标签上，macOS 的 `McpConnectSection`（`mcp-connect.tsx`）有一排「AI 助手」标签（Claude Code、Codex、命令行、可写入的客户端、其他），给出着色的安装和移除命令、命令行用法和 JSON 配置，两个权限开关「允许修改设置」「允许读取词库」决定命令、配置和一键写入带不带 `--allow-write`、`--allow-dictionary-read`，带权限写入前先确认，已连接的条目权限不同时提示「更新」，复制反馈 1.6 秒后恢复。Windows 只有一段说明、一个只读的原始 JSON 框、一句让用户自己往 `args` 里加参数的话、点后永久变成「已复制」的复制按钮和每个客户端一个「写入」，写入不传权限，也不会更新已连接的条目，尽管 `msime_client_mcp_install` 早已接受 `flags`。

## Decision

### 语音输入

- 语音页用 `page_tab_bar` 分「识别与输入」（`input`）和「录音行为」（`recording`）两个标签，自动化前缀 `voice-tab`。「识别与输入」是页首一组、「识别」、「识别结果」和「语音快捷键」；「录音行为」是「录音行为」和「录音设备」。页首引导随标签换成 `SettingsMascotIntro.h` 里已有的两句。
- 「识别」组在识别服务下面加一行「服务配置 ›」（`link_row`，自动化 ID `voice-service`）。`link_row` 加一个可选的 `value`，照共享页 `LinkRow` 的 value 画在箭头左边 6 DIP 处的 12 DIP 灰字，并放进读屏的帮助文本。状态由 `VoiceSettings.h` 的 `service_status` 按 `desktopVoiceServiceStatus` 的 Windows 分支计算：本地识别看有没有选模型，网络服务看当前令牌或 `voice_input.asr_tokens[服务]`，豆包旧版鉴权缺一项时写「未填写完整」，豆包没填时照实写「未填写」（Windows 的豆包没填密钥时拒绝录音，不改用系统识别），系统识别没有状态。Windows 总有「文本润色」一组，所以选系统识别时这一行照样画，只是没有状态，与共享页的 `serviceGroups` 相同。
- 子视图记作语音页在 `page_tabs_` 里的第三个标签 `service`，标签条上不画。打开时整页换成带返回按钮（`voice-service-back`）的「服务配置」标题，下面是「识别服务」组（原来的「识别服务配置」，系统识别时不画）和「文本润色」组；返回写回 `input`。打开后焦点落在返回按钮上，返回后落回「服务配置 ›」那一行；离开语音页时 `navigate` 把 `service` 换回 `input`，与共享页隐藏时 `resetServiceSubview` 相同。页首引导在子视图里仍是「识别与输入」那一句（`mascot_intro` 找不到 `service` 时取第一条）。
- 搜索：建索引时子视图的两组照样画一遍，记在 `service` 名下，`index_tabs_` 补一项「服务配置」，结果的位置写成「语音输入 › 服务配置 › 分组」。选中这些结果时 `open_search_result` 照常把 `service` 写进 `page_tabs_`，子视图直接打开，不用另写一套展开逻辑；子视图里的「更多选项」仍按原来的方式随搜索展开。

### AI 输入辅助

- 「服务」组照共享页排成：启用 AI 辅助、服务提供商、接口地址、API Token（Ollama 不画）、接入说明、模型、服务模型（获取模型列表和它的状态）、「自定义提示词」折叠区、测试 AI 辅助配置。「更多选项」去掉，接口地址和服务模型常驻显示。
- 接口地址的占位文字是 `https://example.com/v1/chat/completions`；为空时说明写「请填写服务提供商给出的完整接口地址」，不能用时说明是 `msime_client_ai_endpoint_check` 给的问题并把输入框标红，输入照样写进设置（共享页的 `aria-invalid` 只是标记，不拒收）。
- API Token 的说明在地址能用时写「服务地址：」加 `AiSettings.h` 的 `service_address`：Rust 给的来源键去掉默认端口，再接上原文里的路径，去掉末尾的 `/chat/completions`、`/responses`、`/messages` 和末尾的 `/`，不带查询参数，与 `aiEndpointServiceAddress` 相同。地址不能用时写「请先填写有效的接口地址」：桌面的共享页不给这一行传 `endpointHint`，地址的问题由上面的接口地址行自己说。
- 「自定义提示词」是独立的折叠区（默认收起，`ai_prompt_open_` 跨重画保留，自动化 ID `ai-prompt-fold`），里面是当前槽位的「提示词内容」：说明换成共享页的短版，文本框至少 96 DIP 高（4 行）。「服务模型」的说明换成「可从服务获取模型，也可直接填写模型名称。」。
- 服务商下拉照 `IconProviderSelect` 带品牌图标：每项是内容为图标加名字的 `ComboBoxItem`，读屏名是服务商名，收起时选择框显示选中项的同一份内容。图标是 `packages/ui/src/assets/providers/*.svg` 用 `rsvg-convert -w 80 -h 80` 转成的 PNG，放在 `platforms/windows/settings/assets/providers`（同一份 MIT 许可在旁边），`Settings.rc` 打成 RCDATA 150–161，`AiSettings.h` 的 `provider_icons` 记服务商、图源文件和底板颜色。底板照 `ai-provider-icon.tsx`：24 DIP、5 DIP 圆角，白色或 #171719（Kimi、OpenRouter），一圈 0.5 DIP 的淡描边；「自定义」用 Fluent 的链接图标。位图按服务商缓存在窗口上。

### 连接助手

- `append_mcp_section` 照 `McpConnectSection` 重写：说明（共享页的新版，讲两个开关默认都开）、读不到状态或复制失败时的一句、一排紧凑分段的「AI 助手」标签（`segmented_control` 放开 `maximum_segments`，自动化 ID `mcp-tab-<标签>`，外面一层横向滚动的 `ScrollViewer`，800 宽窗口里六个标签也不会被裁掉），标签下的内容，写入结果（读屏礼貌播报），两个权限开关行（说明前写着参数名），信任提示和「服务器程序 <路径>」。运行时选项还没有时只说尚未完成初始化。
- 文字和规则放在只依赖标准库的 `McpConnect.h`：权限表、`toggled`、`same_flags`、注册表位、授权确认框的标题和说明、写入结果、过期提示、`shell_quote`（Windows 路径用双引号）、`server_program`、安装和移除命令（服务器名取 `mcp_server_name()`，full 是 msime，与共享页写死的 msime 相同，其他版本用自己的名字）、命令行用法（Windows 路径时参数经 `@<文件路径>` 传），以及 MCP 配置的着色和加权限。配置按 JSON 词法切成记号（键、字符串、标点、空白），有权限时找到 `mcpServers` 下条目的 `args` 数组，照数组已有的换行和缩进把参数接在最后一个元素后面，结果与共享页 `JSON.stringify(…, null, 2)` 的输出逐字相同；读不懂时原文不着色、不加权限，与共享页的回退一致。
- 着色的命令和配置画成一个 `TextBlock`，每个记号一个 `Run`，颜色取共享页 `styles.css` 的 `--syntax-*`（深浅各一套），标点用次要色，占位符斜体，换行用 `LineBreak`；Consolas 12 DIP、1.625 倍行高，圆角 8、12 DIP 内边距、带描边的浅底。命令折行，配置不折行、横向滚动。
- 复制按钮复制记号拼成的文字，成功后只有这个按钮显示「已复制」，1.6 秒的 `DispatcherQueueTimer` 到点后就地换回原来的字，不重画页面；复制失败时页面顶上说「复制失败，请重试。」。
- 权限：未连接的标签和复制的内容用本机记着的选择 `mcp_preferred_`；已连接的客户端标签上开关先显示条目现有的权限，改了之后记在 `mcp_drafts_[客户端]` 直到写入成功。权限不同或条目 `stale` 时显示警告提示条，按钮换成「更新 X」；已连接且一致时按钮不可按。写入时把开关的权限作为 `flags` 交给 `msime_client_mcp_install`；带权限时先弹「允许 X 中的助手使用这些权限？」，主按钮「允许并写入」，`mcp_entry_exists` 时照旧再问要不要替换。写入期间整页重画一次，按钮写「正在写入…」，所有写入按钮不可按。
- 本机的选择存在 `HKCU\<MSIME_EDITION_REGISTRY_KEY>` 的 `McpFlags`（REG_DWORD，每项一位），读不到时两项都开，与共享页 localStorage 的 `msime.mcp.flags` 同义：只属于这台电脑上的这个设置窗口，不进偏好、不同步。

## Alternatives considered

- **子视图用一个独立的布尔成员，照插件页的 `plugin_view_`**：与插件页写法一致，读起来更直白。否决：搜索命中子视图里的行时还得另写判断（插件页为此维护了 `plugin_sound_titles_`），而把它记成一个不画在标签条上的标签，搜索、离开页面的重置和页首引导都沿用现成的标签路径，只多一处 `navigate` 里的重置。
- **保持一整页，只加「服务配置」行跳到页下方**：改动最小。否决：页面仍然一长列，和 macOS 的层级不一样，状态值也没有地方放。
- **服务商图标直接用 SVG（`SvgImageSource`）**：不用另存一份 PNG，图源改了自动跟上；主题页已经这样解码皮肤里的 SVG。否决：这批图标的宽高写成 `1em`、带 `style` 属性和 `currentColor`，WinUI 的 Direct2D SVG 渲染对这些的支持要逐个在 Windows 上验收，而本机没有 Windows；栅格化成 80 × 80 的 PNG 后与插画走同一条已经验证过的 RCDATA 路径，`test-windows-settings-art.py` 核对图源、顺序和尺寸。
- **服务地址让 `msime_client_ai_endpoint_check` 一并返回**：用 `url` crate 的规范化路径，与浏览器的 URL 完全一致。否决：要改 Rust 和 C ABI，这一批只能在 CI 上编译；地址能用时 Rust 已经给出规范化后的来源，剩下的路径按原文取，只有含 `.`、`..` 段或非 ASCII 字符的路径会和 macOS 显示得不同，接口地址里几乎没有。
- **助手标签用 WinUI 的 `SelectorBar` 或下拉框**：差距清单建议过，`segmented_control` 默认超过三项就退成下拉框。否决：共享页这里就是一排紧凑分段控件，`segmented_control` 已经能放开段数，加一层横向滚动就解决了窄窗口；`SelectorBar` 是另一种下划线样式的控件，下拉框要多点一次才看得到有哪些助手。
- **权限选择存进偏好文件，或只记在窗口里**：存偏好最省事，只记在窗口里不用落盘。否决：偏好会同步到别的设备，而这是「这台电脑上给哪个助手放多大权限」的本机决定，共享页也只放在 localStorage；只记在窗口里则每次打开都回到两项都开，用户关掉的权限下次复制命令时又悄悄带上。注册表的当前用户、本版本键是本机、按版本分开、不需要新文件的地方。
- **用 `Windows.Data.Json` 解析配置、加参数后再序列化**：不用自己切词法。否决：WinRT 的 `JsonObject` 不保证键的顺序，也不按 `JSON.stringify` 的两格缩进输出，显示出来的配置会和宿主写进文件的、和 macOS 上看到的不一样；词法切分只在原文上插入几个记号，其余逐字不动，也能在主机上单独测试。
- **「服务配置」行的状态随输入就地更新**：共享页的状态随草稿实时变化。否决：令牌输入框在子视图里，「服务配置」行和它不会同时出现，返回时整页重画，状态自然是新的。

## Consequences

- 语音页一次只画一个标签的内容，录音设备在「录音设备」组每次变为可见时读：切到「录音行为」标签，或停在这个标签上进入语音页。画别的标签或「服务配置」子视图时 `build_voice_page` 把 `capture_stale_` 记成过期，与共享页 `useVoiceDevicePicker` 每次 `active` 变真都重读一致；在「录音行为」上来回重画不会重读。
- `link_row` 多了一个默认空的参数，原有调用不变。
- 连接助手标签每次重画都同步读一次 `msime_client_mcp_status`（会读两个助手的配置文件），切标签、拨开关都会重画，沿用原来的做法；文件很小，没有改到工作线程。写入仍在 UI 线程上调 `msime_client_mcp_install`。
- `McpFlags` 是新的本机落盘位置；卸载不清它，留下的只是一个 DWORD。服务器名按版本取，非 full 版本的命令和 macOS 写死的 msime 不同，这是有意的：宿主写进配置的名字本来就是按版本取的。
- AI 服务组常驻的行多了两行（接口地址、服务模型），预设服务商也要看到它们，换来的是先确认 Token 送往哪里，与 macOS 的取舍相同。
- 服务商图标是 PNG 快照，共享 UI 换了图标要重新转换并核对；新增服务商时 `provider_icons`、`Settings.rc` 和 PNG 要一起加。服务商下拉的选中项在收起时依赖 ComboBox 把 `ComboBoxItem` 的内容挪进选择框，这个行为只能在 Windows CI 的截图里确认。
- `page_tabs_` 里出现一个标签条上没有的值；`page_tab_bar` 遇到它会落到第一个标签，但语音页在画标签条之前就按它换成子视图，所以不会出现。别的页面若要照搬，需要同样在 `page_tab_bar` 之前分流。
- 子视图标题照共享页 level 2 的 `SettingsPageHeader`：40 DIP 圆形返回按钮、22 DIP 左箭头，标题 26 DIP 粗体；插件页视图头仍是自己的「‹ 我的插件」样式，两处不统一。

## Verification

- `tests/ui/voice_settings.cpp` 覆盖 `service_status` 的本地模型、系统识别、网络服务（当前令牌、空白令牌、按服务记下的令牌）、豆包新版和旧版鉴权；`tests/ui/settings_navigation.cpp` 核对 `service` 标签的页首仍是「识别与输入」那句。本机用 clang++ 编译运行，并用 `x86_64-w64-mingw32-g++ -std=c++20 -Wall -Wextra -Werror -fsyntax-only` 检查。
- `tests/ui/ai_settings.cpp` 覆盖 `service_address`（去掉动作和默认端口、保留 /v1 前缀和非默认端口、动作后的斜杠、查询参数和两头空格、没有路径、只去一个末尾斜杠、别的路径原样）、短版提示词说明和图标资源号与深色底板；`scripts/test-windows-settings-art.py` 核对 `provider_icons` 与 `ai-provider-icon.tsx` 的服务商、图源和深色底板一致，PNG 都在且是 80 × 80，`Settings.rc` 的资源号与表的顺序一致、不和插画重叠，许可原样带上。
- `tests/ui/mcp_connect.cpp`（新增 CMake 测试 `windows-mcp-connect`）覆盖权限的排序、切换、比较和注册表位，授权说明、写入结果和两种过期提示的原文，`shell_quote` 的三种情况和 Windows 路径判断，Claude Code、Codex 的安装和移除命令（带或不带运行时选项、按版本的服务器名）及各段的种类，两种命令行用法，配置原样着色、按缩进接上参数、另一个版本的服务器名和键序、一行写法、空数组，以及字符串没结束、没有 `args`、`args` 不在 `mcpServers` 下这三种回退。
- `main.cpp` 本机编不了，由 PR 的 Windows CI 以 `/W4 /WX` 编译并跑 `tests/tools/settings_ui.ps1`：语音页核对「识别与输入」有「服务配置」行、没有录音设备和文本润色；点开子视图后有返回按钮和「文本润色」、标签条不在，截图后返回；「录音行为」标签的页首引导和内容；AI 页核对服务商下拉、常驻的接口地址和服务模型、收起的「自定义提示词」折叠区，并且没有「更多选项」；合成状态目录里没有运行时选项，主题循环里的连接助手核对尚未初始化的提示、没有助手标签；循环之后另开一次窗口，经 `MSIME_CLIENT_HOST_OPTIONS` 交给它一份空对象的运行时选项（文件不叫 `runtime-options.json`，不改偏好目录），从 `mcp` 路由直接打开连接助手，核对打开在 Claude Code、安装和移除命令与两个开关都在，点「复制命令」后按钮显示「已复制」、再换回「复制命令」（弱引用记着的按钮计时结束时就地复原），切到「其他」截图。权限开关的注册表写入、一键写入和授权对话框会改动 runner 的注册表和助手配置文件，不在 CI 里点。

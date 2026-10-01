# macOS InputMethodKit 宿主

## 目录结构

macOS 平台源码统一放在 `src/` 下按 `backend/`、`voice/`、`candidate/`、`input/`、`cloud/`、`dictionary/`、`settings/` 和 `core/` 分层；`tests/` 保存测试，`resources/` 保存输入法资源，`scripts/` 保存构建脚本，平台根目录只保留 CMake、资源模板和文档。

本目录提供可复现的 IMK bundle、Rust/C++ 构建和离屏/替身测试，安装与注册核查由 `scripts/install.sh` 和 `scripts/check_input_source.swift` 完成。

## 安装与输入源注册

**`install.sh` 装的是 bundle，不准备词库，装完直接打字是打不出中文的。** 输入会话由 `InputController.mm` 的 `prepareSession` 用 `MSIMELoadRuntimeOptions()` 建起来，而 `RuntimeOptions.h` 的查找顺序是 bundle 内的 `Resources/runtime-options.json`、再退到 `~/Library/Application Support/app.msime.macos/runtime-options.json`（那里还没有时读旧目录 `app.msime.client` 里的同名文件，见下文的标识迁移）。开发构建两处都没有——发布包里这一步由设置应用在首次启动时完成（内嵌 `EngineResources`，见《发布包》一节），构建产物里没有任何等价物。没有配置就没有会话，而没有会话的控制器把每个按键原样交给应用，看起来与用户自己切到英文一模一样。开发机上补齐这一步：

```sh
cargo run --quiet -p msime-client-core --example install_resources --locked -- target/resources   # 拉取并逐个校验固定词库发布，打印目录
bash platforms/macos/stage-resources.sh target/resources/<上一步的目录>                            # 再加上仓库自带的辅助码表，暂存到 target/macos/EngineResources
ditto target/macos/EngineResources "$HOME/Library/Application Support/app.msime.macos/EngineResources"
cargo run --quiet -p msime-host-api --example prepare_host --locked -- "$HOME/Library/Application Support/app.msime.macos/EngineResources" "$HOME/Library/Application Support/app.msime.macos"
```

`ditto` 那一条不是多余的。`prepare_host` 只是把传进去的目录记到 `runtime-options.json` 里，指向 `target/resources` 的安装会在这个被忽略的产物目录被清掉时（AGENTS.md 里那条磁盘写满的记录说明它随时会）安静地失效，表现又是打不出中文。词库不合适时输入法只写一行 `MSIME has no input session (…)` 到统一日志，`log show --predicate 'process == "水杉输入法"'` 能看到，原因分「no usable runtime options」（没准备）和「session refused the runtime options」（准备过但 Engine 不接受）两种。

`scripts/install.sh` 会停掉正在运行的实例、在暂存目录用本机 Developer ID 连同 `resources/VoiceInput.entitlements` 重签名（`--deep`，因为 Sparkle 自带其发布方的签名，hardened runtime 下 Team ID 不一致会导致进程加载失败）、原子替换并保留旧 bundle，任一步失败自动回滚，最后调用 `--register-input-source` 并用 `scripts/check_input_source.swift` 查注册表，而不是只看退出码。

签名身份按证书的 SHA-1 取，不按名字。钥匙串里同时存在两张有效的同名 Developer ID Application 证书是常态（续期的那张和它替换的那张并存），而 `codesign` 遇到同时匹配两张的名字不会自己挑一张，直接报 `ambiguous (matches … and …)` 拒签——那时运行中的输入法已经被停掉了。哈希不会有歧义；本机安装用哪张都可以，所以脚本取第一张并把找到几张打出来，要指定就把 `MSIME_SIGNING_IDENTITY` 设成 `security find-identity -v -p codesigning` 里的 SHA-1。

`check_input_source.swift` 的退出码分三种，`install.sh` 按它选提示：0 是每条注册的源都 enabled；2 是能从输入菜单选中（至少一条 select-capable 的源是 enabled）但还有别的源没启用；1 才是不可用——不在注册表里，或在注册表里却没有任何可选中的源是 enabled 的。带模式的输入法在注册表里是多条：一条不可选中的总源，加上每个模式一条，所以判断是否可用要看可选中的那些，不是看是不是全部 enabled。以前任何一条 disabled 都退 1，于是一个中文模式正常、能打字的安装会被报成「本次登录会话看不到它」，并让人去注销重新登录，而那改变不了任何事情。LaunchServices 会把每个 worktree `target/` 下的构建产物都按同一个发布 identifier 登记，`install.sh` 因此在注册前只保留已安装的那一条。bundle 本身是否装配正确由 `bundle-contents` 这项 CTest 检查（图标是否真的暂存进去、用途字符串是否每种语言都有、语音提示音 `audios/start.mp3` / `end.mp3` 是否随包、本地识别助手 `msime-voice-local` 与 sherpa-onnx 运行库及其许可证是否随包）。

bundle 使用系统已登记的 `app.msime.inputmethod.MetasequoiaIME`，已在列表中的 identifier 原地更新正常：`install.sh` 安装之后，`check_input_source.swift` 稳定报出 `app.msime.inputmethod.MetasequoiaIME`、`.Hans` 与 `.Roman` 三条。

其中每个模式是否 enabled 不在本项目的控制范围内。macOS 27.0（26A428）上，类型为键盘输入模式（`TISTypeKeyboardInputMode`）的输入源无法由进程启用：`TISEnableInputSource` 返回 noErr，模式仍是 disabled。这不是签名、bundle 内容或 plist 的问题，也不是那个登录会话限制：拿苹果自己的 `com.apple.inputmethod.SCIM.Shuangpin`（父输入法已启用、同胞模式 `ITABC` 正在用）做对照，行为完全一样；而同一个会话里对键盘布局 `com.apple.keylayout.US` 调同一个函数，启用和还原都正常。所以被拒的是输入源的类型，不是这个 API、也不是这个 bundle。一台机器上某个模式已经是 disabled 时（比如用户在系统设置里移过它），`--register-input-source` 和 `install.sh` 都加不回来，只能由用户在 系统设置 > 键盘 > 文字输入 > 输入源 里手动添加；英文模式缺失时的行为见《输入源菜单与输入模式》一节——控制器不请求这个不可选的模式，菜单栏保持「中」，中英切换照常工作。

注册结果在刚替换 bundle、刚调用 `--register-input-source` 之后有一段分钟级的不稳定窗口：`TISCreateInputSourceList` 按 bundle id 查不到东西，过一会儿不做任何操作自己会回来，所以同一条 `check_input_source.swift` 隔几秒跑，会先报 not in the registry，再报 enabled。`install.sh` 只查一次就下结论，两个方向的误判都可能出现——判断安装结果时多查几次再看。

换一个新的 bundle identifier 时要知道 macOS 本身的一条限制：一个在本次登录会话开始时不在输入源列表里的 identifier，无论 bundle 内容如何都进不去列表——`TISRegisterInputSource` 返回 noErr 而 `TISCreateInputSourceList` 查不到，重启 `imklaunchagent`/`TextInputMenuAgent`/`TextInputSwitcher`/`keyboardservicesd`、`lsregister -f` 重新登记、`lsregister -r -domain user` 重扫用户域都改变不了。这与签名方式、名字是否 ASCII、plist 是否正确无关：把同机注册正常的输入法整体复制一份、只改 identifier 并用同一张 Developer ID 证书重签，失败方式完全一样。重新登录一次即可，之后同一 identifier 的更新都不再需要。`install.sh` 因此把安装与注册分开——bundle 就位并签名成功即视为安装成功，只报告需要重新登录。

应用支持显式 `--register-input-source` 启动参数：调用 Carbon TIS 注册当前 bundle，按 bundle identifier 找到可启用的输入源并启用自身、停用其他匹配列表中的输入源。`install.sh` 在替换 bundle 后直接调用它，注册和启用两条路径另有注入函数指针的原生测试覆盖。启用模式那一半在 macOS 27.0 上是空操作，且系统不报错（上面那条限制），所以这个参数退 0 不等于每个模式都启用了——判据只能是 `check_input_source.swift` 读出来的注册表。

## 窗口打开日志

输入法是 `LSBackgroundOnly` 进程，它替用户打开的窗口（设置、帮助、账号、词库、翻译与 AI 设置、云剪贴板与云词典、表情、手写、屏幕键盘）一旦被压在正在打字的应用下面，屏幕上看起来和「点了没反应」完全一样。这条链路上的每一步都写进统一日志的 `app.msime.inputmethod.MetasequoiaIME` 子系统、`ui` 分类，不依赖诊断日志开关，复现之后直接读：

```sh
log show --last 5m --style compact --info --predicate 'subsystem == "app.msime.inputmethod.MetasequoiaIME" && category == "ui"'
```

按顺序能看到：悬浮工具栏哪个按钮被按下、背后有没有控制器（`toolbar_action ... delegate=`）；要打开的设置页、是否找到并启动了设置应用，没有的话为什么退回进程内窗口（`desktop_route_*`，`reason=` 为 `settings_app_not_found`、`launch_failed`、`no_provider` 等）；进程内设置窗口构建和切页各用了多久（`settings_window_shown`、`settings_page_shown`）；以及每个窗口呈现前后、0.5 秒和 2 秒后的状态（`present_*`）。判断「被压在后面」看 `front_index`：它是窗口在同层屏幕窗口里的位置，0 即最前，`visible` 为真不代表没被遮住。激活、成为或失去 key、关闭窗口的事件也一并记录。日志只含窗口编号、类名、标题、激活策略、标志位、耗时和错误码，不含输入内容、候选或路径。

## 发布包

`.github/workflows/release-macos.yml` 手动触发，在 macos-15（Apple silicon）上运行 `package-release.sh`，产出 `msime-macos-<版本>-arm64.dmg` 与 `SHA256SUMS`，作为 workflow artifact 上传；`publish` 输入打开时才以 `macos-v<版本>` 发布这两个文件。同一个脚本可在本机跑出同样的包：

```sh
MSIME_SPARKLE_ROOT=/absolute/path/to/Sparkle-2.9.6 platforms/macos/package-release.sh [版本] [输出目录]
```

DMG 里是设置应用（`MSIME.app`）、指向 `/Applications` 的链接和一份 `安装说明.txt`（提醒拖进去之后要打开一次 MSIME）。设置应用的 `Contents/Resources` 内嵌锁定词库 `EngineResources`（经 `verify_resources` 校验）、手写模型、许可证文件，以及输入法本体 `水杉输入法.app`（Release 构建，链接 Release 版 Rust 静态库）。安装方式与 Windows 的「下载、双击、能打字」对应：把应用拖进「应用程序」并打开。本机从未装过这个输入法（`~/Library/Input Methods` 里没有它的任何一个历史名字）时，设置应用先以一个 480×440、不可缩放的安装窗口打开，用户点「立即安装」后才把内嵌的输入法复制到 `~/Library/Input Methods`，并以 `--register-input-source` 登记；装好后点「进入设置」，窗口恢复成设置页的尺寸。复制和登记本身不报告进度，只要几秒，所以安装窗口的进度条是按时间推进的：越往后越慢，停在 93% 等待安装真正结束，最短显示 2.2 秒。已经装过（包括 pkg 时代的 `MetasequoiaIME.app` 和预览版名字）时不出现安装窗口，仍在启动时静默刷新（见「词库准备与 Tauri 设置应用」一节的启动时安装与刷新）。macOS 27 上登记不会把输入源加进用户的输入法列表（见上文），输入源不在列表里时由设置页顶部的状态提示说明添加步骤（系统设置 → 键盘 → 文字输入 → 输入法 → 编辑…），并提供「打开键盘设置」按钮。`/Library/Input Methods` 里有这个输入法的副本（任何名字）时，引导先请用户用管理员权限把它移到废纸篓，设置应用不替用户提权。首次安装有一个例外：这个 bundle identifier 在本次登录会话开始时还不在输入源列表里（全新的机器就是这样）时，按「安装与输入源注册」一节的限制，注册在下次登录前不会生效。此时 bundle 保留在原处，设置页提示注销并重新登录，重新登录后在系统设置里添加即可，与 `scripts/install.sh` 的处理相同；「安装 / 更新」在这种情况下同样保留 bundle，但报告失败。首次启动时从内嵌词库准备用户数据。输入法 bundle 里的 Sparkle.framework 以符号链接组成、签名把链接本身封在里面，所以打包时用 `ditto` 放回签过名的副本（Tauri 的资源复制会把链接展开成文件），设置页的安装也原样重建包内的相对链接，指向 bundle 之外的链接一律拒绝。桌面的整句重排模型（`target/macos/settled-model`）目前不随包，Windows 安装包带它。

包里只能证明本机能构建出的东西：脚本在构建后和挂载 DMG 后各检查一遍内嵌资源、输入法的 bundle identifier 与麦克风 entitlement、两层签名的 `codesign --verify --deep --strict`，任何一项不符就失败。

签名与公证取决于仓库 secrets，全部可选。括号里是 workflow 把它们交给 `package-release.sh` 时用的环境变量名，本机打包时直接设置这些变量：

- `MACOS_DEVELOPER_ID_CERTIFICATE_BASE64`（`MACOS_CERTIFICATE_P12_BASE64`）：Developer ID Application 证书连私钥导出的 `.p12`，base64 编码
- `MACOS_DEVELOPER_ID_CERTIFICATE_PASSWORD`（`MACOS_CERTIFICATE_PASSWORD`）：`.p12` 的导出密码
- `MACOS_DEVELOPER_ID_APPLICATION`（`MACOS_SIGNING_IDENTITY`）：签名身份名，如 `Developer ID Application: Name (TEAMID)`
- `MACOS_NOTARY_APPLE_ID`、`MACOS_NOTARY_TEAM_ID`、`MACOS_NOTARY_APP_SPECIFIC_PASSWORD`（`APPLE_ID`、`APPLE_TEAM_ID`、`APPLE_APP_SPECIFIC_PASSWORD`）：`xcrun notarytool` 的公证凭据

workflow 曾经读取的是括号里的名字，而仓库的 secrets 是以前面的名字存的；读不存在的 secret 得到空串，不报错，于是每次发布都悄悄退回 ad-hoc。

有证书时，输入法按 `scripts/install.sh` 的方式签名（`--deep --options runtime --timestamp`，带 `resources/VoiceInput.entitlements`），设置应用与 DMG 再各签一层（外层不加 `--deep`，以免覆盖输入法的 entitlements）；三项公证凭据也齐全时 DMG 经公证并 staple。只有签名且公证过的包才能走完上面的安装流程。本机设置 `MACOS_SIGNING_IDENTITY`（钥匙串里已有的 Developer ID 身份）与三项公证凭据即可走同一条签名与公证路径，CI 另从 `MACOS_CERTIFICATE_P12_BASE64` 导入证书；目前只验证过 ad-hoc 路径。

没有这些 secrets 时所有签名都是 ad-hoc、也不公证。下载的应用被 Gatekeeper 隔离，第一次要右键「打开」，或执行 `xattr -dr com.apple.quarantine "/Applications/MSIME.app"`；之后设置应用可以运行、词库可以准备，但内嵌的输入法是 ad-hoc 签名，macOS 不会把它登记为输入源（见上一节与 `scripts/install.sh` 开头的说明），安装窗口、启动时的刷新与「安装 / 更新」都会在注册这一步失败：已有安装时回滚到原有安装，设置页提示安装失败；没有安装时 bundle 按上面的首次安装规则留在原处、提示重新登录，但 ad-hoc 签名的输入源重新登录后也不会出现。这种包只对有自己 Developer ID 的开发者可用：用 `scripts/install.sh` 重签并安装其中的 `水杉输入法.app`。

DMG 不提供 Sparkle appcast，输入法与设置应用的「检查更新…」打开官方发布页。

### Homebrew

```sh
brew install --cask metasequoiaime/tap/msime
```

cask 放在 [metasequoiaime/homebrew-tap](https://github.com/metasequoiaime/homebrew-tap) 的 `Casks/msime.rb`，模板是本目录的 `homebrew/msime.rb.in`。改 cask 改模板，tap 里的副本每次发布都会被覆盖。`release-macos.yml` 在 `publish` 打开、`prerelease` 关闭的发布之后，用刚发布的 DMG 的版本与 SHA-256 填好模板并推到 tap；DMG 没有通过 `xcrun stapler validate`（没有公证）或仓库没有 `HOMEBREW_TAP_TOKEN`（能推送该仓库的 token）时跳过并留下警告，因为 ad-hoc 包装得上但输入法注册不了。

cask 安装 `MSIME.app`，并把其中的 `msime-mcp` 链接到 `PATH`，供在终端里配置的 AI 助手使用。和拖进「应用程序」一样，装完要打开一次 MSIME，在它的安装窗口里点「立即安装」，把输入法装进 `~/Library/Input Methods` 并登记；全新的机器还要按上面的首次安装规则注销并重新登录。`brew uninstall` 同时删除 `~/Library/Input Methods/水杉输入法.app`，`--zap` 再删除设置、词库与缓存。只提供 Apple silicon，最低 macOS 13，与 DMG 相同。

## 标识与数据目录

这里有两个不同产品进程，不能用同一个概念混写：输入法本体是 InputMethodKit bundle，继续使用系统已经登记的 `app.msime.inputmethod.MetasequoiaIME`；承载共享 React 设置页的设置应用使用 `app.msime.macos`（`tauri.macos.conf.json` 覆盖共享配置里其他桌面平台用的 `app.msime.client`）。设置应用的默认状态根和输入法读取的原生定位器都在 `~/Library/Application Support/app.msime.macos/`，外部皮肤、偏好、统计与 `runtime-options.json` 以此为当前默认来源。

设置窗口同一时间只有一个，与 Windows 一致：设置窗已经开着时，再从输入法菜单、悬浮工具栏或 Finder 打开设置、关于或检查更新，会把已有窗口拉回前台并切到请求的页面，而不是再开一个窗口。屏幕键盘、emoji、手写、语音和云剪贴板 / 云词典面板仍按输入会话各自启动独立进程。

设置应用和原生宿主均以 `app.msime.macos` 作为正式客户端标识与默认状态目录；新安装按该标识初始化，不创建额外的预览目录。此前 macOS 与其他桌面平台共用 `app.msime.client`：设置应用首次以新标识启动、且新目录为空时，按 `macos_launch.rs` 的 `LEGACY_APPLICATION_IDS` 把旧默认状态复制过来（不删除旧目录）并以新路径重建 `runtime-options.json`；在那之前输入法继续读旧目录的 `runtime-options.json`，只装了输入法也不会没有会话。输入法拉起设置应用时不把这份旧文件作为 `MSIME_CLIENT_HOST_OPTIONS` 传过去，因为带覆盖启动的设置应用不做迁移，两边会一直停在旧目录。

SwiftUI 设置同步覆盖 26 个当前宿主偏好：全局主题、自定义主题底色、自定义候选皮肤、布局、字号、页大小、输入方案、翻页快捷键，以及纠错、辅助码、中文标点、智能标点、重复标点转中文、成对标点、标点锁定、中英/Emoji/颜文字混输、中英混输阈值、中英文模式、切换快捷键、中英文切换提示、全角、浮动工具栏、繁体输出、五笔自动上屏、双拼提示和手写板主题。浮动工具栏的本地设置还支持分别显示标点、全半角、简繁、Emoji、屏幕键盘和设置按钮，并选择 75/100/125/150% 缩放和 16–28pt 字号；这些字段通过 `preferences.floating_toolbar` 共享。共享 Tauri 与原生备用屏幕键盘都为普通键提供 450ms/75ms 的按住重复，粘滞修饰键不自动重复；键盘和辅助功能激活仍单次发送。macOS Tauri 键盘首次显示及隐藏后重开时使用显示器物理工作区底部居中，不混用逻辑点与 Retina 像素，也能处理负坐标副屏和比面板更小的工作区。云端桌面快照覆盖 Apple 固定提交 `2b0250f4dd7012520392b310dfcc0288c3208a75` 的 20 项字段，其中 `candidate_skin` 已由 `global_theme`、`custom_theme_base` 和 `custom_candidate_skin` 三项取代，并保留客户端新增的 `shuangpin_preedit_uses_raw`、`smart_punctuation` 和 `smart_punctuation_repeat`，共 25 项：全局主题及自定义主题底色/外部皮肤、两套辅助码方案、候选学习、本地扩展模式、候选布局/尺寸、输入方案、纠错、标点、智能标点、重复标点转中文、英文模式、全角、工具栏、繁体输出、五笔自动上屏和双拼键位图。辅助码方案按稳定枚举索引同步；本地扩展模式的兼容布尔值为全开/全关，应用时写回八个本地模式而不丢弃未知键。三项主题字段同进同出：服务端 schema 缺少其中任一项时，上传会去掉全部三项、照常上传其余设置，并提示“云端暂不支持主题设置，主题没有上传”；下载的云端快照缺少主题字段时保留本机的值。布尔字段只接收 JSON 布尔值，旧云端快照缺少新增字段时不会部分应用，可先上传本机完整快照；其他平台云端字段由既有合并逻辑保留。

浮动工具栏沿 Windows 前台策略适配 macOS：输入法激活且用户开启工具栏时显示；全屏应用中也会在打字时显示。工具栏跟随输入法的选中而不是输入框焦点：切换应用、点桌面或切到没有输入框的窗口时它保持显示，切换到其他输入源时隐藏。连续 10 秒没有键盘输入时自动隐藏，再次输入时恢复；点击或拖动工具栏会重新计时，后台偏好刷新和普通焦点切换不会唤醒已经空闲隐藏的工具栏。检测只读取窗口几何，不记录应用名称、窗口标题或输入内容。工具栏最左侧的水杉 logo 兼作拖动柄（来源 `ToolbarDragHandle` 的那条 `#8E8CD8` 竖条不再单独绘制），鼠标悬停时显示张开手形光标，按住即可拖动面板，其后是一条分隔线再接按钮；按钮本身不会拖动面板。logo 是品牌标识，不随主题变色；其余配色按主题契约从候选调色板派生：按钮悬停与按下的圆角底色取候选行悬停色（跟随系统时深色为白色 0.10、浅色为黑色 0.06 透明度），分隔线取候选边框色。

账户入口走共享设置页的 `settings:account` 路由；桌面设置不可用时回退到随 bundle 加载的 SwiftUI 账户窗口，不要求手填账户 ID。登录、设置同步、云词典、剪贴板、快照及社区资源由共享账户会话管理。云剪贴板入口同样使用该会话：已登录时打开该账户的剪贴板，未登录或读取会话失败时打开登录窗口，不要求手填 access token。本入口不迁移旧 Keychain 条目。`backend-account-entry` 测试验证原生到窗口的分派及接口缺失处理，`backend-library-load` 验证实际 dylib 的账户类与选择器。

## 输入源菜单与输入模式

输入源菜单按 `InputController.mm` 的 `- (NSMenu *)menu` 收敛为输入模式、输入开关、输入方案加即时工具：中文输入 / 英文输入 / 英文候选模式（⌃⇧E）/ 简体输出 / 繁体输出 / 全角字符（⌃⇧空格）/ 中文标点（⌃.）/ 显示译文 / 「输入方案」下的全拼、双拼（当前双拼方案名）、五笔 86、日语、韩语单选 / 悬浮工具栏（带勾选态）/ 水杉表情面板… / 云剪贴板… / 水杉屏幕键盘… / 手写输入… / 开始/结束语音输入 / 主题（当前主题名，子菜单列出全局主题目录）/ 词库… / 水杉输入法设置… / 关于水杉输入法…。全角与中文标点切的是与悬浮工具栏、快捷键同一份当前应用状态，标点锁定时「中文标点」置灰；显示译文与输入方案、主题改的是保存的偏好，和设置窗口里对应的开关、单选是同一个值，切换输入方案前先完成当前组合。菜单外观仍是系统 NSMenu：行高、圆角和悬停色都由 AppKit 决定，「输入方案」小标题在 macOS 13 上用一行置灰条目代替 `sectionHeaderWithTitle:`。账户、更新、帮助这些管理页统一进设置窗口——把它们逐条列进来会让菜单高过屏幕，而收进二级子菜单又会让常用的工具太难够到。默认中文；英文模式不准备 Engine 会话，按键直接放行。Shift 轻点先上屏原始字母再切换；英文模式切换与英文候选模式开关（⌃⇧E、菜单、悬浮工具栏）取消当前组合并隐藏候选，什么都不上屏，与来源的 `FUNCTION_CANCEL` 一致（韩语方案里正在组合的音节是已经写下的文字，这两种切换都先把它上屏）；Engine 失败时保留原模式。经任何途径切到英文（Shift 轻点、Shift+空格、菜单「英文输入」、Ctrl+空格）都会一并退出英文候选模式，所以再切回中文总是拼音，与来源 `event_listener.cpp` 的 `SetEnglishInputMode(false)` 一致；悬浮工具栏的 En 按钮与「中文输入」在英文候选模式下直接回到中文。Shift+空格默认切换中英文，设置中可关闭；竞争 Command/Control/Option 修饰键不触发切换，重复事件消费但不反复更改偏好。英文模式激活后切回中文会恢复保留会话焦点与设置轮询。英文模式与快捷键偏好保存在宿主自身偏好域，不复用混合输入选项或算法状态。中英文状态按 `ime_mode_scope` 按应用或全局记忆；切到其他输入源（如 ABC）再切回时，两种作用域都从 `default_ime_mode` 重新开始，与 Windows 重新激活 TIP 时一致；在本输入法自身的模式之间切换不会清空记忆。macOS 还提供可由 Tauri 设置页开关的非激活 HUD：切换成功后在当前光标附近短暂显示水杉 logo 与“中”或“英”，自动避让屏幕边缘，不抢焦点、不吞鼠标事件，失活输入源时隐藏；HUD 是品牌徽标而不随主题变化，底色取品牌绿（浅色 `#2C7A4B` 配白字，深色 `#5FBF84` 配深色字）。

输入模式对应来源托盘语言栏的中 / 英 / 日，另加双拼、五笔和韩语：bundle 按中、双、五、日、한、英的顺序声明中文模式 `app.msime.inputmethod.MetasequoiaIME.Hans`（名称「水杉输入法 · 中」/「Metasequoia · ZH」）、双拼模式 `app.msime.inputmethod.MetasequoiaIME.Shuangpin`（名称「水杉输入法 · 双拼」/「Metasequoia · Shuangpin」）、五笔模式 `app.msime.inputmethod.MetasequoiaIME.Wubi`（名称「水杉输入法 · 五笔」/「Metasequoia · Wubi」）、日语模式 `app.msime.inputmethod.MetasequoiaIME.Japanese`（名称「水杉输入法 · 日」/「Metasequoia · JA」）、韩语模式 `app.msime.inputmethod.MetasequoiaIME.Korean`（名称「水杉输入法 · 韩」/「Metasequoia · KO」）与英文模式 `app.msime.inputmethod.MetasequoiaIME.Roman`（名称「水杉输入法 · 英」/「Metasequoia · EN」），六者在系统设置的输入源列表里是六条，图标是水杉 logo 加右下角的 中 / 双 / 五 / 日 / 한 / 英 角标（`MSIMEClientInputMethodMenuIcon{Chinese,Shuangpin,Wubi,Japanese,Korean,English}.tiff`，由 `scripts/render_menu_icon.swift` 生成，`info-plist-icons` 检查各模式的图标互不相同），菜单栏和系统的 Ctrl+空格 切换条据此分得清当前模式；bundle 本身和切换 HUD 用不带角标的 `MSIMEClientInputMethodMenuIcon.tiff`。Ctrl+空格与地球键轮换也会停在英文、日语和韩语条目上。六个模式都默认启用：系统设置的「添加」对话框不单独列出第三方输入法的模式，默认不启用的模式用户无处可加。选中「双」或「五」把方案切到双拼或五笔，在设置页换到这两个方案时菜单栏也跟随到对应条目；此时选中「中」表示全拼，从日语或韩语回来时也一样，否则刚选的「中」会立刻被跟随回「双」或「五」。用户在系统设置里移除了「双」或「五」时，这两个方案显示「中」，选中「中」保持当前方案。悬浮工具栏的模式按钮用同一套角标（中 / 双 / 五 / 日 / 한，英文时为「英」），不论是否添加了上面两个模式；一个字分不出双拼键位，所以按钮的悬停提示和读屏标签以当前方案全称开头，例如「小鹤双拼 · 切换到英文输入」「五笔 86 · 切换到英文输入」。韩语模式对应输入方案「韩语」，与日语模式的规则相同：选中「한」切到韩语，选中「中」回到进入日语或韩语前的中文方案，经系统菜单换方案前先把正在组合的音节上屏。日语模式对应输入方案「日语」：从菜单选中「日」把方案切到日语，选中「中」回到进入日语前的中文方案（`last_chinese_scheme`，未知时为全拼），选中「英」只打开英文模式、不动方案；反过来在设置页或共享设置里改方案，菜单栏同样在「中」与「日」之间跟随。英文模式优先于方案，日语方案下切到英文显示「英」。Shift / 快捷键、输入法菜单与悬浮工具栏切换中英文时，控制器用 `selectInputMode:` 选中对应模式；从系统输入法菜单选中某一条或轮换到它时，系统经 `setValue:forTag:client:` 报告模式，控制器随之切换中英文状态。两边共享一份「当前显示的模式」记录（`src/input/InputModeIdentifiers.h`），只重复当前模式的报告和控制器自己请求引起的回报都不算用户选择，因此不会来回回声；中英文状态按 `ime_mode_scope` 按应用或全局记忆，客户端获得焦点时把菜单栏模式对齐到当前作用域的状态；输入法进程重启后各应用从 `default_ime_mode` 开始，菜单栏同样在获得焦点时对齐。英文或日语模式在系统设置里被移除，或旧安装尚未重新登记（`install.sh`、设置应用启动时的安装与刷新以及设置页的「安装 / 更新」以 `--register-input-source` 登记并启用全部模式，设置页的「重启输入法」以 `--reregister-input-source` 重新登记；带 `SUFeedURL` 的构建经 Sparkle 应用内更新时只替换 bundle，不重新登记，所以输入法启动时由 `MSIMEEnableNewInputModes` 把还没提供过的模式各启用一次，记在偏好 `MSIMEOfferedInputModes` 里——首次记录时中、英、日三个老模式算作已提供，之后用户在系统设置里移除的模式不会再被加回来；设置应用的启动检查从不降级，不会用自带的旧版本覆盖更新的安装）时，控制器不请求这个不可选的模式：英文模式不可选时保持选中「中」，双拼、五笔、日语或韩语模式不可选时对应方案也选中「中」，行为与只有一个模式时相同。大写锁定用系统自带的指示；全半角与标点显示在悬浮工具栏上。macOS 14 起 `selectInputMode:` 可能弹出系统自己的输入源光标提示，与上面的 HUD 重复时可在设置页关掉 HUD。

手写识别板（菜单「手写输入…」或悬浮工具栏）用随包的 zinnia 模型识别：一行横写或竖写的笔迹先按书写方向切成字格，每格各自识别，候选里就有整个词或短语，首位是各格首选拼成的整串，共 12 个、中文在前，与 Windows 来源的多字候选一致；单字或近似方形的笔迹仍按单字识别。切分与组合在共享的 `crates/host-api/src/handwriting_cells.rs`。

打开字符面板同样先完成组合，再请求系统 Character Viewer；测试以替身记录系统入口，不实际打开面板。原生测试覆盖菜单勾选、偏好保存、组合完成/失败、英文按键旁路、七种竞争修饰组合、重复/禁用快捷键、无会话懒加载与焦点恢复。输入法菜单和悬浮工具栏的“检查更新…”优先打开 Tauri 共享 About 页；Tauri 不可用时，有 `SUFeedURL` 的发布包回退 Sparkle 原生更新控制器，未配置 feed 的构建会明确说明不能应用内检查，并在用户确认后打开固定的官方发布页，打开失败也会显示错误，而不是静默无动作。共享 `theme` 与 `candidate_theme` 作用于实际 IMK 候选窗：`dark`/`light` 表面覆盖全局主题，`follow` 继承全局，`system` 通过 `NSPanel.appearance = nil` 交给 AppKit 跟随系统；偏好热更新会保留当前 Engine 组合并重绘候选皮肤。

原生菜单提供“简体输出”（默认）与“繁体输出”，保存到宿主偏好 `MSIMEClientTraditionalOutput`。调用 `msime-host-api` 的 `msime_client_simplified_to_traditional` 导出转换候选显示、完整 tooltip 和最终上屏文本（包括 Ctrl+Enter 上屏的译文、释义页选中的义项与 Option/Ctrl+数字取的释义列；释义页按转换后的文本绘制，选中后上屏的就是页面上显示的那一项），与 Windows、Linux 及来源同用 OpenCC s2t 词级词典（「头发」→「頭髮」而非逐字的「頭發」），输入不是合法 UTF-8 时保留原文；日语方案与 Unicode 精确码点模式不转换。转换只发生在原生展示/插入边界，Engine 原文、候选 ID、组合与运行时视图保持不变。

## 候选辅助码与纠错

候选辅助码后缀由引擎的宿主接口按辅助码注释规则给出（`crates/engine/src/host/`，映射由 `crates/engine/src/helpcode.rs` 加载），按当前会话自身资源目录与辅助码方案加载不可变映射；仅启用辅助码的全拼/双拼普通模式和超级简拼附加后缀，Unicode、日期、快捷短语等合成模式及五笔/日语不附加。未提供共享或本地覆盖时按 Windows 基线使用全拼自然码且隐藏、双拼蓝天且显示。共享 JSON 候选新增 `annotation` 字符串，原 `text` 和候选 ID 不变；macOS 候选面板把后缀（或五笔编码提示）作为文字之后独立的一段绘制，间隔 4 点，同行放不下时移到文字下方并按列宽换行；tooltip 与辅助功能标签仍使用原文加后缀。两者都在繁体展示转换之后显示，上屏仍只提交 Engine 原文。映射缺失时后缀为空，不读取其他会话的全局映射。共享偏好现有延迟应用机制确保组合期间不切换映射；旧宿主无 annotation 字段仍显示原文。

全拼的字母错位纠正与邻键误触纠正默认分别开启，已有对应字段或原生设置显式保存为 `false` 时保持关闭。旧的单一 `autocorrect` 值继续保留在兼容快照中，但不再作为两项纠错的 fallback。

## 双拼与五笔

双拼键位提示面板迁自固定 Apple `ShuangpinKeymapPanel.h/.mm`，候选设置中提供开关，默认关闭，保存在宿主偏好 `MSIMEClientShuangpinKeymap`。开启后仅在有效会话、双拼组词和有效光标时显示；上屏/取消、失活、英文模式或关闭设置时隐藏。保留 620×203 点布局、四套 Engine 权威键位、微软分号键、零声母说明、末尾字母/分号高亮、明暗配色与可访问文本。面板不接受鼠标事件，以非激活浮动窗口显示；位置按上游规则为候选保留间距，避让屏幕边缘。

五笔四码唯一候选自动上屏默认关闭，使用 `MSIMEClientWubiAutoCommitUnique`。仅当 Engine 明确报告原生五笔候选、四码恰好一个候选且本次按键为小写字母时提交；拼音回退、其他方案、非四码或多候选均不提交。该判断完全依赖 Engine 快照元数据，不从候选文本猜测。

共享 JSON 视图新增 `shuangpin_profile`（实际生效的 xiaohe/ziranma/shoudao/microsoft），不使用尚未应用的偏好绘图；字段缺失时隐藏提示。Rust 测试覆盖四方案元数据和组词期间延迟切换，原生测试覆盖四方案×两外观离屏绘制、设置保存、微软键位、高亮/可访问文本、边界定位以及显示/隐藏条件。检查了微软亮色、小鹤暗色合成图；字体测量按 macOS 系统字体进行，不宣称与 Windows 逐像素一致。

## 全角输入

全角输入按同一固定 Apple 版本的 `FullWidthInput.h` 与控制器回退顺序迁移，默认关闭。候选设置中的勾选框（宿主偏好 `MSIMEClientFullWidthInput`，共享字段 `character_width`）是每个应用的起始值；中文模式下 Option + Shift + H、任意模式下 Control + Shift + Space（来源的 Shift + Space 在 macOS 上是中英文切换）以及悬浮工具栏的全角按钮只切换当前应用、不保存，与来源按线程保存在 TSF compartment 里一致，从本输入法切到别的输入源后所有应用回到起始值，起始值一旦改变所有应用都跟着重置；重复按键只消费不反复切换；Option + Shift + H 遇 Command/Control 竞争修饰键或在英文模式下不触发。按键先交给 Engine，已处理的中文组词、选词与标点不再转换；未处理的按键先完成剩余组合，确认空闲后才将 ASCII 空格变为 U+3000、ASCII 可打印字符变为对应全角字符。无会话、失败响应或组合未完成时不插入全角回退，非 ASCII 字符不转换。英文模式没有 Engine 往返：全角开着时，不带 Command/Control/Option 的可打印 ASCII 直接变为全角（空格为 U+3000），与来源输入法关闭时仍按 `FUNCTION_DOUBLE_SINGLE_BYTE` 处理一致；转换表与 Linux 宿主共用 `shared/input/EnglishModeOutput.h`，中文标点同时开着时标点先按中文标点输出。原生测试覆盖 95 个字符、快捷键/偏好、Engine 优先、组合提交顺序及失败/未完成排除，使用替身会话驱动。

## 共享运行时视图字段

共享 JSON 视图新增实际 Engine `scheme`（0 全拼、1 双拼、2 五笔、3 日语、4 韩语、255 未知）。有提交的 transition 携带分派前的 `commit_context`（`scheme`、`local_mode`），无提交时为 null；提交后 `view` 可能已清除局部模式或应用延迟配置，因此不能用它推断本次上屏来源。缺失/未知上下文不转换。Rust 测试覆盖五种选词路径、Unicode 模式重置和延迟切换日语；原生测试覆盖转换、排除路径、偏好与菜单、候选 ID/原文不变。

## 候选外观与皮肤

候选窗、悬浮工具栏与菜单配色跟随全平台共用的全局主题（`global_theme`）：跟随系统、水杉、浅色、纸白、夜青、墨与自定义，列表与标题取自 host C ABI 3 的 `msime_client_theme_catalog`，配色经 `msime_client_resolve_theme` 按当前布局（横排/竖排）与皮肤目录解析，macOS 不再自带配色表。「跟随系统」使用 macOS 原生配色（design-tokens-desktop §1.2：半透明卡片、品牌绿实色选中配白字、10 点卡片圆角与 6 点行圆角）；水杉、浅色、纸白、夜青、墨固定各自的明暗；自定义在所选底色上叠加七个取色器（`custom_theme.candidate_colors`），也可以挂一个外部皮肤包。`CandidateSkin.h/.cpp` 保存解析结果，`CandidateChrome.h` 负责绘制。跟随系统外观变化重新着色；设置变更立即重绘但不改变组合、候选 ID 或高亮。选择存入宿主自身偏好域（`MSIMEClientGlobalTheme`、`MSIMEClientCustomThemeBase`、`MSIMEClientCustomCandidateSkin`），云端同步字段为 `platform.macos.global_theme`、`platform.macos.custom_theme_base` 与 `platform.macos.custom_candidate_skin`。原先的 Fluent、微信绿、石墨、杨柳青四套内置皮肤已随全局主题退役。

外部皮肤读取同一固定来源的 `skin.toml` schema 1：元数据、内置 base、supports 布局/主题列表、明暗候选颜色（含译文色 `translation`）、最小宽度、卡片圆角 `corner_radius_dip`、顶部装饰图（`decoration.image` 与 `align`）、背景图 `candidate_window.background`、悬浮工具栏 `[toolbar]` 的圆角与明暗配色，以及仅作展示的 `license`。宿主只扫描 `~/Library/Application Support/app.msime.macos/skins/<id>/skin.toml`，不读取或改写 MSIME-Apple 产品目录。把包放入该目录后，打开候选设置页或点击“重新读取皮肤”；全局主题在前，外部项按名称排序；选中外部包即选中自定义主题并以包声明的 base 作为底色。`skin.toml` 按完整 TOML 1.0 解析，用的是设置页和其他宿主共用的 client-core 加载器（经 host C ABI 的 `msime_client_skin_package` / `msime_client_skin_catalog` 调用，见 `src/candidate/SkinManifestBridge.mm`），字段校验与 Windows 相同，所以设置页列为有效的包在候选窗和悬浮工具栏里按其配色绘制；输入法内原生皮肤窗口（`SkinSettingsView.mm`）显示的无效原因由该文件换成中文，共享设置页显示加载器给出的原因原文。符号链接的包目录和非普通文件的 manifest 不算皮肤包。无效、缺失或不支持当前布局的包按自定义主题的底色绘制，但保留用户选择的安全 ID。候选配色与图片缓存于设置快照，不在按键重绘时扫描磁盘。

候选装饰画的是加载器给出的 `decorationImage`：manifest 的 `decoration.image`，没有时退回图片格式的 `preview`；按 `align`（`left`、`center`，缺省 `right`）贴齐卡片对应边缘放在顶部留白里并等比例缩放，宽度与顶部留白来自 manifest；缺失或无法解码的图片不绘制，保留声明的布局。`corner_radius_dip` 替换候选卡片圆角（选中行在首尾时随卡片圆角），不影响悬浮工具栏。背景图画在卡片底色之上、描边与候选之下，裁在圆角轮廓内，按 `fit`（`cover` 裁切铺满、`contain` 完整居中、`stretch` 拉伸）与 `opacity` 绘制，只在包实际生效的明暗模式里出现；设置窗口的预览同样绘制圆角、对齐和背景。包给了译文色时（解析结果里 `secondary` 与 `number` 不同），候选窗每一行（含固定位置行）的译文都用它，否则译文照旧取序号色。悬浮工具栏的配色按层叠加：先由主题派生，再叠 manifest `[toolbar]` 当前模式的 `background`（底色）、`border`（描边）、`divider`（分隔线，缺省取描边色）、`icon`（按钮文字与图标）、`hover`（按钮悬停与按下底色）和 `corner_radius_dip`（工具栏圆角，随工具栏缩放），最后才是 `toolbar_stylesheet`，样式表写到的同名项胜出。`handle` 在 macOS 上没有对应控件：拖动把手就是左侧的品牌标志，保持自身配色，所以这一项不绘制。外部 manifest 限 64 KiB，拒绝非普通文件、越界或经符号链接逃逸的包/manifest/资源路径；非法颜色忽略并保留基础配色。`toolbar_stylesheet` 在 macOS 原生悬浮工具栏中采用受限适配：支持颜色、圆角（`border-radius` / `--toolbar-radius`，随工具栏缩放）、边框宽度和内边距；按钮的悬停与按下底色取工具栏调色板的悬停色，样式表里 `:hover` 状态或 `--toolbar-hover` 变量给出的颜色会覆盖它；布局、脚本、图片、滤镜和其他 WebView 专属 CSS 会被忽略，不会注入 AppKit。

皮肤浏览入口优先打开共享 Tauri `settings:skin` 页面，Tauri 不可用时回退到按固定 Apple `SkinSettingsView` 迁移的原生卡片式页面：七个全局主题、外部包描述、每卡独立预览与单选启用开关（固定明暗的主题不提供预览明暗切换），已启用项再次点击不会关闭；「不使用外部皮肤」让自定义主题回到纯取色器配色。外部包通过“打开目录 → 复制皮肤文件夹 → 刷新皮肤”加载，提供空状态和无效包扫描诊断。固定 macOS 源码没有内置导入/删除按钮，本页保持该目录管理流程，不另外创建导入器。目录打开仅由用户点击触发；宿主目录创建/打开失败会显示错误，不触碰 MSIME-Apple 目录。共享页面与原生回退共用同一受控皮肤目录和设置快照。

macOS 原生候选翻译回退窗口与共享 Tauri 设置保持一致：可直接开关不联网的英文释义，并写入同一 `candidate_english_gloss` 偏好。在线候选翻译关闭但离线释义开启时，主、次目标语言仍可编辑；两种释义都关闭时才禁用语言选择。保存仍使用共享快照的 CAS 版本检查，不放宽凭据验证或并发写入保护。翻译服务可选腾讯云、小牛翻译、自定义 DeepLX 或「水杉账号」；没有选择「水杉账号」、也没有填好自己服务的凭据时不联网；只有显式选择「水杉账号」（`translation_account`）才会把当前页的中文候选词发送到 `api.msime.app`，输入法激活时不再预先创建匿名账号。

没有选择任何翻译服务时，macOS 26 及以上用 Apple 的 Translation 框架（`TranslationSession(installedSource:target:)`）在本机为离线词典答不上的中文候选补释义，排在英文词典与非英语离线词典之后，只填它们留空的行。只用系统设置里已下载的语言对，未下载的对 30 秒内不再询问；每种目标语言同时只有一批在途，新页替换排队中的旧页。结果进内存翻译缓存，与原文相同的译文按空值缓存，同一候选不再重复请求。`Translation.framework` 以弱链接载入，macOS 13–25 上不调用。该文件需要 Swift 6.2（macOS 26 SDK）编译，更旧的工具链编译出的后端不含这项功能，输入法经弱引用的入口为空时照常只用离线词典，与 SpeechAnalyzer 语音识别相同。

腾讯、自定义与小牛翻译取回的英文释义与来源的 `PersistGloss` 一致，每次取回成功即写入偏好目录下的用户释义库 `translation-glosses.db`，不必等候选上屏；只存目标语言为英文、格式化后不超过 32 个字符且与原文按 ASCII 忽略大小写不相等的释义，其他目标语言只留在内存缓存。之后的离线释义查询先读这个库，其记录覆盖随包词典中的同词释义；过期的在线回调不写入。托管账号释义不属于来源的服务，仍只在对应候选上屏时写入。

AI 候选与候选释义相互独立：与来源的 `ai_eligible` / `UpdateAiInput` 一致，只由 AI 助手开关 `ai_assistant.enabled`（以及英文模式、特殊模式和纯拼音资格）控制，关闭“显示候选释义”只影响释义与翻译，不阻止或取消 AI 候选请求。与来源出厂模板一致，AI 助手在全新 profile 上默认开启并指向 DeepSeek（`deepseek-v4-flash`）；没有填 Token 时 `chat_completion_http_request` 直接拒绝构造请求，不会产生任何网络流量。

`skin-settings` 原生测试覆盖七个主题卡的选择与保持启用、无副作用明暗预览、系统主题标题、外部卡重复刷新无累积、无效包诊断、空状态、打开目录的路径/失败检查，以及入口窗口构造与重复使用。测试目录打开器为替身，不启动 Finder；只操作临时合成目录，卡片离屏图像逐项检查。

设置窗口的候选预览迁自同一固定版本的 `CandidateSkinPreviewView`，随布局、每页数量、字号和皮肤实时更新。使用固定演示样例而非真实输入；竖排最多展示五行并提示剩余项，横排不足时显示省略号。可单独切换预览明暗主题，也可同时展示横排、竖排及状态栏样式；这些预览操作不写偏好或调用输入会话。预览里的状态栏是非交互展示，实际悬浮工具栏由 `FloatingToolbarPanel.mm` 绘制。外部装饰图和长预览在可滚动区域显示。预览使用注入的设置快照与皮肤目录，不读取 MSIME-Apple 产品设置；强制主题的系统文字颜色在对应绘制外观下解析。

`skin-preview` 原生测试覆盖七个全局主题 × 两布局 × 三页大小 × 三字号 × 两种外观的绘制，设置控件联动、无副作用的预览主题/展示切换、系统外观跟随、外部包回退、装饰区域可见像素和长内容滚动。原生合成 PNG 使用明确的 RGB 数据与 sRGB 标记，避免透明夹具掩盖绘制缺失；像素断言允许系统 ColorSync 转换，不宣称跨显示器逐像素一致。

皮肤验证包含原生明暗两组配色与几何基准、主题目录顺序与各主题解析结果，以及七个全局主题 × 两布局 × 两外观的原生离屏绘制、设置保存和组合保留测试。原生 Objective-C++ 构建启用 `-Wall -Wextra -Werror`。

外部皮肤追加固定源解析测试和路径/颜色拒绝测试，以及生成的 PNG、真实设置控件选择与保存、两布局/两主题装饰位置、`decoration.image` 优先于 preview、左/中/右对齐、包圆角不进入工具栏、背景像素与圆角裁切、译文色、工具栏 manifest 配色与圆角及样式表覆盖、缓存与显式重载回退的原生测试；只使用临时合成包和独立偏好 suite，不操作用户皮肤目录。

每页候选按 Apple `CandidatePageSize.h` 提供 5/7/9 项，默认及非法值归一化为 9。原生偏好在创建/激活会话及设置变更时请求共享分页覆盖；组合期间保持当前页、高亮和数字选词映射，组合结束后使用最后一次选择。此 macOS 原生覆盖优先于共享偏好中的 candidate_page_size，并在共享 Engine 配置重建后保留；不修改共享偏好文件。视图的 page_size 反映实际生效值。原生设置和 C API 测试覆盖保存、延迟、非法值、重复请求与重载。

布局和字号按固定 Apple 来源的 `CandidatePanelStyle.h`、`CandidateFontSize.h` 与原生设置控件迁移：默认横排/18 点，提供横向排列、纵向列表，以及 16/18/20 点字号。“水杉输入法设置…”打开共享设置页的外观页，Tauri 不可用时回退到原生偏好窗口的同一组设置项。值保存到宿主自身 NSUserDefaults 域，不读取或改写 MSIME-Apple 产品偏好，也不改动共享 Engine 配置。已激活候选立即重绘，组合及页内高亮保留。横排左右键导航、上下键消费；竖排上下键导航，左右键与候选隐藏时一样移动组合光标并刷新候选，对齐 Windows 候选显示时 `VK_LEFT`/`VK_RIGHT` 映射为光标移动。候选排版按来源 `CandidateList::MeasureItem` / `Measure`（经 Windows `candidate_item_layout` / `candidate_page_layout` 移植为 `CandidateItemLayout.h`）：候选文字、辅助码和译文比所在列宽时在列内折行，不截断；每行取自身高度，竖排各行按自身高度堆叠；竖排译文放得下时与文字同行，放不下时移到文字下方换行，辅助码一旦下移译文也随之下移；横排译文总在文字下方，每项取自然宽度从左向右排列，放不下的一项起新的一行，只有单项比整行还宽时才收窄到行宽并在其中折行。候选窗宽度以所在屏幕可见区域的一半封顶，预编辑行同样受此限制。

## 候选学习与跟随光标

候选设置还提供候选窗口跟随光标开关，以及候选学习与拼音调频：关闭跟随后，在一次组合期间锁定首次有效光标位置；切换输入会话、结束组合或重新开启跟随时重新定位。学习开关以及关闭、置顶、折半、线性、置前五种调频模式，触发次数和线性步长均为 1–10。值保存在宿主偏好域，并按共享 `candidate_follow_cursor`、`learning` / `frequency` 快照合并；输入会话仍由共享 host-api 在组合结束后延迟应用，Engine 继续负责学习数据和调频算法。旧快照缺少这些字段时保持默认值，不改写原文件。Emoji SwiftUI 面板消费共享 `emoji_theme`：显式深色/浅色覆盖全局，`follow` 继承全局，系统主题保持 `nil` 让 SwiftUI/AppKit 自然跟随；旧快照未提供该字段时沿用全局主题。

## 云候选的首次同意

云候选是唯一一项装完就会把输入内容发出设备的功能：输入过程中把正在输入的拼写通过 HTTPS 发送给 Google 的 input-tools 服务（inputtools.google.com），换回一条额外候选；已上屏的文本、词库内容和学习到的词频都不会发送。全新配置下，输入法第一次激活时弹出「联网功能」对话框说明这一点，按钮为「启用云候选」（默认）与「不启用」，回答之前不发任何云候选请求。对话框不阻塞当前应用的输入，没有回答就在下次激活时再问。升级不问：宿主偏好里已有云候选选择，或输入法第一次判断时共享 `preferences.json` 已存在、Engine 用户数据目录已有内容（用过输入法但从没改过设置的配置也算），就沿用原值。判断结果只做一次，记在宿主偏好 `MSIMEClientCloudCandidatesConsent`；答案写入共享偏好 `cloud_candidates`，之后可在原生设置的「云端与智能候选」或 Tauri 设置输入页的「云候选」里更改，原生设置里改动开关本身也算作回答。实现在 `src/settings/AppearancePreferences.mm`（`resolveCloudCandidatesConsentWithPreferencesDirectory:userDataDirectory:`、`cloudCandidatesEnabled`、`answerCloudCandidates:`）与 `src/input/InputController.mm`（`requestCloudCandidatesConsentIfNeeded`、`presentCloudConsent:`）。

## 语音输入

原生语音波形面板消费共享 `voice_theme`：显式 `dark`/`light` 覆盖全局主题，`follow` 继承全局，全局主题为 `system` 时使用 `NSPanel.appearance = nil` 并让 AppKit 重绘跟随系统。波形、状态文字、转写预览和确认/取消按钮同步切换明暗 palette；主题热更新只改变展示，不取消录音、识别或润色请求。`voice-wave-overlay` CTest 覆盖四种解析路径、系统外观回退和原有动作/转写边界。

语音设置的原生备用窗口提供 CoreAudio 录音设备选择：只列出有输入流且有稳定 UID 的设备，将当前系统默认置顶，按名称/UID 稳定排序；保存 UID 而非易变的序号或显示名，设备暂时不可用时保留选择并让下一次录音明确失败，不静默切换麦克风。空选择使用系统默认设备。共享 `capture_device` 与该 UID 双向同步；`voice-capture-device` CTest 覆盖输入设备过滤、默认排序、UID 缺失和失败路径。Tauri 设置页通过 `list_voice_capture_devices` 提供刷新列表。

原生备用窗口修改的有效语音字段会回写共享 `voice_input` 偏好，避免与 Tauri 页面形成第二套配置；ASR 与润色 Token 都按 provider 槽位保存，切换 provider 会先保存旧槽位再加载新槽位，缺失槽位继续兼容旧扁平字段。豆包鉴权模式与 Tauri 同步支持新版 API Key 和旧版 App ID + Access Token，缺失模式时按已有 App ID 兼容推断。文本润色开关同时维护 `polish_text` 与兼容的 `polish_enabled`，保证所有原生请求路径一致。损坏的 provider 值回退到安全首项，缺失或非法的本机默认值不会覆盖共享字段。凭据不纳入云端外观同步，CoreAudio 设备仍以稳定 UID 保存。共享快照写进 NSUserDefaults 之前，润色服务的原生回退值统一是 DeepSeek（`https://api.deepseek.com/chat/completions`、`deepseek-v4-flash`，定义在 `SharedVoicePreferences.h`），与共享层在 macOS 上的首次运行默认一致，原生语音设置窗口、备用 provider 窗口与录音请求不会各报一个 provider。

录音提示音是产品自己的 `start.mp3` / `end.mp3`，CMake 直接从 `platforms/windows/installer/assets/audios/` 放进 bundle 的 `Resources/audios/`，仓库里不另存一份。`VoiceCuePlayer.mm` 启动时一次性解码为 `NSSound`，每次播放先 `stop` 再 `play`，与来源 `cue_player.cpp` 一样从头重播。某一个文件缺失或解码失败时该侧回落到系统的 Glass / Pop 并写一行日志，而来源此时不出声：这里宁可保留一个能听见的开始 / 结束反馈。`voice-cue-player` 与 `bundle-contents` 覆盖这两点。

「录音时静音其他声音」按来源的时序：开始提示音播完再静音，结束、取消与失败时先恢复再播结束提示音，所以两段提示音都听得见；录音在开始提示音播完前就结束时不会再去静音。macOS 13（部署目标）没有只静音其他进程的公开接口，`VoiceAudioMuter.mm` 仍静音整台默认输出设备；14.2 起的 CoreAudio process tap 能做到，但要抬高部署目标并申请「系统录音」权限，为一个静音选项不值得。静音期间监听默认输出设备，插拔耳机或连上 AirPods 时恢复原设备、静音新设备；用户自己已静音的设备不接管，事后也不解除。崩溃恢复记录最多同时记 8 台欠恢复的设备，只欠一台时写旧的 `version 1` 格式；仍找不到的设备留在记录里，但不挡住下一次录音静音当前设备。`voice-audio-muter`、`voice-mute-recovery` 与 `http-voice-controller` 覆盖切换、恢复与提示音顺序。

录音时长按实际上传的格式封顶，不再在 60 秒处作废。批量识别上传的是共享层编出的 16 位 WAV，上限与来源同为 20 MiB（`shared/voice/VoiceProviders.h` 的 `batch_upload_sample_limit`，16 kHz 下约 655 秒），再扣掉 SiliconFlow 两端的补静音，保证录满的一段对每家都编得出来。本地模型不走批量上传，经 `msime-voice-local` 助手流式识别，不受这条上限约束。录满上限时宿主像用户松开一样结束录音：浮层转为「识别中...」、播结束提示音、提交已录的部分。来源的做法是攒下全部音频、提交时报「超过 20 MiB 上传限制」并丢掉整段；这里提交前 11 分钟的文字，内存也有界。豆包流式不设长度上限，缓冲只保留尚未发送的样本，只保留 30 秒请求超时与结束后等最终结果的 30 秒，不再有按 60 秒录音估出的 100 秒整场时限。

识别失败时浮层状态行显示类别文案，正文区显示服务商给出的原因，文案逐字取自来源：HTTP 失败取服务商 JSON 的 `error` / `message`（附 `code`），取不到则显示 `HTTP N`，SiliconFlow 5xx 附追踪 ID；豆包分别报连接失败（按新版 API Key 或旧版 App ID + Access Token 提示该检查哪项）、握手失败与带 `code` 的服务端错误。原因来自 `shared/voice` 的 `CloudAsrError` 与 `VoiceFailureMessages.h`，不含 API Key、请求头或请求体；共享错误的 `what()` 不变，Windows 宿主不受影响。没填 ASR Token 且没有共享 provider socket 时，在请求权限和开始采集之前就在浮层提示去设置里填写，不弹模态窗口。来源的失败提示是模态消息框，这里留在浮层上。

浮层的尺寸与布局照来源 `wave_overlay.cpp`：录音时 78×32 的紧凑波形，识别或润色时 112×40 的「识别中...」/「处理中...」胶囊，锁定录音或等待结果时 142×40、两端带圆形「×」/「✓」的操作条，流式转写时 420×112 的转写面板，最多 3 行，放不下时丢掉最早的文字并以「…」开头，切点落在组合字符边界上。12 条波形每 16 ms 刷新，只在浮层显示波形时计时。圆形按钮只在按住录音后按 Space 锁定、或识别与润色进行中出现，点击由透明按钮接收并保留「取消」/「确认」无障碍标签，只有操作条可见时浮层才接收鼠标。浮层位于可用区域底边上方 10 pt。与来源的两处不同：锁定且有转写时操作条内不画转写；三条识别路径停止录音后一律显示「识别中...」胶囊加按钮。

## 菜单主题

原生输入源主菜单和候选右键菜单共同消费共享 `menu_theme`：显式 `dark`/`light` 覆盖全局主题，`follow` 继承全局，`system` 交给 AppKit；菜单每次创建时读取最近一次偏好快照，不改变候选动作或输入会话状态。

## 标点

中文标点的勾选框（宿主偏好 `MSIMEClientChinesePunctuation`，共享字段 `chinese_punctuation`）同样只是每个应用的起始值：Ctrl + . 与悬浮工具栏的标点按钮只切换当前应用、不保存，中英文切换让标点重新跟上模式，对应来源的 `SyncPunctuationWithImeMode`：进入英文模式时标点变为英文（标点锁定固定为中文时除外），回到中文模式时丢掉当前应用的切换、回到起始值；从本输入法切到别的输入源后所有应用回到起始值，普通的焦点切换不会。英文模式与来源输入法关闭时一样仍转换中文标点：标点锁定固定为中文，或锁定跟随且进入英文模式后用 Ctrl + . 或工具栏打开了中文标点时，不带 Command/Control/Option 的 ASCII 标点按 Engine 正向表换成中文标点（引号交替、书名号嵌套，模式切换或换客户端后重新开始），小键盘不转换，其余键交给应用。英文模式下标点锁定固定为中文或英文时，Ctrl + . 与工具栏保持锁定值，对应来源的 `ResolvePunctuationOpen`。成对标点和标点锁定跟随 Windows 基线迁移到候选设置。成对标点默认开启；组合中按左符号键时引擎把候选连同左符号一起提交（`nihao(` 上屏 `你好（`），宿主按提交末字符照样补上右符号、把引号改写为左引号，而用空格或数字选中的、本身以引号或括号结尾的候选原样上屏。中文标点与成对标点都打开时，`{` 上屏 `{` 并把 `}` 留在光标后（全角输入为 `｛｝`），组合中则先上屏高亮候选再开这一对，与来源 `_GetPairedPunctuationClosingFor` 及 Linux 宿主一致。标点锁定默认跟随中文标点，也可固定为中文或英文；设置值分别写入宿主偏好域，并按共享 `paired_punctuation`、`punctuation_lock` 字段同步。输入会话只通过 `MSIMEClientSession` 调用 Engine 的运行时覆盖，重建会话时恢复覆盖值；原生控件和替身会话测试覆盖默认、持久化、非法值和同步路径。

智能标点和重复标点转中文按 Windows 基线适配到 macOS 编辑器上下文：空闲态先看宿主最后送到应用的一个字符，这个记录被清掉时才通过 NSTextInputClient 读光标前一个 Unicode scalar，据此判断逗号、句号和冒号前是否为 ASCII 字母/数字，并按「数字后直出」「字母后直出」两个开关分别决定是否保留 ASCII。智能标点、重复标点转中文和这两个直出开关与来源一样默认关闭，需要用户主动打开。组合态依据高亮候选末尾走 Engine 的 ASCII 标点入口。短时间在同一编辑器重复输入同一标点时，将前一个 ASCII 或全角标点替换为对应中文标点，与「成对标点」开关无关（重复键只有逗号、句号和冒号，都不成对），与来源一致；切换客户端、退格、关闭选项或不满足上下文时清除重复状态。全角输入仍优先转换为全角标点，小键盘物理键路由优先于智能标点。测试使用合成 NSTextInputClient，不保存或记录真实输入。

宿主在所有应用里都记住最后送到应用的一个字符（上屏的末字符或直通的可打印键），直出判断先看它。Terminal.app、iTerm2 与 VS Code 终端这类宿主读不回正文，也不接受按范围替换，没有正文可以退回去读，全靠这个记录：在终端里输入 `1` 再按 `.` 照样直出 ASCII。退格、Delete、回车、Tab、Esc、方向键、Home/End、翻页、Control/Option/Command 组合键、被输入法吃掉却没有上屏的键、切换客户端与失焦都会清掉这个记录，此后回到读文档。重复标点转中文与「中文标点后按空格转换」在这些宿主里改为向前台应用投递一次 Delete 加一个 Unicode 按键来改写，只在折叠光标处连正文首字符都读不回时才这样做：普通编辑器里用鼠标拖出的选区、或光标点到正文开头时读不到前一个字符，但正文可读，不投递按键，空格和标点键照常交给 Engine，需要已授予辅助功能权限，且前台应用仍是该客户端、距按键不超过 500ms；输入法只检查权限，不会在打字时弹出授权请求。未授权时不改写：终端里重复按 `.` 得到 `1.。`，空格按普通空格输入。

「中文标点后按空格转换」使用空格作为改写手势：成功时把刚上屏的 `。，！？；：、` 或单独出现的引号、方括号、书名号和圆括号替换成对应 ASCII 字符，并消费空格，不向正文追加空白。标点键经 Engine 提交时，宿主按提交文本的末字符布防，包括组合中按标点把候选连同标点一起提交（`nihao,` 上屏 `你好，`，空格后成为 `你好,`）；目标 ASCII 取自标点映射而不是按下的键，所以反斜杠键输入的 `、` 转成 `/`。成对标点自动补全的左半边、用空格或数字选中的以标点结尾的候选都不布防。宿主在改写前重新读取光标前字符；切换编辑器、夹入其它按键、移动到不同字符、存在组合、全角输入或成对标点仍有自动补全尾部时均放弃改写。打开「重复标点转中文」时，改写成功后 2 秒内在同一编辑器再按同一个标点键（Shift 不影响），宿主会把光标前的 ASCII 字符原位改回原来的中文标点（右引号仍是右引号），该键被消费，只生效一次；切换编辑器、按其它键、退格、光标前已不是该 ASCII 字符、存在组合、全角输入或成对标点仍有自动补全尾部时都取消，这个键照常交给 Engine。该规则与共享 `client-core` 策略及固定 Windows 来源一致。日语输入方案（`scheme` 为 3）下空格转换和重复标点转中文都不改写已上屏的标点，空格按普通空格交给 Engine，字母或数字后直出 ASCII 仍然生效，与 Windows 来源 `KeyEventSink` 只在 `!JapaneseInputModeEnabled` 时处理这两个可逆手势一致；临时日语模式不参与该判断。

韩语输入方案（`scheme` 为 4）是两套式（두벌식，Dubeolsik）谚文音节自动机，没有候选、汉字转换、云候选、学习和句子模型。字母键以所打的 ASCII 字母经 `msime_client_character` 发给 Engine，大小写只由 Shift 决定（Shift+Q/W/E/R/T/O/P 是 ㅃ ㅉ ㄸ ㄲ ㅆ ㅒ ㅖ），大写锁定不改变字母也不把字母直接放行；Shift+字母不进入临时模式。正在组合的音节总是作为 marked text 显示在光标处，不受「输入码显示」偏好影响；开始下一个音节时上一个音节随同一次 transition 上屏。空格、回车、数字、方向键、Home/End、向前删除、Tab 与带 Command/Control/Option 的组合键先把音节上屏，再交给应用照常处理；标点总是半角 ASCII，组合中与音节一起上屏，空闲时交给应用，不受中文标点开关、标点锁定、全角开关和成对标点 `{` 补全影响，空格转换与重复标点转中文也不生效。Esc 放弃音节，退格一次删一个字母（jamo）。失去焦点和切换客户端时 Engine 把音节提交给离开的客户端，而不是取消。关闭方向键或翻页键的候选导航后，这些键交给应用移动光标，也先把音节上屏。

## 混合输入

中英、Emoji 和颜文字混输设置跟随 Windows 的 `mixed_input` 偏好迁移到 macOS。中英混输默认开启，触发阈值默认 5 个字符，Emoji 默认开启（与来源出厂的 `emoji_mixed_input = true` 一致）、颜文字混输默认关闭；阈值限制为 1–8，关闭中英混输时控件保留数值但暂时禁用。设置写入宿主偏好域，活动组合期间由共享 host-api 延迟应用，平台不复制 Engine 的候选混排算法；原生测试覆盖共享读取、持久化、控件联动和非法值保护。

## 候选窗口翻页按钮与定位

多页候选在卡片顶行右侧显示页码「1 / 3」（12 点等宽数字，可访问标签「第 1 页，共 3 页」）与 `‹` / `›` 鼠标翻页按钮（28×26 点），顶行左侧是预编辑串；横排与竖排相同，翻页按钮不再占用候选行的宽度或竖排底栏。单页不显示，首页/末页禁用越界方向，并提供中文可访问标签。候选译文固定为 12 点，不随候选字号放大。页码和候选由共享运行时返回，点击分派既有翻页命令；回调校验会话、组合代次、原页面与焦点，防止旧按钮影响新输入。原生测试覆盖实际按钮点击、方向禁用、单页隐藏和过期回调。滚轮翻页对齐 Windows `candidate_wheel_paging.h` 的累加语义：触控板精确滚动累计满 40 点（WebKit 一格滚轮的距离）才翻一页，多格拆成多次翻页，惯性事件不翻页，反向、手势开始、面板隐藏与候选刷新都清空残留；传统滚轮一个事件翻一页。

候选定位与焦点约束对照同一 Apple 提交的 `CandidatePanel.mm`：无效光标隐藏窗口、以光标垂直中点选屏、与光标相隔 4 点、底部不足时向上放置，超大窗口至少锚定可见屏幕原点。候选窗口不能成为 key/main window，按钮不接受键盘焦点但支持首次鼠标点击。宽度随候选文字变化，以屏幕可见区域的一半封顶，且至少是候选字号的 7 倍（与来源皮肤的 `.container { min-width: 7em }` 和 Windows `CandidateCardSize.h` 同一规则），皮肤包的 `min_width_dip` 与装饰宽度只能抬高这个下限，7em 下限本身不超过上述屏宽封顶；竖排行铺满卡片，横排列保持自然宽度靠左排列、余量留在右侧；超出列宽的文字在列内折行而不截断，tooltip 仍给出完整文本；候选行使用主题解析出的行圆角（原生为 6 点，高亮连段的首尾行取卡片圆角），并区分普通行和选中行的 hover 颜色。

## 构建与本地测试

`src/` 下每个源文件都必须被某个 target 编译。固定 Apple 快照里那对直连引擎的适配器（`MetasequoiaInputController.mm` 与 `DictionaryRuntime.mm`）从未参与构建，已随 C++ Engine 一起删除；产品用的是 `src/input/InputController.mm` 与 `src/core/ClientDictionaryRuntime.mm`。#912 就是把一个输入会话的修复提交进了那份不参与构建的 `DictionaryRuntime.mm`，通过评审、合并，而产品一行没变；`scripts/test-macos-orphan-sources.py` 列出 `src/` 下不被 CMake 编译的源文件，新出现的一个会直接失败。

并行开发时使用独立产物目录，避免其他平台构建覆盖最低系统版本设置：

最低系统版本用 `CFLAGS` / `CXXFLAGS` / `CMAKE_OSX_DEPLOYMENT_TARGET` 分别交给 C、C++ 与 CMake，**不要**改回一个全局的 `MACOSX_DEPLOYMENT_TARGET`。rustc 会把该变量一并应用到为宿主编译的 proc-macro 动态库上，而它随后加载不了自己产出的这个库，于是冷缓存构建以 `can't find crate for zerofrom_derive` 失败；cargo 不把这个变量算进指纹，坏掉的 proc-macro 会留在产物目录里，之后即使不再设置该变量也继续复用，失败因此看起来时有时无。上面的写法只影响真正需要最低版本的 C/C++/CMake 目标，链接时没有版本不匹配告警；Rust 目标文件按 rustc 默认的 11.0 产出，低于 13.0 下限，不会抬高最终 bundle 的最低系统版本。

原生更新控制器依赖固定的 [Sparkle 2.9.6](https://github.com/sparkle-project/Sparkle/releases/tag/2.9.6)，不可省略。下载该发布的 `Sparkle-2.9.6.tar.xz`，用 `shasum -a 256` 校验为 `52bf9e88cdd972fc0c81501377a880e90d47031bd8ca5462488f843e2609e192` 后解压到独立依赖目录。下列 `MSIME_SPARKLE_ROOT` 必须指向包含 `Sparkle.framework` 的目录，不是框架内部；请替换示例绝对路径。CMake 校验框架版本，并将其复制到应用的 `Contents/Frameworks`。构建不会自动下载框架，也不应从其他已安装应用复制依赖。

```sh
CFLAGS="-mmacosx-version-min=13.0" CXXFLAGS="-mmacosx-version-min=13.0" CMAKE_OSX_DEPLOYMENT_TARGET=13.0 CMAKE_PREFIX_PATH="$(brew --prefix)" CARGO_TARGET_DIR=target/macos-cargo cargo build -p msime-host-api --locked
cmake -S platforms/macos -B target/macos-isolated -DMSIME_HOST_LIBRARY="$PWD/target/macos-cargo/debug/libmsime_host_api.a" -DMSIME_SPARKLE_ROOT="/absolute/path/to/Sparkle-2.9.6"
cmake --build target/macos-isolated --parallel
ctest --test-dir target/macos-isolated --output-on-failure
```

原生测试使用不显示窗口的面板子类验证布局、完整 tooltip 和无效光标隐藏，以及纯几何边界和焦点方法。

## 剪贴板本地测试

剪贴板回归注册在本地 CTest 里。完成上述配置后，可单独构建并运行：

```sh
cmake --build target/macos-isolated --target macos-clipboard-tests --parallel
ctest --test-dir target/macos-isolated -L clipboard-local --output-on-failure
```

该标签涵盖历史适配与观察、采样过滤、监听重试与取消、单服务生命周期、行/提示像素、窗口释放、复制与最近记录、开启设置的保留语义。测试仅使用合成回调和独立命名的剪贴板，不启动输入法、不访问系统通用剪贴板或真实历史。Swift 测试以当前构建主机架构和配置的最低 macOS 版本编译，显式保持断言开启；每项有 30 秒超时。

## 表情与剪贴板面板

贴纸和 GIF 按固定 Windows 版本提供首页“更多”入口、主导航页及内容源未接入提示；该版本自身没有媒体内容源，本实现不新增第三方服务。进入这些页面保留搜索文字，使用通用搜索占位提示，不读取目录数据库，不显示分类、分页、键盘选项或插入按钮。首页入口在目录加载中或失败时仍可使用。首页测试覆盖页面标识、提示文字与浅色／深色原生提示渲染。

复制及开启剪贴板反馈采用固定 Windows 版本的底部胶囊提示：持续 1.6 秒，新操作重新计时，切换页面立即关闭，不占内容布局也不拦截点击。复制成功提示去除 CR/LF 后显示最多 24 个 UTF-16 单位的预览和省略号，截断时避免破坏代理对；失败提示不包含复制内容。字体、内边距、高度、底部距离及两种主题透明度按原版 2/3 比例绘制。提示测试使用合成文本及注入的等待函数，覆盖过期计时器隔离、取消、文本边界和原生渲染像素，不访问真实剪贴板。

### 导航与分类

表情主导航只在首页显示，顺序按固定 Windows 的 Page 枚举与图标表校正为首页、表情、贴纸、GIF、颜文字、符号、剪贴板。最近使用属于表情页二级分类：从首页入口或“更多”进入表情时，有最近记录则优先显示最近，无记录则在分类元数据载入后选择第一个分类；从最近切换具体分类保留明确选择。最近使用为空时，共享 Tauri 面板与原生回退面板都按来源 `EmojiPanel.cpp` 显示 `Your recently used items will appear here`（英文原文，来源无本地化），输入搜索时也不变；已有最近记录但无搜索匹配时显示 `No results`。详情页显示返回首页及对应分类／标题，切换保留搜索文字。原生输入源剪贴板采集与来源 `NormalizeClipboardText` 规范化方式一致：剥掉尾部 NUL 与 `\r`，超过 4000 个 UTF-16 单位的复制截取前 4000 个单位保存，不拆开字符（含补充平面字符），因此始终不超过 12000 个 UTF-8 字节，不会整条丢弃；历史适配器和 Tauri 监视器按同一 4000 单位／12000 字节边界接受条目，中日韩文字不会被旧 4096 字节边界提前拒绝。NUL 以外的控制字符（换页、ESC、DEL、C1 等）作为用户内容保存、显示和粘贴，只有含 NUL 的文本被拒绝。输入源采集和 Tauri 监视器按同一份类型表跳过不含纯文本、或带 nspasteboard.org 标记类型（Concealed／Transient／AutoGenerated）及密码管理器与片段工具私有类型（如 `com.agilebits.onepassword`）的剪贴板变更，不记录其内容；读取期间剪贴板计数变化的一次不配对旧类型，留给下次轮询重读。测试覆盖入口顺序、默认路由、异步分类解析、截断与控制字符规范化以及 Unicode 容量边界。


主标签按 `emoji_panel_icons.cpp` 的码位依次解析已安装的 Segoe Fluent Icons / Segoe MDL2 Assets，逐字检查是否存在字形；不下载、捆绑或安装字体。两者都不可用时使用原版中文回退文字，其中首页的可见回退为“最近”，但辅助功能标签及路由仍为首页。标签相对宽度、间距、高度、悬停颜色、选中下划线及字号采用原版 2/3 比例；本地测试强制覆盖缺字回退，检查两种主题的七种选中位置像素。Windows 字体图形在缺少相应字体的主机上走中文回退，不宣称与 Windows 逐像素一致。

表情二级分类使用原版区分大小写的分类名称映射，最近使用显示秒表图标，未知名称使用微笑回退。符号大类按元数据顺序取各大类第一个符号作为图标，默认选择首个有效大类；元数据和图标一并加载，取消页面任务会取消后续图标读取。分类标签宽度不超过原版 52 单位的 2/3，并随可用宽度均分收窄；标题通过辅助功能标签与悬停帮助提供。符号搜索忽略当前大类和子分类，匹配原版全局搜索行为。测试使用合成目录回调验证映射、顺序、缺失／失败、搜索过滤及两种主题的选中下划线像素。

符号内容按目录中连续的子分类分段展示标题及独立六列网格，移除额外的子分类下拉过滤。标题高度、半粗字号、相对缩进和分组尾部留白采用固定 Windows 版本的 2/3 比例；相同标题的非连续分组不合并、不重排。分组内按钮保留完整结果的全局索引，复制和键盘步长不因独立起行而指向其他条目。批次拼接后统一分组，读取批次边界不再拆分标题。合成测试覆盖重复标题、分组边界、全局索引、部分行之间的导航及两种主题的原生尺寸／选中像素。

### 目录读取游标

连续目录读取的宿主接口提供可选 `cursor:true`：`msime_client_emoji_catalog_request` 返回 `items`、`next_offset` 和 `complete`。详情 UI 已移除手动分页，按 `next_offset` 连续读取，仅在 `complete:true` 后一次性显示完整结果，不用返回条数或空数组判断结束。游标按实际扫描的匹配行前进，跳过空文本／无效分类行但不提前终止，保留有效重复条目；恰好满批时再读一批确认结束。切换查询取消后续批次，失败不展示部分结果。读取前后检查数据库及 WAL 的设备、inode、大小和修改时间，普通替换或写入会使本次读取失败；此防护不是跨请求 SQLite 快照，无法保证检测元数据不变的原地修改。未提供该选项的旧分页接口继续逐页去重，响应格式不变。首页预览仍保留各分类数量限制。合成游标测试覆盖空批次继续、重复项、协议错误、取消、失败和文件变化。

首页预览也使用游标接口，按有效条目数读取到预览上限或明确 EOF；空扫描批次继续前进，保留重复项，不再使用旧分页去重结果。批次大小不超过剩余预览配额，达到配额前仍检查取消和文件变化。完整详情仍读取到 EOF，不受预览配额影响。重复条目的 UI 标识增加同文本／同分组的出现序号，绘制、选择及键盘导航共用该标识。合成测试覆盖空批次、重复项、短目录、非法配额、达到配额时文件变化以及重复项导航；文件检查仍非跨请求数据库快照。

### 标题与搜索框

首页标题对齐原版 `Recently used`、`Emoji`、`Kaomoji`、`Symbols`、`Sticker`、`GIF`，采用 32 点标题行、12 点半粗字及 12 点组尾留白，移除额外组间距及空组提示。除最近记录外，整个标题行与右侧箭头均可进入对应分类；箭头按原版几何绘制，24 点点击图形区域使用原版悬停／按下颜色。首页测试覆盖两种主题下六个标题的渲染尺寸。

普通表情详情显示当前分类标题，最近记录标题为 `Recent`，颜文字标题为固定原版目录的 `All`，与符号页共用 32 点标题高度、12 点半粗字及 12 点组尾留白。颜文字移除原版没有的分类下拉框，搜索无匹配时不显示其分组标题。内容仍使用原网格／流式布局及索引，标题不参与键盘条目计数。分组测试覆盖两种主题下三个标题的空／单项／跨行尺寸，并继续检查符号分组顺序、导航和选中像素。

搜索框占位文字对齐固定 Windows 提交的 `UpdateSearchPlaceholder`：首页为 `Search emoji, kaomoji, and symbols`，表情／最近为 `Search emojis`，颜文字为 `Search kaomoji`，符号为 `Search symbols`，贴纸／GIF 为 `Search`，剪贴板为“搜索剪贴板”。普通页输入／占位字号为 14／12 点，剪贴板均为 16 点；搜索修改立即清除复制提示。搜索测试覆盖页面映射、字号配置以及两种主题、两种焦点状态的边框像素。

### 滚动与选择

首页和详情的滚动容器在搜索、页面／分类切换时重建视口，回到顶部；重复点击当前主标签或二级标签也触发重置。只重建滚动内容，不重建搜索框和键盘入口。普通最近记录／剪贴板刷新不改变滚动身份，内容尺寸变化仍可能由系统钳制位置。原生合成测试实际滚动 SwiftUI 内的 NSScrollView，验证查询及重复导航重置到顶部，并验证普通刷新保留滚动位置。

首页重置后的有效选择为首个可见条目，选中样式和键盘导航共用此选择。空分组自动跳过，最近记录优先，异步目录到达后无需先按方向键即可激活首项；第一次右键／下键从首项按原步长移动。显式选择仍按条目标识保持，条目被移除后激活不回退复制其他条目。合成测试覆盖空目录、分组顺序、最近记录、目录到达、初始激活及步长、显式选择和过期选择。

### 面板本地测试

原生宿主游标集成测试 `emoji-host-cursor-test` 直接链接 `apple-client` 与当前构建的 Rust host 静态库，经真实 `MSIMEClientSession.emojiCatalogRequest` 读取临时合成 SQLite 数据库。覆盖表情／颜文字／符号的空扫描批次、重复项、短尾批次、显式 EOF、旧接口去重及响应格式、相对路径拒绝。此测试验证 Objective-C → Rust → SQLite 链路，不使用模拟宿主，不读取真实输入。运行前需从当前提交重新 `cargo build -p msime-host-api`，并将产物传给 `MSIME_HOST_LIBRARY`。

```sh
cmake --build target/macos-isolated --target emoji-host-cursor-test --parallel
ctest --test-dir target/macos-isolated -R '^emoji-host-cursor$' --output-on-failure
```

`emoji-swift-host-test-build` 进一步使用实际 Swift `MacEmojiCatalog`，通过动态类／selector 查找调用真实 Objective-C → Rust → SQLite 链路。临时合成数据库包含首批 255 个无效行、260 个相同有效条目及一个尾项，验证完整读取跨批次推进、18 项预览、搜索无匹配、分类元数据与符号父分类过滤。静态宿主显式 force-load，未定义模拟 `MSIMEClientSession`。这覆盖 Swift 读取器与宿主协议的兼容性。

```sh
cmake --build target/macos-isolated --target emoji-swift-host-test-build --parallel
ctest --test-dir target/macos-isolated -R '^emoji-swift-host$' --output-on-failure
```

表情首页、普通网格与颜文字流式布局同样提供独立的本地测试（搜索框使用 `emoji-search-test-build`，滚动使用 `emoji-scroll-test-build`，同属 `emoji-local`）：

```sh
cmake --build target/macos-isolated --target emoji-home-test-build emoji-flow-test-build emoji-grid-test-build emoji-toast-test-build emoji-main-tabs-test-build emoji-category-tabs-test-build emoji-symbol-sections-test-build emoji-catalog-cursor-test-build --parallel
ctest --test-dir target/macos-isolated -L emoji-local --output-on-failure
```

### 布局度量

首页预览和完整颜文字目录共用文本测量、按宽度换行和长文本缩字逻辑，键盘上下移动使用同一布局的行号及横向中心。间距、留白、最小宽度、字号边界和换行容差以 Windows 固定提交 `04a8df56f86312474a069f4335a1b58da7afaa9e` 为准，应用其 2/3 面板比例；字体测量使用 macOS 系统字体，不声称与 Windows 字体逐像素一致。测试覆盖合成文本的换行、缩字、窄宽度边界、不同宽度下首页导航与流式导航一致性，以及 SwiftUI 原生渲染的尺寸与选中背景像素，不读取真实输入或剪贴板。

普通表情、符号及最近记录使用与该固定 Windows 版本一致的六列网格：原始 84 单位步长及 8% 单元留白应用 2/3 比例，原生点数为 56 步长、51.52 单元尺寸。文本按 UTF-16 长度大于 4 判定为长文本，并按原版字号上下界缩字。选择圆角及边框也应用相同比例。首页和详情页共用网格；剪贴板仍为独立单列列表。网格测试覆盖两种主题的选择填色、边框、间隙和末行留白像素，以及 1/6/7/18/28 项排布与最多 255 项导航边界。原生窗口最小内容宽度包含完整六列和滚动条留白，不强行压缩单元格；字体测量仍为 macOS 平台适配，非 Windows 字体逐像素复现。

## 按键处理与翻页快捷键

Home/End 在候选可见时通过共享运行时移到当前页首/末候选，不改变编辑串、光标或提交文本；最后不足一页时止于实际末项。候选隐藏后沿用编辑光标 Home/End。测试覆盖可见/隐藏状态、完整页及末页、过期候选和全局索引提交。翻页快捷键是共享 `navigation` 偏好里的一组独立开关，可以任意组合：`minus_equal`（减号/等号）、`comma_period`（逗号/句号）、`brackets`（方括号）、`tab`（Tab/Shift+Tab）、`page_up_down`（Page Up/Page Down）、`arrows`（方向键选择候选）和 `mouse_wheel`（在候选面板上滚动滚轮翻页）。某个开关在共享偏好和本机 `MSIMEClientNavigation` 字典里都没有值时才回退到默认：`minus_equal` 与 `brackets` 由旧的 `candidate_page_shortcut` 预设（本机 `MSIMEClientCandidatePageShortcut`）推出，预设 0（默认）开减号/等号、1 开方括号、2 两者都不开；`comma_period`、`tab`、`page_up_down`、`arrows` 默认开启，`mouse_wheel` 默认关闭。

字符键按物理 ANSI 键位判定，仅在候选可见且无 Shift/Command/Control/Option 时翻页；与以词定字占用同一组键时由以词定字处理，日语方案的减号/等号和 Unicode 模式的 `+` 仍交给 Engine；开关关闭或带 Shift 时将实际字符交给 Engine。候选可见时 Tab 向后翻页、Shift+Tab 向前翻页；若高亮候选带释义（`translation` 有第一或第二列），Tab/Shift+Tab 先在不选、第一列、第二列释义之间循环选中，不发翻页命令，也不受 `tab` 开关影响，没有释义的候选照常翻页。候选可见时，已关闭的 Tab/Shift+Tab、Page Up/Page Down 与 ↑/↓ 会被输入法吞掉，编辑串和候选保持原样，与 Windows 一致；没有候选显示时这些键交还应用。候选不可见时 Tab/Shift+Tab 无论开关如何都交还应用，交还前若仍有编辑串或候选，先通过 Engine finish 提交当前组合，避免组合残留在原输入框；Page Up/Page Down 与 ↑/↓ 关闭后交还应用时，只有韩语方案会先结束当前音节。设置保存在宿主原生偏好域，测试覆盖默认、非法值归一化、控件保存、48 种字符组合以及修饰键优先级。

迁移对照固定为 MSIME-Apple 远端默认分支 develop 的提交 `b637828e15eafcb5e459edd270a962dd14517285`。Command、Control、Option 快捷键沿用其 `MetasequoiaInputController.mm` 行为：先通过 Engine finish 提交当前高亮对应组合，再放行快捷键。原生控制器测试覆盖三个修饰键的分派、提交、清空预编辑和返回未处理，使用替身会话驱动。

真实 IMKServer / IMKInputController 入口，静态链接共享 Rust/C++ 运行时。平台代码负责系统按键、文本插入、预编辑和不激活候选面板。分页、高亮、会话代次、候选选择与组词仍由共享层负责。

按键处理覆盖 ASCII 输入、退格、移动编辑光标、空格选择、回车原文、Esc 取消、候选上下移动和翻页、鼠标选词，以及由共享运行时处理的当前页数字选词与标点结束组词。候选面板显示对应数字；Engine 优先接收字符，保留 Unicode 等输入模式与拼音分隔符。未被 Engine 接收的 ASCII 标点先按当前高亮完成组词，再复用 Engine 中文标点转换；关闭中文标点时保留 ASCII。设置后台重读、输入法菜单和 Tauri 安装入口均已接入。原始 ASCII 编辑串用于内联预编辑，光标单位与 Engine 一致；日语等美化预编辑另行处理。日语方案下空格通常开始转换，但当唯一候选是快捷模式的原文 Fallback（候选 `source` 为 9，即 `CandidateSource::Fallback`）时，空格直接交给 Engine 上屏这条原文，与 Windows 一致。

V 模式（算式、数字大小写与日期）、/ 指令和 @ 名单是 Engine 的本地模式，宿主不另做判断：视图的 `spelling_symbols` 列出当前状态下 Engine 当作输入的非字母键（V 模式是数字和 `+ - * / . ( ) % ^`，Unicode 模式是数字，空闲时是开着的模式入口 `/`、`@`），列在里面的键一律交给 Engine。于是这两种模式里数字不选词，改用 Shift + 数字选词；Shift + 数字打出的字符本身也在列表里时（美式键盘 V 模式下的 `%`、`^`、`*`、`(`、`)`）仍是输入，其余照常选词。`-`、`.` 不再当翻页键，`-` 也不触发以词定字，`.` 不走智能标点，`)` 不跳过成对标点补上的右括号。原来只认 `local_mode == "unicode"` 的数字门槛改成读 `spelling_symbols`。这三个模式上屏的文本带 `commit_context.typing_statistics: false`，不计入输入统计（也就不计入成就）。

按键音、上屏音、按键旋律、背景音乐和成就音由 host-api 在输入法进程里播放，偏好在共享 `preferences.plugins`，全部默认关闭。宿主只在三处调用：`handleEvent:client:` 对每个非自动重复的按下调 `msime_client_key_sound`（空格、回车与小键盘回车、退格各有键类，其余为默认），不论这个键最后是否被输入法处理；英文模式下不出声，与 Windows、Linux 一致；`apply:` 在有上屏时调 `msime_client_commit_sound`，语音上屏除外；`activateServer:` 让背景音乐可以播放，`deactivateServer:` 停止，偏好更新生效后再告知一次（开着的播放器才会记住激活状态）。多个控制器之间只有最后一个让音乐播放的控制器能停止它，因为 IMK 不保证上一个客户端的失活先于下一个的激活。`IsSecureEventInputEnabled()` 为真时（密码框、终端的安全键盘输入）不发任何声音、背景音乐暂停，这个状态是窗口服务器级的，别的应用没关掉它时本输入法同样静音。内置音效包随 bundle 放在 `Contents/Resources/sound-packs`，会话创建时作为 `sound_packs` 传给 host-api（`resources` 指向 Application Support 里的 EngineResources，旁边没有音效包）；运行时选项文件里已经写了 `sound_packs` 时以文件为准。`shortcut` 测试覆盖键类、自动重复与松键不发声、安全输入、V 模式各键的路由、统计排除、语音上屏不发上屏音以及音乐归属；`session-smoke` 用真实 host 库确认开关全关时三个调用都不排队。

## 词库准备与 Tauri 设置应用

先下载锁定词库，然后在隔离的开发状态目录中准备工作词库；该步骤要求相关会话已停止，不用于对现有输入法在线升级：

```sh
CFLAGS="-mmacosx-version-min=13.0" CXXFLAGS="-mmacosx-version-min=13.0" CMAKE_OSX_DEPLOYMENT_TARGET=13.0 CMAKE_PREFIX_PATH="$(brew --prefix)" cargo run -p msime-host-api --example prepare_host -- <已校验资源目录> target/macos-state
CFLAGS="-mmacosx-version-min=13.0" CXXFLAGS="-mmacosx-version-min=13.0" CMAKE_OSX_DEPLOYMENT_TARGET=13.0 CMAKE_PREFIX_PATH="$(brew --prefix)" cargo build -p msime-host-api --locked
cmake -S platforms/macos -B target/macos -DMSIME_OPTIONS_FILE="$PWD/target/macos-state/runtime-options.json"
cmake --build target/macos --parallel
ctest --test-dir target/macos --output-on-failure
```

打包 Tauri macOS 设置应用前，还要把同一份已校验资源暂存到应用 bundle：

```sh
platforms/macos/stage-resources.sh <已校验资源目录>
```

第三个可选参数是非英语离线释义目录（默认 `target/offline-glosses`，由 `scripts/fetch_offline_glosses.py` 按锁下载，见 [docs/third-party.md](../../docs/third-party.md#非英语离线释义resourcesoffline-glosseslockjson)）。其中的 `zh-<语言>.db` 与 NOTICE 暂存到 `target/macos/offline-glosses`，即资源目录的同级目录，宿主在翻译目标包含该语言时读取；没有时只有英语走离线释义。`release-macos.yml` 打包前先运行这个脚本，`package-release.sh` 在有暂存的释义时把它们复制进设置应用的 `Contents/Resources/offline-glosses`，签名一并封入，并在 DMG 内的副本上检查 NOTICE 在场；没有时照常打包。

打包时如果 `cargo` 报某个过程宏 crate「can't find crate for `xxx_macros`」，看它前面一行的 dlopen 错误：`mis-aligned LINKEDIT string pool` 表示过程宏的 dylib 被产出成 dyld 拒绝加载的形状。成因是 `MACOSX_DEPLOYMENT_TARGET`：`tauri build` 会按 `bundle.macOS.minimumSystemVersion` 导出它，rustc 把它也用在宿主过程宏上，同一个 crate 不设该变量时产出的 dylib 能正常加载。cargo 不把这个变量算进指纹，坏掉的 dylib 会留在产物目录里被后续构建继续复用，所以换一个干净的 `CARGO_TARGET_DIR` 看起来也能「修好」——但那只是另起一整棵要从头编译、占几个 GB 的产物树，坏掉的 dylib 还留在原处，不要这么做。`package-release.sh` 因此先用不带该变量的 `cargo build --features tauri/custom-protocol` 编译设置应用，再用 `tauri bundle` 只做打包；已经坏掉的过程宏要删掉 `target/<profile>/deps` 里对应的 `.dylib` 让它重编。

`tauri.macos.conf.json` 会把 `target/macos/EngineResources` 嵌入为 `EngineResources`；不要直接把未校验的词库目录配置到 bundle。`stage-resources.sh` 还把仓库自带的六套辅助码表（`resources/helpcodes`，不在词库发布里）连同来源声明放进其中的 `helpcodes/`，Engine 从资源目录下的这个子目录读辅助码表；缺了它们，Shift 字母仍被当作辅助码却筛不掉任何候选。`package-release.sh` 检查打出的应用里有这几个文件。资源目录缺少 `others.db` 或 `dict_japanese.dat` 时，宿主会安全关闭对应的 Emoji、颜文字或临时日语触发键，而不会吞掉普通大写字母。

Tauri macOS 设置宿主首次启动时，如果应用数据目录中没有 `runtime-options.json`，会从 bundle 内的 `EngineResources` 调用共享 Host API 准备默认用户词库、缓存和配置，并以同目录原子发布配置；已有配置不会被覆盖。`MSIME_CLIENT_HOST_OPTIONS`（兼容 `MSIME_IBUS_OPTIONS`）显式指定配置时不会触发自动准备，`MSIME_CLIENT_STATE_DIR` 仍可指定偏好与用户状态根目录。资源校验或准备失败会以通用错误终止本次设置宿主启动，不泄露路径、输入或 Host API 诊断内容。

升级应用之后不需要重新准备状态。Windows 安装程序在每次升级时回放用户词库，macOS 没有按用户执行的安装步骤（Sparkle 升级只替换整个 app bundle，连同其中的 `EngineResources`，不会触碰用户数据目录里的 `runtime-options.json` 与工作词库），所以改由宿主在启动时完成：IMK 宿主在 `input_method_main.mm` 建立 `IMKServer` 与独立偏好窗口之前、设置应用在 `macos_launch.rs` 读取配置之前，都对 `runtime-options.json` 调用共享的 `msime_client_refresh_host`（`RuntimeOptionsRefresh.h` / `msime_host_api::refresh_host_options`）。它用编译进去的词库锁算出当前代次，与 `dictionaries` 指向的代次目录比较；不一致时校验新词库、由 Engine 复制出新代次的工作词库并把用户词库日志回放进去，再原子替换这份文件，只改 `resources` 和 `dictionaries` 两项，provider socket 等其他键原样保留；已是当前代次时只读一次文件。默认应用数据目录与 IMK 读取的定位文件是同一份 `~/Library/Application Support/app.msime.macos/runtime-options.json`，谁先启动谁完成刷新，另一个读到的已是新代次。旧代次目录不会被改动，还在用它的宿主重启前照常工作；准备失败时文件保持原样，输入法继续用旧词库，诊断只写一行不含路径的通用日志，下次启动再试。符号链接、签名 bundle 内嵌的开发配置以及路径不符合 `prepare_host` 布局的文件都不会被改写，文件尚不存在（尚未配置）时 IMK 不做刷新。首次启动升级后的版本要同步完成一次词库复制与回放，会多花几秒。`~/Library/Input Methods` 里的 IMK 由设置页的「安装 / 更新」复制，可能落后于设置应用：旧 IMK 编译的是旧词库锁，看到设置应用刷新出的新代次会按自己的锁去准备，校验新资源失败后保留文件不动，因此在重新安装 IMK 之前每次启动都会多一次失败的校验，但不影响输入。

产物为 `target/macos/水杉输入法.app`。可选开发配置包含本机绝对路径，不得对外分发。未嵌入开发配置时从客户端的应用数据目录读取 `runtime-options.json`；配置缺失时不拦截输入。Tauri macOS bundle 会把同一 IMK bundle 随应用资源打包。设置应用每次启动都在后台比较随附与已安装 bundle 的 `CFBundleShortVersionString`（按数字逐段比较）和 `CFBundleVersion`：未安装时安装，随附的更新时刷新，已安装的相同或更新（例如先装了新版 DMG 又打开旧版 DMG 里的设置应用，或用 `install.sh` 装了更新的构建；带 `SUFeedURL` 的构建还可能经 Sparkle 更新过）时不动，从不降级。只有打包好的 `.app` 做这一步，只看它自身 `Contents/Resources` 里的 bundle：`tauri dev`、`cargo run` 与 `target/<profile>` 下的二进制不是打包应用，它们的资源目录是 cargo 产物目录，tauri-build 在那里放了一份 Sparkle 符号链接被展开的开发构建，这些运行一律跳过，不会拿它或 `target/macos` 的构建去覆盖开发者已安装的输入法；设置了 `MSIME_CLIENT_HOST_OPTIONS` / `MSIME_IBUS_OPTIONS` 的运行和输入法拉起的面板进程也跳过。启动检查、「安装 / 更新」与卸载在进程内互斥，staging 与备份目录不会被两次操作同时改写。检查结果连同输入源是否已在系统设置的输入法列表里（经 `defaults export` 读 `com.apple.inputsources` 的 `AppleEnabledThirdPartyInputSources` 与 `com.apple.HIToolbox` 的 `AppleEnabledInputSources`，任一列表有它即算已启用：新版 macOS 只把第三方输入法记在前者）交给设置页：刚安装或刷新过会提示版本，首次安装要等下次登录时提示注销并重新登录，未启用时提示到「系统设置 > 键盘 > 文字输入 > 输入法」点「编辑…」添加并给出打开该页的按钮，失败时指向手动按钮。安装与刷新和设置页的“安装 / 更新”走同一条路径：先在 staging 目录完成校验和原子替换到 `~/Library/Input Methods/水杉输入法.app`，再直接启动 bundle 的 `--register-input-source`，失败不会删除旧安装，也不会替用户切换当前输入源；没有旧安装时新 bundle 留在原处等下次登录（见「发布包」一节）。当前 bundle 装好（或已是最新）之后，同目录下旧名字的副本会被删除：`水杉输入法（预览）.app`，以及 `.pkg` 时代装进 `~/Library/Input Methods` 的 `MetasequoiaIME.app`（只在其 Info.plist 带本产品的 `app.msime.inputmethod.MetasequoiaIME` 或改名前的 `com.houko.inputmethod.MetasequoiaIME` 时删除），否则一个输入源会对应两个 bundle。“安装 / 更新”保留为不比较版本的强制重装。比较依赖 `Info.plist.in` 里的 `CFBundleShortVersionString` 随每次发布递增：`CFBundleVersion` 是提交数，没有 git 的源码树构建出来是 1，只在同一发布版本内排序。静态库与宿主均以 macOS 13 为最低构建目标，必须使用同一架构。

`prepare_host` 生成的配置包含 `preferences_directory`。宿主激活时立即后台读取此目录，之后每秒检查一次，前一次未完成时不重叠读取；失活后停止定时器。文件锁和读取不占用会话主线程，应用仍在主线程且有组合时延迟；读取错误保留原配置；如果 `preferences.json` 不是合法 JSON，输入法进程在每个目录上自动修复一次：原文件备份为同目录的 `preferences.json.corrupt-<UTC 时间戳>`，仍可解析的字段（含服务凭据）保留，诊断日志写 `preferences_recovered backup=<name>`。合法 JSON 但含未知字段或未来格式时不自动改写，设置页的「修复配置文件…」可以显式修复并在 Finder 中显示备份。旧配置没有此字段时不自动重读，需要重新准备开发配置（先停止该开发宿主）。

从仓库根目录让设置页写入同一份隔离配置：`MSIME_CLIENT_STATE_DIR="$PWD/target/macos-state" pnpm --filter @msime/desktop tauri dev`。保存后活跃宿主通常在下一次轮询收到快照，当前组词结束后生效；无需 Tauri 常驻。

设置页的词库管理与来源一致：快捷短语编码只能包含英文字母（1 到 32 个，导入时先转小写），新增、编辑和导入里带数字的编码会被拒绝，文本导入不收这一行，计入失败数并列在错误明细里。已经存有数字编码的条目仍出现在列表里、照常导出和同步，但 K 模式打不出来（引擎在 K 模式只接受字母）；可以删除它，或改成纯字母编码后保存，重新导入导出文件时这些行计入失败数。

## 词库维护

词库增改删、导入和清除学习记录需要独占维护锁，而每个打开的 IMK 会话都持有共享锁（IMK 为每个输入客户端各建一个控制器，各持一个会话）。设置窗口拿不到锁时，与 Linux 宿主同一套做法：在用户数据目录写入带过期时间的 `.msime-dictionary-quiesce` 租约（最长 30 秒，格式与读取见 `platforms/common/DictionaryQuiesceLease.h`），随即发出 `MSIMEDictionaryMaintenanceWillBeginNotification` 分布式通知（`DeliverImmediately`，后台输入法进程也立即收到），在约 2.5 秒内重试，仍取不到锁才返回 busy。超过单次请求上限的导入分批发送时，租约与通知只在第一次遇到 busy 时各发一次，租约在各批之间保持并在每批前续期，整次操作结束后立即删除。

输入法收到通知后，只有租约确实存在时才让出：取消语音与在途的云候选和翻译，把当前组合上屏（上屏失败则清掉预编辑），关闭所有控制器的会话。没有租约的通知什么也不丢。租约存在期间按键直接交给应用、不打开新会话；租约删除后的下一次按键重新打开会话，并恢复让出前的专用英文模式。通知丢失时，每秒一次的偏好定时器发现租约后同样让出；设置进程中途退出时，租约过期后输入最多停 30 秒。

输入法内的原生词库窗口在同一进程里走同一条路：自己写租约、在主线程同步发出本地通知、在 2.5 秒内重试后删除租约。`dictionary-quiesce-controller` 测试用两个持有真实会话的控制器覆盖让出、租约期间不重开、租约删除后恢复专用英文，以及过期租约不再挡住输入。

## 数据目录迁移

发行设置页的“关于 → 数据目录”可以把词库、学习记录、偏好、统计、剪贴板历史、皮肤和缓存移动到另一块磁盘。设置 bundle 与 IMK bundle 各自在固定 Application Support 目录保留一个小型 `runtime-options.json` 定位器，二者指向同一个可移动状态根；bundle 内只读资源不搬。迁移会先停掉 IMK 进程，只接受真实的空目录，在目标卷完成复制和按最终路径重新准备后原子切换两个定位器，最后才清理旧目录。失败保持旧目录有效；无 `.metasequoiaime-data` 所有权标记的非默认源目录不会自动删除。成功后设置窗口关闭、设置进程退出（macOS 设置窗关闭即退出，不像 Linux / Windows 那样驻留），重新打开会启动新进程并从新目录继续。

## 文本适配与维护快捷键

文本适配测试验证提交、ASCII 光标和清除预编辑。

维护快捷键只在水杉输入法当前 IMK 上下文中处理，不安装全局键盘监听。`Control+Shift+Option+1–8` 删除当前候选页对应槽位，`Control+Shift+Option+C` 通过共享 Host API 清除当前会话 Engine 缓存，`Control+Shift+Option+R` 用独立 helper 实例重新注册当前输入源并退出旧进程，`Control+Shift+Option+T` 退出当前输入法进程。物理键位不受当前键盘布局字符影响，Command 明确排除，Caps Lock 可共存，重复事件只消费一次。macOS 设置页相应使用 Option 与输入上下文文案；桌面重新注册命令按输入法 bundle identifier 定位目标，而不是重新打开设置应用。

## 快照激活测试

完整词库快照激活集成测试需显式运行，因为它要一份已校验的词库目录，不自动下载资源：

```sh
cmake --build target/macos --target snapshot-activation-test
target/macos/snapshot-activation-test /absolute/path/to/verified-desktop-resources
```

资源必须匹配仓库根目录的 `resources/desktop-dictionary.lock.json`，测试通过生产校验器检查文件。它在新建临时目录中准备用户数据和暂存快照，验证真实激活、英文模式开启/关闭的保留、导入词条候选与提交；成功后删除测试状态，不修改传入的资源目录或实际用户词库。

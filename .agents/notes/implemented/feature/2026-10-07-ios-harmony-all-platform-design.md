# Agent Note: iOS 与鸿蒙宿主采用全平台设计稿

Status: implemented

## Problem

全平台设计稿（`全平台 UI.dc.html`）给六个平台定了同一套外观：跟随季节的应用主题、重新编排的设置首页与四个标签页（设置 / 社区 / 统计 / 我的）、新的引导流程，以及键盘的工具栏、品牌键打开的分页功能面板、123 / #+= 符号层、九键、字母角标与下滑输入、单手模式、隐私模式等。Android 已经先按它改完，iOS 和鸿蒙还停在旧样子。设计稿是原型，其中有演示数据、做不到的平台能力和需要改跨平台契约的东西，不能照抄；而这三个移动宿主读的是同一份共享偏好和同一个 Rust 层，iOS 和鸿蒙各自偏离 Android 就会让同一个账号在不同设备上表现不一。

## Decision

- **以 Android 为先例。** 设计稿与 Android 已落地的做法不一致时，跟 Android 走，并在 PR 描述里列出偏差。具体跟随的有：
  - 季节色只从 Rust 取：两个宿主都调用 `msime_client_resolve_app_theme({app_theme, month, dark})` 和 `msime_client_app_theme_catalog()`，返回 `{id, season, accent, accent_soft, on_accent, background, card, hair}`，主题 id 为 `siji` / `chunya` / `xiayin` / `qiushan` / `dongxue`，`siji` 按设备本地月份换季。种子色与月份规则不在宿主里留副本；设计稿在种子色之上的派生色（`system` 键盘的背景、字母键、功能键和品牌标颜色）逐位移植 Android 的 `AppThemePalette.java` 的 `color-mix`：iOS 在 `platforms/ios/SharedUI/core/AppThemePalette.swift`，鸿蒙在 `platforms/harmony/entry/src/main/ets/keyboard/skin/AppThemePalette.ts`。解析失败时一律退回经典绿色 token。
  - 主题选择只存本机、暂不随设置同步（Android 经 `android_local` 同步，见「代价」）：iOS 存 App Group 的 `general.app_theme`，鸿蒙存设置页与键盘共用的状态目录里的 `app-theme.json`（`AppThemeStore`），这样键盘扩展和设置 App 看到的是同一个季节。鸿蒙 NAPI 新增 `ResolveAppTheme` 与主题目录两个入口。
  - 暗色下强调色上的文字用 Rust 的 `mix(accent 25%, #000)`，不用设计稿的白字。
  - 功能面板按 Android 的工具顺序排（鸿蒙为原型的 14 项加鸿蒙自有项），隐私模式移植自 Android 的 `ImePrivacyGate`。
  - 统计页去掉「常用词」「最常打错」（隐私原因，同 Android）；徽章「动口不动手」「换装达人」在 Android 记录的同一批位置调用 `record_voice` / `record_skin`。
  - 中文模式下 Shift 仍切到英文，不采用设计稿的「单次大写」，Android 也没有采用；iOS 只换了 ⇧ / ⌫ 的图标和 Android `styleShiftKey` 的配色。
  - 再点一次「设置」标签回到它的根页，同 Android 的标签栏。
- **刻意偏离设计稿的地方：**
  - 设置首页保留「键盘」一行（两个宿主都是）。设计稿的移动端首页去掉了它，但「键盘」页现在承载候选栏、键盘工具栏、AI 润色与回复、按键反馈，Android 也保留了。鸿蒙首页另外保留候选栏、外接键盘快捷键、剪贴板、AI 辅助这几个真实页面。
  - 工具栏默认值不变：共享的 `touch_toolbar` 默认仍是布局、表情、皮肤开，剪贴板、AI 等关。iOS 新增的「常用语」「输入方式」开关和「显示方式」（输入时显示 / 隐藏）存在 App Group 的 `TouchToolbarLocalPreference`，对应 Android 的 `platform.android.toolbar_*`，默认开；所以 iOS 默认栏与设计稿的「表情 / 常用语 / 剪贴板 / 皮肤 / 输入方式」不同，剪贴板不在默认栏里。鸿蒙不在 `host_surface.rs` 为自己打开 `touch_toolbar` 的显示开关，因为那会让剪贴板默认被隐藏。
  - 按键振动默认仍为关。设计稿画的是开，改默认值是产品决定，不在这次里做。
  - 季节主题只作为应用的强调色，以及「跟随系统」的 `system` 键盘皮肤和品牌标的配色；不进键盘皮肤目录。皮肤页和键盘内皮肤面板没有设计稿里的「水杉四季 / 春芽 / 夏荫 / 秋杉 / 冬雪」皮肤卡：client-core 的 `GlobalTheme` 目录只有 `system` / `shuishan` / `light` / `paper` / `night` / `ink` / `custom`，加卡片是跨平台的 Rust 改动。
  - 键盘高度的范围受 `touch_keyboard_height_adjustment` 限制。这个字段是经过校验、随设置同步的跨平台字段（`crates/client-core/src/preferences.rs` 只接受 `-12..=48`），所以界面显示百分比、仍按点 / vp 存储，可调范围约为 93%–129%，不是设计稿的 75%–130%。
  - 鸿蒙键盘条上的「译」「🎙」「💬」三个按钮移进功能面板，代价是多点一次；鸿蒙键盘不再按 `touch_voice_shortcut` 在条上放语音键，设置页里对应的开关在鸿蒙上隐藏。
  - 鸿蒙 2in1（`hm2`）只跟随强调色系，背景不变；侧栏顺序不改，因为 `settingsNavGroups` 是所有桌面共用的。
- **隐私模式。** 两个宿主的功能面板都有「隐私模式」开关，存本机。
  - 鸿蒙（`PrivacyGate.ts`）：开启时不记输入统计、按键热力图、语音时长，不存剪贴板历史，不读写云剪贴板；本次会话关闭引擎学习，并经 `msime_client_set_private_session` 把会话标为私密。为此 `crates/host-api/src/lib.rs` 的 `PRIVATE_SESSIONS_SKIP_STATISTICS` 从 Android 扩到 `target_env = "ohos"` 与 `target_os = "ios"`，鸿蒙的私密会话也不记选词位置（鸿蒙本来就不计上屏效率）。
  - iOS（`KeyboardPrivacyGate`，开关存 App Group 的 `keyboard.privacy.incognito`）：与 Android 的 `ImePrivacyGate` 对齐，开启时关闭本次会话的学习，不记输入统计、按键次数、皮肤与语音时长，不存剪贴板历史，不读写云剪贴板，不写诊断日志；并经 `msime_client_set_private_session` 把会话标为私密（会话重建后由 `MetasequoiaInputSessionBridge` 重新应用），host-api 因此也不记选词位置。凭据输入框（密码、新密码、一次性验证码）同样不记上述统计、剪贴板与诊断日志，但不关学习、不标私密会话，与 Android 的密码框一致；iOS 没有 Android 那种不允许学习的输入框标记。
- **鸿蒙设置页仍是共享的 `@msime/ui` 网页。** 视觉改动都挂在 `harmony:` / `hm2:` 变体和 `[data-platform="harmony"]` / `hm2` 的 token 上，Windows、macOS、Linux 的桌面布局不变；跨包的类型契约固定在 `packages/ui/src/core/host-contracts.ts`。移动端页面改名（表达、皮肤等）不加平台门槛，也作用于 Tauri 手机构建，那些不是产品。

## 不做的部分

**设计稿里的演示数据，不照抄：** 林杉 / lin.shan@icloud.com 等假账号与假数字、`metaseq.app`（真实域名是 `msime.app`）、剪贴板的「端到端加密」（服务端经 HTTPS 存明文）、只定义没画出来的日 / 周 / 月统计切换、无确认的「重置所有设置」、点状态卡就标为完成。

**缺后端或依赖：** Google / Apple / 华为登录与 magic link（鸿蒙）、账号关联、头像上传、云端数据与导出我的数据、我的设备（`/sessions`）、下载页「发送链接」、反馈截图、MCP 开发者日志、上传语音以改进识别、自动云同步开关与「已同步 · N 分钟前」（iOS 只有手动上传下载，鸿蒙没有同步调度）。

**缺键盘或引擎能力：** 候选栏高度与翻页按钮、辅助码模式、郑码、搜狗双拼、五笔 06、粤语仓颉 / 速成、设计稿的 20 种翻译目标语言、手写的书写模式与识别后显示拼音、离线识别兜底、命名词库包（`msime_client_dictionary_collections`，单独成 PR）、表达页「发现短语」与社区短语包、App 内的常用语编辑器（鸿蒙；iOS 的「常用语」由键盘面板管理）、鸿蒙的长按空格语音、鸿蒙手机的开发者选项、iOS 的「单次最长」（Rust 只在 Android 上记连续输入，显示 `—`）。

**iOS 平台做不到：** 检测键盘是否已添加、是否给了完全访问，所以状态卡和引导第一步没有 ✓ / ! 标记（读 AppleKeyboards 默认值是未公开做法、有审核风险，键盘自己写的标记在权限被收回后会过期）；外接键盘快捷键页和翻页键（扩展收不到硬件按键）；键盘内实时语音识别（扩展不能用麦克风，长按空格打开的是语音面板）；从扩展里画状态栏和宿主的「显示键盘」胶囊；自更新渠道（App Store 负责）；插件页；Fluent 图标（用 SF Symbols 和生成的路径）。

**留给后续：** iOS 17/18 上自绘的浮动标签栏胶囊（保留原生 `TabView`，iOS 26 走 Liquid Glass）、跟随季节的启动屏、iOS 键盘内皮肤面板的新缩略图（仍是 `KeyboardSkinMiniature`）、鸿蒙 `hm2` 的插件卡片网格与账号内联登录。

## Alternatives considered

- **首页完全照设计稿，去掉「键盘」行** — 视觉与设计稿一致，首页更短；但候选栏、键盘工具栏、AI 润色与回复、按键反馈要另找入口，UI 测试路径要跟着改，且和 Android 不一致。
- **工具栏默认值改成设计稿的五项** — 新用户开箱就是设计稿的样子；但默认值在共享的 `touch_toolbar` 里，改它会改变老用户已有的工具栏和其他宿主。鸿蒙打开 `touch_toolbar` 显示开关则会默认隐藏剪贴板，与设计稿相反。
- **把季节主题做成皮肤卡** — 设计稿的皮肤页就是这样画的；但要在 client-core 的 `GlobalTheme` 目录里加五项，是跨平台 Rust 改动，所有宿主的皮肤选择器都会跟着变，不该夹在两个宿主的改版里。
- **键盘高度放开到 75%–130%** — 设计稿的范围，矮键盘可选；但 `touch_keyboard_height_adjustment` 有校验并随设置同步，放宽要改 Rust 契约，其他宿主读到范围外的值会被拒。
- **暗色下强调色上用白字** — 设计稿如此；但在暗色强调色上对比度只有约 1.7–2:1，Rust 已给出 `mix(accent 25%, #000)`。
- **按键振动默认开** — 与设计稿一致；但这是改变老用户手感的产品决定，留给产品拍板。
- **iOS 用 AppleKeyboards 默认值判断键盘是否已添加** — 能画出设计稿的 ✓ / ! 状态；但这是未公开做法，有 App Review 风险，结果也不可靠。
- **鸿蒙改版后不再被调用的键盘静态成员加进 `ALLOWED` 而不是删除** — 保留以后可能用到的代码；但 `ALLOWED` 用来记平台差异，这些是被设计拿掉的功能，所以按未接线符号检查的规则删掉，键距行距仍在设置页可调。

## Consequences

- **收益**：三个移动宿主的季节色同出一源，换季规则只在 Rust 里改一处；设计稿与 Android 的分歧在 iOS 和鸿蒙上按同一方向解决，没有引入新的共享偏好字段，唯一的 Rust 改动是 `PRIVATE_SESSIONS_SKIP_STATISTICS` 的平台条件。
- **代价**：iOS 没有 Android 的不学习输入框标记，这类输入框在 iOS 上照常学习和统计。iOS 与鸿蒙的主题选择不随账号同步，换设备要重选；iOS 的设置同步还不带应用主题、单手模式、工具栏的「常用语」「输入方式」「隐藏」开关和手写设置，而 Android 经 client-core 的 `android_local` 同步这些。鸿蒙条上的翻译、语音、回复多一步。`guides/harmony.md` 描述的功能面板与实现的原型顺序不同，以实现为准，指南待更新。

## Verification

- **iOS**（iPhone 18 Pro Max 与 iPad Air 11 英寸模拟器，iOS 27）：`MSIMEApp` 构建通过；完整 `MSIMEClientTests` 通过（`MSIMEKeyboardTests` 559 个、`MSIMEServiceTests` 70 个、`MSIMESharedTests` 67 个，0 失败）；`OnboardingUITests` 38 个在 ad-hoc 签名（`CODE_SIGN_IDENTITY=-`）下全过，未签名构建没有 App Group 和钥匙串，相关用例会假失败；`platforms/ios/tests/settings/ProjectConfigurationTests.py`、`scripts/test-support-channels.py` 等 Python 门禁、`generate_keyboard_icons.py --check` 与 `swift test --package-path shared/backend` 通过。设置各页、社区、统计、我的、引导和键盘都在浅色与深色下截图，与设计稿渲染图逐区对照（十月，即秋季配色）。iOS 26 以下的界面分支在这台机器上看不到，真机手感未验收。
- **鸿蒙**：`pnpm --filter @msime/desktop typecheck`、`pnpm --filter @msime/harmony typecheck`、完整 vitest（515 个文件、2540 个用例）、`pnpm lint`、`pnpm format:check`、`bash platforms/harmony/tests/run.sh`、`scripts/test-harmony-arkts-subset.py`、`scripts/test-harmony-settings-bundle.py`、`bash scripts/run-checks.sh` 全部通过；完整 HAP 链路（`install_resources` 到 `hvigorw assembleHap --no-daemon`）构建成功。桌面不受影响的检查：在 develop 与本分支上用 jsdom 渲染 `SettingsPage`，覆盖 windows / macos / linux × 22 页 × 2 种宿主能力，共 132 次，去掉平台变体的类名后 DOM 一致。没有鸿蒙模拟器和签名材料，HAP 未签名、未上机，所以 `Shape.viewPort` 描边缩放、IME 面板里的 Swiper、`stateStyles` 与 `onClick` 同用、高度条拖动、`WebDarkMode.Auto` 等只经编译和逻辑测试，没有在屏幕上确认。
- **host-api**：只改了 `PRIVATE_SESSIONS_SKIP_STATISTICS` 的 `cfg`（加上 `target_env = "ohos"` 与 `target_os = "ios"`），`test` 配置原本就打开它，计法的单元测试不变。

## 相关笔记

写这篇前检索了 `.agents/notes/` 的全部活跃笔记：[滑行输入引擎](2026-10-07-glide-typing-engine.md) 和 [iOS 键盘的滑行输入宿主](2026-10-07-ios-glide-typing-host.md) 与本次改版无关（滑行开关仍在「键盘」页的「手势」一节，鸿蒙设置页保存按键反馈时会保留滑行输入的值），两篇 process 笔记也无关，没有需要合并或归档的。

[iOS 账号登录的弹窗与 task 挂在 List/Form 上](../bug-fix/2026-10-08-ios-account-login-presentation.md) 被本次改版部分取代：「我的」页和登录面板不再是 List/Form，那篇的 `AppleAccountModel` 与 `appleAccountPresentation` 随之移除；它定下的「List/Form 里的 `Section` 上不挂 `.sheet`、`.alert`、`.task`」仍然成立，`AppleAccountSection` 的 `.task` 因此挂在登录选项那一行上，切后台保留邮箱输入的 UI 测试也改成走登录面板。

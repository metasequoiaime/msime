# Agent Note: 鸿蒙手机和平板的插件页

Status: implemented

## Problem

鸿蒙的设置是共享 React 设置页。2in1 上它有完整的插件页（导入、编辑器、社区插件），手机和平板上整页藏起来（`hasPlugins` 要求 `!mobilePlatform`，`mobileHiddenPageIds` 里有 `plugins`），设置桥的 `SettingsFormFactorCapabilities` 也只在 2in1 声明 `plugin_triggers`、`wordbook_packs`、`symbol_set_packs`。可手机键盘早就读得到插件：设置应用把 `plugins/` 镜像到共享沙箱（[共享沙箱镜像](../architecture/2026-10-09-harmony-ime-shared-sandbox.md)，不分形态），host-api 按 `local_modes.quick_phrase` / `command` 从键盘自己的 `<state_root>/plugins` 读启用的短语表和指令表，触屏按键音也按 `plugins.key_sound.pack` 播（[触屏音效包选择](2026-10-25-harmony-touch-key-sound-pack-picker.md)）。缺的是手机上装包、选包的入口，以及键盘这边两处按形态关掉的读法：符号面板不读符号集插件，功能面板「本地输入」没有进入 / 指令的格子。iOS 和 Android 同时按同一份规格做原生插件页，三端要长得一样、做的事一样。

## Decision

设置桥新报一项 TypeScript 能力 `HostCapabilities.touch_plugin_packs`，`SettingsFormFactorCapabilities.touchPluginPacks` 在手机和平板上为真、2in1 上为假。页面不看平台名，只看它：

- 有这项能力且有插件客户端时，触屏设置给出插件页（`hasPlugins`），触屏列表也不再藏它（`mobileHiddenPages`）。没有这项能力的触屏宿主照旧没有插件页；桌面的判断原样不动。
- 「我的插件」只列触屏键盘用得上的类型（`touchPluginKinds`）：音效包（只列 `keys` 模式）、指令表和短语表（要 `plugin_triggers`）、辅助码表（要设置里有辅助码）、单词本、符号集（要 `wordbook_packs` / `symbol_set_packs`）。特效包、音乐包、按键旋律和脚本插件不列，列表末尾一句「另有 N 个已安装的插件（类型）在这台设备上用不到」。「无法载入的插件」也只报这几类。
- 音效包按触屏规则选用：「设为按键音效」只写 `plugins.key_sound.pack`，不动 2in1 的 `key_sound.enabled` 和模式；触屏的「按键音」开关是宿主本机 `key-feedback.json` 的 `sound`，页面经 `mobileKeyboardFeedback` 读到后交给插件页（`TouchPluginRules`），开关关着时选包会一起打开它，和桌面「设为当前音效包会同时打开按键音」同一个意思。列表标记按这个开关给「使用中」或「已选 · 按键音未开」；旋律选择不报缺失。
- 短语表、指令表详情里 K、/ 模式关着的提醒按触屏键盘的入口写（「在键盘功能面板的「本地输入」里选…」），指令表详情的「打开 / 指令」照旧就地打开 `local_modes.command`。
- 导入只给「导入 .zip」。`plugin_import` 在没有 `FolderSelection` 系统能力时拒绝文件夹，而文件夹选择按华为文档只有 2in1 提供。
- 新建和编辑沿用共享编辑器，宽度在 390 px 的手机布局下照常可用。
- 子页（详情、编辑器、缺失项）在触屏上各推一条历史记录（`usePluginNavigation` 的 `mobile`），系统返回键退一层；编辑保存后退回原来那层详情，新建保存后详情顶替编辑器那层。社区插件详情同理（`communityDetail.kind = "plugin"`）。
- 社区插件页只给这几类的筛选按钮，「全部」里服务端给的其他类型在页面上滤掉，滤掉的条数和宿主滤掉的新类型一样计入下一页的偏移量（`skipped`），「加载更多」不会重读或跳过。发布不改：桥里的 `community_plugin` 本来就不分形态。

能力上的三处翻转：`pluginTriggers`、`wordbookPacks`、`symbolSetPacks` 每种形态都为真。`plugin_triggers` 是设置页给出 / 指令开关、插件页列指令表和短语表的前提；它同时让输入页出现 V、@ 的开关和插件页的「@ 名单」。这两项在手机上并非摆设：实体键盘接上手机时，硬件路由本来就不分形态地把 V、/、@ 交给 Engine，只是此前在手机上没有开关能打开它们。

键盘一侧两处：

- `KeyboardView.pluginSymbolGroups` 不再按 `isDesktop()` 返回空表，手机的符号页和颜文字页也排上符号集插件的组，读的仍是镜像过来的 `plugins/`。
- 功能面板「本地输入」改列 `LocalInputMode.touchModes(...)`：设置打开了 / 指令（`local_modes.command`，缺省关）且当前出中文标点（`KeyboardSession.offersCommandMode`）时，在「拆字」之后加一格「指令」。点它按普通标点键送 `/`（`entersWithShift` 只对字母触发词为真），Engine 在没有组字时进入指令模式；英文标点下 Engine 不认 `/` 为入口，列出来只会打出一个 /，所以不列。九键也给：进入任何本地模式后键盘画全键字母面（`KeyboardScheme.usesNineKeyFace` 在本地模式下为假），指令字母照常能打。V 和 @ 仍不进触屏面板，触屏没有数字运算和名单的键面。

短语表在 K 模式里生效不需要改动：host-api 的 `PluginTables` 从 `PluginRoots.installed`（键盘的 `state_root/plugins`）读，镜像条目 `plugins` 的 `fullModeOnly` 为假，手机上也镜像。

## Alternatives considered

- **用平台名判断（`harmony && mobile`）打开插件页。** 改动最少。但设置页其他地方都按能力收放，而且安卓和 iOS 的设置不是这份页面，平台名换不来任何东西；能力还说清了页面在触屏上换了哪几条规则，将来别的触屏宿主要接时照着报就行。
- **把触屏插件页做成独立组件。** 手机的规则差别不大（列哪些类型、音效包怎么选、导入按钮、历史记录），复制一份列表、详情和编辑器只会让两份漂移；在共享组件上加一个 `touch` 规则对象，桌面不传就完全不变。
- **不翻 `plugin_triggers`，另起一项只管指令表。** 能让手机不出现 V、@ 开关和 @ 名单，和 iOS、Android 的插件页更像。但 `plugin_triggers` 的含义就是宿主路由这三种模式，手机接实体键盘时确实路由，另起一项等于在手机上留着打不开的行为；而 / 指令开关、指令表、短语表都挂在它上面，拆开要动输入页和插件页好几处判断。
- **在手机上列出全部类型，只把用不上的标成「这台设备不…」。** 2in1 就是这样做的，详情里早有这类说明。但手机屏幕上一屏看不完，特效包、音乐包这些点进去全是「不支持」，规格也要求三端只列生效的类型。

## Consequences

鸿蒙手机和平板能导入 .zip、新建和编辑四种文本插件、启用短语表和指令表、选按键音效包、选辅助码表、去背单词，并在社区里浏览、安装和发布这六类插件；符号集在手机键盘的符号面板里出现，/ 指令能从「本地输入」进入。2in1 和桌面的插件页不变。默认值都没动：/ 指令、@ 模式、V 模式出厂仍是关，按键音仍是关。

代价和遗留：

- 手机输入页会多出 V、/、@ 三个开关，插件页多出「@ 名单」；iOS、Android 的插件页没有名单。在手机触屏上 V 和 @ 只能靠实体键盘进入。
- 社区列表的条目不带音效包的 `mode`，从社区装来的按键旋律在手机上只会出现在「用不到」那一句里。
- 子页的历史记录在离开插件页时不会被清掉，换页后再按返回可能先回到插件页的那一层，和其他触屏子页一样。
- 「指令」格子的位置排在「拆字」之后，iOS 的菜单若放在别处，两端要再对齐一次。

## Verification

- `apps/desktop/tests/settings/harmony-phone-plugins.test.tsx`：手机只列六类、末尾说明三个用不到的包、没有「声音与效果」；只有「导入 .zip」并以 `archive` 调用；没有能力的手机不显示插件页；2in1 照旧列全部类型和「导入文件夹」；手机上设为按键音效只写包并打开按键音；短语表和指令表的触屏提醒、「打开 / 指令」写入 `local_modes.command`；详情推一条历史记录、系统返回回到列表；社区筛选和列表只有这六类。
- `platforms/harmony/tests/run.sh`：形态能力（三项每种形态为真、`touchPluginPacks` 只在手机和平板）、`LocalInputMode.touchModes` 与 `entersWithShift`。
- 共享设置与社区的 vitest 全部通过，桌面和鸿蒙的类型检查、lint 通过。
- 编译机上 `ohpm install`、`hvigorw assembleHap --no-daemon` 成功（原生库和语音 HAR 借另一份检出暂存的），改动的 `.ets` 没有 ArkTS 告警。
- 手机布局用无头浏览器按 390 px 宽截了列表、音效包详情、新建、编辑和社区页。
- 审查时合并 `develop`：插件指南同时保留 Android、iOS 已新增的原生插件页和本分支的鸿蒙触屏能力，解决六处平台支持说明冲突。合并后手机插件设置 8 项测试、鸿蒙共享设置构建及键盘逻辑/音效渲染回归通过。本轮 quick 的 workspace、Android、wasm/clippy 和 Android Java 均通过；公共 Windows runner 契约待 #9986，系统盘剩余 33 GiB 时在新建 Linux 构建树之前停止，未声称完整 quick 通过。
- 没有覆盖：没有鸿蒙设备或模拟器；手机上的 .zip 选择器、键盘读到镜像的符号集和短语表、「指令」格子进入 / 模式都没有在设备上验证。

# Agent Note: macOS 与 HarmonyOS 输入法发现正在使用的皮肤包被同名替换

Status: implemented

## Problem

用户选着 `shuishan-fireflies`，00:26 经 MCP 的 `create_candidate_skin`（`replace=true`）换掉了这个包：新清单的装饰带是 `top_inset_dip = 73`、`width_dip = 132`，图是 264x152，旧包是 26 dip 的装饰带和 192x80 的图。00:03 启动的输入法到 08:18 还在画旧的小装饰，重启进程才好。MCP 服务的说明承诺改动「几秒内」在输入法里生效。

原因在 `platforms/macos/src/settings/AppearancePreferences.mm`：皮肤包只在 `-reloadSkins` 里重读（初始化、打开设置窗口、导入云快照、`SkinSettingsView`），其余路径走 `-resolveSelectedSkinIfChanged`，它比较的 `-themeResolutionKey` 只含选择、取色器、布局和皮肤根目录，注释明说不含「包文件本身」。同名替换不改选择，偏好文档的 `revision` 也不变，`InputController.mm` 的 `-completePreferenceLoad:` 每秒轮询读到同一 revision 就直接返回，于是 `-resolvedSkinForDark:`、`-decorationImageForDark:`、`-canvasAssetsForDark:` 和工具栏调色板一直是进程启动时解析的那份。

社区图库有同一类问题：`packages/ui/src/community/use-community-candidate-skin-gallery.ts` 的预览 LRU 只按发布 id 取，作者用 PUT 原地替换作品后 id 不变，已经显示过它的设置应用一直给出旧预览，直到重启。

## Decision

`MSIMEAppearancePreferences` 记两份文件身份，每个路径一次 `stat`，取设备号、inode、纳秒修改时间和大小：

- 解析前取浅色、深色槽位里选中的包（`custom_theme.candidate_skin` 与 `candidate_skin_dark`，去重）的目录和 `skin.toml`。安装、皮肤编辑器、MCP 和图库都经 `candidate_community::install`：整包写进 `.staging`，再 rename 换掉旧目录（`folder_import::replace_directory`），所以目录 inode 一定变；手工改 `skin.toml` 时它的修改时间变。在读包之前取，读的过程中包又被换掉时下一次检查会看到不同的身份，不会把新身份记在旧内容上。
- 解析后取两种明暗实际画的装饰图和背景图，覆盖原地改写图片、目录 inode 不变的情形。

新方法 `-reloadReplacedSkinPackages` 比较这两份身份，变了就 `-resolveSelectedSkin` 并返回 YES。它不发 `MSIMEAppearanceDidChangeNotification`：设置没有变，发了会让每个控制器取消云候选、重置释义状态、再把整份偏好写回文档。

`MSIMEInputController` 在两处调用它（`-applyReplacedSkinPackages`，换过时同时把新调色板交给悬浮工具栏）：

- `-renderCandidates` 里候选窗从隐藏到出现时，在读皮肤和装饰图之前。同一次输入里的后续按键不再检查。
- 每秒一次的偏好轮询（`_preferencesTimer`）里，紧跟 `-reloadPreferences`；候选窗正显示着时重画一次。显示期间发生的替换在一秒内画出来。

社区图库的预览加载器改成 `(id, revision)`，`revision` 是作品的 `updated_at`。服务端（msime-cloud `docs/skin-community.md`）在 PUT 替换时更新 `updated_at`，客户端的列表和详情请求都带 `fields=sync`，所以这个字段总在。缓存每个发布 id 只留一份，记着读它时的 `updated_at`，对不上就重新读并顶掉旧的一份；卡片和详情的 `CandidateSkinPreviewImage` 把 `skin.updated_at` 传进去，它变了也会重新发起读取。

## 检查的开销

一次检查在常见情形下（一个包给两种明暗用、带装饰图、没有背景图）是 3 次 `stat` 加拼一个短字符串。本机 Apple Silicon 上对 `shuishan-fireflies` 的实测是每次约 4 µs（20000 次取平均，`-O2`）；两个槽位各一个包、都带背景图时最多 8 次 `stat`，同一量级。它每秒跑一次，加上每次组字开始时一次，不列目录、不读文件、不解码图片；只有身份变了才重新解析和解码。

## 其他平台

逐个宿主读过加载与重载的触发点，共享层（`msime_client_resolve_theme`、`msime_client_skin_catalog`）每次调用都读盘、没有缓存，问题都在各宿主自己的缓存上：

- Windows 已经做了：服务器循环每 500 ms 用 `platforms/windows/src/candidate/SkinResourceRevision.h` 对两个槽位的包算文件指纹，变了就清掉主题和资源缓存。
- Android 每秒轮询时 `CandidateSkinResolver.read()` 都重新解析，约 1 秒内生效。
- iOS 的解析缓存键含 `skin.toml` 的修改时间和大小，但只在键盘下一次出现（`viewWillAppear`）时重算；键盘显示期间替换不生效。
- HarmonyOS 与 macOS 同样过时：`KeyboardSession.applyAppearance` 只在会话启动、自身保存之后和偏好 revision 变化时运行，`syncSharedState` 把新的 `skins` 目录镜像过来后也不重画。已按同一做法修复，见下一节。
- Linux 两个前端读发布在运行时选项文件里的皮肤目录。MCP 服务原先从不重新发布它，替换后 IBus 的配色和 Fcitx5 非 canvas 包的配色、尺寸都过时；现在 MCP 写皮肤后也重新发布，见 [Linux 上 MCP 写入皮肤后重新发布皮肤目录](2026-10-20-linux-mcp-skin-catalog-republish.md)。Fcitx5 的装饰图和 canvas 包按文件戳刷新。

这些都不是同一处共享代码，先修了 macOS 和共享的图库预览，HarmonyOS 随后按同一做法修复（下一节），Linux 的 MCP 路径另见上面链接的笔记，iOS 列为后续。

## HarmonyOS

键盘（`InputMethodExtensionAbility`）和设置应用各有一份 `files/state`，键盘这边的 `state/skins` 只有一个写入方：`SharedStateMirror` 把设置应用发布的 `skins` 整棵复制到旁边，再用 `StagedResources.replaceDirectory` 换入（见[鸿蒙共享沙箱](../architecture/2026-10-09-harmony-ime-shared-sandbox.md)）。导入发生在扩展 `onCreate`（会话启动前）和每次 `KeyboardSession.attach` 的 `syncSharedState` 里；键盘自己从不安装或改写皮肤包，图库、皮肤编辑器、文件夹导入都在设置应用一侧。

所以 HarmonyOS 只在 `attach` 一处检查，紧跟 `reloadChangedPreferences`：

- `applyAppearance` 开头取两个槽位（`custom_theme.candidate_skin` 与 `candidate_skin_dark`）选中的包目录和 `skin.toml` 的身份，读完装饰图、背景图之后取这两张图的身份（只在 2in1 上，触屏键盘不画它们）。路径规则和拼身份的逻辑在 `keyboard/candidate/CandidateSkinPackageIdentity.ts`，键盘侧用 `fs.statSync` 取 `ino`、`mtimeNs`（没有时用秒级 `mtime`）和 `size`；鸿蒙的 `fs.Stat` 没有设备号。
- `KeyboardSession.reloadReplacedSkinPackages()` 重新取这两份身份，变了就用当前的 `preparedPreferences` 再跑一次 `applyAppearance` 并 `refreshToolbar()`，返回 true 时 `attach` 调视图的 `onViewPreferencesReloaded` 重画。它不重建引擎会话，也不调扩展的 `onPreferencesReloaded`（那是给按键绑定和状态面板用的，设置没变）。修订号变了的情况由 `restartIdleSession` 里的 `start` 重新应用并记下新身份，这一步随后比较相同，不会重复读包。

镜像每次导入 `skins` 都换掉整棵目录，每个包目录都是新建的，所以只要设置应用发布过皮肤目录的任何变化，选中包的目录 inode 就会变，键盘重新应用一次外观；没有导入时只是 2 到 4 次 `stat`。不在候选窗出现时检查：键盘的皮肤目录在两次 `attach` 之间不会变，那里查只是白做 `stat`。

## Alternatives considered

- **把包文件的身份并进 `-themeResolutionKey`**：一处改动，所有已有的 `-resolveSelectedSkinIfChanged` 调用点自动获得检查。但 `-canvasAssetsForDark:` 在每次绘制里被调用不止一次，`-preferencesChanged` 在每次设置改动时也调用它，检查会从「每次组字开始一次」变成「每次绘制若干次」；而且仍然要另找一个不依赖偏好变化的调用点，单靠它解决不了同名替换。
- **照 Windows 对包里每个文件算 `mtime` 和大小**：能发现任何文件的原地改写，包括 canvas 素材和工具栏样式表。代价是每次检查要列目录、`stat` 包里所有文件；macOS 上所有正规写入都经暂存加 rename，目录 inode 已经能发现它们，没必要每秒遍历目录。
- **对皮肤目录挂 FSEvents 或 dispatch source**：零轮询、即时。但要处理目录本身被 rename 换掉之后监视的描述符指向旧目录、重新挂载、去抖和跨线程投递，复杂度远高于几次 `stat`，而输入法本来就有每秒一次的轮询可以搭车。
- **HarmonyOS 上看 `syncSharedState` 的导入结果，`imported` 含 `skins` 就重新应用**：零次额外 `stat`，信号准确，而且今天键盘的皮肤目录确实只有镜像这一个写入方。不用，是因为它把外观刷新绑在镜像条目名和「键盘不写皮肤」这个现状上：以后键盘侧多一个写入方（比如在键盘里切换或试用皮肤包），或者镜像改成按包增量复制，它就悄悄失效；文件身份不关心是谁、怎么写进来的，与 macOS、Windows 是同一个契约，代价只是每次获得焦点几次 `stat`。
- **让 MCP 的替换顺带改一下偏好文档的 revision**：输入法现有的轮询就会重新应用。但这只修 MCP 一条路径（皮肤编辑器、文件夹导入、手工改包仍然过时），还让一个只写皮肤目录的工具去写偏好文档，跨了它的写入边界。
- **图库重新打开或刷新时清空预览缓存**：不依赖服务端字段。但列表里的 `updated_at` 已经能精确说明哪一款换过，清空会让没换过的预览也重新下载。

## Consequences

- **收益**：经 MCP、皮肤编辑器、图库重装或文件夹导入替换正在用的皮肤后，下一次出现的候选窗就画新包，候选窗正显示时一秒内重画，不必重启输入法；悬浮工具栏同时换上新调色板。图库里作者替换过的作品在下一次列表或详情读取后显示新预览。
- **代价与已知上限**：只看目录、`skin.toml` 和装饰图、背景图。手工原地改写 canvas 素材或工具栏样式表而不动这几个文件时不会被发现，要等下一次 `-reloadSkins`（打开设置窗口等）。替换发生在两次轮询之间、候选窗又一直开着时，最多晚一秒。iOS 仍然过时，见「其他平台」。HarmonyOS 上替换要等键盘下一次获得焦点才生效：设置应用发布有 300 ms 合并，键盘在两次 `attach` 之间不导入；镜像的目录指纹按秒级修改时间和大小算，同一秒内大小不变的改写不会被发布，下一次变化时一起带过去。

## Verification

- `platforms/macos/tests/candidate/SkinPackageReplaceTest.mm`（ctest `skin-package-replace`）：按安装的暂存加 rename 方式整包替换，断言替换前解析结果仍是旧包（复现原问题），`-reloadReplacedSkinPackages` 之后装饰带高度、宽度和装饰图像素尺寸都是新包的；再分别原地改写装饰图、原地改写 `skin.toml`、删掉再装回，每次都读到新内容，什么都没变时返回 NO。
- `platforms/harmony/tests/candidate-skin-package-identity.test.ts`（`platforms/harmony/tests/run.sh` 里随其他测试一起跑）：在临时目录里按安装的暂存加 rename 替换包、按镜像的方式换掉整棵 `skins`、原地改写装饰图、原地改写 `skin.toml`、删掉再装回，每次身份都要变，什么都没变时不变；两个槽位同一款只 stat 一次，不像包名的 id 不拼路径；并检查 `KeyboardSession.ets` 在读包之前记身份、`attach` 在修订号检查之后调用 `reloadReplacedSkinPackages`。改过 `.ets` 后本机 `hvigorw assembleHap` 通过（ArkTS 编译无新告警）。没有在鸿蒙设备或模拟器上跑过：本机没有连着的 `hdc` 目标，键盘重画新装饰这一步只由接线检查和编译覆盖。
- `apps/desktop/tests/community/use-community-candidate-skin-gallery.test.tsx`：同一发布 id 换了 `updated_at` 后重新读取预览，给出新图，再取同一 revision 时复用。

## Existing notes audit

按皮肤包重载、`reloadSkins`、`resolveSelectedSkin`、预览缓存、同名替换检索：

- [候选窗口皮肤图库状态独立复用](../simplification/2026-10-17-community-candidate-skin-gallery.md) 持有预览 LRU 的归属，就地同步了缓存键的事实。
- [候选皮肤详情](../simplification/2026-10-17-candidate-skin-detail.md)、[预览缓存搜索等待](../testing/2026-10-16-candidate-preview-cache-search-wait.md) 管展示边界和测试等待，与缓存键无关，不动。
- [鸿蒙共享沙箱](../architecture/2026-10-09-harmony-ime-shared-sandbox.md) 持有镜像的发布与导入时机，就地改了「皮肤在获得焦点时生效」那条事实，指向本篇。
- [候选皮肤 v2 原生宿主生命周期](../architecture/2026-10-13-candidate-skin-v2-native-host-lifecycle.md)、[浅色深色槽位](../feature/2026-10-09-candidate-skin-light-dark-slots.md) 管动效生命周期和槽位规则，本篇只补「包被替换时何时重新解析」，没有同机制的旧提案需要吸收或拒绝。

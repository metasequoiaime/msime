# Agent Note: Windows 原生设置窗口自己画「社区皮肤」「社区插件」两个标签

Status: implemented

## Problem

原生设置窗口的「主题」页还有一行「社区皮肤」，用 `MSIME.exe --route=settings:skin` 拉起共享 Tauri 应用，在那里浏览、安装和发布候选窗口皮肤；「插件」页则根本没有「社区插件」。共享外壳（`packages/ui/src/index.tsx`）在 Windows 上给这两页顶上各放一对标签：「我的皮肤 / 社区皮肤」挂 `CommunityCandidateSkinsPage`（`community/community-candidate-skins.tsx`，连同 `candidate-skin-publish-dialog.tsx` 和 `useCandidateSkinSync`），「我的插件 / 社区插件」挂 `CommunityPluginsPage`（`community/community-plugins.tsx`）；`ExternalSkinCard` 上还有「发布到社区」。整项工作要求主题和插件页的任何一行都不再拉起共享应用。上一步已经备好 `msime_client_community`（见 [原生设置窗口社区页的 C 接口](../architecture/2026-10-11-windows-settings-community-host-api.md)）。

## Decision

- 两页顶上各画一对来源标签（分段控件，名字是「皮肤来源」「插件来源」），停在哪一边跨页面保留，与共享外壳的 `skinView`、`pluginView` 一样；图库随页面挂上和卸下（`enter_*_community` / `leave_*_community`，`owner` 递增作废在途回答），挂上时从第一页读起，皮肤图库还同步一次皮肤库。「社区皮肤」「社区插件」进搜索索引，选中时打开图库，选中本页别的设置时回到本页。图库顶替本页设置时，主题页上方的候选窗口预览也不画。
- 两个图库共用一套模板化的状态和操作（`Gallery<Item>`，对应 `useCommunityGallery` 和 `useCommunityInstallState`）：读页（插件整页被宿主略过时在工作线程上最多再读五页）、按 id 去重的「加载更多」、打开详情先显示列表项再换上详情、一次只做一个操作、评分后重读详情、下架后从第一页重读、举报、未登录时的「去登录」（打开「账号与同步」页）。皮肤的分类和插件的类型筛选第一页读失败时退回上一次成功的值。安装前先扫本机目录，同名时先问「替换安装」，皮肤另外认 `candidate_skin_exists`；装好皮肤后「我的皮肤」重扫、再同步一次皮肤库；装好插件后「我的插件」回来时重读目录。
- 预览图经 `candidate_skin_preview` 取 data: 地址，在 UI 线程解码成 `BitmapImage`；每款只读一次、卡片和详情共用，读到后就地换进正在显示它的卡片，不重画整页（重画会丢掉搜索框的焦点）。正在显示预览图的卡片位置以 `Border` 强引用记在 `community_preview_views_`，每次画页面时清空，不用 `weak_ref`：只由可视树持有的 `Border`，弱引用在创建时还能解析，之后就解析不出来了，卡片却还在页面上，读到的图于是一直填不进去，要等「加载更多」、切换标签之类的整页重画才从缓存里显示出来（#10005，在 Windows 11 实机上用临时记录确认：二十张卡片里另持强引用的十张就地换上了图，没持的十张在解码完成时弱引用全部解析为空，期间没有重画，回调都在 UI 线程上）。缓存容量是 16 加当前列表的条数，免得列表一重画就把刚读到的又读一遍；读失败的留着显示「预览图加载失败」，重新挂上图库时再试。
- 两个发布对话框是 `ContentDialog`，内容随异步状态重建（排到当前事件之后），名称、说明、自定义授权和权利确认输入时只就地更新主按钮，不重建、不丢焦点。主按钮按下时取消关闭、由操作成功后自己关上；忙时「取消」和 Esc 关不上。发布 id 是 `CoCreateGuid` 的 UUID，换皮肤、公开范围、分类、名称或说明时换一个，失败重试沿用。名称框 `MaxLength(32)` 与共享输入框的 `maxLength` 同样按 UTF-16 单元，建议名称按字截到 32 个。
- 「生成预览图」照 `skin-preview-render.ts` 在工作线程上用 Direct2D、DirectWrite 和 WIC 画：皮肤支持浅色时画浅色，装饰带、按包圆角和最小宽度的卡片、按 cover / contain / stretch 和不透明度裁切的背景图、拼音和设置预览的前五个候选，每 dip 两像素；PNG 超过 256 KiB 改用质量 0.85 的 JPEG，仍超过时失败。配色用扫描时 `msime_client_resolve_theme` 解析好的那一份。图片来自打开对话框时 `read_skin_catalog` 读到的包内图片，SVG 解码不了时那一部分不画（共享页读不出的图片也是不画）。设置工程因此多链接 `d2d1.lib`、`dwrite.lib`、`windowscodecs.lib`。
- 规则和文案放进只有头文件的 `CommunityPageModel.h`：`candidateSkinMessage`、`communityPluginMessage`（回落到 `PluginPageModel.h` 的 `error_message`）、审核类三句、授权行、作者行、分类、举报原因、`communityPublishFields`、`toFixed(1)`（正好在中间时取大的那个，与 JavaScript 一致）、zh-CN 千分位、`boundedGraphemes`、`TextEncoder` 字节数、空列表的句子和两段上传须知。由 `tests/ui/community_page_model.cpp` 核对，加进 CMake。
- `shell_links::skin` 删掉，`all` 和导航测试一起改；`community` 留着给账号页「我的内容」（词包与回复模板不在这一步）。README 写明两个标签。

## Alternatives considered

- **只把「社区皮肤」一行改成打开共享应用的社区页** — 改动最小，但仍拉起共享应用，正是这一步要去掉的。
- **两个图库各写一套** — 读页、详情、评分、下架、举报的流程完全相同，分开写会有两份要一起改的代码；用模板按条目类型选操作名和出错说明。
- **按 Intl.Segmenter 精确分字（ICU）** — Windows 的 `icu.dll` 要 1903 以后，WinUI 3 支持到 1809；名称框本来就按 UTF-16 单元限 32 个，分字只用在截断建议名称和搜索词上，按 UAX #29 里会碰到的几条规则近似。
- **「生成预览图」不做，只让作者自己加** — 共享对话框在 Windows 上有这个按钮（宿主给了 `readSkinImage`），缺了它没有预览图的皮肤就要手改 `skin.toml`。
- **预览图读失败就从缓存删掉（照共享页）** — 原生页面每次操作都整页重画，删掉后会马上再读，失败的图会被反复请求。

## Consequences

- [共享图库的手动整库同步](../bug-fix/2026-10-13-candidate-skin-explicit-library-sync.md)取消共享页面及设置组件启动的自动同步；本篇记录的 Windows 原生 WinUI 触发时机独立于共享图库，不随该修复改变。

- 主题页和插件页不再有任何一行拉起共享应用；仍会打开共享应用的只有输入工具面板（屏幕键盘、语音、手写、云词库、云剪贴板）和账号页「我的内容」打开的社区词包与回复模板。
- 有意与共享页不同的几处：下架后连同「已安装」状态一起清掉（共享页漏清，下一个打开的作品会显示「已安装」）；卡片固定大小，长名称和说明截断；确认和举报都是页面里的内联块，与共享页一样，不是对话框。
- 共享的社区组件、`community-helpers.ts`、`skin-preview-render.ts` 或 `plugin-catalog-helpers.ts` 的文案和规则改了，要同步 `CommunityPageModel.h` 和 `main.cpp`。

## Verification

- `tests/ui/community_page_model.cpp` 和 `tests/ui/settings_navigation.cpp` 用 cl `/W4 /WX` 单独编译运行，都通过。
- 设置工程按 `/WX` 编译通过（`settings-test-build.ps1`，链接上一步构建的 `msime_host_api.dll.lib`）。Rust 和共享 TypeScript 没有改动。
- 没有启动窗口，也没有对真实社区服务发请求：两个图库、详情、安装、评分、举报、下架、两个发布对话框、皮肤库同步和 Direct2D 画的预览图都没有在运行中的窗口里看过。

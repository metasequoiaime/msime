# Agent Note: 桌面主题页的「制作皮肤」候选窗皮肤编辑器

Status: implemented

## Problem

候选窗皮肤包（`skin.toml` 加图片）此前只有三个来源：手工在电脑上整理文件夹再导入、从社区下载、让 AI 助手经 MCP 的 `create_candidate_skin` 写一份。普通用户既不会写 TOML，也不知道颜色键、尺寸上限和 schema 2 的动效写法，想换一套自己的配色只能在「自定义配色」里调七个取色器，做不出带背景图、装饰图和动效的完整皮肤，也存不成一个能在「本地皮肤」里切换、能同步和发布的包。后续还要让 AI 直接生成皮肤草稿，这需要先有一个能装下草稿、能预览、能校验并保存的编辑器。

## Decision

桌面主题页的顶部标签变成「本地皮肤 · 制作皮肤 · 社区皮肤」。「制作皮肤」紧跟本地皮肤：做好的皮肤回到本地皮肤里，社区标签没有客户端时不出现，制作入口的位置不变。标签由宿主能力 `HostCapabilities.candidate_skin_editor` 加上客户端的 `SettingsClient.candidateSkinEditor` 共同决定，不看平台名：client-core 对三个桌面平台报真；HarmonyOS 由 `SettingsFormFactorCapabilities` 只在 2in1 上打开；手机外壳本来就没有桌面皮肤标签。

### 草稿

编辑器的全部状态是一份可 JSON 序列化的 `CandidateSkinDraft`（`packages/ui/src/skin/candidate-skin-draft.ts`），图片以 base64 存在草稿里。所有来源都经 `normalizeCandidateSkinDraft` 收紧到编辑器能表示的范围再载入：空白、内置主题（配色取自 `theme-catalog.json`）、Windows 外观、复制已安装的皮肤。以后的 AI 方案同样交一份草稿给 `CandidateSkinEditor` 的 `initialDraft`，不需要另一条路径。

- 编辑：名称、ID（按名称推导，手改后不再跟随；规则与 `catalog::is_external_id` 相同）、说明、底色（`system`、内置主题或 Windows 外观）、候选排列和明暗模式、每种明暗的八个主色和选中竖条、圆角（可交给宿主）、最小宽度、背景图（填充方式、不透明度）、装饰图（顶部留白、宽度、对齐、待机动效和周期）、schema 2 的出现、消失、选中动效与时长、逐行间隔。
- 底色是固定明暗的内置主题时，皮肤只用在那种明暗，另一种的勾选框禁用，与 `skinSlot` 的槽位规则一致。
- 每种明暗都写全八个主色，宿主画出的就是预览里的样子，不依赖各宿主自己的默认色。
- 配色行复用公共 [`HexColorRow`](../simplification/2026-10-24-hex-color-row.md)：原生取色保留透明度，手输色值只提交有效颜色，无效草稿失焦恢复当前值；明暗模式的实例 key 继续由编辑器持有。
- 复制已安装的皮肤时，编辑器没有控件的部分（细分颜色和右键菜单配色、工具栏、作者、授权、版本）放在 `carried` 里原样写回；内边距、字体、阴影这些 msime-windows 布局键和 canvas 贴图不复制，页面会说明。
- 用到动效或装饰图待机动效时写 schema 2，并把三个预设都显式写出（macOS 只播放写出来的预设），关掉动效也写成 `none`，免得 Windows 和网页套用默认组合；否则写 schema 1，任何版本都认。
- 每种明暗实时给出候选文字、编号、释义在窗口底色和选中背景上的 WCAG 对比度，半透明颜色先叠到底色上再算，低于 4.5:1 时提示。背景图不计入，说明里请用户看预览。

### 预览

预览用本地皮肤卡片同一套 `SkinCandidatePreview`、`customCandidateStyle` 和 `skinGeometryStyle`，浅色、深色并排，横排或竖排切换，不叠加用户在「候选窗口」页的圆角和不透明度。横排预览贴在内容区顶部，改颜色时一直看得见；竖排太高，不贴。「播放动效」按 web 引擎（`packages/web-engine/src/candidates.js`）的同名关键帧依次播放消失、出现和选中，装饰图的待机动效一直在动；系统要求减少动态效果时都不播放，并说明原因。

### 保存

保存时先用发布对话框补画预览图的同一个 `renderSkinPreview` 画出 `preview.png`（超过 256 KB 时是 `preview.jpg`），再写出引用它的清单，连同 `background.*`、`decoration.*` 交给宿主。宿主一侧是 client-services 的 `skin::candidate_editor`：

- `create` 走 `candidate_community::install`，与社区下载和 MCP 的 `create_candidate_skin` 是同一条校验与原子替换路径；清单被拒时再用 `check` 读一遍，把目录加载器的原因放进 `detail`，页面换成中文说明。
- `check` 在临时目录里按目录扫描的规则读一份清单，不写皮肤目录。编辑器用它向宿主要 Windows 外观补齐后的配色：这些配色只在 `catalog/windows_looks.rs` 里有一份，页面不复制。
- Tauri 的 `candidate_skin_check`、`candidate_skin_create` 直接调用它们，`create` 之后重扫皮肤目录（Linux 由此重新发布 `candidate_skin_catalog`）；C ABI 的 `msime_client_candidate_skin_editor` 供 HarmonyOS 和以后的原生设置使用，鸿蒙页面经 `startRequest` 的 `candidate_skin_editor` 调用，皮肤目录由 ArkTS 填，页面不指名路径。

同 ID 的皮肤只在用户确认后整体替换：宿主以 `candidate_skin_exists` 拒绝，页面弹出确认，确认后带 `replace` 重试。这次编辑已经保存过的 ID 再保存只是「保存修改」。保存成功后通知皮肤目录改动，本地皮肤随即重扫；「立即使用」与本地皮肤卡片的开关一样经 `applyCandidateSkin` 放进它所属的明暗槽位。

编辑器画在偏好表单之外（与社区图库相同），有自己的保存，不触发偏好自动保存；切走标签或页面时保持挂载，草稿不丢。

### 后续

- Windows 原生设置（`platforms/windows/settings`）、iOS 和 Android 的原生设置各自实现编辑器界面，宿主一侧直接用 `msime_client_candidate_skin_editor`（Android 经 JNI 包一层）。
- 第二阶段：AI 生成候选窗皮肤草稿，填进这个编辑器。
- msime-windows 布局键（内边距、行距、字体、阴影、背景材质等）、canvas 九宫格与自由挂件、工具栏配色、细分颜色的编辑。
- MCP 的 `create_candidate_skin` 仍在 `crates/mcp-server/src/skins.rs` 里自己诊断清单问题，可以改用 `candidate_editor::check`。

## Alternatives considered

- **页面自己写 Windows 外观的配色表** —— 最强理由是不必经过宿主，浏览器预览里也能用。但这些颜色是从 msime-windows 渲染端抄来的，client-services 已经有一份，网页 SDK 还从那份生成自己的；再在 TS 里抄一份，Windows 改配色时三处要一起改。改成让宿主用装皮肤的同一条规则补齐。
- **把预览区的 DOM 截图存成预览图** —— 最强理由是预览图和编辑器里看到的完全一样。但在 webview 里把 DOM 画进 canvas 要引入 html2canvas 一类的依赖，跨域的 data URL 图片和字体还会污染画布；发布对话框已经有一个按清单画预览图的 `renderSkinPreview`，社区卡片和本地列表看到的预览也来自它，所以沿用它。
- **编辑器放进偏好表单里，作为皮肤页的一部分** —— 最强理由是少一层外壳接线。但表单里的改动会触发偏好的自动保存和「未保存」状态，而皮肤包的保存与偏好无关；社区图库因为同样的原因画在表单之外，编辑器照此处理。
- **按平台名决定是否显示标签** —— 最强理由是最简单。但鸿蒙同一个 HAP 在手机和 2in1 上能力不同，平台名分不出这两种形态；能力位与仓库里其他宿主能力的做法一致，以后的原生设置也能按它判断，每个宿主在接通编辑器的那次改动里自己打开。

## Consequences

- **收益**：用户在设置里就能从任意起点做出一款完整的候选窗皮肤，保存出的包与社区下载、MCP 生成的包遵守同一套规则，能直接同步和发布；草稿结构和 `normalizeCandidateSkinDraft` 是第二阶段 AI 草稿的入口。
- **代价与已知上限**：编辑器只覆盖每个宿主都画的主色和几何，复制带有布局键或 canvas 贴图的皮肤会丢掉这些部分；背景图浅色深色共用，对比度不计背景图；动图（APNG、GIF、WebP）超过 1 MB 或 2048 像素时页面不缩小，请用户先压缩。Windows、iOS、Android 的原生设置还没有这个标签。目录加载器新增的拒绝原因要在 `candidate-skin-editor-messages.ts` 里补一句中文，否则页面会附上英文原因。

## 历史审计

- [MCP 独立设置页](../architecture/2026-10-18-mcp-independent-settings-page.md)无改动：MCP 的 `create_candidate_skin` 仍是 AI 助手的入口，与编辑器共用安装器。
- [装饰图待机动效与 APNG](2026-10-16-skin-decoration-idle-motion-and-apng.md)部分重叠：本地皮肤卡片的预览照旧画静态第一帧，编辑器预览按同样的幅度播放待机动效，减少动态效果时停下。
- [macOS 只播放声明的动效](2026-10-16-macos-candidate-declared-motion.md)部分重叠：编辑器据此显式写出三个预设。
- [皮肤编辑器改用平台控件](2026-10-12-skin-editor-platform-controls.md)无改动：那是触屏键盘的「设计我的皮肤」，候选窗编辑器沿用它的平台控件和「预览在控件上方」的布局。
- [社区候选皮肤测试夹具](../../implemented/testing/2026-10-19-candidate-skin-gallery-fixtures.md)无改动。

## Verification

- `apps/desktop/tests/skin/candidate-skin-draft.test.ts`：草稿到清单的序列化（schema 1/2、显式预设、图片路径、TOML 转义、复制时带回的部分）、ID 推导与校验、对比度、来源收紧、打包时预览图的文件名、宿主拒绝的中文说明。
- `apps/desktop/tests/settings/candidate-skin-editor.test.tsx`：能力决定标签、保存交给宿主的内容、「立即使用」、替换前确认、错误显示、Windows 外观向宿主取配色、复制已安装皮肤、预览的明暗说明与动效按钮。
- Rust：`client-services` 的 `skin::candidate_editor` 测试、`host_surface` 能力测试、host-api 的 `msime_client_candidate_skin_editor` 测试、Tauri 命令测试；HarmonyOS 的 `platforms/harmony/tests/run.sh` 与 `scripts/test-harmony-bridge-parity.py`。
- 浏览器里以合成宿主打开设置页，在浅色和深色下截图检查了标签、贴顶预览、竖排预览、配色与对比度、图片和动效各组以及保存后的状态。

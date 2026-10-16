# Agent Note: Windows 原生设置窗口自己画社区词库与回复模板

Status: implemented

## Problem

原生设置窗口里除了输入工具面板以外，最后一处会拉起共享 Tauri 应用的地方是「账号与同步」页的「我的内容」：「我发布的词库」「我发布的回复模板」「收藏的词库」「收藏的回复模板」四行都用 `MSIME.exe --route=settings:community` 打开共享应用的社区页。共享路由带不了要打开哪个列表，四行都落在社区首页，按钮上的「在社区中打开」也不是共享页的文案。整项工作要求设置窗口不再为任何设置页拉起共享应用，只留下屏幕键盘、手写、云词库、云剪贴板这些输入工具面板。共享应用在 Windows 上的社区页是 `community-page.tsx` 只有 `resources` 时的 `CommunityHomePage`（`community-home-page.tsx`）：「词库」「回复模板」两个分类，各是一个 `CommunityResourcesPage`（`community-resources.tsx`）。`msime_client_community` 已经有 `resource_*` 七个操作和 `report`（见 [原生设置窗口社区页的 C 接口](../architecture/2026-10-11-windows-settings-community-host-api.md)），本机词库的导入有 `msime_client_settings_dictionary`。

## Decision

- 「我的内容」的四行改成共享页桌面版 `MeRow` 那样的整行按钮（右侧是 ›），点开后在「账号与同步」页里换上社区词库与回复模板，停在 `communityDestinationView` 给的分类和范围上（我发布的是 `mine`，收藏的是 `saved`）。顶上一个「账号与同步」返回本页，样子同插件页视图的返回；离开「账号与同步」页时卸下图库，作废在途回答并关上发布对话框，下次从头读起。
- 图库接进主题页和插件页已有的 `Gallery<Item>` 模板，加第三种条目 `CommunityResourceItem`：读页按 `kind` 和 `scope` 调 `resource_list`；`detail`、`rate`、下架的操作名由 `community_operation<Item>` 选；出错说明用 `resourceMessage`，不给「去登录」（共享页的资源图库没有 `onLogin`）；举报的 kind 是 `dictionaries` 或 `replies`；评分说明是「我的评分（可重新选择）」。照 `ResourceDetail`，下架后只关上详情，列表不重读、不说明；第一页读失败时不再给「加载更多」。范围按钮是「全部 / 收藏 / 我的作品」三段，换分类时范围和搜索词照旧，与共享页同一个组件实例换 `kind` 的行为一致。
- 详情照 `ResourceDetail`：作者和版本、收藏与评分一行；词库是词条预览、「导入这版词库到本机」「导入这版词库到云端」和说明，回复模板是提示词预览；然后是「收藏，关注后续更新 / 取消收藏」、作者的「编辑并发布新版本」、评分或「下架作品」和确认、别人的作品的「举报」。Windows 宿主没有命名词库，本机导入走 `importCommunityDictionary`：词条按类型分组，每组一次标准格式的 `import`，经 `msime_client_settings_dictionary`（与「词库」页和共享应用的 `dictionary_request` 是同一段代码），请求 id 是 `community-local-<毫秒>-<类型>`；读不到宿主选项时按存储失败说明。桌面没有「高情商回复」键盘，所以没有「添加到高情商回复键盘」。
- 「发布作品」和「编辑并发布新版本」是 `ContentDialog`，照 `ResourceEditor`：作品名称、作品说明、权利确认（新版本不要）；回复模板是最多 2000 字的提示词，词库是类型、编码、词语、权重和「添加词条」，下面是待发布的词条和「移除」。与共享表单一样主按钮一直能按，按下时才校验，不合规则时在对话框顶上说明。发布 id 打开时定下（新作品随机，新版本沿用原来的），改名称或说明不换。成功后关上；新作品从第一页重读图库，新版本重读详情。
- 规则和文案放进 `CommunityPageModel.h`：`resourceMessage`、分类和范围、`communityDestinationView` 的四行、图库标题和说明、卡片的图标字和收藏一行、词条类型、`addEntry` 的规则（`Number(weight)` 按 JavaScript 的写法解析，再要求是非负的安全整数）、`submit` 的校验和 `importCommunityDictionary` 的分组，由 `tests/ui/community_page_model.cpp` 核对。
- `shell_links::community` 删掉，`all` 只剩五个面板；导航测试改成要求每个 `shell_links` 都是面板、不是设置页。README 写明这一页和剩下的面板。

共享首页组合与受控分类控件的模块归属见[社区首页](../../implemented/simplification/2026-10-16-community-home.md)，原生分类与范围规则保持。

## Alternatives considered

- **保留打开共享应用社区页的四行** — 改动最小，但这正是要去掉的最后一处设置页跳转。
- **给本窗口加一个独立的「社区」页** — 共享应用的桌面侧栏没有社区页，Windows 只从「我的内容」进入；单独一页还要改导航模型、路由别名和搜索索引，而 `community` 路由已经指向主题页的「社区皮肤」。
- **为词库与回复模板另写一套图库** — 读页、详情、评分、下架、举报与皮肤和插件的流程相同，模板多一种条目即可，差别（操作名、出错说明、有没有「去登录」、下架后的处理）用 `if constexpr` 分开。
- **给 Rust 加一个「导入社区词库到本机」的接口** — 共享页本来就是在界面里分组、再逐组调本机词库的导入，原生窗口已经有同一个导入入口，不需要新的 C 接口。

## Consequences

- 设置窗口里会拉起共享应用的只剩输入工具面板：屏幕键盘、语音输入、手写、云词库和云剪贴板。`PageHost::Shell` 和 `build_shell_page` 仍在，但没有任何页用它们。
- 有意与共享页不同的几处：卡片固定大小，长名称和说明截断；分类和范围是分段控件，不是按钮组和窄屏菜单；返回「账号与同步」是本窗口加的，共享应用里回去靠侧栏。
- 共享的 `community-resources.tsx`、`community-home-page.tsx`、`community-category-tabs.tsx`、`community-helpers.ts` 的 `resourceMessage` 或 `community-destination.ts` 改了，要同步 `CommunityPageModel.h` 和 `main.cpp`。

## Verification

- `tests/ui/community_page_model.cpp` 和 `tests/ui/settings_navigation.cpp` 用 cl `/W4 /WX` 单独编译运行，都通过。
- 设置工程按 `/WX` 编译通过（`settings-test-build.ps1`，链接重新构建的 `msime_host_api.dll.lib`）。Rust 和共享 TypeScript 没有改动。
- 没有启动窗口，也没有对真实社区服务发请求：图库、详情、本机和云端导入、收藏、评分、举报、下架和发布对话框都没有在运行中的窗口里看过。

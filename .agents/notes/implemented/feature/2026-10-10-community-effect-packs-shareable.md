# Agent Note: 特效包可以发布到社区插件库，插件库只放在 msime-cloud 数据库

Status: implemented

## Problem

社区插件库最初上线时（`c326db1469`），客户端把特效包排除在可发布类型之外，理由写在 `PUBLISHABLE_KINDS` 的注释里：特效包只是给各宿主内置样式填几个参数，没有值得当文件分享的东西。于是 `pack` 对 `effect` 返回 `plugin_community_kind`，列表里的特效包条目被 `validate_page` 去掉、计进 `skipped`，桌面端发布对话框也不列出特效包，社区页没有「特效包」筛选。

可服务端早已接受 `effect`：它在 `internal/account/community_plugin_archive.go` 的 `pluginKinds` 白名单和永远返回的冻结旧类型集合里，`validPluginEffect` 按 `effect_pack.rs` 的规则校验清单，表的 `kind` 约束也包含它。结果是一个用户调出来的配色、强度和时长没法分享，服务端会列出特效包而客户端只能跳过，两边对同一个类型的态度不一致。

同时还有一个悬而未决的问题：插件库该放在哪里。`docs/plugins.md` 写着「社区插件收集在 metasequoiaime/msime-plugins，欢迎把自己的包提交到那里」，而应用内的「插件 → 社区插件」读写的是 msime-cloud 数据库，两个入口说的不是同一个库。

## Decision

特效包与其他七种类型一样可以发布、浏览、筛选和安装：

- `crates/client-core/src/plugins/community.rs` 的 `PUBLISHABLE_KINDS` 加入 `PluginKind::Effect`，顺序与 `PluginKind::ALL` 一致，现在等于全部八种类型。`effect` 属于服务端冻结的旧类型集合，不用写进 `KINDS_DECLARATION`；`kinds_declaration_lists_the_new_publishable_kinds` 显式排除旧类型后比较，仍然成立。
- `pack` 把特效包的清单和说明文件打成归档，`install` 按同样的校验装回去；`validate_page` 不再去掉特效包条目。`validate_page` 里按 `PUBLISHABLE_KINDS` 过滤的那一步保留，现在什么也不去掉，留给将来旧类型集合与可发布类型不一致的情况。
- `packages/ui/src/community/community-plugins.tsx` 的 `CommunityPluginKind` 等于 `PluginKind`，`communityPluginKinds` 加入 `effect`，社区页多一个「特效包」筛选，发布对话框列出所有非内置的已装包；`community-helpers.ts` 里不会再出现的「特效包暂不支持分享。」删掉。

插件库只放在 msime-cloud 数据库，不同步到 GitHub 仓库。`docs/plugins.md` 改为指向应用内「插件 → 社区插件 → 发布我的插件」，并写明服务端的配额：压缩后 8 MB、每个账号 20 个、合计 32 MB、每小时 10 次。GitHub 仓库 `metasequoiaime/msime-plugins` 已于 2026-10-10 由维护者删除：其中 17 个精选包由 msime-cloud 的种子一次性导入数据库，文档里所有指向它的链接（作者模板、`msime-pack` 的用途说明）一并去掉；每种类型的清单写法都在 `docs/plugins.md` 里，能通过校验的完整例子在 `crates/client-core/tests/fixtures/plugin-packs/valid/`。

## Alternatives considered

- **特效包继续只留在本机。** 最强的理由是原注释说的：特效包没有文件，参数又都有固定范围，用户在设置里几下就能调出同样的效果，分享价值低，少一种类型就少一份要测的路径。不这样做，是因为一套配色和节奏仍然是别人可以直接拿来用的作品；服务端早已接受并校验 `effect`，客户端放开只是删掉一条限制，没有新增服务端工作；而继续排除会让服务端列出、客户端跳过，翻页还得靠 `skipped` 补偿。
- **把插件库改成 GitHub 仓库 `metasequoiaime/msime-plugins`，或把数据库里的发布同步过去。** 最强的理由是仓库公开、可审阅、有版本历史，CI 已经用 `msime-pack` 按客户端同样的规则校验每个包，作者也能用熟悉的 PR 流程投稿。不这样做，原因有四：
  - 客户端的 `BackendAccountClient` 只连 `https://api.msime.app`（`crates/client-core/src/account.rs` 的 `ACCOUNT_ORIGIN`），这是为了中国大陆用户能稳定访问；从应用里直接读 GitHub，在那里经常连不上。
  - 评分、下载计数、下架与举报的审核状态、每个账号的数量和容量配额、发布频率限制都在数据库里，仓库承载不了，同步过去也只能是数据库的一份不完整镜像。
  - 仓库按 `packs/<id>/` 存放，包 id 必须全局唯一；数据库的 `community_plugins` 不要求 `plugin_id` 唯一，不同作者可以各自发布同 id 的包，同步时无法无损映射。
  - 仓库投稿需要有人审 PR，现在没有维护者审核队列；应用内发布由服务端校验后立即上架，事后靠举报和下架处理。

## Consequences

- **收益**：八种类型的分享路径完全一致，客户端与服务端对可发布类型的判断重新对齐；用户只需要知道应用内一个入口，文档不再指向一个与应用不相通的仓库。
- **代价**：特效包没有文件，归档里只有清单和说明，社区里可能出现大量只差几个参数的近似条目，目前只能靠评分和举报来筛。`plugin_community_kind` 这个错误码在当前类型集合下已经不会出现，`pack` 里的检查仍然保留；前端删掉了它的专门文案，万一出现会落到通用的插件错误提示。
- **验证**：client-core 的 `plugins::community` 单测覆盖特效包的打包、发布请求校验、列表解码保留特效包、按 `kind=effect` 请求和下载后安装；桌面端 `apps/desktop/tests/community/community-plugins.test.tsx` 覆盖「特效包」筛选和发布对话框列出特效包。没有对线上服务端做端到端发布。

# Agent Note: 社区屏蔽作者只认登录账号，由服务端过滤目录

Status: implemented

## Problem

App Store 审核（Guideline 1.2）要求有用户生成内容的应用让用户能屏蔽滥用的用户。社区的皮肤、候选窗皮肤、插件、词库、回复模板和短语包此前只有举报、发布时筛查、封号和下架，没有用户自己能用的屏蔽。作品条目里的 `author` 只是显示名，不是稳定身份，客户端没有可靠的东西可以按它屏蔽。

## Decision

- 屏蔽按作品发起：客户端发 `POST /v1/community/blocks {kind, item_id}`，`kind` 与举报相同，服务端解析作品的 `owner_id` 并记在调用者账号下（`community_blocks(user_id, blocked_id)`，每个账号最多 500 条）。服务端从不把作者的账号编号返回给客户端，客户端只拿到这条屏蔽自己的编号、作者现在的显示名和屏蔽时间；`GET /v1/community/blocks` 列出，`DELETE /v1/community/blocks/{id}` 取消。
- 过滤只在服务端做：四个公开目录（键盘皮肤、候选窗皮肤、插件、社区资源）在除 `mine` 以外的所有 scope 里排除调用者屏蔽的作者，详情、下载和收藏接口不变。客户端屏蔽或取消屏蔽之后只重新读取列表，不在本机维护屏蔽名单。
- 屏蔽只用登录账号，never 回退到本机匿名账号，这一点与举报不同。客户端共享实现是 `crates/client-services/src/community/block.rs` 的 `BackendCommunityBlockService`（三个方法都用 `request_with_account_session(..., true, ...)`），宿主经 `msime_client_community` 的 `block`、`block_list`、`unblock` 三个操作调用；iOS、Android 与 HarmonyOS 没登录时点「屏蔽作者」先请用户登录。
- HarmonyOS 的社区投稿总闸（`HostCapabilities.community_contribution`）不管屏蔽：屏蔽不公开任何内容，所以总闸关着、举报不显示时，屏蔽作者照常提供。

## Alternatives considered

- **像举报一样允许匿名账号屏蔽。** 最强的理由是没登录的用户也能立刻屏蔽，门槛最低。否掉是因为目录按列表请求带的令牌过滤，而目录请求只在有登录会话时带令牌（`request_with_account_session(..., false, ...)`），匿名账号的屏蔽对这些请求不起作用；要起作用就得让四类目录请求都改成带匿名令牌，牵动所有宿主。匿名账号还随重装丢失、不跟着用户换设备，用户会以为屏蔽失效。举报是一次性的通知，丢了身份无所谓，屏蔽是要长期生效的状态。
- **只在客户端按作者显示名隐藏。** 不用改服务端，最快能交审核。否掉是因为显示名可以改、可以重名，按它屏蔽既会漏掉改名后的作者，也会误伤同名的人；而且每个平台要各存一份名单，换设备就没了。
- **在作品条目里返回作者的账号编号，让客户端按编号屏蔽。** 客户端能自己过滤、也能本机缓存。否掉是因为这会把每位作者的内部账号编号公开给所有浏览者，只为一个列表过滤付出隐私代价不值得；按作品发起屏蔽、由服务端解析作者，同样稳定又不暴露编号。

## Consequences

- 收益：屏蔽在用户的所有设备上一致生效，重装后仍在，注销账号时随外键级联删除；客户端不存名单，不需要跨平台同步本机状态；作者编号不出服务端。
- 代价：没登录的用户不能屏蔽，只能举报；需要先登录这一步要在审核录屏里演示。服务端每个公开目录查询多一个 `NOT EXISTS` 探测（走 `UNIQUE(user_id, blocked_id)` 索引，没登录时恒真）。
- 详情接口不过滤：从旧链接或缓存仍能打开被屏蔽作者的单个作品，客户端屏蔽成功后关掉详情并刷新列表。
- `community_invalid` 在 `block` 上才表示「不能屏蔽自己」，`community_conflict` 表示超过 500 条；Rust 的错误码是归并过的粗码，各宿主只在 `block` 操作上把它们换成这两句话。
- 入口：iOS 与 Android 是原生页面；HarmonyOS、macOS 和 Linux 用 `packages/ui` 的共享社区页（`community-block.tsx`），HarmonyOS 经设置桥的 `community_block` 请求、桌面经 Tauri 的 `community_block`、`community_block_list`、`community_unblock` 命令。宿主没有提供屏蔽客户端时，共享页不显示「屏蔽作者」和「已屏蔽的作者」，不会出现点了无效的按钮。
- Windows 原生设置不使用共享社区页，目前没有屏蔽入口，也没有「已屏蔽的作者」名单；它的社区页如果以后要提供，接同一组 `msime_client_community` 操作即可。过滤在服务端按账号做，所以在别的设备上屏蔽的作者在 Windows 的社区图库里同样不出现，但要取消屏蔽只能回到 iOS、Android、HarmonyOS、macOS 或 Linux 上。

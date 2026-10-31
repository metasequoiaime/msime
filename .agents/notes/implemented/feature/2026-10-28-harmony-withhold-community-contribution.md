# Agent Note: 鸿蒙版在实名认证到位之前关闭社区投稿，并且不列 AI 生成的社区皮肤

Status: implemented

## Problem

华为应用市场（中国大陆）审核指南第 12 章要求「信息发布」类服务和生成合成内容服务先做真实身份认证。鸿蒙版当时允许登录用户向社区发布键盘皮肤、词包、短语、回复模板和插件，修改已发布作品的分类，发布新版本，以及给他人作品评分和举报，全程没有实名核验：邮箱登录不核验手机号，华为账号登录只申请 openid 和 idtoken。

另外，别的平台上用 AI 皮肤生成发布的皮肤会出现在鸿蒙的社区里，没有任何 AI 标识；鸿蒙版自己的 AI 皮肤生成已经由 [AI 生成类功能的总闸](2026-10-27-harmony-hide-ai-generation.md)关掉，但浏览别人的 AI 作品仍然等于在展示生成合成内容。

维护者的决定：只在鸿蒙版上关闭发布、评分和举报入口，浏览和安装照常；服务端给 AI 生成的皮肤打来源标记，鸿蒙版不列这些皮肤。

## Decision

### 投稿类功能的总闸

总闸是一个宿主能力：`crates/client-core/src/host_surface.rs` 的 `HostCapabilities.community_contribution`，`for_platform` 里只有 `HostPlatform::Harmony` 为 false。

- **共享设置页**（`packages/ui`）：`settingsCapabilities` 派生 `showCommunityContribution = host?.community_contribution !== false`，经页面模型交给 `SettingsCommunityPage`、`SettingsSkinCommunitySection`、`SettingsPluginCommunitySection` 和 `CommunityGallerySubview`，再以 `contributions` 一路传进 `CommunityPage`、`CommunityHomePage`、`CommunitySkinsPage`、`CommunityResourcesPage`、`CommunityPluginsPage` 和资源详情。为 false 时不画：发布按钮和发布对话框（皮肤、词包与回复模板、插件），资源详情的「编辑并发布新版本」，自己作品的分类选择，他人作品的评分按钮和举报。`CommunityModerationSection` 的 `onRate`、皮肤详情的 `onChangeCategory`、插件详情的 `onRate` 改为可选，不给就不画，与早已可选的 `onReport` 同一个写法。皮肤页交给皮肤编辑器和 AI 皮肤生成的 `communitySkins` 也按它给，不给就没有 AI 皮肤的「发布到社区」（鸿蒙的 AI 皮肤另受 `ai_generation` 关闭，这里防的是两道闸先后打开的那段时间）。
- **鸿蒙设置宿主**（`apps/harmony/src/main.tsx`）：能力为 false 时三个社区客户端经 `withoutCommunityContribution` 包一层，`publish`、`packPreview`、`rate`、`setCategory` 直接拒绝（`community_contribution_unavailable`），`report` 不给。页面漏掉一处没隐藏，请求也发不出去。
- **鸿蒙设置桥**：`platforms/harmony/entry/src/main/ets/keyboard/CommunityContributionGate.ts` 在 `Settings.ets` 的 `attach` 里 `adopt` 宿主能力，只有明确为 true 才打开；`startRequest` 在分发社区请求之前问 `allowsSettingsRequest`，对 `community_skin`、`community_resource`、`community_plugin` 三类请求拒绝 `keyboard_skin_publish`、`keyboard_skin_rate`、`keyboard_skin_set_category`、`resource_publish`、`resource_rate`、`plugin_pack_preview`、`plugin_publish`、`plugin_rate`、`report`，读不出操作名的社区请求也拒绝，回 `community_contribution_unavailable`。键盘进程只读本地 `CommunityLibrary.json`，不经社区接口联网，不接这道闸。
- 照常提供的：浏览、搜索、详情、获取和试用皮肤、安装插件、收藏、导入词包、存取回复模板，以及下架或删除自己的作品（撤回内容不算发布）。账号页的「我发布的…」只是跳到社区的「我的作品」，照常显示。改昵称和头像保留：昵称只会作为作者名出现在作品上，鸿蒙账号在别的平台发过作品时才会露出来。
- 屏蔽作者（2026-10-31 加入，见 [社区屏蔽作者](2026-10-31-community-block-authors.md)）不算投稿，鸿蒙版照常提供：它不公开任何内容，也不对作品公开表态，只是让本账号的社区列表不再列出那位作者的作品，所以不属于「信息发布」，审核指南第 12 章的实名要求管不到它；反过来，举报关着之后它是鸿蒙用户面对滥用内容时唯一能自己动手的办法，再关掉就只剩反馈。实现上它不经三条图库通道：页面经设置桥自己的 `community_block` 请求发 `block`、`block_list`、`unblock`，`CommunityContributionGate` 只管三条图库通道、不拦它，它只要隐私政策的授权；`apps/harmony/src/main.tsx` 的屏蔽客户端（`communityBlockClient`）不经 `withoutCommunityContribution`，共享页面里的「屏蔽作者」也不看 `contributions`，在举报被隐藏时照样画在别人的作品上。
- `msime_client_community` 不按平台判断：它也给 Windows 原生设置窗口用，而且拿不到平台信息，所以闸在宿主和页面两侧。

### 不列 AI 生成的社区皮肤

这一半挂在已有的 `HostCapabilities.ai_generation` 上，不另设开关：鸿蒙版拿到 AI 生成类资质之前，AI 生成的作品既不能生成也不展示；那道闸打开时，鸿蒙会和其他平台一样看到 AI 皮肤，那时应改为显示「AI 生成」标识而不是隐藏。

- **ArkTS**：`HarmonyAccountRequests.communityRequest` 多一个由宿主给的 `excludeAiGenerated`，`HarmonyAccountCloudBridge.community` 按 `!AiGenerationGate.enabled()` 传入；为 true 时请求文档带顶层的 `exclude_ai_generated: true`，与页面的 `action` 并列，页面写进 `action` 的同名字段会被 Rust 的 `deny_unknown_fields` 拒绝，关不掉它。
- **Rust FFI**：`crates/host-api/src/ffi/community.rs` 的 `CommunityRequest.exclude_ai_generated`（缺省 false）交给 `keyboard_skin_list`、`keyboard_skin_detail`、`keyboard_skin_download`、`keyboard_skin_install`，其他操作不看它；契约写在 `crates/host-api/include/msime_client.h`。
- **client-services**：`crates/client-services/src/skin/community.rs` 的 `CommunitySkinApi` 三个读接口和 `BackendCommunitySkinService` 的 `list`、`detail`、`download`、`download_with_anonymous` 多一个 `exclude_ai_generated`；Tauri 移动端（`mobile_community.rs`）传 false，行为不变。为 true 时经 `BackendAccountClient::json_with_required_header` 发请求，要求成功响应带着能力头，否则是 `community_unavailable`；这时服务端答的 400 也读成 `community_unavailable`，因为参数由本端拼出、事先校验过，400 只会是旧服务端不认 `fields=origin`。`CommunitySkin` 多一个可选的 `origin`（`user` / `ai` / `unknown`，将来的新值读作 `unknown`），列表里 `origin` 不是 `user` 的条目（包括没带 `origin` 的）在客户端再去掉一次，详情不是 `user` 时按 `community_not_found` 处理，与服务端「只留 `user`」的语义一致。

### 与服务端的契约

服务端（msime-cloud）按下面实现，部署到所有副本之后鸿蒙版的社区皮肤才能用；在此之前鸿蒙的皮肤图库答「社区暂不可用」，宁可不显示也不交出没筛过的列表。

- 查询参数 `exclude=ai`：`GET /v1/community/skins` 在分页之前只留下 `origin='user'` 的作品，`ai` 和所有 `unknown` 都去掉，公开列表、`scope=mine`、`scope=saved` 都算；`GET /v1/community/skins/{id}` 和 `POST /v1/community/skins/{id}/download` 对被排除的作品答 404 `skin_not_found`。只接受空值或 `ai`。`unknown` 是服务端推不出来源的作品：带照片的存量作品、滚动升级期间旧副本写入的作品，以及服务端后台回填还没改成 `user` 的没有照片的作品（上线后第一次回填之前，所有存量作品都是 `unknown`）。
- `fields=origin`：条目多一个 `"origin": "user" | "ai" | "unknown"`，只在请求声明了才给（旧客户端的条目结构拒绝未知字段）；要与 `moderation` 合写成 `fields=moderation,origin`。
- 认得 `exclude=ai` 的副本在上面三个接口的成功响应里带 `X-MSIME-Skin-Origin: v1`。不认得的旧副本对 `exclude=ai` 照常答 200、不带这个头，对 `fields=origin` 答 400 `invalid_fields`；两种情况客户端都按社区不可用处理。
- 鸿蒙发出的三种请求：列表 `?offset=..&q=..[&scope=mine&fields=moderation,origin | &fields=origin][&category=..]&include=category&exclude=ai`，详情 `?fields=moderation,origin&include=category&exclude=ai`，下载 `/download?exclude=ai`。

## 重新打开

- 实名认证到位后，把 `host_surface.rs` 的 `community_contribution: platform != HostPlatform::Harmony` 改成 `true`，同步改 `only_harmony_withholds_community_contribution` 测试和 `apps/desktop/tests/support/host.ts` 的 `community_contribution: !harmony`。页面、宿主客户端和设置桥都跟着这一个值回来。
- AI 生成类资质到位、`ai_generation` 打开时，`exclude_ai_generated` 自动不再发送，鸿蒙会列出 AI 皮肤；那之前要先让各端读 `fields=origin` 并画出「AI 生成」标识，否则会重回驳回理由。`host_surface.rs` 的 `ai_generation` 文档和 `only_harmony_withholds_ai_generation` 测试的说明都写了这一点。

## Alternatives considered

- **补上实名认证（发布前绑定并核验手机号）**：最强的理由是直接满足审核要求，鸿蒙用户也保留投稿能力。不选是因为账号体系目前没有手机号核验流程，华为账号登录也没申请手机号权限，要改服务端、客户端和隐私政策，周期远长于这次上架；维护者决定先关入口。
- **只隐藏页面上的按钮**：改动最小。不选是因为页面之外还有旧页面、手写请求和别的页面复用同一组客户端，[AI 生成类功能的总闸](2026-10-27-harmony-hide-ai-generation.md)同样在页面、宿主客户端和设置桥三层都拦。
- **由 Rust 按平台拒绝发布**：只改一处。不选是因为 `msime_client_community` 也给 Windows 原生设置窗口用，Rust 侧拿不到调用方平台，宿主能力本来就是平台差异的唯一出处。
- **只在客户端按 `origin` 过滤 AI 皮肤**：不依赖服务端先上线。不选是因为过滤发生在分页之后，会出现短页甚至空页；详情和下载拦不住按 id 打开的作品；旧服务端根本不给 `origin`，客户端分不出来。客户端过滤只作为服务端过滤之后的第二道。
- **服务端默认隐藏 AI 皮肤、由客户端声明才显示**：最强的理由是连已发出的旧鸿蒙包也受益。不选是因为 Android、iOS 和桌面的旧版本也会一起看不到 AI 皮肤；而社区请求的 User-Agent 是默认的 `MSIME/Android`，服务端认不出鸿蒙。鸿蒙版尚未上架，旧包只有 GitHub 上旁加载的几个版本，损失可以忽略。
- **没有能力头时退回不过滤的列表**：社区在服务端上线前仍然能用。不选是因为那正是要避免的状态：AI 生成的皮肤会在没有标识的情况下出现在鸿蒙上。
- **鸿蒙不请求 `fields=origin`，只靠服务端过滤和能力头**：旧副本就不会答 400。不选是因为客户端的第二道过滤要靠 `origin`；400 改读成「社区不可用」之后，两种做法对用户的表现相同。
- **保留举报，或在详情页放一个预填作品 id 的「问题反馈」入口**：评审指出《网络信息内容生态治理规定》第十六条要求展示用户生成内容的平台设置便捷的投诉举报入口，关掉举报之后，审核员可能以「有用户生成内容展示、没有举报机制」为由驳回。举报本身是把内容私下交给审核员，不算信息发布。维护者的决定是这一轮连举报一起关，所以这里没有改；如果审核因此驳回，最小的改法是把 `report` 从 `CommunityContributionGate.ts` 的 `CONTRIBUTION_OPERATIONS`、`apps/harmony/src/main.tsx` 的 `withoutCommunityContribution` 和页面上 `contributions &&` 的举报条件里拿出来，三处一起改。反馈（`feedback`）目前仍然开着，用户可以从那里反映具体作品。

## Consequences

- **收益**：鸿蒙版的社区不再有发布、评分和举报的路径，审核信息可以写明应用内无信息发布功能；AI 生成的皮肤在服务端筛掉，分页正确，详情和下载也拦得住。闸只有两个值，其余五个平台的界面和请求不变。
- **代价**：鸿蒙用户暂时不能分享自己的设计、词包和插件，也不能评分和举报；在服务端部署上面的契约并全部副本升级之前，鸿蒙的社区皮肤图库一直是「社区暂不可用」，词包、回复模板和插件不受影响。服务端上线后，所有 `unknown` 的皮肤都不在鸿蒙上显示：带照片的存量作品（其中有用户自己拍的照片）要等审核员改判，没有照片的存量作品要等服务端第一次后台回填。作者自己在鸿蒙上的「我的作品」同样只列 `user`，看不到自己 `unknown` 或 `ai` 的作品，要在别的平台下架。
- **没覆盖的**：服务端的判定只是兜底，下面这些 AI 皮肤会被记成 `user`、出现在鸿蒙上：用户在应用外用 AI 生成图片、再当作照片背景上传；把插画翻转、裁剪或加边后再发布；只有名字、描述和配色由大模型生成、没带插画的 AI 皮肤（Android 的 `AiSkinPage` 照片压不下去时就不带照片）；服务端部署之前生成、之后才发布的插画（服务端只给部署之后生成的插画记指纹，AI 皮肤可能在本地放几周才发布）。堵住后三类要靠各端在发布 AI 皮肤时声明 `"origin":"ai"`（`packages/ui/src/keyboard/use-ai-skin-generation.ts` 经 `communitySkins.publish`，以及 Android 原生的 AI 皮肤页），这要在本地皮肤记录里留住「来自 AI 生成」，而且要等服务端所有副本升级之后单独提交：旧副本以 400 `invalid_json` 拒绝这个键，提前发会让所有平台的发布失败。设置同步会不会把别的设备上的 AI 设计带进鸿蒙本地皮肤库，这一轮没有核实。改昵称和头像仍然开着，见上文。
- **要重访的信号**：实名认证到位；AI 生成类资质到位；或者以后新增任何社区投稿入口——它必须同时接 `contributions`（页面）、`withoutCommunityContribution`（`apps/harmony`）和 `CommunityContributionGate`（设置桥），否则会绕过这道闸。

## Verification

- Rust：`crates/client-core/src/host_surface/tests.rs` 的 `only_harmony_withholds_community_contribution`；`crates/client-services/src/skin/community/origin_tests.rs` 用本机回环上的假服务端核对请求路径、能力头、旧服务端的 400、404 和客户端只留 `user` 的二次过滤（`ai`、`unknown` 和没带 `origin` 的都去掉）；`crates/host-api/src/ffi/community/tests.rs` 的 `the_host_may_ask_to_exclude_ai_generated_skins`。
- 共享设置页：`apps/desktop/tests/community/community-contribution-gate.test.tsx` 覆盖能力派生、三个图库在关着和开着时的入口，以及鸿蒙手机「社区」标签从宿主能力到页面的整条路径。
- 鸿蒙原生：`platforms/harmony/tests/community-contribution-gate.test.ts` 覆盖能力解析和设置桥的放行规则，`account-requests.test.ts` 覆盖 `exclude_ai_generated` 的拼法；`tests/run.sh` 钉住 `adopt`、`startRequest` 里闸在社区分发之前，以及 `HarmonyAccountCloudBridge` 按 AI 总闸传参的接线。ArkTS 没有在本机用 `hvigorw` 编译，鸿蒙真机上的验收还没做。

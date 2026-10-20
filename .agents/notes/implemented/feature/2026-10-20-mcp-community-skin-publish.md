# Agent Note: MCP 把候选窗皮肤发布到社区

Status: implemented

## Problem

MCP 服务器（`crates/mcp-server`）能做、改、选用和删除本机的候选窗皮肤（见 [MCP 管理候选皮肤](2026-10-19-mcp-candidate-skin-management.md)），但做好的皮肤要分享出去，用户还得自己打开设置应用、同步、在卡片上点发布。用户希望助手一并完成发布。

发布是对外的动作：公开作品所有人都能看到、下载和评分，而且要用账号身份。MCP 此前刻意不持有账号会话，服务器说明里写着「this server never signs in to the account itself」。

## Decision

新增四个工具，会话一律借用设置应用登录后保存的那一份，服务器自己从不登录。

- `publish_candidate_skin`（需要 `--allow-write`）：参数 `package_id`、`visibility`（`public` 或 `private`，必填、没有默认值）、可选 `category`、`name`、`description`、`license_assets`。结果给出社区 ID、标题、可见性、分类和 `replaced`（是否替换了账号库里已有的那一行）。
- `list_community_skins`（只读，总是提供）：账号社区库的同步列表，每行 `id`、`package_id`、`visibility`、`updated_at`。
- `set_community_skin_visibility`（需要 `--allow-write`）：按社区 ID 改成私有或公开。
- `delete_community_skin`（需要 `--allow-write`）：按社区 ID 删掉库里那一行，走 `candidate_sync::unpublish`，同步状态里对应的记录一并忘掉。

### 会话从哪里来

`community::session_location` 与各平台设置应用的 `desktop_account` 一致：

- macOS：`~/Library/Application Support/<设置应用的 bundle id>/account-session.json`，Swift 后端的 `Apple` 布局，输入法也读这一份；
- Windows：`%LOCALAPPDATA%\<本版本的用户目录>\account\account-session.json`，`Native` 布局，Server 也读这一份；
- Linux：共享状态目录（即偏好所在的 `state_dir`）下，`Native` 布局。

版本按运行时选项里记录的版本取，与 MCP 管理的状态目录对得上。每次调用重新建 `BackendAccountSession`，用户在设置应用里登录、退出或换账号后不用重启服务器。`FileAccountSessionStorage` 声明跨进程共享，每次访问重读文件，刷新令牌只在 `account-refresh.lock` 下进行，所以 MCP 与设置应用、输入法同时用这份会话时不会拿同一枚刷新令牌刷两次（这条约束见 [宿主注入会话存储](../../proposed/architecture/2026-10-16-host-injected-session-storage.md) 的 Problem）。会话文件不存在或没有登录时，工具不发任何请求、不改任何文件，回答「请在水杉输入法设置应用里登录」。令牌只在 client-services 里使用，不进结果，审计行只写工具名和结果。

### 发布的顺序

1. 检查 `package_id` 是已安装的外部皮肤。
2. 读会话确认已登录（只读文件，不联网）。没登录就到此为止，连清单里的授权都不写。
3. 给了 `license_assets` 而清单没有 `[license] assets` 时，用共享的 `candidate_community::add_license` 写进清单；清单已有同样的值时不动，已有别的值时拒绝，不替用户改掉。
4. 按所选可见性在本机 `pack_as` 一次：被拒时给出具体原因（缺授权、缺预览、尺寸超限等），同时拿到默认标题和简介。标题 1–32 个字符、简介至多 280 个字符在这里先查。
5. 调用共享的 `candidate_sync::publish`。它在同步状态文件或账号库同步列表里按 `package_id` 找已有的行，找到就替换内容、再按需改可见性和分类，找不到才用这次随机生成的发布 ID 新建，最后把结果记进同步状态。

第 5 步与设置应用的发布对话框完全相同，所以「设置应用和 MCP 不为同一款本地皮肤各建一行」靠的是按 `package_id` 去重，而不是两边算出同一个发布 ID：设置页的发布 ID 本来就是每次随机的。`replaced` 由返回行的 ID 是否等于这次生成的 ID 判断。

### 给调用模型的约束

工具说明要求：公开发布前用大白话向用户确认「所有人都能看到」以及用户自己做的图或有权分享；从别人社区作品下载来的皮肤不是用户的；公开作品缺授权时先问用户再传 `license_assets`。`visibility` 没有默认值，模型必须明说。分类用 MCP 自己的枚举，写错的值被拒绝，不像 `CandidateSkinCategory` 那样读成 `other`。账号服务器的错误按 `AccountError` 翻成能照着做的话（库满 100 款或公开满 20 款、每小时公开 10 次、内容含禁用词、服务端还不接受扩展资源等）。

## Alternatives considered

- **MCP 自己登录（验证码或浏览器登录）。** 最强的理由是不依赖设置应用：只装了命令行、没开过设置应用的用户也能发布。不用：登录要处理验证码、Google 回调和密码类凭据，等于让模型经手账号凭据；而且 MCP 和设置应用会各有一份会话或互相覆盖。用户本来就在设置应用里登录，借用那一份最少、最安全，没登录时一句话就能让用户去登录。
- **按 `package_id` 和账号算出确定的发布 ID，让两边重试落到同一行。** 看起来能在网络中断后避免重复建行。不用：设置页的发布 ID 是随机的，单改 MCP 不能让两边一致；而 `candidate_sync::publish` 在新建之前已经先查同步状态和账号库的同步列表，同一个 `package_id` 已有行就替换，重复建行的窗口只剩「服务端建好了、回应没收到、紧接着又在另一边发布」这种情况，与设置页现有行为相同。
- **公开、私有和改可见性合成一个 `share_candidate_skin`，不单列可见性工具。** 调用面更小。不用：把已发布的作品转回私有只需要社区 ID，本机甚至可能已经没有那款皮肤，用发布工具做不了；单独的可见性工具对应服务端的 PATCH，也是设置应用卡片上的同一个动作。
- **不提供删除。** 删除会清掉下载和评分，风险比改私有大。仍然提供：共享层的 `candidate_sync::unpublish` 一行就能接上，并且它同时清理同步状态，避免设置应用下一次同步把本机皮肤删掉；说明里要求先确认，并指出只想隐藏时改私有即可。

## Consequences

- **收益**：「做一款皮肤、换上、分享出去」可以全程由助手完成；打包规则、去重和错误码与设置应用完全一致，没有第二套发布逻辑。
- **代价**：MCP 不再是纯本机工具。只读的 `list_community_skins` 在没有 `--allow-write` 时也提供，会用设置应用的会话访问账号服务器；刷新令牌时也会写回会话文件。设置页「连接 AI 助手」和 Windows 原生设置窗口的说明从「不联网」改成「除了把皮肤发布到社区之外不联网」。
- **代价**：发布持有 MCP 的跨进程写锁直到请求结束，最长约 90 秒，期间其他 MCP 写操作要等。
- **已知上限**：Linux 上 `add_license` 改了清单后不重新发布 IBus 和 Fcitx5 读的 `candidate_skin_catalog`（`create_candidate_skin` 会发布，见 [Linux 上 MCP 写入皮肤后重新发布皮肤目录](../bug-fix/2026-10-20-linux-mcp-skin-catalog-republish.md)）：授权不在目录条目里，宿主没有可更新的东西。

## Testing

- `crates/mcp-server/src/community/tests.rs`：用只在内存里应答的假账号服务器（实现 `AccountApi` 和 `CandidateSkinCommunityApi`）加上临时目录里真实的 `FileAccountSessionStorage` 会话文件，覆盖没有会话时不发请求也不改清单、私有新建后公开替换同一行（顺序为 replace、visibility、category，每个请求都带会话文件里的令牌而结果里没有令牌）、只在账号库里有的行按 `package_id` 替换而不新建、清单已有授权时的处理、本机就能拒绝的情况不发请求、可见性和分类拼错时拒绝、列表和改私有和删除、错误说明和各平台的会话位置。输出 schema 的必填字段都在结果里（`assert_required_fields_present`）。
- `crates/mcp-server/src/server.rs`：工具只在 `--allow-write` 时提供（列表除外），说明里有确认和不登录的要求。`tests/stdio.rs` 的工具列表随之更新；stdio 测试不调用社区工具，因为测试机上的真实会话会让它连到正式服务器。

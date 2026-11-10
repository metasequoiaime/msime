# Agent Note: MCP 把手机键盘皮肤发布到社区

Status: implemented

## Problem

MCP 服务器（`crates/mcp-server`）能做候选窗皮肤并发布到社区（见 [MCP 把候选窗皮肤发布到社区](2026-10-20-mcp-community-skin-publish.md)），但手机键盘皮肤没有任何工具。键盘皮肤是 iOS、Android 和 HarmonyOS 触屏键盘的外观，一份 `TouchKeyboardSkinDesign` 文档（配色、键形、一张可选的背景照片），和候选窗皮肤包（`skin.toml` 加图片）是两种东西，社区里也是两张表（`/v1/community/skins` 与候选窗皮肤的接口）、两套 id。要往键盘皮肤社区放一批作品，只能在手机上一款一款手工做、手工发。

键盘皮肤的发布比候选窗皮肤更不可逆：没有私有状态，发布即公开；没有编辑，改一处只能删掉重发，下载数和评分随之清零。

## Decision

新增三个工具，主体在 `crates/mcp-server/src/keyboard_skins.rs`，`server.rs` 里只有登记。会话与其他社区工具相同，借用设置应用登录后保存的那一份，服务器自己从不登录。

- `publish_keyboard_skin`（需要 `--allow-write`）：参数 `name`、可选 `description`、可选 `category`、`design`、可选 `photo_file`、可选 `id`。结果是 `id`、`name` 和 `photo_bytes`（没有照片时为 null）。
- `list_community_keyboard_skins`（只读，总是提供）：账号自己发布的键盘皮肤，带 `scope=mine` 逐页读，每款给 `id`、`name`、`description`、`category`、`moderation`、`downloads`、`rating_count`、`rating_average`。服务端的条目没有创建或修改时间，所以结果里也没有。
- `delete_community_keyboard_skin`（需要 `--allow-write`）：按 id 删掉自己发布的一款。

三个工具都只调用共享的 `msime_client_services::skin::community::BackendCommunitySkinService`（`publish`、`list`、`unpublish`），与手机设置页是同一个接口。桌面上没有触屏键盘，工具不写本机的键盘皮肤库和偏好。

### 请求里的设计

`design` 是 MCP 自己的结构，键名与手机上的设计 JSON 相同（camelCase），两处不同：

- 颜色写成 `"#RRGGBB"`，手机文档里是整数。`#` 和六位十六进制缺一不可，其他写法拒绝。
- 没有 `photo` 键，照片只能经 `photo_file` 给出。

`keyShape`、`keyMaterial` 和 `category` 是严格的枚举，写错的值在解析时拒绝；多余的键也拒绝（`deny_unknown_fields`）。没写的 `cornerRadius`、`borderWidth`、`shadow`、`pattern`、`monospaced` 取 `TouchKeyboardSkinDesign::default()` 的值。

取值范围交给共享的 `TouchKeyboardSkinDesign::validate()`，也就是手机发布和社区服务用的同一个函数。不合范围时拒绝并用一句话列出全部范围，从不调用 `normalized()` 把值收进范围：发布之后改不了，悄悄改掉代理给的值等于发出一款没人确认过的皮肤。

### 照片只读不转码

`photo_file` 必须已经是符合社区上限的 JPEG：长边不超过 1024 像素、不超过 512,000 字节。服务器只做三件事：

1. 按 `plugins.rs` 的 `read_local` 同样的规则读文件：绝对路径、`symlink_metadata`、拒绝路径上的符号链接、必须是普通文件、读之前按大小拒绝、读时按同一上限截断。
2. 检查字节以 JPEG 的 SOI 标记 `FF D8 FF` 开头，不是就明说要一张什么样的 JPEG。
3. 用 `TouchKeyboardSkinDesign::encode_photo` 写成 `photo` 字段的形式。手机宿主和 `validate()` 要的是不带 `data:` 前缀、带填充的标准 base64（Swift `JSONEncoder` 对 `Data` 的写法），`encode_photo` 用的就是 `validate()` 解码的那个引擎。

512,000 这个上限在 `validate()` 里是函数内的字面量，没有公开常量，`keyboard_skins.rs` 里照写一份并注明出处。像素尺寸和扫描数这里不查，由社区服务器判定，被拒时的说明把这两条列为可能的原因。

`encode_photo` 是这次给 `client-core` 加的唯一一个函数：`mcp-server` 不依赖 `base64`，这次也不加依赖，而 `client-core` 本来就用它解码这个字段。

### 顺序与重发

1. 查标题（去掉首尾空白后 1–32 个字符，不含控制字符）、简介（至多 280 个字符，控制字符只允许 `\n` 和 `\t`）、`id`、颜色和取值范围。
2. 读会话文件确认已登录，只读文件、不联网。没登录到此为止，照片文件也不读。
3. 读照片。
4. 调用一次 `service.publish`，一次调用只发一个请求。

`id` 省略时每次生成新的 v4 UUID，所以每次调用是一款新皮肤。它只为一种情况存在：请求发出去却没拿到明确回答（连不上、超时、服务端 5xx）。这时错误说明写明皮肤可能已经发出去、也可能没有，并给出这次的 id，代理带着同一个 id 和同样的参数再发一次。

### 给调用模型的约束

工具说明写明：这是手机触屏键盘的外观，不是候选窗皮肤；发布即公开且之后不能编辑；发布前要用大白话向用户确认所有人都能看到、照片是用户自己的或有权分享；AI 生成的画面不能用这个工具发布；各字段的范围和照片的要求；一个账号至多 50 款；`keyForeground` 要在 `keyBackground` 上看得清。服务器说明（`INSTRUCTIONS`）和命令行帮助各加一句，把键盘皮肤和候选窗皮肤分开。

### 来源不能声明

社区皮肤有来源标记（`user`、`ai`、`unknown`），由服务端在发布时判定，HarmonyOS 不列来源不是 `user` 的作品（见 [鸿蒙版不列 AI 生成的社区皮肤](2026-10-28-harmony-withhold-community-contribution.md)）。共享的发布请求（`CommunitySkinPublishRequest`）只有 `id`、`name`、`description`、`design`、`category`，没有声明来源的字段，所以这个工具没有办法把一款皮肤标成 AI 生成。因此工具说明禁止用它发布 AI 生成的画面，请求里出现 `origin` 键会被当作未知键拒绝。列表工具也不给 `origin`：`scope=mine` 的请求不带 `fields=origin`，服务端不返回它，这里不去猜。

### 账号上限

一个账号至多保留 50 款键盘皮肤，服务端满了答 409。官方发布者白名单只对候选窗皮肤生效，不放宽键盘皮肤的这个上限。同一个 id 被另一款皮肤占用时服务端也答 409，客户端分不出这两种，所以 409 的说明两种原因都写。要放超过 50 款，只能分账号，或者由服务端另行放宽，这不在本次改动之内。

## Alternatives considered

- **在服务器里解码、缩放并重新编码成 JPEG。** 最强的理由是代理通常生成 PNG，而服务端对不合规的照片只回一个笼统的 400，由服务器转码能让任何常见图片一次发布成功。不用：`mcp-server` 没有图片和 base64 依赖，转码要么给它加依赖，要么在 `client-services` 里新开一个只有 MCP 调用的编码模块；解码不可信的大图要在持有跨进程写锁的阻塞线程上花几百 MB 内存和几秒 CPU；重发还要求两次编码逐字节相同，升级图片库之后就不再成立。能调用这个工具的代理本来就要用代码生成图片，让它直接存成合规的 JPEG 只多一行，服务器这边只剩「安全地读一个小文件」。
- **颜色沿用线上格式的整数。** 与手机文档逐字段相同，少一层转换。不用：让模型把 `#D8F0E4` 换算成十进制 14217444 容易出错，错了也看不出来，而发布之后改不了。
- **允许在 `design` 里直接给 base64 的 `photo`。** 不能写文件的代理也能带照片。不用：几十万字符的 base64 抄进调用很容易错一个字符，`create_candidate_skin` 已经因此改成优先收文件路径；键盘皮肤发布后不能改，更不该走这条路。
- **不合范围的值收进范围后照发（`normalized()`）。** 手机从库里读设计时就是这样做的，调用更不容易失败。不用：那是读旧文档时的容错，不是发布时的规则；发布不可编辑，收进范围等于发布一款代理和用户都没见过的皮肤。
- **加一个声明来源的参数。** AI 生成的皮肤就能如实标注并照常发布。不用：这要改共享的发布请求和它的线上格式，而且是否允许经 MCP 批量发布 AI 作品是产品决定，不是这次的范围。
- **提供原地更新。** 改一处配色不必丢掉下载数和评分。做不到：服务端的 PATCH 只改分类，没有改设计的接口。

## Consequences

- **收益**：键盘皮肤可以由助手成批制作和发布，校验、接口和错误码与手机设置页是同一套，没有第二套发布逻辑。
- **收益**：发布前的检查都在本机完成，范围、文字和照片的问题不用等服务端一个笼统的 400。
- **代价**：代理必须自己把照片存成合规的 JPEG。像素尺寸和扫描数本机不查，超了要等服务端拒绝，说明只能把它列为可能的原因之一。
- **代价**：对比度只写在说明里，没有强制，代理仍然可以发出一款看不清字的皮肤。
- **代价**：没拿到回答之后如果不带 id 重发，会多出一款重复的皮肤；这靠错误说明里的 id 减轻，不能杜绝。
- **代价**：`list_community_skins`、`delete_community_skin` 指的是候选窗皮肤，新工具名里带 `keyboard`，说明里也写了区别，但代理仍可能调错删除；两边 id 不通用，最坏是一次「没有这款皮肤」。
- **已知上限**：一个账号至多 50 款，官方发布者白名单不放宽。
- **有意没改**：`packages/ui` 的「连接 AI 助手」页（`settings/mcp-command-model.ts`）和 Windows 原生设置（`platforms/windows/settings/McpConnect.h`、`win7/McpPage.cpp`）里描述 MCP 能做什么的文字没有动。`packages/ui` 只修缺陷，不为新能力改文案；这几处写的都是「用设置里登录的账号把皮肤发布到社区」，对键盘皮肤同样成立，只是没有单独提到它。
- **待办在别处**：单独仓库里的用户指南还没有这三个工具，需要另行更新。

## Testing

- `crates/mcp-server/src/keyboard_skins/tests.rs`：用只在内存里应答的假账号服务器（实现 `AccountApi` 和 `CommunitySkinApi`）加临时目录里真实的 `FileAccountSessionStorage` 会话文件。覆盖：没有会话时先回答登录，不读照片也不发请求；完整的设计按手机文档的字段发出，省略的键取默认值；颜色写法、取值范围（边界值照发，越界拒绝且说明列出全部范围）、未知键和写错的枚举、标题和简介的规则；`photo_file` 的相对路径、不存在、目录、超限的稀疏文件、非 JPEG、符号链接和经符号链接目录的路径都拒绝且不发请求；一段合成的 JPEG 开头字节到达假服务器时正好是事先算好的 base64，并通过共享的 `validate()`；正好 512,000 字节的照片通过；`id` 的解析与省略时每次不同；409 说明两种原因，连不上时说明可能已发布并带着 id，错误里没有照片内容、路径和令牌；列表逐页读到 45 款、不带设计和来源，服务端一直说还有时在 5 页处停下；按 id 删除。输出 schema 的必填字段都在结果里（`assert_required_fields_present`）。
- `crates/mcp-server/src/server.rs`：写工具只在 `--allow-write` 时提供，列表工具总是提供；登记的说明就是 `keyboard_skins` 里的三段，服务器说明把键盘皮肤和候选窗皮肤分开。`tests/stdio.rs` 的工具列表和数量随之更新；stdio 测试不调用社区工具，因为测试机上的真实会话会让它连到正式服务器。
- 没有对正式的社区服务器做过真实发布。带同一个 id 和同样内容重发不会多出一款、50 款的上限且白名单不放宽、409 的两种原因、照片只收 1024 像素以内的 JPEG，这几条来自动手前对服务端（msime-cloud）代码的调研，不是这次测试验证的结果。

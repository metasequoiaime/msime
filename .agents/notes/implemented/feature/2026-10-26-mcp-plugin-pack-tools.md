# Agent Note: MCP 管理插件包：列出、读回、创建、导入、删除和发布

Status: implemented

## Problem

插件生态要让普通用户按需做自己的插件，包括直接请 AI 助手做：「给我做一个 K 模式的邮件签名短语表」「一个打出今天日期的 / 指令」「一组箭头符号」。MCP 服务器（`crates/mcp-server`）此前能做、改、删和发布候选窗皮肤（见 [MCP 管理候选皮肤](2026-10-19-mcp-candidate-skin-management.md)、[MCP 发布皮肤](2026-10-20-mcp-community-skin-publish.md)），插件却一样都不能：助手看不到装了哪些包、哪些启用着，写好的清单只能让用户自己存成文件夹、打开设置页导入，分享还得用户自己点「发布我的插件」。

## Decision

MCP 服务器新增 8 个工具，形状照皮肤那一套；写入的 5 个列进 `WRITE_TOOLS`，只在 `--allow-write` 时提供，并和其他写入工具一样经 `claim_write` 和跨进程的 `mcp-write.lock` 限速、串行：

- `list_plugins`（读）：`plugins::scan` 列出的每个包（类型、id、名称、版本、许可证、作者、简介、是否内置、是否官方包、一句话内容摘要、偏好里是否正在用），`PluginCatalog.issues` 里载不入的文件夹和原因，以及偏好的选用：`plugins.key_sound`（开关、方式、包）、`melody.pack`、`music`（开关、包）、`effect_pack`、`command_tables`、`phrase_tables`、`helpcode_pack_quanpin`、`helpcode_pack_shuangpin`，外加决定指令表和短语表起不起作用的 `local_modes.command` 和 `local_modes.quick_phrase`。
- `get_plugin`（读）：一个包的 `plugin.toml` 全文、文件夹绝对路径、其余每个文件的名字、大小和绝对路径。内置包也能读，可以照着改成用户自己的包。
- `create_plugin`（写）：清单文本加文件，装一个数据类包。
- `install_plugin_folder`（写）：按绝对路径导入已有的包文件夹或 `.zip`，脚本插件只能走这条。
- `delete_plugin`（写）：`plugins::remove` 删掉用户装的包。
- `publish_plugin`（写）：用设置应用登录的账号把已装的数据类包发布到插件社区。
- `list_community_plugins`（读）、`delete_community_plugin`（写）：账号自己发布的包，列出（`scope=mine`，被下架的也在内）和删除。插件社区 API 本来就有这两项（`CommunityPluginApi::community_plugins` 的 `mine` 和 `delete_community_plugin`），所以一并提供。

服务器说明（`INSTRUCTIONS`）加一段：插件有哪些类型；遇到上面那类普通请求，助手按 `create_plugin` 说明里的规则自己写清单、装好，再用大白话告诉用户怎么打开（短语表和指令表要在「设置 → 插件 → 我的插件」里打开「启用」，并在「输入 → 快捷模式」里开着 K 模式或 / 模式）。`create_plugin` 的说明写全了八种数据类型的清单规则（摘自 `docs/plugins.md`，以 client-services 的代码为准），助手不用另找文档。

### 规则只有一份

MCP 不另写任何插件规则。`create_plugin` 把清单和文件写进服务器自己用 `tempfile` 建的临时目录（Unix 上只有本用户可读写），拼成一个包文件夹，再交给设置页导入用的 `plugins::import_from_settings`；`install_plugin_folder` 直接把代理给的路径交给它。所以经 MCP 装进来的包与「插件」页导入的包过同一套检查：清单的每个键、文件清单、数据文件解析、保留 id、暂存再整包换入。拒绝时把 client-services 报的那条中文原因原样带回去，助手据此改清单。

导入本身遇到同类型同 id 的包总是整包替换；MCP 在导入之前先 `plugins::validate` 一次拿到类型和 id，已装着且没给 `replace: true` 时拒绝，与 `create_candidate_skin` 的 `replace` 语义一致。这一步和真正导入之间不持有插件根目录的锁：MCP 自己的写入有 `mcp-write.lock` 串行，与设置页同时导入同一个 id 的竞争只会让后到的整包替换，不会留下半个包。

### 文件怎么给

`files` 是「包内文件名 → 本机绝对路径」，读取规则与皮肤的 `image_files` 相同：必须是绝对路径，`msime_path_trust::reject_symlinked_components` 拒绝路径上任何一级符号链接（系统别名如 macOS 的 `/tmp` 放行），以 `file_lock::open_private_file_in` 不跟随符号链接地打开，必须是普通文件，超过 16 MiB（任何类型允许的最大单文件，一首音乐曲目）在读之前就拒绝，读时再按同一上限截断，全部合计不超过导入复制时的整包上限 66 MiB。服务器只读这些文件，不移动、不改写。为此 `msime-mcp-server` 加了对仓库内 `msime-path-trust` 的依赖（Windows 上 `open_private_file_in` 只对最后一级不跟随，中间一级的链接要靠它挡）。

内联的不是 base64，而是 `texts`：「包内文件名 → UTF-8 文本」，每个最多 1 MiB。数据类插件里的二进制只有音频，助手本来就要先用代码生成成文件；而辅助码表、单词本和说明文件都是文本，直接写进调用最省事。包内文件名在写进临时目录之前只检查是一个普通路径分量、不以点开头、不是 `plugin.toml`，挡住 `..` 和分隔符；字符、长度和保留名由导入检查。

### 内置目录

内置音效包和官方插件包的目录按 host-api `PluginRoots::new` 的规则从运行时选项读（`BuiltinPaths::from_options`）：`sound_packs`、`plugin_packs` 给了就用（只认绝对路径），没给时取 `resources` 旁边的 `sound-packs`、内置音效包目录旁边的 `plugin-packs`，存在才用。读不到时内置包列不出来，用户装的包照常列出、照常管理。`PluginRoots` 是 host-api 的 crate 内类型，这十几行在 MCP 里照抄而不是把它公开：host-api 的公开面是给宿主的 C 接口和 Rust 入口，多导出一个路径推导类型只为这一个调用方不划算。

### 脚本插件

`create_plugin` 拒绝 `kind = "script"`：脚本包要带一个 wasm 模块，助手应当用 `msime-pack` 构建、校验好文件夹，再用 `install_plugin_folder` 导入。拼包之前先按行认一次清单里的 `kind = "script"`，好给出指向 `install_plugin_folder` 的明确说法；导入检查之后再按解析出的真实类型拒一次，所以换一种写法绕不过去。

`install_plugin_folder` 导入脚本包后，结果的 `next_step` 明说它不会运行，要用户在「设置 → 插件 → 脚本插件（实验）」里打开并逐项授权。MCP 从不写 `script-grants.json`：导入时 client-services 的 `script_grants::installing` 按内容哈希收紧已有条目，那是导入自己的规则，与设置页导入相同；删除脚本包时它的授权条目由 `plugins::remove` 一并去掉。

### 删除

`delete_plugin` 拒绝 `plugins::is_builtin` 认的所有 id（内置音效包、背景音乐包和官方包），拒绝没装的包，其余交给 `plugins::remove`。偏好里还选着或启用着这个包时照删，结果的 `still_selected_in` 列出是哪几项；输入法读不到选中的包时回到默认，与设置页删除正在使用的包之后相同。

### 发布

`publish_plugin` 走设置页「发布我的插件」对话框同一条路（`use-community-plugin-publication.ts` → `plugin_community_pack_preview`、`plugin_community_publish`）：先拒绝不可发布的类型（脚本插件，`PUBLISHABLE_KINDS` 之外）、内置包和没装的包；只读会话文件确认有人登录（没有就回「请到设置应用登录」，一个请求都不发）；`community::pack_preview` 在本机按分享规则打一次包，被拒时给出 client-services 报的原因，也拿到默认的标题和简介；最后 `BackendCommunityPluginService::publish_installed` 以新生成的发布 ID 打包上传，字段与对话框发送的相同：类型、插件 id、发布 ID、标题、简介。插件社区没有私有和同步，发布即公开，所以工具说明要求先用大白话征得用户同意（所有人可见、音频和文字是自己的或有权分享、别人可按包内许可证下载使用），对应对话框里那个勾选框。

会话位置复用皮肤社区工具的 `community::session_location`，没登录时的说法复用同一个 `SIGN_IN`。

## Alternatives considered

- **像皮肤一样内联 base64。** 最强的理由是与 `create_candidate_skin` 的 `images` 对称，任何二进制都能内联。不用：数据类插件的二进制只有音频，几十 KB 起步，正是皮肤那边 base64 转写出错的同一种情况；文本数据文件内联成 base64 反而更难写对。而且 MCP 服务器目前不依赖 `base64`，为一种不推荐的写法新增依赖不划算。
- **由 MCP 替用户启用短语表、指令表或打开 / 模式。** 最强的理由是用户少走一步。不做：任务只要求告诉用户怎么打开；启用会改偏好，`update_preferences` 有修订号比较和一套只放行部分字段的规则，插件选用要进那一套得单独设计，不该顺手在创建里写偏好。现在由结果里的 `next_step` 说清楚怎么开。
- **像删除皮肤那样，偏好里选着就拒绝删除。** 皮肤这样做是因为删了会让槽位指向不存在的包、而 `update_preferences` 能改槽位。插件不同：设置页删除正在使用的包本来就允许，输入法读不到时回默认；MCP 又改不了插件选用，拒绝的话用户只能去设置页先关掉再回来删。所以照删并在结果里说明。
- **公开 host-api 的 `PluginRoots`。** 理由是推导规则只有一份。不做的原因见上文「内置目录」；两份规则都短，任何一边改了推导，`official_packs_from_the_runtime_options_are_listed_read_only` 测的只是 MCP 这一份，这是代价。

## Consequences

- 助手能独立完成「做一个插件、装上、告诉用户怎么开、愿意的话发布」整条路，规则与设置页导入完全一致，设置页拒绝的包 MCP 也拒绝。
- `create_plugin` 的工具说明很长：它写全了八种类型的清单规则。规则改了（`crates/client-services/src/plugins/` 与 `docs/plugins.md`）要同步改这段说明，否则助手会按旧规则写清单，被导入拒绝后再按报错改，代价是多一轮调用而不是装出坏包。
- 没登录、没装、内置、脚本插件和不合分享规则的包都在本机拒绝，不发请求；联网之后的拒绝按 `community_*` 代码说成助手知道下一步的话。
- 列出、读回只读插件目录和偏好；写入与其他写入工具共用限速和跨进程锁。

## Testing

- `crates/mcp-server/src/plugins/tests.rs`：八种数据类型逐个创建并在列表里出现、摘要和 `active` 正确；拒绝的清单说出是哪条规则（未知的键、缺数据文件、大写编码、保留 id、申请权限）；`replace` 语义；读回后用读回的文件路径整包替换；不安全的文件（相对路径、不存在、目录、超过 16 MiB 的稀疏文件、`..` 和分隔符、同名两边都给、符号链接、上一级是符号链接）在装任何东西之前拒绝；`create_plugin` 拒绝脚本清单；桌面目标上 `install_plugin_folder` 装下手写的最小 wasm 脚本包、`next_step` 说明要授权、`active` 为 null、删除后目录消失；删除拒绝内置、官方和没装的包，偏好里还启用着时报出是哪一项；从运行时选项的 `plugin_packs` 列出官方包为内置且只读。
- `crates/mcp-server/src/plugin_community/tests.rs`：内存里的假账号服务器。没登录时发布、列出、删除都回登录说明且不发请求；登录后发布的请求字段与对话框相同、标题和简介默认取清单、令牌不出现在结果里；标题长度检查；内置、没装、脚本插件、不合分享规则的包在本机拒绝；列出和删除自己的包；社区错误码的说法。
- `server.rs` 的工具清单测试和 `tests/stdio.rs` 的工具名单、数量随新工具更新；`plugin_tools_explain_enabling_grants_and_publishing` 核对说明里的关键句。
- 没在真机上让一个 MCP 客户端调用这些工具去装包再在输入法里打出短语；导入路径与设置页相同，宿主读同一个插件目录。

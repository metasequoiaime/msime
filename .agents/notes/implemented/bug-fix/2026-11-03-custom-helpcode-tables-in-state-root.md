# Agent Note: 自定义辅助码表放到状态根下，升级不再丢

Status: implemented

## Problem

#10707（QQ 群转述，没有平台和版本）：设置了自定义辅助码，升级输入法后配置丢了。

「自定义辅助码」在各端有两种落地。一种是辅助码表插件包（`kind = "helpcode"`，移动端「导入辅助码表…」和各端插件页装的包），装在状态根下的 `plugins/helpcode/<id>`，选择记在 `plugins.helpcode_pack_*`。状态根在 Windows 是 DataDir（安装器只清它自己的条目），macOS 是 Application Support，Linux 是用户目录，Android 是 `files/bootstrap/state`，iOS 是 App Group，HarmonyOS 是应用沙箱，升级都不碰它；包读不了时 host-api 退回 `schema`，偏好不改。这一种不会因为升级丢。

另一种是 `custom/<stem>` 方案：偏好写 `*_helpcode.schema = "custom/<stem>"`，Engine 和三个设置界面（Tauri 的 `list_helpcode_schemas`、Windows 的 `helpcode_schemas_call`、Win7 的 `helpcode_sources`）只从 `resources/helpcodes/custom` 读。`resources` 是安装包或词库代次，不是用户数据：

- Windows：`resources` 是 Program Files 下的 `server\resources`，安装器 `PrepareToInstall` 每次都 `TryDeleteTree` 整个 `server` 目录，轻量包也一样；装了在线词库更新后 `resources` 又是下载的代次。用户放在 DataDir `helpcodes\custom` 的表本来就不被读，而且 `CleanAppDataExceptUserFiles` 把整个 `helpcodes` 当安装包条目清掉。
- macOS：`resources` 是应用包里的 `EngineResources`（Sparkle 和手动替换都整个换掉），或者 Application Support 里手工暂存的目录，后者在词库锁变化时被 `refresh_host_options_from` 换成应用包里的那份。
- Linux、Android、iOS、HarmonyOS：在线词库更新把 `resources` 换成新代次，新代次里没有 `helpcodes/`。Linux 的 `msime-linux-setup --update` 复制资源时保留 `helpcodes/custom`，但换代次后不再指向那里。移动端用户写不进资源目录，`custom/<stem>` 实际只能来自同步下来的桌面偏好。

偏好里的 `custom/<stem>` 只做语法校验，找不到文件不会被改回默认，设置页也继续把它列在下拉框里；Engine 读不到表时装一张空表（`Session::set_helpcode_table` 与 `plugin_tables::helpcode_table` 都这样兜底）。用户看到的是「选的还是我的码表，但辅助码什么也不筛、不注释」，也就是报告里的「丢失」。

## Decision

用户自己的表的正式位置是状态根下的 `helpcodes/custom`，与资源目录同样的相对布局，`custom/<stem>` 先读这里：

- Engine：`load_helpcode_keymap_in(resources, schema, user_root)` 对自定义方案先读 `user_root/helpcodes/custom`，没有时读 `resources` 里的；内置方案不读 `user_root`。`EngineOptions::custom_helpcode_root` / `SessionOptions::custom_helpcode_root` 携带它，会话建立、`set_helpcode_schema`、`Session::set_helpcode_table(None)` 都经 `load_helpcode_keymap_with_fallback` 走这个顺序。只认绝对路径。
- host-api：`HostOptions::into_engine_options` 把 `preferences_directory` 填成 `custom_helpcode_root`，所以所有宿主（含 Windows 各应用进程里的 TIP、`msime-mcp`）不用改。`plugin_tables` 判断回退方案读不读得出来时用同一个函数。
- 列表：`msime_client_helpcode_schemas(resources, len, state_root, state_root_len)` 合并两处，同一主干取状态根那份（`client-core::helpcode::list_custom_helpcode_schemas_in`），与 Engine 读的那张一致。Tauri、Windows 设置和 Win7 设置都从运行时配置取 `preferences_directory` 传进来。产品未发布，C ABI 直接改签名，不留旧入口。
- 搬家：`client-core::helpcode::adopt_custom_helpcodes` 把资源目录里还在的表复制到状态根下，同一主干已有时不动，源文件不删，超过 1 MiB 的不搬，经临时文件 `persist_noclobber` 落地。host-api 在 `prepare_host_with_updates`（传入的资源目录、更新记录里的随包目录、选中的下载代次）和 `refresh_options_file`（改写前文档里的 `resources` 与 `helpcode_fallback_resources`）各调一次。这一步救的是「表还在、`resources` 将要或已经换走」的情况：macOS 从 Application Support 换到应用包、各端换成下载代次、以及修复版之后的每一次升级。
- Windows 安装器：`PrepareToInstall` 在 `CleanAppDataExceptUserFiles` 和删 `server` 目录之前，用 robocopy（`/XC /XN /XO`，同名不覆盖）把 `server\resources\helpcodes\custom` 和数据目录换位置时旧 DataDir 的 `helpcodes\custom` 搬进 DataDir 的 `helpcodes\custom`；`CleanPackageHelpcodes` 清内置码表时留下 `custom` 子目录。升级到修复版的那一次，旧 Server 目录在新代码运行之前就被删了，只有安装器来得及搬。

Windows 上状态根就是 DataDir，也是内置码表的回退目录（`helpcode_fallback_resources`）。`adopt_custom_helpcodes` 跳过与状态根相同的源目录。

## Alternatives considered

把旧的 `custom/<stem>` 表转成辅助码表插件包，并把偏好改写成 `plugins.helpcode_pack_*`。最强的理由是只剩一种机制，插件包早已放在状态根下、各端都有列表和删除入口。不用它：插件包按严格规则校验（同一个字出现两次、码超过两个字母都拒整张表，见 [移动端码表导入](../feature/2026-10-12-mobile-helpcode-table-import.md)），桌面 `helpcodes/custom` 一直按宽松规则读，很多现成的表转不过去；而且要在宿主启动时改写同步中的偏好，转换失败还得决定改不改回默认，比搬文件风险大。

只在 host-api 里自己读状态根下的表、作为 `helpcode_table` 交给 Engine（插件包就是这样接的），不动 Engine。最强的理由是 MPL 的 Engine 不加选项。不用它：Engine 还有 `set_helpcode_schema` 和 `set_helpcode_table(None)` 两条按方案名重新读表的路径，宿主给的表在那里会被资源目录里的空结果换掉，要在 host-api 里再盖文件戳、在每条路径上补；读表顺序放在 Engine 的一个函数里，三条路径和 `plugin_tables` 的回退判断一起跟上。

自定义目录选 DataDir 之外的新名字（例如 `custom-helpcodes/`），安装器就不用改。不用它：Windows 用户看到内置码表在 DataDir 的 `helpcodes` 里，自然会把自己的表放进 `helpcodes\custom`，那份以前既不被读、升级还被清；用同样的相对布局，这些表直接生效、也被保下来，文档里各端说法也一致。[下载代次回退](2026-10-23-downloaded-generation-helpcode-fallback.md)当时以「安装器会清掉 DataDir 的 `helpcodes`」为由不把自定义目录指到 DataDir，这里改的正是这一点：清理时留下 `custom`。那篇管内置码表的回退，决定不变，两篇互链。

只在换 `resources` 时搬文件，Engine 仍只读资源目录。不成立：Windows 和 macOS 应用包的资源目录归安装包，搬回去还是会被下一次升级删掉，而且 Program Files 和已签名的应用包普通用户进程写不进。

## Consequences

升级、在线词库更新、换资源目录之后，状态根下的自定义表照样生效、照样列在设置页里；资源目录里以前放的表在第一次准备或刷新配置时被复制过去，Windows 由安装器在删除之前复制。

已经在修复前的升级里被删掉的表找不回来：Windows 的 `server\resources\helpcodes\custom` 和 DataDir 的 `helpcodes\custom`、macOS 应用包里的 `EngineResources/helpcodes/custom`，升级那一刻就没了。这些用户要把码表文件重新放进状态根下的 `helpcodes/custom`，偏好里的 `custom/<stem>` 没被改动，放回去之后下一次聚焦就用上。

两处都有同一主干时状态根优先，资源目录里那份之后被改不再生效；搬家不覆盖，用户在状态根下改过的表不会被旧副本盖掉。准备流程每次多一次 `read_dir`（目录通常不存在）。

测试：

- `crates/engine/src/helpcode.rs` 的 `custom_tables_are_read_from_the_user_root_first`：状态根优先、资源目录兜底、内置方案不读状态根、相对路径不算。
- `crates/client-core/src/helpcode.rs` 的 `lists_user_tables_before_resource_tables_of_the_same_stem`、`adopts_resource_tables_into_the_user_root_once`。
- `crates/host-api/src/tests.rs` 的 `a_custom_helpcode_table_outlives_the_resource_directory_it_was_placed_in`：合成词库，表只放在 `server/resources/helpcodes/custom`，经 `prepare_host_with_updates` 准备后删掉资源目录里的 `helpcodes`，模拟升级；建会话后 `yh` 下「杨」带 mu，`yhm` 把「杨」提到首位，设置页列表里有 `custom/mine`。去掉 `prepare_host_with_updates` 里的搬家这一步，这条断言失败。`custom_helpcode_schemas_reach_native_presenters_without_the_settings_shell` 覆盖新的 C ABI 参数。
- `platforms/windows/installer/tests/lifecycle.ps1`：清理留下 `helpcodes\custom`，`AdoptCustomHelpcodes` 排在清理和删 `server` 之前。它读脚本文本，不装机；对 develop 上的脚本运行时失败。

没有在 Windows 真机上跑安装器升级，Inno 脚本只经文本检查，robocopy 参数没有实际执行过；Windows 设置窗口和 Win7 设置的改动只在 MinGW 交叉编译里编译。macOS、Linux 的宿主没有装机验收。

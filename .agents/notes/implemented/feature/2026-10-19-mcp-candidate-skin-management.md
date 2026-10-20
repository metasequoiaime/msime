# Agent Note: MCP 管理候选皮肤：按文件给图、选用、读回和删除

Status: implemented

## Problem

MCP 服务器（`crates/mcp-server`）原来只能 `create_candidate_skin`（图片以 base64 内联、可选 `replace`）和 `list_candidate_skins`。代理做完一款皮肤之后还缺三件事：让输入法用上它、读回一款已装的皮肤来改、删掉不要的。另外图片只能内联成 base64：一张几十 KB 的图要由模型逐字转写进调用参数，实际出现过 32K 字符的 base64 转写出错、整包被拒的情况。

## Decision

### 图片按本机文件给出

`create_candidate_skin` 在 `images`（base64）之外接受 `image_files`：键与 `images` 相同，是清单里的包内路径，值是本机文件的绝对路径。两者合起来必须正好是清单引用的那些图，同一个包内路径只能出现在一边。`images` 不再是必填。

读取放在共享的 `candidate_community::install_with_local_files`（`install` 改为它的特例），不在 MCP 里另写：

- 每个文件按 `read_local_image` 读：必须是绝对路径，先过 `msime_path_trust::reject_symlinked_components`（经 `storage::reject_symlink`，系统别名如 macOS 的 `/tmp` 放行一次），再经 `storage::open_private_file_in` 以不跟随符号链接的句柄打开，要求是普通文件（FIFO 以非阻塞方式打开后拒绝），读之前按 1 MiB 拒绝，读时再按同一上限截断。读不出来统一给 `candidate_skin_file_unreadable`，MCP 侧再按路径做几项便宜的检查（不是绝对路径、不存在、是符号链接、不是普通文件）指出是哪一项。
- 读进来的字节和解码后的 base64 走同一个 `load_files`：先核对全部包内路径，再逐张检查单张大小、整包 2 MiB、完整解码和像素与帧数预算，超了就停，不读后面的。
- 全部读完才写暂存目录再换入，所以 `image_files` 可以指向要被替换的那款皮肤自己的文件，这正是 `get_candidate_skin` 之后改清单重装的做法。

服务器只读这些文件，不移动、不改写；写入仍只发生在皮肤目录里。

### 选用皮肤

`get_preferences` 多报告 `global_theme`、`candidate_skin`（浅色槽位）和 `candidate_skin_dark`（深色槽位）；`update_preferences` 接受同名字段，槽位给 `null` 表示取下，仍走 `expected_revision` 的比较交换。规则与设置页一致（见 [浅色、深色两个槽位](2026-10-09-candidate-skin-light-dark-slots.md)）：

- 槽位里的 id 必须是 `catalog::load_package` 接受的皮肤，即 `list_candidate_skins` 列出的那些；内置主题 id 被拒绝并提示改用 `global_theme`。
- 皮肤只在它清单 `base` 的明暗下绘制，所以深色 base（`night`、`ink`、`shuishan`）的皮肤只能放深色槽位，浅色 base（`light`、`paper`）的只能放浅色槽位，`system` 两边都行。放错的直接拒绝，而不是存下一个永远画不出来的选择。
- 没给 `global_theme` 时，选上皮肤会把全局主题换成 `custom`；给了就按给的。内置主题只用 `global_theme` 选。
- 两个槽位都被取空时，`custom_theme.base` 改回 `system`，与 `removeCandidateSkin` 相同。选上皮肤时不像设置页那样把包的 base 写进 `custom_theme.base`：画皮肤时 `resolve` 用的是包自己的 base，`custom.base` 只在这个模式没有可画的包时才用，而且只在属于这个模式时才用，所以不写它结果相同，还保住了用户在自定义主题里自己选的底，`get_preferences` 报告的三项也就足以把选择原样恢复。

### 读回一款皮肤

只读工具 `get_candidate_skin` 返回 `skin.toml` 全文、皮肤文件夹的绝对路径、是否同步过，以及清单引用的每张图片：包内路径、本机绝对路径、字节数、按内容认出的格式（`png`、多于一帧时 `apng`、`jpeg`、`gif`、`webp`，认不出为 `null`）、一帧的宽高和帧数。共享的 `catalog::package_files` 先按扫描规则 `load_package`，图片路径与 `validate_image_dimensions` 共用 `referenced_image_paths`，格式和帧数沿用扫描时的 `png_frames`、`decoded_animation_frames`。清单没引用的文件不列，因为重装只接受清单引用的图。

### 删除

写入工具 `delete_candidate_skin`，与创建一样只在 `--allow-write` 时提供。共享的 `candidate_sync::delete_installed`：

- 只删皮肤目录下直接的、真实的文件夹；`is_external_id` 不认的名字（内置主题、msime-windows 内置外观、`canvas`、`default`、带路径分隔符的）、不存在的、文件和符号链接都拒绝，什么都不删。
- 持有同步运行锁和同步状态文件锁，与设置应用的同步运行互斥；先改名为扫描看不到的 `.removed-<id>` 再删，失败改回。
- 返回删之前它是否在同步状态里，MCP 报告为 `synced`。同步状态不动。

偏好里任一槽位选着这款皮肤（不论当前主题是不是 `custom`）时拒绝删除，提示先用 `update_preferences` 换掉或置 `null`。

## Alternatives considered

- **`source_folder`：给一个装着 `skin.toml` 和图片的文件夹。** 最强的理由是代理只需写一个文件夹、传一个路径，和设置页的文件夹导入（`folder_import::import`）一致。不用它：文件夹里清单没引用的文件怎么办要另立规则（拒绝会让一个多放了草稿图的文件夹失败，忽略又与安装后「只有引用的图」不对称）；清单要从文件读，现有按内联清单给出的 `manifest_problem` 诊断要改走另一条路；而且没法混用 base64，也没法在改清单时直接指向已装皮肤自己的图。`image_files` 让清单照旧内联（它是几 KB 的文本，转写不是问题），只把容易转错的二进制换成路径。
- **MCP 自己读文件、编成 base64 再交给 `install`。** 改动只在 MCP 里，但 MCP 没有 base64 依赖，手写编码违反仓库「不造轮子」的约定；而且 MCP 拿不到 `storage::reject_symlink` 和 path-trust（`msime-client-services` 只私有地引入它们），要么加依赖要么复制一份规则。放进共享安装器后，同一套路径规则和检查只有一处。
- **删除选着的皮肤时顺手把槽位清空。** 一次调用就能删掉，省一步。不用：删除工具不带 `expected_revision`，顺手改偏好会绕开偏好的比较交换，可能覆盖用户刚在设置页做的修改；而且「当前皮肤消失、主题悄悄变了」不是用户要求的效果。拒绝并说明先换掉，两步都各自可见。
- **删除时同时从同步状态里忘掉这款皮肤。** 看起来更「干净」。不行：同步对「本地没有、库里有、状态里也没有」的包的处理是把它当成别处新加的而下载回来（`(None, Some(row), None) => download`），于是删掉的皮肤会在下一次同步时复活。保留状态时走 `(None, Some(row), Some(saved))`：库里那行没变就删掉云端私有的那份，公开的保留，被别的设备改过的才下载回来，与用户在磁盘上删文件夹相同。
- **选用皮肤时像设置页一样把包的 base 写进 `custom_theme.base`。** 与设置页逐字一致。不用的理由见上：对绘制结果没有影响，却会改一个 `get_preferences` 不报告的字段，代理事后无法原样恢复。

## Consequences

- **收益**：代理能完整地做「做一款皮肤、换上、不满意就换回、改了重装、删掉」，图片不再经过模型转写；每一步都沿用设置页和同步已有的规则，没有另立一套。
- **代价**：`image_files` 让 MCP 能读本机任何当前用户能读的普通文件，但读进来的字节必须完整解码成清单所写格式的图片才会被装进皮肤目录，不是图片的内容只得到「读不出来」或「解码失败」，不会出现在结果里；装进去的图只有在用户主动同步时才离开这台机器。
- **Linux 宿主的皮肤目录**：Linux 的 IBus 和 Fcitx5 宿主从运行时选项里的 `candidate_skin_catalog` 读皮肤。`create_candidate_skin`、`delete_candidate_skin` 写成功后和 `update_preferences` 保存时都重新发布这份目录，规则与设置应用的重扫和保存共用 `skin::linux_runtime_options`，见 [Linux 上 MCP 写入皮肤后重新发布皮肤目录](../bug-fix/2026-10-20-linux-mcp-skin-catalog-republish.md)；macOS、Windows 等直接扫皮肤目录的宿主不经这份目录。

## Testing

- `crates/client-services/src/skin/candidate_community/tests.rs`：`local_files_install_the_same_bytes_as_base64_and_may_come_from_the_skin_itself`、`local_files_are_refused_by_the_same_rules_and_leave_the_previous_skin`、`local_files_refuse_symbolic_links_and_fifos`。
- `crates/client-services/src/skin/candidate_sync/tests.rs`：`a_deleted_synced_package_leaves_the_library_on_the_next_run_without_coming_back`（删掉后下一次同步删云端那份、不再下载回来）、`deleting_refuses_reserved_names_missing_skins_and_anything_but_a_folder`。
- `crates/client-services/src/skin/catalog/tests.rs`：`package_files_return_the_manifest_and_each_referenced_image`。
- `crates/mcp-server`：各工具的单元测试，输出 schema 必填字段都要出现在结果里（`assert_required_fields_present`，覆盖 `DeletedSkin`、`SkinFiles`、`SkinFileView` 和 `PreferencesView`），以及 `tests/stdio.rs` 的 `an_agent_manages_candidate_skins` 经 stdio 走完按文件创建、读回重装、选用、换回和删除。

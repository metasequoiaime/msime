# Agent Note: 原生设置窗口社区页用到的 C 接口

Status: implemented

## Problem

原生设置窗口接下来要把社区做成原生页，不再打开共享应用。共享社区页在 Windows 上有三个画廊和举报（`apps/desktop/src/main.tsx` 只在 Windows 上接 `communityResources`）：候选窗皮肤社区经 Tauri 的十四个 `candidate_skin_community_*` 命令（`desktop_candidate_skin_community.rs`）浏览、看详情和预览图、安装、打包检查、补预览图、补素材许可、发布、评分、改可见性和分类、下架、同步；插件社区经七个 `plugin_community_*` 命令（`desktop_plugin_community.rs`）浏览、详情、打包检查、发布、安装、评分、删除；社区词包与回复模板经七个 `community_resource_*` 命令（`community_resources.rs`）；举报经 `community_report`（`desktop_community_report.rs`）。窗口只能经 `msime_client.h` 调 Rust，而 host-api 里只有给自带 HTTPS 栈的移动宿主用的 `msime_client_community_moderation`（只组请求，不发）。

几处判断只写在 Tauri 里：账号错误到 `community_*` 码的对照（`account_helpers::community_error`）、打包检查结果的响应形状、预览图的 `data:` URL、装包后记下来源（同步因此不会把别人的作品传进自己的库）、同步状态文件放在皮肤根目录旁边、插件打包后发布和下载后安装的两步、举报按标签解析原因、词包范围的名字。

## Decision

- 这些判断搬进 client-core，Tauri 改调共享实现：`AccountError::community_code`；`candidate_community::pack_preview` 与 `CandidateSkinPackPreview`、`CandidateSkinPreview::data_url`；`candidate_sync::state_path` 与 `install_downloaded`（安装加 `record_install`）；`plugins::community::pack_preview` 与 `CommunityPluginPackPreview`、服务上的 `publish_installed` 和 `install_publication`；`CommunityReport::from_labels`（`msime_client_community_moderation` 的 `report` 也改用它）；`CommunityResourceScope::from_name`。
- 新导出 `msime_client_community`：请求 `{directory, legacy_directory?, skins_directory?, state_root?, action:{operation, ...}}`，二十九个操作与上面的 Tauri 命令一一对应（`candidate_skin_*`、`plugin_*`、`resource_*`、`report`），参数同名改成 snake_case，返回与 Tauri 相同的 JSON。例外是 #8201 的决定 13（2026-10-17）：`report` 没有登录时用同一账号目录里的匿名会话（`report_with_anonymous`），两个都没有时仍是 `community_unauthorized`，Tauri 的 `community_report` 仍只认登录账号。装包、补图、补许可后返回重新扫描的 `{directory, packages, issues}`，与 Tauri 的 `SkinCatalogResponse` 相同。
- 服务经 `msime_client_account` 里同一个会话对象认证（`session_for` 改成模块内可见），每个账号目录一组服务，放在静态表里，与 `desktop_account::manage` 建的四个服务相同。
- 错误沿用插件页的失败记录（`plugin_response`）：`error` 是码，插件规则失败时另有 `detail`；账号失败是 `community_*`，皮肤包规则失败是 `candidate_skin_*` 或 `storage`，与共享页认的码相同。
- 补预览图的图片用标准 base64 传（Tauri 是字节数组），C++ 那边编码方便；为此 host-api 加了 client-core 已在用的 `base64 = "0.23"`。

## Alternatives considered

- **每个画廊一个导出** — 四个导出的请求外壳（账号目录、旧目录、根目录）完全一样，分开只是多三份重复的解析和文档；一个导出按 `operation` 分派，与 `msime_client_account` 一致。
- **让原生窗口经 `msime_client_community_moderation` 的路子自己发 HTTP** — 那是给自带 HTTPS 栈的移动宿主的；原生窗口没有自己的账号令牌，令牌也不该出 Rust。
- **在 host-api 里照抄 Tauri 的这些判断** — 改动小，但两边会分叉；做法与之前账号、AI 的 C 接口一致。
- **补预览图沿用字节数组** — 一张 256 KiB 的图在 JSON 里是上百万个字符的数字数组，C++ 拼起来又慢又占内存。

## Consequences

- 原生社区页可以完成共享页在 Windows 上能做的全部社区操作，用的是共享应用读的同一份会话、同一个皮肤根目录和插件根目录、同一份同步状态文件。
- 候选皮肤同步仍由显式的 `candidate_skin_sync` 操作提供。共享设置组件的启动同步由[手动整库同步](../bug-fix/2026-10-13-candidate-skin-explicit-library-sync.md)收窄；Windows 原生页的触发时机另见[原生社区页](../feature/2026-10-11-windows-settings-community-tabs.md)。
- 以后改这些判断只改 client-core 一处。

## Verification

- `cargo test --locked -p msime-host-api --lib community` 通过（6 项，不联网）：已装皮肤缺预览和许可时的打包检查、补预览图和许可后重新扫描、私有包不要许可；已装插件的打包检查；不合法请求和缺目录在联网前被拒绝；未登录时要账号的操作以 `community_unauthorized` 结束且没有建出皮肤根目录；举报原有用例。
- `cargo test --locked -p msime-client-core --lib community`（98 项，含新加的 `from_labels` 用例）和 `--lib candidate_sync`（36 项）通过；`cargo test --locked -p msime-host-api --lib account` 通过。
- `cargo build --release --locked -p msime-host-api`、`cargo check --locked -p msime-desktop` 通过，警告都是改动前就有的；Tauri 自己的单元测试在 Windows 上因 unix 专用测试代码编不过，改动前就是这样。
- 设置工程按 `/WX` 编译通过（`settings-test-build.ps1`），没有启动窗口。没有对真实社区服务发过请求：联网路径都是 Tauri 已在用的同一实现。

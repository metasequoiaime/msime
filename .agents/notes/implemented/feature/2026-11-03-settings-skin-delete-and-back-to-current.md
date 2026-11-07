# Agent Note: 「主题」页的皮肤删除和轮播「回到当前」

Status: implemented

## Problem

#10746：macOS 0.57.0 的「主题」页上，社区下载或自己导入的皮肤卡片只有开关、「预览浅色 / 预览深色」和「发布到社区」，没有删除；装了十几款以后，轮播翻走就只能一张张点回正在用的那张。Windows WinUI 的主题页同样是 FlipView 轮播，同样缺这两个按钮。

删除的规则其实已经在共享 Rust 里：`candidate_sync::delete_installed` 只删外部皮肤的真实文件夹，同步状态不动，下一次同步把云端私有那一行一起删掉（见 [MCP 管理候选皮肤](2026-10-19-mcp-candidate-skin-management.md)）。但「偏好的槽位还放着它时不能删」只写在 MCP 那层，宿主也没有经 host-api 拿到删除的入口。

当天定下的迁移期规则是 `packages/ui` 只修缺陷、不加新设置（见 [全部界面改为原生](../../proposed/architecture/2026-11-03-all-native-ui.md)），而反馈者用的 macOS 设置正是 `packages/ui`。

## Decision

为这两个控件在冻结期破例，范围只限「主题」页：外部皮肤卡片上的「删除」和主题轮播的「回到当前」。维护者于 2026-11-06 在审查会话中明确回复「批准这两项有限例外」，批准 #10791 按当前范围继续验收；本次直接授权取代此前等待 #10746 破例评论的条件，不扩展到其他新设置，也不免除真机、前后截图、本地门禁和适用 CI 终态成功要求。授权来源是本次会话，没有另行 GitHub 评论链接。规则先进 Rust，WebView 只画按钮和确认框，Windows WinUI 主题页同批补上。

### 规则在 client-services

- `candidate_sync::delete_unselected(root, state_path, preferences, id)`：不是外部皮肤 id 时先返回 `candidate_skin_package`；读偏好，自定义主题的浅色或深色槽位（`selected_slots`，不看当前全局主题是不是 `custom`）还放着它时返回 `candidate_skin_in_use`，什么都不删、也不改偏好；偏好读不出来时返回 `preferences_unreadable`，同样什么都不删，与皮肤文件夹或同步状态读写失败的 `storage` 分开，免得宿主把「偏好坏了」说成「文件夹删不掉」；其余交给 `delete_installed`，返回它是否同步过。
- MCP 的 `delete_candidate_skin` 改用它，只把拒绝码翻成给代理的说明，槽位名照旧写进提示；`preferences_unreadable` 时再读一次偏好，把原来的 `PreferencesError` 说明交给代理，与改用共享规则之前相同。
- 读偏好和删文件夹之间不持偏好锁。这中间另一个进程把它选上的话，结果与用户自己在磁盘上删掉文件夹相同：宿主找不到皮肤，退回自定义主题的底。

### 宿主入口

- host-api 新增 `msime_client_skin_delete`：`{directory, preferences_directory, id}`，成功时回 `{synced, directory, packages, issues}`（重新扫描的目录与 `msime_client_skin_catalog` 同形），失败时 `error` 是上面的码或 `candidate_skin_not_installed`、`storage`。界面不单独翻 `preferences_unreadable`，与 `storage` 一样说「没能删除，请稍后重试」。放在 `msime_client_skin_import` 旁边，不放进 `msime_client_community`：删除是本机操作，不该要求账号目录和会话。
- Tauri 加一条薄命令 `delete_skin`（`platform/desktop/desktop_skin_delete.rs`），根目录和偏好都取宿主自己的状态，删完走 `rescan_skin_catalog`，Linux 上随之把新目录发布给 IBus 和 Fcitx5 宿主。
- Windows WinUI 设置经 C ABI 调 `msime_client_skin_delete`，皮肤根目录是状态目录下的 `skins`，偏好目录是同一个状态目录，与读写偏好用的是同一个。

### 界面

- `SettingsClient.deleteSkin` 可选，没有它的宿主（HarmonyOS 等）卡片上没有「删除」。`ExternalSkinCard` 只多一个 `onDelete`，按钮与「发布到社区」同样式。
- `skin/use-skin-deletion.tsx` 负责确认框、调用和结果：确认框说明删的是本机文件夹、无法恢复，同步过的私人副本下次同步时一起从云端删掉，公开发布的不受影响。删完 `notifySkinCatalogChanged`，轮播和社区页重扫；`candidate_skin_in_use` 时提示先关掉开关。卡片显示着「使用中」以外的情况（全局主题不是自定义、槽位里却留着它）换一种说法：先打开再关掉开关把它取下，删完再选回原来的主题。开关打开和关掉都会把全局主题写成 `custom`（`skin-page.tsx` 的 onSelect / onDeselect，Windows 的 `use_skin`），照着做会让用户从「夜间」之类的内置主题换到自定义主题并停在那里，提示必须说出这个副作用。不另加「从自定义主题取下」的操作：冻结期只破例两个控件。「使用中」的判断只用来挑说法，不用来拦删除。
- `ThemeCarousel` 导航行末尾加「回到当前」，`show(selectedIndex, true)`；停在在用的那张时不可点，但始终占位，导航行不随翻动左右跳。
- Windows 的文案和「回到当前」可点与否放在 `SkinDeleteModel.h`（不依赖 WinRT，`windows-skin-delete-model` 单测），`main.cpp` 只放接线：卡片上的按钮、确认后在工作线程调用的协程、轮播导航行末尾的按钮和结果那一句。

### 范围

- 不改任何出厂默认值，不新增依赖。
- Win7 设置（`win7/ThemeForm.h`）不在这次范围：它的主题页是列表而不是轮播，维护者这次只点了 WinUI 主题页。
- 手机端（iOS、Android、HarmonyOS 手机）的主题页是键盘皮肤网格，不列外部候选窗皮肤文件夹，不涉及。
- 「回到当前」长在共享的 `ThemeCarousel` 上，凡是画轮播的 `packages/ui` 宿主都有，包括不是手机的 HarmonyOS（`skin-page.tsx` 只在 `harmonyPlatform && mobilePlatform` 时换成网格）；「删除」只出现在提供了 `deleteSkin` 的宿主上（macOS、Linux 的 Tauri 外壳），HarmonyOS 的设置客户端没有它，卡片上不画。

## Alternatives considered

- **不破例，只做 Rust、host-api 和 Windows 原生，macOS 和 Linux 等原生迁移时再做。** 最强的理由是守住当天刚定的冻结，`packages/ui` 不再长。不选：迁移顺序是 Windows → macOS → HarmonyOS → Linux，macOS 原生设置还要几十周，反馈者正用着 macOS；而这两个控件的规则全在 Rust，WebView 只是一个按钮加确认框，迁到原生时照搬，不会成为追赶的负担。维护者已于 2026-11-06 在审查会话批准这一有限例外。
- **把删除做成 `msime_client_community` 的 `candidate_skin_delete` 操作。** 理由是皮肤的安装、发布、同步都在那里，宿主已有调用它的代码。不选：那个入口要账号目录和会话对象，删除本机文件夹不该依赖账号；而且它不接偏好目录，「选着的不能删」还得另加参数。
- **删除选着的皮肤时先把它从槽位取下再删。** 一步完成，用户不用先关开关。不选：理由与 MCP 相同，删除不带偏好修订号，顺手改偏好会绕开保存时的比较交换，可能覆盖别处刚做的修改；页面先改草稿再删又有保存和删除的先后竞争。拒绝并说明怎么做，两步各自可见。
- **页面在卡片「使用中」时直接禁用「删除」。** 少一次确认就知道删不了。不选：页面判断的「使用中」要求全局主题是自定义，与 Rust 的判断（任一槽位放着就算）不同，禁用只能覆盖一部分情况，剩下的还是要靠 Rust 的码；两处判断还会慢慢分叉。按钮始终可点，结论只看 Rust。

## Consequences

- **收益**：macOS、Linux 的共享设置和 Windows 原生设置都能删掉不要的外部皮肤、一下回到在用的那张；MCP 和三个宿主共用一条删除规则，同步语义与 MCP 删除、在磁盘上手动删文件夹完全相同。
- **代价**：`packages/ui` 在冻结期多了一个 hook 和两个按钮；它们迁到原生时要在 macOS、Linux 各做一遍。删除在用的皮肤要多点一次（确认后才被告知先关开关）。
- **Testing**：`candidate_sync` 单测覆盖三种槽位情况的拒绝（偏好和同步状态都不动）、偏好读不出来时什么都不删、未选中时删除并保留同步状态、内置 id 和已删除的码；host-api 单测经 C ABI 走一遍拒绝、删除、重扫回复和非法请求；Tauri 的 `delete_skin_at` 单测；MCP stdio 测试照旧通过（拒绝提示里仍有槽位名），MCP 单测核对偏好读不出来时说的是偏好而不是皮肤文件夹；vitest 覆盖没有删除能力时不画按钮、确认后删除并重扫、取消不删、`candidate_skin_in_use` 的两种说法（卡片没显示「使用中」时要说删完选回原来的主题，显示时不说）和「回到当前」的可点状态与滚动；`windows-skin-delete-model` 覆盖 Windows 的文案和可点判断，但 CI 只在 Windows GNU 交叉编译作业里编译它、不运行（现有 Windows ctest 的 `-R` 过滤都匹配不到 `windows-*-model`，`windows-about-page-model` 等同样如此），断言要在 Windows 本机跑过才算验证。WinUI 页面本身只能在 Windows CI 编译，界面和真机验收留给设备轮。

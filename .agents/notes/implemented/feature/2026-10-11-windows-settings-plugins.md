# Agent Note: Windows 原生设置窗口自己画「插件」页的「我的插件」

Status: implemented

## Problem

原生设置窗口的「插件」是 Shell 页，只有一行「打开」，用 `settings:plugins` 拉起共享 Tauri 应用。按键音、发声方式、音量、上屏音、成就音效、特效样式与强度、连击计数、背景音乐、指令表和短语表的启用、辅助码表、@ 名单和插件的导入删除都只能在那里改，不满足「每个设置都在原生窗口里改」的目标。共享页（`plugins-page.tsx` → `plugins-section.tsx`，视图 `plugin-list-view.tsx`、`plugin-sound-effects-view.tsx`、`plugin-mentions-view.tsx`、`plugin-detail-view.tsx`）的规则散在 TypeScript 里：选用和删除后的偏好（`plugin-preferences.ts`）、行尾标记和缺失的选择（`plugin-catalog-helpers.ts`）、名单校验（`mentionListIssue`）。宿主这边 host-api 早有 `msime_client_plugins`（HarmonyOS 在用），catalog、import、remove、load_mentions、save_mentions 与 Tauri 的 `desktop_plugins.rs` 调同一份 client-core。

## Decision

- 「插件」页改成 Native，画共享页在 Windows 上的「我的插件」：两个入口、按八类分组的包和缺失的选择、导入、无法载入的文件夹，以及从列表在原处打开的「声音与效果」、包详情、缺失选择和「@ 名单」四种视图。Windows 的宿主能力（`host_surface.rs`）除辅助码随版本外全为真，所以只移植这些分支：HarmonyOS 的只闪光文案、「这台设备不播放音效」之类不会出现的分支不画；辅助码表沿用「输入」页辅助码组的条件（本版本提供全拼或双拼）。
- 规则和文案照搬进头文件 `platforms/windows/settings/PluginPageModel.h`，不依赖 Windows 头文件，由 `tests/ui/plugin_page_model.cpp` 核对（C++17 和 C++20 都编过），并加进 `CMakeLists.txt`。`main.cpp` 只管读写文档、调宿主和画控件。
- 不加 Rust：直接用现成的 `msime_client_plugins`。插件目录是数据目录下的 `plugins`（与 Tauri 的 `PluginsState` 相同），内置声音包是 `resolve_state_directory()/sound-packs`，与 Tauri 在 Windows 上取的 `server_state_directory()/sound-packs` 同一顺序；目录不在时传 null，只列已安装的包。「插件」页自己的请求（列目录、读写授权）另带 `plugin_packs`：本程序所在 Server 目录（Program Files 下）的 `plugin-packs`，随桌面安装包分发的官方插件包在那里，见 [官方插件包随三个桌面安装包分发](../architecture/2026-10-13-official-plugin-packs-in-desktop-installers.md)。所有调用都在工作线程上，进入页面时递增的序号作废上一次留下的回答。
- 导入用系统的 `IFileOpenDialog`，与 Tauri 一样分成「导入文件夹」（`FOS_PICKFOLDERS`）和「导入 .zip」两个按钮；对话框在 UI 线程上模态显示，选好的绝对路径交给工作线程上的 import。
- 偏好按点分路径逐项读出、只写变了的项；指令表和短语表的顺序是用户的，原样写入。「声音与效果」里的开关从点下时存着的设置改起，因为滑块拖动后页面不重画，画出来时的那份已经旧了，否则拨一个开关会把刚拖过的音量写回去。
- 名单编辑就地更新提示和「保存名单」，不重建页面；没存的改动在离开视图和页面后保留，进入页面重读名单时不覆盖它（共享页组件一直挂着时的行为）。
- 单词本的「去背单词」在工作线程上先 load 再 set_settings（只换 wordbook），把回答记成背单词的状态，再打开「词库」页的「词书」一行；为此把 `vocabulary_request` 的请求体和回答解析拆成 `vocabulary_body`、`vocabulary_call`、`apply_vocabulary_response`，行为不变。
- 搜索索引除了列表上的入口，也收「声音与效果」视图里的行；选中这样的结果时页面打开那个视图并聚焦那一行。

## Alternatives considered

- **把这些规则移进 client-core，由 host-api 回答「标记」「缺失的选择」「选用后的偏好」** — 最强的理由是各平台只有一份实现，React 和原生窗口不会走样。没有这样做：共享 React 页今天就是这些规则的实现者，要让它改调 Rust，得同时改 Tauri 命令、共享页的数据流和现有的 vitest，范围远超这一页；只给原生窗口用 Rust 版就会有 TypeScript 和 Rust 两份。与候选字体、翻译服务、打字统计的做法一致，在头文件里逐条移植并配主机单测，日后统一时可以照测试换实现。
- **保留 Shell 页，只把「声音与效果」的开关画成原生** — 改动小一半；但导入、删除、指令表启用、@ 名单仍要拉起共享应用，不满足目标。
- **导入只给一个按钮，用同一个对话框既选文件夹又选文件** — Windows 的通用对话框做不到两者兼选（`FOS_PICKFOLDERS` 只能选文件夹），共享页本来就是两个按钮。

## Consequences

- 原生窗口不再为「插件」拉起共享应用；`SettingsNavigation.h` 里 `plugins` 是 Native，导航测试和 README 同步，`shell_page_description` 里的插件说明删除。
- 「社区插件」标签（浏览、安装、发布社区插件）现在不在原生窗口里，也不再能从这一页打开；它由后续的一项补上。在那之前用户在原生窗口里看不到社区插件。
- 共享页的规则改了，`PluginPageModel.h` 要跟着改；头文件开头列了它对应的 TypeScript 文件。脚本插件的分组、详情和授权对话框后来也加进了这一页，见 [Windows 授权页](../architecture/2026-10-12-windows-script-grants-page.md)。
- 「我的插件」列表的布局（两个入口、按类分组的行、页底的「导入插件」）后来换成 macOS 的插件库，见 [Windows 设置的「我的插件」改成 macOS 的插件库](2026-10-20-windows-settings-plugin-library.md)；各视图、选用规则和宿主调用仍按本篇。

## Verification

- `tests/ui/plugin_page_model.cpp`（选用、删除、标记、缺失的选择、使用按钮、出错说明、名单校验）和 `tests/ui/settings_navigation.cpp` 用 `cl /W4 /WX /std:c++17` 和 `/std:c++20` 单独编译运行通过。
- 设置工程按 `/WX` 编译通过（`settings-test-build.ps1` 输出 `SETTINGS_BUILD_OK`），链接的是现有的 `target\release\msime_host_api.dll.lib`，没有改 Rust。
- 没有启动设置窗口：列表和各视图的实际样子、文件夹和 .zip 对话框、导入删除后的重读、焦点落点、名单输入时的提示、「去背单词」的跳转都没有在运行中的窗口里检查过；`-Wall -Wextra -Werror` 的 GCC/Clang 编译也没有在本机跑。

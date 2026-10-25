# Agent Note: 手机底部第一格改名「首页」，根页以应用名作标题

Status: implemented

## Problem

[全平台设计稿](2026-10-07-ios-harmony-all-platform-design.md)把手机底部四格定为「设置 / 社区 / 统计 / 我的」，[社区回到第二格](2026-10-16-mobile-community-bottom-tab.md)后三端都是这四格。维护者认为第一格叫「设置」别扭：国内 App 习惯把「设置」放在「我的」里，底栏上「设置」和「我的」并排让人分不清各管什么；第一格又是打开 App 的落地页，顶上是启用键盘的状态卡片，更像首页。

只改标签名不够。根页的大标题也写「设置」，标签写「首页」、标题写「设置」会对不上；大标题改成「首页」两个字又说明不了这一页是什么。状态卡片首行原本就写「水杉输入法」，如果大标题改成应用名，两处紧挨着重复。

## Decision

- iOS、Android 与 HarmonyOS 手机（共享触屏壳）底部第一格叫「首页」，图标是房子：iOS 用 SF Symbol `house.fill`，Android 用 Material Symbols `home`（`ic_ms_home`），鸿蒙用 Fluent `home_20_regular`，其他走蒙版图标的触屏宿主用 `assets/home.svg`。
- 首页根页的大标题和滚动后的紧凑标题是应用名：iOS 和共享 UI 写「水杉输入法」（`mobilePageTitle("home")`，iPad 分栏的侧栏标题同源），Android 取 `@string/app_name`，所以拼音、五笔等版本显示各自的名字。
- 状态卡片不再写应用名。共享 UI 与 Android 的首行写设置状态（「已启用」「尚未完成设置」「检查中」），下一行写当前方案；宿主报告不了状态时（共享 UI 没有设置客户端，iOS 宿主 app 判断不了键盘是否已启用）首行只写当前方案。
- 搜索框仍写「搜索设置」，设置项照旧是首页上的分组行；键盘功能面板里打开 App 的「设置」工具不改名。内部的页面 id（`home`、`tab_settings`、`AppNavigation.Tab.settings`）不变。

## Alternatives considered

- 保留「设置」：与设计稿一致，页面内容确实全是设置入口，齿轮加「设置」一眼看出能调键盘。维护者认为与「我的」并排的别扭感更要紧，而且这一页兼作落地页。
- 大标题去掉，让卡片里的「水杉输入法」充当页头：改动最小，但首页和其他三个标签页的版式会不一致，滚动后的紧凑标题也没有对应的大标题。
- 大标题写「首页」：标签与标题一致、卡片不动，但「首页」作标题不传达任何信息。

## Consequences

底栏不再出现与「我的」语义重叠的「设置」，首页顶部是品牌名，卡片专门说明设置状态，三处信息不再重复。代价是偏离设计稿的标签名，以后对照设计稿时第一格要按这篇读。iOS 的首页卡片首行从「水杉输入法」变成当前方案，信息量没有减少，但少了品牌的视觉锚点，由大标题承担。

依赖标签名的测试一并改了：共享 UI 的 vitest、iOS UI 测试（标签按钮找「首页」，导航栏找「水杉输入法」）、Android 的 `ChatNavigationDeviceSmoke`（根页标题找「水杉输入法」）。

## Verification

- 共享 UI：`apps/desktop` 全量 vitest 5414 项中只有 5 项失败，都是 develop 上已有的方案顺序与 14/18 键用例（干净的 `origin/develop` 上同样失败），与本改动无关。
- Android：`check-host.sh` 通过（JVM 冒烟、设备源码编译、资源检查）。
- iOS：`MSIMEClientUITests` 的模拟器 `build-for-testing`（编 App、键盘扩展和 UI 测试）通过；UI 测试只改了查找用的名字，没有在模拟器上运行。
- 没有在三端设备上看过实际画面。

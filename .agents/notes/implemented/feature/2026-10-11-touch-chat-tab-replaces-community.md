# Agent Note: 触屏底部标签栏的第二格从「社区」换成「对话」

Status: implemented

底部第二格先后被 [AI 对话改为二级页面](2026-10-16-mobile-chat-secondary-page.md) 与 [触屏宿主底部第二格恢复为社区](2026-10-16-mobile-community-bottom-tab.md) 取代。

## Problem

[触屏设置去掉「社区」标签页](2026-10-11-touch-community-merged-into-feature-pages.md) 把社区内容并进了皮肤、词库和 AI 辅助页，同时把底部标签栏从四格删成了三格（设置 / 统计 / 我的）。仓库负责人纠正：底部必须保持四个标签页，「让你换，不是让你删」。空出来的那一格要放一个值得常驻底部的功能。

共享设置界面里现成的 AI 对话页（`chat`，`ChatPage`）原来藏在「AI 辅助」页最后一组的一行里，要先进「设置」、再进「AI 辅助」、滚到底才能打开。它是这套设置界面里唯一一个用户会反复回来用、而不是设一次就走的页面，适合放在底部。

## Decision

- 触屏宿主（`mobilePlatform` 为真：鸿蒙手机，以及同样渲染这套外壳的 Android、iOS Tauri 界面和 iPad）的底部标签页是 设置 / 对话 / 统计 / 我的，「对话」在原来「社区」的第二格。`mobilePrimaryPageIds` 是 `["home", "chat", "typing-statistics", "account"]`，`MobilePrimaryPageId` 和 `mobileLastPageByTab` 随之加上 `chat`，所以对话也有自己的标签页记忆，`mobileTabForPage("chat")` 是 `chat`。
- 点「对话」直接打开现有的 `chat` 页，不新写对话功能。鸿蒙、Android、iOS 三个触屏宿主都提供对话客户端（`apps/harmony/src/main.tsx` 的 `chatClient`，`apps/desktop/src/core/mobile-host-services.ts` 的 `accountChat`），`settingsPageProjections` 只在 `hasChat` 时提供这一页；宿主没有对话客户端时标签栏就没有这一格，`mobileTabBar` 按标签个数均分列宽，三格也不会挤在左边。
- 标签名是「对话」（`mobileTabTitle`），页面标题仍是「AI 对话」，与页面自己的 `aria-label` 一致。鸿蒙手机画 Fluent 的 `chat` 气泡（`harmonyTabIcon`）；设计稿的图标表里没有对话图标，路径原样取自 `@fluentui/svg-icons@1.1.344` 的 `chat_20_regular.svg`，来源写在 `fluent-icons.tsx` 和 `packages/ui/UPSTREAM.md`。其他触屏宿主用蒙版图标，`mobileTabIcon` 给 `chat` 换成 `feedback.svg` 的对话气泡：注册表里 `chat` 页的图标是问号（`help.svg`），它原本只从一行进入、用不到图标，放上标签栏会被看成「帮助」。
- 在触屏宿主上 `chat` 是标签页的根页面，不再是「AI 辅助」的子页面：外壳算 `navigationPage` 时，触屏宿主的标签页根页面不查 `subPageParents`，所以鸿蒙手机上它不滑入、不带返回按钮，Android、iOS 上也没有「‹ AI 辅助」返回链接。`subPageParents.chat = "ai"` 保留给桌面。
- 进入对话的入口由标签页承担：触屏宿主的「AI 辅助」页不再列「AI 对话」那一组（`AiSettingsPage` 按 `mobilePlatform` 不渲染那个 `SubPageEntries`）。桌面的「AI 辅助」页照旧保留这一行。其他指向 `chat` 的地方在触屏上本来就不存在第二个入口：触屏首页的搜索只搜「设置」根页的各行（`chat` 不在导航分组里），触屏首页的 `onOpenChat` 只供状态卡片上的「试用键盘」切到这个标签（见 [触屏首页的「试用键盘」切到「对话」标签](2026-10-11-touch-try-keyboard-is-the-chat-tab.md)），不另列一行，原生宿主也没有路由到 `chat` 的深链接；`window.history` 里旧的 `page: "chat"` 记录照常恢复，落在「对话」标签页上。

## Alternatives considered

- **保持三个标签页（上一篇的做法）。** 最强的理由是社区内容已经并进功能页，没有现成的东西需要一个底部入口，少一格也更简洁。不用它：仓库负责人明确要求底部保持四个标签页，删掉是误解了「去掉社区模块」的意思——要换的是那一格放什么，不是格数。
- **「对话」标签页和「AI 辅助」里的那一行并存。** 理由是改动更小，习惯从「AI 辅助」进入的人不受影响。不用它：同一个页面两个入口，而且从那一行进入时页面还会被当成「AI 辅助」的子页面（带返回、点亮「设置」标签），和从标签页进入时的样子不一致；标签页常驻底部，那一行已经没有存在的理由。
- **把 `chat` 从 `subPageParents` 里整个删掉。** 理由是一处改动就让它在所有宿主上都没有上级页面。不用它：桌面没有底部标签栏，从「AI 辅助」进入的对话页仍需要「‹ AI 辅助」这条回去的路；只在触屏宿主的标签页根页面上跳过 `subPageParents`，桌面不变。
- **用图标集里已有的图标（`person_feedback`、`text_quote`）而不是补一个 `chat`。** 理由是不碰「路径都原样复制自设计稿图标表」这条约定。不用它：`person_feedback` 已经是「我的」里「帮助与反馈」那一行的图标，`text_quote` 读起来是引用，都不是对话；`chat` 来自同一个官方图标库、同样是 20px Regular，渲染风格与其他标签一致，例外在注释和 `UPSTREAM.md` 里写明。

## Consequences

- **收益**：底部仍是四个标签页，AI 对话从三层深的一行变成一点就到。上一篇里社区并进功能页、「浏览更多」子视图这些改动不受影响。
- **常驻标签的行为**：触屏上第一次打开「对话」后 `ChatPage` 一直挂着，切到别的标签只用 `hidden` 隐藏，对话记录和进行中的回复都不丢；点标签进来不自动聚焦输入框，不弹键盘，点输入框才弹，与 Android 的对话标签一致。每次重新显示（`active`）都重读模型列表，在「我的」里登录或退出后回来看到的是新状态。
- **代价**：换账号后旧对话仍留在页面上（`ChatPage` 拿不到账号身份），Android 那边会清空；要一致的话得把账号变化传进对话页。
- **已知上限**：只覆盖 `packages/ui` 的共享设置界面。Android、iOS 原生应用自己的底部导航是否也换成「对话」由各自的改动决定。没有在 HarmonyOS 模拟器或真机上看过新图标和四格标签栏。

## Verification

- `apps/desktop/tests/core/mobile-tab-bar.test.tsx`：鸿蒙手机的标签栏是「设置 / 对话 / 统计 / 我的」，Fluent 图标依次是 `settings`、`chat`、`data_bar_vertical`、`person`；Android 的四个蒙版图标各不相同，「对话」画的是对话气泡；点「对话」打开 AI 对话页、历史记录是 `page: "chat"`、「对话」高亮而「设置」不高亮、没有返回按钮，去「统计」再回来仍是对话；触屏宿主的「AI 辅助」页没有「AI 对话」那一行；macOS 的「AI 辅助」页仍有这一行并能打开对话。
- `apps/desktop/tests/settings/mobile-app-chrome.test.tsx`：鸿蒙手机上「对话」带「AI 对话」大标题、没有返回按钮；Android 直接打开 `chat` 时没有「返回AI 辅助」链接，「对话」标签高亮。
- `apps/desktop/tests/settings/settings-page-projections.test.ts`、`settings-navigation-helpers.test.ts`、`mobile-tab-helpers.test.ts`、`settings.test.tsx`：标签页顺序、`mobileTabForPage("chat")`、没有对话客户端时只剩三格、`chat` 不在「设置」根页的行里也不在 iPad 侧栏里、标签名和图标映射。

## 对话页的触屏样子

对话做成底部标签后，原来的页面直接露在用户面前：标题下一行「边聊天，边试键盘 / 使用共享账号与 EveryAPI 对话」和「刷新模型」按钮，未登录时一个红框报错加一个按钮，中间一大块白卡片，方框输入框和两个文字按钮，底下还有一行「最多保留最近 14 条消息」。触屏上（`touch`）改成和「试用键盘」同一套样子，桌面不变：

- 顶部一行小胶囊：左边模型选择（原生 select 套成胶囊），右边「新对话」。不再有「刷新模型」，每次回到标签自动重读。
- 对话区不再套卡片，气泡直接放在页面底色上；自己发的是强调色气泡靠右，回复按 Markdown 画（关掉 HTML、链接和图片：设置页在 WebView 里，点链接会把整个设置页导航走）。
- 这一页是用来试键盘的：空对话时正中一个气泡图标、「打几个字试试键盘」和一句说明（发给 AI，看看候选、皮肤和手感），不给「写生日祝福」这类代写的开头，那些开头点一下就把字填好，用户反而没打字。未登录时同一个位置写「登录后边聊边试键盘」和一个「登录使用 AI」主按钮，不画红框。其他错误是对话里一行次要文字加「重试」。
- 底部是圆角输入框（最多长到四行）加圆形发送键，发送中变成停止键。页面撑满内容区，输入栏贴着底部标签栏。

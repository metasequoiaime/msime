# Agent Note: Windows「账号与同步」页的版式对齐 macOS

Status: implemented

## Problem

[Windows 原生设置窗口自己画「账号与同步」页](2026-10-11-windows-settings-account-page.md)把共享账号页的流程和文案搬进了原生窗口，版式却沿用设置页的标签、说明、操作三栏：未登录时是两张左对齐的卡片（52 DIP 头像配「水杉账号」和一句 Windows 自己的说明；「登录」组每种方式一行，行尾一个按钮；另有「登录状态」组放「刷新」「清除」），验证码表单也是 add_row 的行加右对齐按钮。登录后资料卡用 52 DIP 头像和 17 DIP 昵称；「编辑资料」里的账号信息是标签在左、值在右的表格；「我的内容」每行是插件页那种带图标和描边的独立卡片，「云剪贴板」是行尾按钮；「绑定 QQ 群助手」不管有没有绑定都显示绑定码表单；页面底部每个状态下都有一段共享页没有的脚注。macOS 设置用的是共享 React 账号页，用户要求 Windows 以 macOS 为准迁移（差距清单 G3.16–G3.21，批 11）。

## Decision

数值和文案取自共享源码 `packages/ui/src/account/account-page.tsx`（桌面那一支）、`account-style.ts`、`account-verification-form.tsx`、`account-identity-details.tsx`、`qq-link-card.tsx` 和 `settings/document-style.ts` 的 `linkRow`，都落在 `platforms/windows/settings/main.cpp`，文案和可测的规则放进 `AccountPageModel.h`：

- 未登录时整页是一张居中的卡片（`account.signIn`）：8 DIP 圆角、左右 24 上下 32 的内边距；84 DIP 的「杉」头像（装饰，不进无障碍树），17 DIP 半粗体标题「欢迎来到水杉」和 12 DIP 灰色说明「登录后在设备之间同步设置和词库，还可以发布你的候选窗口皮肤」（宿主有 `settingsSync` 时的那句）。下面是 `MaxWidth` 320、拉伸对齐因而居中的一列按钮（`account.signInBody`）：`account_wide_button` 铺满整列、文字居中、8 DIP 圆角、14,10 内边距、中等字重。等浏览器时 Google 按钮写「正在等待浏览器完成 Google 登录…」，下面是「取消 Google 登录」。一种登录方式都没有时是一句居中说明，分「读到了但一个都没开」「正在重读」「读失败」三种（`providers_unavailable`，靠新的 `account_providers_loaded_` 区分，对应共享页的 `providersLoaded`）。与共享页的 `refreshProviders` 一样，点「刷新登录方式」时先把它清成 false，请求期间说正在重读、失败时说读失败，只有成功才置回 true；否则一个都没开之后再刷新，会一直停在「当前没有可用的验证码登录方式」那句。
- 恢复动作「刷新登录方式」「清除失效登录状态」是卡片底部居中的灰色小链接（`account.signInFooter`），出现条件不变。`account_link` 改成共享页 `account.link` 的样子：12 DIP 灰字，内边距清成 0（`p-0`；模板默认的 `ButtonPadding` 会让底部两个链接之间远大于 `gap-x-4` 的 16 DIP），悬停和按下换成正文色、没有底色，颜色写进 `HyperlinkButtonForeground*` 这组模板资源；共享页悬停时还加下划线（`not-disabled:hover:underline`），模板里没有对应资源，所以内容是一个 `TextBlock`，可用时 `PointerEntered` 加 `TextDecorations::Underline`、`PointerExited` 去掉，读屏名字用 `AutomationProperties.Name` 写上原文。编辑资料里的账号 ID 是强调色的 `copyId`，不再借用 `account_link`，单独建一个 `HyperlinkButton`。
- 选了邮箱或手机号后标题换成「邮箱登录」「手机号登录」，说明换成验证码那句，按钮列换成整宽的验证码表单：12 DIP 灰色标签在上、隔 7 DIP 的输入框（最小高 38、8 DIP 圆角），「获取验证码」没发过时是主按钮、发过后是普通按钮，「6 位验证码」，主按钮「登录」，居中提示和「取消」。就地更新按钮和收起验证码一栏的做法不变。发过验证码后改号码时，共享页的 `updateTarget` 清掉 `challenge`，按钮马上变回 `account.submit`；Windows 这里不重画页面（输入框要留着焦点），所以在原位把「获取验证码」换成一个新画的主按钮。不在原按钮上调 `make_primary`：往已经显示出来的按钮上插的模板颜色资源要等下一次视觉状态变化才生效。
- 资料卡片整张是按钮（`make_row_button` 伸到卡片边缘，左右 24 上下 20）：58 DIP 头像，昵称 14 DIP 半粗体，隔 7 DIP 是 12 DIP 灰色邮箱，行尾 24 DIP 的「›」。
- 「编辑资料」仍是 `ContentDialog`。账号信息改成两列小卡片（8 DIP 圆角、`palette_.subtle` 底、12 内边距，12 DIP 灰色标签在上，值在下隔 5 DIP）；没有邮箱时那格收起，后两格在刷新时往前挪，不留空位。「保存修改」照 `make_primary` 的颜色：对话框的默认按钮套 `AccentButtonStyle`，把 `AccentButton*` 资源放在对话框上。
- 「云剪贴板」和「我的内容」四行改用 `link_row`（共享页的 `MeRow`），行尾「›」。
- 「绑定 QQ 群助手」加 `qq_link_adding_`（共享卡片的 `adding`）：有绑定时组名是「QQ 群助手」、说明换成已绑定那句（与表单说明一样是 `account.note` 的 12 DIP 灰字）、表单收起，列表下面是「绑定其他 QQ」，点了才展开表单，表单里多一个「取消」；绑定成功后收起表单、说「绑定成功。」。绑定列表读到之前不画表单，读列表时卡片也算在忙（「正在处理…」），读失败时给「重试」。标题和表单是否出现是 `qq_link_title`、`qq_link_form_shown`。
- 页底脚注「皮肤设计和打字统计保存在本机……」不再画在页面底部，改成「账号」组的组注，只在登录后出现。

## Alternatives considered

- **登录卡片仍用 `add_group`，只换内容**：能复用组的边距和卡片样式。没有采用：`add_group` 的行面板有 16 DIP 的左右缩进和行分隔线约定，居中的一列按钮和 84 DIP 头像要层层抵消；共享页这里本来就是一张 `section` 卡片，不是分组。
- **脚注直接删掉**：共享页没有这段文字。没有采用：这句说明本机数据不会因登录上传，迁移计划定的是挪进组注；放在「账号」组里只在登录后出现，未登录的卡片与 macOS 一致。
- **编辑资料改成自绘模态框，右上角放「关闭」**：最接近共享页。没有采用：`ContentDialog` 的标题区放不下按钮，自绘要自己处理焦点、Esc 和读屏，底部的「取消」作用相同。
- **主按钮颜色通过 `PrimaryButtonStyle` 换一个自建样式**：可以完全控制按钮外观。没有采用：这个工程不经 XAML 编译器，在 C++ 里拼样式比给对话框插几项资源复杂，本机又编不了 WinUI。

## Consequences

- **收益**：未登录页、资料卡、账号信息、跳转行和 QQ 卡片的版式、字号和文案与 macOS 相同；Windows 独有的「水杉账号」标题、「登录状态」组和页底脚注不再出现。
- **代价**：`account_link` 变灰影响页内所有小链接（「取消」「取消 Google 登录」「移除头像」和恢复动作），与共享页一致，但与窗口其他页的强调色链接不同。读绑定列表期间 QQ 卡片的按钮不可点，网络慢时多一段等待。
- **未验证**：本机编不了 WinUI，`main.cpp` 只由 PR 的 Windows CI 编译；没有在运行中的窗口里看过新版式，`HyperlinkButton` 和对话框默认按钮是否按插入的资源取色要看 CI 截图或真机。`MaxWidth` 加拉伸对齐时居中是 XAML 的布局规则，同样等截图确认。
- **重访信号**：共享 `account-style.ts` 的 `signIn*`、`provider`、`submit`、`link`、`details` 改了数值，或 `qq-link-card.tsx` 的状态流转变了。

## Verification

- `tests/ui/account_page_model.cpp` 新增标题、说明、三种无登录方式说法和 QQ 卡片标题、表单出现条件的检查，在 macOS 上用 clang++ `-Wall -Wextra -Werror` 编译运行通过，`x86_64-w64-mingw32-g++ -fsyntax-only` 通过。

# Agent Note: iOS 底部弹窗的高度贴合内容

Status: implemented

## Problem

iOS App 里 29 处 `.sheet` 只有 3 处设了高度，其余都按系统默认弹成整屏。添加常用语、编辑词条、选服务商、举报、保存皮肤这类弹窗只有一两个输入框或几行选项，却占满整屏，下面空一大片；里面的 `Form` 还是系统灰色，不是 App 的页面底色和卡片。维护者在真机上看到后要求全部修掉。系统分享面板包在 sheet 里时同样被撑成整屏。

## Decision

`shared/backend-ui/core/FittedSheet.swift` 提供 `fittedSheet()`，加在弹窗里的 `Form`、`List` 或 `ScrollView` 上：iOS 18 起用 `onScrollGeometryChange` 读出滚动内容的高度加上方留白，底部最多计 34pt（键盘弹起时底部留白会加上键盘高度，按它算弹窗会跟着键盘变高），弹窗停在这个高度，往上拉可以到整屏；iOS 17 读不到滚动内容的尺寸，退回半屏起；macOS 上什么也不做。它放在和 macOS 共用的 `backend-ui` 里，是因为共用的 `CloudDictionaryEditor` 也是 iOS 上的底部弹窗，而 `backend-ui` 看不到 App 的 DesignKit。

所有内容不多的弹窗都用它，并在 App 自己的表单上加 `.designPage()`：添加常用语、选择服务商、云词库编辑、个人词库编辑与导入、社区举报、发布皮肤与作品、保存我的皮肤、登录后发布、验证码登录、云剪贴板添加内容、关于统计，以及键盘深链弹出的精简录音页（同一个页面被推入时不加）。两个系统分享面板设 `[.medium, .large]`。照片选择器、皮肤试用（下面要留出键盘）、AI 生成皮肤保持整屏；登录弹窗早就自己量内容高度，不变。

以后新加的底部弹窗，内容只有几行时同样加 `fittedSheet()`。

## Alternatives considered

- **统一 `[.medium, .large]`**：一行就能加，iOS 17 也一样。但一两个输入框的弹窗停在半屏仍然空着半截，登录弹窗当初正是为此改成按内容量高度的。
- **把这些表单改成 `VStack` 再量高度**：iOS 17 也能贴合，但要逐个重写表单，丢掉 `Form` 的分组、键盘避让和读屏行为，改动面远大于问题本身。
- **把修饰器放进 App 的 DesignKit**：命名与 App 其他设计组件一致，但共用的 `CloudDictionaryEditor` 用不到它，只能在调用处从外层加，而读滚动尺寸的修饰器要加在滚动视图本身上。

## Consequences

小弹窗的高度跟着内容走，内容变化时（比如出现一行错误）跟着调整，用户拉到整屏后不再自动收回。弹窗刚出现的一瞬间以半屏打开，量到内容高度后再落到贴合的高度，会有一次很短的调整。iOS 17 仍是半屏起。

## Verification

真机（iPhone 17，iOS 27.0.1）Release 签名构建通过并安装；`platforms/macos/scripts/typecheck_backend_swift.sh` 通过。各弹窗在真机上的实际高度还要维护者看过确认，没有自动化测试覆盖弹窗高度。

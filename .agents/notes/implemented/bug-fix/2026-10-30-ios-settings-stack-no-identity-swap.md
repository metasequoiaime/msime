# Agent Note: iOS 设置栈不再靠换栈回到根页面

Status: implemented

## Problem

iPhone XR（iOS 18.7.10）上：冷启动后进「皮肤」，再点一次「首页」回到根页面，此后根页面的各行（表达、输入、插件、键盘）点了都不推入页面，直到重启 App；其他标签不受影响。重选「首页」和指名设置页的深链接都靠替换 `AppNavigation.settingsRoot` 给手机设置栈的 `NavigationStack` 换一个 `.id`，让 SwiftUI 重建一条新栈回到根页面。

真机日志确认了失败点：换栈后 UIKit 层级里是一个新的 `UIKitNavigationController`（stack=1，在窗口里），新的 `SettingsView` 出现了，点行时 `selectedPage` 也确实写成了新页面，但 `navigationDestination(item:)` 不再推入。换栈时旧栈上推入过页面（stack=2）。深链接走同一条换栈路径，同样坏：从「统计」打开 `msime://settings/ai`，`selectedPage` 写成了 `.ai`，屏幕却停在根页面。iOS 26.5 和 27.0 模拟器上同样的步骤都正常，所以模拟器测不出来。之前猜测的「iOS 18 标签栏自己把旧栈弹回根页面、屏幕上留着脱离 SwiftUI 的旧栈」不成立：两个系统版本上标签的视图控制器都是 `TabHostingController`，导航控制器只是它的子控制器，换栈后在窗口里的也确实是新栈。

## Decision

手机设置栈的身份固定不变，回到根页面只改状态，不换栈：

- 重选「首页」时 `AppNavigation.select` 给 `settingsReselection` 加一，`SettingsView` 看到它变了就把 `selectedPage` 置空，根页面推入的页面和它上面更深的页面一起弹出；`settingsChat` 照旧置为 `false`。
- 深链接仍由路由器（`SettingsRouter.settingsPage`）保存待消费的请求。`SettingsView.consumeRequestedPage` 发现栈上已经推入了页面时先把 `selectedPage` 置空、保留请求，等根页面再次出现（`onAppear`）时再消费、推入要的页面；栈在根页面时直接推入。这样「同一页面已经开着、上面还压着更深一层」时也会落到链接指名的那一页。
- `SettingsView` 不再持有创建时的根栈标识，手机 `NavigationStack` 去掉 `.id(navigation.settingsRoot)`。`settingsRoot` 只剩 iPad 使用：深链接时替换它重建分栏；重选时 iPad 用 `.id(settingsRoot).id(settingsReselection)` 照旧整体重建，行为与改动前相同。

这部分取代了 [iOS 设置深链接隔离导航栈代次](2026-10-20-ios-settings-route-stack-generation.md) 中「手机设置视图持有创建时的根栈标识」「保留现有按深链接重建设置栈的行为」两点：那里的根栈标识只是为了在换栈时分辨新旧两条栈，不换栈以后就用不上了。路由器只存请求、栈内选中页归视图所有这一归属仍然成立。也部分取代 [触屏 AI 对话二级页](../feature/2026-10-16-mobile-chat-secondary-page.md) 中「设置根标识直接作用于导航栈」的做法，该篇其余决定不变。

## Alternatives considered

- **只改重选，深链接继续换栈。** 改动最小，换栈对深链接的作用（越过保留下来的更深页面）也不用重新实现；但真机上深链接同样落在根页面、之后同样推不进页面，问题只是换了个入口。
- **改用 `NavigationPath` 驱动整条栈，回根时清空路径。** 能把整条栈的状态集中到一处；但各设置页内部更深一层的页面是用 `NavigationLink(destination:)` 和 `navigationDestination(isPresented:)` 推入的，不在路径里，清空路径管不到它们，要做到就得改写所有设置页的导航方式，范围远超这次修复。
- **深链接时把 `selectedPage` 置空、同一轮更新里再写成要的页面。** 不用等根页面出现；但两次写入会合并成一次，目标页面与原来那页相同时值没有变化，用户仍停在更深一层，正是当初引入换栈要解决的问题。

## Consequences

- 收益：iOS 18 上重选「首页」和指名设置页的深链接以后，根页面各行照常推入页面，不必重启 App；iOS 26 / 27 行为不变。重选时只弹栈不重建，根页面的滚动位置、搜索框等视图状态保留下来，不再随新栈重置。
- 代价：栈上已有页面时，深链接先弹回根页面、根页面出现后再推入，能看到一次返回再进入的转场，而不是直接换成新栈。消费请求依赖根页面的 `onAppear`，以后若把根页面包进不会触发 `onAppear` 的容器，深链接会一直等着。
- iPad 分栏仍靠换 `.id` 重建，iOS 18 上的 iPad 没有设备验证。手边只有 iOS 26 / 27 模拟器，同样的换栈在那里不出问题，所以 iPad 是否受同一缺陷影响还不知道。

## Verification

- iOS 18.7.10 iPhone XR，未修复的构建：冷启动 →「皮肤」→ 重选「首页」→「表达」不推入，之后「键盘」也不推入。带日志的构建日志显示换栈后新栈在窗口里、`selectedPage=ai` 已写入，但没有推入。
- 同一台 XR，修复后的构建：上面的步骤推入「表达」；「表达 → 标点」两层时重选回到根页面，之后「词库」「键盘」都能推入；从「统计」打开 `msime://settings/ai` 落到「表达」，返回后「键盘」能推入；在「表达 → 标点」上切到「统计」再打开同一链接，落在「表达」而不是「标点」；从 AI 对话打开 `msime://settings/input` 落到「输入」，返回后「插件」能推入。
- iOS 26.5 iPhone 17 模拟器：新增的 `OnboardingUITests.testSettingsRowsStillPushAfterReselectingHome`（两轮重选后点行、返回后点行、深链接后点行），以及已有的 `testMainTabsKeepIndependentNavigation`、`testSettingsStackSurvivesSwitchingTabs`、`testSettingsDeepLinkLandsOnTheNamedPageOverAKeptStack`、`testSettingsDeepLinkFromTryoutOpensInputSettings`、`testChatIsSecondaryAndReturnsToSettings`，修复后全部通过。新用例在 iOS 26 / 27 模拟器上修复前同样通过，它的失败只能在 iOS 18 设备上复现；iOS 18 模拟器运行时 Xcode 27.1 已无法下载。

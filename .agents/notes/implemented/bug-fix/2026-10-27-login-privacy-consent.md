# Agent Note: 登录前勾选同意《用户协议》和《隐私政策》

Status: implemented

## Problem

HarmonyOS 设置应用（`app.msime.hmos`）在 AppGallery 审核中因账号登录页被驳回两条：

- 应用功能：「登录页面，用户协议点击无响应。」
- 用户隐私：「隐私政策无勾选框，且使用'登录即代表同意'字样。」要求提供默认不勾的空白复选框，由用户自愿、明确勾选（明示同意）。

鸿蒙手机和平板的登录是共享 React 设置页里的 `LoginSheet`（`packages/ui/src/account/login-sheet.tsx`），底部一行写「登录即表示同意《用户协议》和《隐私政策》」，只有《隐私政策》是按钮。《用户协议》是普通文字，不是漏接了点击事件，而是根本没有可以打开的文档：仓库、`msime.app`（`/terms/`、`/agreement/` 都是 404）和 msime-web 里都没有用户协议，Android「关于」页的注释也写着「用户协议还没有确定的页面，不显示」。鸿蒙 2in1 报 `mobile_settings: false`，走的是桌面式账号页（macOS、Linux 共用同一张卡片），那里连一行同意文字都没有，同样不满足明示同意。

## Decision

- 新增 `packages/ui/src/account/login-consent.tsx`：`useLoginConsent` 持有勾选状态，`LoginConsent` 画「☐ 我已阅读并同意《隐私政策》」。勾选框默认不勾，状态只在当前这次登录界面里，不落盘；《隐私政策》是独立的链接按钮，经宿主的 `onOpenUrl` 打开 `https://msime.app/privacy/`，点它不改变勾选，宿主不能开网页时是普通文字。
- 发送验证码、验证码登录、Google 登录和 Apple 登录在发请求前先调 `consent.confirm()`。没勾时不发请求，在勾选框下显示 `role="alert"` 的「请先阅读并勾选同意《隐私政策》」并把焦点移到勾选框（勾选框以 `aria-describedby` 关联这条提示）；勾上后提示消失。选「使用邮箱登录」「使用手机号登录」只展开表单、不发请求，不拦；「刷新登录方式」「清除失效登录状态」是修复动作，不拦。
- 鸿蒙手机的 `LoginSheet` 用这一行替换原来的「登录即表示同意…」；桌面式账号页（macOS、Linux、鸿蒙 2in1，以及只在开发合包里出现的 Android/iOS 共享页）把它放在登录卡片底部，登录成功后重置，退出登录后要重新勾。
- 同意文字是「我已阅读并同意《用户协议》和《隐私政策》」，两个文档名都是按钮，分别打开 `https://msime.app/terms/` 和 `https://msime.app/privacy/`（`app-resources.ts` 的 `termsUrl`、`privacyUrl`）。用户协议由 msime-web PR #670 起草、维护者审定后发布到 `/terms/`，这个 PR 在它上线之后才把《用户协议》接回来；之前列一个打不开的文档名就是第一条驳回本身。
- 鸿蒙上两个文档名经设置桥接的 `SettingsBridge.openExternalUrl`（`pages/Settings.ets`）用 `startAbility` 交给系统浏览器，首次启动授权弹窗里的两条链接走同一个方法。桥接方法是同步的，页面拿不到 `startAbility` 的失败，`useExternalUrl` 的「无法打开外部链接」提示因此永远不会出现；设备上没有能打开网页的应用时，点协议看起来又是「点击无响应」。所以失败时由原生侧弹出 `TextPolicy.unopenedLinkNotice` 的提示「没能打开浏览器，请在浏览器中访问 msime.app/terms/」，带上去掉协议头的地址，用户可以自己去浏览器里打开。
- 从登录面板点开的 msime.app 用户协议正文同样不能写成默认同意：msime-web 的 `src/content/terms.md` 原来写「确认登录，或者开始使用本服务，即表示你已阅读并同意本协议」和「修改生效后你继续使用本服务的，视为接受修改后的协议」，审核员点开就能看到与应用内勾选流程矛盾的条款。msime-web 分支 `fix/explicit-consent`（取代早先的草稿 `fix/terms-explicit-consent`）把它改成以登录界面勾选同意、隐私授权弹窗选择「同意」或官网表单勾选确认为前提，删掉「继续使用视为接受」，重要变更在同意新版本之前沿用此前同意的版本；同一个分支把 `/privacy/` 的标题统一成《隐私政策》，网站登录窗口也换成同一行勾选。条款措辞要等维护者审定，上线前点开的仍是旧正文，鸿蒙重新提审要等它上线。

## Android 与 iOS 原生登录

移动端的键盘界面三端同步改，登录也一样：Android 发布包和 iOS App 的登录是各自的原生界面，不含共享 React 设置页，规则照搬上面这一套，文字、提示和两个网址一字不差。

- 规则各有一份纯逻辑的小类：Android `home/LoginConsentPolicy.java`、iOS `App/Sources/account/LoginConsentPolicy.swift`，与 `useLoginConsent` 一样只有 `agreed`、`prompting`、`confirm()`。状态是登录界面自己的字段（Android 每次 `LoginSheet.show` 新建面板，iOS 是 `AccountLoginOptions` 的 `@State`），不落盘，每次打开登录都要重新勾。
- Android 的唯一登录入口是 `LoginSheet`：「我的」页、新手引导、插件和社区页都经 `SignIn.start` 打开它，「添加登录方式」（`link`）也是同一个面板。按钮下方一行是 Material 勾选框加「我已阅读并同意《用户协议》和《隐私政策》」，点勾选框或这行字都能勾上；两个文档名是 `ClickableSpan`，经 `LinkActions.open` 打开网页。TextView 的整行点击和链接点击在同一次抬手里都会触发：TextView 先经 View 的触摸处理把整行点击 post 出去，随后 `LinkMovementMethod` 才同步执行链接的 `onClick`，所以链接记下标志后 post 的复位排在整行点击之后，整行点击看到标志就不改勾选。Apple、Google、发送验证码和验证码登录四处都以 `if (!confirmConsent() || !begin(...)) return;` 开头；没勾时提示（`colorError`、polite live region）显示在勾选框下，收起键盘、把面板重新展开并把提示滚进可见区，再把读屏焦点移到勾选框；提示状态下勾选框的读屏 hint 就是这句提示（和 iOS 的 `accessibilityHint` 一样），焦点播报打断 live region 时也不会漏听。面板打开即展开（`STATE_EXPANDED`）并跳过折叠高度（`setSkipCollapsed(true)`），往下拖就是关掉，小屏和大字号下底部的勾选不会停在屏幕之外。
- iOS 的登录按钮在 `AccountLoginOptions` 里，登录面板 `AccountLoginSheet` 和发布皮肤时 Form 里的 `AppleAccountSection` 共用它，勾选也放在这里，两处一起生效。勾选框是带「已勾选／未勾选」值的按钮，读屏标签是整句同意文字，提示状态下把提示作为 hint；两个文档名是独立的 `Link`，原来缺的《用户协议》补上了。一行放不下时（小屏、大字号，字体用随动态字号缩放的 `.caption`）由 `ViewThatFits` 把文档名换到第二行。
- iOS 有两处和共享页不同。系统的 `SignInWithAppleButton` 一按就弹出 Apple 授权，没有在请求前拦下的机会，所以没勾时在它上面盖一层透明按钮，点它只显示提示，读屏也只看到这一层；`onCompletion` 里再检查一次，授权结果在没勾时不发给后端。手机号登录在自己的 `CodeLoginView` 面板里进行，那个面板盖住了勾选框，所以「手机号登录」在打开面板前就要求勾选，共享页的「使用手机号登录」只展开表单、不拦。
- Android 的 `check-host.sh` 钉住接线：`LoginSheet` 里没有「登录即」，每个 `begin("…")` 都在同一行先过 `confirmConsent()`，带类名的 `SignIn.startGoogle`、`startApple`、`requestEmailCode`、`verifyEmailCode` 调用只出现在 `LoginSheet`，文字和提示与 `login-consent.tsx` 的相同。它看不见 `SignIn.java` 内部不带类名的调用（例如目前没人用的两参数 `startGoogle(Activity, Consumer)`），那个文件里的流程靠评审守住。
- iOS 的 `scripts/test-ios-login-consent.py` 钉住同样的接线：`AccountLoginOptions` 里四处 `model.signInWithApple`、`signInWithGoogle`、`requestCode`、`signInWithCode` 所在的闭包或函数都以 `guard confirmConsent()` 开头，打开手机号面板的 `phoneChannel = .phone` 也一样；这几个方法在 `App/Sources` 里只从 `AppNavigation.swift` 和 `AccountCodeLoginView.swift` 调用；文字、提示和两个网址与 `login-consent.tsx` 的相同。UI 用例 `testLoginRequiresExplicitConsent` 在账号服务不提供邮箱登录时会跳过，这道源码检查不依赖线上服务。

Windows（WinUI `platforms/windows/settings/main.cpp` 与 Win7 `settings/win7/AccountForm.h`）的登录区仍然没有任何同意文字或勾选框，要在国内商店过同样的审核时需要补上，规则与本篇相同。msime-web 的新条款上线后，Windows 上的登录不再构成对协议的同意（新条款只承认勾选、授权弹窗和官网表单），在 Windows 补上勾选框之前这段空窗期是已知的。

注销账号前的重新登录也要勾选。服务端要求注销用的会话是 10 分钟内建立的（`account_recent_login_required`），Android「我的」页这时经 `SignIn.start` 弹出同一个 `LoginSheet`，iOS 和共享账号页则要退出后重新登录，三处都要先勾「我已阅读并同意」。不同意修改后条款、只想注销的用户因此得先勾选同意才能注销，和条款里「不同意的可以注销账号」不一致。这次没有改：给重新验证单开一个不要求勾选的模式，要同时处理「重新登录的是另一个邮箱、服务端就地新建账号」这种情况（新账号没有经过勾选），而且要三端和共享页一起改，单独处理。

## Alternatives considered

- **用户协议打包进应用，而不是链接到官网。** 离线也能看。不采用：隐私政策已经链接官网，两份文档放在同一处更新，改条款不用发版；审核员在联网环境下看。
- **按钮在没勾选时置灰，不给提示。** 实现更少，但用户不知道为什么点不动，审核要求的也是「用户尝试时给出明确提示」，所以按钮保持可点，点了提示并聚焦勾选框。
- **把同意状态存进偏好，勾过一次就一直算同意。** 少点一次，但审核要的是每次登录前的明确动作，且偏好会跨设备同步，状态来源不清，所以只放在内存里。
- **把 `openExternalUrl` 改成返回 Promise，让页面显示已有的「无法打开外部链接」。** 页面一侧不用改，提示和其他外链一致。不采用：设置桥接经 `registerJavaScriptProxy` 只注册同步方法（`Settings.ets` 的 `onControllerAttached` 注释写了异步列表的 Promise 不会结算，页面会卡在启动画面），而且首次启动弹窗是原生 `CustomDialog`，不经过页面，同样需要原生侧给出提示。

## Consequences

- **收益**：鸿蒙手机、平板和 2in1 的登录都要先明确勾选同意隐私政策，不再出现「登录即代表同意」和无法打开的文档名；macOS 和 Linux 的共享账号页同时获得同样的明示同意；Android 和 iOS 的原生登录也换成了同一行勾选和同一句提示。
- **代价与已知上限**：用户协议和隐私政策都在官网，离线时点开会失败。Windows 的原生登录仍没有同意文字。鸿蒙应用首次启动的隐私政策弹窗见 [首次启动的隐私授权](../feature/2026-10-27-harmony-privacy-first-launch.md)。

## Verification

- `apps/desktop/tests/account/login-consent.test.tsx`：鸿蒙登录面板的勾选框默认不勾、不再有「登录即」字样，《用户协议》是能点开的按钮；没勾时 Google 登录、发送验证码和登录都不调用客户端，显示提示并聚焦勾选框；勾上后才发请求；《用户协议》《隐私政策》各自打开对应网页、不改变勾选；桌面式账号页同样拦截，退出登录后勾选被清掉。这些用例在改动前全部失败。
- `apps/desktop/tests/account/account-page.test.tsx`、`login-sheet-client-generation.test.tsx` 里原有的登录流程用例先勾选同意再操作。
- `pnpm exec vitest run`（`apps/desktop`）、`apps/desktop` 与 `apps/harmony` 的 `tsc --noEmit`、`pnpm run lint` 通过。
- 打开失败的提示：`platforms/harmony/tests/privacy-consent.test.ts` 核对提示文字带上去掉协议头的协议地址，`tests/run.sh` 钉住 `openExternalUrl` 失败时调用 `showToast` 的接线。
- 2026-10-28 在 HarmonyOS 6.0.2(22) 手机模拟器上装了从 develop 打出的 HAP（干净安装）：首次启动弹出授权弹窗，「不同意」「同意」并列；弹窗和登录面板里点《用户协议》都拉起了系统浏览器（`com.huawei.hmos.browser` 到前台，模拟器里的页面没有渲染出内容，正文是否加载没能确认）；登录面板勾选框默认不勾，没勾就点「发送验证码」出现「请先阅读并勾选同意《用户协议》和《隐私政策》」，从浏览器回来勾选框仍未勾，勾上后提示消失。打开失败时的提示没能在设备上触发：模拟器的浏览器是系统应用，`bm` 不能停用它。最早那次（10-27）只在浏览器里用临时页面渲染过账号页。
- Android（2026-10-29）：`tests/home/LoginConsentPolicySmoke.java` 覆盖默认不勾、没勾时挡下并提示、勾上后放行、取消勾选后再次挡下，以及文字和两个网址；`check-host.sh` 的接线检查在删掉「发送验证码」那一处 `confirmConsent()` 后失败，恢复后通过，整套 `check-host.sh` 退出码 0（299 个冒烟）。在 API 35 模拟器上装了这个分支打出的 APK：勾选框默认不勾；没勾就点「通过 Google 登录」只出现提示、没有拉起 Google；填好邮箱点「发送验证码」时提示出现、键盘收起、没有换成验证码输入；点《用户协议》《隐私政策》分别用浏览器打开 `https://msime.app/terms/` 和 `/privacy/`，回来后勾选框仍未勾；点「我已阅读并同意」几个字能勾上，提示随之消失。深色模式、720×1280 屏幕加 1.3 倍字号下同意行和提示都折行显示、没有被挤出面板。读屏（TalkBack）没有在设备上试。
- iOS（2026-10-29）：`tests/core/LoginConsentPolicyTests.swift` 两个用例通过；`UITests/OnboardingUITests.swift` 的 `testLoginRequiresExplicitConsent` 在 iPhone SE（第 3 代，iOS 27.0）模拟器上通过：勾选框默认「未勾选」，两个文档名是链接，不再有「登录即」；没勾就点「发送验证码」出现提示且停在填邮箱这一步；勾上后提示消失；关掉面板再打开又是「未勾选」。同一台模拟器换成深色和 `accessibility-medium` 字号后，文档名换到第二行；没勾就点「通过 Apple 登录」只出现提示、没有弹出 Apple 授权；点《用户协议》由 Safari 打开 msime.app，回来勾选框仍未勾。`testGoogleSignInOpensGoogleAndCancelReturnsToSheet` 改为先勾选再点 Google，这次没有重跑（模拟器构建不带 GoogleSignIn 时它本来就跳过）。手机号登录和 Form 里的 `AppleAccountSection` 没有在模拟器上点过。
- 评审后补的改动（2026-10-29）：iOS 的 `scripts/test-ios-login-consent.py` 在删掉「发送验证码」那一处 `confirmConsent()` 后失败，恢复后通过。Android 面板跳过折叠高度、没勾时重新展开、勾选框在提示状态下的读屏 hint 这三处改动经 `check-host.sh`（299 个冒烟）和 `build-apk.sh` 编译通过，没有装到模拟器或真机上看过；TalkBack 下同意文字会被勾选框和那行字各读一遍，这一处等真机试过再定，没有为它把那行字从读屏里藏掉（藏掉后两个文档链接在读屏里就点不到了）。

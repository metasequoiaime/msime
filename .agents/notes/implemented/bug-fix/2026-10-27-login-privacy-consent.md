# Agent Note: 登录前勾选同意隐私政策，去掉打不开的《用户协议》

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
- 同意文字只列《隐私政策》。用户协议没有文档，列一个打不开的文档名就是第一条驳回本身；以后有了用户协议（例如发布在 `msime.app/terms/`），在 `LoginConsent` 里加第二个链接和对应的勾选文案即可。

## 其他平台的现状（这次没有改）

- Android 原生 `LoginSheet.java` 的 `agreement()`：「登录即表示同意《用户协议》和《隐私政策》」，只有隐私政策可点，没有勾选框，和鸿蒙改之前一样。
- iOS `AppNavigation.swift`：「登录即表示同意《隐私政策》」，隐私政策是链接，没有勾选框。
- Windows（WinUI `platforms/windows/settings/main.cpp` 与 Win7 `settings/win7/AccountForm.h`）：登录区没有任何同意文字或勾选框。

这些是各平台的原生代码，要在国内商店过同样的审核时需要各自补上，规则与本篇相同。

## Alternatives considered

- **起草一份用户协议，打包进应用或发布到 `msime.app/terms/`，让《用户协议》能打开。** 最贴近审核原话，商店里不少应用两份文档并列。但用户协议是需要维护者确定的法律文本（运营主体、责任条款、适用法律都不是代码能决定的），代码代理代写并让用户勾选同意，不可逆的成本落在用户和项目身上；发布到官网还要改另一个仓库。没有文档时先不引用它，等维护者定稿再加链接，代价只是一行文案。
- **按钮在没勾选时置灰，不给提示。** 实现更少，但用户不知道为什么点不动，审核要求的也是「用户尝试时给出明确提示」，所以按钮保持可点，点了提示并聚焦勾选框。
- **把同意状态存进偏好，勾过一次就一直算同意。** 少点一次，但审核要的是每次登录前的明确动作，且偏好会跨设备同步，状态来源不清，所以只放在内存里。

## Consequences

- **收益**：鸿蒙手机、平板和 2in1 的登录都要先明确勾选同意隐私政策，不再出现「登录即代表同意」和无法打开的文档名；macOS 和 Linux 的共享账号页同时获得同样的明示同意。
- **代价与已知上限**：没有用户协议。如果 AppGallery 之后要求必须同时提供用户协议，要先由维护者定稿并发布，再在 `LoginConsent` 加链接。Android、iOS、Windows 的原生登录仍是「登录即表示同意」或没有同意文字。鸿蒙应用首次启动没有隐私政策弹窗，这不在本次审核意见里，没有处理。

## Verification

- `apps/desktop/tests/account/login-consent.test.tsx`：鸿蒙登录面板的勾选框默认不勾、不再有「登录即」和「用户协议」字样；没勾时 Google 登录、发送验证码和登录都不调用客户端，显示提示并聚焦勾选框；勾上后才发请求；《隐私政策》只打开网页、不改变勾选；桌面式账号页同样拦截，退出登录后勾选被清掉。这些用例在改动前全部失败。
- `apps/desktop/tests/account/account-page.test.tsx`、`login-sheet-client-generation.test.tsx` 里原有的登录流程用例先勾选同意再操作。
- `pnpm exec vitest run`（`apps/desktop`）、`apps/desktop` 与 `apps/harmony` 的 `tsc --noEmit`、`pnpm run lint` 通过。
- 没有在鸿蒙真机或模拟器上装 HAP 看过；只在浏览器里用临时页面按 `data-platform="harmony"` 和 `hm2` 渲染了账号页并截图。

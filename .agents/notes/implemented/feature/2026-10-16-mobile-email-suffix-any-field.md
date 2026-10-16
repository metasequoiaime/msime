# Agent Note: 三端任何输入框打到 xxx@ 都给邮箱后缀候选

Status: implemented

## Problem

产品方在 iPhone 上反馈：打 `@` 后候选栏没有常用邮箱后缀。iOS 和鸿蒙宿主根本没有这项补全；Android 按 [邮箱后缀](2026-10-09-android-email-suffixes.md) 只在系统标记为邮箱的输入框里给；引擎的 [网址模式](2026-10-15-engine-email-url-completions.md) 只在 ASCII 组字中按 `@` 时进入，触屏上 `@` 多半从 123 层、「符」面板或长按打出，那时组字已经上屏或根本没有组字，引擎看不到。

## Decision

iOS、Android、鸿蒙在任何可编辑输入框里，光标前打到 `<本地部分>@<已打的域名>` 且没有组字时，候选栏给出能接上的邮箱后缀，点一下补全；密码框（iOS 还有验证码框）不读上文、不给。匹配与补全规则三端逐条相同，沿用 Android 的 `EmailSuffixPolicy.match`：只看 `@` 前紧挨的一个字符（ASCII 字母、数字或 `._%+-`），已打的域名只含 ASCII 字母、数字、`.`、`-`，以最后一个 `@` 为准，全角 `＠` 不算；按已打域名缩小范围，已是完整后缀时不给；补全删掉已打的域名、上屏去掉 `@` 的后缀，统一小写、始终 ASCII。读到的上文只在内存里匹配一次，不记录。

- iOS：`KeyboardExtension/Sources/input/EmailSuffixPolicy.swift`；`KeyboardViewController` 在候选栏重画、`textDidChange`、`selectionDidChange` 时重新匹配。宿主不会为键盘自己的 `insertText` 回调 `textDidChange`，所以每次键盘自己的编辑之后在下一轮主循环补一次匹配，「符」面板、长按角标打出的 `@` 也能立刻出后缀。
- Android：`ImeEmailSuffixes` 的门禁从 `EditorPolicy.emailAddress` 换成 `offersEmailSuffixes`（非密码框即可），其余不变。
- 鸿蒙：`keyboard/input/EmailSuffixPolicy.ts` 与 `KeyboardSession.refreshEmailSuffixes`，在选区变化、外部编辑、编辑器属性到达、插入之后和 `render()` 末尾匹配；2in1 只在组字时有候选窗，不接这项。
- 后缀表 20 个，国内常用的在前，四处（引擎 `EMAIL_SUFFIXES`、Android `SUFFIXES`、iOS `suffixes`、鸿蒙 `EMAIL_SUFFIXES`）同一份同一顺序，`scripts/test-email-suffixes.py` 核对，随 `run-checks.sh` 进 CI。iOS 与鸿蒙「符」面板的「网络」分类在网址片段之后直接引用这份表，与 Android 一致（原来只有 @qq.com、@gmail.com）。

### 旧笔记审计

- [Android 邮箱后缀](2026-10-09-android-email-suffixes.md)：「只在邮箱输入框里出现」由本篇取代，该篇的后缀表、匹配规则、点按补全和「网络」分类仍成立；双方互链。
- [引擎网址模式补全](2026-10-15-engine-email-url-completions.md)：组字中按 `@` 进入网址模式的路径不变，与宿主补全互不冲突（组字时候选栏归引擎）。

## Alternatives considered

- **维持只在邮箱输入框里给。** `git@`、`user@` 这类非邮箱写法不会占掉候选栏；但真正要发邮箱地址的地方常是聊天框、备忘录，产品方明确要任何输入框都给。
- **只靠引擎网址模式。** 不用在三个宿主各写一份；但触屏上 `@` 几乎从不在 ASCII 组字中按下，等于没有。
- **做成开关。** 不想要的人可以关；但规则已排除单独的 `@`、`微信@` 和全角 `＠`，误触发主要是 `git@` 这类少见写法，多一个开关不抵成本。

## Consequences

- 收益：三端在任何输入框里打完邮箱前缀加 `@` 就能点选后缀；四份后缀表由同一个检查守住。
- 代价：`git@`、`user@` 之类也会给后缀并暂时占用候选栏；每次选区变化多读一次光标前 64 个字符（三端都在内存里匹配，不记录）。中文模式下 iOS 的 123 层是数字键盘、没有 `@`，`@` 要从「符」面板、a 键角标或引擎标点打出。

## Verification

- iOS：Mac Studio 上 `MSIMEClientTests` 的 `build-for-testing` 通过；本机 iPhone 17 / iOS 27.0 模拟器上 `EmailSuffixPolicyTests`（Android 冒烟用例逐条对应）与 `EmailSuffixKeyboardTests`（真实 `KeyboardViewController`：`zhangsan@` 首个是 @qq.com 且点按得到 `zhangsan@qq.com`、`@GM` 缩小并小写补全、上文变化只收起、`微信@` 与全角不给、键盘自己打出的 `@` 触发），以及全量单元测试，结果见 PR。
- Android：`check-host.sh` 通过，`EditorSmoke` 断言普通文本框、URI、短信等都给，四种密码类型不给；Studio 上 `compileFullReleaseJavaWithJavac` 通过。改过的 `EmailSuffixDeviceSmoke`（普通框补全、密码框不出现）只编译过，未在模拟器上运行。
- 鸿蒙：`platforms/harmony/tests/run.sh` 0 failure（新增策略用例与「网络」条目数）；HAP 在「网络」改动之前打包通过。
- `scripts/test-email-suffixes.py`：四处一致（20 个），在临时副本上构造顺序、缺项、多项和缺文件时都报错退出。

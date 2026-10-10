# Agent Note: Android 邮箱后缀：符号面板补全常用后缀，邮箱输入框里打到 @ 后候选栏给出后缀

Status: implemented

## Problem

#6147：Android 符号面板「网络」分类里的邮箱后缀只有 `@qq.com` 和 `@gmail.com`。用户要两件事：补上 18 个常用后缀（@163.com @126.com @139.com @189.cn @aliyun.com @foxmail.com @outlook.com @hotmail.com @live.com @icloud.com @yahoo.com @proton.me @protonmail.com @aol.com @mail.com @gmx.com @naver.com @yahoo.co.jp）；打完邮箱前缀、按下 @ 之后，候选栏自动列出后缀，点一下补全。

宿主里原来没有任何和邮箱有关的补全：英文直输的建议（`refreshEnglishSuggestions`）只认光标前的纯字母单词，打到 `@` 就清空；中文模式下 Engine 对 `@` 只有默认关闭的「提及」本地模式。

## Decision

- 后缀表是一份常量 `EmailSuffixPolicy.SUFFIXES`（`platforms/android/java/app/msime/android/policy/`，无 Android 依赖），20 个，国内常用的在前：@qq.com、@163.com、@126.com、@foxmail.com、@139.com、@189.cn、@aliyun.com，然后是 @gmail.com、@outlook.com、@hotmail.com、@live.com、@icloud.com、@yahoo.com、@proton.me、@protonmail.com、@aol.com、@mail.com、@gmx.com、@naver.com、@yahoo.co.jp。符号面板「网络」分类在原来的网址片段之后直接接上这份表，候选栏用同一份、同一个顺序，两处不会各写一份。
- 符号格里超过 4 个码位的非颜文字条目（`http://`、邮箱后缀）缩小字号：9 个码位以内 13sp，再长 11sp，并设成单行、放不下时末尾省略，不折行（`SymbolPanelModel.cellTextSizeSp` / `singleLineCell`）。4 个码位以内（单个符号、`.com`、`www.`）仍是 18sp。「常用」里点过的后缀按同样的规则画。
- 候选栏的后缀只在系统标记为邮箱的输入框里出现：`EditorPolicy.emailAddress` 认 `TYPE_TEXT_VARIATION_EMAIL_ADDRESS` 和 `TYPE_TEXT_VARIATION_WEB_EMAIL_ADDRESS`。其他输入框（聊天、搜索、网址）里的 `@` 不触发，也没有开关。
- 匹配规则在 `EmailSuffixPolicy.match`：光标前文字末尾要是 `<本地部分>@<已打的域名>`；本地部分只看紧挨 `@` 的那一个字符，必须是 ASCII 字母、数字或 `._%+-`；已打的域名可以为空，只含 ASCII 字母、数字、`.`、`-`；以最后一个 `@` 为准；全角 `＠` 不算。给出以已打域名开头（不分大小写）的后缀，所以 `abc@` 给全部，`abc@1` 只剩 163、126、139、189；已打的域名已经是一个完整后缀时不再给。
- 宿主 `ImeEmailSuffixes`（`core/`）在 `onUpdateSelection` 末尾（和 `ImeCalculator.refresh()` 挨着）重新匹配：有输入连接、是邮箱输入框、没有组字时才向编辑器要光标前 64 个字符；结果变了才 `render()`。离开输入框（`onFinishInput`）时清掉。读到的文字只在内存里匹配一次，不记录、不保存。
- 后缀占候选行，和英文建议同等地位：`render()` 的空闲判定多一个 `!hasEmailSuffixes`；候选分支里在英文建议之前先给后缀，按钮样式与英文建议相同（候选字号、候选字体），无障碍描述是「邮箱后缀 n：@163.com」。组字时候选行归引擎的候选，后缀不显示。
- 点按时按这一刻的上文重新匹配，确认这个后缀仍然能接上，再在一次 batch edit 里删掉已打的域名、上屏后缀去掉 `@` 的部分（统一小写）；不走 `fullWidthOutput`，邮箱在全角模式下也是 ASCII。上文变了、对不上时只收起。补全后上文以完整后缀结尾，下一次匹配自然没有结果，候选行换回工具栏。

## Alternatives considered

- **任何输入框里只要光标前形如 `xxx@` 就给后缀** — 更贴近用户原话，在聊天里发邮箱也能用；但 `git@`、`user@` 这类不是邮箱的写法也会抢走候选行和工具栏，而真正要填邮箱的地方（注册、登录表单）系统几乎都会标成邮箱输入框。先只在邮箱输入框里给。
- **做成「键盘 › 输入辅助」里的开关，默认开** — 不想要的人可以关；但只在邮箱输入框里出现之后误触发很少，多一个开关就多一行设置、一个本地设置键和搜索索引，收益不抵成本。
- **用工具栏上的胶囊（像算式结果那样）给后缀** — 不占候选行，改动小；但后缀有二十个，工具栏胶囊最宽 96 dp，只放得下一个，选不了。
- **严格按「光标前以 `xxx@` 结尾」才给，接着打域名就收起** — 规则最简单；但用户在 `@` 后打了一两个字母（想打 gmail 先敲了 g）时后缀会消失，反而用不上。按已打的域名缩小范围是同一条规则的自然延伸，`@` 刚打完时的行为不变。
- **iOS 和鸿蒙的「网络」分类一起补** — 三个平台的列表原本是同一份的副本；但这次只改 Android，其他平台的面板、测试和发版节奏各自独立，留给它们自己的 issue。

## Consequences

- **收益**：「网络」分类有 20 个常用后缀，长条目不再折行或溢出；在邮箱输入框里打完 `@` 就能点选后缀，不用切到符号面板。
- **代价**：邮箱输入框里每次选区变化多一次 `getTextBeforeCursor` IPC（64 个字符），与算式结果、英文建议同一量级；`http://`、`https://` 在符号格里从 18sp 变成 13sp。只在 Android 上有，iOS 和鸿蒙的「网络」分类仍只有两个后缀。
- **未覆盖**：中文模式下在邮箱输入框里按 `@` 时 Engine 上屏的是不是半角 `@`，没有逐行核实；邮箱输入框进框时会临时切到英文（`EditorPolicy.prefersLatin`），主要场景不受影响。本地 arm64 API 35 模拟器上用 adb 截图看过：WebView 的 `<input type=email>` 里 `abc@` 列出 @qq.com、@163.com、@126.com、@foxmail.com…，`abc@1` 缩到 163/126/139/189，点 @163.com 得到 abc@163.com；符号面板「网络」分类每个后缀单行、缩小，长的末尾省略（@outlook.co…、@protonmai…）。没有在设备上确认普通输入框里不出现后缀（只有 `EditorSmoke` 的策略检查）；宿主接线（batch edit 里的删除加上屏、`render()` 里排在英文建议前面的分支）没有自动化测试，CI 的 android-device.yml 没有扩展用例。补全到 abc@163.com 之后，已有的英文建议会对 `com` 给出单词，这是英文建议在点号之后的原有行为，没改。

## Verification

- `ANDROID_SDK_ROOT=… bash platforms/android/check-host.sh`：新增 `tests/keyboard/EmailSuffixPolicySmoke.java`（后缀表的顺序与格式、`abc@` / `abc@1` / `abc@gm` / `abc@yahoo.c` / 大写域名、已是完整后缀或未知域名不给、`@` `微信@` `a b @` `@@` 全角 `＠` 不触发、以最后一个 `@` 为准、补全时删除和上屏的内容）；`SymbolPanelModelSmoke` 新增「网络」的条目数、全部后缀、国内在前、与 `SUFFIXES` 同一份、长条目字号与单行；`EditorSmoke` 新增 `emailAddress` 只认两种邮箱 variation。
- Gradle `compileFullReleaseJavaWithJavac` 编译 `ImeEmailSuffixes` 和 `MSIMEInputService` 的接线。

# Agent Note: Windows 原生设置窗口自己画「账号与同步」页

Status: implemented

## Problem

「账号与同步」是原生设置窗口里最后一个 `PageHost::Shell` 页：一张「打开」卡片，用 `MSIME.exe --route=settings:account` 拉起共享 Tauri 应用。整项工作的目标是任何设置都在原生窗口里改。共享账号页（`packages/ui/src/account/account-page.tsx`）在 Windows 上走桌面那一支，宿主（`apps/desktop/src/main.tsx`）给它的是：账号客户端（含 `googleLogin`、`googleCancel`、`avatar`、`chooseAvatar`、`removeAvatar`）、Windows 才有的 `settingsSync`（`createDesktopSettingsSync`）、`onOpenCloudDictionary`、`onOpenCloudClipboard` 和 `onOpenCommunity`（Windows 的社区页只有词包与回复模板，`communityHasSkins` 为假）。没有 `appIcon`（只有手机）、没有 `customTouchKeyboardSkins`（只有 Linux，所以没有「我的设计」）、没有 `onOpenAbout` 等（只给手机），也没有 `onCancelLogin`（只在从别的页面跳来登录时）。任务说明里猜的「Windows 上没有设置同步和社区」核对下来不成立：两者都有。上一步已经备好 `msime_client_account`（见 [原生设置窗口账号页用到的 C 接口](../architecture/2026-10-11-windows-settings-account-host-api.md)）。

## Decision

- `SettingsNavigation.h` 里 `account` 改为 `PageHost::Native`，`shell_page_description` 删掉（没有 Shell 页了，`build_shell_page` 保留为不带说明的通用卡片），`build_page` 调新的 `build_account_page`。新增 `shell_links::community`（`settings:community`），`all` 和导航测试一起改：每个页面都是原生页，社区链接是共享应用接受的路由。
- 昵称规范化、合法性、默认显示名和长度上限由 `client-services::account::name::present_name` 经纯 C ABI `msime_client_account_name` 提供；合法性直接复用改名请求的 `validate_display_name`，不再在 C++ 复制控制字符和码点计数规则。`client-base::text::trim_ui_whitespace` 保留 JavaScript `trim` 的空白集（含 BOM、不含 U+0085），语音服务切换也复用它。仍由原生表单处理的登录目标、QQ 绑定码和社区字段共用 `SettingsText.h` 的去空白与 UTF-16 计数，社区模型不再反向依赖账号模型。`AccountPageModel.h` 只保留展示规则与登录目标去空白：账号 ID 短写、头像首字（不拆代理对）、登录方式从共享资料的 `providerNames` 查表、`accountMessage` 连同审核类三句、两种确认的文字、验证码只留数字和 `/^\d{6}$/`、重发倒计时和两个按钮的文字、`createdAt` 的 ISO 8601 解析（与 JavaScript `Date` 同样把只有日期的读作 UTC 零点）、`toLocaleDateString("zh-CN")` 的「年/月/日」，以及 Windows 设置同步卡片的说明和确认文字。由 `tests/ui/account_page_model.cpp` 核对，加进 CMake 测试。
- 页面状态照共享页的组件状态：每次进入页面重置并重读（`enter_account_page`），先并行读 `status` 和 `providers`，有用户再读 `profile`，读完才画（之前显示「正在读取账号状态…」）。`account_begin`/`account_finish` 就是共享页的 `perform`：忙时不接新操作，开始时清掉错误和提示，失败时除 `account_cancelled` 外按 `accountMessage` 说明。`account_generation_` 在进出页面时递增，丢掉离开前发出的回答。所有联网操作经 `run_host` 在工作线程上；`google_cancel` 只清内存里的等待，在 UI 线程上直接调，离开页面和关窗时都调，与共享页卸载时的 `cancelGoogle` 一样。
- 验证码表单的号码框和验证码框不随输入重画页面：改号码时就地收起验证码一栏（共享页改号码就丢掉 challenge），验证码框就地滤掉非数字，「获取验证码」和「登录」两个按钮由 `update_account_code_controls` 就地改文字和可用状态，一秒一次的计时器也只改它们，所以输入焦点不丢。`TextChanged` 是异步事件，重画时填回去的号码也会晚一步报上来，所以号码框只在文字和记下的号码不同时才当作用户改了号码、丢掉验证码，否则「验证码已发送」后重画页面会把刚发的验证码一栏收起。
- 「编辑资料」是一个 `ContentDialog`：头像按钮（系统打开对话框，标题「选择头像」、只列 png/jpg/jpeg，与 Tauri 的 `account_choose_avatar` 相同，选好后经 `upload_avatar {path}` 上传，`account_invalid` 换成头像的说法）、名字预览、「移除头像」（只对 `avatarUploaded`）、社区昵称、账号信息（ID 点按复制 1.8 秒显示「已复制」，复制不了时说明）、「保存修改」「取消」。对话框开着时状态变了（上传完、资料读到、「已复制」）由 `account_dialog_refresh_` 就地更新；忙时 `Closing` 被取消，与共享对话框忙时关不上相同。共享对话框右上角另有一个「关闭」，与「取消」作用相同，`ContentDialog` 只保留「取消」。
- 头像不加载远程图片：avatarUrl 变了才经 `avatar` 取 `data:image/png|jpeg;base64,…`，在本窗口解码成 `BitmapImage` 画进圆里；取不到时是强调色底上的首字。
- 设置同步卡片有自己的忙碌和说明行（共享卡片的 `report` 把错误和结果写在同一处）：换了账号就重读字段表和云端设置；上传和应用前各用一个确认对话框（标题取共享卡片的 aria-label，正文和按钮相同），先 `flush_pending`；偏好目录传本窗口读写的 `state_directory()`。应用成功或写回被拒都重读本窗口的设置文档，各页显示存下的值；应用期间离开了账号页或换了账号时卡片的结果作废，但设置文档照样重读，免得之后在别的页上按旧文档保存。
- 「云端」两行仍打开共享应用的云词库、云剪贴板面板（输入工具，不是设置）。「我的内容」四行打开共享应用的社区页。
- 剪贴板页的「发到云剪贴板」上一阶段已经经 `msime_client_account_cloud_clipboard` 原生实现，这一步不用再接。

## Alternatives considered

- **在 Windows 继续保留昵称预检，只让提交时的 Rust 拒绝不合法值** — 预检不需要跨 ABI，代码也已有测试；但 C++ 只拒绝 ASCII 控制字符，Rust 同时拒绝 C1 控制字符，规则已经漂移。纯计算入口不会访问文件、会话或网络，可以直接放在 UI 线程执行，因此原生页读取同一份结果更容易维护。
- **昵称接口采用 Rust `trim`** — 可以直接使用标准库；但它不去 BOM，却把 U+0085 当作空白，会改变原设置界面的草稿和默认名字。先按现有 UI 空白集规范化，再复用后端校验，保留编辑行为且不扩大后端接受范围。

- **「我的内容」四行不画** — 不必依赖共享应用；但共享页在 Windows 上有这一组，删掉就少了进入自己发布和收藏的作品的入口。社区是作品库，不是设置，与「主题」页「社区皮肤」一行一样留在共享应用里。
- **给共享路由加上社区列表的参数** — 四行就能各自落到「我发布的词库」等列表；但要改 `host_surface` 的路由词表、Tauri 的路由处理和共享 UI 的初始页逻辑，是跨平台的契约改动，不在这一步。
- **号码和验证码框改动也重画整页** — 写法简单；但每输入一个字都会丢焦点。
- **确认用页面里的内联确认块（照共享页）** — 原生窗口其他页的不可撤销操作都用 `confirm` 对话框，键盘和读屏的行为一致。

## Consequences

- 原生设置窗口不再为任何设置页拉起共享应用；`PageHost::Shell` 还在类型里，但没有页面用它。还会打开共享应用的只有输入工具面板（屏幕键盘、语音、手写、云词库、云剪贴板）和社区作品库（社区皮肤、社区词包与回复模板）。
- 页面的版式（未登录时的居中卡片、资料卡、编辑资料里的账号信息、「云端」「我的内容」的跳转行、QQ 群助手卡片的状态）由 [「账号与同步」页的版式对齐 macOS](2026-10-20-windows-settings-account-mac-layout.md) 接手，本篇的流程和状态管理照旧。
- 有意与共享页不同的两处：改了号码以后重发倒计时照样走完（共享页 challenge 清空后不再更新 `now`，「N 秒后可重新发送」会停住，按钮一直不可用）；「我的内容」四行都落在社区首页，要在那里自己选列表。
- 昵称现在在发送改名请求之前就拒绝 C1 控制字符；旧的 Windows 预检会放行，随后由 Rust 拒绝。后端接受范围不变。控件的输入上限改为容纳 64 个码点所需的 128 个 UTF-16 单元，超过 64 个码点的草稿可以继续编辑但不能提交，避免把合法的非 BMP 昵称截短。昵称计算增加一次同步 JSON/C ABI 转换，但不触及会话、文件或网络；ABI 失败时保留草稿，保存报服务不可用。
- 共享账号页的展示文案、`account-errors.ts` 或 `createDesktopSettingsSync` 的规则改了，要同步 `AccountPageModel.h` 和 `main.cpp`。Windows 的昵称业务规则归共享 Rust，`packages/ui` 的 `account-labels.ts` 仍有自己的同步实现，后续迁移不能引入异步旧草稿覆盖。错误码的说法由共享用例 `shared/contracts/account-errors/cases.json` 约束，`accountMessage` 和 `error_message` 逐条跑同一份（[账号会话布局与账号错误码的共享用例](../testing/2026-10-19-account-shared-contract-cases.md)）。

## Verification

- `tests/ui/account_page_model.cpp` 和 `tests/ui/settings_navigation.cpp` 用 cl `/W4 /WX` 单独编译运行，都通过。
- 原生设置工程由 PR 的 Windows CI 按 `/WX` 编译并链接 `msime_host_api.dll.lib`；当前昵称切片的本地验证与 CI 结果记录在 PR。共享 TypeScript 本片没有改动。
- 昵称切片以合成值测试规范化、64/65 码点边界、表情、C1 控制字符和默认名；C ABI 测试不提供会话目录，覆盖固定错误与请求上限。Windows 的 `TextBox.MaxLength` 使用 Rust 给出的 `maximum_utf16_units`（128），使 64 个非 BMP 字符也能完整输入；草稿超过 64 个码点时由共享 `valid` 拒绝，控件的容量不作为业务校验。长度上限回归先因该字段缺失失败，再验证 64 个表情所占的 128 个 UTF-16 单元能装入控件。本片没有启动窗口，也没有对真实账号服务发请求：登录、Google 浏览器回跳、头像上传与显示、编辑资料对话框的就地更新、设置同步和倒计时都没有在运行中的窗口里看过。

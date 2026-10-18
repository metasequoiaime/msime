# Agent Note: 账号会话布局与账号错误码的共享用例

Status: implemented

## Problem

#8201 把各平台的账号会话和账号请求收进 Rust 以后，还剩两类东西每个平台各有一份：存储里的会话字节（Swift 写的 Apple 布局、Rust 写的 Native 和 Apple 布局、鸿蒙旧版 ArkTS 写的扁平布局、Android 原来的 Java 写的 v2 会话），以及把 Rust 的账号错误码说成一句中文的文案表（共享账号页 `account-errors.ts`、Windows 的 `AccountPageModel.h`、Swift 的 `BackendRustClient.Failure`、Android 的 `AccountFailureText`）。原来的测试各写各的内联样例：`file_storage/harmony_tests.rs` 自己拼扁平会话，Rust 的 Apple 布局测试用手写的「Swift 形状」，Swift 的回调测试自己拼 Rust 写的字节，Android 的设备冒烟自己拼 v2 会话，四张文案表各挑几个码核对。哪一边改了字节形状或措辞，另一边的测试照样是绿的，只有装到设备上读不出会话、或者同一个码在两个平台说成两句话才会发现。

## Decision

两份用例放在 `shared/contracts/`，Rust 和各平台的测试读同一份文件：

- `account-session-layouts/cases.json`：每条是一份存储里的原样字节（`document`，一个 JSON 字符串）、它的布局、谁写的，以及 Rust 在 `now_unix_ms` 时读出的会话（按 Native 布局的字段名）或读不出时的错误码。Swift 那一份是用本机 `swift test` 实际编码一次得到的字节（键的顺序不固定、斜杠转义成 `\/`、`sessionID` 大写），Android v2 那一份按 Android 的 `org.json` 的写法（斜杠转义、键按插入顺序），鸿蒙扁平的两份（一份未过期、一份早已过期）按已删除的 `AccountCloudBridge.ts` 的 `JSON.stringify`。`rust_writes` 为真的两条要求 Rust 写出的字节与 `document` 逐字节相同。所有过期时间都在过去，交给 `status` 这类不刷新的操作时既不会因为「过期时间太远」被拒，也不会联网。
- `account-errors/cases.json`：每条是一个账号错误码、可选的 Retry-After 秒数、Rust 从哪些输入得出它（HTTP 状态加服务端码、断网、本地失败），以及三张文案表各自的那一句：`shared`（共享账号页和 Windows，逐字相同）、`apple`、`android`（个人资料页的 `AccountFailureText.profile`）。三张表的措辞本来就不同，用例把每一处不同都写出来，而不是只收大家一致的码。

消费方：

- Rust：`client-services` 的 `file_storage::layout_cases_tests`（逐条解码、核对 Rust 写出的字节、经真的会话文件再读一遍）、`host_storage` 的测试（经注入的存储读，Native 和 Apple 两种登记都读）、`error_cases_tests`（每个 `from` 得出那个码和秒数；`AccountError` 的每个变体、`AccountFailure` 的每个本地码和它特判的服务端码都要有用例，加一个变体而不加用例时编译或测试失败）；`host-api` 的 `every_shared_layout_case_reads_through_a_host_store` 经 C 接口注册的宿主存储读每一条。`harmony_tests.rs` 的扁平会话、Apple 布局的那条内联样例改成读用例。
- Swift：`shared/backend/Tests/BackendSharedContractCaseTests.swift` 逐条解码 Apple 布局、逐条核对 `errorDescription`；`BackendRustSessionStoreTests` 里 Rust 写的字节改用 `apple_rust`。从源码树里读文件，只在 `swift test` 里跑。
- 共享账号页：`apps/desktop/tests/account/account-errors.test.ts` 逐条核对 `accountMessage`。
- Windows：`windows-account-page-model` 在 CMake 配置时把用例转成字节数组编进测试（与 `windows-ai-endpoint-policy` 相同，Wine 里不挂仓库也能跑），逐条核对 `error_message`。
- Android：JVM 冒烟 `AccountFailureTextSmoke` 按用例文件的固定写法逐条匹配（冒烟里的 `org.json` 只是抛 `Stub!` 的桩）；设备冒烟 `SessionStoreDeviceSmoke` 读 `build-editor.sh` 打进测试包 assets 的用例，把 `android_v2` 原样经 `AndroidAccountSessionStorage` 写进 `msime_account_session_v2`，Rust 的 `status` 读出用例里的账号，并先核对这台设备的 `org.json` 写回的字节与用例逐字相同。

## Alternatives considered

- 各平台继续各写内联样例，只是彼此对齐。最强的理由是每个测试自包含、读起来不用跳文件。否掉：这正是原来的状态，对齐只在写的那天成立，没有东西在之后把它们拴在一起；会话字节的问题只在设备上才暴露。
- 文案用例只收各平台措辞一致的码。这样每条只有一个答案，文件更短。否掉：一致的码很少（`account_conflict`、`account_too_large`、限流带不带秒数、Android 的整张表都不一样），只收一致的就等于不约束大部分码；把不同写出来，改任何一张表的措辞都要同时改用例，审查时看得见。
- Android 的 JVM 冒烟带一个 JSON 解析库读用例。否掉：要新增依赖；用例文件的写法是生成的、固定的，按固定写法匹配并核对匹配条数等于声明条数，漏匹配会直接失败。

## Consequences

- 改会话的编解码、任何一张文案表的措辞、或 Rust 新加一个账号错误码，都要同时改这两份用例，否则 Rust 或那个平台的测试失败。用例的 `description` 写明了权威实现和每个字段的意思。
- Android v2 和 Swift 的字节是「当时写出来的样子」，那两段已删或不再写这种形状的代码不会再变；Swift 那份若将来 Foundation 改了编码细节，用例仍代表已经落在用户设备上的旧字节，这正是要它读得出的东西。
- 设备冒烟和 Windows 测试这次没有跑：设备冒烟只编译并打包过（测试包里有 `assets/contracts/account-session-layouts/cases.json`），`org.json` 写回的字节与用例逐字相同这一条要在设备上才能证实；Windows 测试在本机用 clang 编译运行通过，并用 mingw-w64 做了 `-fsyntax-only`，CMake 的部分交给 CI。
- 相关：[Windows 原生设置的账号页](../feature/2026-10-11-windows-settings-account-page.md)（`AccountPageModel.h` 的文案现在由用例约束）、[Android 设置页改调 Rust](../architecture/2026-10-18-android-settings-pages-through-rust.md)（`AccountFailureText` 的来历）、[Apple 的 Rust 函数表](../architecture/2026-10-18-apple-rust-client-table.md)（布局 1 的注入存储）。

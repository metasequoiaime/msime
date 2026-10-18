# Agent Note: Apple 宿主经一张函数表调用 Rust，会话存储由 Swift 注入

Status: implemented

## Problem

#8201 要把 macOS 与 iOS 的账号和云端业务从 `shared/backend` 的 Swift 实现迁到 Rust 的 C 接口（`crates/host-api/include/msime_client.h`）。会话放在只有平台能读写的地方：iOS 的登录会话在 App Group 访问组的钥匙串里，macOS 的在输入法状态目录的 `account-session.json`，两个平台的匿名会话都在 App Group 的 `BackendLocalStore` 文件里，所以 Rust 要经 `msime_client_session_store_register` 拿宿主注入的存储（契约见 [宿主注入会话存储](../../proposed/architecture/2026-10-16-host-injected-session-storage.md)）。

`shared/backend` 有三种编法，只有一种链着 Rust：iOS 的 App 和测试 target 把源文件直接编进去并链接 `libmsime_host_api`；macOS 把它编成 `MSIMEBackend.dylib`，Rust 静态库链在输入法可执行文件里，dylib 不链接 Rust（原来云剪贴板的端到端加密驱动就是经 `MSIMEInstallCloudClipboardDriver` 交函数指针过去的）；`swift test` 完全没有 Rust。迁移期间 Swift 的 `BackendAccountSession` 还会和 Rust 读写同一份会话，两边必须用同一把锁（`msime_client.h` 的「共存」）。

## Decision

- `shared/backend/rust/MSIMERustClient.h` 定义一张带版本号的函数表 `msime_rust_client`（`version`、`size` 加账号与云端用到的二十个入口），`msime_rust_client_linked()` 用本进程链进来的 `msime_client_*` 填满它。`shared/backend/rust/module.modulemap` 把它和 `msime_client.h` 作为 `CMSIMEClient` 模块交给 Swift：只有类型和内联函数，不链接任何库。SwiftPM 是 `systemLibrary` target，macOS 的 swiftc 和 iOS 的 `SWIFT_INCLUDE_PATHS` 都加 `-I shared/backend/rust`。
- Swift 只经这张表调用 Rust（`BackendRustClient`）。macOS 的 `input_method_main.mm` 在 `dlopen` 后端 dylib 以后把表交给 `MSIMEInstallRustClient`（取代 `MSIMEInstallCloudClipboardDriver`，云剪贴板驱动改从同一张表取）；iOS App 启动时 `BackendRustClient.install(msime_rust_client_linked())`。表的版本或大小不对时不收，之后每个入口都报 `rust_client_unavailable`；大小与 `MemoryLayout<msime_rust_client>.stride` 比，因为 C 的 `sizeof` 含结尾的填充（现在的布局两者相同）；dylib 先只读开头的 `version` 和 `size`，不会按本版本的大小越界读一张更短的表。
- 阻塞的 Rust 入口放在专用 `Thread` 上（2 MiB 栈），不进 Swift 协作线程池、不在主线程；流式对话包成 `AsyncThrowingStream`，消费方不再读时调 `msime_client_account_chat_cancel`；AI 皮肤的取消走 `msime_client_ai_skin_cancel`，取消在调用 Rust 之前到达时本地直接以 `ai_skin_cancelled` 结束。宿主已经调用、Rust 还没登记请求 id 的那一小段里到达的取消由 Rust 记下：client-services 的 `ChatStreamRequests` 和 `AiSkinRequests` 把在表里找不到的取消记进 `EarlyCancels`（最近 64 个），同一个 id 随后登记时按已取消开始，所以宿主每次都要用新的请求 id。Swift 侧没有办法在 Rust 登记以后再看一次自己的取消标记（登记发生在那次阻塞调用里面，没有回调），所以放在 Rust。
- `BackendRustSessionStore` 是注入的存储：布局 1（Swift 的 `BackendSavedSession`），置 `SHARED`。四项的位置与两把锁写死成现有 Swift 用的那几个（`BackendRustStoreLayout.standard`）：
  - iOS：ACCOUNT 是钥匙串 `app.msime.backend.account` / `https://api.msime.app`，CLIPBOARD_KEYSTORE 是同服务名、账号 `cloud-clipboard-keystore` 的项，访问组都是 App Group；两个 item 的锁都是 App Group 的 `backend-account-refresh.lock`。
  - macOS：ACCOUNT 和 CLIPBOARD_KEYSTORE 是状态目录里的 `account-session.json`、`cloud-clipboard-keystore`（仅本人可读写、原子替换），ACCOUNT 的锁是旁边的 `account-refresh.lock`，ANONYMOUS 的锁是 App Group 的 `backend-account-refresh.lock`。
  - 两个平台：ANONYMOUS 和 ANONYMOUS_IDENTITY 是 App Group 的 `anonymous-session.json`、`anonymous-account.json`，读写持 `backend-local-store.lock`；身份是「不存在才写」，已存在返回 1。
- 回调的语义：钥匙串只有 `errSecItemNotFound` 是 0（不存在），-34018、`errSecInteractionNotAllowed` 和其他错误都是 -1；文件读不了、权限不对、没有 App Group 容器同样是 -1，不当作不存在。密钥串读出时去掉末尾的换行。锁用 `flock`，每次新开一个描述符、轮询等到期限，所以同一进程里仍用 `BackendFileRefreshLock` 的 Swift 代码也被排斥；持锁期间按宿主给的 `backgroundActivity` 保持后台执行（iOS App 用 `beginBackgroundTask`）。这个参数在 `BackendRustSessionStore.init`、`BackendRustAccount.register` 和 `registerShared` 上都没有默认值，宿主必须明确交一个或者 nil（macOS 交 nil），iOS 漏交时编译期就能看出来，不会悄悄退回没有后台执行。注册失败时存储自己放掉交给 Rust 的那次 retain，成功时由 `release` 回调放。
- 这一组只有这些接线，没有页面改用 Rust；注册和调用方的切换在第 4b 步以后。

## Alternatives considered

- **macOS 的 dylib 直接链接 Rust 静态库，Swift 直接调 C 函数。** 最强的理由是不用表，Swift 写法和 iOS 一样。不行：输入法可执行文件已经链着同一个静态库，dylib 再链一份会在一个进程里有两份 Rust 全局状态（两个会话存储注册表、两套 HTTP 客户端和进程内锁），同一份存储在两边各注册一次正是头文件禁止的；dylib 也要再带一份 Rust 代码。
- **用 `@_silgen_name` 在 Swift 里声明 C 函数和结构体，不要 module map。** iOS 里已经有这种写法，理由是不动构建配置。不行：注入存储要按 C 的内存布局填 `msime_client_session_store`，Swift 结构体不保证 C 布局，手写一份等于第二份头文件；macOS 的 dylib 里引用这些符号还会链接失败。
- **把每个 C 入口当成一个 `@_cdecl` 安装函数的参数逐个交过去**（`MSIMEInstallCloudClipboardDriver` 的写法）。理由是现成。不行：二十个参数没有版本，加一个入口就得改签名、新旧二进制混用时没法发现；一张带 `version` 和 `size` 的表可以拒收。
- **钥匙串读不了时照旧当作没有会话**（`BackendKeychain.load` 现在的做法）。理由是那样「我登录了吗」这类页面不会整页报错。不行：Rust 会把「没有会话」当作已退出、去注册匿名账号或清本地状态；头文件要求失败返回负数，Rust 报 `account_storage`，页面在第 4b 步自己决定怎么显示（迁移差异 M41）。

## Consequences

- **收益**：`swift test` 和 macOS 的 dylib 不需要 Rust 就能编译和测试注入存储的全部回调；Rust 与还没迁移的 Swift 用同一组文件和锁，第 4b 步以后可以逐页切换；云剪贴板驱动不再有单独的安装入口。
- **代价**：多了一个 C 头文件和一份 module map，四处 Swift 编译（SwiftPM、`build_backend_swift.sh`、`test_backend_account_swift.sh`、`typecheck_backend_swift.sh`）和 iOS 工程都要带 `-I shared/backend/rust`，CMake 里显式列源文件的 Swift 测试用到 `BackendRustClient.swift` 时也要带；表要跟着头文件加字段并升版本。
- **之后的切换**：macOS 的页面怎样改调 Rust、哪些故意没切，见 [macOS 原生账号页与云端窗口改调 Rust](2026-10-18-macos-account-through-rust.md)。
- **还没做的**：键盘扩展不注册存储（负责人决定：没有真机量内存，扩展保留 Swift 的 `BackendAccountSession`，与 Rust 共用 App Group 的锁，见 #8201 迁移计划第 9 节）；`performExpiringActivity` 的那一半因此没有写。iOS 的 `beginBackgroundTask` 在第 4b 步注册时接上；它的到期回调不能替 Rust 放锁，持锁超过后台时间（约 30 秒）时进程仍可能被系统按 0xdead10cc 杀掉。

## Verification

- client-services 的 `a_cancel_that_arrives_before_begin_cancels_the_request`、`a_cancel_that_arrives_before_begin_cancels_the_generation` 和 `early_cancels` 的测试：先取消、后登记的 id 拿到的是已经取消的把手，记录只用一次、最多 64 条；让 `EarlyCancels::remember` 什么也不记时前两个失败。
- `swift test --package-path shared/backend`：`BackendRustClientTests`（表的拒收、信封、专用线程、恰好释放一次、流式对话与取消、AI 皮肤进度与取消）和 `BackendRustSessionStoreTests`（`standard` 布局的文件与锁、注册的结构体、`release` 与注册失败各放掉一次、四项的读写删、容量和读不了是失败、密钥串换行、身份只写一次、钥匙串状态码、与 `BackendFileRefreshLock` 在同一个文件上互斥、后台执行覆盖持锁期间），用替身 Rust 经 C 回调调用。把钥匙串的 -34018 改回「不存在」、把 ACCOUNT 的锁换成另一个文件时对应的用例失败。
- macOS ctest：`backend-rust-session-store` 对着真的 Rust 宿主库，Swift 写的 `account-session.json` 和匿名会话经回调读出（`status`、`msime_client_account_access_token` 不联网就拿到令牌，说明 2001 年起的秒数读对了），`forget` 删掉会话和密钥串；`backend-library-load` 检查真的 dylib 收下合格的表、拒收版本或大小不对的表（把安装改成总是收下时它失败）。
- iOS：`MSIMEServiceTests/RustSessionStoreTests` 在模拟器上用 `msime_rust_client_linked()` 注册，钥匙串是内存替身，检查同样的读出与 `forget`，以及 iOS 的 `standard` 布局。

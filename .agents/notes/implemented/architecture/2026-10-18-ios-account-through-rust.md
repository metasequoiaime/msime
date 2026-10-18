# Agent Note: iOS App 的账号、云端页面和云同步改调 Rust，键盘扩展保留 Swift 会话

Status: implemented

## Problem

iOS App（`platforms/ios/App`）的「我的」页、登录面板、资料页、社区、云剪贴板、云词库、AI 对话、AI 皮肤、打字统计上传和 QQ 群助手，都用 `shared/backend` 的 Swift 实现：`BackendAccountSession` 自己读写钥匙串里的会话、刷新令牌，`BackendAccountClient` 自己发 `/v1/auth`、社区、云剪贴板、云词库等请求，另外还有一份 Swift 的社区错误映射和资料解码。#8201 要求账号与云端逻辑只有 Rust 一份（`crates/host-api/include/msime_client.h`）。第 4a 步已经让 App 启动时装好 Rust 函数表、实现了注入的会话存储（[Apple 宿主经一张函数表调用 Rust](2026-10-18-apple-rust-client-table.md)），macOS 已经切过去（[macOS 原生账号页与云端窗口改调 Rust](2026-10-18-macos-account-through-rust.md)），这一篇记录 iOS App 怎样切、键盘扩展为什么不切。

App 和键盘扩展读写同一份会话：登录会话和云剪贴板密钥串在 App Group 访问组的钥匙串项里，匿名会话和身份在 App Group 的文件里，两项的刷新锁都是 App Group 的 `backend-account-refresh.lock`。Swift 的 `BackendAccountSession` 和 Rust 的注入存储用同一把 `flock`、每次在锁里重读，所以 App 换成 Rust 以后扩展仍可以留在 Swift。

## Decision

- 注册（4b）：App 启动时 `IOSRustAccount.install()` 装表并注册本进程的会话存储（布局 1、`SHARED`、`BackendRustStoreLayout.standard`），持锁期间用 `beginBackgroundTask` 保持后台执行，免得挂起时还拿着 App Group 的文件锁被系统杀掉（0xdead10cc）。client_agent 由 Rust 拼：平台 `ios`、App 的版本、`uname` 的机型标识（模拟器上是 `SIMULATOR_MODEL_IDENTIFIER`）、「iOS」加系统版本、本版本的 edition。启动时的匿名账号注册改调 `ensure_anonymous`，已经登录或读不了登录状态时不调。
- 账号（4b）：登录方式、验证码、Apple（`apple_challenge`、`apple_login`）、Google（`google_challenge`、`google_token_login`）、资料、改名、退出、退出所有设备、注销（`account_delete`）、清除失效登录（`forget`）都经 Rust；读不了登录状态是 `account_storage`，页面报原因并提供清除，不当作已退出。Rust 的登录和退出没有「发布新身份之前」的清理钩子，所以登录换了账号、退出、注销和清除以后由 Swift 取消原来那个账号排队中的词库快照（`SkinCommunityAPI.signIn`、`logout`、`clearExpiredLogin`），取消失败只记诊断。
- 社区与云端页面（4c）：键盘皮肤和词包、回复模板、举报经 `msime_client_community`（键盘皮肤的下载和举报没登录时由 Rust 用本机匿名账号）；云剪贴板经 `msime_client_account_cloud_clipboard`，每次带本机的设备名（P6），密钥串仍是钥匙串里与键盘共用的那一项；云词库的列表、增删改、导入、导出、整份备份与恢复经 `msime_client_cloud_dictionary`，中转目录是 App 自己临时目录下的 `msime-cloud-dictionary`；「应用到本机」由 Rust 下载并校验整份快照（`snapshot_export_file`），再照旧排进与键盘共用的 Swift 快照队列（`DictionarySnapshotQueue`），由键盘扩展激活。AI 对话改用流式的 `msime_client_account_chat_stream`，进页时读一次 `chat_models` 的默认模型；AI 皮肤改用 `msime_client_ai_skin_generate`；单套皮肤生成、打字统计上传（`typing_upload_*`）和 QQ 群助手（`qq_links`、`qq_link_redeem`、`qq_link_delete`）经 `msime_client_account`。
- 页面只按打开时的 user.id 绑定：C 上拿不到会话代次，请求前后各读一次 `status`，退出或换了号就丢掉结果；同一个账号的令牌轮换和重新登录不再作废页面。
- 与 macOS 共用的 `shared/backend-ui` 三页（词包与回复模板、完整词库目录、云端候选与排序）随 iOS 一起换成 Rust：`CommunityResourceAccount` 和两页自己的 `request` 都在请求前后核对打开时的账号；macOS 的云词库窗口不再给它们 Swift 会话的令牌。
- 云同步（4d）：自动云同步改由 Rust 的 `msime_client_cloud_sync` 编排，能力只开设置和无编码常用语。App 交出 App Group 里的键盘设置（`IOSCloudSettings.native`：方案、简繁、学习、主题和自定义键盘设计取共享文档里的，按键反馈取 App Group 的）和「社区皮肤试用中」；一轮要写本机时（`pending_commit`）先写 App Group 的按键反馈，再 `commit` 让 Rust 写共享文档，失败时恢复 App Group，成功后把文档抄回 App Group 的镜像。状态从 Swift 的 `CloudSync/state.json` 换成 Rust 的 `cloud-sync/state.json`，App 每次启动调一次 `import_legacy`；退出、注销后先 `sign_out` 再对齐。键盘扩展和 App 记改动都调 `mark_dirty`，扩展看开关只用 `peek`。手动的「设置同步」页上传经 `msime_client_account_ios_settings_export` 加 `preferences_put`（带修订号和读文档时的账号），应用经 `msime_client_account_ios_settings_apply` 的 plan、commit 两步。Rust 的 iOS 映射原来不认九键双拼，在这一组之前补上（`fix(client-services): iOS 设置映射认得九键双拼`）。
- 键盘扩展不注册（负责人决定：没有真机量内存，「未测，扩展保留 Swift」，计划第 9 节的默认路线）：扩展里的候选释义、云剪贴板和快照激活仍用 Swift 的会话，与 App 里的 Rust 共用同一把锁。4f 只把释义的请求和解读换成 Rust 的 sans-IO 入口 `msime_client_account_gloss`（request 拼请求体、parse 读回答），HTTP 和令牌照旧在 Swift，401 时 Swift 会话刷新一次、同一个账号里重试；云剪贴板照旧是 Swift 传输加 Rust 的端到端加密驱动。

## Alternatives considered

- **键盘扩展也注册 Rust 的会话存储。** 最强的理由是这样整份 Swift 会话可以删掉，App 和扩展只剩一个刷新方。没有采用：扩展的内存上限很紧，Rust 的 HTTP 客户端和会话对象常驻以后能不能过门槛要在最老的支持机型上量，没有真机时按计划的默认路线保留 Swift；共用的 `flock` 保证两边轮流刷新同一枚令牌不会互相吊销。
- **登录前先取消旧账号的快照，取消失败时不让登录。** 这是 Swift 原来的做法（`BackendAccountSession.signIn` 的 `replacingAccount`）。没有采用：Rust 的登录在一次调用里完成换号，宿主插不进「保存新会话之前」这一步；登录以后再取消，键盘在这之间看到的账号已经是新的，`DictionarySnapshotWorker` 按当前账号核对请求，不会把旧账号的快照应用到新账号上。
- **AI 对话和 AI 皮肤顺势对没登录的用户开放。** Rust 两个入口都会退回本机匿名账号，鸿蒙就是这样做的。没有采用：这是功能变化，iOS 这两页原来要求登录，这一组只做迁移；页面在调 Rust 之前先看 `status`，没登录照旧请用户登录。
- **云同步的状态文件留在 Swift，只把单轮的合并交给 Rust。** 理由是键盘扩展记改动不用再链 Rust 的状态机。没有采用：状态文件、节流、租约和提交是一套，拆开就是两份对「这一轮还算不算数」的理解；键盘扩展本来就链着 Rust 宿主库，`mark_dirty` 只读写状态文件、不读会话。
- **云剪贴板页的刷新逻辑留在 View 里。** 没有采用：抽成 `IOSCloudClipboard` 以后才能在 KeyboardTests 里对着替身函数表测设备名、按需读设备状态和账号绑定。

## Consequences

- **收益**：iOS App 的登录会话只由 Rust 刷新和改写，「我的设备」显示 iOS 和机型，注销需要最近登录时有明确提示，锁屏或权限问题读不了会话时不再被当作已退出，AI 对话边生成边显示、能停下来，限流时说清要等多久。
- **代价**：KeyboardTests 新增替身函数表（`platforms/ios/KeyboardTests/core/FakeRustTable.swift`），页面测试只断言发出的 action、绑定和提示，HTTP 和会话行为由 host-api 和 client-services 的测试覆盖；扩展仍有一份 Swift 会话实现。
- **删掉的**（4g 的 iOS 和 shared/backend 部分）：`shared/backend` 里 iOS App 和共用三页原来用的 Swift 传输——AI 候选（`BackendAiClient`）、AI 插画（`BackendSkinArtworkClient`）、常用语（`BackendPhrasesClient`）、打字统计上传与 QQ 群助手（`BackendTypingUploadClient`）、云同步策略（`BackendSyncPolicy`）、iOS 设置映射（`IOSPreferencePlan`），以及设置、候选调频、云词库、社区资源、整份快照、对话和 App 云剪贴板页的 HTTP 调用，只留页面还在解码的类型；iOS 里 AI 皮肤的 `msime_client_ai_skin_plan` 包装。对应的测试一起删掉或改写。键盘扩展还在用的 Swift 会话 actor、匿名账号、释义传输和云剪贴板的端到端加密传输留着；macOS「本机账号」卡片用的改名、资料和注销留着。
- **只留空文件的**：`BackendChatClient.swift` 的 Swift `translate` 在 4f 以后没人调用，已经删掉，只测它的 `BackendChatClientTests`、`BackendTranslationRetryTests` 一起删（会话的 401 刷新一次、换号不重试由 `BackendAccountSessionTests` 覆盖）。文件留着、只剩一段说明：Tauri 外壳 iOS 键盘扩展的生成工程逐文件列着它，那份工程本机重新生成不了（缺 Tauri 自己生成的目录）。
- **还没删的、待负责人同意**：macOS `src/cloud/` 和 `src/core/Account*` 里没人引用的 Objective-C 代码，计划第 4g 步写明单独一个提交、等负责人同意。
- **行为变化（4d）**：设置按键三方合并、常用语按正文三方合并，删掉的不再复活；云端被清空时多一个「关闭同步，保留本机」；同一个账号重新登录保留同步进度；调频设置开始同步，触感强度「跟随系统」也上传。
- **行为变化**：资料页只显示 Google 身份的验证邮箱，邮箱登录的账号显示「通过邮箱登录」；社区的服务端细码（例如「下载使用后才能评分」「最多发布 50 款」）被 Rust 归并成禁止、冲突两类，提示跟着变泛；退出时断网，本机已经退出，资料页照样退回并说明服务端没撤销成；登录换号时取消旧快照失败不再挡住登录。

## Verification

- KeyboardTests：`SkinCommunityAccountTests`（登录、资料、退出、注销、清除和快照取消）、`SkinCommunityTests`（社区 action、校验和提示）、`AISkinGenerationTests`、`RustCloudPagesTests`（云剪贴板、云词库、打字统计上传、QQ 群助手、AI 对话）。整套 MSIMEClientTests 在 iOS 27.0 模拟器上通过。
- `swift test --package-path shared/backend`：`BackendRustAccountTests` 新增挑战、资料、登录方式的解码和限流提示。
- macOS ctest `backend-account-swift`：共用三页的绑定、目录解码和候选的查询 action（`rustSharedCloudPages`）。
- 4d：KeyboardTests 的 `RustCloudSyncTests`（run 的请求、先写 App Group 再提交、提交失败恢复、云端被清空的选择、开关和退出、设置同步页的上传和应用），`SchemeDocumentSettingsTests`、`KeyboardSkinTests` 改为对着真的 Rust 导出核对上传的方案、简繁和主题；SharedTests 的 `CloudSyncStateTests`、`CloudSyncMarkingTests` 对着真的 Rust 状态文件测导入、记改动和抑制。Rust 侧 client-services 的 `account::ios_settings` 新增九键双拼的往返用例。
- 4f：KeyboardTests 的 `TranslationProviderTests` 对着真的 Rust 入口测释义请求体（ZH、大写目标语言）、一条坏值只让那一条没有释义、401 刷新一次后重试、条数不对整页失败。
- 守护脚本 `test-community-session-binding.py`、`test-community-cloud-import-credentials.py` 改成盯住新的绑定方式；`test-ios-cloud-clipboard-view-lifecycle.py`、`test-ios-cloud-dictionary-files-lifecycle.py`、`test-ios-cloud-dictionary-kind-lifecycle.py`、`test-ios-settings-sync-view-lifecycle.py`、`test-ios-special-file-readers.py`、`test-macos-cloud-dictionary-session-binding.py`、`test-chat-system-prompt-parity.py` 按 Rust 的 user.id 绑定和新文件位置重写，每个都临时改回源码确认过会失败。

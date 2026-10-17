# Agent Note: 打字统计可选上传到云端并绑定 QQ 群助手

Status: implemented

## Problem

水杉的 QQ 群助手（msime-cloud 的 QQ 机器人）要给群友回 `/stats`：今天打了多少字、速度、常用的输入方式、和上一周比。打字统计一直只存在本机的 `typing-statistics.json` 里，服务端没有任何一份，统计页也向用户承诺「只保存在本机，不联网」。机器人只能收到 QQ 群里的成员标识，不知道这个人是哪个水杉账号。

## Decision

上传是用户主动打开的开关，默认关闭，且要先登录。打字统计页的「上传到云端」打开后，客户端向 `PUT /v1/users/me/typing-statistics` 上传最近 7 个有字数的日子：每天的字数、活跃秒数、24 个小时桶、字符类别和输入来源的计数，以及全部历史的候选位次。上传内容只有计数，没有输入内容，也不含按键热力图、徽章、皮肤和语音时长。每次都重传这 7 天，服务端按设备和日期整行覆盖，漏传的日子下次自然补上。关掉开关时，先在本地关掉，再调用 `DELETE` 删掉云端副本；删除失败时开关仍然保持关闭，由页面提示用户。

构建、节流和状态都在 `crates/client-services/src/typing_statistics/upload.rs`，与账号会话无关：`TypingStatisticsStore::upload_if_due` 接收一个发送闭包。状态文件 `typing-statistics-upload.json` 和统计文件放在同一目录、共用同一把锁，里面记着开关、设备标识和上次上传时间。两次自动上传至少间隔 10 分钟，打开开关时不受这个间隔限制。设备标识是第一次打开时生成的随机 UUID，关闭后仍然保留，下次打开服务端会认作同一台设备；它不复用匿名使用统计的 `install_id`，因为那个标识承诺过不与账号关联。状态文件损坏时按关闭处理：宁可少传，也不在用户不知情时上传。读取统计时持文件锁；设置应用的上传和删除命令还持同一份进程内操作锁跨过网络请求，避免关闭时的 `DELETE` 与已经开始的 `PUT` 交错（详见[关闭上传时串行化云端操作](../../implemented/bug-fix/2026-10-17-typing-statistics-upload-race.md)）。统计文件本身仍可在网络请求期间照常记数。如果发送过程中用户关掉了开关，就不再写回上传时间。

设置应用没有常驻进程，所以不做定时上传。补传的时机有两个：应用启动时调一次，统计页挂载时调一次。没开开关、没登录或 10 分钟内已经传过时，宿主直接返回，不发请求。

账号绑定方向固定为「机器人发码，App 里输入」：群友在群里发 `/bind`，拿到一个 8 位、10 分钟有效、只能用一次的码，在账号页的「绑定 QQ 群助手」卡片里输入（`POST /v1/users/me/qq-links`）。两种失败映射成各自的错误码，页面分别说明：`account_qq_link_code_not_found` 表示码不对或已过期，`account_qq_link_limit` 表示已经绑了 10 个。

接入范围是三个桌面宿主的 Tauri 设置应用（`apps/desktop/src-tauri/src/platform/desktop/desktop_typing_upload.rs`）。页面通过 `TypingStatisticsClient.cloudUpload` 和 `AccountClient.qqLinks` 两个可选能力判断显示与否，宿主不提供时不显示。

原生宿主经 C ABI 复用同一份 Rust 逻辑，按宿主怎么联网分成两种形状：

- 用 Rust 账号会话的宿主（Windows 原生设置）走 `msime_client_account` 的新操作 `typing_upload_status`、`typing_upload_set`、`typing_upload`、`qq_links`、`qq_link_redeem`、`qq_link_delete`（`crates/host-api/src/account_ops.rs`），语义与 Tauri 命令逐条相同。每次调用都新建统计目录的 store，它自带的操作锁管不到别的调用，所以上传和关闭后的删除改用一把进程级的锁串行化。Windows 原生设置的「打字统计」页和「账号与同步」页各画了开关和卡片，设置窗口打开和进入统计页时各补传一次。
- 自己发 HTTP 的宿主（Android、iOS、鸿蒙）走 `msime_client_typing_statistics` 的 `upload_status`、`set_upload_enabled`、`upload_request`、`upload_succeeded`：Rust 交回要发的请求（或者不发的原因 `disabled`、`not_due`、`nothing`），宿主带登录令牌发出去，成功后再告诉 Rust 记下时间。为此 `upload_if_due` 拆成 `prepare_upload` 和 `record_upload` 两步，前者不记任何东西，后者只在开关还开着、设备标识没变时记。QQ 绑定在这些宿主上直接调服务端的三个接口，没有要放进 Rust 的状态。
- Android（`platforms/android`）：`home/TypingUpload` 把 `upload_request`、发送、`upload_succeeded` 和关闭后的 `DELETE` 放在一个单线程执行器上，`HomeActivity.onResume` 和打开打字统计页时各补传一次，节流仍由 Rust 判断；只有真实账号（`SyncSwitch.validLoginKind`）上传和绑定，匿名账号什么都不发。请求由 `account/TypingUploadApi` 带账号令牌发出，错误说法与共享设置页一致。打字统计页「概览」末尾加「云端」组的「上传到云端」开关，打开时页脚改写「只保存在本机」；「我的 → 同步」加「绑定 QQ 群助手」，进 `QqLinksPage` 输入绑定码、列出和解绑。
- 鸿蒙（`platforms/harmony`）：设置页通过 `Settings.ets` 的 `typing_upload` 桥接请求串行执行 `upload_request`、发送、`upload_succeeded` 和关闭后的 `DELETE`；只使用真实账号令牌，绑定 QQ 群助手走账号桥接的三个操作。共享界面在鸿蒙手机和 2in1 布局显示绑定卡片与上传开关，应用打开和回到前台时补传；请求体限制为 64 KiB。本机 `hvigorw assembleHap` 的 ArkTS 编译和逻辑测试通过，尚未在签名真机上验证。
- iOS 原生 App（`platforms/ios`）：`App/Sources/settings/TypingUpload` 是一个 actor，所有入口经一条依次等待的任务链串行（actor 在 `await` 处可以重入，只靠 actor 挡不住关闭后的删除和还在路上的上传交错）；应用回到前台（`MetasequoiaImeApp` 的 `scenePhase == .active`）和打开统计页时各补传一次，节流仍由 Rust 判断。只有 `BackendAccountSession.shared` 上的真实账号上传和绑定，匿名账号的会话是另一个，不碰它。`TypingStatisticsStore` 加了 `uploadStatus`、`setUploadEnabled`、`uploadRequest`、`uploadSucceeded` 四个调用；请求和 QQ 绑定由 `shared/backend/clients/BackendTypingUploadClient.swift` 带账号令牌发出，错误说法与共享设置页、Android 一致。统计页「概览」末尾加「云端」组的「上传到云端」开关，打开时页脚和「关于统计」里「只保存在本机」的说法随之改写；「我的 → 同步」加「绑定 QQ 群助手」，进 `QQLinksView` 输入绑定码、列出和解绑，没登录时这一行是灰的。键盘扩展不上传：它没有完全访问权限时连不上网，而且上传要的是真实账号的令牌。

## Alternatives considered

- **App 出码、在群里发给机器人**：最接近「扫码登录」的习惯，用户也不用在设置里找输入框。问题是群聊公开，码一发出来，别人就能抢先发给机器人，把自己的 QQ 绑到受害者的账号上，读到受害者的打字数据。反过来由机器人出码，看到码的人最多只能把别人的 QQ 绑到自己的账号上，读到的仍是自己的数据。
- **只传当天，按增量累加**：请求最小。但设置应用不常开，传漏的日子再也补不上，而且服务端的累加没法幂等，重试会重复计数。整行覆盖最近 7 天天然幂等。
- **复用遥测的 `install_id` 作为设备标识**：不用新增状态。但 PRIVACY.md 写明 `install_id` 是匿名的、不和账号关联，复用它等于把匿名使用统计和账号打通。
- **从输入法进程在后台上传**：可以不依赖设置应用是否打开，数据最新。但这要在六个原生宿主里各接一套带账号令牌的网络调用，牵涉面远超一个开关，这次不做。

## Consequences

收益：群助手可以读到打开了上传的用户的打字数据，统计页的说明也随开关改写，不再在上传开着时声称「只保存在本机」。

代价：这是一条新的、带账号令牌的上传路径，PRIVACY.md 已经加了「打字统计上传」一节。上传只在设置应用或 App 打开时发生，长期不开设置应用或 App 的用户，群助手看到的数据会停在最后一次上传。Windows 原生设置、Android、iOS 和鸿蒙已经接上，macOS 原生仍只有 C ABI，尚未接入界面和发送。Android、iOS 和鸿蒙只在应用回到前台和打开统计页时上传，键盘进程不上传，长期只用键盘、不开应用的用户同样停在最后一次上传。两步形状把串行化交给了宿主：`upload_request`、发送、`upload_succeeded` 和关闭后的 `DELETE` 要放在同一个串行队列里，否则关闭时的删除可能先于还在路上的上传到达服务端，`record_upload` 只能保证本地状态不被写回。统计模型的分层见 [打字统计契约](../../implemented/architecture/2026-10-15-typing-statistics-model.md)，这里只在模型上加了 `cloudUpload` 这个可选能力。

## Verification

- `cargo test -p msime-client-services`：覆盖构建（7 天上限、60 天窗口、键名过滤和 32 个上限、24 桶校验）、状态（默认关闭、节流、发送失败不记时间、发送中被关掉不写回、坏文件读成关闭）、绑定码和编号的形状、列表解析，以及两个错误码的映射。
- `cargo clippy -p msime-desktop --all-targets -D warnings`：在 macOS 上跑，用空的资源目录占位，构建脚本只检查这些目录存在。
- vitest 覆盖开关的打开、关闭、删除失败和未登录时禁用，覆盖说明文字随开关改写，也覆盖绑定卡片的空格处理、绑定与解绑和错误文案。
- `cargo test -p msime-client-services typing_statistics::upload`：两步上传只为准备时的设备记时间，准备本身不记，关闭后不写回。
- `cargo test -p msime-host-api`：`account_ops` 的打开即上传、十分钟节流、`force`、关闭先本地后删除且删除失败仍关闭、未登录时不退回匿名账号、请求不合法时不联网，QQ 绑定的列表、绑定码清洗、服务端错误码透传；`msime_client_typing_statistics` 的 `upload_request` 各种跳过原因和 `upload_succeeded` 只认同一设备。
- Windows 原生设置的两个模型头（`AccountPageModel.h`、`TypingStatisticsModel.h`）的单测在 macOS 上用 clang 编译跑过；`main.cpp` 没有在本机编译，靠 PR 的 Windows CI。
- iOS：`swift test --package-path shared/backend --filter BackendTypingUploadTests` 覆盖只发往唯一路径、空请求体不发、绑定码清洗后才联网、列表解析和各种错误说法；`KeyboardTests/settings/TypingStatisticsTests.testUploadRequestsComeFromTheSharedStore` 用真的共享层覆盖没打开不发、没字数不发、请求体带 `ios`、成功后十分钟内不发而 `force` 照发、关闭后不发，由 PR 的 iOS Simulator CI 运行；App 的 Swift 源码在本机按 iOS 17 模拟器 SDK 做过类型检查，没有在模拟器上运行。
- 没有在真实账号上连通线上服务端验证。

- 鸿蒙：`platforms/harmony/tests/account-qq-typing.test.ts` 用假传输层覆盖绑定列表只取两个字段、坏行整份拒绝、绑定码清洗和不合法时不联网、拒绝码映射、解绑编号校验，以及上传原样发出、超限本地拒绝、未登录不发、关闭即删除；vitest 覆盖鸿蒙手机和 2in1 账号页的绑定卡片、鸿蒙外观统计页的开关和说明改写。本机 `hvigorw assembleHap`（先 `ohpm install`）ArkTS 编译通过，没带原生库、没上设备；`Settings.ets` 的串行队列没有单测，真机和真实账号还没验证。

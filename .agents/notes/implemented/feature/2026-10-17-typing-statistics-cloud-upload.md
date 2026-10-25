# Agent Note: 打字统计可选上传到云端并绑定 QQ 群助手

Status: implemented

## Problem

水杉的 QQ 群助手（msime-cloud 的 QQ 机器人）要给群友回 `/stats`：今天打了多少字、速度、常用的输入方式、和上一周比。打字统计一直只存在本机的 `typing-statistics.json` 里，服务端没有任何一份，统计页也向用户承诺「只保存在本机，不联网」。机器人只能收到 QQ 群里的成员标识，不知道这个人是哪个水杉账号。

## Decision

上传是用户主动打开的开关，默认关闭，且要先登录。打字统计页的「上传到云端」打开后，客户端向 `PUT /v1/users/me/typing-statistics` 上传最近 7 个有字数的日子：每天的字数、活跃秒数、24 个小时桶、字符类别和输入来源的计数，以及全部历史的候选位次。上传内容只有计数，没有输入内容，也不含按键热力图、徽章、皮肤和语音时长。每次都重传这 7 天，服务端按设备和日期整行覆盖，漏传的日子下次自然补上。关掉开关时，先在本地关掉，再调用 `DELETE` 删掉云端副本；删除失败时开关仍然保持关闭，由页面提示用户。

构建、节流和状态都在 `crates/client-services/src/typing_statistics/upload.rs`，与账号会话无关：`TypingStatisticsStore::upload_if_due` 接收一个发送闭包。状态文件 `typing-statistics-upload.json` 和统计文件放在同一目录、共用同一把锁，里面记着开关、设备标识和上次上传时间。两次自动上传在墙上时钟正常前进时至少间隔 10 分钟，打开开关时不受这个间隔限制。节流比较保留时钟回拨这个分支：当前 Unix 毫秒小于上次成功时间时允许补传，发送成功后用修正后的时间重新计间隔，发送失败仍不记时间。否则 `saturating_sub` 会把回拨算作零间隔，重启应用也要等旧时间被追上之后才能补传。[Rust 的 `SystemTime` 文档](https://doc.rust-lang.org/std/time/struct.SystemTime.html)明确系统时钟并不单调；这里保留 Unix 毫秒，因为这个状态要跨宿主和进程重启持久化。设备标识是第一次打开时生成的随机 UUID，关闭后仍然保留，下次打开服务端会认作同一台设备；它不复用匿名使用统计的 `install_id`，因为那个标识承诺过不与账号关联。状态文件损坏时按关闭处理：宁可少传，也不在用户不知情时上传。读取统计时持文件锁；设置应用的上传和删除命令还持同一份进程内操作锁跨过网络请求，避免关闭时的 `DELETE` 与已经开始的 `PUT` 交错（详见[关闭上传时串行化云端操作](../../implemented/bug-fix/2026-10-17-typing-statistics-upload-race.md)）。统计文件本身仍可在网络请求期间照常记数。如果发送过程中用户关掉了开关，就不再写回上传时间。

设置应用没有常驻进程，所以不做定时上传。补传的时机有两个：应用启动时调一次，统计页挂载时调一次。没开开关、没登录或同一条向前走的墙上时钟上 10 分钟内已经传过时，宿主直接返回，不发请求。

共享界面的上传状态由 `useTypingStatisticsUpload` 管理，包入口公开 hook、选项和状态类型，`TypingStatisticsUpload` 只绘制开关、时间和提示。hook 复用 `useAsyncActionRunner` 的同步互斥、客户端代次与卸载保护，避免短时间重复切换；客户端变化时清掉旧状态和提示，并释放旧操作的 UI 忙碌状态。自动补传失败时静默读一次开关，手动变更失败时保留错误说明并补读实际状态；每次异步返回都检查归属，旧客户端、旧挂载以及手动变更之前的自动补传结果不覆盖当前状态，也不通知页面。回调更换不重新补传，结果交给最新 `onEnabledChange`。这些约束修复旧组件中失败补读缺少守卫、客户端替换后忙碌状态不复位的问题；宿主已经发出的请求仍由原有 Rust 串行化逻辑完成。

账号绑定方向固定为「机器人发码，App 里输入」：群友在群里发 `/bind`，拿到一个 8 位、10 分钟有效、只能用一次的码，在账号页的「绑定 QQ 群助手」卡片里输入（`POST /v1/users/me/qq-links`）。两种失败映射成各自的错误码，页面分别说明：`account_qq_link_code_not_found` 表示码不对或已过期，`account_qq_link_limit` 表示已经绑了 10 个。

共享设置页的触屏布局将 QQ 群助手放在「我的 → 同步 → 绑定 QQ 群助手」二级页，与 Android 的导航层级一致；首页只显示跳转行，进入后才读取绑定。鸿蒙未登录时入口禁用并说明「登录后可用」，宿主未提供 `qqLinks` 时不显示。页面复用 `accountSubpage` 历史记录，页头返回按钮与系统返回手势都回到「我的」，恢复历史记录也能回到绑定页；2in1 和其他桌面布局继续显示账号页卡片。

共享设置页根据已读取的绑定列表展示卡片：空列表显示绑定说明和表单，已有绑定时默认只显示使用说明、绑定时间与解绑操作；「绑定其他 QQ」主动展开表单，取消或绑定成功后收起，以保留多绑定能力。读取期间不先显示未绑定表单，读取失败提供重试，不把未知状态当作未绑定；解绑最后一项后恢复表单。这里修正的是共享页面展示，原生设置界面和服务端绑定上限保持原有契约。

接入范围是三个桌面宿主的 Tauri 设置应用（`apps/desktop/src-tauri/src/platform/desktop/desktop_typing_upload.rs`）。页面通过 `TypingStatisticsClient.cloudUpload` 和 `AccountClient.qqLinks` 两个可选能力判断显示与否，宿主不提供时不显示。

原生宿主经 C ABI 复用同一份 Rust 逻辑，按宿主怎么联网分成两种形状：

- 用 Rust 账号会话的宿主（Windows 原生设置）走 `msime_client_account` 的新操作 `typing_upload_status`、`typing_upload_set`、`typing_upload`、`qq_links`、`qq_link_redeem`、`qq_link_delete`（`crates/host-api/src/account_ops.rs`），语义与 Tauri 命令逐条相同。每次调用都新建统计目录的 store，它自带的操作锁管不到别的调用，所以上传和关闭后的删除改用一把进程级的锁串行化。Windows 原生设置的「打字统计」页和「账号与同步」页各画了开关和卡片，设置窗口打开和进入统计页时各补传一次。
- 自己发 HTTP 的宿主（Android、iOS、鸿蒙）走 `msime_client_typing_statistics` 的 `upload_status`、`set_upload_enabled`、`upload_request`、`upload_succeeded`：Rust 交回要发的请求（或者不发的原因 `disabled`、`not_due`、`nothing`），宿主带登录令牌发出去，成功后再告诉 Rust 记下时间。为此 `upload_if_due` 拆成 `prepare_upload` 和 `record_upload` 两步，前者不记任何东西，后者只在开关还开着、设备标识没变时记。QQ 绑定在这些宿主上直接调服务端的三个接口，没有要放进 Rust 的状态。
- Android（`platforms/android`）：`home/TypingUpload` 把 `upload_request`、发送、`upload_succeeded` 和关闭后的 `DELETE` 放在一个单线程执行器上，`HomeActivity.onResume` 和打开打字统计页时各补传一次，节流仍由 Rust 判断；只有真实账号（`SyncSwitch.validLoginKind`）上传和绑定，匿名账号什么都不发。请求由 `account/TypingUploadApi` 带账号令牌发出，错误说法与共享设置页一致。打字统计页「概览」末尾加「云端」组的「上传到云端」开关，打开时页脚改写「只保存在本机」；「我的 → 同步」加「绑定 QQ 群助手」，进 `QqLinksPage` 输入绑定码、列出和解绑。
- 鸿蒙（`platforms/harmony`）：设置页通过 `Settings.ets` 的 `typing_upload` 桥接请求串行执行 `upload_request`、发送、`upload_succeeded` 和关闭后的 `DELETE`；只使用真实账号令牌，绑定 QQ 群助手走账号桥接的三个操作。共享界面在鸿蒙手机以二级页、2in1 以账号页卡片提供绑定，并保留上传开关，应用打开和回到前台时补传；请求体限制为 64 KiB。本机 `hvigorw assembleHap` 的 ArkTS 编译和逻辑测试通过，尚未在签名真机上验证。
- iOS 原生 App（`platforms/ios`）：`App/Sources/settings/TypingUpload` 是一个 actor，所有入口经一条依次等待的任务链串行（actor 在 `await` 处可以重入，只靠 actor 挡不住关闭后的删除和还在路上的上传交错）；应用回到前台（`MetasequoiaImeApp` 的 `scenePhase == .active`）和打开统计页时各补传一次，节流仍由 Rust 判断。只有 `BackendAccountSession.shared` 上的真实账号上传和绑定，匿名账号的会话是另一个，不碰它。`TypingStatisticsStore` 加了 `uploadStatus`、`setUploadEnabled`、`uploadRequest`、`uploadSucceeded` 四个调用；请求和 QQ 绑定由 `shared/backend/clients/BackendTypingUploadClient.swift` 带账号令牌发出，错误说法与共享设置页、Android 一致。统计页「概览」末尾加「云端」组的「上传到云端」开关，打开时页脚和「关于统计」里「只保存在本机」的说法随之改写；「我的 → 同步」加「绑定 QQ 群助手」，进 `QQLinksView` 输入绑定码、列出和解绑，没登录时这一行是灰的。键盘扩展不上传：它没有完全访问权限时连不上网，而且上传要的是真实账号的令牌。

## Alternatives considered

- **节流直接改用 `Instant`**：测量进程内经过时间时不受系统校时影响。但上传状态要跨应用重启和不同宿主保存，单调时钟没有可持久化的统一纪元；为这个边界引入第二份进程内时钟会扩大状态契约。继续保存 Unix 毫秒，只在发现回拨时让旧间隔失效。

- **App 出码、在群里发给机器人**：最接近「扫码登录」的习惯，用户也不用在设置里找输入框。问题是群聊公开，码一发出来，别人就能抢先发给机器人，把自己的 QQ 绑到受害者的账号上，读到受害者的打字数据。反过来由机器人出码，看到码的人最多只能把别人的 QQ 绑到自己的账号上，读到的仍是自己的数据。
- **只传当天，按增量累加**：请求最小。但设置应用不常开，传漏的日子再也补不上，而且服务端的累加没法幂等，重试会重复计数。整行覆盖最近 7 天天然幂等。
- **复用遥测的 `install_id` 作为设备标识**：不用新增状态。但 PRIVACY.md 写明 `install_id` 是匿名的、不和账号关联，复用它等于把匿名使用统计和账号打通。
- **从输入法进程在后台上传**：可以不依赖设置应用是否打开，数据最新。但这要在六个原生宿主里各接一套带账号令牌的网络调用，牵涉面远超一个开关，这次不做。
- **上传状态继续写在视图内部**：组件内部就能复用开关 UI，调用链最短。但其他共享界面不能独立使用补传和开关状态，异步守卫也容易在各个返回分支漏掉；把状态收进公共 hook，互斥和生命周期继续复用现有 action runner。

## Consequences

收益：群助手可以读到打开了上传的用户的打字数据，统计页的说明也随开关改写，不再在上传开着时声称「只保存在本机」。

代价：这是一条新的、带账号令牌的上传路径，PRIVACY.md 已经加了「打字统计上传」一节。上传只在设置应用或 App 打开时发生，长期不开设置应用或 App 的用户，群助手看到的数据会停在最后一次上传。Windows 原生设置、Android、iOS 和鸿蒙已经接上，macOS 原生仍只有 C ABI，尚未接入界面和发送。Android、iOS 和鸿蒙只在应用回到前台和打开统计页时上传，键盘进程不上传，长期只用键盘、不开应用的用户同样停在最后一次上传。两步形状把串行化交给了宿主：`upload_request`、发送、`upload_succeeded` 和关闭后的 `DELETE` 要放在同一个串行队列里，否则关闭时的删除可能先于还在路上的上传到达服务端，`record_upload` 只能保证本地状态不被写回。统计模型的分层见 [打字统计契约](../../implemented/architecture/2026-10-15-typing-statistics-model.md)，这里只在模型上加了 `cloudUpload` 这个可选能力。

## Verification

- `cargo test -p msime-client-services`：覆盖构建（7 天上限、60 天窗口、键名过滤和 32 个上限、24 桶校验）、状态（默认关闭、节流、发送失败不记时间、发送中被关掉不写回、坏文件读成关闭）、绑定码和编号的形状、列表解析，以及两个错误码的映射。
- `cargo clippy -p msime-desktop --all-targets -D warnings`：在 macOS 上跑，用空的资源目录占位，构建脚本只检查这些目录存在。
- vitest 覆盖开关的打开、关闭、删除失败和未登录时禁用，覆盖说明文字随开关改写，也覆盖绑定卡片的空格处理、绑定与解绑和错误文案。
- `use-typing-statistics-upload.test.tsx` 与上传组件回归覆盖静默补传与失败补读、最新回调、同轮防重、错误码、客户端替换和卸载后的迟到结果、旧操作不释放新操作的忙碌锁，以及 StrictMode 的代次隔离。三个组件失败场景在抽取前先复现，再验证修复；该 hook 的抽取不改变原生存储或网络协议。
- 账号页回归覆盖触屏入口进入前不请求绑定、二级页表单与已有绑定、页头返回、系统返回、历史恢复、未登录禁用、缺少宿主能力隐藏，以及 2in1 保留卡片。
- 绑定卡片回归还覆盖已绑定时隐藏初次绑定表单、读取期间不闪现表单、主动追加绑定及取消、绑定成功收起、解绑最后一项后恢复，以及读取失败重试。
- `cargo test -p msime-client-services typing_statistics::upload`：两步上传只为准备时的设备记时间，准备本身不记，关闭后不写回。`automatic_upload_recovers_after_the_wall_clock_moves_backwards` 先通过公开入口保存成功时间，再重建 store 并把合成时钟回拨一周；旧实现返回 `NotDue` 而失败。修复后两步准备和闭包发送都恢复，失败不改时间，成功重置间隔，并检查十分钟前一毫秒、恰好十分钟与关闭后的边界。测试不修改系统时钟、不发网络请求。
- `cargo test -p msime-host-api`：`account_ops` 的打开即上传、十分钟节流、`force`、关闭先本地后删除且删除失败仍关闭、未登录时不退回匿名账号、请求不合法时不联网，QQ 绑定的列表、绑定码清洗、服务端错误码透传；`msime_client_typing_statistics` 的 `upload_request` 各种跳过原因和 `upload_succeeded` 只认同一设备。
- Windows 原生设置的两个模型头（`AccountPageModel.h`、`TypingStatisticsModel.h`）的单测在 macOS 上用 clang 编译跑过；`main.cpp` 没有在本机编译，靠 PR 的 Windows CI。
- iOS：`swift test --package-path shared/backend --filter BackendTypingUploadTests` 覆盖只发往唯一路径、空请求体不发、绑定码清洗后才联网、列表解析和各种错误说法；`KeyboardTests/settings/TypingStatisticsTests.testUploadRequestsComeFromTheSharedStore` 用真的共享层覆盖没打开不发、没字数不发、请求体带 `ios`、成功后十分钟内不发而 `force` 照发、关闭后不发，由 PR 的 iOS Simulator CI 运行；App 的 Swift 源码在本机按 iOS 17 模拟器 SDK 做过类型检查，没有在模拟器上运行。
- 没有在真实账号上连通线上服务端验证。

- 鸿蒙（#8201 第 2 步以后这个测试连同 ArkTS 账号桥接一起删掉，绑定与上传改由 Rust 的 `qq_*`、`typing_upload_*` 发出）：`platforms/harmony/tests/account-qq-typing.test.ts` 用假传输层覆盖绑定列表只取两个字段、坏行整份拒绝、绑定码清洗和不合法时不联网、拒绝码映射、解绑编号校验，以及上传原样发出、超限本地拒绝、未登录不发、关闭即删除；vitest 覆盖鸿蒙手机的绑定二级页和 2in1 账号页的绑定卡片、鸿蒙外观统计页的开关和说明改写。本机 `hvigorw assembleHap`（先 `ohpm install`）ArkTS 编译通过，没带原生库、没上设备；`Settings.ets` 的串行队列没有单测，真机和真实账号还没验证。

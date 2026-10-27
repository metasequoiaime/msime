# Agent Note: iOS 原生宿主接入云剪贴板的自建中继和 WebDAV 网盘

Status: implemented

## Problem

[自建中继](2026-10-12-cloud-clipboard-relay.md) 和 [WebDAV 网盘](2026-10-26-cloud-clipboard-webdav.md) 的 Rust 客户端、C ABI 和桌面、Android 的设置界面都已落地，iOS 原生宿主却只有水杉账号一条路：App 的云剪贴板页经 `msime_client_account_cloud_clipboard`，键盘扩展的「云端」经 Swift 的账号会话，「我的 → 云剪贴板」没有登录时根本进不去。配置了中继或网盘的偏好（例如经设置文件以外的途径写进共享文档）在 iOS 上也没有任何入口认得，内容照样去 `api.msime.app`。

iOS 的特殊处在于 App 和键盘扩展是两个进程，共用 App Group 容器和 App Group 访问组的钥匙串；账号路径的设备密钥串已经放在钥匙串里。同一时期维护者报告过一台手机在云剪贴板上出现三条待批准请求，根因是同一个安装生成了不止一个设备身份——新加的路径不能再制造这种情况。

## Decision

- **与 Android 同一种做法：Swift 跑驱动、钥匙串存密钥串、Rust 只发请求**：端到端加密的操作（列表、上传、设备管理）由 Swift 跑与账号路径同一个驱动循环（`BackendAccountClient.clipboardE2E(_:e2e:send:)`，`shared/backend` 里把原来的循环抽成只换传输的静态函数，账号路径的行为不变），每个请求经 `msime_client_relay_cloud_clipboard_exchange` 发出；开关、置顶、保留、删除和清空经 `msime_client_relay_cloud_clipboard`。地址校验、路径前缀、令牌、网盘的虚拟中继、日志缓存和锁全在 Rust 里，Swift 不写一行 HTTP 或 WebDAV。实现在 `platforms/ios/SharedUI/core/CloudClipboardRelay.swift`，App 和键盘扩展都编译它；键盘扩展不编译 `BackendRustClient.swift`，所以它直接经 `@_silgen_name` 调四个 C 入口，自己在专用线程上调用、拆信封。
- **密钥串是钥匙串里的另一项，App 和键盘共用**：service `app.msime.backend.account`、account `cloud-clipboard-relay-keystore`、App Group 访问组（与账号那份同组，两个进程原本就有这个 entitlement，不新增），`AfterFirstUnlockThisDeviceOnly`。内容是 `{binding, keystore}`，`binding` 与 Android 的绑定同一规则（中继是地址和令牌的摘要，网盘是地址和账号的摘要，不含应用密码），绑定不同就读作没有、写入时覆盖；退出水杉账号不删它（`BackendKeychain.clear` 只删账号那两项）。
- **一个安装、一个中继只有一个设备身份**：共享契约是「密钥串的保存从不覆盖另一个已有的身份」（与重复批准请求的修复 #10032 同一条规则，`BackendClipboardE2E.saveKeystore` 发现另一个身份时不写、交回存着的那份）。中继这份没有会话锁可借，`IOSCloudClipboardRelayKeystore.save` 自己在 App Group 容器的 `cloud-clipboard-relay/cloud-clipboard-relay-keystore.lock` 的 flock 里读、比、写，同一个中继里已经存着另一个身份（`backendClipboardKeystoreIdentity` 那一段不同）就以 `BackendClipboardKeystoreConflict` 拒绝，App 和键盘拿的是同一把锁；驱动循环在还没发出任何请求时改用存着的身份从头再来，发出请求之后再冲突就失败（`clipboard_keystore_conflict`），不再换身份。钥匙串读不出来（设备锁着、缺权限）抛出 `relay_storage`，不当作没有，否则会生成新身份、在中继上多登记一台待批准的设备。驱动只在中继的设备列表里找不到本机时才发登记请求，身份不变，刷新和重试就不会重复登记。
- **网盘日志的本机缓存在 App Group 容器的 `cloud-clipboard-relay/`**：`exchange` 和直接动作传同一个目录，App 和键盘看到同一份缓存、同一把 `cloud-clipboard-relay.lock`。目录建成 0700 并排除在设备备份之外（与 Android 放 `noBackupFilesDir` 同样考虑）。缓存里只有密文和公开数据。
- **选路以共享层为准，从不退回账号**：App 的云剪贴板页每次刷新先问 `msime_client_cloud_clipboard_source`，选了中继就只经 `IOSCloudClipboardRelay`；账号路径的请求带上偏好目录，Rust 发出前再确认一次（这时已改成中继是 `clipboard_source_changed`）。页面按第一次读到的那一方绑定（账号的 user.id，或中继的绑定），操作前后都核对，换了就作废。键盘的 `KeyboardCloudClipboardBinding` 多一个 `relay` 绑定：读列表、插入和上传之前都核对同步方式和绑定还是读列表时那一份，不在配置了中继时经账号读写，也不把一条记录发到刚换掉的中继。账号页「云剪贴板」那一行配置了中继时不显示条数。
- **「我的 → 云剪贴板」没有登录也能进**：页面顶部是「云剪贴板同步方式」卡片（`CloudClipboardSyncModeView.swift`）：同步方式用设计稿的选项面板选，中继和网盘各有一个填写页。做法和说法与 Android 和共享设置页相同：令牌和应用密码不回显、留空沿用，地址和凭据按共享规则当场提示（`IOSCloudClipboardRelayPolicy`，规则照抄 Rust 和共享页，只为当场说明，保存时 Rust 校验整份偏好）；网盘「测试连接」只测不存，「保存」先测、通过才写 `cloud_clipboard_webdav` 并删掉中继，不通过时填写页和草稿都留着；改回水杉账号两者都删，网盘和中继上的内容、本机的中继密钥串都不动。走账号而没有登录时，内容那一块换成一张说明卡和「登录水杉账号」按钮。
- **错误码的说法取自共享设置页**：`relay_unauthorized`、`relay_unavailable`、`webdav_rate_limited`、`webdav_rollback`、`webdav_unsupported` 等用 `cloud-clipboard-send.ts` 的句子，只把「设置的『剪贴板』页」换成「我的 → 云剪贴板」；测试连接的说法与 `cloudClipboardWebDavProbeMessage` 逐字相同；驱动的码（`clipboard_device_pending` 等）沿用 iOS 账号路径原有的句子。驱动报的错先按 Android 和 Rust 同一张已知码表归类（`driverFailureCode`）。设备批准、核对码、回退检测都来自 Rust，宿主不另做判断。

## HarmonyOS 要做的同一件事

移动端界面要三端一致。HarmonyOS 原生宿主接入时需要：设置页（ArkWeb 里的共享设置页已经有这一组，宿主要提供 `cloud_clipboard_webdav_probe` 那样的测试连接桥）、键盘「云端」先问 `msime_client_cloud_clipboard_source`；密钥串与账号那份同等保护、设置应用和键盘共用一份，第一次同时使用时只留一个身份；网盘缓存目录两个进程相同；配置了中继时不经账号、不为条数去问账号。这些都只在宿主一侧，Rust 不用改。

## Alternatives considered

- **整个操作交给 `msime_client_relay_cloud_clipboard`，Rust 把密钥串存成 App Group 容器里的 0600 文件并排除备份** — 这是中继笔记当初写下的 iOS 设想，最强的理由是一份实现：锁、核对中继空间、拒绝别人可读的文件都在 Rust 里，与桌面端完全相同，Swift 只拼请求，App 和键盘传同一个目录就是同一台设备。不用：它弱于 iOS 账号路径（钥匙串）和 Android 中继路径（AndroidKeyStore），拿到容器文件就拿到设备私钥和剪贴板密钥；Android 评审时维护者已经要求中继的密钥串不弱于账号那份。Swift 里早有账号路径那套驱动循环，换成它只是把传输换成 `exchange`。
- **把 `exchange` 和 `webdav_probe` 加进 `MSIMERustClient.h` 的函数表，经 `BackendRustClient` 调用** — App 已经经这张表调 Rust，加两项就能复用它的线程和信封。不用：表是 macOS 输入法和 iOS 共用的版本化结构，改它要同步 macOS 的装表和尺寸检查；键盘扩展不装这张表，仍然要另一条路，两条路反而更乱。直接 `@_silgen_name` 是 `CloudClipboardDriver.swift` 已有的做法。
- **钥匙串项不带绑定，换中继时由设置页删掉旧项** — 少一层 JSON。不用：键盘可能在设置页删除之前写回旧中继的身份；带绑定之后谁读到不符的都当作没有，与 Android 和 Rust 那份同样的规则，不依赖删除的时机。

## Consequences

- **收益**：iOS 用户不登录水杉账号也能用云剪贴板，App 和键盘只连用户的中继或网盘，内容只以密文离开设备；密钥串与账号那份同等保护。共享层（Rust、C ABI、参考中继）没有改动。同一安装在一个中继里只登记一台设备，有测试锁住。
- **代价**：iOS 的中继有两条代码路径（端到端加密的操作走 Swift 循环加 `exchange`，直接动作走 Rust 的整操作入口），与 Android 相同。地址和凭据的当场提示规则照抄了第四份（Rust、共享页、Android、iOS），靠两边的单测对照。键盘扩展里多一份很小的 C 调用和钥匙串封装，因为它不编译 `shared/backend` 的 Rust 客户端。
- **已知缺口**：账号路径（App 经 Rust 会话存储、键盘经 Swift）的同一条规则由重复批准请求的修复负责（Rust 驱动和 `saveKeystore`），这里的 `saveKeystore` 与它同形，两边合并时取同一份。键盘扩展里的「云端」经中继的那一半只有单元测试，没有在系统键盘里点过（模拟器里要另外开启第三方键盘和完全访问）。HarmonyOS 还没接（见上一节）。
- **顺带修的**：云剪贴板页原来靠 `.task` 读第一页，而 `.task` 在 iOS 27 模拟器上先于 `trackViewRequest` 的 onAppear 运行，那时页面还不算可见，`run` 直接返回，进页面看到的是「重试」；现在出现时还没有内容就在 onAppear 里再读一次（`run` 在已有请求时不重复发）。填写页关掉之前清空令牌和应用密码框，否则系统会弹出「保存密码？」把它们存进系统的密码库。

## Verification

- `swift test`（`shared/backend`）141 项通过，新增 `testKeystoreConflictRestartsWithTheStoredIdentityBeforeAnyRequest`：另一个进程先存下身份时改用它重来、冲突的身份不发给服务端，发出请求之后再冲突报 `clipboard_keystore_conflict`；驱动循环换成可注入传输后账号路径的用例照常通过。
- iOS 模拟器（iPhone 17，iOS 27）上 `MSIMEKeyboardTests`：新增 `CloudClipboardRelayTests` 10 项，地址、令牌、网盘账号和应用密码的规则与共享设置页、Rust 的用例对应；绑定不含凭据、换应用密码不换绑定；说法；密钥串是 App Group 访问组里 `cloud-clipboard-relay-keystore` 一项、`AfterFirstUnlockThisDeviceOnly`，两个实例读写同一份；另一个身份写不进去、换中继读作没有；钥匙串读不出来不生成新身份；真实 Rust 驱动下两个实例同时第一次打开只登记一台设备，之后三次刷新不再发登记请求；共享层 `source` 跟着偏好变、中继和网盘同时配置存不进去；配置了网盘时连不上报 `relay_unavailable`、一个账号请求都不发，测试连接的草稿不合规不发请求。整个 `MSIMEKeyboardTests` 1001 项通过（7 项按环境跳过），含原有的 `KeyboardCloudClipboardTests` 和 `RustCloudPagesTests`。
- 模拟器上的 App 对着本机 Apache httpd（`mod_dav`，Basic 鉴权，单进程逐个处理请求）走了一遍：没登录时「我的 → 云剪贴板」能进，显示同步方式和登录说明；网盘地址、账号的当场提示；错误的应用密码「测试连接」报「网盘拒绝了这个账号或应用密码。」，此时「保存」不写偏好、草稿留着；改对后测试连接通过（网盘访问日志里有探测目录的独占创建、412 和删除），保存后偏好有 `cloud_clipboard_webdav`、页面经网盘登记本机、添加一条、置顶一条（直接动作），设备列表只有本机一台；重启 App 后读回同一个身份；已存的应用密码不回显、留空沿用；改回水杉账号后偏好里两个键都没了，本机的缓存目录不动。截图在维护者机器的 `~/Downloads/ios-cloud-clipboard-webdav/`。
- 没有验证的：真实的坚果云、Nextcloud 和 https 的参考中继；第二台设备的批准和互传；键盘扩展里经中继的「云端」在系统键盘里的实际表现；真机上的钥匙串访问组和 App Group（模拟器构建是本机签名）。

# Agent Note: Android 原生宿主接入云剪贴板自建中继

Status: implemented

## Problem

#6787 的提出者用的是 Android 0.3.1。[云剪贴板自建中继](2026-10-12-cloud-clipboard-relay.md) 落地了协议、Rust 客户端、两个 C ABI（`msime_client_cloud_clipboard_source`、`msime_client_relay_cloud_clipboard`）和 Linux 的设置页，但 Android 原生宿主的三个入口都只经水杉账号：「我的 → 云剪贴板」页用 `CloudClipboardApi` 走 `CloudApi`，键盘剪贴板面板的「云端」经 `BackendAccount` 读列表和「发到云剪贴板」，账号页那一行为显示条数也经账号读一次列表。JNI 只导出了端到端加密驱动。对这位用户来说，请求还不可用。

难点不在发请求，而在「配置了中继就只走中继」在 Android 上怎么成立：设置页在主进程，键盘在 `:ime` 进程，偏好每秒才重读一次，任何一个入口漏判一次，内容就会去官方服务，而用户以为在用自己的中继。

## Decision

- **经 JNI 调共享层，Java 不写中继的 HTTP 客户端**：`NativeClient.cloudClipboardSource`、`NativeClient.relayCloudClipboard` 和 `NativeClient.relayCloudClipboardExchange`（上限与头文件相同，16 KiB、128 KiB、128 KiB）。联网（地址校验、路径前缀、静态令牌、401 不交给驱动、响应上限）都在 Rust 里，与桌面端同一份实现，这是母笔记的决定，这里照做。
- **中继的密钥串与账号那份同等保护：AndroidKeyStore 加密，在 `no_backup` 下**（评审后维护者定的：中继密钥不得进云备份和设备间迁移，保护不得弱于账号那份）。端到端加密的操作（列表、上传、设备）不经 `msime_client_relay_cloud_clipboard`，那条路由 Rust 把密钥串写成只靠 0600 保护的明文文件；Android 改为与账号路径同一个 Java 循环 `CloudClipboardE2e.run`：驱动是同一个 sans-IO 的 `msime_client_cloud_clipboard_e2e`，密钥串交给 `CloudClipboardKeystore.forRelay`（同一个类、同样的 AES-256-GCM、AndroidKeyStore 里另一把不导出的密钥，文件 `noBackupFilesDir/cloud-clipboard-relay/cloud-clipboard-relay-keystore`，目录 0700），每个请求经新加的 `msime_client_relay_cloud_clipboard_exchange` 由 Rust 发出。这个 C ABI 只发一个请求、原样交回状态码和正文，不读偏好、不读写任何密钥串；地址和令牌是 `CloudClipboardRelay.open` 那一刻从偏好读到的那份，每次都传同一份，中途改了偏好也不会把这一页的请求发到另一个中继。Rust 侧把 `RelayClipboard` 的发送部分抽成 `RelayTransport`，桌面的行为不变。不涉及内容、不碰密钥串的动作（开关、置顶、保留、删除、清空）仍经 `msime_client_relay_cloud_clipboard` 直接请求。
- **加密的那份带着中继的绑定**：密文里是 `relay:<摘要>` 一行加密钥串，绑定与当前中继不同就读作没有、保存时直接覆盖（换中继不算身份冲突），同一个中继里的第二个身份照账号那份的规则拒绝（`IdentityConflict`，后到的进程改用文件里的身份重来）。写之前确认偏好里还是打开时那个中继（`requireCurrent`，否则 `relay_changed`、不写）。Rust 那条路用整次操作的文件锁防两个进程各登记一台设备，Java 这条路与账号路径一样靠写入时拒绝第二个身份，效果相同。驱动报的错按 `CloudClipboardRelayPolicy.driverFailureCode` 归类，与 Rust 的 `RelayError::code` 一致（已知码原样，400、404 各有码，其余是 `relay_unavailable`），界面上的说法不变；密钥串或 AndroidKeyStore 读写失败是 `relay_storage`。
- **这个改动之前的开发构建写过的明文文件**：同名、同目录，第一个字节不是格式版本，读作没有；第一次保存时被加密的那份原子替换。没有发布过，不另做迁移。
- **`CloudClipboardApi` 抽出 `Backend` 接口，两条路的解析只写一份**：账号的实现原样搬进 `CloudClipboardApi.AccountBackend`，中继的是 `account/CloudClipboardRelay`。中继答的结果与驱动的形状相同，`parsePage`、`parseDevices` 和键盘的 `BackendAccount.clipboardPage`、`clipboardItem` 直接复用。驱动的 `status` 在中继的动作集里叫 `devices`，`CloudClipboardRelay.drive` 负责换名；置顶、保留期、清空走中继专有的动作，不退化。
- **每个入口在发请求之前问共享层**：`CloudClipboardRelay.selected(偏好目录)` 包着 source ABI。它说 `relay` 就只走中继；读不了偏好（`storage`）或中继配置不合规（`relay_not_configured`）时抛出，按失败处理，从不改走账号。偏好目录还不存在（首次设置之前）时按账号处理，那时不可能配置过中继。
- **键盘用「绑定」代替账号编号认出条目属于谁**：主线程只看内存里的偏好快照，配置了中继时绑定是 `relay:` 加上地址和令牌的 SHA-256 前 16 字节（`CloudClipboardRelayPolicy.binding`），登录代次记 0，沿用原来的 `acceptsBinding`；没有中继时仍是账号编号和登录代次。后台线程发请求前经 `CloudClipboardRelay.forPanel` 再问一次 source，并核对偏好文件里的绑定与主线程记下的相同，两边对不上（快照最多晚一秒）就抛出 `relay_changed`、按失败处理：不在配置了中继时经账号读写，也不把一条记录发到用户刚换掉的中继。插入和上传回调照旧比对绑定，地址或令牌一换，已画出的条目就不再可用。
- **中继的密钥串在 `noBackupFilesDir/cloud-clipboard-relay/`**：主进程和 `:ime` 进程用同一个文件，同一台设备只登记一次。离开设备的路径逐一查过：清单（各 edition 共用 `platforms/android/AndroidManifest.xml`，Tauri 合包的清单同样）是 `allowBackup="false"`，没有 `fullBackupContent` 也没有 `dataExtractionRules`；`targetSdk` 是 35，而 API 31 起 `allowBackup="false"` 只关云备份、不关设备间迁移，迁移时会带走 `files/` 下的东西——账号那份密钥串在那里，靠 AndroidKeyStore 的密钥不随迁移走而解不开。`no_backup` 不论云备份还是设备间迁移，系统的备份代理都不遍历；API 35 模拟器上用本地传输的迁移模式（`is_device_transfer=true`）实测过，见 Verification。所以中继那份既不离开设备，离开了也解不开。
- **Tauri 合包只经账号答，配置了中继就拒绝**：`build-client-apk.sh` 的 Tauri 工程里，共享设置页的「发到云剪贴板」和共享云剪贴板面板都经 Tauri 的 `cloud_clipboard_request`，移动端那个命令只有账号一条路。现在它发请求前先读共享偏好（`platform::cloud_clipboard::require_no_relay`）：配置了中继答 `relay_unsupported`，偏好读不了或中继配置不合规同样拒绝，内容不会在用户选了中继时发往 `api.msime.app`。共享界面把 `relay_unsupported` 说成「云剪贴板已改用自建中继，这个页面还不能经中继收发」，不当作未登录。iOS 的 Tauri 外壳走同一个命令，一并拒绝。
- **设置放在「我的 → 云剪贴板」页顶部**：一张「云剪贴板同步方式」卡，「同步方式」选水杉账号或自建中继；选了中继再列「中继地址」「中继令牌」，点哪一行都打开同一个对话框。地址和令牌按 `clipboard/CloudClipboardRelayPolicy` 当场检查（规则与 Rust 的 `valid_relay_url`、`valid_relay_token` 和共享页的 `cloud-clipboard-relay.ts` 逐条对应），合规才能点「保存」；令牌框不回显已存的令牌，已配置时留空表示保留。保存走 `KeyboardSheets.save`，Rust 还会再校验一遍。「改回水杉账号」删掉 `cloud_clipboard_relay` 这个键（与 Rust 没配置时不写这个键一致），中继上的内容和本机的中继密钥串都不动。
- **错误码各有说法，令牌被拒不算未登录**：中继的失败以 `CloudApi.Failure` 抛出，`status` 为 0，`code` 是共享层给的码。键盘把 `relay_unauthorized` 和 `relay_not_configured` 归为新的 `Status.RELAY_REJECTED`（一句「请到我的 → 云剪贴板检查」），连不上和密钥串读写失败仍是可重试的 `FAILED`。设置页的 `failureMessage` 先看中继的码，再看其余已知码，最后才按 `status == 0` 说连不上服务器；原来 `status` 先判，驱动报的 `clipboard_device_pending` 也是 0，会被说成「连不上服务器」，这里顺带纠正。
- **账号页配置了中继时不显示条数，也不问账号**：那一行只为显示一个数字。经账号读列表会让这台设备在水杉账号的云剪贴板上登记，为一个数字去连用户的中继也不值得。

## Alternatives considered

- **整个操作交给 `msime_client_relay_cloud_clipboard`，密钥串由 Rust 存成 `no_backup` 下 0600 的明文文件**（这个 PR 第一版的做法） — 最强的理由是一份实现：锁、核对中继地址和令牌空间、拒绝别人可读的文件都在 Rust 里，与桌面端完全相同，Android 只拼请求；`no_backup` 已经保证它不进备份和迁移。不用：评审指出它弱于账号那份，拿到应用数据目录（root、可调试构建的 `run-as`、取证工具）就拿到中继上的设备私钥和剪贴板密钥，维护者要求至少与账号那份同等保护。Java 侧早有账号路径那套循环和 AndroidKeyStore 封装，换成它只多一个只发请求的 C ABI，不必在 Java 里重写中继的 HTTP。
- **给 `msime_client_relay_cloud_clipboard` 加加密回调或密钥参数，让 Rust 写加密文件** — 驱动和锁仍全在 Rust，桌面以后也能用同一个口子接系统密钥库。不用：AndroidKeyStore 的密钥不能导出，Rust 拿不到它，只能经 JNI 回调进 Java 做每一次加解密，C ABI 要带函数指针、JNI 要在回调里处理异常；而 Java 已经有一模一样的循环（账号路径），复用它比造回调简单，两条路在 Android 上的行为也因此一致。
- **Tauri 合包也接上中继，而不是拒绝** — 开发用的合包里设置页和面板就能用中继。不用：Tauri 进程里没有那份 AndroidKeyStore 加密的密钥串，要么经 Kotlin 插件把整个 Java 循环再接一遍，要么让 Rust 再存一份明文，后者正是这次要去掉的；合包不是发布包（AGENTS.md：产品本体是原生宿主），拒绝已经满足「内容不去 `api.msime.app`」。
- **密钥串放在 `filesDir` 下与偏好、账号会话相邻，和桌面端「与账号会话同一目录」对齐** — 位置统一，查问题时少记一个目录，而且应用已经关了备份。不用：`allowBackup="false"` 是清单里的一个属性，日后为了别的功能打开备份时，`filesDir` 里的明文私钥会被一起带走；`no_backup` 由系统保证不进备份和设备迁移，不依赖这个属性一直关着。
- **键盘只在后台线程问共享层，主线程不看偏好快照** — 选路只有一个来源，少一份绑定逻辑。不用：插入和上传的回调在主线程上，要立刻判断画出来的条目还属不属于现在的那条路，不能为此在主线程读文件；账号路径本来就是主线程比对绑定、后台再核对会话，中继沿用同一个形状，`CloudClipboardPanelPolicy.acceptsBinding` 不用改。
- **设置放到「设置 → 键盘」的「剪贴板」一组，与 Linux 共享设置页把它放在「剪贴板」页对齐** — 跨平台的位置一致，文档好写。不用：Android 键盘页的「剪贴板」组只管本机历史和面板排列；云剪贴板的开关、列表和设备都在「我的 → 云剪贴板」，同步方式决定了这一页显示的是谁的内容，放在同一页才能保存后立刻看到中继上的结果，也不用另做一处跳转。
- **在 Java 里直接用 `HttpURLConnection` 请求中继，像账号路径那样** — Android 账号路径就是这样做的，Java 层可以用系统信任库（企业 CA、用户装的证书都能连），局域网自签证书的用户也许能用上。不用：母笔记已经决定联网收进 Rust，Java 里再写一份就要自己处理路径前缀、静态令牌、401 不刷新、响应上限和密钥串锁；信任库与其他平台不同还会让「同一个中继在这台机器上连得上、在那台连不上」，排查更难。

## Consequences

- **收益**：Android 用户不登录水杉账号也能用云剪贴板，设置页、键盘「云端」和「发到云剪贴板」只连用户的中继；中继只见到密文。共享层的选路、Rust 的客户端和参考中继都没有改动，Android 只多了 JNI 的两个入口和一层 Java 适配。
- **代价**：Android 的中继有两条代码路径——端到端加密的操作走 Java 循环加 `exchange`，直接的动作走 Rust 的整操作入口——不再是桌面那样一份 Rust 实现；驱动错误码的归类在 Java 里照抄了一份 Rust 的已知码清单（`CloudClipboardRelayPolicy.driverFailureCode`，JVM 冒烟钉住归类，`scripts/test-android-relay-error-codes.py` 核对两份清单相同）。令牌明文存在共享偏好里（与语音、翻译凭据相同），每个 `exchange` 请求经 JNI 带着它，不写日志。每次读写多一次 source 调用和一次偏好读取（都是本地文件，在工作线程上）。键盘配置了中继后偏好刚改的那一秒内点刷新会得到一次「读取失败」，再刷新即可。
- **已知缺口**：Tauri 合包（`build-client-apk.sh`，开发用，不随发布）配置了中继时云剪贴板不可用（拒绝，不走账号）。桌面端的中继密钥串仍是 Rust 写的 0600 明文文件（Linux），接系统密钥库要另做。证书只认公网可信的（webpki-roots，不读系统信任库），只在局域网里部署的用户要走 DNS-01 签证书，这在手机上最常见，是母笔记的既定取舍。#6787 仍未完成：macOS、Windows、Linux Fcitx5 键盘栏、iOS 和 HarmonyOS 各自后续接入。

## Verification

`platforms/android/check-host.sh` 通过（223 个 JVM 冒烟，含新的 `tests/settings/CloudClipboardRelayPolicySmoke.java`：地址和令牌规则与 Rust、共享页的用例一一对应，source 只认两个值，绑定随地址和令牌变化且不含令牌，`relay_unauthorized` 不是未登录；`SettingsSearchIndexSmoke` 覆盖新的三行；设备套件 89 个源文件编译通过，`client_jni.cpp` 对共享头文件交叉编译通过）。`gradlew compileFullReleaseJavaWithJavac` 编译了全部宿主 Java，包括 `home/` 下的设置页。

新增设备冒烟 `tests/device/settings/CloudClipboardRelayDeviceSmoke.java`（`smoke.sh --settings` 里跑）：在应用进程里经 JNI 对本进程的回环 HTTP 中继（背后是驱动冒烟的假服务端）跑 source 切换、路径前缀和 Bearer 令牌、两台设备登记和批准、加密上传、列表、置顶、删除、令牌被拒是 `relay_unauthorized`、没有服务的端口是 `relay_unavailable`、不合规的动作不出本机，并确认 Rust 在 `noBackupFilesDir` 下写得出密钥串。后半段不再手拼 JSON，而是经设置页和键盘实际用的 Java 这一层再走一遍（评审指出前半段绕过了这一层，`drive` 的换名或键盘的选路改坏了它照样通过）：`CloudClipboardRelay.forPanel` 在配置了中继时拒绝按账号走、拒绝旧绑定，没配置时返回 null；`CloudClipboardApi` 经 `CloudClipboardRelay` 读总览和设备、第三台设备登记待批准、批准、移除、置顶、保留时长、删除和清空；`BackendAccount.clipboardPage`、`clipboardItem` 解析中继的列表和上传结果；换令牌后已打开的页面和旧绑定都报 `relay_changed`。`CloudClipboardRelay` 不编进测试包（它经 NativeClient 加载应用的 `.so`），冒烟反射调用它包内可见的 `open`、`forPanel` 目录重载。

没有验证的：这次改动里没有构建 Android 原生库、没有在模拟器上跑设备冒烟和装包（交给后面串行的设备验证一步），`verify-native.sh` 的导出清单已加上两个新符号但没对着新构建的 `.so` 跑过。合并前要在 API 28 和 35 模拟器上跑 `smoke.sh --settings`，并在真机上对一台 https 的参考中继走一遍：填地址和令牌、两台设备批准、从键盘「发到云剪贴板」、换令牌后旧条目不再插入、改回水杉账号。

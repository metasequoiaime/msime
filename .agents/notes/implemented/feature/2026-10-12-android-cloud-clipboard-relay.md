# Agent Note: Android 原生宿主接入云剪贴板自建中继

Status: implemented

## Problem

#6787 的提出者用的是 Android 0.3.1。[云剪贴板自建中继](2026-10-12-cloud-clipboard-relay.md) 落地了协议、Rust 客户端、两个 C ABI（`msime_client_cloud_clipboard_source`、`msime_client_relay_cloud_clipboard`）和 Linux 的设置页，但 Android 原生宿主的三个入口都只经水杉账号：「我的 → 云剪贴板」页用 `CloudClipboardApi` 走 `CloudApi`，键盘剪贴板面板的「云端」经 `BackendAccount` 读列表和「发到云剪贴板」，账号页那一行为显示条数也经账号读一次列表。JNI 只导出了端到端加密驱动。对这位用户来说，请求还不可用。

难点不在发请求，而在「配置了中继就只走中继」在 Android 上怎么成立：设置页在主进程，键盘在 `:ime` 进程，偏好每秒才重读一次，任何一个入口漏判一次，内容就会去官方服务，而用户以为在用自己的中继。

## Decision

- **经 JNI 调共享层的两个 C ABI，Java 不写中继的 HTTP 客户端**：`NativeClient.cloudClipboardSource` 和 `NativeClient.relayCloudClipboard`（上限与头文件相同，16 KiB 和 128 KiB）。联网、加密、密钥串读写和锁都在 Rust 里，与桌面端同一份实现，这是母笔记的决定，这里只是照做。
- **`CloudClipboardApi` 抽出 `Backend` 接口，两条路的解析只写一份**：账号的实现原样搬进 `CloudClipboardApi.AccountBackend`，中继的是 `account/CloudClipboardRelay`。中继答的结果与驱动的形状相同，`parsePage`、`parseDevices` 和键盘的 `BackendAccount.clipboardPage`、`clipboardItem` 直接复用。驱动的 `status` 在中继的动作集里叫 `devices`，`CloudClipboardRelay.drive` 负责换名；置顶、保留期、清空走中继专有的动作，不退化。
- **每个入口在发请求之前问共享层**：`CloudClipboardRelay.selected(偏好目录)` 包着 source ABI。它说 `relay` 就只走中继；读不了偏好（`storage`）或中继配置不合规（`relay_not_configured`）时抛出，按失败处理，从不改走账号。偏好目录还不存在（首次设置之前）时按账号处理，那时不可能配置过中继。
- **键盘用「绑定」代替账号编号认出条目属于谁**：主线程只看内存里的偏好快照，配置了中继时绑定是 `relay:` 加上地址和令牌的 SHA-256 前 16 字节（`CloudClipboardRelayPolicy.binding`），登录代次记 0，沿用原来的 `acceptsBinding`；没有中继时仍是账号编号和登录代次。后台线程发请求前再问一次 source，并核对 `CloudClipboardRelay.open` 读到的绑定与主线程记下的相同，两边对不上（快照最多晚一秒）就按失败处理：不在配置了中继时经账号读写，也不把一条记录发到用户刚换掉的中继。插入和上传回调照旧比对绑定，地址或令牌一换，已画出的条目就不再可用。
- **中继的密钥串在 `noBackupFilesDir/cloud-clipboard-relay/`**：主进程和 `:ime` 进程用同一个目录，同一台设备只登记一次；Rust 第一次用到时建目录并收紧到 0700，文件 0600、原子替换。`no_backup` 是系统明确不备份、不迁移的目录，应用的 `allowBackup="false"` 之外再保一层。
- **设置放在「我的 → 云剪贴板」页顶部**：一张「云剪贴板同步方式」卡，「同步方式」选水杉账号或自建中继；选了中继再列「中继地址」「中继令牌」，点哪一行都打开同一个对话框。地址和令牌按 `clipboard/CloudClipboardRelayPolicy` 当场检查（规则与 Rust 的 `valid_relay_url`、`valid_relay_token` 和共享页的 `cloud-clipboard-relay.ts` 逐条对应），合规才能点「保存」；令牌框不回显已存的令牌，已配置时留空表示保留。保存走 `KeyboardSheets.save`，Rust 还会再校验一遍。「改回水杉账号」删掉 `cloud_clipboard_relay` 这个键（与 Rust 没配置时不写这个键一致），中继上的内容和本机的中继密钥串都不动。
- **错误码各有说法，令牌被拒不算未登录**：中继的失败以 `CloudApi.Failure` 抛出，`status` 为 0，`code` 是共享层给的码。键盘把 `relay_unauthorized` 和 `relay_not_configured` 归为新的 `Status.RELAY_REJECTED`（一句「请到我的 → 云剪贴板检查」），连不上和密钥串读写失败仍是可重试的 `FAILED`。设置页的 `failureMessage` 先看中继的码，再看其余已知码，最后才按 `status == 0` 说连不上服务器；原来 `status` 先判，驱动报的 `clipboard_device_pending` 也是 0，会被说成「连不上服务器」，这里顺带纠正。
- **账号页配置了中继时不显示条数，也不问账号**：那一行只为显示一个数字。经账号读列表会让这台设备在水杉账号的云剪贴板上登记，为一个数字去连用户的中继也不值得。

## Alternatives considered

- **中继的密钥串也用 AndroidKeyStore 加密一层，与账号那份（`CloudClipboardKeystore`）同等保护** — 最强的理由是对称：Android 上账号的设备私钥离开 AndroidKeyStore 就解不开，中继的却是明文文件，拿到应用数据目录（root、调试版备份）就拿到私钥。不用：密钥串由 Rust 自己读写（加锁、核对中继地址和令牌空间、拒绝别人可读的文件），要包一层就得给 C ABI 加一个加密回调或者把读写挪回 Java，后者等于在 Java 里重写 Rust 那份锁和绑定逻辑；六个平台的中继都是同一个取舍，加固应当跨平台一起做，不在 Android 上先分叉。
- **密钥串放在 `filesDir` 下与偏好、账号会话相邻，和桌面端「与账号会话同一目录」对齐** — 位置统一，查问题时少记一个目录，而且应用已经关了备份。不用：`allowBackup="false"` 是清单里的一个属性，日后为了别的功能打开备份时，`filesDir` 里的明文私钥会被一起带走；`no_backup` 由系统保证不进备份和设备迁移，不依赖这个属性一直关着。
- **键盘只在后台线程问共享层，主线程不看偏好快照** — 选路只有一个来源，少一份绑定逻辑。不用：插入和上传的回调在主线程上，要立刻判断画出来的条目还属不属于现在的那条路，不能为此在主线程读文件；账号路径本来就是主线程比对绑定、后台再核对会话，中继沿用同一个形状，`CloudClipboardPanelPolicy.acceptsBinding` 不用改。
- **设置放到「设置 → 键盘」的「剪贴板」一组，与 Linux 共享设置页把它放在「剪贴板」页对齐** — 跨平台的位置一致，文档好写。不用：Android 键盘页的「剪贴板」组只管本机历史和面板排列；云剪贴板的开关、列表和设备都在「我的 → 云剪贴板」，同步方式决定了这一页显示的是谁的内容，放在同一页才能保存后立刻看到中继上的结果，也不用另做一处跳转。
- **在 Java 里直接用 `HttpURLConnection` 请求中继，像账号路径那样** — Android 账号路径就是这样做的，Java 层可以用系统信任库（企业 CA、用户装的证书都能连），局域网自签证书的用户也许能用上。不用：母笔记已经决定联网收进 Rust，Java 里再写一份就要自己处理路径前缀、静态令牌、401 不刷新、响应上限和密钥串锁；信任库与其他平台不同还会让「同一个中继在这台机器上连得上、在那台连不上」，排查更难。

## Consequences

- **收益**：Android 用户不登录水杉账号也能用云剪贴板，设置页、键盘「云端」和「发到云剪贴板」只连用户的中继；中继只见到密文。共享层的选路、Rust 的客户端和参考中继都没有改动，Android 只多了 JNI 的两个入口和一层 Java 适配。
- **代价**：中继的密钥串是明文文件，弱于账号那份；令牌明文存在共享偏好里（与语音、翻译凭据相同）。每次读写多一次 source 调用和一次偏好读取（都是本地文件，在工作线程上）。键盘配置了中继后偏好刚改的那一秒内点刷新会得到一次「读取失败」，再刷新即可。
- **已知缺口**：Tauri 合包（`build-client-apk.sh`，开发用，不随发布）里共享设置页的「发到云剪贴板」和共享云剪贴板面板仍只走账号（Tauri 移动端的 `cloud_clipboard_request` 没接中继）。证书只认公网可信的（webpki-roots，不读系统信任库），只在局域网里部署的用户要走 DNS-01 签证书，这在手机上最常见，是母笔记的既定取舍。#6787 仍未完成：macOS、Windows、Linux Fcitx5 键盘栏、iOS 和 HarmonyOS 各自后续接入。

## Verification

`platforms/android/check-host.sh` 通过（223 个 JVM 冒烟，含新的 `tests/settings/CloudClipboardRelayPolicySmoke.java`：地址和令牌规则与 Rust、共享页的用例一一对应，source 只认两个值，绑定随地址和令牌变化且不含令牌，`relay_unauthorized` 不是未登录；`SettingsSearchIndexSmoke` 覆盖新的三行；设备套件 89 个源文件编译通过，`client_jni.cpp` 对共享头文件交叉编译通过）。`gradlew compileFullReleaseJavaWithJavac` 编译了全部宿主 Java，包括 `home/` 下的设置页。

新增设备冒烟 `tests/device/settings/CloudClipboardRelayDeviceSmoke.java`（`smoke.sh --settings` 里跑）：在应用进程里经 JNI 对本进程的回环 HTTP 中继（背后是驱动冒烟的假服务端）跑 source 切换、路径前缀和 Bearer 令牌、两台设备登记和批准、加密上传、列表、置顶、删除、令牌被拒是 `relay_unauthorized`、没有服务的端口是 `relay_unavailable`、不合规的动作不出本机，并确认 Rust 在 `noBackupFilesDir` 下写得出密钥串。

没有验证的：这次改动里没有构建 Android 原生库、没有在模拟器上跑设备冒烟和装包（交给后面串行的设备验证一步），`verify-native.sh` 的导出清单已加上两个新符号但没对着新构建的 `.so` 跑过。合并前要在 API 28 和 35 模拟器上跑 `smoke.sh --settings`，并在真机上对一台 https 的参考中继走一遍：填地址和令牌、两台设备批准、从键盘「发到云剪贴板」、换令牌后旧条目不再插入、改回水杉账号。

# Agent Note: 鸿蒙接入云剪贴板的自建中继与 WebDAV 网盘，一台手机只登记一台设备

Status: implemented

## Problem

[自建中继](2026-10-12-cloud-clipboard-relay.md) 和 [WebDAV 网盘](2026-10-26-cloud-clipboard-webdav.md) 的协议、Rust 客户端和 C ABI 都已经有了，Android、Windows、macOS、Linux 也接上了，但鸿蒙的原生宿主还只经水杉账号：设置页（ArkWeb 里的共享 React 页面）的云剪贴板面板经 `msime_client_account_cloud_clipboard`，键盘的「云端」也一样。共享页的同步方式那一组只在宿主声明 `cloudClipboardRelay` 时显示，WebDAV 还要宿主提供「测试连接」，鸿蒙两样都没给，所以「剪贴板」页里根本看不到这一组。

鸿蒙另有一个别的平台没有的难点：设置应用和键盘扩展是两个沙箱，各有各的 `files/state`（[共享沙箱镜像](../architecture/2026-10-09-harmony-ime-shared-sandbox.md)）。驱动遇到没有密钥串的目录就生成新的设备身份并登记，两边各跑一次，同一台手机就在空间里成了两台设备，另一台设备上要批准两次；账号路径一直就是这样。维护者看到过一台手机在另一台设备上挂着三个待批准请求，要求鸿蒙上的设备身份每次安装只生成一次、由设置应用和键盘共用，重试和每次同步都不再重新登记。

## Decision

- **整次操作交给 Rust（`msime_client_relay_cloud_clipboard`），不在 ArkTS 里跑驱动**：鸿蒙的账号路径本来就是 Rust 目录模式，密钥串是 `files/state/cloud-clipboard-keystore` 这个应用沙箱里的文件；中继和网盘的密钥串 `cloud-clipboard-relay-keystore`、网盘日志缓存 `cloud-clipboard-webdav-state` 和锁文件放在同一个目录，保护与账号那份相同（Android 评审定下的「不弱于账号那份」在这里就是同一种文件）。NAPI 加三个入口：同步的 `cloudClipboardSource`（只读偏好，参数是目录本身，与 `accountStatus` 一样不能拿去发联网请求），工作线程上的 `relayCloudClipboard` 和 `cloudClipboardWebDavProbe`。
- **每个请求先问共享层走哪条路，选了中继就只走中继**：设置桥接（`HarmonyAccountCloudBridge.cloudClipboard`）和键盘（`CloudClipboardClient`）都先调 `cloudClipboardSource`，读成 `HarmonyAccountRequests.cloudClipboardRoute`：`relay`（含 WebDAV，`backend` 区分）就经中继入口，并把读来源时的偏好修订号作为 `expected_revision` 交给 Rust，配置在两步之间变了就是 `clipboard_source_changed`；读不出来源、回答读不懂都按失败答，从不当成「走账号」。账号请求现在总带 `preferences_directory`，Rust 发出前再读一次偏好，已经改成中继或网盘就拒绝。动作名单与账号路径相同（`PAGE_CLOUD_CLIPBOARD_OPERATIONS`）。
- **设置界面就是共享页那一组**：`apps/harmony/src/main.tsx` 在同意隐私政策后给 `cloudClipboardRelay: true` 和 `probeCloudClipboardWebDav`，「剪贴板」页因此出现「云剪贴板同步方式」，三种方式与桌面完全相同。「测试连接」经新的桥接请求种类 `cloud_clipboard_webdav_probe`，草稿只认 `{url, username, password}` 三个字符串（`webDavProbeRequest`），带上 `files/state` 作 `directory`，通过后 Rust 删掉那里的网盘缓存。
- **每条联网路径都在隐私授权之后**：`cloud_clipboard_webdav_probe` 加进 `PrivacyConsentPolicy` 的 `NETWORK_REQUESTS`（`cloud_clipboard` 原来就在），设置桥接的 `startRequest` 照旧在分派前拒绝；键盘的 `sendRelay` 与账号的 `send` 一样先查 `HarmonyPrivacyConsent.networkAllowed()`，`scripts/test-harmony-privacy-consent-gates.py` 盯住这两处调用点。页面在没有授权时也不给这两项，同步方式整组不显示。
- **一次安装一个设备身份，只由设置应用生成**：`msime_client_relay_cloud_clipboard` 和 `msime_client_account_cloud_clipboard` 多收一个可选的 `create_identity`。为 `false` 时只用本机已有的密钥串：中继那边在 `cloud-clipboard-relay.lock` 里检查（`RelayClipboard::existing_identity_only`），账号那边在驱动读密钥串之前检查（`BackendAccountSession::has_clipboard_identity`；密钥串只会随会话一起被清掉，那时驱动取凭据就停了，所以不必在同一把锁里）；没有可用的身份就答 `clipboard_identity_missing`，不生成、不登记、不联网。不经驱动的开关、删除不受影响。键盘的两条路都带 `create_identity: false`，设置应用不带（默认生成）。
- **身份经共享沙箱从设置应用镜像给键盘**：`SharedStateMirrorPolicy.SETTINGS_ENTRIES` 多了 `cloud-clipboard-relay-keystore`（基础访问模式也导入：键盘从不改写身份，只读得到就够）和 `cloud-clipboard-keystore`（只在完整体验模式，跟着账号会话走）。键盘不发布任何身份。设置应用那边还没有身份时，键盘的副本不删（`keepsLocalWhenAbsent`，在 `SharedStateSync.keyboardSync` 的 `beforeFile` 里）：旧版本的键盘在账号路径上自己登记过一台设备，设置应用生成身份之前它照旧能用，生成之后整份换成设置应用的；退出登录时键盘那份随会话由 Rust 的 `session_adopt` 清掉，不靠镜像。同一台手机因此在空间里只是一台设备，键盘每次读取时设备已在列表里，驱动不再发登记请求。
- **网盘缓存也随设置应用镜像**：`cloud-clipboard-webdav-state` 是同一台设备读到的同一份日志，键盘拿设置应用那份接着往后读不会误报回退。镜像它是为了让「测试连接」通过后的重新信任传到键盘：设置应用删掉自己的缓存，键盘的副本跟着删掉（这一条按普通文件的规则，发布方没有就删），否则用户清理过网盘后键盘会一直报 `webdav_rollback`。
- **键盘列表绑定偏好修订号**：账号路径仍按登录的账号 id 绑定；中继路径是 `relay:<backend>:<revision>`（`relayBinding`），不含地址和凭据，上传时把修订号原样交给 Rust 再比一次。换了中继、网盘或改回账号，已画出的条目不再插入、上传不发出。
- **键盘的两种新说法**：`relay_unauthorized`、`relay_not_configured` 读作 `RELAY_REJECTED`（「请到水杉输入法应用的「剪贴板」设置检查地址和凭据」，不说成未登录），`clipboard_identity_missing` 读作 `SETUP_REQUIRED`（「请先在水杉输入法应用里打开一次云剪贴板，键盘随后与它共用本机的设备身份」）；连不上、限流、回退、来源变了仍是可以稍后重试的 `FAILED`。

## Alternatives considered

- **照 Android 的做法：ArkTS 跑驱动、密钥串交给 HUKS 或资产存储加密，只借 `msime_client_relay_cloud_clipboard_exchange` 发请求** — 最强的理由是密钥串不再是明文文件，拿到应用数据目录也解不开，Android 评审要的就是这种保护。不用：鸿蒙账号路径的密钥串现在就是沙箱里的文件，中继那份用同一种保护已经满足「不弱于账号那份」；HUKS 要在 ArkTS 里再写一遍驱动循环、错误归类和身份冲突规则（Android 的 `CloudClipboardE2e.run` 那一套），而 HUKS 的密钥能不能在设置应用和输入法扩展两个沙箱之间共用，这次没有核实过，共用不了就回到两台设备。把两条路一起换到 HUKS 应该是另一次改动。
- **两个沙箱各用各的身份（原状）** — 什么都不用镜像，键盘不依赖设置应用先用过云剪贴板。不用：同一台手机在空间里是两台设备，另一台设备上要批准两次，设备列表里两项同名；这正是维护者要求消除的。
- **键盘直接读写共享目录里的同一份密钥串** — 只有一份文件，不用复制。不用：Rust 的每个驱动操作要在密钥串所在目录建锁文件、加锁，基础访问模式下键盘对共享目录只读，连列表都读不了；共享沙箱那篇笔记放弃「直接共用状态目录」也是同一个原因。
- **身份双向镜像，谁先用云剪贴板谁生成** — 键盘先用也能工作，不必先打开设置应用。不用：两边都可能在对方的副本到达之前各生成一份，镜像按变化时刻裁决会在两个身份之间来回换，两台设备照样都登记过；只让设置应用生成，规则简单，键盘停在本机等着即可。
- **中继列表按地址和令牌的摘要绑定（Android 的 `relay:` 摘要）** — 偏好里别的设置改了，键盘已画出的列表仍然有效。不用：ArkTS 要读出令牌或应用密码、再用异步的 CryptoFramework 算摘要，凭据会进键盘的 ArkTS 内存；修订号不含凭据，Rust 还在读配置的同一快照里按它再比较一次。代价写在下面。

## Consequences

- **收益**：鸿蒙用户不登录水杉账号也能用云剪贴板，设置页、云剪贴板面板和键盘「云端」都只连用户的中继或网盘，内容和元数据不去 `api.msime.app`；一台手机在空间里只登记一次、只批准一次，账号路径也一样。
- **代价**：键盘在设置应用打开过一次云剪贴板之前用不了它（显示 `SETUP_REQUIRED` 的提示），新装的用户要先在应用里打开一次。老用户升级后，键盘原来在账号路径上登记的那台设备在设置应用的身份到达后不再使用，会在设备列表里留着，要手动移除。设备私钥现在还有一份在共享目录里（与账号会话的刷新令牌同样处理）。偏好的任何改动都会让键盘里的中继列表失效，要重新打开一次云端。镜像网盘缓存让设置应用每次发布都要读一遍这个文件（上限 4 MiB）算指纹。模拟器上没有共享目录：键盘读不到偏好和身份，配置了中继的设置在键盘里不生效，键盘照旧走账号路径（没有登录会话就不联网）。
- **已知缺口**：没有在真机上跑过：中继和网盘的完整流程、两个沙箱共用一个身份、升级后旧设备的去留都只在 Rust 测试和 node 测试里证明过。HUKS 加密这两份密钥串留给以后。

## Verification

- Rust：`clipboard_webdav_tests.rs` 的 `a_process_that_borrows_the_identity_never_registers_a_device_of_its_own` 在假网盘上证明只用已有身份的「键盘」在没有身份时三次读写都停在本机、网盘上没有任何请求、目录里不写出密钥串；拿到设置应用那份之后反复读写，空间里始终只有一台已批准的设备、没有待批准的设备，上传设置应用都看得到；开关这类直接请求不需要身份。host-api 的 FFI 测试覆盖两个入口的 `create_identity: false`（经驱动的动作答 `clipboard_identity_missing` 而不是 `relay_unavailable`，即没有联网；不写出密钥串；类型不对是 `invalid_cloud_clipboard`；有可解析的密钥串时照常往下走）。
- `platforms/harmony/tests/run.sh`：`account-requests.test.ts` 覆盖来源的解析（读不懂一律失败，不当成账号）、绑定不含凭据、中继请求的形状和修订号、键盘带 `create_identity: false`、设置应用不带、账号请求带偏好目录、测试连接的草稿形状；`shared-state-mirror-policy.test.ts` 覆盖三个镜像条目、键盘不发布身份、基础与完整模式的导入范围、发布方没有身份时保留键盘的副本；`keyboard-logic.test.ts` 覆盖两种新状态和它们的说法；`privacy-consent.test.ts` 覆盖测试连接要授权。
- 调用点的文本检查：`scripts/test-harmony-cloud-clipboard-identity.py`（键盘的两种请求都以 `false` 结尾、设置应用以 `true` 结尾、键盘和设置以外没有别处调云剪贴板的 NAPI、`keyboardSync` 先判断保留身份；在副本上把键盘改成 `true`、去掉保留判断都让它失败），`scripts/test-harmony-privacy-consent-gates.py`（键盘的 `sendRelay` 在授权之后），`scripts/test-harmony-session-adoption-order.py` 照常通过。
- 共享页：`apps/desktop/tests/settings/cloud-clipboard-relay.test.tsx` 新增鸿蒙宿主的用例（「剪贴板」页有中继和 WebDAV，测试连接通过后才保存）。`apps/harmony` 的 `tsc --noEmit` 和构建、`hvigorw assembleHap --no-daemon`（ArkTS 编译）通过；`build-native.sh arm64-v8a`（OpenHarmony NDK 下的 Rust、NAPI 交叉构建与导出检查）在 Mac Studio 上通过；`cargo clippy -p msime-client-services`、`cargo clippy -p msime-host-api --all-targets` 带 `-D warnings` 通过，`cargo test -p msime-clipboard-relay` 通过。
- 没有在模拟器或真机上跑：这台开发机没有装 DevEco 的模拟器，设置页没有截图。

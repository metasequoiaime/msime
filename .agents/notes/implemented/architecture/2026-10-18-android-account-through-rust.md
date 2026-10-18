# Agent Note: Android 的登录会话、令牌、登录和刷新改由 Rust 持有

Status: implemented

## Problem

#8201 迁移计划 5b：原生 Android 的登录会话、匿名会话、登录和刷新原来都在 Java（`BackendAccount` 的 `FutureTask` 单飞刷新、`keepSession`、补 session_id，`BackendAnonymousAccount` 自己注册和重新登录，`CloudApi.Tokens`，`AccountSessionProvider`）。5a 已经把注入存储的 JNI 胶水接好（[Android 的会话存储经 JNI 注入](2026-10-18-android-session-store-jni.md)），这一组要把所有写入方一起切过去。

存储按 flags=0 注册：Rust 读一次就缓存。主进程里只要还留一个 Java 写入方（刷新、登录、补 session_id、匿名重新登录），Rust 就会拿缓存里已经被轮换掉的刷新令牌去刷新，服务端随即吊销整个会话（头文件 `:84` 的共存规则）。所以这几处必须在同一个提交里一起切，不能逐页切换。

## Decision

- `account/AccountStore`：主进程第一次用到时懒注册一次，不是主进程（`:ime`）或第二次注册都在碰 Rust 之前抛出。适配器把四个项映射到原来的位置：`msime_account_session_v2`（登录会话）、`msime_anonymous_session_v1`（匿名会话）、`msime_anonymous_account_v1`（匿名身份，save 在 Java 的锁里「不存在才写」）、`CloudClipboardKeystore` 的文件（密钥串）。client_agent 由 Rust 的 `msime_client_client_agent` 拼，Java 的 `loginUserAgent` 删掉。
- `BackendAccount` 的会话来源抽成 `Sessions`：主进程是 Rust（令牌 `msime_client_account_access_token`，账号编号 `status`），`:ime` 是 provider。登录（Google ID token、邮箱验证码、Apple 网页）和登录方式都调 `msime_client_account` 的 op；`signOut` 是 `forget`。对话、云剪贴板开关和云剪贴板的端到端循环还在 Java，用 Rust 给的令牌发请求（5c 再切）。
- 页面和请求按账号编号（user.id）绑定：`sessionId()` 返回 user.id，provider 的 `session_id` 也是 user.id（Tauri 合包的 v1 只读会话同样改成它的 user.id）。同一个账号重新登录不再算换号，与 Rust 一致。
- `AppleWebSignIn`：verifier 和 S256 摘要由 Rust 的 `apple_web_start` 生成，授权地址也由 Rust 校验；Java 把 Rust 交回的 `codeVerifier` 存进原来的 `msime_auth_pending`（浏览器打开期间进程可能被杀）。`account_invalid` 时保留等待中的流程。
- 失败文案按 Rust 的错误码选（`SignInFailurePolicy`）。登录的老操作把断网和「这种登录方式没开放」都报成 `account_unavailable`，文案合成一句，不再假装分得清。
- provider：`anonymous_access_token` 换成 `shared_access_token`（登录的优先、否则匿名，Rust 唯一给的那种）；语音贡献改成 `voice_contribution`：`:ime` 把 WAV 写进 cache 下的 `voice-contributions/`，主进程只读那个目录里的普通文件，交给 Rust 的 `voice_contribution`（只用匿名账号）。
- `check-host.sh` 守着：Java 里不能再出现 `"/v1/auth/` 的请求路径；三份会话存储的名字只能出现在 `AccountStore`。

## Alternatives considered

- **语音贡献留到 5e，5b 里让主进程从适配器读匿名会话的字节、把访问令牌交给 `:ime`。** 最强的理由是 5b 不用碰 `:ime` 的语音代码，计划把语音排在 5e。不行：Rust 没有「只要匿名令牌」的取法（计划第 1 节决定不加），登录了的用户的匿名访问令牌过期以后，主进程既刷新不了它（Rust 的 access_token 只刷新登录的那份），也不能自己刷新（那又是一个 Java 写入方）。所以 5b 里语音贡献就要改走主进程，5e 只剩释义。
- **删除密钥串的回调同步执行。** 理由是删不掉时能把失败报给退出登录的界面，原来的 Java 就是这样。不行：Rust 在持着会话锁时调 remove，而 Java 的端到端循环在持着密钥串的锁时经 guard 核对会话（主进程调 Rust 的 status 要等那把会话锁；`:ime` 经 provider 绕回主进程，同样要等），等这把锁的同步删除会让两边互等，键盘那一路还是跨进程的死锁。
- **remove 起一条线程去删、立即返回（这一组最初的做法）。** 理由是不碰密钥串的锁就不会死锁。审查后改掉：remove 返回 0 时文件还在，违反「load 返回最后一次 save/remove 留下的」，下一个登录的账号可能先读到上一个账号的设备私钥，迟一步的删除也可能删掉下一个账号刚存的密钥串。现在的做法：`CloudClipboardKeystore.delete` 不拿密钥串的锁，只拿 `cloud-clipboard-keystore.removals` 这把短锁，把删除次数加一、删掉文件再返回；保存在调 guard 之前记下删除次数，写入时在同一把短锁里核对，删过就以 CancellationException 结束、不写。短锁持有时不调外部代码，删除不会排在 guard 后面，删不掉时 remove 报失败，与文件存储相同。
- **退出登录改用 `logout{all:false}`，顺便撤销服务端会话（计划第 10 节列的 Android 用户可见变化）。** 理由是第 10 节要这个变化，而退出正是会话的写入方。没在 5b 做：设备页「移除本机」和注销账号之后也调 `SignIn.signOut`，那时服务端会话已经撤销或账号已经删掉，再发 logout 只会报一个多余的失败；`logout` 要和账号页一起切（5c），在那里分开「退出登录」按钮和这两处。5b 的 `forget` 与原来 Java 的本地清除相同。
- **Java 继续在本机生成匿名身份（`ensureSubject`），只是改成「不存在才写」。** 理由是设置页显示匿名编号时不用联网。没留：它没有调用方（`AccountIdentity.subject` 早就没人用），留着就是第二份身份生成器；身份的生成、保存和注册都交给 Rust 的 `ensure_anonymous`。

## Consequences

- **收益**：主进程里不再有 Java 的刷新、登录和会话写入；匿名账号改为用刷新令牌续期，不再每次过期都重新登录（每个出口地址每天 5 次的匿名登录额度省下来）；「我的设备」按 Rust 拼的 client_agent 显示这台设备，刷新和匿名注册也带它。
- **代价**：几处主线程上的 `sessionId()`（对话页、账号页的回调）现在调 Rust 的 `status`：不联网、不拿刷新锁，但要等 Rust 的会话锁和一次 AndroidKeyStore 解密，5c 把这些页改调 Rust 时一并挪到了工作线程（[Android 设置页面的账号与云端操作改调 Rust](2026-10-18-android-settings-pages-through-rust.md)）。设备测试包的编译清单多了 `NativeClient` 和它引用的几个类。
- **还没做的**：设备套件（API 28、35）这一次没有跑，`SessionStoreDeviceSmoke` 新加的几项和 `BackendAccountSessionDeviceSmoke` 只编译过。

## Verification

- JVM 冒烟（`check-host.sh`）：`AccountSessionRoutingSmoke` 改用假的 `Sessions`，核对键盘进程只从主进程拿令牌、不跑 Rust 的账号操作、换号后旧请求连驱动都不碰、不带账号编号的令牌不用；`SignInFailurePolicySmoke` 核对按错误码选的文案；`AppleWebSignInPendingSmoke` 核对只有 `account_invalid` 保留等待中的流程。逐个改回（删掉 `account_network` 那一行、`keepPendingAfter` 返回 false、去掉「令牌要带账号编号」、provider 不认新方法）时对应冒烟失败；往 Java 里加一个 `"/v1/auth/refresh"` 或在 `BackendAccount` 里写 `msime_account_session_v2` 时 `check-host.sh` 的守卫失败。
- JVM 冒烟 `AccountStoreKeystoreItemSmoke`：产品的适配器把第 2 项读写到 `CloudClipboardKeystore` 的文件；另一条线程持着密钥串的锁、在 guard 里等 remove 的调用方手里的锁时，`remove(2)` 很快返回 0、返回时文件已经删掉，那条线程的保存以 CancellationException 结束，下一个账号存的密钥串留得住；删除失败时报失败。改回起线程去删、改成等密钥串的锁、或去掉删除次数的核对，它分别失败。
- 设备冒烟（只编译过）：`SessionStoreDeviceSmoke` 把一份 Java v2 形状的会话经 `AndroidAccountSessionStorage` 加密写进 `msime_account_session_v2`、把产品的适配器再注册一次，Rust 的 `status` 和 `access_token` 读得出；匿名身份第二次 save 返回 1；密钥串那一项对产品私有目录里的文件做同样的「同步但不等锁」核对；`AccountStore` 的两道守卫；`voice_contribution` 拒绝约定目录以外的路径。`BackendAccountSessionDeviceSmoke`（原 `BackendAccountRefreshDeviceSmoke`，Java 刷新的用例删掉）用假的 `Sessions` 核对换号、退出和同一账号重新登录。

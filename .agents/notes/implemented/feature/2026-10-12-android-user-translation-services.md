# Agent Note: Android 候选翻译接入用户自己的翻译服务

Status: implemented

## Problem

#7078（来自 #6792 第二点的 Android 平板用户）：离线词典查不到的词和长词在 Android 上没有译文，用户想接自己的翻译服务。共享层早就有完整的「自己的服务」通路：`crates/host-api/src/ffi/providers.rs` 的 `selected_translation_services` 决定用哪个服务（小牛 > 自定义 > 腾讯，水杉账号只在显式选择且没有自有服务时用），`msime_client_{custom,niutrans,tencent}_translation_http_request` 构造并签名请求，`msime_client_parse_*_translation_response` 解析回复，自定义服务按主机名识别 DeepL（见 [自定义翻译服务识别 DeepL 官方 API](2026-10-11-custom-translation-deepl-api.md)）。iOS 和鸿蒙已经接上，Android 没有：JNI 没导出这些函数，键盘只有水杉账号（`BackendTranslationClient`）和离线释义两条路，`CandidateTranslationPolicy.accountSelected` 缺腾讯条件，原生设置也没有地方选服务、填凭据。

维护者的决定：只接内核已有的三种服务（含 DeepL），与桌面同一套共享偏好键，密钥和 AI 辅助的令牌一样存在本机偏好里；40 字上限不动；地址规则跟内核 `is_secure_endpoint`，不放行局域网 http；保持 Android 离线释义优先、在线补缺；不做 AI 大模型翻译。

## Decision

- **JNI**：`client_jni.cpp` 导出十个函数：三个请求描述器、三个回复解析、两个「回复是否失败」（`msime_client_{custom,niutrans}_translation_reply_failed`，区分失败和空回答）、设置页的 `msime_client_translation_endpoint_issue` 和 `msime_client_test_api_credential`。`NativeClient` 加对应的包装，`verify-native.sh` 的 host 导出表和 JNI 方法表各补十个。共享头文件和 Rust 都不用改。
- **选路**：`CandidateTranslationPolicy.route(...)` 逐条照 `selected_translation_services`，参数全是基本类型，返回不可变的 `CandidateTranslationRoute`（NONE / ACCOUNT / NIUTRANS / CUSTOM / TENCENT 加凭据）。宿主自己加一道门：候选翻译关闭时所有服务都是 NONE（共享层这时只排除账号，用户服务由宿主拦下，Linux 也是这样）。所选服务不可用时是 NONE，不回落：小牛要两项凭据可用，自定义只要求接口地址去掉首尾空白后非空（DeepL 地址缺密钥由共享描述器在请求时拒绝，设置页问描述器后显示「凭据不完整」），腾讯要启用且两项凭据可用。原来的 `accountSelected` 删掉，改由路由的 ACCOUNT 推出，补上了腾讯条件。路由不用 record：record 的 `toString` 会把密钥打进日志；`toString` 只写服务名，`scope()` 用凭据的 SHA-256 前缀。
- **键盘**：账号路径原封不动（后来改由 Rust 拼请求、读回答，见 [Android 键盘的「水杉账号」释义改用 msime_client_account_gloss](../architecture/2026-10-18-android-account-gloss-sans-io.md)），另加一条并行的用户服务路径。路由变化（含凭据）时丢掉旧的用户 `CandidateTranslationStore`，按新路由新建一个，`UserTranslationClient` 持有不可变的路由，所以工作线程不会读到改了一半的凭据，也不需要 `bindTo`。`scheduleUserCandidateTranslations` 的门槛与账号路径相同（没有本地模式、不是日文方案、方案显示释义），另外跳过表情和颜文字候选（source 6、7），只取前 10 个候选。`mergedCandidateGlosses` 的在线行在路由是用户服务时从用户 store 读，账号那一支不变；`glossLines` 的在线参数改成「账号或用户服务」。
- **请求**：只翻译码点数不超过 40 的词；腾讯每批最多 9 个词、秒级时间戳；小牛和自定义一词一请求，小牛带毫秒时间戳；源语言固定 zh，目标语言是 store 传进来的代码。`TranslationHttpTransport` 照云候选的写法：不跟随重定向，超时夹在 1 到 10 秒，只接受 2xx，回复用 `readWithin` 加守护线程在总时限内读完，有 `body_utf8` 时逐字节原样发送。地址再查一遍（https，或 `localhost`、`127.0.0.1`、`[::1]` 的 http，不带用户名、密码和片段），不只信描述器，因为 `network_security_config.xml` 整体允许明文。这一遍用 `java.net.URL` 切分，不用 `java.net.URI`：后者比共享层的 WHATWG 解析严，带下划线的主机名、国际化域名和查询串里的 `|` 都会被它拒掉，结果是共享层和「测试连接」都说没问题、键盘却从不发请求。设置页判断地址能不能存时直接用共享层 `msime_client_translation_endpoint_issue` 的 `accepted`，与偏好校验同一条规则，共享库不可用时才退回本地规则。
- **停得下来**：`CandidateTranslationStore.Service` 多了一个带 `BooleanSupplier stale` 的默认方法，store 传入「epoch 已变」的判断（`requestEpoch` 改成 volatile），逐词（逐批）请求之间问一次。worker 是单线程、长度 1 的队列，过期的一页不停下就会占着它、继续花用户的额度。
- **记下回答和退避**（`UserTranslationMemory`，随客户端一起按路由重建，最多 512 个词）：它的寿命跟路由，不跟输入会话。`stop()` 每次换输入框、收起键盘、聊天应用发送后重启输入都会跑，第一版在里面把路由换成 NONE，下一次会话就新建客户端，记下的回答和 30 秒退避全丢了，密钥错或服务挂了时每个新会话都会再发请求，与 `PRIVACY.md` 的承诺不符；现在 `stop()` 只暂置路由（`suspendCandidateTranslationRoute`），偏好算出同一路由时沿用原来的客户端，`check-host.sh` 钉住这一点。问过的词连同回答记下，同一路由下不再问；没有能显示的译文（空、与原词相同、超长）也是回答，记为空，相当于负缓存。正面的回答也要记，因为 store 在重绘时会作废还在路上的回答：第一种目标语言的译文到达后键盘重绘，`refresh` 先 `cancel`，正在请求第二种语言的那一批就被丢掉，不记的话下一轮会原样再请求一遍；单个词的回复是失败时这个词 30 秒内不再问；整个请求没有回复（连不上、超时、非 2xx，或腾讯回报错）时整个服务 30 秒内不发请求。另一个原因是每次 `render()` 都会让 store 重新排一次请求，而 store 只缓存非空译文。
- **设置**：新的「翻译服务」页（`home/TranslationServicePage`，`PageId.TRANSLATION_SERVICE`），从「输入 → 翻译」和「我的 → 隐私」都能进。五个选项：不使用、水杉账号、小牛翻译、腾讯云机器翻译、自定义（DeepLX 或 DeepL API）。选服务时三个 `enabled` 互斥，腾讯没选时显式写 false，`translation_account` 只在选账号时写 true、否则删掉，照 `use-translation-settings.ts` 的 `setTranslationProvider` 和 iOS 的 `select`。字段规则在纯 Java 的 `TranslationServiceSettingsPolicy`，照 `Preferences::validate` 当场说明哪个字段为什么不行，不合规的不写入。密钥行只显示「已填写」「未填写」。隐私页的「用水杉账号翻译候选」打开时在同一次读改写里关掉三个自有服务，否则自有服务优先，这个开关开了也不生效。
- **顺带修好的缺陷**：`applyPreferencesSnapshot` 里 `CandidateTranslationPolicy.displayInvalidated` 原来在把新值赋给字段之后才比较，前后两份永远相同，开关翻译、换目标语言后候选下的旧译文要等下一次按键才消失。现在在赋值之前比较，并把路由变化算进去。
- **设备验收时发现并修好的缺陷**：`scheduleCandidateGlosses` 只在装了非英文离线释义词典时才把结果记进 `candidateOfflineGlosses`，随包的英文释义没有记进去，于是默认配置（目标语言英语、没装别的词典）下 `mergedCandidateGlosses` 看不到离线行，在线译文把「你好 hello」这类离线英文释义盖掉了，离线优先只对非英文词典成立。账号路径原来也是这样。现在目标语言含英语时英文释义也记进去，合并时离线优先；目标语言不含英语时行为不变。

## 与其他平台的差异

共享的小牛回复判定要求 `tgtText` 为字符串；缺失或类型错误走失败退避，合法空字符串仍为可负缓存的回答。该边界也用于 macOS，见[小牛回复结构](../bug-fix/2026-10-15-niutrans-reply-shape.md)。

- 地址规则与 iOS 不同：iOS 只放行 https（`TranslationProviderClient.urlRequest`），Android 跟内核 `is_secure_endpoint`，也放行回环 http，方便在本机跑 DeepLX。局域网 http 两边都不放行；AI 辅助的地址规则（`AiEndpointPolicy`）放行局域网，与这里不同。
- 离线和在线谁优先与鸿蒙相反：Android（和 macOS 的账号路径）离线释义优先，在线译文只补离线没有的行；鸿蒙用户服务优先、离线补缺（`platforms/harmony/entry/src/main/ets/keyboard/candidate/TranslationPolicy.ts`）。
- 桌面三端和鸿蒙会把成功的英文译文写回用户释义库（`translation_gloss_save`，`docs/windows-parity.md`），Android 还没有，这是已知的对等缺口。

## Alternatives considered

- **整体改用 `msime_client_translation_query`**：鸿蒙就是这样做的。选路由共享层算出，Android 不需要一份 Java 镜像，Rust 改优先级时也不会悄悄分叉，这是它最强的理由。不用它，是因为要重写整条账号路径和 `check-host.sh` 钉住账号路径的全部约束，Android 现在也没有导出 `translation_query`，改动面远大于这一刀。镜像的风险靠 `CandidateTranslationRouteSmoke` 的表驱动用例锁住。
- **复用账号的那个 `CandidateTranslationStore`，按路由换 Service**：少一个 store，`mergedCandidateGlosses` 也不用分两支。但账号 store 的到达回调 `applyCandidateTranslations` 开头就是 `if (!candidateTranslationAccount) return;`，被 `check-host.sh` 钉住，还要过账号绑定校验（`verifyCandidateTranslationBinding`），用户服务跟账号无关，硬塞进去要么放宽约束、要么让用户服务等一次不相干的账号查询。iOS 用 `use(service, scope)` 换服务，这里换成整个重建 store，效果相同、没有可变共享状态。
- **只在每个词上退避，不做服务级退避**：照计划字面「30 秒内不再请求同一个词」。可是地址填错、断网、密钥失效或额度用完时，每个新词仍要各自超时 2.5 秒，一页十个词、两种语言就是 50 秒占着 worker。连不上、非 2xx 这类失败几乎总是整个服务的问题，所以服务级也退避；单个词的报错正文仍按词退避。
- **放行局域网 http（issue 里提到的「局域网 http」）**：自建 DeepLX 常在局域网另一台机器上。内核 `is_secure_endpoint` 本身不接受，偏好也存不进去，放行需要先改共享层的规则并让所有宿主跟进，维护者决定这次不做。
- **设置页先写一份草稿再整体保存（像 iOS 那样）**：可以「测试未保存的表单」。Android 原生设置页的惯例是每个字段点开即存（`AiSettingsPage`、`VoicePage`），测试用已保存的值，与键盘实际用的一致。

## Consequences

收益：Android 用户可以用小牛、腾讯或自定义服务（含 DeepL）给离线词典查不到的词补译文，请求格式、签名和解析只有共享层一处权威实现；选路与共享层一致，凭据不全时不联网、不改用别的服务；换服务立即清掉旧译文。

代价和缺口：选路是 Java 对 Rust 的镜像；小牛和自定义一词一请求，最坏 10 个词乘两种语言乘 2.5 秒，靠可取消的逐词循环、记下的回答和退避控制，额度消耗写进了设置页和 `PRIVACY.md`；英文译文不写回用户释义库；Tauri 合包里的共享设置页在 Android 上仍隐藏服务选择（`translation-settings-bindings.ts`、`translation-candidate-settings.ts`），合包用户只能在原生页配置；英文候选的 en→zh 方向、`/fy` 命令翻译、AI 翻译和 40 字上限都没做。隐私页开关的语义变了：打开「用水杉账号翻译候选」会关掉用户自己的服务。

## Verification

- `bash platforms/android/check-host.sh`：契约约束（账号路径的旧约束、用户服务路径只经过路由、两个用户服务类里不许出现鉴权和签名请求头）、JNI 头文件编译检查和全部 JVM 冒烟。新增 `CandidateTranslationRouteSmoke`（对照 providers.rs 的优先级表、凭据判定、`toString` 不带密钥、`scope` 随凭据变化）、`UserTranslationMemorySmoke`、`TranslationHttpTransportPolicySmoke`、`TranslationServiceSettingsPolicySmoke`，`CandidateTranslationStoreSmoke` 加了「null 位置不缓存」「epoch 变了服务能看到并停下」，`CandidateTranslationPolicySmoke` 补了腾讯条件和路由变化清屏。
- `tests/core/NativeSmoke.java` 加了合成配置的描述器、解析、失败判断和设置页检查调用；它要加载 `libmsime_android.so`，只在设备上跑。
- 设置页和 `MSIMEInputService` 用 Gradle 的 `compileFullReleaseJavaWithJavac` 编译通过。
- 设备验收（API 35 模拟器，本机 DeepLX 兼容 mock 经 `adb reverse` 映射到设备的 127.0.0.1:1188）：局域网 http 地址在设置页被拒、「保存」不可用；回环地址能保存，「测试连接」成功；离线释义查不到的候选显示 mock 译文，离线有英文释义的候选显示离线释义；同一页重打、换输入会话都不再请求；mock 对某个词回 `{"code":500}` 后几次重绘只请求了一次；重启 mock 之后约 30 秒内没有任何请求、30 秒后恢复（推断是重启时连不上触发了整服务退避，没有抓日志证实）；切到「不使用」后不再请求、译文消失；目标语言换成法语后按 FR 请求。`smoke.sh` 在 API 35 全套、API 28 `--core` 通过，`verify-native.sh` 核对了新增的 10 个 host 导出和 10 个 JNI 方法。没有用真实的小牛、腾讯或 DeepL 凭据请求过；原生设置页没有设置第二目标语言（`translation_secondary_language`）的入口，应用不可调试也没法直接改偏好文件，第二目标语言没有在设备上验证。

自定义服务的成功码也必须经过共享结果结构校验，异常结构不进入回答集合，见[自定义翻译回复结构边界](../bug-fix/2026-10-15-custom-translation-reply-shape.md)。

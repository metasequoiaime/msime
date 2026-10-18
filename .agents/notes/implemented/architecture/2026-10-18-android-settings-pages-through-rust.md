# Agent Note: Android 设置页面的账号与云端操作改调 Rust

Status: implemented

## Problem

#8201 迁移计划 5c。5b 以后 Android 主进程的会话、令牌、登录和刷新已经在 Rust 里（[Android 的登录会话、令牌、登录和刷新改由 Rust 持有](2026-10-18-android-account-through-rust.md)），但「我的」页、我的设备、云端数据、帮助与反馈、开发者选项的 MCP 日志上传、其他平台下载的「发送链接」、AI 对话、AI 设计皮肤、社区和云剪贴板页仍各自用 Java 发请求：`CloudApi` 拼路径和 multipart、`DeviceDataApi` 自己流式下载导出包、`DiagnosticsApi` 自己读诊断包并按服务端规则重建、`SkinJobsApi` 自己开插画任务并轮询、`CommunityCatalog` 自己选令牌和重试、`BackendAccount` 自己读 SSE。这些逻辑 Rust 的 `msime_client_account`、`msime_client_community`、`msime_client_account_chat_stream`、`msime_client_ai_skin_generate` 和 `msime_client_account_cloud_clipboard` 都已经有一份，5a 已经把 JNI 入口接好。

## Decision

- 新的 `account/RustAccount` 是页面经 Rust 的唯一入口：请求一律带 `AccountStore` 注册的 `session_store`；`userId`、`requireUser` 读 Rust 的 `status`；反馈、诊断上传、AI 皮肤、社区的下载和举报在两种会话都没有时先 `ensure_anonymous`（照旧受 Retry-After 约束）再试一次，因为 Rust 不会自己注册匿名账号。页面按打开时的账号编号绑定，请求前后各读一次 `status`，变了就丢掉结果。读 `status` 也要等 Rust 的会话锁，所以对话页、下载页、AI 皮肤页、社区页和「我的」页的回调里不再在主线程上读登录状态（迁移差异 M81）。
- 「我的」（`DeviceDataApi`、`ProfilePage`、`AccountFragment`、`DevicesPage`）：资料、改名、头像（Rust 下载，`data:` URL 交回；上传写进缓存目录交给 Rust 读）、我的设备、云端数据汇总、导出（Rust 写临时文件、读得出 zip 中央目录才换上）、删除云端数据、注销（`account_delete`）都经 `msime_client_account`。「退出登录」按钮是 `logout{all:false}`：Rust 先清本地会话再请求服务端，断网时本机照样退出、提示服务端没有撤销；「移除本机」和注销之后只清本机的同步状态。删除云端数据多一个 voice 分区，汇总的 `anonymous_voice` 是 unknown 时写「语音样本数暂不可知」，删除和注销后那一步没删掉匿名账号的语音样本时提示到「语音」里重试；「语音」设置多一个「删除上传过的语音样本」（`voice_delete`）。
- 反馈经 `feedback`：页面照旧把截图缩到长边 1600 像素、1 MiB 以内重新编码成 JPEG（去掉 EXIF 里的位置），再按 Rust 的限制（1 MiB、单边 8192、总像素 8388608，`FeedbackApi.fitsRust`）核对，写进缓存目录交给 Rust，提交完就删。
- MCP 日志上传经 `diagnostics_upload`，交给 Rust 的是与导出诊断包同一份 bundle（`{state_root, include, sources}`），Rust 现读现建；查看、换令牌、删除带着上传时 Rust 交回的 `user_id`。页面打开时按「登录账号，否则匿名账号」的编号读状态。
- 下载链接经 `download_link`。
- AI 对话经 `msime_client_account_chat_stream`：Rust 要求请求带 `model`，第一次发送前读一次 `chat_models` 的 `defaultModel`，用户仍然选不了模型；页面照旧要求登录。退出登录时 `SignIn.signOut` 调 `RustAccount.cancelForSignOut`，取消进行中的对话和 AI 皮肤生成（Rust 只在回答完整以后核对账号）。
- AI 设计皮肤经 `msime_client_ai_skin_generate`：一次生成三套带插画的方案，页面真的关掉时取消。只在不带 Retry-After 的 `account_unavailable` 时把入口藏一天（`AccountFailureText.aiSkinUnavailable`），任务一时繁忙的 503 带 Retry-After，只提示稍后再试。生成前注册匿名账号（`ensure_anonymous`，已有操作，断网也是不带 Retry-After 的 `account_unavailable`）失败时换成只在 Java 里出现的 `ai_skin_anonymous_unavailable`，提示检查网络、不藏入口，否则刚装好、没登录的用户断网点一次「生成」，入口就消失一天。
- 社区经 `msime_client_community`：列表是 `keyboard_skin_list` 和 `resource_list`，举报是 `report`，作者改分类是 `keyboard_skin_set_category`。「获取」皮肤是 `keyboard_skin_download`（服务端同时记一次下载）再存进 Java 的 `CustomSkinLibrary`，不换上（决定 14）。
- 云剪贴板页里不涉及内容的开关、保留时长、置顶、删除和清空经 `msime_client_account_cloud_clipboard`；列表、上传和设备管理照旧是 Java 的端到端加密驱动（令牌由 Rust 给），与键盘进程共用同一把密钥串锁。
- 错误码到页面文案集中在 `AccountFailureText`（社区的在 `CommunityRequest.rustMessage`），措辞沿用各页原来的那几句。个人资料页那张表（`profile`）逐条跑共享用例 `shared/contracts/account-errors/cases.json` 的 `android`（[账号会话布局与账号错误码的共享用例](../testing/2026-10-19-account-shared-contract-cases.md)）。
- `check-host.sh` 守着：这些请求路径（`/v1/users/me/sessions`、`data`、`avatar`、`diagnostics`、`download-link`、`/v1/feedback`、`/v1/skins/jobs`、`/v1/community/`、`/v1/models`）不能再出现在 Java 里；`CloudClipboardApi` 不再自己拼设置类的路径；对话页不再用 `BackendAccount`。页面的接线（退出撤销会话、退出取消进行中的调用、先下载再存库、藏入口的条件、语音样本、截图限制）由 `scripts/test-android-account-pages-through-rust.py` 逐条盯住。

## Alternatives considered

- **云剪贴板页整个改经 Rust，包括列表、上传和设备管理。** 最强的理由是 `msime_client_account_cloud_clipboard` 十二个动作都有，Java 的端到端加密驱动就可以只留给键盘。没有采用：Rust 的驱动经注入存储读写密钥串（`CLIPBOARD_KEYSTORE` 映射到 `CloudClipboardKeystore` 的文件），但读、联网、写之间不持 Java 那把跨进程的密钥串锁；键盘进程的 Java 驱动同时在跑时，两边可能各自给本机登记一个设备身份。计划写的就是「剪贴板的简单操作」，内容和设备那一半等键盘也换成 Rust 时一起切。
- **反馈截图原样交给 Rust，只在超限时才缩图（计划第 6 节的字面做法，迁移差异 25）。** 理由是不超限的截图不必重新编码，画质更好。没有采用：原来 Java 总是重新编码，顺带去掉 EXIF 里的位置，Rust 不重新编码，原样交过去就会把位置一起上传，这是隐私上的倒退。照旧总是重新编码，交给 Rust 的那一张一定在限制以内，「超限先缩图」自然成立。
- **「获取」皮肤用 Rust 的 `msime_client_custom_skin_library` 的 create 存库（决定 14 的原话）。** 理由是皮肤库的规则也只剩 Rust 一份。没有采用：Android 的皮肤库是 Java 的 `CustomSkinLibrary`，键盘进程经它的锁文件读写同一份 `library.json`；JNI 没有接 `msime_client_custom_skin_library`（5a 的十六个入口里没有），换成 Rust 的库要另起一组 JNI 和锁的对齐。下载这一步按决定 14 换成了 `keyboard_skin_download`，存库仍用 Java 的那一份。
- **AI 对话继续不带 `model`。** 这是 2026-10-11 的产品决定（[Android 对话页不选模型，由服务端决定](../feature/2026-10-11-android-chat-server-picks-model.md)）。不能保留：Rust 的流式入口要求 `model`（迁移差异 121），缺了是 `account_invalid`。改成用 `chat_models` 的 `defaultModel`，用户仍然选不了，模型仍是服务端说了算的那一个，代价是第一次发送前多一个请求。
- **这一组顺手删掉不再有人调用的 Java 实现（`BackendAccount` 的对话和云剪贴板开关、`CloudApi` 里只给它们用的部分）。** 理由是删得越早，越不会有人接着用。没有在这里做：计划把删除排在 5f，与快照队列、云同步切完以后剩下的一起删；5f 删掉了它们（[删掉 Android 已经交给 Rust 的账号与云端重复实现](../simplification/2026-10-19-android-duplicate-account-code-removed.md)）；这一组只删了被改写的类里已经没人调用的部分（`DiagnosticsApi` 的读包和编码、`SkinJobsApi` 的任务轮询、`CommunityCatalog` 的令牌选择和重试、`CommunityRequest` 的路径拼接、`DeviceDataApi` 的流式下载），和只测它们的冒烟、守卫脚本。

## Consequences

- **收益**：设置页面的请求、令牌、401 后的重试和响应校验只剩 Rust 一份；导出失败时原来的文件不动；诊断上传多带最多 200 条崩溃记录，超过 2 MiB 时先截掉最旧的事件而不是整次失败；AI 皮肤一次生成只用一个账号；限流时提示约要等多久。
- **用户可见的变化**（计划第 10 节 Android 的那几条）：「退出登录」撤销服务端会话，断网时提示失败、但本机已经退出；流式对话的超时改成整条 125 秒，点停止后连接要晚一点才断；「获取」皮肤断网会失败；「删除云端数据」多一个语音样本分区，注销后也尽力删匿名账号的语音样本、可能多一次匿名登录；昵称不能再留空恢复默认（Rust 要求 1–64 个字）；举报、改分类的服务端细码被 Rust 归并，提示跟着变泛；Rust 的 AI 皮肤必须恰好三套，模型只给一两套时整次失败。AI 皮肤的入口原来只在 HTTP 503 时藏；Rust 把所有 5xx 报成同一个码，刷新令牌时断网也是这个码，所以登录账号（或本机匿名账号）的访问令牌过期后断网点「生成」、服务端一时回 500 或 502、重定向被拒，现在也会把入口藏一天。这是 Rust 契约分不出来的，要分开得让 Rust 给没开 AI 皮肤的 503 一个自己的码，这一组不改 Rust。
- **代价**：页面的接线只能用源码守卫和设备冒烟盯，JVM 冒烟跑不了 org.json 和 JNI；设备冒烟 `SessionStoreDeviceSmoke` 新加的几项经真实的 Rust、不联网，核对页面拼的请求 Rust 认得、页面在本机先做的检查与 Rust 的上限一致。
- **留给后面几组的**：常用语没有显示云端列表的页面，只在云同步里用，按 `position` 排序随 5d 的 `msime_client_cloud_sync` 处理（[Android 的词库快照队列与自动云同步改调 Rust](2026-10-18-android-cloud-sync-through-rust.md)）；打字统计上传和 QQ 群助手（`TypingUploadApi`）、`SignIn` 登录后读 `/v1/users/me` 记账号绑定（5d 在这之后再调一次云同步的 `status` 对齐；5f 改成读 Rust 的 `status`，见[删掉 Android 已经交给 Rust 的账号与云端重复实现](../simplification/2026-10-19-android-duplicate-account-code-removed.md)）、更新检查（负责人决定除选版本外留在 Java）还在 Java。

## Verification

- JVM 冒烟（`check-host.sh`，249 个）：新加 `AccountFailureTextSmoke`（各页文案、Retry-After 的写法、AI 皮肤藏入口的条件、语音样本）、`ActiveRequestsSmoke`（退出时只取消还在进行的调用）；`DeviceDataApiSmoke`（昵称不能为空、删除分区含 voice、头像 data URL）、`FeedbackApiSmoke`（截图限制、云剪贴板的错误码）、`SkinJobsApiSmoke`（描述的形状）、`CommunityRequestSmoke`（社区错误码）按 Rust 的契约重写，只测已删代码的部分删掉。每一项都临时改回源码确认会失败。
- 源码守卫：`check-host.sh` 的请求路径和云剪贴板、对话两道守卫，`test-android-account-pages-through-rust.py`、改写的 `test-android-keyboard-tryout.py`，各自临时改回源码确认会失败。
- Gradle 编译整个应用的 Java（`compileFullReleaseJavaWithJavac`）通过；`client_jni.cpp` 没有改，`check-host.sh` 照旧用固定 NDK 编译它和 `JniCallbacks.cpp`。
- 设备冒烟：`BackendAccountSessionDeviceSmoke` 的社区用例换成假的 `CommunityCatalog.Calls`（请求形状、哪些操作允许退回匿名账号、请求前后换号就丢掉结果）；`SessionStoreDeviceSmoke` 新加 `settingsPageRequests`。设备上跑没跑、在哪台 AVD 上跑，见这一组的 PR 说明。

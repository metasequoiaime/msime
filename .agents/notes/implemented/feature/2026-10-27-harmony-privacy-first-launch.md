# Agent Note: 鸿蒙首次启动征得隐私政策授权，未同意时不发任何受政策约束的网络请求

Status: implemented

## Problem

AppGallery（中国大陆）的隐私审核要求首次启动时先展示隐私政策并征得同意：「隐私政策有提供空白复选框且由用户自愿、明确勾选／弹窗有拒绝选项」。鸿蒙设置应用 `app.msime.hmos` 首次启动直接进开屏和欢迎流程，没有任何授权提示；而进程一创建就会联网：`EntryAbility.onCreate` 发送匿名使用统计、注册本机匿名账号，设置窗口打开就拉公告，页面挂载后自动检查词库更新、补传打字统计。键盘扩展是另一个进程，同样在 `onCreate` 里注册匿名账号、开始使用统计会话，之后按用户配置发云联想、AI、在线翻译、云端语音、语音润色和云剪贴板请求。

登录面板底部那句「登录即表示同意《用户协议》和《隐私政策》」只覆盖登录这一个动作，覆盖不了启动时就发生的统计、匿名注册和公告，也不是审核要的「可以拒绝」的弹窗。

## Decision

设置应用首次启动时用原生 `CustomDialog` 弹出授权弹窗（`privacy/PrivacyConsentDialog.ets`）：几句话说明本机打字不上传、哪些联网功能要同意后才启用、匿名使用统计不含输入内容；可以点开的《用户协议》和《隐私政策》（`https://msime.app/terms/`、`https://msime.app/privacy/`，与 `packages/ui/src/settings/app-resources.ts` 的 `termsUrl`、`privacyUrl` 相同，经设置桥接的 `openExternalUrl` 交给系统浏览器）；「不同意」「同意」两个按钮并排、同样大小。首次弹窗不能用返回手势关掉，必须二选一。协议列表是 `PrivacyConsentPolicy.DOCUMENTS`，弹窗按列表画链接，只列已经上线、点得开的页面：AppGallery 驳回过登录页里点了没反应的《用户协议》（#10009），`/terms/` 随后上线。

规则全在纯逻辑的 `privacy/PrivacyConsentPolicy.ts`，读写和进程内状态在 `privacy/HarmonyPrivacyConsent.ets`，由 `platforms/harmony/tests/privacy-consent.test.ts` 和 `scripts/test-harmony-privacy-consent-gates.py`（调用点）守着。

### 授权的存储与版本

- 记录写在设置应用的 `files/state/privacy-consent.json`：`{format:1, version, decision:"granted"|"refused", decided_at_ms}`。`version` 是用户做决定时看到的隐私政策版本，当前 `PRIVACY_POLICY_VERSION = 1`。
- 只有「同意了当前或更新的版本」才能联网（`networkAllowed`）。隐私政策有实质变化时把版本号加一：已同意旧版本的用户下次打开应用重新弹窗，同意之前联网功能关闭。
- 拒绝是粘性的：之后启动不再自动弹窗（工信部 164 号文把拒绝后频繁弹窗列为违规），版本号变了也不追问拒绝过的人，因为他们本来就没有联网。用户随时可以在「我的 › 隐私」（2in1 上在「关于 › 法律信息」）的「隐私政策授权」一行重新打开弹窗同意，也可以在那里撤回同意（选「不同意」）。
- 键盘经共享沙箱读到同一份记录：`SharedStateMirrorPolicy.SETTINGS_ENTRIES` 新增 `privacy-consent`（单个文件、不限完整体验模式、只由设置应用发布）。键盘在扩展 `onCreate` 导入镜像之后、以及每次获得焦点导入之后（`KeyboardSession.attach`）重新读。拿不到共享目录时（签名不带 data-group-id、模拟器）键盘读不到授权，按未同意处理。
- 设置应用做出决定后立即 `publishShared()`，再经 `msimeHarmonyPrivacyConsentChanged` 通知页面。

### 拒绝后哪些不发、哪些照常

不发（各自在发出请求之前问 `HarmonyPrivacyConsent`）：

| 路径 | 进程 | 门禁位置 |
| --- | --- | --- |
| 匿名使用统计、崩溃记录的发送 | 两个 | `Telemetry.ets`：设置应用的发送和键盘会话的开始都等授权，`send` 每次再查一次 |
| 本机匿名账号注册 | 两个 | `HarmonyAccountIdentity.ensureAnonymous` 先等授权，再走会话采用和注册 |
| 服务公告 | 设置 | `Settings.loadNotices`，没有授权时撤掉卡片 |
| 账号登录（含 Google）、反馈、对话、设置同步、云词库与快照、云剪贴板、打字统计上传、社区皮肤／词包／插件、AI 皮肤、AI 模型列表与测试、凭据测试、检查更新、本地语音模型下载 | 设置 | `SettingsBridge.startRequest` 按 `PrivacyConsentPolicy.requestNeedsConsent` 拒绝，回 `privacy_consent_required` |
| 词库更新的 `automatic`、`check` | 设置 | `SettingsBridge.dictionary` |
| 云联想、AI 联想、在线翻译、高情商回复 | 键盘 | `KeyboardSession.fetchCloud`、`fetchAi`、`translationHttp`、`thoughtfulReplies` |
| 豆包与 HTTP 云端语音识别、语音润色 | 键盘 | `HarmonyVoiceRecognizer.start`（报「未同意隐私政策，云端服务不可用」）、`HarmonyVoicePolisher.polish` |
| 云剪贴板 | 键盘 | `CloudClipboardClient.send` |

页面一侧（`apps/harmony/src/main.tsx`）在没有授权时不给这些联网 client：账号、对话、反馈、社区、AI 皮肤、检查更新、云剪贴板、云词库、AI 测试与凭据测试、词库更新、统计上传，页面因此不画对应入口，自动检查词库更新和补传统计也不会发起；授权一变就换一个新的 client。桥接拒绝是第二道墙。本地语音模型页保留，下载按钮报「需要先同意隐私政策才能联网」，从文件导入照常。

照常可用：本机打字的全部输入方式、本地词库与个人词条、皮肤与自制皮肤、插件、打字统计、剪贴板历史、手写、系统语音识别（CoreSpeechKit）、本地语音模型、设置的全部本机项。云剪贴板中继和 WebDAV 鸿蒙上没有。会话镜像的采用（`session_adopt`）只读写本机文件、不联网，不受门禁影响。

### 老用户

升级后没有授权记录，按首次启动处理：打开设置应用时弹一次窗，同意之前设置应用和键盘都不联网。AppGallery 审的是全新安装，但《个人信息保护法》对已经在用的人同样适用，而老用户此前从没被征得过同意；所以宁可让老用户点一次，也不默认替他们同意。代价是只用键盘、很久不打开设置应用的老用户，云联想、AI、在线翻译、云端语音和云剪贴板会静默停下（没有结果，语音给出上面那句提示），直到他们打开应用同意。

## Alternatives considered

- **拒绝即退出应用**。不少国内应用这样做，审核也接受，最强的理由是实现最简单：没有「未授权」状态要处理。不用它，是因为输入法的核心是本机打字，拒绝联网不妨碍用户使用；审核条款要求的是「有拒绝选项」而不是「拒绝后退出」，强制退出反倒容易被认定为「不同意就不让用」的捆绑授权。键盘扩展是系统拉起的进程，设置应用退出也挡不住它，退出本身解决不了键盘联网的问题。
- **只靠登录面板的那句「登录即表示同意」**。最强的理由是登录面板已经有这句话、各平台一致，不需要新界面。不用它，是因为启动时的使用统计、匿名注册和公告都发生在登录之前，那句话没有「拒绝」，也不是审核要求的首次启动弹窗。
- **在 React 设置页里画弹窗**。最强的理由是能复用共享组件、各平台将来可以共用。不用它，是因为网页要等桥接和开屏，弹窗会晚于 `EntryAbility.onCreate` 里已经发出的请求；原生 `CustomDialog` 在页面显示时就盖在启动画面上，而且只服务鸿蒙，不改动其他平台的设置页。
- **拒绝后每次启动都再弹**。能让更多人最终同意，但这正是 164 号文点名的「频繁征求同意」。只在用户自己点「查看并同意」时再弹。
- **页面照常显示联网入口、点了再报错或再弹窗**。改动更小，但页面挂载时的自动请求（词库自动检查、统计补传、欢迎流程查登录状态）会在用户刚拒绝后立刻触发，要么满屏报错，要么反复弹窗。改为没有授权时不给这些 client，入口不出现，「隐私」页说明现状。

## Consequences

- 首次启动多一步，必须二选一；同意以后行为与此前相同，匿名注册和统计在同意的那一刻接着进行。
- 拒绝或撤回同意时清空设置应用本机排队待发的使用统计（`HarmonyTelemetry.reportingTurnedOff`）；键盘排队的不再发送。
- 模拟器上共享沙箱不可用，键盘永远读不到授权，键盘里的云端功能在模拟器上不可用；真机签名带 data-group-id 时才验证得到键盘一侧。
- 改隐私政策时记得同时决定要不要把 `PRIVACY_POLICY_VERSION` 加一：加一会让所有已同意的用户重新确认，并在确认前关掉联网。
- `privacy_consent_required` 是新的错误码，只有鸿蒙宿主会发；共享 UI 的 `PrivacyConsentRow` 和 `localModelErrorMessage` 认它，其他平台不提供 `privacyConsent`，页面不变。

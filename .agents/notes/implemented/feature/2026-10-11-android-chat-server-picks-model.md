# Agent Note: Android 对话页不选模型，由服务端决定

Status: implemented

## Problem

产品决定：用户不能选择 AI 对话用的模型，由水杉后端决定。后端在请求省略 `model`（或给空串）时用它配置的默认模型（msime-cloud `internal/server/handlers.go` 的 `chat`：`len(s.config.Chat.Models) == 0 || v.Model == ""` 时取 `s.config.Chat.Model`），服务端的兜底模型另行添加。

Android 原生宿主的对话页（[底部「对话」tab](2026-10-11-android-chat-tab.md)，`home/ChatFragment.java`）界面上没有模型选择器，但请求侧仍在替用户选：进页面和每次换账号都调 `BackendAccount.chatModels` 去拉 `GET /v1/models`，校验整份目录（条数、去重、`default_model` 必须在 `data` 里），然后把 `models.get(0)` 作为 `model` 发出去。这有三个问题：

- 选模型的是客户端，不是服务端。拿的还是目录的第一项，不是 `default_model`，两者不同时用户实际用的不是服务端指定的默认模型。
- 目录是发送的前置条件：目录没到时发出的那句要先挂起（`pendingSend`），目录加载失败就整句报失败，哪怕后端本可以用默认模型回答。
- 每打开一次对话页都多一个账号请求，只为了取一个服务端本来就知道的值。

## Decision

**对话请求不带 `model`，客户端不再拉模型目录。**

- `BackendAccount.chatBody` 组装的请求体只有 `messages`、`max_tokens` 和 `stream`，没有 `model` 字段；`chat(messages)`、`chatStream(messages, call, listener[, expectedSessionId])` 都去掉了 `model` 参数，`validChatRequest(messages)` 只校验消息。
- `BackendAccount` 里只为挑模型存在的 `chatModels`、`validChatModels`、`ChatModel` 和 `MAX_CHAT_MODELS` 一并删除，Android 原生宿主的账号对话不再请求 `/v1/models`。
- `ChatFragment` 去掉模型目录、`loadModels`、`flushPendingSend` 和挂起的那句：有字就能发，点发送就直接发，换账号时只清空对话并改绑新会话。没登录时 `chatStream` 按会话校验失败，对话里出现和以前一样的失败提示。
- 早已没有引用的字符串 `tryout_ai_absent`（「登录后点“加载 AI”选择模型」）一并删除，`platforms/android/README.md` 对话页那一段改成「用哪个模型由服务端决定」。

**不在本次范围内的两处 `model`：**

- 「AI 润色与回复」（`home/AiSettingsPage.java` 的「模型」行，存在共享偏好 `ai_assistant.model`）是用户自己的 AI 服务，端点和凭据都是用户填的，模型当然也由用户填，和水杉后端的账号对话无关，不动。
- AI 设计皮肤（`account/SkinJobsApi.java`）仍然 `GET /v1/models` 取 `default_model` 并显式发给 `/v1/chat/completions`。它没有让用户选，发的也是服务端给的默认值，这次不改；以后若要让它也省略 `model`，是 `SkinJobsApi` 和 client-core `skin/ai.rs` 共同的契约变更。

## Alternatives considered

- **保留拉目录，但改发 `default_model` 而不是 `models.get(0)`** — 最小改动，修掉「第一项不是默认模型」这个偏差，请求里的模型仍是服务端说了算的那个值，请求体也和其他平台一样带 `model`。不用的原因是它仍让客户端参与选模型：服务端换默认模型或加兜底模型时，客户端拿着目录里的旧值去发，后端要按 `unsupported_model` 拒掉；而且目录加载失败仍会让整句对话失败、每次打开都多一个请求。省略 `model` 正是后端为「交给我决定」留的入口。
- **`model` 发空串** — 后端把空串和缺省同样对待。不用的原因是空串是一个「看起来填了」的值，`validChatRequest` 原先恰恰拒绝空模型；直接不带这个字段，语义就是交给服务端，客户端的校验也不必为一个空串留特例。
- **只改 `ChatFragment`，`BackendAccount.chatModels` 留着备用** — 删得少，设备冒烟里拿它做账号请求重试和换账号用例的那几条不用改。不用的原因是产品已经决定客户端不选模型，留一个没人调用的目录接口和它的整套校验，等于给以后再把选择器接回来留了口子；那几条设备冒烟测的是会话重试，换成同样走 `authorizedRequest` 的云剪贴板读取（`clipboard("")`）就行。

## Consequences

- **收益**：模型完全由服务端决定，换默认模型、加兜底模型都不用发 Android 新版本；对话页少一个账号请求，目录拿不到不再连累发送。
- **代价与已知上限**：客户端不再知道这次用的是哪个模型，也就没法在界面上显示它；以后如果产品要显示模型名，得由后端在回复里带回来，而不是恢复目录。`SkinJobsApi` 仍然依赖 `/v1/models` 的 `default_model`，后端不能因为对话页不再用它就去掉这个接口。

## Verification

- `scripts/test-android-keyboard-tryout.py`：`ChatFragment` 里没有模型目录和 `models`，`chatStream` 只传消息、取消把手、回调和会话；`BackendAccount.chatBody` 的参数和请求体里都没有 `model`，源码里没有对 `"/v1/models"` 的请求，也没有 `chatModels` / `ChatModel` / `MAX_CHAT_MODELS`。
- `platforms/android/tests/device/settings/BackendAccountRefreshDeviceSmoke.java`：流式请求和退回非流式的请求体都 `!body.has("model")`；原先借 `chatModels` 做的账号 401 重试、换账号和退出登录用例改用 `clipboard("")`。这份是设备套件，在 CI 的 `android-device.yml` 上运行。
- `platforms/android/check-host.sh`、全部 `scripts/test-android-*.py` 和 Gradle `compileFullDebugJavaWithJavac`（arm64-v8a）通过。没有在模拟器上装包走过对话页。

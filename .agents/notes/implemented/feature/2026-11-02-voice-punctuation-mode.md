# Agent Note: 语音标点：上屏前按偏好去掉或替换识别器加的标点

Status: implemented

## Problem

语音结果里的标点是识别器自己加的：本地模型（sherpa 的 X-ASR、SenseVoice）、系统识别、豆包流式和整句上传的模型都会出标点，AI 润色的预设提示词也要求补标点。宿主拿到什么就上屏什么。用户分两次说「明天去那里玩？」「去日本如何？」，每段末尾都带着句号或问号，只能手动删（#10558）。

设置里唯一相关的开关是「自动添加标点」，绑的是 `voice_input.doubao_enable_punc`，只决定豆包请求时要不要标点，其他服务下置灰，提示还写着「仅豆包语音识别可以自动加标点」。出厂默认的识别服务是本地模型，这些用户完全没有办法去掉标点。

## Decision

### 偏好

共享偏好 `voice_input.punctuation_mode`（`crates/client-core/src/preferences.rs`），字符串，取值：

- `auto`：识别结果原样上屏。出厂默认，等于加这个选项之前的行为；缺键读成它，等于默认时不写进文档。
- `omit_trailing`：去掉结尾的一串句读，句中的保留。
- `omit_all`：去掉所有句读。
- `space`：每一串句读换成一个空格，开头结尾的直接去掉。

取值不在偏好层校验：不认识的值（例如更新版本写下的新选项）照常读写，消费方一律按 `auto` 处理。这样一个更新版本的设置不会让旧版本整份偏好读不了。`voice_input` 整组本来就不进设置文件导出、账号同步只带识别语言，这一项也只留在本机。

「自动添加标点」保留，仍然只管豆包请求；它的说明改为只对豆包生效、其他服务用「语音标点」。

### 规则只在共享层一处

`crates/client-providers/src/voice/punctuation.rs`：`PunctuationMode::{parse, as_str, configured}` 和纯函数 `apply(text, mode)`。

- 只动识别器会写出的句读：`，。、；：？！…～`、全角句点，以及半角 `, . ; : ? !`。引号、括号、书名号、连字符、撇号、`%`、`@`、半角 `~` 不动。
- 两边紧挨字母或数字的半角点（3.5、v1.2、example.com）不是句号；两边是数字的 `:`、`：`、`,`（10:30、1,000）不是标点。
- 去掉标点会让两个拉丁字母或数字粘在一起时补一个空格（`Hello,world` → `Hello world`）；汉字之间直接相连。空格代替标点时，空格不放在左引号、左括号之后，也不放在右引号、右括号之前。
- 句末标点外面还包着右引号、右括号（`他说：“明天见。”`）时，`omit_trailing` 跳过这串收尾符号去掉里面的句读，再把它们接回去，得到 `他说：“明天见”`；识别器和润色提示词都常写出这种结尾。
- 整段只有标点时得到空串，宿主按没有识别到文字处理。

与[本地识别的数字规整](2026-10-17-local-voice-accuracy.md)一样只处理最终文本。宿主经 `msime_client_voice_punctuation({text, mode}) -> {text}`（`crates/host-api/src/ffi/voice.rs`）调用，文本上限与热词纠正相同。取值由 `msime_client_mobile_voice_configuration` 的响应字段 `punctuationMode` 给出，已经按 `parse` 归一，宿主不必自己认取值。

### Android 接线

只处理最终文本，在所有识别服务和润色之后、上屏或存进语音结果之前，每条路径处理一次：

- 键盘里的聆听：`ImeVoiceEntry.delivered()`。本地、豆包、整句上传、系统识别四种引擎和润色后的结果都从这里过，长按锁定后等待发送时显示的、因输入位置变化存进语音结果的，都是处理后的文字。识别出了内容、处理后为空且识别器没报错时，这里补上「没有识别到文字」作为失败提示，与识别窗口一致，不会无声收起。取值在开始聆听时从 `VoiceConfiguration` 读，随 `generation` 一起更新。
- 识别窗口：`VoiceRecognitionActivity.saveResult()`，它是这个窗口存结果的唯一出口；键盘从语音结果领取时不再处理。
- 外部应用调起的离线识别窗口：`OfflineVoiceRecognitionActivity` 在工作线程上处理后交给 `returnResult`。

Java 侧 `VoicePunctuation`（`platforms/android/java/app/msime/android/voice/VoicePunctuation.java`）只认取值、调 JNI；`auto` 和空文本不进共享层，共享层调用失败时返回原文。设置页 `VoicePage` 的「识别」组在「自动添加标点」下面加「语音标点」四选一，说明收进「?」。

### 其他平台

iOS、鸿蒙暂不读这个字段，各自的接线见 #10595、#10596。macOS 偏好覆盖清单（`platforms/macos/tests/settings/preference_coverage.py`）会在桌面接线完成后移除暂未接上的条目。

### 桌面接线

Windows、macOS、Linux 都在最终文本（润色之后）交给会话上屏之前处理一次，中间结果不处理；`auto` 和空文本不进共享层，共享层调用失败时保留原文，处理后为空（原文非空、只剩标点）时与识别结果为空走同一条「没有识别到文字」提示，不会无声收起。与热词纠正不同，共享层返回的空串原样保留，这条提示才触发得了。`HostCapabilities.voice_punctuation` 对三端打开，共享设置页的「识别结果」组给出同一行四选一。issue 原文写的是「识别」组；这一行跟着鸿蒙接线放进「识别结果」，那里是「识别结果怎么用」的设置，Windows 原生设置页也是同一组。

- **Windows**：`VoiceInputConfig.punctuation_mode` 由 Server 读偏好时填好，`VoiceInputSession` 在润色之后、繁体转换和三条上屏路线之前调用 `voice_punctuated`（`platforms/windows/src/voice/VoicePunctuation.h`）。面板的审阅录音也是最终文本，一并处理，与 Android 每个出口都处理一致。原生设置页的「识别结果」组加同一行，「自动添加标点」补上只对豆包生效的说明；Win7 设置页加同名下拉框和一句说明。`auto` 不写进文档，而 Win7 的 `PreferencesStore::save` 只给白名单里的缺省叶字段补路径、其余缺失路径一律按 `invalid_change` 拒绝，所以 `voice_input.punctuation_mode` 要和 `quote_style` 一样列进字符串叶字段，否则第一次从 `auto` 改成别的方式时保存被拒、偏好原样不变。
- **macOS**：`InputController` 每次读到共享偏好时从 `voice_input.punctuation_mode` 取方式（`platforms/macos/src/voice/VoicePunctuation.h` 的 `MSIMEVoicePunctuationMode`），豆包、整句上传、本地与系统识别三条路线在 `applyVoiceText:` 之前、空文本判断之前调用 `MSIMEVoicePunctuated`。不像其他语音偏好那样搬进 `NSUserDefaults`：`SharedVoicePreferences.h` 的映射是「缺键保留本机旧值」，而 `auto` 是出厂默认、不写进文档，从别的方式改回 `auto` 后本机会一直留着旧值，原生设置窗口保存时还会把旧值写回共享文档。
- **Linux**：IBus 和 Fcitx5 两个宿主在开始录音时取下方式，在 `msime_voice_result_or_transcript` 之后、交给会话之前调用 `voice_punctuated`（`platforms/linux/src/voice/VoicePunctuation.h`）。不放在 Python 语音服务里：服务交回空结果时宿主会退回中间转写，只在服务里处理的话，只剩标点的结果会以带标点的中间转写上屏；源码检出时服务也加载不到 host 库。
- **语音面板（Linux、macOS）**：面板不走上面的宿主语音路线：Tauri 的 `recognize_voice` 直接从语音服务的 socket 拿最终文本，用户再点提交经 `send_voice_text` 投给编辑器（Linux 走 `send_panel_voice_text`，macOS 走 `macos_panel_session::submit`），途中不经过 IBus、Fcitx5 或 `InputController` 的处理点。Linux 从 IBus、Fcitx5 菜单的「语音面板…」打开它，macOS 在找得到语音服务 socket 时由菜单和悬浮工具栏的语音按钮打开它，所以面板在 `recognize_voice` 交回最终文本前按本次录音前刷新过的偏好处理一次（`panel_voice_punctuated`，Rust 宿主直接调用 `voice::punctuation::apply`，`auto` 不调用）。处理在识别结束时做，而不是在提交时做：面板显示的就是会上屏的文字，用户在面板里改过的文本原样提交。

### 不做

报告者要的键盘内二级菜单（从水杉图标的功能菜单进入、带整行返回键）不做：这是设一次就很少再改的偏好，放进功能菜单收益小、菜单更挤；手机键盘界面要三端一起改；整行返回栏是子页导航的另一种设计，应当单独讨论。

## Alternatives considered

- **在每个识别器里关掉标点** — 最干净的做法是让识别器根本不出标点，连润色前的文本都不带。但只有豆包有请求参数；sherpa 模型的标点是模型输出的一部分，系统识别由设备上的服务决定，整句上传是模型原样输出，润色提示词还会再补回来。只能在最终文本上处理。
- **存在 Android 本机设置（`AndroidLocalSettings`），像停顿时长那样** — 不碰共享偏好，其他平台不会出现「字段在、不起作用」的空窗。但规则和取值本来就要放共享层让三端复用，存成共享偏好后 iOS、鸿蒙读同一个文档就能接上；空窗期用跟进 issue 和 macOS 覆盖清单记下。维护者选了共享偏好。
- **把「自动添加标点」并进四选一** — 一个设置更好懂。但 `doubao_enable_punc` 是桌面三端共用的字段，并进去要同时改 Windows、macOS、Linux 的设置页和豆包请求；而且「请求时不要标点」和「上屏前去掉标点」对豆包的效果并不完全一样（前者连润色输入都不带标点）。这次保留开关，只改说明。
- **在偏好层校验取值、非法值归回 `auto`** — 能保证文档里只有四个值。但现有字符串字段（`polish_thinking`、`language`）都不校验，校验失败在这里会让整份偏好读不了；更新版本加新选项时旧版本也会读不了。改成消费方按 `auto` 处理。

## Consequences

- **收益**：默认用本地模型的用户第一次能去掉语音标点；一处规则对四种识别服务和润色都生效，iOS、鸿蒙、桌面接线时只需要在上屏点调一次 FFI。
- **代价**：iOS、鸿蒙在跟进 issue 做完之前读不到这一项。`omit_all` 会让「3.5%，10:30」变成「3.5%10:30」这类紧挨在一起的写法，这是「省略所有标点」本身的含义；数字之外的全角句点、半角 `~` 等边缘写法按上面的列表处理，出现误删时改 `punctuation.rs` 的字符表并补测试。
- **语音样本上传**：打开「上传语音以改进识别」时，上传的文本是处理后的上屏文本，与此前上传润色后文本的做法一致。

## Verification

- `cargo test -p msime-client-providers --lib voice::punctuation`：取值往返、issue 里的例子、句末一串标点、引号括号、句末标点在右引号右括号里面、数字和网址、拉丁单词分隔、只剩标点。
- `cargo test -p msime-client-core --lib voice_punctuation_mode`：缺键读成 `auto` 且不写进文档，选过的值和不认识的值都能读写。
- `cargo test -p msime-host-api --lib voice_punctuation mobile_voice_configuration_reports`：FFI 的四种方式、未知方式、只剩标点、缺字段、超长、空指针；共享解析给出归一后的 `punctuationMode`。
- `cargo test -p msime-client-core --lib host_surface`：鸿蒙和桌面三端报 `voice_punctuation`，Android、iOS 不报。
- `apps/desktop` 的 `tests/settings/voice-settings-copy.test.tsx`：Windows、macOS、Linux 的「识别结果」组给出四选一，选中后写进 `voice_input.punctuation_mode`；Android、iOS 不给。
- Windows `windows-voice-punctuation`（不调用、保留原文、空串保持为空），`tests/ui/windows7_voice.cpp` 断言 Win7 下拉框在缺键文档上也能写入（漏列白名单时这条断言在 CI 的 Windows native settings 上失败过）；macOS `voice-punctuation`（取值和经共享层的四种方式）与 `http-voice-controller`（会话收到处理后的文字、只剩标点时提示没听到）；Linux `linux-voice-punctuation`（取值、桩和经共享层的四种方式）。三端真机上的语音上屏仍需人工验收。
- 接线顺序由源码契约守住：`platforms/linux/tests/core/fcitx5_contract.py` 断言 Fcitx5 和 IBus 都在 `msime_voice_result_or_transcript` 之后、判空和上屏（Fcitx5 `commitText`、IBus `msime_client_voice_apply`）之前调用 `voice_punctuated`，只剩标点时不报识别服务出错；`scripts/test-windows-voice-punctuation-order.py` 断言 Windows 在润色之后、判空提示之前、繁体转换和投递之前处理，且不在条件分支里（审阅录音同样处理）。
- `cargo test -p msime-desktop panel_voice`：Linux、macOS 语音面板交回的最终文本按四种方式处理，缺键、`auto` 和不认识的值原样返回。
- macOS `doubao-voice-controller`（本地模型和豆包共用的流式路线）与 `live-voice-controller`（系统识别路线）：行内预编辑开着、`omit_all` 时，带标点的中间结果原样显示，最终结果去掉标点后替换它上屏；整段只有标点时提示没听到、清掉预编辑、停掉录音并放开请求，下一次录音照常上屏。
- 2026-11-03 macOS 真机验收（本地模型、扬声器放音进内置麦克风）：`auto`、`omit_trailing` 正确；`omit_all` 第一句两次没有上屏，另有一次按住右 Option 录音松开后不停。对照代码，这两件事都不在本 PR 的改动里：本 PR 在 macOS 只改最终文本交给 `applyVoiceText:` 之前的一步和读取偏好，按键、录音开始与结束、Esc 取消的代码与 develop 逐字相同；处理只在录音已经结束、最终结果到达之后发生，碰不到录音是否停止；「天啊，他是谁？」在 `omit_all` 下得到「天啊他是谁」，不会走空结果路线（上面两条控制器测试即此路径）。同一轮验收里 `auto` 和 `omit_trailing` 下也出现过松开后 25 秒浮层不收（结果表 18:27:44、18:29:26、18:34:26 三条 `stopped=0`），其中 `auto` 的 18:27:44 那次和随后的重试都没有上屏，与 `omit_all` 第一句一模一样，那时 `MSIMEVoicePunctuated` 在 `auto` 下原样返回、行为与 develop 一致。录音不停期间文档里出现的带标点句子是行内预编辑的中间结果（按设计不处理），杀掉输入法进程后留在了编辑器里。录音松开不停作为既有问题另行跟进；`omit_all` 和 `space` 的真机上屏需在安静环境（BlackHole 回环）重测。
- `platforms/android/check-host.sh`：`VoicePunctuationSmoke`（取值、标签、`auto` 不进共享层、调用失败保留原文），设置搜索收录「语音标点」，`client_jni.cpp` 对共享头编译。

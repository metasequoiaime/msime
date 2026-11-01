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
- 整段只有标点时得到空串，宿主按没有识别到文字处理。

与[本地识别的数字规整](2026-10-17-local-voice-accuracy.md)一样只处理最终文本。宿主经 `msime_client_voice_punctuation({text, mode}) -> {text}`（`crates/host-api/src/ffi/voice.rs`）调用，文本上限与热词纠正相同。取值由 `msime_client_mobile_voice_configuration` 的响应字段 `punctuationMode` 给出，已经按 `parse` 归一，宿主不必自己认取值。

### Android 接线

只处理最终文本，在所有识别服务和润色之后、上屏或存进语音结果之前，每条路径处理一次：

- 键盘里的聆听：`ImeVoiceEntry.delivered()`。本地、豆包、整句上传、系统识别四种引擎和润色后的结果都从这里过，长按锁定后等待发送时显示的、因输入位置变化存进语音结果的，都是处理后的文字。取值在开始聆听时从 `VoiceConfiguration` 读，随 `generation` 一起更新。
- 识别窗口：`VoiceRecognitionActivity.saveResult()`，它是这个窗口存结果的唯一出口；键盘从语音结果领取时不再处理。
- 外部应用调起的离线识别窗口：`OfflineVoiceRecognitionActivity` 在工作线程上处理后交给 `returnResult`。

Java 侧 `VoicePunctuation`（`platforms/android/java/app/msime/android/voice/VoicePunctuation.java`）只认取值、调 JNI；`auto` 和空文本不进共享层，共享层调用失败时返回原文。设置页 `VoicePage` 的「识别」组在「自动添加标点」下面加「语音标点」四选一，说明收进「?」。

### 其他平台

iOS、鸿蒙、桌面三端暂不读这个字段，各自的接线见 #10595、#10596、#10597。macOS 偏好覆盖清单（`platforms/macos/tests/settings/preference_coverage.py`）把它列为暂未接上，接上后删掉那一条。

### 不做

报告者要的键盘内二级菜单（从水杉图标的功能菜单进入、带整行返回键）不做：这是设一次就很少再改的偏好，放进功能菜单收益小、菜单更挤；手机键盘界面要三端一起改；整行返回栏是子页导航的另一种设计，应当单独讨论。

## Alternatives considered

- **在每个识别器里关掉标点** — 最干净的做法是让识别器根本不出标点，连润色前的文本都不带。但只有豆包有请求参数；sherpa 模型的标点是模型输出的一部分，系统识别由设备上的服务决定，整句上传是模型原样输出，润色提示词还会再补回来。只能在最终文本上处理。
- **存在 Android 本机设置（`AndroidLocalSettings`），像停顿时长那样** — 不碰共享偏好，其他平台不会出现「字段在、不起作用」的空窗。但规则和取值本来就要放共享层让三端复用，存成共享偏好后 iOS、鸿蒙读同一个文档就能接上；空窗期用跟进 issue 和 macOS 覆盖清单记下。维护者选了共享偏好。
- **把「自动添加标点」并进四选一** — 一个设置更好懂。但 `doubao_enable_punc` 是桌面三端共用的字段，并进去要同时改 Windows、macOS、Linux 的设置页和豆包请求；而且「请求时不要标点」和「上屏前去掉标点」对豆包的效果并不完全一样（前者连润色输入都不带标点）。这次保留开关，只改说明。
- **在偏好层校验取值、非法值归回 `auto`** — 能保证文档里只有四个值。但现有字符串字段（`polish_thinking`、`language`）都不校验，校验失败在这里会让整份偏好读不了；更新版本加新选项时旧版本也会读不了。改成消费方按 `auto` 处理。

## Consequences

- **收益**：默认用本地模型的用户第一次能去掉语音标点；一处规则对四种识别服务和润色都生效，iOS、鸿蒙、桌面接线时只需要在上屏点调一次 FFI。
- **代价**：iOS、鸿蒙、桌面在跟进 issue 做完之前读不到这一项，桌面的共享设置页也还看不到它。`omit_all` 会让「3.5%，10:30」变成「3.5%10:30」这类紧挨在一起的写法，这是「省略所有标点」本身的含义；数字之外的全角句点、半角 `~` 等边缘写法按上面的列表处理，出现误删时改 `punctuation.rs` 的字符表并补测试。
- **语音样本上传**：打开「上传语音以改进识别」时，上传的文本是处理后的上屏文本，与此前上传润色后文本的做法一致。

## Verification

- `cargo test -p msime-client-providers --lib voice::punctuation`：取值往返、issue 里的例子、句末一串标点、引号括号、数字和网址、拉丁单词分隔、只剩标点。
- `cargo test -p msime-client-core --lib voice_punctuation_mode`：缺键读成 `auto` 且不写进文档，选过的值和不认识的值都能读写。
- `cargo test -p msime-host-api --lib voice_punctuation mobile_voice_configuration_reports`：FFI 的四种方式、未知方式、只剩标点、缺字段、超长、空指针；共享解析给出归一后的 `punctuationMode`。
- `platforms/android/check-host.sh`：`VoicePunctuationSmoke`（取值、标签、`auto` 不进共享层、调用失败保留原文），设置搜索收录「语音标点」，`client_jni.cpp` 对共享头编译。

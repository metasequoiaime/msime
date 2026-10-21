# Agent Note: 本地语音识别：离线 X-ASR、VAD 判断说完、热词、数字规整、按住说话与评测

Status: implemented

## Problem

msime 默认的本地模型是 X-ASR zh-en 的 480 ms 流式导出。同一模型还有整句的离线导出，在 X-ASR 官方表上 WenetSpeech net/meeting 的 CER 是 5.83/7.06，流式导出是 7.46/9.11；离线导出配 Silero VAD 分段和更宽的 beam 是现成可用的提升。

梳理本地识别链路时还查出几处问题：

- X-ASR 的原生热词从这个模型上线起就没有生效过。它的 token 表里每个汉字都是带 SentencePiece 词首标记的独立词（`▁写`，4735 个，没有一个不带前缀的汉字），目录写的 `cjkchar+bpe` 让 sherpa-onnx 找不到汉字 token，整份热词被丢弃；共享层的过滤本来也会先把所有中文热词滤掉。
- 流式模型结束时只补 0.6 秒静音，sherpa-onnx 自带的四条 test_wavs 里有两条丢掉句末一两个字。
- Android 与 HarmonyOS 的触屏录音按「1.5 秒没有新文字」判断说完。经 VAD 分段的整段模型只在一段说完后才出文字，用户停顿一下出了第一段、再连续说超过 1.5 秒，录音就被截断。
- X-ASR 一个阿拉伯数字都不写（二零零二年、百分之五十七），默认路径没有任何规整。
- 仓库里没有任何语音准确率数据，换模型、调 beam 都无从比较。

## Decision

iOS 未配置语音服务时默认选择本地识别，见 [iOS 本地识别默认值](2026-10-18-ios-local-voice-default.md)；这里的默认模型仍指模型目录中的选择。

### 模型与解码（`resources/local-asr-models.json`、`shared/voice/LocalAsr.cpp`）

- 目录新增 `x-asr-zh-en-offline`（`offline_transducer`），作为可选模型，默认仍是流式。离线 transducer 与 SenseVoice 走同一条 Silero VAD 分段路径，`modified_beam_search`、`max_active_paths = 16`、`hotwords_score = 2.0`，热词随每段新建的流传入（`SherpaOnnxCreateOfflineStreamWithHotwords`）。
- 目录条目带 `model_type`（流式 `zipformer2`、离线 `transducer`），三端原样交给 sherpa-onnx，省掉它为读 encoder 元数据多建的一次推理会话。值不认识时 sherpa 只是退回探测，不会加载失败；旧安装没有这个字段照常工作。
- 两个 X-ASR 条目的 `modeling_unit` 改为 `bpe`。识别器以 token 表为准：表里没有不带前缀的汉字、却有 `▁`+汉字时按 `bpe` 编码，热词里的每个汉字独立成词、前后隔空格，要求 `▁`+这个字在表里，否则整个词丢掉。已安装的模型清单仍写着 `cjkchar+bpe`，按 token 表判断让它们不重装也能用上热词。判断与整理导出为 `transducer_hotword_lines` 供测试。
- 流式结束时补 1.2 秒静音。识别结果整理时，紧跟在汉字（CJK 统一表意文字及扩展区、兼容区）后的半角 `.?!,` 换成全角，小写字母与大写字母之间的 `.?!` 后补空格。
- `LocalAsrSession::speech_detected()` 报告 VAD 此刻是否在说话；流式模型没有 VAD，始终为 false。
- iOS（`LocalSpeechRecognizer.swift`）与 HarmonyOS（`LocalAsrWorker.ets`、`LocalAsrPolicy.ts`）是这份 C++ 的移植，以上每一条都同步实现并各自有测试。HarmonyOS 的 HAR 不导出带热词建流的接口，离线识别器的热词经临时文件、以 `hotwordsFile` 交给原生配置（HAR 自己的字段拼成了 `hotwordsFfile`），加载完即删；热词因此随识别器而不是随流，词库一变要重载一次，与它的流式模型一样。

### 说完的判断（Android、HarmonyOS）

停顿计时到点时，VAD 仍判定在说话就再等一轮；VAD 从说话变为静音时从那一刻重新计时。60/120 秒的总时长上限不变。iOS 的录音在主应用里由用户结束，没有这种计时，不改。

### 数字规整（`crates/client-providers/src/voice/numbers.rs`）

规则是本项目自己写的，原则是宁可少改：只在读起来确定是数的场合改写——百分数、年份与年代、完整的月日、带「分」「半」「整」或时段词的时刻、后接单位或整数为零的小数、含十百千万亿且读得通的整数、三个以上数字字连写的串；约数、并列的两个数、叠词和几个固定说法保留汉字。读数先按亿、再按万切开，每节处理千百十、中间的零和末尾省略的位（三千五、两万五）。宿主经 `msime_client_voice_normalize_numbers({text, kind})` 调用，Rust 按 `kind` 决定是否改写（`applies_to_kind`：只有两种 transducer），所以三端对每个本地模型的最终文本都调用它，规则只在一处。只处理最终文本，在拼音热词纠正之后，失败时保留原文。

数字解析在进入下一节之前拒绝低位中重复的同级大单位（「一万一万」「一亿一亿」）：这种低位读得通时必然不小于当前单位，原有规则最终也会拒绝，但递归读完才判断会让识别器重复输出随文本长度增加调用栈。提前检查把每条递归路径固定在亿、万、节三层，同时保留「一万亿一万」这种亿两侧分别有万的写法。百分数、日期、时刻和小数本来就要求整段连续数字都读得通，现在直接解析整段，不再从最长前缀反复尝试，避免长串非法数字的反复扫描。非法串保留原文，宿主接口的 10,000 字上限不变。

### 按住空格说话（Android、HarmonyOS）

长按空格（与拖动移光标同一套 450 ms 仲裁）开始的键区聆听是按住说话：松手结束并出结果，按住期间停顿不自动结束；手指上移超过键盘高度的 25% 进入取消区，显示「松开取消」，在那里松手或触摸被系统取消都丢弃这次录音。纯规则在 `VoiceHoldPolicy.java` 与 `SpaceVoiceGesture.ts`。iOS 键盘扩展不能录音，长按空格转交主应用并由主应用自动开始录音；入口取舍见[直接开始语音笔记](../bug-fix/2026-10-18-ios-space-voice-direct-start.md)。

### 评测（`scripts/voice_eval.py`）

用 `msime-voice-local` 跑 FLEURS 普通话测试集（945 条，CC-BY-4.0）和 ASCEND 中英混说测试集（1315 条，CC-BY-SA-4.0），按混合错误率打分：汉字按字、拉丁字母与数字按词，不计标点和大小写。可选经宿主库做数字规整后再打分。数据不入库，基线写在 `resources/eval/voice-baseline.json`；`scripts/test-voice-eval.py` 钉住打分规则。

## Alternatives considered

- **直接把默认模型换成离线 X-ASR**：准确率更高，用户什么都不用做就能受益。没有采用：离线模型说话时不逐字出字，松手后要等最后一段整段编码，是交互上的取舍，应由产品决定；而且目录的 `default` 是全局的，切默认会同时改掉桌面端，需要先有按平台的默认值。先作为可选模型上线，用评测和真机延迟数据再决定。
- **缩小宿主的文本长度上限或给解析加递归深度上限**：改动小，能直接挡住长串。没有采用：缩小接口上限会额外拒绝长编号等输入；深度上限还保留了从最长前缀逐次重试的扫描成本。按大单位本身的拒绝规则提前退出，并只解析调用者本来就要求完整的数字段，保留原有接受集合。
- **在 C++ `LocalAsrSession::finish()` 里做数字规整**：Android 与桌面直接受益，不必每个宿主各调一次。没有采用：iOS 与 HarmonyOS 的识别器是各自的移植，不经过这份 C++，规则会写成三份；放在 Rust 里三端（以及以后的桌面宿主）共用一份。
- **用清单字段（如 `"itn": "rules"`）而不是 `kind` 决定是否规整**：更显式，模型属性写在目录里。没有采用：清单是安装时的原样副本，已装好的流式 X-ASR 没有这个字段，默认模型的用户要重装才能得到规整；按 `kind` 判断对旧安装同样生效，判断也只在 Rust 一处。
- **热词修正只改目录、不看 token 表**：规则最简单。没有采用：清单在安装时写死，改目录不影响已经装好的模型，默认模型的用户仍然没有热词。
- **用输入法自己的拼音语言模型给 n-best 重排**：识别器把同音的两种写法都放进候选时，语言模型能挑出常用的那个。没有采用：sherpa-onnx 的 C API 不给 n-best，要 fork 运行时，HarmonyOS 走 HAR、iOS 是独立实现，三处都得改；在有评测数据证明收益之前不值得。
- **ORT 格式内存映射、decoder 自己量化成 int8**：能省内存和包体。没有采用：int8 decoder 只省约 8 MB；内存映射要在每个平台 fork sherpa-onnx，`.ort` 绑定架构而目录各平台共用；iOS 模型在主应用里，几乎没有收益。
- **VAD 段前补一小段音频、调大用户热词上限**：前者可能补回被 VAD 切掉的首字，后者让更多用户词起作用。没有采用：段前补多少是手调值，首字识别会随它来回变；热词上限的收益没有数据。没有评测之前不调。

## Consequences

- **实测**（`resources/eval/voice-baseline.json`，macOS，FLEURS 945 条与 ASCEND 1315 条，无热词）：离线与流式的混合错误率 FLEURS 6.97% 对 7.18%、ASCEND 13.61% 对 13.95%，离线只好约 3%（相对值），远小于官方表上两成的差距；数字规整（参考文本与识别结果都规整后比较）把 FLEURS 降到 5.05% 与 5.25%，是这组改动里对准确率贡献最大的一项。松手到出结果的中位数在 FLEURS 上离线 0.45 秒、流式 0.78 秒（流式要多解码 1.2 秒尾静音）。
- **收益**：用户词库热词第一次真正作用到 X-ASR 上；离线模型给要准确率的用户一个选择（实测提升不大，是否切默认看真机延迟和更多评测再定）；流式模型不再丢句尾；连续说话不再被截断；数字按打字习惯上屏；有了可复现的准确率基线。
- **代价与已知上限**：离线模型没有段内 partial，松手后的等待取决于最后一段长度，真机延迟未测。热词现在真正生效，`hotwords_score = 2.0` 对 200 个用户词是否过强没有测过；出现热词「抢字」的反馈时先用评测比较 1.5 与 2.0。噪声持续时 VAD 一直判定在说话，触屏录音会等到噪声停止或总时长上限。HarmonyOS 的离线热词经临时文件、按住说话期间不显示聆听面板（展开面板会替换键区、可能打断触摸流），都还没有上真机。半角标点规则只认汉字，日文假名后的半角标点不换。

## Verification

- `ctest --test-dir build/shared-voice`（配置 `MSIME_LOCAL_ASR_TEST_RUNTIME`、`MSIME_LOCAL_ASR_TEST_MODEL`、`MSIME_LOCAL_ASR_TEST_WAV`）：11 项通过，其中 `shared-voice-local-asr-model` 用真实模型检查识别结果与 VAD 说话状态，`shared-voice-local-asr` 覆盖标点规则与热词整理。
- 数字规整回归在子进程的 1 MiB 工作线程栈上处理 10,000 字的重复万/亿、重复千和纯数字串；父测试同时核对子进程状态和完成标记，精确测试名未匹配时不能误报成功，栈溢出不会终止整套测试。原实现的调试构建对重复万的用例触发进程终止；同一最小程序以 `rustc -O` 编译后，5,000 字和 10,000 字的重复万均在 10 秒时超时，修复后的优化构建对这两种长度均在数毫秒内完成。修复后保留非法原文并正常改写长编号。
- `cargo test -p msime-client-providers --lib voice::numbers`、`cargo test -p msime-host-api --lib voice_number_normalization`。
- iOS `MSIMEServiceTests`（86 项）、HarmonyOS `platforms/harmony/tests/run.sh`、Android `check-host.sh` 的 JVM smoke 与 `LocalSpeechHandleDeviceSmoke`（API 35 模拟器）。
- 热词：离线模型对 test_wavs 的 `3.wav` 加热词「写上去」「现在时」，「一般现代时……时态先上去」变为「一般现在时……时态写上去」；旧清单（`cjkchar+bpe`）同样生效，macOS 与 iOS 模拟器结果一致。

# Agent Note: 鸿蒙键盘收起后不再空转，打字热路径上的同步读写挪走或缓存

Status: implemented

## Problem

低端手机用户反馈水杉键盘耗电、发热、打字卡顿。一次按平台分线的性能审计在鸿蒙键盘上找到两类开销，都在 ArkTS 主线程（也就是输入法扩展的 UI 线程）上同步发生。

键盘收起后仍在做的事：

- 词库快照队列在会话建好后每两秒查一次，收起后也不停，直到扩展进程退出。每次查询都同步调用 `snapshotQueue`：重新序列化偏好、扫一遍用户词库三张表算 SHA-256 作本机版本，再重写并 fsync `dictionary-snapshots/state.json`。鸿蒙上没有任何东西往键盘这份队列里排请求（设置应用的云同步不带 `snapshot_queue_directory`），算出来的本机版本也没人读。
- 快照请求处理不掉、或个人词库同步整批被拒剩余条数不动时，每两秒整个重建一次 Engine 会话，没有退避，键盘隐藏时照样。
- 录音中途收起键盘时 `KeyboardView` 不销毁，`inputStop` 和系统的 `keyboardHide` 都不碰语音状态，流式和上传式识别在看不见的地方一直录到识别器 120 秒上限，麦克风也一直开着。

打字时每个键或每个词都付的：

- 打字统计开着时每次上屏用 `setTimeout(0)` 调同步的 `client.typingStatistics`，仍在 UI 线程上：加文件锁、整份读入解析、改完整份写临时文件、fsync、rename。
- 候选释义或候选翻译开着时，每个改变候选页的按键都重新打开 `msime-english.db`（算 learned 还要再开一次）、学到的释义库、两份释义库和离线目标语言库，并把 `custom_translations.txt` 读两遍。
- 直接英文（手机默认的英文纠错模式下每个字母同步上屏）每个字母做四次 `getForwardSync`，每次都要等宿主应用的 UI 线程作答。
- Canvas 候选皮肤在候选条一空就释放全部 PixelMap，下一个词第一个字母又从 base64 重新解码整套皮肤，APNG 还在 UI 线程上逐帧逐像素合成。
- 手写板每个触摸移动事件都清屏，把已写的每一笔和正在写的一笔从头描一遍，还为每一笔 `slice()` 复制点数组，写一个字是平方级。
- 完整体验模式下每次切换输入框，失焦发布一次共享状态、获得焦点在 `panel.show()` 之前又发布一次，每次都把偏好、按键反馈、账号文件和打字统计整份读出来算指纹；获得焦点时比较偏好修订号还要经 NAPI 读出整份偏好文档；每次构建会话不管队列空不空都先算一遍本机版本并 fsync。

## Decision

收起时停下空闲工作：

- 快照队列只在键盘显示（`attach`）时排一次检查，查到没有待处理请求就停，不自己续排；失焦（`detach`）和系统收起（`keyboardHidden`）经 `pauseIdleDictionaryWork` 取消检查，上一次个人词库排空没有进展时也停下重试，`attach` 时按退避接着排。有进展的排空照常在后台跑完。
- 建会话之前先排空个人词库队列，再同步设置应用的词条镜像；只有排空返回 `settled` 且镜像同步完成时才停止空闲重试。三态排空继续决定退避，镜像未完成不能被排空成功掩盖。
- 两个重建会话的重试由 `IdleDictionaryRetryPolicy` 定间隔：有进展（送出新一批、剩余条数减少）两秒后接着处理，连续没有进展按 2、4、8 秒翻倍，最多五分钟；队列查空或排空、用户在键盘里新排入词条时复位。
- 显示后的那次查询（`state`）和构建会话时的处理（`process`）都带 `skip_when_idle: true`：队列里没有排队或准备中的请求时 Rust 只读状态返回，不算本机版本、不写 `state.json`、不建队列目录。真有请求时照旧先发布本机版本再处理。这个字段由共享 Rust PR 提供，默认 false，其他宿主不带，行为不变；同一条 PR 里 `publish_local_version` 在版本没变时也不再重写 `state.json`。
- 会话在 `keyboardHide` 和 `detach` 时经 `onPanelHidden` 通知视图，视图按 `VoiceRecognitionPolicy.cancelsOnHide` 取消录音和还在走的空格长按计时，与 Android 收起键盘即取消语音一致。已经点了「说完了」或静音自动结束、在等最终结果的不取消，麦克风已经关了，取消会丢掉用户说完的话。2in1 的语音面板不动。
- 候选释义缓存（见下）在 `detach` 和 `keyboardHidden` 时经新的 NAPI `releaseGlossCaches`（调 Rust PR 新增的 `msime_client_release_gloss_caches`）释放。

打字热路径：

- 打字统计的提交和语音时长排进会话自己的一个 `taskpool.SequenceRunner`，在 TaskPool 线程上经 `writeTypingStatistics` 调原生库。用 SequenceRunner 而不是 `taskpool.execute`，是因为活跃时长按相邻提交的间隔算，乱序会算错。成就音在回答到达时检查 `soundsSilencedByEditor()`。键盘收起（`dismiss` 和 `keyboardHide`）、失焦和扩展销毁时在同一个 runner 上发 `{"operation": "flush"}`，`detach` 等 flush 写完再把统计文件发布给设置应用。
- 共享 Rust PR 里，engine 的 `candidate_glosses_with_user` 和 `candidate_target_glosses` 在查询线程的 `thread_local` 里缓存打开过的发布词库、学到的释义库、最多 8 份离线释义库连接和读好的自定义译文，键是路径加文件身份（长度、修改时间，unix 上加设备号和 inode，复用日文模型缓存的 `FileIdentity`）；发布词库和学到的释义库的键还带同目录 `custom_translations.txt` 的身份。文件被替换、改写或删除时下一次查询就重新打开。Windows 上 SQLite 打开文件不带 `FILE_SHARE_DELETE`，翻译线程与服务同寿命，常开的连接会让卸载离线释义包删不掉文件，所以那里只缓存自定义译文。engine 新增 `release_gloss_caches()`；`msime_client_release_process_cache` 一并释放调用线程的释义缓存，新增 `msime_client_release_gloss_caches` 只放释义、不动重新加载很贵的日文词典。鸿蒙的查询线程就是 UI 线程，所以本 PR 在收起和失焦时调它。
- `render()` 和 `insert()` 末尾改走 `refreshCaretSuggestions`：英文补全和邮箱后缀都适用时只读一次上文（取较长的 129 个字符）。`applyEnglishSuggestions` 不再重读当前词，靠 `englishSuggestionEpoch` 判断过期；选用补全时 `useEnglishSuggestion` 仍按编辑器里的词核对再删。新增 `CaretContextEcho`：`selectionChange` 是输入法自己改动的回声、回声已到齐、读上文后没有新改动、读时没有还没落地的异步改动，并且 `textChange` 报来的文字长度变了时，邮箱后缀不重读。点光标、拖选区不改长度，照常重读。手机默认模式下每个字母从四次同步读降到一次。
- `CandidateCanvasDecoder` 改经 `CandidateCanvasLeaseCache` 取用和放手：最后一个读者离开时停放最近一份，同一 key 再被取用直接接回；最多停放一份，换皮肤或切深浅色时旧份被挤掉。键盘收起、停止输入和扩展销毁时 `trim`，到下一次取用之前离开的读者直接释放，隐藏的键盘不占位图内存。
- 手写移动事件只从上一个点到新点补画一段（`drawHandwritingSegment`）。半透明墨色（「跟随皮肤」可能取到带透明度的文字色）在接点会叠深，这时退回整幅重画，判定在 `HandwritingStrokePolicy.opaqueInk`，认不出的颜色一律按半透明处理。按下、抬笔、撤销、重做、清除和画布重建仍整幅重画，定稿画面与原来一致。
- `SharedStateMirror.fingerprintOf` 对本地普通文件和偏好按路径缓存内容指纹（`FingerprintCache`），记下当时的 inode、大小和纳秒修改时间，身份不变就用上次的指纹，拿不到纳秒修改时间或读不到时不缓存。指纹仍是内容指纹，防止两边来回打转的不变量不变。`reloadChangedPreferences` 在 `preferences.json` 文件身份与上次相同时直接用上次读到的修订号（`storedRevisionIfMoved`）。

## 可见性与排程

键盘可见性由 `idleDictionaryVisible` 单独记录：`attach` 置真，收起或失焦时置假。`keyboardHide` 不一定清除编辑框连接，不能把 `inputClient` 仍存在当成键盘仍显示。隐藏时有进展的个人词库排空可以继续；后续批次一旦停滞，排程入口立即停止重试，后台重建会话也不能重新开启快照查询。`idle-dictionary-lifecycle.test.cjs` 执行会话的真实排程方法，用受控计时器覆盖「显示时推进 → 隐藏 → 首次停滞 → 不再排程 → 再显示恢复」以及隐藏前已经停滞的情况；`run.sh` 同时检查显示和隐藏事件对可见状态的接线。

## 与共享 Rust PR 的关系

本 PR 叠在共享 Rust 那条 PR（`perf/rust-hot-path`，见 [共享 Rust 层空闲 I/O](2026-10-27-shared-rust-idle-io-hot-path.md)）上，在它之后合入，只含鸿蒙宿主的改动。打字统计的进程内攒批与 `{"operation":"flush"}`、快照队列的 `skip_when_idle` 和版本没变时不重写 `state.json`、候选释义按文件身份缓存与 `msime_client_release_gloss_caches` 都在那条 PR 里，连同它们的 Rust 用例。鸿蒙这边 flush 被拒时只记第一次日志（例如统计目录不可用）；攒批本身在 Rust PR 里还会在会话失焦和销毁时写出一次，所以宿主的 flush 是让收起键盘和发布统计文件前的那段一定落盘。

## Alternatives considered

- **只在 `detach` 时停快照轮询、`attach` 时恢复两秒轮询（审计的第一版建议）**：改动最小，但键盘显示期间每两秒仍要扫整个用户词库算 SHA-256 并 fsync，学习让本机版本几乎每次都变。鸿蒙键盘这份队列没人排请求，显示期间持续轮询换不来任何东西，所以改成每次显示查一次、空了就停，再让空队列上的查询只读。
- **构建会话时先单独发一次只读的 `pending` 查询，再决定调不调 `process`**：要新增一个 NAPI 往返和一个 Rust 动作。在 `process` 和 `state` 上加可选的 `skip_when_idle` 达到同样效果，其他宿主不传就不受影响，少一个协议动作。
- **去掉 `attach` 时 `keyboardSync` 里的发布，每次切换只在失焦时发布一次**：能把切换输入框时的发布减半。没有采用：这次发布也负责扩展启动和进程重启后第一次获得焦点时先把键盘的最新状态和 `applied` 记录发出去，设置应用的 `statisticsInStep` 判定依赖它；去掉要另找时机补，冲突裁决的时序也要重新论证。指纹缓存已经让这次发布只剩每个条目一次 stat 加两份小清单。
- **打字统计按元数据（大小、修改时间、inode）算指纹**：比内容指纹省，但这个文件以外的条目是双向镜像的，元数据指纹会让导入写回的副本看起来变了并被发布回去。指纹缓存保留内容指纹的语义，只在文件身份不变时省掉重读，对所有条目都安全。
- **在 `KeyboardView` 里钉住当前皮肤的解码租约**：视图存在期间位图常驻，与停放的效果接近，但要在视图里另管皮肤切换和收起时的释放，而视图在面板收起时并不销毁。停放放在解码器里，由会话的收起回调统一 `trim`，视图不用知道租约。
- **打字统计按会话缓存的开关先在宿主侧门控**：能在统计关着时连 TaskPool 任务都不排。没有做：开关在设置应用里打开后要到下一次获得焦点才生效，会少计这期间的提交；写盘已经离开 UI 线程，关着时原生库直接回 `recorded: 0`，代价很小。
- **把 `candidateGlosses`、英文补全查询整个挪到 TaskPool**：能让 UI 线程上不再有任何释义或补全的 SQLite 查询，但要新增一层异步代次校验。缓存词库连接之后每键剩下的开销已经很小，先不做。`render()` 在应用翻译结果时第二次调度在线服务和翻译的那部分也没有动。
- **键盘收起时结束语音会话但保留跨输入框继续录音**：录音中途切换输入框会先经过 `inputStop`，改后会立刻结束采集、开始转写，切换后继续说的话不会再录进去。为一个罕见用法让麦克风在面板看不见时保持打开不值得，结果仍按 `voiceTargetMatches` 留给用户显式插入。

## Consequences

- 键盘收起、编辑框仍有焦点时，扩展进程不再每两秒醒来查快照队列，也不会为停滞的队列每两秒或每五分钟重建会话；有进展的大词库导入仍可继续完成。麦克风随面板收起关闭。显示期间聊天应用每发一条消息失焦再聚焦一次，不再每次扫用户词库、fsync 两份状态文件。
- 设置应用在键盘隐藏时排入的快照请求（鸿蒙上目前没有这样的来源）要到下一次显示或下一次构建会话才处理；卡住的个人词库同步在隐藏期间不再重试，下次显示时按退避接着排。
- 退避上限五分钟意味着队列真的卡住后恢复时，最坏要多等几分钟才再试一次；用户在键盘里新排入词条会立即复位。
- 打字统计：有了 Rust PR 的攒批，扩展进程在两次 flush 之间被系统杀掉，最多丢一个攒批窗口（约 30 秒或若干次提交）的计数；收起、失焦、销毁三处 flush 把这个窗口压到最小。这是与 Android 选词攒批（`SELECTION_BATCH`）相同的取舍，换来每个词不再刷一次闪存。成就音比原来晚一个 TaskPool 往返。
- 候选释义缓存每次显示后的第一次查询重新打开一次词库，之后不再每键打开；隐藏期间不占文件句柄和 SQLite 页缓存。
- 停放的皮肤位图在键盘可见、候选条空着时仍常驻，最多一套皮肤（解码预算 800 万像素，约 32 MB RGBA 封顶，通常远小于此）；收起即释放。
- 半透明墨色的手写仍走整幅重画，这条路径的开销不变。
- 直接英文的上文读取依赖每条改动编辑器的路径都推进 `englishSuggestionEpoch`；漏掉一条会让补全条短暂显示过期的词，但选用时会重新核对，不会删错。自动大写的读取（英文纠错关闭时）没有合并。
- 相关笔记：Android 同一批问题的处理见 [Android 键盘只在窗口显示时每秒重读偏好](2026-10-27-android-preference-poll-only-while-visible.md)；共享状态镜像的条目、指纹和冲突规则见 [鸿蒙设置应用与键盘扩展经共享沙箱镜像交换数据](../architecture/2026-10-09-harmony-ime-shared-sandbox.md)；Canvas 候选皮肤解码和预算见 [HarmonyOS 候选区绘制画布皮肤](../feature/2026-10-19-harmony-candidate-canvas.md)；快照队列目录与锁见 [云端快照队列发布绑定目录句柄](2026-10-09-cloud-snapshot-queue-directory-handle.md)。

## Verification

- `bash platforms/harmony/tests/run.sh`：全部通过（0 failure）。新增或扩充的用例覆盖退避间隔与排空进展（`keyboard-logic.test.ts`）、`CaretContextEcho` 回声判定和点光标后重读、`HandwritingStrokePolicy.opaqueInk`、`VoiceRecognitionPolicy.cancelsOnHide`（`kb-view.test.ts`）、`CandidateCanvasLeaseCache` 停放规则（`candidate-canvas-lease.test.ts`）、`FingerprintCache`（`shared-state-mirror-policy.test.ts`）；ArkTS 接线由 `run.sh` 的源码断言钉住。
- `build-native.sh arm64-v8a` 在编译机上用 OpenHarmony NDK 交叉构建通过，NAPI 库按 `-Werror -Wl,--no-undefined` 链接成功，新的 `releaseGlossCaches` 与 `msime_client_release_gloss_caches` 都在产物里。用这份原生库在本机 `ohpm install` 后 `hvigorw clean assembleHap --no-daemon`（command-line-tools 6.1.1，API 24）通过，`CompileArkTS` 实际执行，产出未签名 HAP；19 条 ArkTS 警告都在本分支没改动的行上（`px2vp` 弃用等）。没有暂存引擎资源和设置页，HAP 只用来证明编译，不能装机使用。
- Rust：释义缓存（改写、删除、替换各类文件后与重新打开一致、释放后再查不变）、`skip_when_idle`（空队列 `state`、`process` 不动、排着的请求照样处理）和版本不变时不重写 `state.json` 的用例随实现挪进了共享 Rust PR，在那里的全量 `cargo test` 里跑，结果记在那篇笔记。
- 没有做的：没有在任何鸿蒙真机或模拟器上运行，没有测量耗电、发热、CPU 时间或帧率，收益只来自代码路径分析。验证办法是在低端真机上收起键盘让输入框保持焦点，用 `hidumper --cpuusage` 或 `top` 对比扩展进程的 CPU 时间增量，并在打字、手写、Canvas 皮肤下对比 UI 线程 trace。`panel.on('hide')` 没有接，系统收起依赖 `keyboardHide`，各 API 版本的事件时序未在设备上确认。

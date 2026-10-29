# Agent Note: Android 引擎会话建好之前工具栏和「中」键照常可用

Status: implemented

## Problem

#10086：Android 上每进入一个新输入框（冷启动的应用，或 Via、火狐每开一个新窗口的地址栏），工具栏的表情、常用语、皮肤、输入方式四个按钮和底行的「中」键先灰一到两秒，这段时间点按无效。

这五个入口的可用条件都含 `session != 0`。宿主每换一个输入框就在 `onStartInput` 里 `stop(false)` 销毁会话（`stopSessionState` 同时清掉 `preferencesDirectory`、`emojiResources`、`preferencesSnapshot`），再经 `scheduleEngineStartup` 在单线程的 `preferencesWorker` 上依次做首次安装准备、读实时偏好（`withLivePreferences`）、个人词库同步，最后回主线程 `NativeClient.create`。[工具栏启动状态](2026-10-08-android-toolbar-startup-state.md)（#5680）去掉了会话建好之后再等偏好的那一秒，会话建好之前的空档留了下来；#10065 只让同一输入框重启连接时留着会话，浏览器新窗口的地址栏是另一个输入框，照样重建。诊断包里也没有建会话的耗时，看不出每次具体灰了多久。

## Decision

不改「换输入框就重建会话」的生命周期：它和词库维护的独占锁绑在一起（见[低端机功耗](2026-10-27-android-keyboard-low-end-power.md)），本次只让会话空档不再挡住这五个入口，并把空档有多长记下来。

- **会话状态三分**：`ToolbarReadinessPolicy.session(sessionReady, startupPending)` 给出 `READY`、`STARTING`、`UNAVAILABLE`。`STARTING` 就是现有的 `engineStartupPending()`：这一代启动排上了、没有换代也没有被 `stop` 取消。`UNAVAILABLE`（密码框、数字框、启动失败退回直接输入）的行为一律不变。
- **表情、常用语立即可用**：表情面板只用 `emojiResources` 和输入连接，表情经 `commitText` 直接上屏；常用语由 `CommonPhrasesStore` 自己读 `runtime-options.json` 里的偏好目录。`startInputRuntime` 同步读到运行时选项后由 `prefillSessionIndependentDirectories` 按 `startEngineSession` 的同一套校验（绝对路径）先填上 `emojiResources` 和 `preferencesDirectory`；`stop` 时照旧清掉。两个按钮的可用性改为 `emojiEnabled` / `phraseEnabled`，`ImePanels.showEmojiPicker` 不再按 `session == 0` 拦。常用语的读取原来排在 `preferencesWorker` 上，正好在这一代建会话的偏好读取和个人词库同步后面，空档里点开面板会一直停在「正在读取常用语…」到会话快建好；现在面板读取和「添加到常用语」一起挪到单独的 `commonPhrasesWorker`，两者同线程，刚添加的一条在随后打开的面板里照样读得到。
- **「中」键立即可用**：`STARTING` 时点按走 `toggleInputLanguageBeforeSession`，只翻转宿主的 `dedicatedEnglish` 并照常写 `InputModeStore`；`startEngineSession` 本来就按 `dedicatedEnglish` 调 `setEnglishMode`，键区不一致时重建。英文直输不经引擎，翻到英文后马上能打字。打开「中英键轮换其他语言」且从英文或其他语言出发时，下一步是切方案，要会话和偏好算出启用了哪些语言键盘，这一下记进 `pendingLanguageTap`，会话和偏好就绪后照常 `languageKeyTapped()`。已记下一下时再点不给反馈、不计数（切方案要等保存完才能接着切，就绪后连按只有第一下生效，与其有声有振动却被吞掉，不如这一下明确无效）。收起键盘（`finishInputViewPresentation`）时作废，同一输入框重新弹出时不在新键盘上自己执行。会话在建时这个键不再画淡。日语 9 键左列自己的语言键不在本次范围，仍按会话判断。
- **皮肤、输入方式点按推迟**：`STARTING` 时按钮可点，`ImePanels.showSkinMenu` / `showSchemePicker` 先问 `deferUntilSessionReady`，把这一下记进 `pendingToolbarPanel`，按钮画成选中；`startEngineSession` 建好会话后 `main.post(this::runDeferredToolbarTaps)` 打开它，建会话前没读到实时偏好时由 `reloadPreferences` 第一次读到偏好后执行。推迟的面板在 `closeToolbarPanels`（用户改点了别的面板、收起键盘）、`stop`（换输入框）、再点一次同一个按钮（`deferrablePanelToggle`，和面板开着时再点回到键盘一致）以及开始打字或删字（`type`、`commitText`、`deleteCodePointBeforeCursor` 里的 `dropDeferredToolbarPanel`）时作废，执行时已有别的面板开着也不再弹出。推迟中两个按钮的读屏状态都念「准备好后打开」。
- **建会话耗时记录**：`DiagnosticEventKind` 新增 `engine_session_create`（主线程上 `NativeClient.create` 的毫秒数）和 `engine_session_ready`（从这一代 `scheduleEngineStartup` 排上到会话可用的毫秒数，同一代的重试不重新计时），经 `ImeDebugOverlay.recordPerf` 写进 `perf.jsonl`，只在用户打开「记录输入日志」且隐私判断允许时写。两者标为不上传（服务端不认），Rust 的 `diagnostics::EVENT_KINDS` 同步加上，进本机导出的诊断包。

## Alternatives considered

- **`stopSessionState` 不再清空 `preferencesSnapshot`，皮肤、输入方式在会话建好前直接打开、直接保存**：triage 计划的原方案，用户点下去面板马上出来，体验最好。不用的原因在保存路径上：`saveKeyboardSkin` 记下 `targetSession` 并在 `preferencesWorker` 上排队写偏好，而建会话的那一轮实时偏好读取排在它前面，会话建好时 `applyPreferencesSnapshot` 先把皮肤画回旧的；保存回来时 `finishKeyboardSkinSave` 见 `session != targetSession` 直接返回，`skinSaving` 一直为真，皮肤面板再也打不开。切方案要经会话的 `applyPreferences`，更没法在会话之前做。保留旧快照还会改变另外七十多处读 `preferencesSnapshot` 的地方在空档里的语义。推迟到会话就绪再打开，点按不丢，代价是面板晚出来等于空档的长度。
- **把 `NativeClient.create` 挪出主线程**：主线程上建会话期间所有点按都在排队，这也可能是「点了要缓一会」的一部分。没做：会话句柄是线程绑定的（`host-api` 的约定），挪线程要重新核对 JNI 的线程约束，本次先用 `engine_session_create` 量出它到底占多少。
- **让会话跨输入框保留**：从根上消掉空档。没做：换输入框时销毁会话是词库维护拿独占锁的时机（`onFinishInput` 里的个人词库同步和快照处理），改它要另立计划。

## Consequences

- **收益**：新输入框里键盘一弹出，表情、常用语和「中」键就能用，皮肤和输入方式不再是灰的、点按不丢；下次有人反馈「键盘弹出来要等」，诊断包能直接给出建会话本身和整段等待各多少毫秒。
- **代价**：皮肤、输入方式在空档里点下去，面板要等会话建好才出来，等的时间没有缩短。会话在建时翻到英文后又在同一代里失败退回直接输入，翻转照样已经记进 `InputModeStore`，与有会话时切中英的记法一致。功能面板里的表情磁贴仍按会话判断，不在本次范围。
- **未覆盖**：只有 `ToolbarReadinessPolicySmoke`、`check-host.sh` 的接线守卫和编译；没有在设备上量过空档、也没有在 vivo Android 11 上回归，需要 API 28/35 模拟器和报告者真机确认。iOS 和鸿蒙的同类情况没有核对。

## Verification

`platforms/android/tests/keyboard/ToolbarReadinessPolicySmoke.java` 覆盖三种会话状态下五个入口的可用性、推迟和「中」键的四种处理，以及已记下一下时再点「中」键不再记；`ImePrivacyGateSmoke` 钉住新增的两个事件种类；`crates/client-services/src/diagnostics/tests.rs` 的 `engine_session_timing_records_stay_in_local_bundles` 确认这两种性能记录进本机诊断包、不在上传白名单里；`check-host.sh` 钉住宿主接线，包括常用语的读写在 `commonPhrasesWorker` 上、推迟面板的取消和作废、收起键盘时作废记下的「中」键。

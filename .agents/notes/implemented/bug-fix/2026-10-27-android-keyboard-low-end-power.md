# Agent Note: Android 键盘在低端机上的耗电、发热与卡顿：只在真有变化时做整套工作

Status: implemented

## Problem

低端手机用户反馈水杉键盘耗电、手机发热、打字卡顿。一轮性能审计在 Android 宿主上找到 18 处热路径（`perf-findings.json` 的 idx 0–17），共同点是「没有变化也每次整套重做」：

- 键盘显示期间偏好轮询每秒一轮，每一轮都经 JNI 读、校验、序列化整份 `preferences.json`；自定义候选皮肤明暗两份各做两次 `resolveTheme`、一次 `skinPackage`，并把每张图读出、base64 过 JNI、`org.json` 解析、解码再算 SHA-256；回到主线程后不看内容是否变化，深拷贝快照、`updatePreferences` 等三次 JNI、清一次剪贴板历史、重读设计库；照片键盘皮肤下还把半兆以上的设计序列化四遍算缓存键。键盘每次显示又连着 `start` 两三次，每次立即整套再读。
- 触屏中文默认的横向候选条每键调 `allCandidates`：Rust 序列化整代候选，Java 在主线程用 `org.json` 整份解析，而条上只建 48 个按钮；首字母时还逼引擎放出暂扣尾部、按 `UNLIMITED_ROWS` 查出整张 `tbl_1_<字母>`（483 到 4490 行）并换一代。
- 每次 `render` 丢掉整排候选按钮再新建，每个按钮换肤时新建五块 `GradientDrawable`、一个 `StateListDrawable` 和一个 `ColorStateList`；同一次 `applySkin` 递归整棵键盘树，包括收着的面板和打开过的几百格表情、符号网格。
- 英文直输每个字母约 22 次阻塞的 `InputConnection` 跨进程读取（光标位置、前后文各被候选条、清除键、自动大写和 `render` 各读一遍），整页重画 3 次。
- 每次 `onStartInput`（包括聊天应用发送后对同一输入框 `restartInput`）都在主线程销毁引擎会话、在工作线程重读偏好和皮肤、同步个人词库、再建会话，并整组重建字母键；焦点落到桌面、设置页这类 `TYPE_NULL` 窗口时也在后台建一个没人用的会话。
- 每次 `onFinishInput` 把 `DictionarySnapshotWorker.process` 直接排进 `preferencesWorker`，Rust 不管有没有排着的请求都先按顺序扫完用户词库三张表逐行算 SHA-256，下一个输入框的建会话排在它后面。
- APNG 候选皮肤每画一次就 `postInvalidateOnAnimation`，候选停在屏上时整列按 60 到 120 Hz 重画，而 APNG 一般约 100 ms 才换一帧。
- 打开「记录输入日志」后每行日志都把整个 jsonl（上限 1 MiB）读出、拼接、写临时文件再改名。
- 打字统计每次上屏整份重写并 fsync（这一半在 Rust，见下文）。

窗口收起后的每秒轮询已由 [键盘只在窗口显示时每秒重读偏好](2026-10-27-android-preference-poll-only-while-visible.md) 停掉；本篇处理的是键盘可见、正在打字和换输入框时的开销。

## Decision

总原则：能证明输入没变时跳过整套工作，证明不了就照旧做；每条跳过都有一个必定走完整路径的边界兜底。

### 偏好轮询（idx 0、1、6、7、9、13、16）

- `CandidateSkinResolver`：轮询中的后续读取（`fresh` 为假）先对 `preferences.json` 和两个槽位指名的候选包目录做 stat（`CandidateSkinStamp`：inode、修改时间、大小，包目录逐个条目，外加 skins 根目录），都与上次整套读取时相同就交回上次的响应，不调 `loadPreferences`、`resolveTheme`、`skinPackage`、`skinResource`。戳在读之前取，读的过程中被改则下一轮重读；读或解析失败不缓存；包目录条目超过 `MAX_PACKAGE_ENTRIES`（256）时不给戳，每轮照常整套读。
- `PreferencesReloader`：`start` 之后的第一次读取 must 是 `fresh` 真读，所以每次键盘显示仍整套重新校验一遍皮肤包。同一目录这一轮首次读取还没交付时再 `start` 只换接收者、不另起一轮。新增 `startAfter`：建会话时刚应用过实时偏好，一秒后再按平常节奏轮询。
- `MSIMEInputService.reloadPreferences`：响应与上次整套应用成功的完全相同，且文档之外的输入（会话、目录、本地设置、月份、系统明暗、学习开关、键盘内调高度、剪贴板模式与存储、候选包解析结果、设计库文件戳）都没变、`preferencesNotice` 为空时直接返回。任何来源的 `applyPreferencesSnapshot` 开头都清空这份记录，建会话和停会话时也清空。
- `clearOnlineProvider` 只把 `msime_client_clear_online_candidates` 操作结果内的 `view` 放进视图字段，先确认 `applied`。这个接口的 `value` 是 `{applied, view}`，不是视图本身；误存外层后方案号和网格都读不到，键区回落成 QWERTY。原来每秒重复应用偏好会纠正它，跳过重复应用后错误会一直保留，因此去重必须同时修正这条返回契约。
- `themeSkin` 的缓存键按设计对象记下内容散列（`WeakHashMap`，身份语义），同一份快照的四块面板不再各自序列化照片。

### 候选条（idx 3、4、12）

- 改用共享 Rust PR 新增的 `Runtime::candidate_range(start, count)` 与 C ABI `msime_client_candidate_range`（见 [共享 Rust 层空闲 I/O](2026-10-27-shared-rust-idle-io-hot-path.md)），本 PR 补 JNI、`NativeClient.candidateRange` 和 `verify-native.sh` 清单。它只复制座位 `start` 起的一段，身份与 `all_candidates` 同一座位相同；暂扣尾部只在 `start` 走到已放出的末尾时才放，放出照旧换一代。`all_candidates` 不变，展开面板、九键面板仍用它。
- `ImeCandidateStrip` 每键只取第一批，滚近尾部再取下一批。英文释义和音标改走 `displayedCandidateSnapshot`：展开面板开着时取整代，否则从 0 起取已放出的那一段（上限沿用释义请求的 4096 条），不放尾部、不换代。
- 候选按钮放进按位置复用的池子，`bindStripCandidateButton` 每次重绑点击、长按并重设全部逐候选状态；样式按按钮记下上次装上的底，外观（`styleCacheKey`）和密度都没变时不再新建 Drawable。
- 安装产品的旧候选设备回归使用同一个创建、重绑入口：旧代次点击仍忽略，把同一按钮重绑到新候选后必须正常上屏。测试不再反射已移除的逐次创建方法。
- `ImeStyler.applySkin` 跳过 GONE 子树并记下来；表情面板和手写键区只按各自皮肤走一遍。兜底：键盘根视图挂画帧前监听，记下的子树被直接显示出来却没换肤时，在画下一帧前补一遍。

### 英文直输（idx 2）

`EditorReadCache` 在调用方显式框起来、不写编辑器的一段代码里，光标位置、光标前 128 字和光标后 1 字每样只读一次；输入法自己的上屏、删除、发按键、编辑器动作和换连接都会作废它。`render()` 支持合批。选区回声只有在英文直输里是输入法自己写入的、写入后已完整重画过（`SelectionEchoTracker` 新增只增不减的预期计数）、相关面板没开、大小写/当前词/英文候选都没变时才跳过重画。

### 输入框边界（idx 8、11、15，部分）

- 同一输入框 `restartInput` 时留着引擎会话，条件是：字段、输入类型、`imeOptions`、包名都相同；引擎里没有组字或已选的半段词；没有待做的隐私重建；此刻该不该学习与会话建立时一致；本机已装资源包集合（`installedResourcePackIds`）没变。留会话时引擎按换输入框的规矩复位：`focus(false)` 写下选词和延迟学习、给脚本插件 `focus_out`，`focus(true)` 丢掉组字、上下文和联想并重读插件表、资源包和音效，再重交隐私标记、中英、全半角和中文标点。任一条件不满足 must 照旧销毁重建。
- 焦点落到 `TYPE_NULL` 窗口、键盘没显示也没接实体键盘时，会话、键行更新和重画推迟到 `onStartInputView`；推迟期间接上实体键盘或来了实体按键时立即建。
- 字母键行（26 键、韩文、大千注音）只在布局、分离式键盘、数字行、中英/数字键互换、数字页状态或容器变了时重建，否则只做重建时顺带的复位；九键、14 键、笔画、手写和符号层照旧整组重建。
- 建会话的 `focus` 可能先按临时中英状态重画键区。恢复编辑器的中英状态后，must 比较 `drawnTouchLayout()` 与最终布局：建会话前预测的布局即使和最终值相同，中间实际画出的键区也可能已经变成 26 键，不能据此省掉重建。否则工具栏显示「双9」而按键仍是 QWERTY。

### 词库快照处理（idx 14）

失焦后的快照处理延后 500 ms 并带代次，下一个要建会话的输入框取消它。`DictionarySnapshotBacklog` 记下被取消的那一轮从何时开始欠；欠满 `MAX_DEFER_MILLIS`（60 s）后下一次失焦不再延后、立即排队且不可取消。服务销毁时还欠着的那一轮在关掉工作线程前补排。

### 其余（idx 10、17、5）

- APNG：`CandidateApngFrames.millisUntilNextFrame` 算出当前帧还剩多久，`CandidateCanvasRenderer` 取所有 APNG 的最早换帧时刻 `postDelayed` 约一次重画；单帧、减弱动效、有限循环停在末帧时不约。GIF/WebP 仍走 `AnimatedImageDrawable` 自己的节奏。
- 输入诊断日志：文件已存在、只有一个链接、追加后不超上限时用 APPEND 原地写新行，打开前后核对 `fileKey` 和链接数，写到一半失败截回原长度；其余情况仍走先写新文件再替换，不跟随硬链接、1 MiB 上限和留四分之三的规则不变。
- 打字统计：在 `onFinishInput`、`onWindowHidden`、`onDestroy`（统计工作线程 shutdown 之前）各排一次 `flush`，与 `record` 走同一条 `typingStatisticsWorker`，只对自上次 flush 以来记过上屏的目录发；`PendingTypingStatistics` 在记录目录变化时先交回旧目录。flush 结果一律忽略。

## 与共享 Rust PR 的关系

本 PR 叠在共享 Rust 那条 PR（`perf/rust-hot-path`，见 [共享 Rust 层空闲 I/O](2026-10-27-shared-rust-idle-io-hot-path.md)）上，在它之后合入。打字统计的进程内攒批与 `{"operation":"flush"}`（失焦或每 30 秒写一次，这是维护者定下的跨平台约定）、`candidate_range`、`personal_dictionary_sync` 无变化时不重写 `sync.json`、快照处理用的预览版本不再全表扫描，都在那条 PR 里，本 PR 只有 Android 宿主的改动。

Rust PR 自己在会话失焦和销毁时就把攒着的统计交给后台线程写出；本 PR 在 `onFinishInput`、`onWindowHidden`、`onDestroy` 再排一次 `flush`，排在同一条统计工作线程上已提交的 `record` 之后，保证最后一段上屏在收起键盘和服务销毁时一定写到盘上。

## Alternatives considered

- **偏好轮询按文件 mtime 跳过**：[上一篇](2026-10-27-android-preference-poll-only-while-visible.md) 否掉过，最强的反对理由是快照来源不止 `preferences.json`，漏一项就是「设置改了键盘不变」。这次采用 stat 戳，但把那篇担心的来源拆开处理：文件类来源（偏好文件、指名的候选包、skins 根目录）进戳；本地设置、应用主题、系统明暗、设计库等在主线程的跳过判据里逐项比较；每次 `start` 后的首次读取总是真读，所以同秒同大小的改写最多留到下次键盘显示。
- **只在主线程比较响应字符串，工作线程照常读**（审计 idx 9 的最小方案）：能去掉主线程的整套重新应用，但每秒仍有整份读文件、皮肤图片 base64 过 JNI 和 SHA-256，那正是带图皮肤的主要开销，所以两层都做。
- **按皮肤指纹整个跳过可见树的 `applySkin`**：省得最多，但 `render` 新建的候选、英文建议按钮和选中状态变化都靠这一遍上色，需要逐视图的脏标记，风险大得多；只跳过 GONE 子树加画帧前兜底。
- **同一输入框所有 `restartInput` 都留会话**：[同编辑器重启时保留拼音组字](2026-10-19-android-same-editor-composition-restart.md) 否掉过，因为应用可能已清空或改写文字，会把旧组字带到新位置。这里只在引擎里没有组字时留，并用 `focus(true)` 丢掉组字和上下文，与那篇的判断不冲突；有组字的重启仍走那篇的核对路径。
- **失焦后的快照处理无条件延后、可被取消**（`1f0ea8ddfb` 的第一版）：审查指出一直在几个输入框之间切换、从不闲下来的用户，每一次都被取消，排着的快照激活和输入记录合并永远做不成，改动之前每次失焦都会做；于是加 60 s 欠账上限。
- **诊断日志一律原地追加**：最省，但会跟随硬链接改写诊断目录外的 inode，破坏原有保证；只在能证明是单链接普通文件时追加。
- **在 Java 里给打字统计攒批**（审计 idx 5 建议仿 `KeyPressBatch`）：iOS 和鸿蒙也按每次上屏调同一个 `record`，攒批放在 Rust 三端共用，宿主只负责在边界叫 `flush`。

## Consequences

- 收益：键盘可见但没改设置时，每秒一轮只剩几十次 stat；候选条每键不再解析整代 JSON、首字母不再查整张声母表；英文直输每字母跨进程读取约 22 次降到 9 次、重画 3 次降到 2 次；聊天发送后的重启连接不再重建会话；切应用经过桌面不再建会话；换输入框不再等用户词库全表扫描；APNG 皮肤只在换帧时重画；诊断日志整份重写从每行一次变成约每 256 KiB 一次。
- 延迟换耐久：偏好文件同秒同大小的原地改写，键盘显示期间可能漏看，到下次显示时补上。一直在输入框之间切换时，排着的快照激活最多晚一分钟（主进程按旧版本排的请求原本就由 Rust 判成冲突后重排，不会用错数据）。留着会话的重启连接要等真正离开输入框才轮到个人词库维护。打字统计攒批（在 Rust PR 里）之后，进程在 flush 之前被杀会丢最多约 30 秒的统计，这是那条约定接受的代价。
- 新的不变量：任何来源的 `applyPreferencesSnapshot` must 先清空「上次整套应用」的记录；`keepsSessionAcrossRestart` 新增判据 must 同时检查本篇列出的全部条件，缺一项就会把旧会话的状态带进新连接（资源包那一项就是审查时补上的，`76b4fbd8c3`）。
- 代码量：`MSIMEInputService` 净增约 360 行判断与兜底，新增 `CandidateSkinStamp`、`EditorReadCache`、`InputViewRefreshPolicy`、`DictionarySnapshotBacklog`、`PendingTypingStatistics` 五个小类，以及对 Rust PR 新 C ABI 入口 `msime_client_candidate_range` 的 JNI 接线。

## Verification

- `CandidateExpansionDeviceSmoke` 按新的候选读取边界分别覆盖：收起面板时逐键请求释义不换代；展开面板时释义与手动展开仍释放暂扣候选并同步当前代次。每轮隔离面板状态，避免上一轮展开状态影响下一轮。API 35 运行 `40647464497` 的实际输出 `shoudao gloss and panel expansion, rapid typing and repeated candidate selection` 通过；同轮整套设备验收仍有失败，不把这一项通过写成整套通过。

- 审查整合共享 Rust 来源修复后的 `bash scripts/verify-local.sh --quick` 完整通过：Windows 便携测试 176 项、仓库契约、笔记、Rust workspace、Android 目标及 JVM 冒烟、WASM 与严格 clippy、Linux 设置外壳和原生宿主（CTest 汇总 112 项，含环境条件跳过）、macOS 原生构建及 Apple 桥均通过。鸿蒙完整 HAP、MinGW 原生宿主和 Windows 专用配置因本机缺少输入或工具链跳过；鸿蒙设置包因这个工作树未安装前端依赖跳过。

- `ANDROID_SDK_ROOT=$HOME/Library/Android/sdk bash platforms/android/check-host.sh` 退出 0：292 项 JVM 冒烟、248 份设备测试源码编译、宿主资源检查通过。新增或扩充的冒烟：`CandidateSkinStampSmoke`（原地改写、原子替换、装包、换嵌套图片、根目录新增包时戳变化，条目过多不给戳）、`PreferencesSmoke`（同目录合并、`fresh` 只在每轮第一次、`startAfter`）、`EditorReadCacheSmoke`、`InputViewRefreshPolicySmoke`（推迟建会话、留会话的判断，隐私与学习开关不一致必须重建）、`SelectionEchoTrackerSmoke`、`DictionarySnapshotBacklogSmoke`、`PendingTypingStatisticsSmoke`、`ImePrivacyGateSmoke`（原地追加、写满仍丢最旧行、硬链接断言照常）。视图层改动 JVM 碰不到，`check-host.sh` 加了接线守卫（候选条从池子取按钮、`keepsChipFace`、`applySkin` 跳过 GONE 并挂兜底、渲染器不再 `postInvalidateOnAnimation`、快照延后与销毁补做、统计 flush 三处接线）。`CandidateCanvasDeviceSmoke` 补上帧边界、有限循环停止和减弱动效断言，但没有在设备上跑。
- Rust：`candidate_range` 的 input-runtime 与 host-api 用例（一批之内不放尾部、越过末尾才放并换代，真实全拼引擎与整代复制逐行一致、可经 `select_any_candidate` 选中）随实现一起挪进了共享 Rust PR，在那里的全量 `cargo test` 里跑，结果记在那篇笔记。
- `StaleCandidateDeviceSmoke` 的按钮实例是 `ContactCandidateButton`，`bindStripCandidateButton` 声明的参数则是 `KeyboardPressButton`；反射按声明类型查找，不能用运行时子类的 `getClass()`，否则还没进入旧回调断言就抛 `NoSuchMethodException`。九键双拼套件等待可用的真实语言键出现后再建立中文前置条件，避免把冷启动第一帧当成已恢复的语言状态。保留所有组码、上屏及候选复用断言。
- `ClipboardExportDeviceSmoke` 显式开启 `touch_toolbar.clipboard` 并恢复原值：它与 `clipboard_history` 是独立开关，共享默认值为关闭，只开启历史记录不会出现工具栏入口。`TextEditRecentClipboardDeviceSmoke` 经正式 JNI 解析补齐序列化省略的默认排列，不再把缺少默认键当作格式损坏。三项反射、语言和默认值修复连同导出入口前置条件经严格 `javac --release 17 -Xlint:all -Werror`、DEX 与 APK 签名构建通过。十八键真实横滑前等待按键范围稳定，并拆开右滑、取消、回中心、反向各阶段的诊断；这些手势断言仍需设备重跑。
- 清除在线候选的返回契约修复由原版设备套件验证：专用 API 28、35 AVD 上的十八键全拼、九键双拼、十八键双拼各完整通过，覆盖禁用云候选、切方案、编辑器回落与几何；API 35 的 `NineKeyNumericLayoutDeviceSmoke` 在按锁校验并临时配置笔画资源后也完整通过，配置在测试后恢复。修复前的诊断只记录布局、方案和网格，实际观察到 `scheme=-1`、`grid=none` 与 QWERTY 回落；修复后没有保留临时诊断日志。
- 覆盖边界：上述原版专项不替代 API 35 CI 的整套验收，整套结果见对应 PR。没有任何耗电、发热或帧时间的实测数字，上面的「收益」都是按调用次数和数据量推算的。需要真机补的：低端机上键盘可见 5 分钟的 `:ime` CPU 时间增量（`adb shell top`、`dumpsys batterystats`）；释义异步到达时点候选、长按菜单、读屏描述；换皮肤和明暗后再打开表情、符号、剪贴板、更多工具面板是否是新皮肤；自定义主题下的手写键区；聊天应用发送后的重启连接、密码框与数字框之间切换、隐私开关切换；`TYPE_NULL` 弹幕框的九键和 26 键中文；装删资源包后对同一输入框重启连接。

# Agent Note: iOS 键盘开着 ONNX 重排时的内存：旧键盘视图释放、重排器进程内共用、iOS 关 ONNX Runtime 内存池

Status: implemented

## Problem

iOS 键盘扩展有硬上限，2026-11-02 的 iPhone 17（iOS 27.2）上是 176 MB（`mem` + `avail`）。同一天开着整句联想「增强」（8x448 ONNX 重排，见 [ONNX 后端](../../implemented/architecture/2026-11-01-onnx-rerank-backend.md)）的诊断日志里有两件事：

1. **重新加载键盘一次涨一截。** 同一个键盘进程里每次系统重新加载键盘（换了宿主应用，`focus_out` 之后又一行 `keyboard_loaded`），`base=` 涨 10 MB 左右：82 → 91 → 102 → 112，内存警告后 149 → 160，`focus_in mem=174 avail=2`，10 秒后进程被换掉。更早的构建里每个会话各加载、自测一份大模型，每次涨二十多 MB（7 → 29 → 51）；改成进程里共用一份后还剩这 10 MB。
2. **不重新加载也只剩二十来 MB。** 一个进程连续打字 30 分钟，大模型挂上后约 105–110 MB，打字中稳定在 152–157 MB，`avail` 19–24 MB，期间收到过一次内存警告。

同样的「每个会话各加载、自测一份」在 macOS 上更明显：输入法进程三分钟里建了 11 个会话，各加载、自测一次大模型，phys_footprint 到 558 MB（峰值 686 MB）。

第一件不在重排器里：iOS 27 的 `UIInputView` 自己释放不掉。它和 UIKit 挂在它下面的 `_UIInputViewContent` 互相强引用（`_leftContentView` ↔ `_inputView`），一个刚建好、从没进过窗口的 `UIInputView` 也是这样；控制器释放了，输入视图和挂在它下面的整套键盘（按键、候选条、皮肤图片）都留下。模拟器上在测试宿主里连续建、显示、收起、释放十次控制器，占用 38 → 109 MiB。

2026-11-04 复查多个 PR 的 iOS CI 后发现，直接摘下整个键盘会在销毁时引入另一种故障。[失败运行的 xcresult](https://github.com/metasequoiaime/msime/actions/runs/41805436283) 保留的符号栈是 `KeyboardViewController.deinit` → `removeFromSuperview` → 递归 `_switchToLayoutEngine` → `NSISEngine.removeConstraintWithMarker` → `_engineVar_rawRemove`，最终报 `Not possible to remove variable`；同批还出现布局系数倒数溢出和精度重建失败。报错用例虽然分散在布局、滑行和短语定位中，异常实际在它们结束、控制器释放时发生。在最新 `develop` 上，不把键盘放进窗口、交替全拼与九键、组字并改键距后反复释放，也能在 iOS 26.5 和 27.0 复现相同断言。已展示键盘的内存回归不足以覆盖这条销毁路径。

第二件主要是 ONNX Runtime 的 CPU 内存池：它按一次运行用到的最大量留着内存、只增不减，而上文和候选前缀的缓存每次按键长度不同。

2026-11-04 在 [#10864](https://github.com/metasequoiaime/msime/issues/10864) 继续复查：公开的批量 `deactivate` 仍会触发相同的 `_engineVar_rawRemove` 异常，不能把之前有限轮次通过当成根因已经消除。iOS 26.5 的独立宿主重复测试中，原未展示键盘回归十轮有五轮失败；去掉观测器对 `intrinsicContentSize` 的读取后，十轮有七轮失败。十二个首次异常都停在隐藏底部提示的普通应用约束 `UILabel.trailing = KeyboardKeyButton.trailing - 2`。标签同时带着 UIKit 生成的、宽高为零的 `NSContentSizeLayoutConstraint`；这是失败所在的子图，尚不能据此推断 UIKit 内部哪一步先破坏了变量归属。仅在诊断注入中去掉底部提示的三条边约束，其他输入、尺寸、等待和清理保持原样，原两条失败测试各十轮全部通过，相关四类 155 项也为 154 项通过、一项既有词库跳过、零失败。

## Decision

- **控制器释放时把键盘视图从输入视图上摘下来。** `KeyboardViewController` 的 `deinit` 在主线程上（UIKit 总在主线程释放视图控制器，所以用 `MainActor.assumeIsolated`）调用 `KeyboardViewTeardown`：先收集待销毁视图树的约束，用公开的 `NSLayoutConstraint.deactivate` 一次停用，再摘下输入视图的子视图，避免逐项迁移包含约四千个变量的完整约束图。不逐层改动排列关系，也不拆控件内部视图。清理只发生在控制器销毁时，正常布局与输入路径不变。输入视图本身和 `_UIInputViewContent` 照旧留下，约 2 KB；挂在下面的全部释放。原模拟器上同样十次是 26 → 28 MiB，清理顺序改变后仍由原内存预算回归检查，不靠撤销释放来规避布局崩溃。
- **底部双拼提示按按键的最终尺寸直接排版。** `KeyboardBottomHintLabel` 仍是单行 `UILabel`，保留 9pt 常规字体、原皮肤强调色的 0.8 透明度、居中、最小缩放 0.6、左右各 2pt 和底部 2pt。`KeyboardKeyButton.layoutSubviews` 取得最终键面尺寸后安排它的 frame；自然高度由 `sizeThatFits` 读取，零尺寸或放不下边距时收为零，恢复尺寸后重新布局。文本改变会让父按键重排，缩字与绘制继续交给系统标签。控制器只创建并注册标签，不再为这组装饰性提示安装三条边约束；角标、主键面、配置和用户默认值保持原行为。
- **重排器进程里只加载一份，自测只做一次。** `ffi::OnnxRerankers`（host-api）是进程级缓存，各平台的宿主都经它挂重排器：同一时刻只加载一个模型，各会话经 `Arc<Mutex<OnnxScorer>>` 共用；档位在第一次挂上时定下，之后的会话沿用，不因为某一刻的内存多少在大小模型之间来回换；大模型的自测结果按根目录和 `lm_step.ort` 的长度、修改时间记下，放开再挂不重测，重新加载的那一份经 `OnnxScorer::accept_calibration` 带上之前的结论（过了预算的大模型同时排上屏后联想）。「增强」关掉时放开，模型文件换了重新定。要换模型时先放开缓存里旧的那个再加载新的。
- **可用内存不够时不加载大模型。** iOS 上 `os_proc_available_memory()` 少于 `LARGE_RERANK_MODEL_MIN_AVAILABLE`（120 MiB）且两档都装着时，直接用小的，不先加载大的去自测；类别写成 `keyboard reranker onnx small (low memory)`。门槛是大模型的加载峰值 74 MiB 再留 46 MiB：会话建立之后键盘界面才画出来，iPhone 17 上那一段是 20 MB 左右，打字时词库页面和候选缓存还会涨。拿不到余量（模拟器、其他平台，函数返回 0）时不按内存挑。
- **iOS 上关掉 ONNX Runtime 的 CPU 内存池。** `OnnxScorer::load` / `load_file` 多一个 `memory_arena` 参数，host-api 的 `RERANK_MEMORY_ARENA` 只在 `target_os = "ios"` 时为假。其他平台没有扩展这样的硬上限，留着内存池省下每次运行重新向系统要内存的时间。
- **让 ONNX Runtime 自己读模型文件。** 宿主用 `OnnxScorer::load_file`（`commit_from_file`），不先把 31 MB 的文件读进 `Vec` 再交过去，加载那一刻少一份文件内容。

## 实测（2026-11-02，iOS 27.1 模拟器，M 系列 Mac，`msime-onnx-rerank` 的 `footprint` 示例）

示例编成 `aarch64-apple-ios-sim`、用 `xcrun simctl spawn` 在模拟器里跑，用的是 iOS 的内存分配器（Mac 上的分配器释放后迟迟不还，phys_footprint 偏高好几倍，量不准）。数字是 phys_footprint 比加载前多出的 MiB；打字负载是 80 次组字、共 1272 次按键，每次 9 个候选，前缀随按键增长并共享。

| 模型 | 内存池 | 读法 | 加载后 | 打字后 | 加载峰值 | 每键 p50 / p95 |
|---|---|---|---|---|---|---|
| 8x448 | 开 | 读进内存（原来） | 39.6 | 79.8 | 103.8 | 1.91 / 9.25 ms |
| 8x448 | 开 | 运行时读文件 | 40.5 | 80.7 | 80.7 | 1.91 / 9.24 ms |
| 8x448 | 关 | 运行时读文件（iOS 现在） | 39.7 | 45.8（最高 47.2） | 74.4 | 2.72 / 11.77 ms |
| 6x256 | 开 | 读进内存（原来） | 10.6 | 28.3 | 28.3 | 0.67 / 2.65 ms |
| 6x256 | 关 | 运行时读文件（iOS 现在） | 9.4 | 10.6（最高 11.7） | 17.2 | 1.01 / 3.73 ms |

大模型自测从 4.0 ms 到 4.5 ms，仍远在 10 ms 的预算之内。另试过的组合（同一负载，内存池开着）：关掉内存图样（memory pattern）打字后 69 MiB；把跨按键缓存的前缀上限从 512 降到 128 是 57 MiB，再关内存池是 40 MiB，p95 都再高 0.5–2 ms。

按这些数估真机：打字中的稳态从 152–157 MB 降 34 MB 左右，`avail` 从 19–24 MB 到 50 MB 以上；重新加载键盘不再累积。这是估算，真机的数要等下一份诊断日志。

## Alternatives considered

- **内存池开着、把前缀缓存上限降到 128** — 不增加每次运行的开销（p95 9.9 ms 对 9.3 ms），打字后 57 MiB，省 23 MiB。但余量只到 40 MB 出头，刚碰到目标的下沿；内存池留下的那部分仍随负载变化，最坏的那一次按键决定它留多少。关内存池省得更多，而且省下的量不靠负载形状。前缀上限在关了内存池之后只再省 6 MiB，却让 p95 再高约 2 ms，保持 512 不动。
- **把模型文件映射进内存、让 ONNX Runtime 直接用映射的字节（`use_ort_model_bytes_directly` 加 `use_ort_model_bytes_for_initializers`）** — 加载峰值最低（关内存池时 64–72 MiB），映射的干净页本身不计入 phys_footprint。实测打字后反而比读文件多 10 MiB 左右（57 对 46 MiB）；还要在 crate 里加 `unsafe` 的映射并让映射活得比会话久。省下的只是峰值的几 MiB，不值。
- **关掉内存图样（memory pattern）** — 输入长度每次不同时它本来就该关，但实测只省 10 MiB，与关内存池重叠，单开它不够。
- **内存警告时放开缓存里的重排器** — 挂着它的会话还在用，放开也省不下内存；会话结束后下一个会话又要加载一遍，加载峰值反而更高。
- **用私有接口拆开 `UIInputView` 与 `_UIInputViewContent` 的互相引用，或者不用 `UIInputView` 当键盘的根视图** — 前者碰私有字段，审核和下一版系统都可能出问题；后者 `UIInputViewController.inputView` 声明的类型就是 `UIInputView`，系统按它取键盘外观和音效，换成普通视图要在真机上逐项验证。摘子视图只用公开接口，漏掉的只有 UIKit 自己那个空壳。
- **只在 iOS 27 执行摘除** — 原泄漏最初只在 iOS 27 上量过，而 CI 的约束异常出现在 iOS 26.5；按版本限定兼容处理本来能减少对旧系统的影响。但未展示键盘的反复销毁回归在 iOS 27 同样触发 `_engineVar_rawRemove`；继续在 iOS 26.5 直接读强引用布局，也确认输入视图与内部内容互相强持有。26.5 取消清理的原三条释放/内存测试全部失败，十次重载为 `[43, 50, 58, 65, 73, 80, 87, 95, 102, 112]` MiB，不能按系统版本放弃清理。
- **从叶端逐个拆开 `UIStackView.arrangedSubviews` 后再摘根视图** — 每次迁移的视图较少，而且可以跳过按钮内部的 UIKit 层次；iOS 26.5 的 155 项相关测试通过。但 iOS 27 的既有临时英文切换回归仍在 `removeArrangedSubview` 时触发同一断言，因此不能只缩小每次拆除的范围，改为先批量停用约束。
- **清理前先 `layoutIfNeeded`** — 布局刷新有机会先把尺寸和引擎状态对齐，26.5 的九键和内存类也通过。但原 155 项仍在短语测试中触发 `_engineVar_rawRemove` 后中止，只有 153 项通过、一项既有跳过，不能作为修复。
- **只停应用跨根约束，再摘应用自己的两个直接子根** — 归属边界清楚，不去碰 UIKit 生成的内部约束；原 hint 子图还在时，这个诊断版本十轮内存回归有四轮失败、十轮临时英文回归有一轮失败，首次异常仍发生在应用根 `removeFromSuperview` 触发的引擎迁移。缩小清理所有权不是该子图故障的充分修复，本次先隔离提示布局的影响。
- **按机型名单决定 iOS 用大模型还是小模型** — 上限因机型和系统版本而异，名单跟不上；自测加可用内存是在每台机器上量要决定的那件事。

## Consequences

- **收益**：重新加载键盘不再一次留下一整套视图；重排器每个进程加载、自测各一次，macOS 输入法不再随会话数累积大模型；iOS 上大模型打字时少占 34 MiB 左右，加载峰值少 29 MiB；内存紧的时候第一次挂上就用小模型，不先把大的加载进来。
- **代价**：iOS 上每次图运行都重新向系统要内存，模拟器里大模型每键 p50 多 0.8 ms、p95 多 2.5 ms，小模型 p50 多 0.3 ms、p95 多 1.1 ms；iPhone 17 上大模型整键 p95 原来 4 ms 上下，按比例估在 5 ms 上下，仍在 10 ms 的线内，没有在真机上量过。可用内存不够时定下的小模型会一直用到「增强」关掉或进程结束。共用的重排器每次打分加一把锁，同一时刻通常只有一个会话在打字。
- **已知上限**：数字来自模拟器，模拟器构建不带 ML Kit，内存页的计法也与设备不同；真机的余量和延迟要拿下一份诊断日志和 `rerank_latency` 核对，偏差大时重访 120 MiB 的门槛和内存池的取舍。`deinit` 里的 `MainActor.assumeIsolated` 依赖 UIKit 在主线程释放视图控制器，不在主线程释放时会直接崩溃而不是悄悄泄漏。
- **清理的所有权边界仍有欠账**：`KeyboardViewTeardown` 递归取得的数组包含应用约束，也包含 `UISV-*`、封装布局和滚动引导等 UIKit 所有的约束。它不能被描述成「只停应用约束」。这次保持 helper 不变以隔离提示布局的因果，未证明将清理收窄到应用根后的组合表现，也未通过私有约束标识符筛选生产清理逻辑。UILabel 沿用 frame 布局的自动调整行为，不把「没有显式边约束」扩大成「UIKit 绝不生成任何引擎边」。

## Verification

- 以 `e270741ee9` 为底稿的底部提示 frame 实现，未改 `KeyboardViewTeardown`、原测试前置条件或默认值：iOS 26.5 与 27.0 各运行原四类 155 项，均 154 项通过、一项既有缺笔画词库跳过、零失败（分别 140.936 秒与 149.901 秒）。26.5 的原未展示释放回归、原临时英文回归各重复十轮，每轮重启宿主，共二十轮全部通过。日志分别为 `target/ios26-hint-frame-related.log`、`target/ios27-hint-frame-related-reboot.log`、`target/ios26-hint-frame-repeat.log`。27 的首次新设备启动停在 `host_support_mig_launch_app`，没有执行测试；只重启该专用设备后才取得上述结果，不把启动超时当成产品回归或通过。
- `KeyboardBottomHintTests` 五项覆盖尺寸变化、零/极小尺寸恢复、隐藏时更新文本再显示，以及真实键盘的提示位置和截图。长文本在保留单行/最小缩放配置后与标准 `UILabel` 离屏绘制一致，并用空文本作负对照避免两张空图误过；纯系统标签的离屏采图无法区分缩字开关，因此不把这条比较称作真实缩字效果测量。26.5 五项全部通过，日志 `target/ios26-hint-frame-focused-reference.log`。同一旧底稿下的真实原生键盘合成短/长/隐藏提示前后图均为 1170×843，解码像素缓冲逐字节一致；截图保留，不随临时诊断清理删除。
- `MSIMEKeyboardTests/KeyboardMemoryTests`：`testADismissedKeyboardControllerReleasesItsViews`（控制器和它挂在输入视图下的子视图都释放）、`testReloadingTheKeyboardTenTimesDoesNotAccumulateMemory`（十次重新加载后占用涨不到 8 MiB）。去掉 `deinit` 里那一行，两条都失败（留下 `KeyboardSkinBackgroundView`、`KeyAreaStackView`；38 → 108 MiB），加回去通过（29 → 31 MiB）。
- 2026-11-04 新增 `testUnpresentedKeyboardsReleaseViewsAfterCompositionAndLayoutChanges`：不进入窗口，交替全拼与九键共 24 次，在真实按键组字和偏好重载后释放控制器，并检查整棵键盘视图的弱引用。生产代码不改时，iOS 26.5 在 8.6 秒内报原 `deinit` 行的 `_engineVar_rawRemove` 异常；iOS 27.0 也触发同一断言。
- 批量停约束版本的第一轮 iOS 27 内存专项中，十次重载的读数为 `[33, 34, 34, 34, 35, 38, 41, 41, 42, 43]` MiB，第 2 到第 10 次增长约 9 MiB，超过原 8 MiB 门槛；保留失败样本，不修改门槛或前置条件。同一生产代码下单独复跑该测试为 `[27, 27, 27, 27, 29, 29, 30, 30, 30, 30]` MiB，通过。把新回归加强为检查所有后代视图的弱引用后，iOS 26.5 完整相关类共 155 项、1 项缺笔画词库跳过、0 失败，重载读数 `[57, 57, 58, 58, 58, 58, 58, 59, 59, 59]` MiB；iOS 27 同轮内存组也通过，读数 `[51, 51, 51, 51, 52, 52, 52, 52, 52, 52]` MiB。这些读数不是性能承诺，单次超限不能用后一次通过抹掉。
- 同一批量停约束版本的首轮 iOS 27 完整相关类中，`testCommitReplacesSelectionOrCompositionAndImmediatelyContinuesBeforeUnicodeSuffix` 在两轮短语提交后编辑器仍是原文，共有 10 条断言失败；当时控制器还活着，没有布局引擎异常。同一模拟器上，该用例单独复跑通过；恢复原生产销毁逻辑并重新编译后，完整短语类 5 项也通过，尚未确认这次丢提交与销毁改动的因果关系，保留失败日志，不改输入、等待或前置条件。
- 最终代码进一步让已展示键盘的既有释放测试也检查所有后代视图。在 iOS 27.0 上重新运行 `NineKeyKeyboardTests`、`QuickPhraseCursorKeyboardTests`、`GlideTypingTests`、`KeyboardMemoryTests`，共 155 项、1 项缺笔画词库跳过、0 失败；短语类完整 5 项通过，十次重载读数 `[43, 43, 44, 44, 44, 44, 45, 45, 45, 45]` MiB。两版系统均沿用仓库 `run_ui_tests.sh` 对 `testKeyLayoutsKeepNineKeyHeight` 的既有排除，不新增跳过。最终日志为 `target/ios27-final-related.log`，之前的失败和对照保留在 `target/ios27-batched-related.log`、`target/ios27-batched-quickphrase-isolated.log`、`target/ios27-original-teardown-quickphrase-control.log`。
- `crates/host-api/src/tests.rs` 的 `keyboard_reranker_prefers_the_onnx_model_when_one_is_installed`：同一个模型连挂十一次都是同一份、细节标 `(shared)`，放开后重新加载，别的根目录挂不上也不放开已加载的；自测只做一次（放开后换预算再挂，沿用那次的毫秒数，换成小的、是另一份；再按原预算挂回大的，仍不重测、照样排上屏后联想）；可用内存少于门槛时用小的且不自测，定下后内存再多也沿用，刚好够时用大的。
- `crates/onnx-rerank/tests/golden.rs` 的 `reading_the_file_without_a_memory_arena_scores_the_same`：运行时读文件、关内存池时分数与参考运行时一致。
- 量内存：`ORT_LIB_LOCATION=$(python3 scripts/fetch_onnxruntime.py --target aarch64-apple-ios-sim) CARGO_TARGET_AARCH64_APPLE_IOS_SIM_LINKER=$(xcrun --sdk iphonesimulator --find clang) cargo build --release -p msime-onnx-rerank --features runtime --example footprint --target aarch64-apple-ios-sim`，再 `xcrun simctl spawn <设备> target/aarch64-apple-ios-sim/release/examples/footprint <模型目录> [arena|no-arena] [file|bytes]`。

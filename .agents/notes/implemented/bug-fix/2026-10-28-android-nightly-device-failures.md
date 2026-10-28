# Agent Note: Android 夜间设备验收连续失败的归因与修复

Status: implemented

## Problem

Nightly builds 从 2026-10-25 起每天失败（上一次成功是 10-20 的 `39499422750`），每晚都有 Android device (API 35) 失败，而且停在不同的冒烟上：10-25 的「launch」「clipboard export backup synthetic device state」「trackpad last word and unterminated final line」，10-25、10-26 两晚的「trackpad triple tap line」，10-27 的「open native saved skins」，统一报 `Expected synthetic UI state was not observed`。10-27 的 API 28 还在安装步骤因 sdkmanager 报 `Error on ZipFile unknown archive` 失败。看上去像共享的等待机制在慢模拟器上太紧，或前一个冒烟把键盘留在了后一个没预料的状态。

夜间和 develop 推送跑的是同一个 `android-device.yml`（`release-android.yml` 与 `ci-platforms.yml` 都调用它，同一种 ubuntu-24.04 runner、同一组 `start-emulator.sh` 参数），PR 上不跑（见 [PR 上只编译和测试](../process/2026-10-09-no-packaging-on-pull-requests.md)）。把 10-25 到 10-27 的 develop 推送运行一起排开后，失败分成三类，没有哪一类是等待时限太短：

1. **合并后才暴露的确定性失配**。改了界面文字的 PR 合进 develop 后，此后每次运行都停在同一处，两个 API 级别一起失败；`smoke.sh` 在第一处失败就退出，后面的失配被挡住，所以每晚看到的是当时排得最靠前的那一个。10-26 的「open native saved skins」由 #9926 修好；10-27 05:35 至 19:05 每次都停在「eighteen quanpin English and number roundtrip」，由 `b731116489` 修好；之后（含夜间 `40590337289`）又全部停在「open native saved skins」。失败信息列出的卡片是「皮肤 合成晨雾，浅深均可选 · 外观固定，点击使用」，`SkinDeleteDeviceCheck` 找的是「皮肤 合成晨雾，点击使用」；读屏文字由 #9924（`caac03ed45`）改出。
2. **只在 API 35 间歇出现的触控板三击失败**。夜间 `40230902154`、`40411840275` 停在「trackpad triple tap line」，夜间 `40192905422` 和 develop 的 `c34513834d` 停在「trackpad last word and unterminated final line」；同期 develop 的 `af0ace384e`、`f03731f317`、`060b72974d` 等又整套通过，API 28 从未失败。出错的两个阶段都含三击，只做双击的阶段从未失败。
3. **下载损坏**：`40590337289` 的 API 28 在安装步骤失败，代码还没构建。

其余几项已有归属，本篇不再处理：10-25 的「launch」（剪贴板常用语提示）由 #9785 修好；`40164019651` 的剪贴板导出和 API 28「deferred composition length 1」见[设备验收回归修复](2026-10-22-android-device-ci-regressions.md)；10-26 develop 上的「clipboard export real keyboard panel clipboard entry」由 #10007 修好。这几项在最近的运行里都已通过。

## Decision

- 皮肤删除验收按 #9924 之后的整句读屏文字找卡片，不放宽成前缀。删除后偏好字节不变的断言保留：删除时 `SkinsPage` 经 `KeyboardSheets.write` 清理引用该设计的明暗槽位，夹具用的是旧式 `global_theme = custom` 加 `custom_theme.keyboard`，没有槽位，这次写入的内容与盘上相同，而 `PreferencesStore::save` 对未变的文档不落盘也不升修订号。
- 触控板双击选词、三击选行的回读代次由 `TextTrackpadSelectionVersion` 持有。新的一下触摸、换编辑器、关面板和编辑器报告的选区变化照旧让在途回读作废；触控板自己设下、编辑器已接受的选区，其回报只认一次、不作废回读。这份记录只陪紧随其后的一下触摸，再下一下触摸时作废；换编辑器和关面板时同样清掉。选区本来就是那一段时 `setSelection` 不改变什么，编辑器不发回报（`InputMethodManager` 只在选区与上次发出的不同时才报），记录若一直留着，之后外部恰好设成同一段的回报会被吞掉，在途回读就盖过它。三击里要保护的只有第三下松手发出的选行回读，下一下触摸本来就让它作废。
- 设备验收的连点组之间，事件时间至少隔开系统双击时限；组内仍用排在当前时刻之后的事件时间表达快速连点。选区断言超时时报出合成编辑器的实际选区偏移。
- 设备验收的 NDK 改用发版和 OCR 模块构建共用的 `platforms/android/install-ndk.sh`，其余 SDK 包照旧一次装完，安装后的 NDK 版本与系统镜像核对保留。`install-ndk.sh` 每次重试前清掉 sdkmanager 的 `.temp`、`.downloadIntermediates` 和没装完的 NDK 目录。
- `DeviceSmoke.await` 的 15 秒时限和失败即退出的套件顺序不变。

## 三击为什么在 API 35 上丢掉选行

`ImeTextEditPanel` 在后台队列里读 `getExtractedText`，结果回到主线程时用代次判断是否过期；改动前 `onUpdateSelection` 每到一次都让代次加一。快速三击时，第二下松手发出选词回读，结果在第三下按下之前就写进了编辑器；编辑器处理这次 `setSelection` 时立即发出选区回报，然后才答复第三下的选行回读。回报和选行结果都排在输入法主线程上，而主线程这时正忙于每一下点按引起的重新布局，第三下的按下、松手从输入通道先被处理，回报随后才到：代次被加一，已经读到这段选区的选行结果被当成过期丢掉，编辑器停在选词。

失败运行的 logcat 与这个次序一致。`40411840275` 的测试应用在这一段有四行 `TextClassifier called on main thread`，与测试经无障碍改选区的四次调用一一对应，最后一次在 22:59:50.595 把选区设回 8；输入法随后在 50.964、51.010、51.062 各有一轮重新布局，和此前双击时的两轮（50.383、50.451）一样与点按一一对应，即三下实际相隔约 50 ms，正好留出选词回读往返一次的时间；此后测试应用在 15 秒等待里只画了 4 帧，选区再没变化。`40230902154` 的三轮在 21:01:01.203、.254、.331。双击只发一次回读，没有中间结果可以晚到，所以从不失败；真人连点间隔通常在 100 ms 以上，回报大多先到，慢设备或忙碌的编辑器上同样会出现。

`df2ae111b0` 当时把失败归为慢模拟器丢掉了过期的第三下，于是去掉每组前的「按下加取消」，改为把事件时间排到当前时刻之后。这没有触及上面的竞态，还让相邻两组可能被算成同一串连点：上一组最后一下与下一组第一下的事件时间若相隔不到双击时限，三击的第一下就成了第三下。现有运行里两组相隔约 500 ms，没有因此失败，但更快的设备会撞上，所以测试一侧补回组间间隔。

## Alternatives considered

- **统一放宽 `await` 时限或给失败的冒烟加重试**：改动最小，慢 runner 上确有等待偏紧的情形。不采用：皮肤卡在 15 秒里根本不会出现，三击失败后编辑器选区再也不变，等多久都一样；重试只会把确定性失配和真实竞态一起盖住。
- **测试逐下同步注入并等待选区稳定后再点下一下**：能让设备验收稳定通过，也不用改产品。不采用：这改变了被测的快速连点，产品在慢设备上的同一竞态仍在，且[触控板笔记](../feature/2026-10-23-android-text-edit-trackpad.md)已记录同步注入会把拖动拉成长按。
- **回读结果发布时比较它读到的选区与最近一次回报**：还能容忍更早的外部回报晚到。不采用：`ExtractedText` 的偏移与 `onUpdateSelection` 的约定不保证在所有编辑器（如 WebView）里一致，不一致时会永远丢结果，比现状更差；只认自身回报时，比较的是产品自己设下的值，对不上就退回原来的作废行为。
- **选区已经是目标范围时不记录自身选区**：直接针对不发回报的情形。不采用：面板手里的选区是最近一次回报，三击竞态里它恰好落后于编辑器（选词的回报还没到），拿它判断会漏记或错记，正是这次要修的时序。
- **收到任何一次回报就清掉记录，不论是否对上**：最简单。不采用：比自身回报更早的外部回报可能先到，按这条会把随后的真回报当成外部变化，三击竞态换个次序又回来；冒烟里「先到的旧回报照旧作废、随后的自身回报仍被识别」钉住这一点。
- **去掉 `onUpdateSelection` 对代次的影响**：一行改动即可。不采用：触控板的设计要求核对最新选区，外部改动后套用旧回读算出的范围会把选区挪到别处。
- **在 `android-device.yml` 里内联「失败后清理再装一次」**：贴近出错位置，只改一个文件。不采用：仓库已有带合成 SDK 测试的 `install-ndk.sh` 供发版使用，再写一份会让两处的重试规则分叉；清理半截下载放进共享脚本，三个 workflow 一起受益。

## Consequences

- 收益：夜间与 develop 推送的设备验收不再卡在皮肤卡片文字上；三击选行在回报晚到时也成立，触控板笔记里「用输入连接身份和代次丢弃迟到结果」的规则收窄为不把自身回报算作外部变化；NDK 下载损坏不再直接让 job 失败。
- 代价：多了一个只在主线程使用的小类；自身选区的回报若被编辑器改写成别的值，仍按外部变化作废，与改动前相同。选词不发回报（选区本来就是那个词）且第三下的选行回读在途时，外部恰好设成同一个词的回报仍会被当成自身回报，这与真回报无法区分，窗口只有一下触摸。连点组之间的间隔只体现在事件时间上，测试不额外等待。
- 仍然暴露的风险：设备套件只在合并后运行，改界面文字的 PR 仍会在 develop 上留下下一处失配，并被夜间第一时间报出；这次又是 #9924 漏改了 `SkinDeleteDeviceCheck`。是否把设备套件或其中的设置类专项放回 PR，属于流程取舍，本篇不改。测试经无障碍设选区后，若输入法主线程恰好阻塞超过点按前约 250 ms 的边界稳定等待，这次外部回报仍会作废随后的回读；目前没有观察到。
- 运行中顺带观察到但未处理：`TextTrackpadView` 的外层在布局回调里调 `setLayoutParams`，每次点按都触发「requestLayout() improperly called … during layout」的二次布局；一次性的「prepare nine-key sidebar」「nine_key Chinese return target 0」（后者失败时键盘停在日文九键）以及 API 28 的「return to later candidate and touch commit」「native provider selection fills empty defaults」各只出现过一次，没有在这里归因。其余 SDK 包（特别是 1 GB 级的系统镜像）仍没有下载重试。

## Verification

- 新增 `TextTrackpadSelectionVersionSmoke` 按主线程上的实际先后重放这条竞态：沿用改动前「每次回报都作废」的语义时在「触控板自己设下的选区回报不作废选行回读」处失败，改动后通过；同时覆盖重复回报、外部变化、旧回报先到和重置。「没等到回报的自身选区过了一下触摸就不再算数」一项在记录永不过期的版本上失败，改动后通过。`ANDROID_SDK_ROOT=$HOME/Library/Android/sdk bash platforms/android/check-host.sh` 退出码 0：291 项 JVM 冒烟、252 个设备套件源码编译通过。最终的设备套件改动另以 `-Xlint:all -Werror` 编译通过。
- `scripts/test-android-install-ndk.py` 新增的「留下坏压缩包、准备完成的暂存操作和残缺 NDK 目录后重试」用例在旧脚本上失败（得到退出码 1、三次尝试），新脚本上两次尝试成功；其余四项照旧通过。`actionlint`（含 ShellCheck）检查 `android-device.yml`，ShellCheck 检查 `install-ndk.sh`，均通过。
- 归因依据是 `gh run view` 的失败日志、各次运行上传的 logcat 和 develop 推送的同期运行；没有在本机模拟器复现，本机 arm64 模拟器的时序与 CI 的 x86_64 swiftshader 不同。

## 验证缺口

修复只能由 CI 上的整套设备验收确认：需要一次 develop 推送或手动触发的 Nightly builds，看 API 28、35 两台设备都通过，特别是 API 35 的触控板专项和 `skin-delete` 专项。皮肤删除在新读屏文字下后续的取消、删除和偏好字节检查，本次只在源码层面核对，没有在设备上跑到。

相关：[设备验收回归修复](2026-10-22-android-device-ci-regressions.md) 记录了更早一批同类失配；[设备门禁](../testing/2026-10-08-android-device-gates.md) 持有套件与模拟器的整体安排。

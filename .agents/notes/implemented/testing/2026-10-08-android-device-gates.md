# Agent Note: 在 Android 模拟器上运行的 PR 门禁和发版门禁

Status: implemented

## Problem

android-v0.3.0 在 CI 全绿的情况下带着三个设备上才出现的问题发了出去：

- 词库准备失败（`resource storage or transport failed: Permission denied (os error 13)`）。`open_private_directory` 从 `/` 起逐级以 `O_RDONLY` 打开，Android 只给应用 `/data`、`/data/user` 搜索权限，第一步就 EACCES；约 50 处私有文件读写一起失败。
- 匿名账号写不进。`write_private_file_at_noclobber` 用硬链接发布，Android 的 SELinux 禁止应用建硬链接。
- Android 9 一打开就崩溃。`splash_glow` 的径向渐变带主题属性，Android 9 的 `GradientDrawable.applyTheme` 读不到 `gradientRadius` 就抛异常。从 android-v0.2.0 起就在。

CI 的 Android job 只做 `check-host.sh`、各版本 Gradle 编译和 NewApi lint，从不在 Android 上运行任何东西。client-core 单测只在 macOS 上跑，Linux CI 不跑它；仓库里本来有在模拟器上跑整套单测的 `run-core-test.sh`，但 `preferences/tests.rs` 里一条桌面专用的 `const assert` 让测试 crate 在 Android 目标上编译失败，它从来没跑起来过。`smoke.sh` 会检查「词库准备失败」，但没有任何流程调用它，而且它和设备测试里的 `ime enable` 用了系统不认的长写 id，在 API 31、35 上都过不了。

## Decision

- `.github/workflows/android-device.yml` 是可复用 workflow，在 GitHub 托管的 `ubuntu-24.04` 上（[runner 选择](../process/2026-10-30-android-device-kvm-runner.md)记录普通 Depot runner 缺少 KVM 的边界）按 API 28（minSdk）和 35（targetSdk）各起一台 x86_64 模拟器，依次：
  - `build-core-test.sh` 交叉编译 client-base、client-core、client-providers 和 client-services 的单测，`run-core-test.sh` 是这个目标的 cargo runner，cargo 把每个测试程序分别交给它在设备上整套运行（拆分见 [按职责把 client-core 拆成四个 crate](../architecture/2026-10-15-client-core-split.md)）；
  - 用开发密钥打 x86_64 APK，`smoke.sh` 装包，走首次启动、词库准备和输入验收。API 28 用 `--core`，只到 DeviceSmoke 的打字主路径。
  - 两个 API 级别的设备套件都运行 `NumberPadDeviceSmoke` 的笔画和注音分支，打包前统一用 `fetch_language_dictionaries.py` 取回锁定词库，并设置 `MSIME_REQUIRE_LANGUAGE_DICTIONARIES=1`；缺失就拒绝打包。nightly 首次运行中，未准备词库的 APK 正常回退到拼音，导致依赖这些方案的键面断言等不到目标键面；这是测试输入缺失，不应靠跳过套件解决。
- `ci-platforms.yml` 在 `android == 'true'`（动到 Android 或共享代码）的 PR 上调用它。这一条已被 [PR 上只编译和测试，打包挪到合进 develop 之后](../process/2026-10-09-no-packaging-on-pull-requests.md) 取代：现在只在推到 develop、main 时调用，PR 上不跑。
- `release-android.yml` 的 `publish` 依赖新的 `device` job，用同一个提交跑同一套，失败就不发。这个 job 不接触发版密钥。
- 设备脚本：`start-emulator.sh` 接受 API 级别，按主机架构选 arm64-v8a 或 x86_64 镜像，每个级别一台独立 AVD、固定端口（`5580 - 2 × (35 - API)`）；`run-core-test.sh` 用 `MSIME_ANDROID_TEST_SERIAL` 选设备；新增 `build-core-test.sh`。
- 设备测试统一用 `ime list` 给出的短写 id `app.msime.android/.MSIMEInputService`。
- `DeviceSmoke.restartIme` 不把三条 `ime` 命令返回加固定睡眠当成切换完成：禁用当前输入法后，系统的异步回退可能覆盖随后的选择。它读取默认输入法标识，连续四次确认水杉被选中；回退覆盖时重新选择，十五秒内不能稳定就报告测试准备失败，不再把系统键盘误报为产品空窗口。第二次 nightly 的高度重启用例等待水杉节点超时，设备没有崩溃，日志中的编辑器实际绑定了 `LatinIME`，因此需要确认选择状态，而非放宽高度断言或跳过套件。
- `PreferencesDeviceSmoke` 也复用 `restartIme`，不再自行执行三条命令并固定等待一秒。本批 API 35 的偏好套件在第一枚字母键前失败，失败后默认输入法为 `LatinIME` 且没有产品崩溃；这是同一异步回退竞态遗漏的调用方。偏好文件准备、编辑器焦点与所有业务断言保持原样。
- 其余直接重绑输入法的八个套件（AI 占位、剪贴板编辑、空编辑器退格、宿主入口、设置、设置生命周期、打字统计和手写）统一调用 `restartIme`；需要版本专用组件 id 的套件通过同一个重载传入 id。`scripts/test-android-device-ime-rebind.py` 把这条约束固定下来，避免新的套件退回三条命令加固定一秒等待。
- `CopyDeviceInfoDeviceSmoke` 在系统输入套件之后也先回到 `HomeActivity` 并复用 `restartIme`。一次 API 28 单项观察在未重新选择输入法时实际落到 `LatinIME`，在第一枚字母键前失败；复制白名单、剪贴板和编辑器断言保持原样。修复后同一 API 28 二进制的单项复制重复通过。
- 本机并行会话不能共用同一台 AVD 做安装验收。复用 `emulator-5580` 时，其他会话替换了测试 APK，设备包的 SHA-256 与本次构建不一致，随后观察到的候选排序、资源校验和 instrumentation 退出都不能作为本次提交的证据。独立验收使用本任务创建的 AVD 名称和端口，并确认设备包哈希；AVD 数据仍放在规定的 `target/android/avd-home`，完成后关闭并删除本任务创建的 AVD，不动原来运行的设备。
- 设备测试读控件状态走 `DeviceSmoke.state()`/`switchState()`：API 30 起读状态描述，更早的系统按应用实际暴露的 `isSelected`/`isEnabled` 折算。
- `DeviceSmoke.await` 超时时附上输入法窗口里看得见的文字（最多 30 条，只读输入法自己的节点），失败信息直接说明界面停在哪里。
- 无障碍点击（`DeviceSmoke.tap` 和 `HandwritingDeviceSmoke.accessibleClick`）在 `performAction(ACTION_CLICK)` 返回 false 时重新查节点再点，最多三次。查到节点和点下去之间面板可能已经重新绑定（开关刷新状态、翻页动画收尾），过期节点上的点击返回 false、并没有发生，所以重试不会把开关点两次；真不接受点击的控件三次后照样失败。2026-10-08 起 API 35 上这一处间歇失败，一天里落在四个互不相关的 PR 上（三次是「更多」面板的按键音开关），重跑即过。同理，`smoke.sh` 启动轮询里跳过引导页时直接用这一拍的 dump 去点，不另取一份：另取的那份可能赶上引导页开关中途，返回空树（`null root node`），找不到按钮就会让冒烟在第一条用例前退出（#6074 的 API 28 上出现过）；点不到就当作还没就绪，下一拍再看。
- `smoke.sh` 的套件改成一个循环；`MSIME_DEVICE_SMOKE_SKIP` 列出的套件跳过并打印出来，不算通过。CI 的跳过名单写在 `android-device.yml`，每一项带原因，修好一个删一个，不往里加新的来让门禁变绿。
- `smoke.sh` 的一项验收失败不结束整套：`check`（instrumentation，按通过标记判定）和 `check_script`（自带清理的辅助脚本，按退出码判定）记下失败和它的第一行信息（套件的 `*_FAILED` 行，其次是 `INSTRUMENTATION_*` 或 adb 的错误行），收拾失败可能留在进程外的状态后接着跑，最后打印一个汇总块、写进 `$GITHUB_STEP_SUMMARY`，有失败就以 1 退出。全部通过时的输出、顺序和退出码与原来相同，判据、时限和 `android-device.yml` 失败时收 logcat 的步骤都不变。
  - 失败后的收拾只管进程外的东西：强停合成编辑器所在的测试包、系统文件选择器和设置页，清掉合成锁屏 PIN，重新选中水杉并连续确认（与 `DeviceSmoke.restartIme` 同一判据）。偏好、键盘层和夹具由失败的套件在自己进程的 `finally` 里还原，这一点本来就要成立，因为下一项不论前一项成败都从同一台设备开始；不强停产品，理由见 `run_instrumentation` 上的说明。
  - 照旧当场停下、并在汇总里写明原因的情况：测试 APK 构建或装包失败、词库准备失败或超时、选不中输入法（开头，以及 `SessionStoreDeviceSmoke` 停用输入法之后）；系统输入主路径 `DeviceSmoke` 失败，后面每一项都经同一个键盘打字；失败后模拟器不再报告开机完成；输出里有 `shortMsg=Process crashed` 或 `INSTRUMENTATION_ABORTED`，崩溃的进程没有执行 `finally`，设备状态未知；失败后选不回输入法；连续五项失败，多半是键盘整体不可用，后面只会是同源的失败，还可能把 job 拖过 90 分钟时限，`if: failure()` 的收日志步骤也就不会运行。
  - 检查之间唯一的显式依赖都在辅助脚本内部（剪贴板导出、诊断目录的 `select` → `verify` 两段），脚本自己在第一段失败时退出，整个脚本记为一项失败。
- instrumentation 的通过条件同时要求命令退出码为零且输出包含对应通过标记。`check` 以及先清锁屏凭据、先恢复输入法再裁决的两条路径都保留原始退出码；非零退出时把退出码写进失败摘要，不能让已经打印的通过标记掩盖 adb 的失败。主路径仍停止，普通检查仍收集后继续。
- `smoke.sh` 不授予通知权限，按没授过权的普通用户来测：输入法的提示显示在键盘的诊断行（`MSIMEInputService.notice`），不依赖 Toast。
- `FourteenKeyDeviceSmoke` 改窄显示尺寸后重新打开编辑器，清掉旧无障碍节点并等待输入框获得焦点，再检查窄屏拼音栏。显示配置变化会重建编辑器、收起输入法，原先只等键面恢复会因没有编辑器焦点或沿用旧窗口坐标而超时，不能把输入法窗口为空当成布局错误。窄屏用例分别报告焦点、键面和拼音栏三个阶段，便于区分测试准备与布局断言。

## 第一次真正跑起来时查到的

设备套件此前从未在任何流程里运行，第一次跑时 12 个套件里 9 个失败。逐个查过：

- 测试过时或写错，已修：两个套件没在清单里声明（`smoke.sh` 一直在调用它们）；Preferences 等一个已改成 Toast 的提示；Emoji 要求首页至少 64 个表情，而首页会去掉设备字体画不出的表情（模拟器上 55 个）；改版前后的无障碍状态读取。
- 真 bug，已修：123 和 #+= 层的标点不跟「中文标点」开关（0.2.0 的改版只按中英文模式选），关掉中文标点后仍上屏「，」「。」。
- 真问题，随后修掉：安卓 13 起未授通知权限时输入法的提示（Toast）全被系统吞掉，改为显示在键盘里；「更多」面板翻页动画中途被移出窗口后卡在两页之间（`PagedTileGrid`）；微软双拼组字时分号键被当成标点，ing 韵母打不出（b73a611a8 起）；开 z=zh 打 `zongguo` 时整句「总国」排在「中国」前（#6033）。
- 其余是测试跟不上改版：组字从不进输入框改为写进输入框、九键组字不写进输入框、读音行可点后 `key()` 会点到读音行、英文释义改为另起一行且默认关、弹出菜单可点的是整行、九键拼音栏会列出更长的补全音节。跳过名单最后只剩 FuzzyPinyin，等 #6033 合并。

## API 28 上的已知缺口

在 API 28 模拟器上，「更多」面板在屏幕上正常打开、点按生效，但 UiAutomation 的无障碍树里拿不到面板容器和任何磁贴；加 `FLAG_INCLUDE_NOT_IMPORTANT_VIEWS` 也一样。原因没有查到底。DeviceSmoke 在 API 30 以下跳过繁体输出这一段，其余套件在 API 28 上不跑。这可能也意味着 Android 9 上读屏用户用不了这个面板，需要单独确认。

## Alternatives considered

- **延长输入法重启后的固定睡眠**：改动最少，较慢的 CI runner 可能因此等到回退结束。但时间本身不能证明最终选中了水杉；发生回退覆盖时等多久都还是系统键盘，因此检查实际选择状态并有界重新选择。
- **在共享 AVD 内只为模糊拼音创建独立状态目录**：可以避免既有学习数据干扰首选断言，启动成本也小。但不能隔离其他会话安装的 APK 和并行 instrumentation，准备时还碰到设备资源校验不一致；这次采用独立 AVD 验证整套，撤回临时夹具改动，保留原有模糊拼音首选断言。
- **只在发版前跑，不在 PR 上跑**：最强的理由是省 CI 时间，每个 PR 少两个各几十分钟的 job。不用它是因为 0.3.0 的两个回归都是发版当天才合入的，发版门禁拦下时离用户要更新只差一步，修的人还得回头找是哪个 PR；在 PR 上失败，责任和上下文都还在。
- **用 `reactivecircus/android-emulator-runner` 这类 action 起模拟器**：省掉 KVM 规则、等开机这些样板。不用是因为本机和 CI 应该走同一套 `start-emulator.sh`，否则 CI 绿而本机复现不了（或反过来）；而且要多引入一个第三方 action。
- **只跑 client-core 单测，不装 APK**：快得多，也能抓住前两个问题。但 Android 9 的启动崩溃是资源加载问题，只有装包启动才看得见。
- **在 Depot runner 上跑，复用 sccache**：构建更快、节省 GitHub 托管并发。单台探测可用不足以保证后续 runner 都提供 KVM，普通 Depot GitHub Actions runner 官方也不支持该设备，因此设备 job 使用 GitHub runner，见[按实际 KVM 支持选择 runner](../process/2026-10-30-android-device-kvm-runner.md)。

- **失败的验收自动重试一次**：最强的理由是 runner 偶发慢时能少一次红灯，改动也只在 `smoke.sh` 里。不采用：2026-10-25 到 10-27 的夜间失败里，触控板三击丢掉选行是产品里的真实竞态，只在 API 35 上间歇出现，重试一过就看不见了；皮肤卡片文字那类确定性失配重试也不会过，只是多等一轮。归因见 #10089 的 `bug-fix/2026-10-28-android-nightly-device-failures.md`。
- **保持第一处失败就退出，靠重跑看后面的**：脚本最简单，失败的位置一定是第一个出问题的地方。不采用：每次重跑要从头构建、装包、准备词库再跑到那里，二十多分钟只多看到一项；那三晚每晚停在不同的地方，同时存在的几个独立问题要排着队一个一个修才看得见。
- **全部跑完、不设任何停止条件**：汇总最完整。不采用：装包失败、输入法选不中、模拟器掉线或进程崩溃之后，后面每一项都会以同一个原因失败，汇总被几十条噪声淹没，还会拖过 job 时限，连 logcat 都收不回来。
- **把每个套件拆成 workflow 里的单独步骤，用 `continue-on-error`**：GitHub 页面上逐项显示成败。不采用：本机和 CI 应该走同一个 `smoke.sh`，拆进 workflow 后本机跑不出同样的汇总；步骤间的输入法重选、锁屏清理也得在 YAML 里再写一份。
- **间歇失败的点击放进跳过名单**：立刻让门禁变绿。但被跳过的是「更多」面板和候选长按菜单这类整段套件，跳过等于不测；而且跳过名单的规矩是修好一个删一个、不为变绿往里加。失败的根源在测试拿着过期节点点击，在测试这一侧修。

## Consequences

- **收益**：Android 上的运行时问题（权限、SELinux、旧系统缺陷、首次启动崩溃）在 PR 上就会失败；发版前同一个提交再过一遍。本机和 CI 用同一组脚本。
- **代价**：动到 Android 或共享代码的 PR 多两个 job，每个都要从头编 x86_64 原生库和测试程序（没有缓存），预计各 40 到 60 分钟；Release Android 也要多等这么久。
- **失败收集**：一次运行列出全部独立失败，修复可以并行，不必一晚一个；代价是有失败时运行更久：失败之后的检查照样跑完，每项失败本身多半要等满 `DeviceSmoke.await` 的 15 秒，再加一两秒收拾，汇总里也可能出现由前一项留下的状态引起的连带失败，读汇总时要从第一项看起。连续失败上限五项是经验值，独立失败恰好连续五项时后面的就看不到了，汇总会写明在哪里停下。
- **仍然没有覆盖**：API 28 上「更多」面板及之后的套件；真机；Android 10（API 29）到 14（API 34）之间的版本只在本机手动跑过 31。

## Verification

- `PreferencesDeviceSmoke` 的旧准备流程在 API 35 失败：第一枚字母键前输入法树为空，截图为系统键盘，默认输入法标识为 `LatinIME`。改用现有状态等待后，设备单项通过，覆盖延迟应用、候选页大小、标点及坏文件恢复；独立审查确认所有业务断言保持原样。
- 本批的 Android 主机门禁通过（231 项 JVM 冒烟、121 个设备源码编译和重绑定契约检查）。独立 API 28 AVD 上核心 `smoke.sh --core` 通过，受影响的空编辑器退格和宿主入口套件也通过；API 35 AVD 上核心 `smoke.sh --core` 与 `PreferencesDeviceSmoke` 通过，后者实际走共享稳定重绑定。API 28 的剪贴板编辑默认参数指向未安装的 pinyin 版本，单独运行该参数属于测试夹具错误，不作为产品结果；设置生命周期和打字统计在清理后的 API 35 AVD 上分别停在既有 WebView／统计准备阶段，未观察到输入法选择失败。
- `CopyDeviceInfoDeviceSmoke` 的旧准备流程在一次 API 28 单项观察中未确认水杉输入法，截图显示系统 `LatinIME`，在第一枚字母键前失败；这是准备状态证据，不是剪贴板白名单断言失败。加入 `HomeActivity` 清焦点和 `restartIme` 后，在同一 API 28 AVD 上 3 次单项复制均通过。
- 重启选择检查在本任务独立的 API 35 arm64 AVD `msime-client-test-api35-nightly-7767`（`emulator-5582`）上通过不带跳过名单的完整 `smoke.sh`，包含高度、模糊拼音首选、十四键、剪贴板图片、实体键翻译、AI 占位和五笔查码。设备安装的测试 APK 与本地构建的 SHA-256 一致。quick 门禁、`pnpm run verify-notes` 和 `git diff --check` 通过。共享 AVD 上此前的单项与失败记录不作为本次完整验收依据。
- 本机 Apple 芯片上，API 28、31、35 三台 arm64 AVD：`build-core-test.sh` + `run-core-test.sh` 各为 1077 通过、0 失败、10 忽略。
- API 28 上 `smoke.sh emulator-5566 --core` 通过（`phrase commit and deletion (API 28)`）；API 35 上带跳过名单的完整 `smoke.sh` 通过（DeviceSmoke、CandidatePanel、EmojiPicker、Preferences、KeyboardHeight），均为全新安装。Preferences 和 Emoji 另在 API 31 上通过。
- 修复前的 0.3.0 代码在同样的设备上：`verify: Io(Os { code: 13 })`、`avc: denied { link }`、Android 9 启动 `XmlPullParserException`。
- 失败收集由 `scripts/test-android-device-smoke-report-all.py` 固定：在临时目录里用假 adb、假辅助脚本和假 `sleep` 跑真实的 `smoke.sh`，覆盖全部通过、三项 instrumentation 加一个辅助脚本独立失败（全部进汇总和 step summary，且跑到最后一项）、主路径失败、进程崩溃、模拟器掉线、选不回输入法、连续五项失败和不连续的五项失败。同一套假设备上，改动前后的 `smoke.sh` 在完整、`--core`、带全部开关、`--oobe` 和 `--text-edit-layout` 五种组合下全部通过时的 stdout、stderr、检查顺序和退出码都相同，多出的只有确认输入法的 `settings get`；对改动前的 `smoke.sh` 跑这个测试，在「失败之后互不依赖的检查应继续跑完」处失败。真实设备上的行为要靠下一次 develop 推送或夜间运行确认。
- 退出码回归用同一套假 adb 固定：打印通过标记后以 `7` 退出，#10094 合并前的脚本以 `7` 退出，合并后的脚本却以 `0` 退出并打印整套通过；修复后主路径记录退出码并停止，包装的普通检查、直接 adb、剪贴板拖选清理和会话存储恢复四条路径均进入失败汇总及 step summary，后续独立检查跑到最后。测试只使用合成输出，不启动设备。
- 写这个测试时发现：完整套件里 `ClipboardCapacityDeviceSmoke` 一项借 `ClipboardCategoriesDeviceSmoke -e capacity true` 运行，而默认清单没有声明 `ClipboardCategoriesDeviceSmoke`（只有 `build-editor.sh` 在 `MSIME_ANDROID_CLIPBOARD_CATEGORIES_SMOKE=1` 时换进清单），`run_instrumentation` 报 `Unknown instrumentation target` 后返回 1。改动前这让 `set -e` 直接结束整套，只是此前的运行都停在更前面，没跑到这里；改为借始终声明、同样指向产品包的 `CandidateCanvasDeviceSmoke`，带 `-e capacity true` 时只跑容量专项（默认清单已满 100 项，不能新增入口）。
- `actionlint` 检查了三个 workflow。CI 上 x86_64 的这条路径要靠这个 PR 自己的运行来证明。

相关：[设备测试包和 ArkTS 的编译缺口](2026-10-08-compile-gates-for-device-suite-and-arkts.md) 只保证设备测试能编译，这篇让它们真正跑起来。

分页网格接手稀疏横拖时的首段位移由[原生触摸回归](../bug-fix/2026-10-19-android-paged-grid-intercept-delta.md)单独约束，该回归接入最低 API 的核心套件。

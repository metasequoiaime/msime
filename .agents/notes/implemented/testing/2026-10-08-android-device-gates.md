# Agent Note: 在 Android 模拟器上运行的 PR 门禁和发版门禁

Status: implemented

## Problem

android-v0.3.0 在 CI 全绿的情况下带着三个设备上才出现的问题发了出去：

- 词库准备失败（`resource storage or transport failed: Permission denied (os error 13)`）。`open_private_directory` 从 `/` 起逐级以 `O_RDONLY` 打开，Android 只给应用 `/data`、`/data/user` 搜索权限，第一步就 EACCES；约 50 处私有文件读写一起失败。
- 匿名账号写不进。`write_private_file_at_noclobber` 用硬链接发布，Android 的 SELinux 禁止应用建硬链接。
- Android 9 一打开就崩溃。`splash_glow` 的径向渐变带主题属性，Android 9 的 `GradientDrawable.applyTheme` 读不到 `gradientRadius` 就抛异常。从 android-v0.2.0 起就在。

CI 的 Android job 只做 `check-host.sh`、各版本 Gradle 编译和 NewApi lint，从不在 Android 上运行任何东西。client-core 单测只在 macOS 上跑，Linux CI 不跑它；仓库里本来有在模拟器上跑整套单测的 `run-core-test.sh`，但 `preferences/tests.rs` 里一条桌面专用的 `const assert` 让测试 crate 在 Android 目标上编译失败，它从来没跑起来过。`smoke.sh` 会检查「词库准备失败」，但没有任何流程调用它，而且它和设备测试里的 `ime enable` 用了系统不认的长写 id，在 API 31、35 上都过不了。

## Decision

- `.github/workflows/android-device.yml` 是可复用 workflow，在 GitHub 托管的 ubuntu-24.04（有 KVM）上按 API 28（minSdk）和 35（targetSdk）各起一台 x86_64 模拟器，依次：
  - `build-core-test.sh` 交叉编译 client-core 单测，`run-core-test.sh` 在设备上整套运行；
  - 用开发密钥打 x86_64 APK，`smoke.sh` 装包，走首次启动、词库准备和输入验收。API 28 用 `--core`，只到 DeviceSmoke 的打字主路径。
- `ci-platforms.yml` 在 `android == 'true'`（动到 Android 或共享代码）的 PR 上调用它。这一条已被 [PR 上只编译和测试，打包挪到合进 develop 之后](../process/2026-10-09-no-packaging-on-pull-requests.md) 取代：现在只在推到 develop、main 时调用，PR 上不跑。
- `release-android.yml` 的 `publish` 依赖新的 `device` job，用同一个提交跑同一套，失败就不发。这个 job 不接触发版密钥。
- 设备脚本：`start-emulator.sh` 接受 API 级别，按主机架构选 arm64-v8a 或 x86_64 镜像，每个级别一台独立 AVD、固定端口（`5580 - 2 × (35 - API)`）；`run-core-test.sh` 用 `MSIME_ANDROID_TEST_SERIAL` 选设备；新增 `build-core-test.sh`。
- 设备测试统一用 `ime list` 给出的短写 id `app.msime.android/.MSIMEInputService`。
- 设备测试读控件状态走 `DeviceSmoke.state()`/`switchState()`：API 30 起读状态描述，更早的系统按应用实际暴露的 `isSelected`/`isEnabled` 折算。
- `DeviceSmoke.await` 超时时附上输入法窗口里看得见的文字（最多 30 条，只读输入法自己的节点），失败信息直接说明界面停在哪里。
- 无障碍点击（`DeviceSmoke.tap` 和 `HandwritingDeviceSmoke.accessibleClick`）在 `performAction(ACTION_CLICK)` 返回 false 时重新查节点再点，最多三次。查到节点和点下去之间面板可能已经重新绑定（开关刷新状态、翻页动画收尾），过期节点上的点击返回 false、并没有发生，所以重试不会把开关点两次；真不接受点击的控件三次后照样失败。2026-10-08 起 API 35 上这一处间歇失败，一天里落在四个互不相关的 PR 上（三次是「更多」面板的按键音开关），重跑即过。同理，`smoke.sh` 启动轮询里跳过引导页时直接用这一拍的 dump 去点，不另取一份：另取的那份可能赶上引导页开关中途，返回空树（`null root node`），找不到按钮就会让冒烟在第一条用例前退出（#6074 的 API 28 上出现过）；点不到就当作还没就绪，下一拍再看。
- `smoke.sh` 的套件改成一个循环；`MSIME_DEVICE_SMOKE_SKIP` 列出的套件跳过并打印出来，不算通过。CI 的跳过名单写在 `android-device.yml`，每一项带原因，修好一个删一个，不往里加新的来让门禁变绿。
- `smoke.sh` 不授予通知权限，按没授过权的普通用户来测：输入法的提示显示在键盘的诊断行（`MSIMEInputService.notice`），不依赖 Toast。

## 第一次真正跑起来时查到的

设备套件此前从未在任何流程里运行，第一次跑时 12 个套件里 9 个失败。逐个查过：

- 测试过时或写错，已修：两个套件没在清单里声明（`smoke.sh` 一直在调用它们）；Preferences 等一个已改成 Toast 的提示；Emoji 要求首页至少 64 个表情，而首页会去掉设备字体画不出的表情（模拟器上 55 个）；改版前后的无障碍状态读取。
- 真 bug，已修：123 和 #+= 层的标点不跟「中文标点」开关（0.2.0 的改版只按中英文模式选），关掉中文标点后仍上屏「，」「。」。
- 真问题，随后修掉：安卓 13 起未授通知权限时输入法的提示（Toast）全被系统吞掉，改为显示在键盘里；「更多」面板翻页动画中途被移出窗口后卡在两页之间（`PagedTileGrid`）；微软双拼组字时分号键被当成标点，ing 韵母打不出（b73a611a8 起）；开 z=zh 打 `zongguo` 时整句「总国」排在「中国」前（#6033）。
- 其余是测试跟不上改版：组字从不进输入框改为写进输入框、九键组字不写进输入框、读音行可点后 `key()` 会点到读音行、英文释义改为另起一行且默认关、弹出菜单可点的是整行、九键拼音栏会列出更长的补全音节。跳过名单最后只剩 FuzzyPinyin，等 #6033 合并。

## API 28 上的已知缺口

在 API 28 模拟器上，「更多」面板在屏幕上正常打开、点按生效，但 UiAutomation 的无障碍树里拿不到面板容器和任何磁贴；加 `FLAG_INCLUDE_NOT_IMPORTANT_VIEWS` 也一样。原因没有查到底。DeviceSmoke 在 API 30 以下跳过繁体输出这一段，其余套件在 API 28 上不跑。这可能也意味着 Android 9 上读屏用户用不了这个面板，需要单独确认。

## Alternatives considered

- **只在发版前跑，不在 PR 上跑**：最强的理由是省 CI 时间，每个 PR 少两个各几十分钟的 job。不用它是因为 0.3.0 的两个回归都是发版当天才合入的，发版门禁拦下时离用户要更新只差一步，修的人还得回头找是哪个 PR；在 PR 上失败，责任和上下文都还在。
- **用 `reactivecircus/android-emulator-runner` 这类 action 起模拟器**：省掉 KVM 规则、等开机这些样板。不用是因为本机和 CI 应该走同一套 `start-emulator.sh`，否则 CI 绿而本机复现不了（或反过来）；而且要多引入一个第三方 action。
- **只跑 client-core 单测，不装 APK**：快得多，也能抓住前两个问题。但 Android 9 的启动崩溃是资源加载问题，只有装包启动才看得见。
- **在 Depot runner 上跑，复用 sccache**：构建更快。Depot 的 runner 不保证有 KVM，没有 KVM 的模拟器慢到不可用。

- **间歇失败的点击放进跳过名单**：立刻让门禁变绿。但被跳过的是「更多」面板和候选长按菜单这类整段套件，跳过等于不测；而且跳过名单的规矩是修好一个删一个、不为变绿往里加。失败的根源在测试拿着过期节点点击，在测试这一侧修。

## Consequences

- **收益**：Android 上的运行时问题（权限、SELinux、旧系统缺陷、首次启动崩溃）在 PR 上就会失败；发版前同一个提交再过一遍。本机和 CI 用同一组脚本。
- **代价**：动到 Android 或共享代码的 PR 多两个 job，每个都要从头编 x86_64 原生库和测试程序（没有缓存），预计各 40 到 60 分钟；Release Android 也要多等这么久。
- **仍然没有覆盖**：API 28 上「更多」面板及之后的套件；真机；Android 10（API 29）到 14（API 34）之间的版本只在本机手动跑过 31。

## Verification

- 本机 Apple 芯片上，API 28、31、35 三台 arm64 AVD：`build-core-test.sh` + `run-core-test.sh` 各为 1077 通过、0 失败、10 忽略。
- API 28 上 `smoke.sh emulator-5566 --core` 通过（`phrase commit and deletion (API 28)`）；API 35 上带跳过名单的完整 `smoke.sh` 通过（DeviceSmoke、CandidatePanel、EmojiPicker、Preferences、KeyboardHeight），均为全新安装。Preferences 和 Emoji 另在 API 31 上通过。
- 修复前的 0.3.0 代码在同样的设备上：`verify: Io(Os { code: 13 })`、`avc: denied { link }`、Android 9 启动 `XmlPullParserException`。
- `actionlint` 检查了三个 workflow。CI 上 x86_64 的这条路径要靠这个 PR 自己的运行来证明。

相关：[设备测试包和 ArkTS 的编译缺口](2026-10-08-compile-gates-for-device-suite-and-arkts.md) 只保证设备测试能编译，这篇让它们真正跑起来。

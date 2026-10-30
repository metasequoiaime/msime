# Agent Note: iPhone 横屏与鸿蒙大屏的分离式键盘

Status: implemented

## Problem

#10224：Android 大屏横屏已经能分离键盘，启用宽度和中间空隙可调；iOS 只在 iPad 横屏分离（`KeyboardSplitLayout`，App Group `keyboard.tablet.split`）；鸿蒙没有。iPhone 横屏和鸿蒙平板横屏都没法双手拇指打字。

issue 留了两个待定问题：鸿蒙走 Android 的规则还是另定一套；iPhone 横屏很矮，分开后每一半的键距、键高怎么定。维护者的代理已经拍板：鸿蒙照搬 Android；iPhone 默认关、可选打开，复用 iPad 的分离路径，太窄的机型不分；三端设置文字一致。

## Decision

### 鸿蒙：逐条移植 Android 的 `SplitKeyboardPolicy`

`platforms/harmony/entry/src/main/ets/keyboard/SplitKeyboardPolicy.ts` 是 Android 同名类的 ArkTS 版，常数、断点和份额计算与 Android 相同：

- 启用条件：主开关打开，屏幕最短边不小于启用宽度（480–1000vp、步进 20、默认 600，默认值就是 `KeyboardFormFactorPolicy.largeScreen` 的界线），横屏或「竖屏也分离」打开，不是 2in1。键面必须是 26 键一族或韩文的字母层，或它们的 123 / #+= 层；九键、14/18 键、笔画、注音、手写、假名九键和借来的数字九宫格不分。
- 几何：空隙至少占整行的「分离中间空隙」（10–40%、步进 5、默认 25），两侧原始键区合计不超过 720vp。断点取最接近一半的键边界，奇数多出的归左。字母第二、三排在右半边内侧重复 G、V，重复键从空隙借一个键宽。底行空格拆成两个，左边那个的份额让空隙居中，每一半至少 15%。分离时单手模式让位，存着的值不变，功能面板的单手磁贴置灰。浮动键盘（[鸿蒙浮动键盘](2026-10-30-harmony-floating-keyboard.md)，#10225）优先：浮动时不画分离式键盘（`SplitKeyboardPolicy.drawn` 的 `floating` 参数），停靠后按存着的设置回来；分离时浮动磁贴照常可用，与 Android `splitKeyboardDrawn` 在 `floatingDrawn()` 时返回 false 相同。

设置的存放与 Android 不同。Android 的主开关、启用宽度、空隙在宿主自己的本机设置文件里（`platform.android.split_keyboard*`），只有「竖屏也分离」是共享偏好 `touch_keyboard_split_portrait`。鸿蒙的本机设置就是共享偏好，于是 client-core 新增三个共享偏好：

| 共享偏好 | 取值 | 同步 |
| --- | --- | --- |
| `touch_keyboard_split` | 布尔，默认关 | 随账号同步，键 `platform.harmony.split_keyboard`（Android 的主开关 `platform.android.split_keyboard` 也同步） |
| `touch_keyboard_split_threshold` | 480–1000、步进 20，默认 600 | 只留本机（Android 的也不同步） |
| `touch_keyboard_split_gap` | 10–40、步进 5，默认 25 | 只留本机（Android 的也不同步） |

`touch_keyboard_split_portrait` 两端共用，仍不同步。默认值都不写进文档，范围外或不在步进上的值由 `Preferences::validate` 拒收（`InvalidTouchKeyboardSplit`），鸿蒙读到不合规的值按默认值（`SplitKeyboardPolicy.settings`）。Android 不读这三个新键，它的行为不变。

设置界面放在共享 React 设置页「屏幕键盘 › 布局」，新能力位 `touch_keyboard_split`。鸿蒙的设置桥只在平板和可折叠手机上报告它，判断与侧边数字键盘相同：直板手机的最短边到不了最低 480vp，从不分离。四行的标题、说明和范围与 Android `KeyboardOptionsPage` 逐字相同，主开关关着时后三行不可调。

服务端字段表要收录 `platform.harmony.split_keyboard`（boolean，与 Android 同名键相同），否则客户端上传前就把它滤掉，见 msime-cloud 的配套 PR。

### iPhone：可选打开，复用 iPad 的分离路径

- 开关是 App Group `keyboard.phone.landscapeSplit`（`KeyboardLayoutPreference.phoneLandscapeSplit`），默认关，现有用户的横屏键盘不变。它只在本机，不随设置同步，与 iPad 的开关一样。它和 iPad 的 `keyboard.tablet.split` 分开存，同一个值在两种设备上意思不同（iPhone 多一道宽度门槛）。App「键盘」页「布局」一组只在 iPhone 上列出这一行，「恢复默认」把它和 iPad 的开关一起还原。
- `KeyboardSplitLayout.phoneIsActive`：要求 `userInterfaceIdiom == .phone`、手机形态、横屏，分开后第一排字母键不窄于 `phoneMinimumLetterKeyWidth`（50pt），开关打开。iPad 的浮动键盘和窄窗口虽是手机形态，也不分。
- 几何走 iPad 那一套：中缝占键区宽度 25%，字符键从中间分，底行在空格处分、两个空格同宽，键盘高度不变。键面仍是手机的，键高仍是手机横屏的 34pt（手写 40pt），iPhone 不再另设键高。iPad 第三排末尾有 ，。，九个字符键在 b 后面分（zxcvb | nm，。）；手机上 ，。 隐藏，只剩七个字母，所以这一排另插一段只给手机键面用的中缝，在 v 后面分（zxcv | bnm）。两段中缝按键面显示其中一段。
- `KeyboardOneHandLayout.effective` 改成只看是否分离，不再限定 iPad：iPhone 分离时单手模式同样让位。

### iPhone 的宽度门槛：50pt，而不是「iPad 分离的最小键宽」

代理最初的要求是「每一半的键不窄于 iPad 分离的最小键宽」。照字面算不出可用的门槛。iPad 分离最窄的是 iPad mini 横屏：键区 1133 − 16 = 1117pt，第一排十个字母加 1.3 份的 ⌫，默认 6pt 键距下字母键约 69.4pt。iPhone 第一排是十个字母，宽度为 `(0.75 × 键区宽 − 8 × 键距) / 10`，默认键距下：

| 机型（横屏宽） | 分开后字母键 | 分不分 |
| --- | --- | --- |
| iPhone SE（667） | 44.78pt | 不分 |
| 6/7/8 Plus（736） | 49.95pt | 不分 |
| X、XS、11 Pro、12/13 mini（812） | 55.65pt | 分 |
| 12、13、14（844） | 58.05pt | 分 |
| 15、16（852） | 58.65pt | 分 |
| 16 Pro（874） | 60.30pt | 分 |
| XR、11（896） | 61.95pt | 分 |
| 12/13 Pro Max（926） | 64.20pt | 分 |
| 14/15 Plus（932） | 64.65pt | 分 |
| 16 Pro Max（956） | 66.45pt | 分 |

最宽的 iPhone 也只有 66pt，按字面要求所有 iPhone 都不分，等于没做。代理确认改用 50pt。这个数留出比 44pt 触控下限更宽的余量，因为底行的功能键跟着变窄。默认键距下 SE 和 Plus 不分，全面屏 iPhone 都分。键距调到最小 3pt 时 Plus 是 52.35pt，会分；SE 是 47.18pt，仍不分。

### 文字

三端开关标题都是「分离式键盘」，iPad 原来的「横屏分离式键盘」随之改名，Tauri 设置页的 iPad 行也一样。说明中间一句三端相同：「26 键和韩文键盘分成左右两半，方便双手握持时用拇指输入」，前后写各自的生效条件。Android、鸿蒙是「默认仅在大屏横屏时生效：……；右半边也有 G 和 V」。iPhone 是「仅在 iPhone 横屏时生效：……；屏幕较窄的 iPhone（如 iPhone SE）保持完整键盘」，iPhone 不重复 G、V，所以没有后半句。

## Related notes audit

- [Android 大屏竖屏也可使用分离键盘](2026-10-23-android-portrait-split.md)：部分重叠。`touch_keyboard_split_portrait` 现在鸿蒙也读，也在共享设置页给出，那篇的事实已就地更新并链回这里。
- [Android 分离键盘启用条件与中间空隙可调](2026-10-27-android-split-keyboard-controls.md)：部分重叠。鸿蒙照搬那两项的范围与默认值，但存成共享偏好；Android 不变。
- [Android 平板分离键盘按窗口限制键区宽度](../bug-fix/2026-10-26-android-tablet-split-width.md)：部分重叠。鸿蒙移植了 720 键区上限。
- [数字行与分离重复键](2026-10-09-android-number-row-and-split-duplicates.md)：部分重叠。鸿蒙移植了重复 G、V；iPhone 沿用 iPad，不重复。
- [大屏单手模式缩到手机大小](2026-10-29-tablet-one-handed-size.md)、[大屏单手侧边数字键盘](2026-10-29-tablet-one-hand-side-number-pad.md)：部分重叠。它们写过「鸿蒙没有分离式键盘」，现在鸿蒙分离时单手让位，代码里的对应注释已改。
- 检索 proposed、rejected 没有相同提案。

## Alternatives considered

- **鸿蒙把三个设置存进键盘自己的 `key-feedback.json`**：单手模式、按键弹出等「Android 本机设置」在鸿蒙一直放在那里，不碰 client-core。不采用：主开关要像 Android 那样同步，而那份文件只有按键反馈三项走账号同步，要同步就得扩 `HostKeyboardFeedback` 和整条 NAPI 通路；共享偏好则已有校验、默认值和 `harmony_settings` 的同步路径。竖屏选项本来也是共享偏好，四项放在一起。
- **鸿蒙复用 Android 的键名，让两端同步同一个开关**：一个开关管两种设备最省事。不采用：鸿蒙所有触屏键都在 `platform.harmony.*` 下，理由写在 `harmony_settings.rs` 开头：鸿蒙手机和 Android 手机是两台各有键盘的设备，共用命名空间会让一台改掉另一台。
- **iPhone 复用 iPad 的 `keyboard.tablet.split`**：少一个键，设置页同一个绑定。不采用：iPhone 多一道宽度门槛，又要默认关，混用一个值会让 iPad 上的打开在恢复备份等情况下带到 iPhone，现有用户的横屏键盘就变了。
- **iPhone 底行每个空格只取一半份额并让中缝居中**（Android 的做法）：功能键不会被两个整宽空格挤窄，中缝也与字母行对齐。不采用：iOS 手机底行有十几种排法（各样式、英文、地球键、隐藏中/英），每种都要另配一套约束；代理要求复用 iPad 的路径，iPad 的隐藏中/英键排法同样容忍中缝不居中。代价写在后果里。
- **iPhone 的中缝调窄**（比如 15%）：同样宽的屏幕上字母键能宽约 8pt，Plus 也能分。不采用：中缝窄了，拇指够内侧键的距离优势就小了。25% 与 iPad、Android 的默认一致，门槛也按它算。

## Consequences

- 鸿蒙平板和折叠屏内屏可以分离，规则、默认值和设置文字与 Android 一致；Android、iPad 的行为不变。
- iPhone 横屏多了一个可选的分离式键盘，默认关；SE、Plus 打开开关也不分，说明里写了。
- client-core 多了三个只有鸿蒙读的共享偏好；Android 的主开关和两项数值仍在它自己的本机设置里，两端的存法不对称。将来 Android 想迁到共享偏好时，可以按这三个键迁，迁移前要处理 `platform.android.split_keyboard` 的同步。
- iPhone 分离时底行两个空格都是原宽，其余功能键约是不分时的一半宽（852pt 宽时「，」约 41pt），中缝也不与字母行居中对齐，与 iPad 相同。
- 鸿蒙两个空格共用一套空格手势状态，两根拇指同时按两个空格时后按下的接管；Android 每个空格各自一份。鸿蒙重复的 G、V 不单独上报触点几何，按下点按左边那个键上的同一相对位置换算，双拼触点纠错在右半边 G、V 上的邻键关系不准。
- 服务端字段表收录 `platform.harmony.split_keyboard` 之前，鸿蒙的主开关不会同步，也不报错。

## Verification

- client-core：新增两组用例，覆盖默认不写进文档、往返、类型错拒收、范围和步进边界；`cargo test -p msime-client-core --lib preferences` 262 项通过。
- client-services：`split_keyboard_switch_syncs_under_the_harmony_key_and_its_bounds_stay_local` 覆盖导出、应用、不读 Android 的键、字段表没收录时不应用；`cargo test -p msime-client-services --lib account` 422 项通过。
- 共享设置页：`apps/desktop/tests/settings/harmony-split-keyboard.test.tsx` 覆盖能力位、文字、默认值、范围、依赖开关和写偏好；iPad 行改名后的既有用例也跑了。`tsc --noEmit` 和四个设置页测试文件（440 项）通过。偏好对齐脚本和 macOS 偏好覆盖脚本通过。
- 鸿蒙：`tests/split-keyboard.test.ts` 覆盖读法、启用条件（600/480/1000 的边界、竖屏、手机横屏、2in1）、空隙比例、三排断点和重复键、底行空格和能力位；`tests/run.sh` 钉住视图、会话、设置桥的接线，全部通过。`scripts/test-harmony-arkts-subset.py` 通过。Mac Studio 上 `ohpm install && hvigorw assembleHap --no-daemon` 编译通过，改过的文件没有 ArkTS 警告。
- iOS：`KeyboardFormFactorTests` 新增 iPhone 一节，覆盖 49.95pt / 50.025pt 边界、各机型宽度、SE 和 Plus 默认键距下不分、只认 iPhone、开关默认关并随「恢复默认」还原、iPhone 横屏四段中缝（第三排 v | b）、两个空格同宽、键盘高度不变、转屏，以及 iPad 不受 iPhone 开关影响、第三排仍是 b | n。`OneHandModeTests` 补 iPhone 分离时单手让位。Mac Studio 上用一台新建、用完即删的 iPhone 18 Pro Max 模拟器（Xcode 27）跑整套 `MSIMEClientTests`：`MSIMEKeyboardTests` 1191 项（含一项不提交的临时截图用例）、10 项跳过、0 失败，其余测试套件也全部通过；`MSIMEApp` 的模拟器构建通过。临时截图用例把 852pt 宽的 iPhone 横屏（分离和不分离）、956pt、667pt（开关开着仍是整块）和 1194pt 的 iPad 分离画成图，看过：iPhone 分离后第三排在 v | b 分开，iPad 的画面与改动前相同。
- 没有验证的：鸿蒙没在平板模拟器或真机上看过（本机只有 2in1 模拟器，2in1 不画触摸按键）；iPhone 没在真机上装键盘打过字，只在模拟器上把键盘视图画成截图。

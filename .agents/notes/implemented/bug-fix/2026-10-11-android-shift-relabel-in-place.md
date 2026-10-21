# Agent Note: Android 大小写变化就地改键面，不重建键行

Status: implemented

## Problem

Shift 状态一变，紧跟着按下的字母常常没有反应，稍等一下再按才行（#6997 第 5 项，Android 10、0.3.1）。Shift 的点击处理、一次性大写用掉（`letterCase.consumeLetter()`）、自动大写（`updateAutomaticCapitalization`）都调用 `ImeLetterRows.rebuildKeyRows()`：先 `keyRows.removeAllViews()`，再新建所有行和字母键。新键在下一次布局之前宽高都是 0。

- 在这段时间里落下的按压：`KeyboardKeyArea.search()` 找不到包含触点的键，`consider()` 跳过宽或高小于 2 的键，键区和行本身不可点击，整个手势无声地丢掉。
- 按压落在重建之前（Shift 的点击是在抬起后 post 的，快速连打时下一个手指已经按下）：手指按在旧键上，旧键随即被摘下，收不到抬起，也就没有点击。

慢设备上重建本身更久（约 30 个键的样式、`render()`、整窗 `requestLayout`），这段空窗也更长。这些推断来自读代码，没有在设备上复现。

## Decision

`ImeLetterRows.refreshLetterCase()` 取代上述调用点里的 `rebuildKeyRows()`：键行是通用分支在字母层建的（记在 `builtLetterLayout`），且显示的布局、分离式键盘、数字行和建的时候一样、也不是 26 键的九键数字键面时，就地改每个字母键的键面和无障碍描述；否则照旧整组重建。

- 字母键面由 `LetterKeyFacePolicy.face(input, chineseMode, japanese, localMode, shifted)` 算，中英和本地模式的变化同样只影响键面，所以 Shift 顺带从中文切到英文（`toggleInputLanguage`）也走就地改，不要求 `chineseMode` 与建键时相同。
- 中英切换在写入新语言状态并重置 `letterCase` 后总是调用 `refreshLetterCase()`，不能只看布局编号与旧 Shift 是否大写（#8982）：中文 26 键在小写状态下也固定画大写，英文同样的小写状态画小写。两边布局与 Shift 都不变时，键面仍必须刷新；不满足就地更新条件的布局变化继续由 `refreshLetterCase()` 重建。
- 微软双拼的 `;` 键不随大小写变，跳过。分离式键盘右半边重复的 G、V 在同一个列表里，一起改。
- 分离式键盘第二行的切分位置和内侧 G 是否重复，建行时按当时第二行有没有微软双拼的 `;` 键（`microsoftTenKeys()`，随中英切换变）算一次，记在 `builtMicrosoftTenKeys`。分离式键盘下它和建行时不同（微软双拼中文空闲时按 Shift 切到英文）就整组重建，否则两半不等宽、少一个重复的 G。
- 韩文键帽不在 `shuangpinKeyButtons` 里，另记 `koreanKeyButtons`，按 `KoreanKeyboardLayout.face/accessibilityLabel(key, shifted)` 改，Shift 换双辅音时同样不重建。
- 字母键气泡在按下时读 `key.getText()`，不再用建键时记下的键面，否则就地改后气泡还显示旧的大小写。
- 无障碍描述先写成不带提示的字母描述，与重建时一样；随后的 `render()` 里 `updateShuangpinKeyHints` 补上双拼提示后缀。

`check-host.sh` 加了一道源码检查：Shift、`consumeLetter`、`applyAutomatic` 之后直接调 `rebuildKeyRows()`（中间只隔着 `//` 注释行也算，Shift 处理里就隔着一行），或气泡用建键时的 `face`，都会失败。

## Alternatives considered

- 重建后同步做一次 measure/layout，让新键马上有宽高：只能补上「按压落在重建之后」那一半，按在旧键上的手指被摘掉的那一半补不上，而且每次 Shift 仍要建 30 个键。
- 在 `KeyboardKeyArea` 收到按下时检查子键是否为零尺寸、必要时先同步布局：能兜住所有重建（123、中/英换布局等）留下的空窗，但同样救不了被摘下的旧键，改动的是所有按键的命中路径。这次只处理大小写，留作以后。
- 判断条件里要求 `chineseMode` 与建键时相同：最常见的路径（中文空闲时按 Shift 进英文大写）会因此退回重建，问题照旧。

## Consequences

Shift、一次性大写用掉和自动大写不再销毁字母键，紧跟着的按压落在原来的键上。布局真正变了的情况（中文九键切英文 26 键、123 层、数字键面、旋转导致分离式键盘变化）仍然重建，那些路径上的零尺寸空窗还在。重建原本顺带做的 `hideKeyPreview()`、`deactivateHandwriting()` 在就地改时不做：通用字母分支没有手写面板，正在按的键的气泡留着是想要的结果。

## Verification

`check-host.sh` 的新检查在改动前的源码上命中 7 处（Shift、`type()` 的 4 处、硬件键盘 1 处、自动大写 1 处）加气泡 1 处，改动后不命中；只把 Shift 处理里注释下面那行改回 `rebuildKeyRows()` 时也命中（最初的正则只容许空白，认不出这种回退）；Android JVM 冒烟与设备套件编译通过。没有在设备或模拟器上复现「Shift 后立即按字母被丢」，也没有在设备上验证修复后的表现；iOS 和 HarmonyOS 是否在 Shift 时重建键行没有检查。

#8982 的设备回归补充普通中英切换：`ChineseShiftReturnDeviceSmoke` 在真实原生服务上检查三轮往返后的全部 26 个键帽、无障碍描述、按键对象保持与小写上屏，再运行临时大写、锁定、永久英文、实体键盘与模式记忆回归；`QuanpinSeparatorDeviceSmoke` 在系统编辑器中检查三排键帽和 `qA` 的实际输出。它们与原始 #6997 的快速连打丢键时序是不同的验收范围。

API 28 的旧安装包先在「切到英文后显示小写键帽」失败，修复后的两套回归在 API 28、35 均通过，原生 Android 宿主 `assembleFullRelease` 及 `check-host.sh` 的 250 项 JVM 冒烟、196 个设备套件源文件编译和 Java/API、清单、资源检查通过。夹具只统计 a-z，不把首道双拼额外的分词键算成第 27 个字母。验收包用本分支 Java 宿主，原生库与资源固定取自官方 `android-v0.5.0` APK（提交 `d61258b1f5dd21959c13c6df7c72ac3a1ec7dbc7`，SHA-256 `924d1727b9ecd1099c7a96d1acaadec4a9cfc95c8a54d2f075e21c2a0a3238a1`），重新打包时排除 `assets/dexopt` 并用编译机开发证书签名；未修改 Rust、资源或记录存储，不将该验收当作当前分支完整 Rust 构建、发布签名或报告者真机验收。

Mac Studio 上 `scripts/verify-local.sh --quick` 与 `pnpm run verify-notes` 通过。quick 覆盖仓库契约、Rust workspace、Android 目标、Android Java 宿主与共享 Apple 桥接层，其他平台按改动范围跳过。

# Agent Note: Android 键盘的「加高底行」

Status: implemented

## Problem

#6354：九宫格第四行（123、中/英、空格、换行）点起来「像减速带」，比上面三行矮。追下去，键行和底栏的高度来自两个常量：所有布局的键行是 `KeyboardGeometry.KEY_ROW_HEIGHT_DP`（56 dp，另加一份行距），底栏是 `STANDARD_ROW_HEIGHT_DP`（46 dp，不加行距）。键帽在行里上下各让出半份行距（默认 3.5 dp），所以默认时九宫格键帽 56 dp、底栏键帽只有 39 dp，矮了三成；键盘高度调高时键行还会分到调整量，底栏不分，差距更大。26 键的底栏是同一行，问题相同。

56 dp 来自 #3782 的改版：按设计的 46 dp 排，九宫格的键像横条，于是所有布局的键行一起加到 56 dp，底栏留在设计键高 46 dp（`KeyboardGeometry` 的注释「底栏仍是 46 dp」）。底栏不分摊高度调整是 [2026-10-08-android-keyboard-height-range](2026-10-08-android-keyboard-height-range.md) 定下的：功能行固定高度，调矮键盘时换行键不被压扁。仓库里没有写明底栏应该比键行矮的设计规范，设计稿（`N/design-tokens.md` §5）不在仓库里，46 dp 是设计里唯一的标准键高。把底栏改成和键行一样高会让每个用户的键盘默认高出约 17 dp，属于产品取舍，交给维护者决定。

## Decision

- 新增本地设置 `platform.android.tall_bottom_row`，布尔，默认关，只在本机（服务端同步字段表没有这个键），入口在设置「键盘」页「布局」里，紧挨「键盘底栏」，设置搜索能搜到。默认关时所有高度与原来完全相同。
- 底栏高度集中到 `KeyboardGeometry.bottomRowHeightDp(tall)` 和 `bottomRowSpacings(tall)`：关时 46 dp、不加行距；开时 56 dp 并像键行一样加一份行距，键帽和上面的键一样高。两种都不分摊高度调整，保留「功能行固定高度」的约定。
- 用到底栏高度的三处都按它算，开关一变，各布局的总高仍然一致：功能行（`actionRow`）和 123 / #+= 层自带的底栏挂 `KeyboardHeightRole.bottomRow()`；日语九键、注音九键没有单独的底栏，四行装在一个块里，块挂 `KeyboardHeightRole.threeRowsWithBottomRow()`，在三行键之外加上底栏那一份。高度在 `ImeStyler.applyKeyboardHeight` 套用几何时按当前设置算，功能行原来不经过这一步，现在也经过。
- `applyLocalSettings` 读到这一项变了且键盘视图已建好时，立刻重新套一次几何，设置页改完下一次弹出键盘就生效。

## Alternatives considered

- **直接把底栏默认改成和键行一样高**：最贴近报告人的期望，也让各行键帽一致，不需要用户去找开关。但每个用户的键盘默认高出约 17 dp，竖屏手机上候选区和输入框都被往上挤；改版时留着 46 dp 可能另有考虑，没有设计稿核对，不由这次改动擅自决定。
- **键盘总高不变，从三行键里匀出高度给底栏**：默认高度不变、各行一样高。但九宫格键和字母键都会变矮（56 → 约 51 dp），#3782 刚因为键太扁把它们加到 56 dp，这等于部分撤回那次决定，同样是产品取舍。
- **底栏分摊键盘高度调整**：调高键盘时底栏跟着变高，和键行的差距不会随高度拉大。但调矮时换行键会被压扁，正是 [2026-10-08-android-keyboard-height-range](2026-10-08-android-keyboard-height-range.md) 要避免的；加高底行开着时底栏已经是 56 dp，比默认的键帽高不少。
- **只加高九宫格的底栏**：issue 只说了九宫格。但 26 键和九宫格用的是同一条功能行，切换布局时一条高一条矮，键盘高度会跳。

## Consequences

- **收益**：想要各行一样高的用户有了开关，开着时底栏键帽与九宫格、字母键同为 56 dp（默认行距下）。底栏高度只在 `KeyboardGeometry` 一处定义，以后要改默认值只改这里。
- **代价**：多了一个本地开关；开着时键盘整体高出 10 dp 加一份行距（默认约 17 dp）。键盘调高很多时底栏仍比键行矮，因为它不分摊调整量。
- **需要维护者决定**：默认要不要打开、要不要改成默认就和键行同高，见 PR 正文。iOS 和 HarmonyOS 的底行高度没有逐一核对。
- **验证**：`KeyboardGeometrySmoke` 钉住两种底栏高度和行距，`AndroidLocalSettingsSmoke` 钉住默认关、不同步，`SettingsSearchIndexSmoke` 核对设置搜索能搜到这一行，均由 `platforms/android/check-host.sh` 运行；API 35 模拟器上装包，开关前后截图对比 26 键和九宫格的底栏高度，并跑 `smoke.sh` 的设备套件。

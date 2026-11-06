# Agent Note: Android 文本编辑面板无缝方向区、七个侧键与新默认排列

Status: implemented

## Problem

[四行网格](../bug-fix/2026-11-06-android-text-edit-panel-four-rows.md)之后，文本编辑面板同屏有十八个键：十字方向键各占一格，上排两角是行首、行尾，底排正中是第八个侧键（默认选择行），顶上还有一行 32 dp 的模式行放「切换为触控板」和触控板模式的「选择」。维护者给出的新设计要求：

- 方向区连成一整块，← 和 → 跨三行，中间一列是 ↑、开始选择、↓，方向键画尖括号而不是字体箭头；
- 精简常驻键：行首、行尾、选择行不再常驻，侧边只留复制、剪切、粘贴、全选和退格、数字、空格；
- 触控板开关进网格，底排是 ⇤、触控板、⇥；
- 侧键默认顺序换成左列复制、剪切、粘贴、全选，右列退格、123、空格。

默认顺序存在共享偏好 `touch_text_edit_button_order` 里。已经发出去的版本把当时的默认排列明文写进了偏好（Android 0.6.0、HarmonyOS 0.3.0-beta、macOS 0.55.0、Linux 0.14.0 写七个动作，Windows 7 0.3.0 写八个），只换 `default_text_edit_button_order` 有两个后果：这些用户的文档里明文写着旧顺序，永远换不到新布局；新版本存一次就把旧顺序连同 `select_line` 写成八项，退回那些只认识七个动作的版本时整份偏好读不了，打不了字。

## Decision

`TextEditPanelModel` 仍是四行、十个半格的网格，左右两列各两个半格放侧键，中央六个半格是方向区：

| 行 | 左列 | 方向区（半格 2–7） | 右列 |
|---|---|---|---|
| 0 | 侧键 1 | ←（跨 0–2 行）· ↑ · →（跨 0–2 行） | 侧键 5 |
| 1 | 侧键 2 | ← · 开始选择 · → | 侧键 6 |
| 2 | 侧键 3 | ← · ↓ · → | 侧键 7 |
| 3 | 侧键 4 | ⇤ · 触控板 · ⇥ | 换行（固定） |

网格没有空格。`SELECT_LINE`、`LINE_START`、`LINE_END` 不生成键，枚举、按键码、修饰键和 `selectLine` 的编辑器适配都保留。

### 触控板开关

面板顶部不再有模式行。`TRACKPAD` 是底排正中的固定键，键面只写「触控板」一行，是开关而不是动作：触控板开着（`android_text_trackpad.enabled` 或临时触控板）时用选中样式，也就是和「开始选择」一样的强调色填充；读屏把它当作开关（`TextEditPanelButton.setToggle`：无障碍类名 `Switch`、可勾选、勾选状态跟随选中），状态描述念「已开启」「已关闭」。点按走 `toggleTrackpad`，不经过 `perform`，保存照旧在偏好线程里改 `android_text_trackpad.enabled`；保存中、临时触控板开着、触控板设置还没读到或没有偏好目录时禁用。改了选中状态立即重新上色，原因同「开始选择」（#10454）。

触控板自己画底面，看上去是方向区换了一种内容：底色、上面两角的圆角与网格画的方向区相同（`TextEditPanelGrid.surfaceRadius`），下边贴着底排，外框线也自己补上，因为触控板盖住了网格画的那一段。底面是一层 16 dp 间距、1 dp 半径的淡色点阵（前景色 22% 不透明度），在两个方向上各自居中（`TextTrackpadSurface.dotCount`、`firstDot`）。中央只有一句淡色提示「滑动移动光标」（前景色 45%），14 sp 放不下时等比缩小，缩到 10 sp 以下或高度不够一行就不画（`TextTrackpadSurface.hintSize`）；手指按下后不画，光标预览和放大镜出现时不和它叠在一起。原来的两行全色说明（「左右移动字符，上下移动行」和长按、双击、三击的说明）不再显示；手势本身不变，双击选词、三击选行仍跟设置走。

触控板模式的「选择」开关压在触控板右上角（`TextEditPanelModel.trackpadSelectBounds`：优先 64 × 32 dp、四周留 4 dp，触控板太小时缩小，不越出触控板），只在触控板模式显示；它比触控板后加进同一个 `FrameLayout`，落在它上面的触摸归开关。

### 网格与绘制

`TextEditPanelModel.bounds` 同时给出测量、摆放和点击范围，`TextEditPanelGrid` 按它摆子视图，不再用按行的 `LinearLayout` 加权。侧键四周留 3 dp 键距；方向区外框上下与侧键首尾齐平，左右各让一个键距。方向区的横缝 `seam(row)` 对齐下一排侧键的上边，所以底排三格与同排侧键等高，上面三行只差整数像素舍入。网格画方向区的圆角底面、外框和中性细线（两条竖缝贯穿四行，两条横缝只在中列），线画在子视图之后；中央键是 `TextEditPanelButton`，只画按下和选中的填充，选中用皮肤强调色。换肤时 `ImeStyler.applySkinToView` 调 `TextEditPanelGrid.setSkin` 更新底面、线色和强调色。

方向键和文档首尾是 `TextEditIconButton`：旋转 `CHEVRON` 得到四向尖括号，旋转新图标 `EDIT_DOCUMENT_START`（竖线加单尖括号）得到文尾。图标路径照例只改 `generate_keyboard_icons.py`，再生成 `KeyboardIconPaths.java`。

触控板的位置也从同一套模型算：盖住方向区外框里底排以上的三行（`centralBounds` 的上沿到 `seam(3)`），不再写死 `height / 4`、`height / 2`。左右两列和底排的 ⇤、触控板、⇥ 在它外面，始终可点。

### 排序与旧默认排列

偏好里仍是八个动作的完整排列，存盘格式不变。`TextEditPanelModel.visibleOrder` 只取七个可排序动作（`isSortable`），依次填左列四格、右列前三格；选择行排在哪里都不占位置。排序页只列这七个，保存时 `mergeVisibleOrder` 把拖好的顺序填回可排序动作原来的位置，选择行留在原位；恢复默认同样只显示默认排列里的七个。

共享默认排列是 `copy, cut, paste, select_all, delete, number_pad, space, select_line`。`deserialize_text_edit_button_order` 补齐排列以后，如果结果正好等于旧默认排列 `LEGACY_DEFAULT_TEXT_EDIT_BUTTON_ORDER`（`copy, paste, select_all, cut, space, number_pad, delete, select_line`），就当作没排过顺序，换成现在的默认排列。已发布版本写下的七项和八项两种写法补齐后都等于它，所以从这些版本升级上来、没排过顺序的用户直接用上新布局；存盘时按[默认排列不写进文件](../bug-fix/2026-10-28-preferences-forward-compat-runtime.md)省掉这个键，退回那些版本仍然读得了。这一步在读偏好的地方统一做，读盘、导入和同步都一样。

## Related notes audit

- [固定网格与七键排序](2026-10-24-android-text-edit-layout.md)：部分取代。网格形状、默认顺序和触控板覆盖范围以本篇为准；七个侧键存在共享偏好、默认值和校验归 `client-core`、排序页草稿与保存的理由仍成立。
- [行边界与换行长按符号](2026-10-25-android-text-edit-line-keys.md)：部分取代。行首、行尾不再有键，Home/End 不带 Ctrl 的按键适配保留；换行短按、长按进符号面板不变。
- [选择当前行](2026-10-26-android-text-edit-select-line.md)：部分取代。选择行不显示、不参与排序；行范围计算、读入时补齐缺项和严格提交校验仍成立。
- [四行网格](../bug-fix/2026-11-06-android-text-edit-panel-four-rows.md)：部分取代。底排正中改放触控板开关，第八个侧键不再显示；触控板位置改由网格模型推出。
- [触控板与放大镜](2026-10-23-android-text-edit-trackpad.md)：手势、模式持久化、临时入口、预览与取消生命周期不变，只改覆盖区域和多了一个网格内的开关。
- [偏好文档读不了时输入运行时照常运行](../bug-fix/2026-10-28-preferences-forward-compat-runtime.md)：默认排列不写进文件的决定不变；其中「旧版本的七个动作补齐后就是现在的默认排列」一句已就地改为经旧默认排列换算。
- [原文本编辑面板](2026-10-08-android-text-edit-panel.md)、[最近复制预览](2026-10-15-android-recent-copy-in-text-edit.md)、[光标边界保护](../bug-fix/2026-10-29-android-text-edit-cursor-edges.md)、[排序面板拖动](2026-10-31-android-order-sheets-drag.md)：无关或只沿用，编辑器权限、复制、预览和拖动排序不受影响。

## Alternatives considered

- **保留 #10973 的十八键布局，只换画法**：选择行、行首、行尾都还有入口，不用迁移任何东西。但方向键各占一格、模式行另占 32 dp，主要的移动目标偏小，设计要求的整块方向区做不出来。
- **在固定的七格之外让选择行也能被拖进可见位置**：用户可以自己决定要不要选择行。但排序页就得表达「哪些显示、哪些藏起来」，网格位置数也随之变化；维护者的设计明确不常驻这三个键，所以它们不参与排序。
- **只换默认值，不处理旧默认排列**：改动只有一行。但已发布版本写下的旧顺序会一直留着，新布局只到全新安装的用户；新版本存盘时把旧顺序写成八项，退回只认识七个动作的版本就整份读不了，`a_default_text_edit_order_is_left_out_of_the_file` 守的正是这件事。
- **保留用户文档里的旧顺序，排列以选择行结尾时只写前七项**：有意排成旧顺序的用户不会被改，退回旧版本也读得了。但已发布版本把默认值明文写进了所有用户的文档，等于绝大多数用户都留在旧布局上，和换默认值的目的相反。
- **保留模式行，键面写「切换为触控板」「切换为方向键」**：[固定网格](2026-10-24-android-text-edit-layout.md)为 #10058 选这种写法，是怕只写模式名会被当成当前状态的标签。现在开关就在网格里、用选中样式和读屏的开关状态表达开着与否，「触控板」三个字本来就是状态标签；保留模式行还要多占 32 dp，六个字在窄窗口里只能缩字。
- **触控板模式的「选择」放回网格（比如盖住某个侧键）**：不占触控板面积。但侧键约定始终可用，盖掉哪个都不对；放进触控板角落只让出一小块，手指从别处起滑不受影响。
- **触控板上保留两行全色说明，或把方向键画成淡色背景**：新用户一眼看得出能做什么。但全色文字和按键文字抢视线，方向键的淡色背景又像是能点的键；只留一句淡色提示，配合点阵表示「这一块可以滑」。
- **开关写两行（「方向键」主文字，下方淡色小字「触控板」）**：一眼看出两种模式。但这个面板里淡色文字表示不可用（没有选区时复制、剪切就是淡的），淡色的「触控板」会被读成触控板不能用；两行字在横屏和浮动时也放不下。所以开关只写「触控板」一行。

## Consequences

方向区是一整块连续的目标，← 和 → 的高度是原来的三倍；网格没有空格，触控板位置不再和网格各写一套比例。去掉 32 dp 的模式行，这部分高度还给网格；开关键只写三个字，不再有「切换为触控板」六个字在窄窗口里缩字或折行的问题。没排过顺序的用户升级后直接换到新的侧键顺序，退回已发布版本仍然读得了偏好。

代价：选择行、行首、行尾失去可见入口，只剩动作身份和适配代码，以后要恢复得另找手势或位置。有意把侧键排成旧默认顺序的用户也会被换成新默认，文档里分不出两者。触控板上不再写长按、双击、三击怎么用，只能靠设置页的说明和读屏公告发现。开关键面不再写出点了会切到哪种模式，只靠选中样式和读屏状态表达开、关；触控板模式的「选择」占掉触控板右上角一小块。中央键不再是独立键帽，换肤和按下反馈依赖 `TextEditPanelGrid` 与 `TextEditPanelButton` 同步；无障碍节点和点击范围仍按键分开。

## Verification

- `cargo test -p msime-client-core`（800 项）在 WSL 通过，包括新增的 `an_untouched_old_default_text_edit_order_moves_to_the_new_default`（七项、八项旧默认读成新默认，只差一个位置的排列原样保留）、`a_default_text_edit_order_is_left_out_of_the_file`（各已发布版本的文档存一次后不再写出这个键）；`msime-client-services` 的 `desktop_settings_sync_works_on_a_document_from_a_newer_version` 和 `msime-host-api` 的偏好保存、读不了的文档两项通过；`cargo fmt --check` 通过。
- Android：按 `check-host.sh` 的同一份源码规则编译 859 份生产和测试源码（`-Xlint:all -Werror`），`TextEditPanelModelSmoke` 通过：十六个键互不重叠、网格无空格，正序、逆序和选择行排在中间时侧键都按可见顺序摆放，排序草稿合并保留选择行位置、残缺草稿被拒，三种尺寸下中央键正好铺满方向区且底排与侧键齐平，触控板模式的选择开关贴右上角、触控板太小时不越界；`TextTrackpadSurfaceSmoke` 覆盖点阵居中、长度不够或参数无效时不画点，以及提示文字的等比缩小和隐藏条件；`TextTrackpadGestureSmoke`、`TextTrackpadHoldSmoke` 照旧通过；271 份设备源码编译通过；`generate_keyboard_icons.py --check` 通过。
- 未覆盖：完整 `check-host.sh`、Gradle 编译（`TextEditOrderSheet` 引用 androidx，只有 Gradle 编得到）、模拟器设备验收都还没跑；`TextEditRecentClipboardDeviceSmoke` 仍按旧布局断言「选择」文字和底排键数，它和 `TextEditLineDeviceChecks`、`TextTrackpadDeviceChecks` 也还在点「切换为触控板」「切换为方向键」，都要随布局更新后再跑。

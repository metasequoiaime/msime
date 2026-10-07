# Agent Note: Android 九键左侧符号栏与数字键面

Status: implemented

## Problem

Android 拼音九键左侧的符号栏原来是 ，。？ 三个键铺满三行键高，每个键约 54 dp 高，间距大到还能再放两个，也没法换成自己常用的符号（#5574）。笔画键盘用的是同一个外框，问题相同。

## Decision

### 左侧符号栏

- 拼音九键和笔画键盘的左栏是 `NineKeySymbolRail`（`platforms/android/java/app/msime/android/keyboard/NineKeySymbolRail.java`）：一个不显示滚动条、带上下渐隐边的 `ScrollView`，一屏显示 `NineKeySidebarPolicy.VISIBLE_ROWS`（5）个符号，多出来的上下滑动。每个符号键的高度在 `onMeasure` 里按栏高均分（`NineKeySidebarPolicy.rowHeight`），并扣掉键自己的上下外边距，所以键盘高度和行距偏好变了不用重建；符号不足五个时按实际个数均分整栏，不留空白。
- 符号表来自本地设置 `platform.android.nine_key_symbols`（`AndroidLocalSettings` 新增的 `TEXT` 类设置）：用空格分开的符号，存储前经 `NineKeySidebarPolicy.normalize` 规范成单空格连接、去重；空串、单个符号超过 4 个码位、含控制字符、超过 30 个符号或文本超过 200 个 UTF-16 单元一律不合规，写入时拒收，读到时整张表回到默认值。默认表是 `， 。 ？ 、 ： ； …… ～ @`：前三个和原来一样，！ 仍在右列最下面，不在左栏重复。
- 设置页「键盘 › 布局 › 九键左侧符号」用 `InputDialog` 编辑这张表，留空恢复默认（删掉这一项）。键盘每次出现时重读本地设置，`ImeLayoutRows.sidebarStale` 发现屏上的符号表和设置不同时，`render` 重建键行。
- 这个设置只在本机，不随账号同步：同步表在 `crates/client-core/src/account/settings_sync.rs` 的 `ANDROID_LOCAL_SETTINGS`，那里只有布尔、选项和整数三种类型。

## Alternatives considered

- **只把三个键改小、固定显示五个，不滚动** — 改动最小；但用户要求能上下滑动和自定义，固定五个装不下自定义表，最后还是要滚动。
- **符号表放进共享偏好（`crates/client-core` 的 `Preferences`），走 Tauri 设置页** — 能随设置同步，其他平台日后也能读；但只有 Android 的九键有这条可滚动的符号栏，共享偏好要改 Rust 结构、默认值、同步和 Tauri 页面，其他平台却没有任何消费者。先按 `platform.android.glide_typing` 的先例只存本机，等别的平台也做同样的符号栏时再挪进共享偏好。
- **每个符号固定 dp 高度** — 不需要自定义 `onMeasure`；但九键块的高度随键盘高度偏好在 75%–130% 之间变，固定高度时一屏可见个数跟着变，有时露出半个键。

## Consequences

- **收益**：同样的高度放五个符号，默认多出 、：；……～@；用户可以把常用符号换进来，顺序即显示顺序。
- **代价与已知上限**：五个符号时每个约 32 dp 高，比原来窄，误触面积变小；左栏滚动时手指从符号键上纵向滑开会被 `ScrollView` 截走，这一下不上屏。设置不随账号同步，换设备要重设。

## Verification

`platforms/android/tests/keyboard/NineKeySidebarPolicySmoke.java` 锁住默认表、一屏五行的行高、解析与校验，以及本地设置按 `TEXT` 收值、不进同步；由 `bash platforms/android/check-host.sh` 运行。设置页的编译由 Gradle 的 `compileFullDebugJavaWithJavac` 覆盖，check-host 不编译 `home/`。

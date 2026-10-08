# Agent Note: Android 九键左侧符号栏与数字键面

Status: implemented

## Problem

Android 拼音九键左侧的符号栏原来是 ，。？ 三个键铺满三行键高，每个键约 54 dp 高，间距大到还能再放两个，也没法换成自己常用的符号（#5574）。笔画键盘用的是同一个外框，问题相同。

拼音九键按 123 切到的数字键面只是把网格键面换成 1–9：没有 0，没有小数点，左栏仍是 ，。？，右列最下面仍是 ！（#5590）。打版本号、价格、算式都要换别的输入法。

## Decision

### 左侧符号栏

- 拼音九键和笔画键盘的左栏是 `NineKeySymbolRail`（`platforms/android/java/app/msime/android/keyboard/NineKeySymbolRail.java`）：一个不显示滚动条、带上下渐隐边的 `ScrollView`，一屏显示 `NineKeySidebarPolicy.VISIBLE_ROWS`（5）个符号，多出来的上下滑动。每个符号键的高度在 `onMeasure` 里按栏高均分（`NineKeySidebarPolicy.rowHeight`），并扣掉键自己的上下外边距，所以键盘高度和行距偏好变了不用重建；符号不足五个时按实际个数均分整栏，不留空白。
- 符号表来自本地设置 `platform.android.nine_key_symbols`（`AndroidLocalSettings` 新增的 `TEXT` 类设置）：用空格分开的符号，存储前经 `NineKeySidebarPolicy.normalize` 规范成单空格连接、去重；空串、单个符号超过 4 个码位、含控制字符、超过 30 个符号或文本超过 200 个 UTF-16 单元一律不合规，写入时拒收，读到时整张表回到默认值。默认表是 `， 。 ？ 、 ： ； …… ～ @`：前三个和原来一样，！ 仍在右列最下面，不在左栏重复。
- 设置页「键盘 › 布局 › 九键左侧符号」用 `InputDialog` 编辑这张表，留空恢复默认（删掉这一项）。键盘每次出现时重读本地设置，`ImeLayoutRows.sidebarStale` 发现屏上的符号表和设置不同时，`render` 重建键行。
- 这个设置只在本机，不随账号同步：同步表在 `crates/client-core/src/account/settings_sync.rs` 的 `ANDROID_LOCAL_SETTINGS`，那里只有布尔、选项和整数三种类型。

### 拼音九键的数字键面

- 左栏是另一张符号表 `platform.android.nine_key_digit_symbols`，默认 `+ - * / = % ( ) : @ ， 。 ？ ！`：先给半角的四则运算和算式符号，！ 挪到第一屏之后（报告人原话「叹号移动到左侧红框区域，手指向上滑动时才能显示」）。设置页另有一行「九键数字键盘左侧符号」，规则与字母键面那张相同。
- 右列最下面从 ！ 换成小数点，字面上屏 `.`（全角模式下转全角），不经 `commitNineKeyPeriod`：那条路在中文标点模式下会变成「。」。
- 底行原来空格的位置分成左边的 `0`（`KeyboardActionRow.DesignSlot.ZERO`，权重 2.2，约一个网格键宽）和右边的空格（权重 1.8），合起来仍是原来空格的 4。`ImeBottomRow` 的排布签名带上「是不是九键数字行」，切换键面时重新排布。0 和网格数字一样经 `commitNineKeyLiteral` 字面上屏，键位 id 是 `Nine0`。

## Alternatives considered

- **只把三个键改小、固定显示五个，不滚动** — 改动最小；但用户要求能上下滑动和自定义，固定五个装不下自定义表，最后还是要滚动。
- **符号表放进共享偏好（`crates/client-core` 的 `Preferences`），走 Tauri 设置页** — 能随设置同步，其他平台日后也能读；但只有 Android 的九键有这条可滚动的符号栏，共享偏好要改 Rust 结构、默认值、同步和 Tauri 页面，其他平台却没有任何消费者。先按 `platform.android.glide_typing` 的先例只存本机，等别的平台也做同样的符号栏时再挪进共享偏好。
- **把 0 放进网格，做成 4×3 的数字键盘** — 和手机拨号盘一样，0 在 8 下面；但九键块固定三行键高，塞进第四行要么整块变高（切换键面时键盘高度跳动），要么每行变矮。报告人的截图也明确把 0 画在空格的左半边。
- **数字键面的右列保留 ！，小数点放进左栏** — 改动更少；但小数点在数字里和 0 一样常用，藏进可滚动的左栏要多一次寻找，而报告人点名要它在右下角。
- **每个符号固定 dp 高度** — 不需要自定义 `onMeasure`；但九键块的高度随键盘高度偏好在 75%–130% 之间变，固定高度时一屏可见个数跟着变，有时露出半个键。

## Consequences

- **收益**：同样的高度放五个符号，默认多出 、：；……～@；用户可以把常用符号换进来，顺序即显示顺序。数字键面能打出 0、小数点和四则运算符号，不用离开九键。
- **代价与已知上限**：五个符号时每个约 32 dp 高，比原来窄，误触面积变小；左栏滚动时手指从符号键上纵向滑开会被 `ScrollView` 截走，这一下不上屏。设置不随账号同步，换设备要重设。数字键面的空格变窄（约原来的 45%），右列的「拆分」在数字键面上仍然什么也不做。

## Verification

`platforms/android/tests/keyboard/NineKeySidebarPolicySmoke.java` 锁住两张默认表、一屏五行的行高、解析与校验，以及本地设置按 `TEXT` 收值、不进同步；`KeyboardActionRowSmoke` 锁住只有拼音九键的数字键面底行带 0、0 在空格左边、两者合起来等于原来的空格宽度；由 `bash platforms/android/check-host.sh` 运行。设置页的编译由 Gradle 的 `compileFullDebugJavaWithJavac` 覆盖，check-host 不编译 `home/`。

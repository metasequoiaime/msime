# Agent Note: 安卓剪贴板历史页进页不自动弹键盘

Status: implemented

## Problem

#10074：从键盘剪贴板面板点「搜索」打开应用里的「剪贴板历史」页（[在水杉应用里搜索本机剪贴板历史](2026-10-09-android-clipboard-search.md)）后，`ClipboardSearchPage.onContentReady` 第一次显示时把焦点给搜索框，并经 `WindowCompat.getInsetsController(...).show(ime())` 主动弹出键盘。键盘占掉下半屏，历史列表只露出第一条。报告者多数时候是来翻列表找一条复制，希望进页后不聚焦，点了搜索框再弹键盘。

## Decision

- **进页不聚焦、不弹键盘**。`ClipboardSearchPage` 删掉首次显示时的 `requestFocus()` 和 `show(ime())`，连同只为这段逻辑服务的 `Query.focused` 字段；`Query` 只保存查询文字。`HomeActivity` 是 `adjustResize` 不带 `stateVisible`，`minSdkVersion` 是 28，API 28 起触摸模式下系统不再把初始焦点隐式交给第一个 `EditText`，所以去掉主动聚焦后进页既没有光标也没有键盘，不需要另设吸走焦点的容器。
- **点搜索胶囊的任何地方都弹键盘**。`SearchPill` 原来点胶囊只 `field.requestFocus()`：点到输入框那一行字时系统会弹键盘，点到放大镜或左右留白时只有光标、没有键盘。进页不再自动弹键盘后，这条路径在剪贴板历史页上会直接被用户碰到，所以胶囊的点击改为 `focusField()`：先 `requestFocus()`，再 `InputMethodManager.showSoftInput(field, 0)`。`SearchPill` 的另外三个使用方也是点了就要打字的搜索框，一并受益：设置首页的搜索框（`KeyboardFragment` 的 `R.id.keyboard_search`，用得最多；它的清除按钮是另加进胶囊里的子视图，自己接点击，不走胶囊的 `focusField`）、内置词库详情的「搜索词条」、插件页社区分段的「搜索插件」。
- **压页时收起下面那一页的键盘**。从设置首页搜「剪贴板历史」点结果进来时，首页搜索框拿着焦点、键盘开着；`SettingsNavigator.push` 只 `transaction.hide(below)`，以前靠 `ClipboardSearchPage` 进页抢焦点让这块键盘接到它的搜索框上，去掉抢焦点后键盘会留在屏幕上、不连着任何输入框，正是 #10074 抱怨的盖住列表。所以 `push` 在藏起下面那一页之前调 `dismissKeyboard`：那一页里有视图拿着焦点时 `hideSoftInputFromWindow` 并 `clearFocus()`。放在导航层而不是剪贴板历史页里，是因为从首页搜索结果进任何一页都是同一个问题；新页面要键盘时（例如 `HomeActivity.openChatSecondary(true)` 压完页再 `focusInput()`）自己再弹，顺序在收起之后，不受影响。
- **剪贴板编辑页自己弹键盘**。从剪贴板历史页行尾点「编辑」进 `ClipboardEditPage` 也走 `push`。以前历史页进页就把键盘弹到搜索框上，压页后键盘接到编辑页的输入框，用户直接能改；现在历史页进页没有键盘，用户在搜索框打过字时 `push` 又先收起键盘，而编辑页到 `READY` 时只 `requestFocus()`，同一个窗口里压页系统不会替它弹键盘，结果是光标亮着却不能打字。所以编辑页渲染 `READY` 时在 `requestFocus()` 之后经 `WindowCompat.getInsetsController(...).show(ime())` 自己弹键盘，和 `ChatFragment.focusInput` 一样。从键盘长按记录点「编辑」经深链打开时本来就是这个效果，这一步只让两条入口一致。
- `check-host.sh` 守住：`ClipboardSearchPage` 里不再出现 `requestFocus()`、`Type.ime()`、`showSoftInput`；`SearchPill` 的点击绑定 `focusField`，而 `focusField` 调 `showSoftInput(field`；`SettingsNavigator` 在 `transaction.hide(below)` 前调 `dismissKeyboard(home, below)`，后者调 `hideSoftInputFromWindow`；`ClipboardEditPage` 的 `input.requestFocus()` 之后要经 insets controller `show(WindowInsetsCompat.Type.ime())`。

## Alternatives considered

- **只在剪贴板历史页单独给胶囊绑「聚焦并弹键盘」**，不动共享的 `SearchPill` — 改动只落在这一页；但另外两个使用方点放大镜同样只有光标没有键盘，是同一个缺陷，各页分别补会让三个搜索框的点按行为不一致。
- **保留自动聚焦，只是不弹键盘** — 光标停在搜索框里提示可以输入；但用户点搜索框时系统才弹键盘，聚焦本身没有省下任何一步，还让人以为键盘该出来了。
- **给固定导航容器设 `setFocusableInTouchMode(true)` 吸走初始焦点** — 防 API 26/27 在触摸模式下把初始焦点隐式交给第一个 `EditText`；`minSdkVersion` 是 28，不存在这个情况，加了反而让点导航条空白处抢走搜索框的焦点。

## Consequences

- **收益**：进剪贴板历史页能直接看到完整列表；要搜时点搜索框任何位置都会弹键盘，设置首页、词库详情和插件页的搜索框也一样。在首页搜索框里打字后点任何一条结果，键盘都随首页一起收起。
- **代价**：从键盘过来确实要搜的用户多点一下搜索框。从首页搜索结果返回后，首页搜索框不再拿着焦点，要接着改查询得再点一下。旋转、换深浅模式重建页面后同样不弹键盘，这和之前「重建后不再拉」的行为一致。
- [安卓剪贴板搜索框固定](2026-10-27-android-clipboard-search-fixed.md) 里重建时恢复「首次聚焦行为」的说法随之失效，已就地改为只恢复查询文字。

## Verification

`bash platforms/android/check-host.sh` 跑上面的守卫和全部 JVM 冒烟；`ClipboardSearchPage`、`SearchPill`、`SettingsNavigator` 和 `ClipboardEditPage` 只在 Gradle 构建里编译，`gradlew --project-dir platforms/android/gradle-app compileFullReleaseJavaWithJavac` 通过。没有在模拟器或真机上走：从键盘剪贴板面板点「搜索」和从设置首页搜「剪贴板历史」两条入口进页后键盘不弹、搜索框没有光标；后一条入口是在首页搜索框里打字、键盘开着时点结果，进页后键盘应已收起；点输入框和点放大镜都弹键盘并能实时筛选，设置首页、词库详情、插件页的搜索框点放大镜同样弹键盘；旋转、切深浅模式、生物认证弹窗结束后键盘不自动弹出。从剪贴板历史页（搜过和没搜过两种）点一条的「编辑」，进编辑页时键盘弹出、光标在文字末尾，能直接改。

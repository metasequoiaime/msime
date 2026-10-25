# Agent Note: iOS 十八键全拼、双拼与横滑精确字母

Status: implemented

## Problem

#8930 请求 18 键双拼，两个字母的键左右横滑取其中一个字母。Android 已经有十八键全拼（#9564）、十八键双拼（#9530）和横滑精确字母（#9593），共享引擎与 C ABI 也提供网格 4、5 和大写精确标记；iOS 只在统计白名单里补了 18 个 `Eighteen*` 键位，没有方案、设置、键面和横滑，Host API 也只在 Android 自动打开 18 键网格。移动端键盘界面要求 iOS、Android、HarmonyOS 一致，HarmonyOS 由另一项工作接入，本篇只负责 iOS。

## Decision

iOS 与 Android 一样提供两个入口，不新增双拼方案值：全拼 18 键是触屏方案 `eighteenKey`（共享文档 `eighteen_key`，引擎方案全拼，默认不启用，网格 4），排在启用顺序的最后；「中文键盘」按 Android 的顺序列出 26、18、14、9 键。双拼 18 键是 `touch_shuangpin_layout` 的第三档 `eighteen_key`，保留当前小鹤、自然码、微软或首道 profile，打开网格 5。Host API 的自动网格门控把两种 18 键从 Android 扩到 Android 和 iOS，新会话、会话重建和设置热加载都由共享偏好决定；鸿蒙的全拼 18 键由宿主自己按偏好调用 `msime_client_set_key_grid`，其他宿主继续保留但忽略这两个偏好。`TouchKeyboardLayout::EighteenKey`、`TouchKeyboardScheme::EighteenKey` 和 `TypingSource::EighteenKey` 的文档注释随之写明这几个宿主。桥接层的 `KeyGrid` 追加 4、5，重建会话时照旧重放。

键盘控制器用 `LetterKeyGrid` 统一表示「当前方案画 14 键还是 18 键」，两套键面各自一组行视图，共用组码入口、长按字母、分词键、读音行拼音条、三栏候选面板、英文和本地模式回落、iPad 不分体和手机底行。布局缓存记录正在画的网格，同一双拼方案在 14、18 键之间切换也会重排；组字期间收到的布局仍留到组字结束再应用。18 键的三排都是七个等宽键：第一排七个字母，第三排是分词、五个字母和 ⌫，都用 `fillEqually`；第二排六个键，两侧 `layoutMargins` 各为 `(行宽 + 键距) / 14`，即半个键加半个键距。行内六个键之间已经有五个键距，两侧只补第七个键那一份键距，不再另外扣一次，这正是 Android #9593 修过的重复扣键距；键距改变时重算。用边距而不是两侧占位视图，是为了让空白里的触摸按现有的 `KeyGapRouting` 归最外侧的键。两个字母的键面清零内边距、只缩小不换行，对应 Android 的 12–22 自适应字号。

横滑只在双拼 18 键的两个字母的键上生效，规则与 Android 的 `EighteenKeySwipePolicy` 相同：横向位移超过 24pt 且大于纵向 1.5 倍，左滑取左字母、右滑取右字母。每个这样的键挂一个 `EighteenKeySwipeGestureRecognizer`：越过阈值前停在 `.possible`，按键照常收到触摸；越过阈值才开始识别并取消按键这次触摸。抬手时还在 `.possible` 就按抬手位置再判一次，过了阈值直接识别为选中的字母，没过才是普通点按，与 Android 在 `ACTION_UP` 上也走一次 `Gesture.move` 相同：快速一甩时 UIKit 可能只送一次没过阈值的移动，抬手那一点才过阈值。识别器因此打开 `delaysTouchesEnded`，按键的抬手等识别器判完才送达，识别了就换成取消，不会既输入字母又点一次整组；判定在抬手当场完成，普通点按不会多等。拖动时键面显示选中的字母，松手把它以大写交给 `gridKey`，引擎只在网格 5 把大写当作同一组里的精确小写字母。回到起点、同一键上第二根手指和系统取消都不输入，反向拖过另一侧可以重选；长按先识别时横滑不再开始。识别器总是挂上，接手那一刻由委托按当前方案决定，全拼 18 键因此只点按，切换方案也不必重建按键。这些键不再参与键盘根视图上的横滑光标。

统计按键记在 `Eighteen*` 上，点按、长按和横滑同一个 id；来源全拼是「全拼 18 键」，双拼仍记各自的方案。账号同步把全拼 18 键上传为 `input.schema=quanpin`、`platform.ios.keyboard_layout=eighteen_key`，旧版本读不懂这个布局时按九键开关落成全拼 26 键；没有布局键的旧文档回写全拼时保留本机的 18 键，与 14 键相同。双拼布局与 14 键一样只存在本机共享偏好，不进账号同步。

### 历史笔记检索

- [Android 十八键双拼](2026-10-24-android-eighteen-key-shuangpin.md)：部分重叠，复用分组、网格 5、分号与 L 共键和默认值；原文写 iOS 不启用 18 键网格，已就地改为 iOS 有三档。
- [十八键双拼横滑限定字母](2026-10-24-eighteen-key-exact-swipe.md)：部分重叠，沿用大写精确标记、阈值和取消规则；iOS 用键上的手势识别器取代 Android 的触摸监听，双方互链。
- [Android 十八键全拼入口](2026-10-24-android-eighteen-key-quanpin.md)：部分重叠，复用网格 4、选项顺序和等宽几何；原文「只在 Android 自动启用」已就地改为 Android 和 iOS。
- [十八键全拼共用字母归组引擎](2026-10-24-eighteen-key-quanpin-engine.md)：部分重叠，算法与 C ABI 不变。
- [iOS 双拼 14 键](2026-10-24-ios-fourteen-key-shuangpin.md)：部分重叠，沿用 `touch_shuangpin_layout`、控制器自存已应用布局、延迟热加载和 Host API 门控，双方互链。
- [全拼 14 键](2026-10-10-fourteen-key.md)：部分重叠，复用组码、确定字母和候选面板边界。
- 检索活跃笔记中的 `十八键`、`18 键`、`eighteen`、`横滑` 与 `8930`，Windows 面板与其他命中不负责 iOS 键面。

## Alternatives considered

- **第二排两侧放占位视图，键宽约束到第一排的键**：等宽由约束直接保证，不依赖行宽；但占位视图里的触摸落在 stack view 上，`KeyGapRouting` 只按半个键距去找键，半个键宽的空白会吞掉按键，所以用边距。
- **用 `UIControl` 的拖动事件或在按钮上自己处理触摸**：少一个类型，但按钮在拖出 24pt 后仍算在键内，抬手照样发出整组点按，要另外记状态去压掉这次点按；手势识别器开始识别时由 UIKit 取消按钮的触摸，长按和横滑的互斥也由识别器的默认规则给出。
- **抬手时识别但保持 `delaysTouchesEnded = false`**：按键抬手不经识别器排队；但 UIKit 的文档只保证推迟送达的抬手在识别后改为取消，不推迟时同一事件里按键是否还会收到抬手、发出整组点按没有契约可依，这一下可能既输入精确字母又输入整组，所以打开推迟。
- **只做双拼 18 键**：issue 只要求双拼，但 Android 两个入口都有，移动端键盘要求三端一致，全拼 18 键的引擎与键面又完全复用，所以一起接入。
- **只在 iOS 宿主显式调用 `set_key_grid`，不改 Host API 门控**：可以不碰共享代码，但会话由词库维护重建或别的进程新建时会回到 26 键，iOS 14 键双拼已经因此扩展过门控，这里沿用。

## Consequences

- iOS 有了与 Android 一致的 18 键全拼和双拼，双拼可以横滑限定字母；默认启用列表和默认布局不变。
- `touch_shuangpin_layout=eighteen_key` 与 `touch_keyboard_layout=eighteen_key` 在 iOS 上开始生效：从 Android 同步过来、或旧版本留下的这两个值，在 iOS 新版本上会画 18 键。
- 18 键第二排的边距在布局后按行宽计算，第一次布局前取第一排的宽度；键距、单手或浮动改变宽度时随 `viewDidLayoutSubviews` 重算。
- 横滑的单元测试用脚本化的触摸驱动识别器和处理函数，不经过真实的系统事件分发，抬手才过阈值时按键的点按被扣下这一点只能核对 `delaysTouchesEnded` 已打开；真机或模拟器系统键盘里的手感、与系统手势的冲突仍需实际设备确认。
- HarmonyOS 的 18 键与横滑由另一项工作接入，见 [HarmonyOS 双拼 14/18 键](2026-10-25-harmony-fourteen-key-shuangpin.md)。

## Verification

构建与测试在远端 Mac Studio（Xcode 27.0，iPhone 17 Pro 模拟器）经 `rbuild` 执行，基点为 `develop` 的 `dccc9e5e50`。

- Rust：`cargo test -p msime-host-api eighteen_key` 单元 6 项与 `tests/eighteen_key.rs` 2 项通过，其中两项门控用例改为要求 Android 和 iOS 都打开网格 4、5，其他宿主仍为 26 键；`cargo test -p msime-client-services ios_settings` 22 项通过，含新增的 18 键往返；三个相关 crate 的全部 target 严格 Clippy 通过。
- iOS 单元测试：`EighteenKeyKeyboardTests` 11 项全部通过，覆盖键表与统计 id、横滑阈值、全拼和四种双拼方案的桥接（整组同时有 bi 和 ni，`N`、`I` 精确标记只留 ni，会话重建后仍在）、320／390／414pt 和键距 3、6 时 18 个字母键与两端键等宽、第二排居中、iPad 排布、横滑选字母、回到起点和取消不输入、长按、14／18 键热加载与微软分号、英文和数字页往返。横滑首轮失败于测试夹具：没有真实事件分发时 UIKit 不会把识别器放回 `.possible`，改为每次用同样配置的新识别器跑脚本化触摸后通过，产品代码未改。同次运行 `FourteenKeyKeyboardTests` 15 项、`NineKeyKeyboardTests` 116 项（1 项既有跳过）、`LanguageSchemesTests`、`SchemeDocumentSettingsTests`、`KeyboardFormFactorTests`、`CompositionBoundaryTests`、`TypingStatisticsTests`、`CandidateTranslationTests` 全部通过。
- 九张键面截图已逐一查看：手机三种宽度、两种键距下各排等宽，`WE`、`XC`、`BN` 单行显示；双拼横滑后读音行只给 ni，iPad 为完整的七列网格和手机底行。
- `bash platforms/ios/build-app.sh <资源目录> simulator` 构建 `MSIMEApp` 与内嵌键盘扩展通过，包含设置页与预览的 SwiftUI 改动。工程文件在远端用与 CI 同版本的 xcodegen 2.46.0 重新生成；生成前确认未改动的树重新生成与已提交的 `project.pbxproj` 完全一致。
- `scripts/test-eighteen-key-table.py` 新增 iOS 键表核对后通过，`scripts/test-fourteen-key-table.py` 通过。

评审后补上抬手判定：`EighteenKeyKeyboardTests` 新增抬手才过阈值的用例（只送一次 15pt 的移动、抬手在 ±30pt），改动前这一用例失败：两次抬手都没有识别，读音行没有 ni（真实分发下按键会各补一次整组点按）；改动后两键识别为 `N`、`I`、只剩 ni，抬手仍在阈值内时不识别、交给按键。同次 `EighteenKeyKeyboardTests` 12 项、`FourteenKeyKeyboardTests` 15 项、`TypingStatisticsTests` 21 项全部通过。

尚未验证：模拟器系统键盘入口与实体设备上的真实横滑手感，以及与系统边缘手势的冲突；`MSIMEClientUITests` 里设置页四档的界面用例已更新，但 CI 目前不运行这一套，本次也未运行。

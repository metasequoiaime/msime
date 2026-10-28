# Agent Note: Android 26 键双拼上报按下点与键位几何

Status: implemented

## Problem

[双拼触点纠错](2026-10-27-shuangpin-touch-correction.md)在引擎和 C ABI 上加了一条旁路：宿主注册键位几何（`msime_client_set_key_geometry`），每键带上按下点（`msime_client_touch_character`），引擎按落点给双拼纠错计价。Android 宿主还没接：26 键的字母键照旧走 `msime_client_character`，引擎拿不到落点，纠错整句仍按平面表沉到表尾。

接线要守住几件事：坐标必须是没有经过键距挪动、不受按下缩放影响的键区坐标，而且和几何同一个坐标系；落点只在 26 键双拼的字母键和微软双拼的 `;` 上送，辅助码 Shift、14/18 键、九键、英文直出、标点和实体键盘照旧；被手势吞掉的按下不能把落点留给后面的点击；宿主不算距离。

## Decision

- **采点在按键区，按键存**：`KeyboardKeyArea` 在 `ACTION_DOWN` / `ACTION_POINTER_DOWN` 时，把按下点记给它落在的键，落在空隙里就记给拥有空隙的那个键（`route` 本来就算出了这个键）。坐标取原始事件的按键区坐标，也就是键距挪动之前的那一个。按下点按键存（`TouchPointPolicy`，最多 10 个键），不按指针存：点击在抬起时才发生，那时已经不知道是哪根手指；多指连按时每个键各自留着自己那一下。只记点击时会取走按下点的键（`shuangpinLetterKey` 造的 `ShuangpinHintButton`：26 个字母和微软的 `;`），数字行、Shift、退格的按下不记。槽满时覆盖按下最早的那个键：最早一版按轮转游标覆盖，空位被重新用上时游标不动，快速两指连按里按着的字母会被后一根手指挤掉、悄悄退回 `msime_client_character`（评审用固定种子的连按序列复现，`TouchPointPolicySmoke` 钉住）。
- **取用即清**：字母键的点击回调把自己交给 `MSIMEInputService.type(char, View)`，它一开头就取出并清掉这个键的按下点，不论这一键后面走哪条路。只有走到最后送给引擎的那一步、`TouchPointPolicy.usesTouch` 成立（26 键字母层、`scheme` 为双拼、`key_grid` 为 `none`、不在专用英文和本地模式、小写字母或 `;`、没有 Shift、当前会话注册过几何）时才改调 `msime_client_touch_character`。辅助码的大写分支、英文直出、韩文和标点都在它前面返回或另走，行为不变。
- **丢弃**：整次触摸 `ACTION_CANCEL`、滑行接管、键行重建时全部丢掉；滑出符号、长按选项吞掉的那一下，以及候选可见时抬起离键太远（超出 touch slop）、宿主替它补的那次 `performClick`，丢掉这个键的；按下到点击超过 2 秒的作废，免得一次没点成的按下被很久以后的无障碍点击捡走。无障碍点击本身没有按下，取不到落点。
- **几何**：`ImeGlideTyping.measureKeys` 抽成共享的 `LetterKeyMeasure`，滑行和这里都用它；它另外量出显示中的微软 `;` 键和分离键盘右半边重复的字母键。`ImeTouchPoints` 挂在按键区的 `OnLayoutChangeListener` 上，每次布局后量一遍：在 26 键字母层、26 个字母键都量到、和上次注册的矩形不同或会话换了时，换一个在同一服务实例内只增不减的代次号注册；服务被销毁后新实例可以从 1 开始。点击时发现会话换过而还没注册，先补注册。量的是 `offsetDescendantRectToMyCoords` 给出的布局位置，和按下点同一个坐标系，按下动画的缩放不算在内。
- **调试导出**：开发者选项「显示调试信息」开着时，把注册给引擎的 JSON 原样写到 `<filesDir>/debug/key-geometry.json`。几何只有键的位置和大小；按下点从不写出、不进日志。设备验收经 instrumentation 状态把它交给 `smoke.sh`，存成 `target/android/device-test/key-geometry/<名字>.json`。
- **JNI**：`setKeyGeometryRaw`、`touchCharacterRaw` 紧挨 `characterRaw`；超长或空引用的几何不复制，交给引擎按「长度非零却是空指针」拒绝（长度 0 在协议里是清除）。`verify-native.sh` 的导出清单加上这两对符号。

## Alternatives considered

- **按指针 id 存按下点，点击时找对应指针**：按键区确实是按指针看事件的，方案原文也这样写。但点击回调里没有指针 id，要把它从按键区传到 `Button` 的点击得另造一条通道；而「一个键一次只会被一根手指点成」在多指连按里也成立，按键存一样准确，接线少一层。
- **在键自己的触摸监听里取点**：键内坐标里，落在键距的按下已经被挪到键内 1px，落点信息丢了，方案明确否掉。
- **每键随落点一起送几何**：宿主不用记代次，但每键多一次 JSON 解析和归一，`KeystrokeLatencyTests` 卡着每键开销，协议也规定只在布局变化时注册。
- **验收跨服务比较几何代次**：直接要求微软方案的代次大于小鹤方案的代次，可以在服务未重建时排除旧导出。但 Android 禁用再启用输入法可能重建服务，旧编号 17 之后的新实例可以合法注册 1、2、3；旧验收会把实际存在的 `;` 几何误报为未导出。验收在重启前删除旧文件，改为核对重新写出的文件、正数代次、对应方案的 `;` 状态和全部按键的实测矩形，不要求代次跨实例递增。
- **调试导出另开一个开发者开关**：语义更直接，但要在开发者页加一行、本地设置加一个键；导出只给开发者拿真机几何用，跟着已有的「显示调试信息」足够。

## Consequences

- **收益**：Android 26 键双拼的点击带上真实落点，引擎的落点计价第一次在真机路径上生效。模拟器上同一串 `mwgfxi`（小鹤，想打「没关系」，r 按成 f）：经无障碍点击（没有落点）时「没关系」不在候选条可见范围内，f 那一键按在偏向 r 的左上角时，它以纠错行（注释 `mwgfxi`）出现在候选条上。
- **代价**：每次按下多一次按键区内的记录（数组写，无分配）；每次按键区布局多量一遍 27 个键；点击时多一次策略判断。几何只在布局真的变了时才过 JNI。
- **不变**：桌面、实体键盘、九键、14/18 键、全拼和五笔一行没改调用；没有落点或不满足条件时仍调 `msime_client_character`，引擎侧保证代次不符时逐字节相同。滑行的测量结果与抽出前相同（多加的可见性判断对字母键恒成立）。
- **验证**：`check-host.sh` 通过，含 `TouchPointPolicySmoke`（路由条件、取用即清、过期、时钟倒流、多指、槽位淘汰、按身份匹配、抬起离键判断、几何 JSON 与拒绝）和一处钉住「`NativeClient.touchCharacter` 只在 `type(char, View)` 的最后一步调用」的检查；NDK 对 `client_jni.cpp` 的语法检查通过。APK 在 Mac Studio 上经 `rbuild` 构建，`verify-native.sh` 通过；专用 API 35 模拟器（`emulator-5580`）上 `MicrosoftShuangpinDeviceSmoke -e shuangpin-touch true` 通过：导出的几何与无障碍树里 26 个键（和微软的 `;`）位置一致，键的边角和键距里的真实触摸组字是命中的键，纠错行随落点出现、前后两遍无障碍点击都不出现，微软 `;` 带落点仍是韵母。`DeviceSmoke`、`MicrosoftShuangpinDeviceSmoke`、`LetterHintPunctuationDeviceSmoke`（`letter-hold`、`letter-swipe`）、`CandidatePanelDeviceSmoke -e strip-partial-selection` 在同一台模拟器上通过。
- **验收新鲜度**：两次重启前都删除旧几何文件，切换微软方案时先删除再发布配置，避免服务抢先完成新注册后删掉它的导出，等待重新导出的有效几何。旧套件在 API 35 上通过旋转并回到竖屏累积到代次 17，再显式禁用并重建服务后，实际导出了含 `;` 的 1、2、3，却因跨服务比较代次而报错；轮转和强制重建只用于诊断，不留在常规套件里。同一受控场景修复后完整通过，导出的微软几何代次为 3。修复只改验收的新鲜度判断，26 键数量、重复键、位置和尺寸、边角与键距、纠错候选、微软 `;` 的输入断言均保留。
- **没覆盖**：真机盲打、分离键盘与横屏的几何（重复键的导出只有 JVM 冒烟覆盖），以及滑行输入在设备上的回归（只改了测量的出处）。计价参数已经用这里导出的模拟器几何重扫，见[双拼触点纠错](2026-10-27-shuangpin-touch-correction.md)的 PR1b 一节；真机几何还没有导出过。

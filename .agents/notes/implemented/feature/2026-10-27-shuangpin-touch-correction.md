# Agent Note: 双拼触点旁路协议与按落点计价的纠错

Status: implemented

## Problem

#10018 要让移动端 26 键双拼在盲打时把按错的邻键纠回来。[双拼纠错整句](2026-10-09-shuangpin-typo-sentence.md)的平面表只看按到了哪个键：一个键换成任何一个 QWERTY 邻键都付同样的 20，它不知道手指落在键的哪一边，所以按在 f 和 r 交界处的一键与按在 f 正中的一键得到同样的改正，真正该改的那个在 96 个键的规划预算里也排不到前面。宿主手里有落点，但没有任何路径把它交给引擎。

[双拼触点评测](../testing/2026-10-27-shuangpin-touch-eval.md)量出了平面表的上限：σ=0.25 的 noisy 整句集里，单错的用例 13% 能在候选表里找到原句，第一页却只有 0.3%。按错之后仍找得到原句的 1105 次里，965 次排在整张表的最后一位，因为 `ordering::runner_up_order` 把第三条以后的同宽整句读法挪到表尾，纠错整句也是其中一行。方案原本假定 runtime 不用改，这个前提不成立。

约束：桌面宿主和实体键盘不送落点，它们的结果必须逐字节不变（G0）；纠错整句这一期仍不抢首选。

## Decision

触屏宿主经一条新的旁路把键位几何和每键的按下点交给引擎；双拼纠错整句在有落点时按落点给每个变体计价、按价全局规划；落点支持的纠错整句留在引擎给的座位上，不再被挪到表尾。没有落点时每一步都走原来的代码，桌面逐字节不变。

### 协议

- **C ABI**（纯增量，`msime_client_abi_version` 仍是 3）：
  - `msime_client_set_key_geometry(session, json, length)`：`{"id":u32,"keys":[[cx,cy,w,h]×26，按 a..z],"semicolon":[…]|null,"duplicates":[["g",cx,cy,w,h],…]}`，形状沿用滑行请求；拒绝未知键，上限 16 KiB；数值有限、宽高为正、26 个字母键齐全、归一后的键心离原点不超过一万个键，否则整份拒绝、原几何不变。最后一条是因为原始数值都有限时，归一（键宽 1e-3、坐标 1e36）或距离平方（键心 ±1e30）仍会溢出成无穷，两个无穷相减得 NaN，罚分和邻键判断随之失效，平面表的邻键会被丢掉；真实键盘用屏幕坐标也只有几百个键宽。落点归一后超出同一范围时这一键不带触点。`id` 是宿主分配的非零代次；空串或 `id` 为 0 清除。只在布局变化时调用。
  - `msime_client_touch_character(session, ascii, shift, geometry_id, x, y)`：先按当前几何把 (x, y) 换成触点交给 runtime，再照 `msime_client_character` 原路派发，派发完不论结果都清掉。代次不符、坐标不是有限数、当前不是 26 键双拼时，回应与 `msime_client_character` 逐字节相同（host-api 有测试逐字节对比）。`ascii` 仍以宿主的命中测试为准。
  - 头文件写明：桌面宿主和实体键盘永远不调用这两个函数。
- **Rust 类型**（`crates/engine/src/touch/mod.rs`）：
  - `KeyGeometry` 按 26 个字母键键宽、键高的中位数归一，保存 a..z、`;` 和重复键的键心；`KeyGeometry::from_json` 是宿主 JSON 的唯一解析。
  - `KeyTouch { dx, dy, copy }`：相对被按键中心的偏移，单位 1/20 键，在入口量化并钳到 ±30；`copy` 是这个字母的第几个实例，分离键盘上两边各有一个 G、V 时才不是 0。打分和缓存签名用同一个量化值。
- **runtime**：`InputEngine` 加两个默认空实现的 setter（`set_key_touch`、`set_key_geometry`）。`Runtime` 自己持有几何（`Arc`），在 `replace_engine_with_touch_layout` 里紧挨 `set_rescoring_context` 重新下发，宿主空闲时重建引擎后触点照样有效。`Runtime::key_touch` 按代次和坐标算出触点，host-api 和评测都用它。没有新增 `Action` 变体。
- **引擎内的生命周期**：`session::Session::set_key_touch` 把触点放进 `InputSession.pending_touch`。`character` 先把它取出来，只有走到 `dispatch_character` 末尾、这一键是小写字母或 `;` 时才在 `handle_key` 之前交给方案（`handle_key` 会同步刷新候选）；其余路径（改字、光标处插入、本地模式、网址、辅助码前导键、九宫格数字）都丢掉它。`command`、`select`、`grid_key`、`glide`、`punctuation` 一开头清掉它，所以被 `candidate_shortcut` 之类改道的按键不会把触点留给后面的字母。其余会改动组字或改道按键的公开入口（`select_edge`、`finish`、`set_caret`、`candidate_key`、`conversion_left_from`、`pin`、`remove`、`fix_position`、`clear_position`、九宫格的两个入口、`expand_initial_candidates`、`reset_context`、`switch_scheme`、`set_key_grid`、`set_dedicated_english`、`set_key_geometry`）同样一开头清掉它；只改配置的 setter 不碰它。runtime 派发 `Action::Character` 时在 `engine.character` 之前不调这些入口，宿主的「设触点、派发、清触点」照样成立。
- **`ShuangpinScheme.touches`** 与 `raw` 逐字节对齐，一个触点都没有时保持为空，不为全 `None` 占位，桌面每键不多一次分配。`'`、大写字母（辅助码）和标记不带触点；退格截掉；`set_raw_input` 先认新旧串的公共后缀、再在剩下的部分里认公共前缀，这两段保留触点，中间新出现的字节没有触点。先认后缀是因为部分选词吃掉前缀、撤销补回前缀是确定的改动：叠词 `xpxp`（谢谢）选走第一个字剩下 `xp`，先认前缀会让它拿到刚选走那两键的触点，按别的按键的落点给剩下的音节计价。光标处删改相同的相邻字母（`hh`）分不清是哪一个，触点可能互换，长度和其余位置仍对得上。几何的布局或代次变化（包括注册和清除）会清掉已记录触点和待用触点，保留 `raw`；没有所属布局的旧偏移不能交给新几何解释。相同几何重复下发保留触点，避免会话重复绑定方案时丢失当前输入的落点。
- **`QueryRequest`** 新增 `key_touches`、`key_geometry`、`touch_tuning`。只有注册了几何、至少一个触点、输入里没有 `'`、句中辅助码没有改写输入串时，`key_touches` 才非空；否则必须是空 Vec 而不是全 `None`。光标前缀查询、撤回选词后重打和模糊音的替代拼写都拿不到触点，按平面表算。
- **`ShuangpinDictionary`** 在每次查询前收下请求的触点（`set_key_touches`，`query`、`insert_online_words`、`expand_initial_candidates` 都先调它）。只对带触点那串输入的前缀用触点；系列缓存、整句缓存、辅助码缓存的键追加这一段的触点签名（几何代次、几何内容指纹 `KeyGeometry::fingerprint` 加逐位量化触点），没有触点时签名为空，键与以前相同。只带代次不够：方案层把同代次、内容不同的几何当新几何处理，同样的量化触点在挪过键的布局下计价不同，缓存却会命中旧布局算出的行。计价参数变了就清掉整句答案。

### 计价

- 罚分：`pen(j) = clamp(B + s·(d²(P, c_j) − d²(P, c_k)), P_MIN, P_MAX)`。P 是还原出的落点，c_k 是被按的那个实例，c_j 取候选键离落点最近的实例；`d² = (Δx/σx)² + ((Δy − b_y)/σy)²`。单位就是词网格边的自然对数分，纠错边从对数概率里扣它，`decode_typo_on_graph_with_policy` 不改。
- 变体集（`collect_priced_shuangpin_typo_edges`）：平面表原有的邻键永不丢弃，只按罚分排；分离键盘上平面表说相邻、几何上相距超过 3 个键的那几对（T 和 Y、G 和 H）不算邻键；七份参考布局里不分离的那几份，平面表邻键最远相距 2.25 个键，分离布局里分在两半的相距 8 个键以上。平面表里没有的键（真实布局下的斜邻键、微软的 `;`）罚分不超过 `P_CUT` 时加进来。对调、没有触点的位置照旧付 20。同一个音节由几种改法得到时取最低价。解码用每个方案一张 27×27 的解码表，和平面变体表放在按方案地址缓存的同一个条目里，一个方案只泄漏一份、只有一把锁。每个位置的计价变体（`priced_shuangpin_variants`）每次查询只算一次，规划纠错边和判断落点是否支持纠错整句共用。
- 规划（`quanpin::typo_edges::collect_priced_span_typo_edges`）：所有位置的所有变体按罚分全局升序排，同价时弱位置优先，再生成跨度键，直到 96 个键的预算。全拼的 `plan_keys_with` 和 `collect_span_typo_edges` 一行没改。
- 不做：一次只换一个音节、只产出一条纠错整句，这是召回上限。

### 座位

- 纠错整句的分数（已扣罚分）不比字面整句低出 `show_margin`，而且它改动的每一键在落点下改过去的价都不超过 `show_penalty`（`touch_supports_correction` 按纠错读音逐音节对回两键码，在规划纠错边时算好的那一份计价变体里查这一键，不重新计价）时，这一行标上 `WordItem::touch_supported`。快照带一列与候选同步排列的 `candidate_touch_supported`（一行都没有时为空），runtime 调 `ordering::runner_up_order_with(scheme, rows, &candidate_touch_supported)`：标上的纠错行不算整句读法，留在引擎给的座位上（整句块之后），前三条字面读法照旧，其余读法照旧挪到表尾。没标上的纠错行和没有触点的表一样沉底，网页引擎和桌面不受影响。按行而不是按整张表：开了模糊音时，替代拼写按平面表查出的纠错整句也带 `corrected_from`，却没有触点、没过闸；整张表一个标志会让它跟着主纠错行一起逃过沉底。
- 纠错整句仍不抢首选：`typo_sentence_seat`、`keep_typo_sentence_off_first_seat`、`corrections_may_lead` 都没动，旧评测的 `top1_changed` 为 0，只能说明原本排第一的金标准没有降位；实际首选是否变化由单独比较候选文本的 `--assert-top1-unchanged` 检查。坐首选的规则后来另立一篇，见[双拼触点纠错坐首选的规则](2026-10-29-shuangpin-touch-first-seat.md)。

### 参数

| 参数 | 默认值 | 含义 |
|---|---|---|
| `base`（B） | 8 | 落点在两键正中间时的罚分 |
| `scale`（s） | 12 | 距离平方之差每一单位加多少罚分；键心按下、换成同排邻键约付 B + s |
| `sigma_x`、`sigma_y` | 1、1 | 横向、纵向距离的单位（键） |
| `bias_y`（b_y） | 0 | 瞄准点往下挪多少键高 |
| `min_penalty`、`max_penalty` | 4、28 | 罚分的上下限 |
| `cut`（P_CUT） | 16 | 平面表外的键超过这个价就不加 |
| `show_margin` | 6 | 纠错整句的分数不比字面整句低出这么多，才可能留在座位上 |
| `show_penalty` | 14 | 改动的每一键在落点下的价都不超过它，才可能留在座位上 |

PR1a 在构造的参考布局上调出暂定值（`show_margin` 4），PR1b 换成三端宿主在模拟器上导出的几何后按同一套流程重扫，只把 `show_margin` 从 4 提到 6，其余不变，见下面「宿主接入与导出几何上的重扫」。`touch::tests::the_default_tuning_is_the_swept_one` 钉住这组默认值。调参覆盖只能经 Rust 侧的 `host::Session::set_touch_tuning`（`convert_eval --touch-tuning`），宿主经 FFI 设不了。

### 扫参（PR1a，构造的参考布局）

调参用 sentences-v1、sentences-particles-v1、sentences-v2（455 条能编码的用例），留出 sentences-neutral-v1（1064 条）做核对；资源、模型和种子与基线相同，几何是 `android-portrait` 和它的 `-microsoft` 版。数字都是金标准在第一页（`top9`）的比例和 clean 集被往后挤的条数。

1. **只调座位的分数差**（B=12、s=8，小鹤，σ=0.25、b=0）：平面表 noisy top9 0.011、found 0.149。`show_margin` 取 −1000（等于全部沉底）、0、10、+1000（只要带触点就留座位）时，noisy top9 是 0.011、0.112、0.200、0.248，clean 被挤的条数是 0、1、1、29（/455）。纠错整句一律留座位时的 29 条就是上面那个备选方案被否掉的原因。
2. **B × s × 分数差**（B∈{6,9,12,15}、s∈{4,8,12,16}、分数差∈{5,6,8,10,15,20}，同一条件）：B 越低召回越高、被挤的也越多；s 在 8–16 之间差别不大。前沿是 B=6、s=12、分数差 10（top9 0.262，挤 3 条）和 B=6、s=16、分数差 8（0.242，挤 1 条）。`cut`（12/16/20）、`max_penalty`（24/28/40）、`min_penalty`（0/4/8）在这个条件下 top9 都在 0.224–0.229 之间；`sigma_y` 取 0.8 或 1.25 比 1 各低或高不到一个点，没有采用。
3. **留出集核对**（neutral，小鹤和微软，σ∈{0.15,0.25,0.35}×b∈{0,0.1}，共 12768 条 clean）：上面三组的 clean 被挤比例是 0.50%–0.55%，压线；按方案和音节、错键数的每个分桶，top1/top3/top9/found/mrr 都不低于平面表。被挤的金标准全都本来就在第 65 名以后（字面整句和金标准差得很远，模型给某条纠错整句的分接近字面整句），但门槛按条数算。σ=0.35 是被挤最多的条件：clean 集要求每键都落在本键里，σ 大时落点贴着键边，改到邻键的价接近 B。
4. **加上每键的价的闸 `show_penalty`**（调参子集，小鹤和微软，σ∈{0.25,0.35}×b∈{0,0.1}，3640 条 clean）：B=6、s=16、分数差 8 时被挤 1.13%、noisy top9 0.182；加 `show_penalty` 14、10 后是 0.82%/0.172、0.69%/0.136。B=8、s=12、分数差 4 时，不加闸 0.44%/0.125，`show_penalty` 14 是 0.19%/0.117，10 是 0.05%/0.077；分数差 0、`show_penalty` 14 是 0.14%/0.090；B=10、分数差 4、`show_penalty` 14 是 0.08%/0.087。扫参前的起始值（B=12、s=8、分数差 0、不加闸）是 0.25%/0.085。选 B=8、s=12、分数差 4、`show_penalty` 14：在最难的 σ=0.35 条件下也给门槛留了余量，召回比起始值高出约四成。

## 宿主接入与导出几何上的重扫（PR1b）

### 三端宿主

三端只在 26 键双拼的字母键和微软 `;` 上取按下点，经 `msime_client_touch_character` 送给引擎；几何只在布局真的变了或会话换了时经 `msime_client_set_key_geometry` 注册一次，代次只增不减、跳过 0。辅助码 Shift、本地模式、专用英文、14/18 键、九键、无障碍点击和实体键盘照旧走 `msime_client_character`。宿主不算距离。

- **Android**（[接线笔记](2026-10-28-android-shuangpin-touch-host.md)）：`KeyboardKeyArea` 在 `ACTION_DOWN` / `ACTION_POINTER_DOWN` 时把键距挪动之前的按键区坐标记给命中的键（落在键距里记给拥有空隙的键），按键存在 `TouchPointPolicy`，只记 `ShuangpinHintButton`（26 个字母和微软 `;`），槽满时让出最早的按下；`type(char, View)` 一开头取走，最后一步才改调 `NativeClient.touchCharacter`。几何由抽出来的 `LetterKeyMeasure` 量（滑行共用），`ImeTouchPoints` 挂在按键区的布局监听上注册。调试导出：「显示调试信息」开着时把注册的 JSON 原样写到 `<filesDir>/debug/key-geometry.json`。
- **iOS**：`KeyboardKeyButton.beginTracking` 记 `touch.location(in: keyArea)`（`KeyAreaStackView` 坐标，不受按下缩放影响，键距里归给这个键的触摸也是真实位置），取消、角标、滑出松开、离开窗口时清掉；`makeTouchKey` 经 `UIAction.sender` 把按钮交给 `handleCharacter(_:touchDown:)`，只在原来那个落点由 `TouchPointPolicy.sendsTouch` 判断（26 键双拼、小写字母或 `;`、有按下点、没开 VoiceOver）。几何取 `glideLetterKeys` 和 `microsoftFinalKey` 的 `layoutFrame`，在带落点的那一键之前比内容，变了才注册。调试导出：诊断日志开着时写到 App Group 的 `MSIME/debug/key-geometry.json`。
- **HarmonyOS**：`flexibleKey` 的 onTouch 在 Down 时记下按下点在被按键里的相对位置，点击时按这个键当前的矩形还原（工具栏在组字时出现会把整排键往下推，按根区域坐标记会差一个工具栏高），只在 `touchDesign()` 为真时记；`KeyboardSession.pressTouch` 与 `press` 共用 `pressKey`，`press` 的签名和实体键盘的调用不变。几何每键现算（不缓存脏标记），`KeyGeometryRegistry` 比字符串、变了才注册。调试导出：注册成功时打一行 hilog `key geometry {…}`。
- **JNI / NAPI / Swift 桥**：Android 的 `setKeyGeometryRaw`、`touchCharacterRaw` 登记进 `verify-native.sh`；鸿蒙的 `client_napi.cpp` 读几何 id 时按 int64 读再查 u32 范围，坐标超出 float 范围或不是有限数时交 NaN，由引擎退回 `character` 的回应；iOS 用 `@_silgen_name` 声明两个函数。

### 键行重建后的注册时序

Android 的几何注册不能只依赖外层 `letterArea` 的 `OnLayoutChangeListener`：从九键切到 26 键双拼时，外层区域的边界可能不变，只替换了它的子键，因此不会再次触发布局监听。`ImeLetterRows` 在键行重建完成以及可复用键行切换输入框后，经 `View.post` 调用 `ImeTouchPoints.register`，等子键完成布局再量矩形；`register` 仍自行过滤非标准 26 键布局。同步调用会在子键尚未布局时读到零尺寸，继续只依赖外层监听则会漏掉调试几何导出和引擎注册。

设备验收又覆盖到一条时序：输入法的键行可能在引擎会话建立前就完成布局，`View.post` 先执行时会因 `session == 0` 放弃注册，之后没有新的布局事件。会话建立完成和保留会话的重启在 `render()` 后再次投递注册，确保会话就绪后仍会量到当前 26 键并写出几何；这不改变非双拼布局的过滤规则。

### 几何的来源

`resources/eval/key-geometry/` 里除 `harmony-portrait-microsoft` 外都换成了宿主在模拟器上原样导出的几何，文件名不变，`id` 统一成 1：

| 文件 | 来源 | 与构造的那份相比 |
|---|---|---|
| `android-portrait`、`-microsoft` | API 35 模拟器（1080×2400、420 dpi）上输入法真正注册的那份，像素 | 键高 137 px ≈ 52 dp（构造的 46）；归一后键高与键宽之比 1.46–1.51（构造的 1.29–1.32），行距 1.13 个键高（构造的 1.17） |
| `ios-iphone-portrait`、`-microsoft`、`ipad-split` | iPhone 17 模拟器（iOS 27.0）上 `TouchPointTests` 里的 `KeyboardViewController` 按 402 pt 屏宽排出，`ipad-split` 是 trait 覆盖模拟的 iPad 横屏分离键盘 | 键宽 34.3 pt、键距 40.2 pt（构造的 32.4、38.4）；iOS 的分离键盘不重复字母键，`duplicates` 从 G、V 两个变成空 |
| `harmony-portrait` | HarmonyOS 6.0.2 模拟器上键盘注册几何时打的 hilog，vp，原点含工具栏 | 键宽逐行 31.51、30.91、32.77（构造的统一 29.2） |
| `harmony-portrait-microsoft` | 仍是构造的：模拟器上未签名包的设置和键盘进程不同步偏好，键盘切不到微软双拼，没导出 | — |

iOS 三份是测试宿主排出来的，不是键盘扩展在系统里真正注册的那份；扩展的调试导出已经接好，还没有从模拟器或真机取过。扫参和基线只用 Android 两份（与 PR1a 相同），所以这一点只影响参考文件本身。换几何以后，普通布局里平面表邻键的键心最远相距 2.37 个键（iPhone 微软版的 J 和 M），分离布局里分在两半的平面表邻键最近 4.6 个键，`FLAT_NEIGHBOR_REACH` 的 3 仍把两者分开。

### 重扫

流程与上面 PR1a 那一节逐项相同，资源仍是词库锁 dict-v2.0.24 那一组：调参集是 sentences-v1、particles-v1、v2（455 条能编码），留出 neutral-v1（1064 条），几何是 Android 两份导出，种子 20261027，`--assert-top1-unchanged` 比实际首选文本。所有组合的 clean 集实际首选都没有变（`first changed` 0），每组按方案 × σ × 偏置 × 音节档 × 错键数档的每个分桶，top1/top3/top9/found/mrr 都不低于同条件的平面表。

1. **只调座位的分数差**（B=12、s=8、不加逐键的闸，小鹤，σ=0.25、b=0）：平面表 noisy top9 0.007、found 0.143。`show_margin` 取 −1000、0、10、+1000 时 noisy top9 是 0.007、0.116、0.196、0.251，clean 被挤 0、1、1、29 / 455，与构造的几何几乎一样。
2. **B × s × 分数差**（同一条件，96 组，不加逐键的闸）：仍是 B 越低召回越高、被挤越多。B=6 一排仍在前沿（B=6、s=12、分数差 10：top9 0.264，挤 4；B=6、s=16、分数差 8：0.244，挤 4；PR1a 是 0.262 挤 3、0.242 挤 1）；新几何上多出一条 B 较高、分数差较大的前沿（B=9、s=12、分数差 10：0.233，挤 1；B=9、s=16、分数差 15：0.244，挤 2；B=12、s=12、分数差 15：0.233，挤 2）。在 B=8、s=12、分数差 6、`show_penalty` 14 上单独改 `cut`（12/20）、`max_penalty`（24/40）、`min_penalty`（0/8），结果与不改逐字相同（top9 0.193，挤 0）；`sigma_y` 0.8 是 0.185，1.25 是 0.202。
3. **加上逐键的闸**（调参集，小鹤和微软，σ∈{0.25,0.35}×b∈{0,0.1}，3640 条 clean；被挤比例 / noisy top9）：

   | B | s | 分数差 | `show_penalty` | 被挤 | top9 |
   |---|---|---|---|---|---|
   | 6 | 16 | 8 | 不加 / 14 / 10 | 0.88% / 0.63% / 0.52% | 0.179 / 0.167 / 0.132 |
   | 9 | 12 | 10 | 不加 / 14 | 0.69% / 0.49% | 0.174 / 0.154 |
   | 9 | 16 | 15 | 不加 / 14 | 1.87% / 1.21% | 0.185 / 0.148 |
   | 12 | 12 | 15 | 不加 / 14 | 1.21% / 0.44% | 0.175 / 0.101 |
   | 6 | 12 | 4 / 6 | 14 | 0.30% / 0.36% | 0.142 / 0.166 |
   | 6 | 16 | 6 | 14 | 0.33% | 0.148 |
   | 7 | 12 | 4 | 14 | 0.27% | 0.126 |
   | 8 | 12 | 4 | 不加 / 16 / 14 / 12 / 10 | 0.33% / 0.22% / 0.19% / 0.11% / 0.05% | 0.123 / 0.120 / 0.112 / 0.099 / 0.072 |
   | 8 | 12 | 0 / 6 | 14 | 0.16% / 0.25% | 0.084 / 0.131 |
   | 8 | 16 | 4 | 14 | 0.11% | 0.101 |
   | 10 | 12 | 4 | 14 | 0.11% | 0.084 |
   | 8 | 12 | 6（`sigma_y` 1.25） | 14 | 0.27% | 0.142 |

   第二步那条 B 较高的前沿只在 σ=0.25、b=0 上好看，到了 σ=0.35 就挤得多：B=9、s=12、分数差 10 加闸以后，σ=0.35 的四组被挤 14 / 1820（0.77%），单组最多 5 / 455；B=8、s=12、分数差 6 同样四组是 7 / 1820（0.38%）。
4. **留出集核对**（neutral，小鹤和微软，σ∈{0.15,0.25,0.35}×b∈{0,0.1}，12768 条 clean）：

   | 配置 | 被挤 | 单组最多 | noisy top9 |
   |---|---|---|---|
   | 平面表 | 0 | 0 | 0.001 |
   | B=6、s=12、分数差 10、不加闸 | 0.93% | — | 0.194 |
   | B=6、s=16、分数差 8、不加闸 | 0.60% | — | 0.182 |
   | B=8、s=12、分数差 4、`show_penalty` 14（PR1a 的默认值） | 0.12% | 3 / 1064 | 0.130 |
   | B=8、s=12、分数差 6、`show_penalty` 14 | 0.16% | 4 / 1064 | 0.146 |
   | B=8、s=12、分数差 6、`show_penalty` 14、`sigma_y` 1.25 | 0.20% | 5 / 1064 | 0.151 |
   | B=6、s=12、分数差 4、`show_penalty` 14 | 0.21% | 5 / 1064 | 0.156 |
   | B=6、s=12、分数差 6、`show_penalty` 14 | 0.27% | 7 / 1064 | 0.174 |

选 B=8、s=12、分数差 6、`show_penalty` 14，其余不变：留出集被挤 0.16%，单组最多 4 / 1064（0.38%，微软 σ=0.35、b=0.1），调参集单组最多 2 / 455；第一页召回比 PR1a 的默认值在留出集上高 1.6 个点（相对一成多）。B=6 那两组召回更高，但最难的那一组分别到了 5 和 7 / 1064，0.5% 的门槛按总数算虽然还过，单组已经贴线或越线。`sigma_y` 1.25 多 0.5 个点、单组多挤一条，和 PR1a 一样不采用。

### 定参后的整句族与词级族

上面 1–4 的扫参是在词库锁 dict-v2.0.24 那一组资源上做的。PR1a 合并以后 develop 把词库锁换成了 dict-v2.0.39，PR1b 改基于 develop，参数不变，按 README 的命令（五个方案 × 四种模式 × σ{0,0.15,0.25,0.35} × b{0,0.1}，两份 Android 导出几何，`--jobs 5 --assert-top1-unchanged`）在 v2.0.39 的资源上重新生成两份基线，另用同一个二进制加 `--no-touch-points` 跑出同条件的平面表作对照（平面表不提交）。资源目录的 12 个文件逐项按 `desktop-dictionary.lock.json` 核对过大小和 SHA-256（generation `576494c4…`），目录里没有 `sentence-model-desktop.safetensors`，stderr 打出 `no settled model at …`。

- **门槛**：`--assert-top1-unchanged` 通过，clean 集 60760 条的实际首选文本一条都没变，`top1_changed` 为 0；按方案 × σ × 偏置 × 音节档 × 错键数档的每个分桶，top1、top3、top9、found、mrr 都不低于平面表；clean 集被往后挤一位的金标准 69 条（0.11%），单组最多 6 / 1519（0.39%，首道 σ=0.35、b=0.1），σ=0.35 的 10 组共 35 条、σ=0.25 的 10 组共 27 条、σ=0.15 的 10 组共 7 条、σ=0 的 10 组为 0，平面表为 0；none 模式五组送不送落点逐项相同。v2.0.24 上同一套是 40 条（0.07%），单组最多 7 / 1519（0.46%，智能 ABC σ=0.35、b=0）。
- **词库本身的变化**：none 模式（不带落点，纠错整句照旧沉底）的整句族 top1 从 v2.0.24 的 0.679 降到 0.632，top9 从 0.876 降到 0.828；词级族 top1 从 0.733 降到 0.697，top9 不变。这一期的改动碰不到 none 模式，这是 develop 换词库带来的，所以下面的数字只和同一份词库上的平面表比。
- **noisy σ=0.25、b=0**：第一页召回从平面表的 0.1%–0.7% 升到 16.4%–17.4%（v2.0.24 上是 16.0%–18.1%），found 从 9.5%–11.3% 升到 21.5%–22.5%，MRR 从 0.002–0.006 升到 0.042–0.046；小鹤单错的 1211 条第一页召回从 0.3% 到 20.2%。σ=0.15 时五个方案 23.8%–25.1%，σ=0.35 时 3.8%–5.8%，b=0.1 时 14.7%–17.1%。
- **realistic σ=0.25**：第一页召回升 4.6–7.2 个点（小鹤 0.550 到 0.596），top1 不变。
- **延迟**：照「代价」一节的量法（`rerank_latency --touch-arms`，sentences-v1 加 particles，σ=0.25，两份 Android 导出几何，每个方案 5 轮、每轮紧接一次 A/A），10-28 晚上在 Studio 上用 v2.0.39 的资源测，负载 59–106，比 v2.0.24 那次（35–65）还重。两臂 p95 之差的中位数：小鹤 −0.17 ms（各轮 −1.27 至 +1.07），微软 +0.08 ms（−0.17 至 +0.77），自然码 +0.08 ms（−0.01 至 +0.70）；A/A 的同一个量 −0.12、+0.08、+0.16 ms（各轮 −1.39 至 +2.29）。三个中位数都在 0.5 ms 之内，但 A/A 自己的摆动到了 2 ms 以上，和上一次一样只能说没有看到超出噪声的增量。带触点那一臂的 p95 中位数 11.05–11.75 ms，最高 12.43 ms，30 次运行都在 16 ms 预算内；比上一次高是机器负载，平面表那一臂同样高（中位数 11.2–12.1 ms）。
- **词级族**（2 音节）：送落点的报告与平面表逐字节相同，与 PR1a 一样，2 音节这一期没有纠错；数字随词库变了。

### PR1b 的验证

- **基于 develop 重建**：PR1a 以改写过的历史合并进 develop（#10083），三条宿主分支和重扫所基于的那个 PR1a 提交不在 develop 上，所以 PR1b 从 develop 重新开分支，把三端宿主的 9 个提交和重扫的 3 个提交逐个挑过来，没有带合并提交，也没有带旧 PR1a 的任何提交。冲突都在清单和测试文件里：`verify-native.sh` 的导出表（把两对新符号插进 develop 的表）、Android `smoke.sh`（develop 改用 `check` / `verdict`，按下点验收照它的写法带退出码交给 `verdict`，几何导出照旧）、鸿蒙 `run.sh` 和 iOS `project.pbxproj`（两边新加的段都留下；Studio 上 `xcodegen generate` 生成的工程与提交的逐字相同）、评测 README（几何挑选沿用 develop 评审后的写法）、`touch::tests`（develop 的几何指纹用例和这里钉住默认值的用例都留下）。develop 合并 PR1a 时的评审修正（缓存签名里的几何指纹、所有非字符入口清掉待用触点、逐行的 `touch_supported`、共享的方案表、评测工具的后续修正）都原样保留。
- **本机**：`platforms/android/check-host.sh`（293 个 JVM 冒烟，含 `TouchPointPolicySmoke`，255 个设备套件源文件编过）和 `platforms/harmony/tests/run.sh`（0 失败，含按下点与几何的 6 项）通过。
- **Mac Studio，经 `rbuild`**：`cargo test -p msime-engine --lib` 2742 项通过、57 项忽略（含 `the_default_tuning_is_the_swept_one` 和 `the_fingerprint_follows_the_content_not_the_id`），`--test golden` 31 项，`msime-input-runtime` 库 405 项，`convert_eval` 29 项、`rerank_latency` 26 项，`cargo test -p msime-host-api --no-fail-fast` 全部目标 1067 项通过、0 失败（库测试 1043 项；PR1a 记下的那两项在 develop 上已经不再失败）；`cargo clippy -p msime-engine -p msime-input-runtime -p msime-host-api --all-targets -- -D warnings` 无警告。
- **iOS**：Studio 上经 `rbuild` 建一台用完即删的 iPhone 17 模拟器（iOS 27.0），`build-for-testing` 通过，`TouchPointTests` 17 项和 `KeystrokeLatencyTests` 5 项通过。同一次运行加 `TEST_RUNNER_MSIME_KEY_GEOMETRY_EXPORT` 导出的三份几何，把 `id` 统一成 1 以后与提交的 `ios-iphone-portrait`、`-microsoft`、`ipad-split` 逐字相同，develop 的按键布局样式（#10070，默认样式逐键不变）没有改 iOS 默认的几何。Android 和鸿蒙的几何没有在 develop 上重新导出。
- **G0**：这一期的改动只在带落点时起作用；两份族里 none 模式的报告送不送落点逐项相同。没有用 develop 的二进制重跑 `verify-local.sh` 的 11 组做逐字节对照，上一次的对照是在 v2.0.24 的资源和旧的合并树上做的（本分支对 PR1a 头逐字节相同）。
- **三端各自分支上做过的设备验收**（基于 develop 重建以后都没有重跑）：
  - Android：自己起的 API 35 模拟器上 `MicrosoftShuangpinDeviceSmoke -e shuangpin-touch true` 通过（导出的几何与无障碍树逐键一致；键的边角和键距里的真实触摸组字是命中的键；小鹤 `mwgfxi` 把 f 按在偏 r 的一角时「没关系」以纠错行出现，前后两遍无障碍点击都不出现；微软 `;` 带落点），`DeviceSmoke` 等回归也过；完整的 `smoke.sh` 没跑。
  - iOS：模拟器上的 XCTest，`TouchPointTests` 等 36 + 68 项通过，完整 `MSIMEKeyboardTests` 1001 项通过（第一轮，修正之前）；没有在系统输入框里用真正的键盘扩展打字。
  - HarmonyOS：模拟器上用 uitest 合成点击，首键注册一次几何、之后不再注册；`womdui` 把 d 按在右缘（偏 f）时第一页第 4 格出现「我们是」纠错行，按在键心或左缘时没有；修正按下点跟键走之后没有再上模拟器。
- **没做的**：三端都没有真机盲打，也没有在真机上量每键延迟；iOS 扩展和 HarmonyOS 微软双拼的几何没有真正导出；Android API 28、横屏、分离键盘的几何没有在设备上走过；扫参没有在 v2.0.39 上重做，只在 v2.0.39 上核对了定下的参数。

## Alternatives considered

- **照方案原样，runtime 不改**：方案假定纠错整句在宽度 ≥3 时会被 `runner_up_order` 留在前三。PR0 量出它几乎总在表尾，只改引擎计价，第一页召回停在零点几个百分点，召回门槛量不出差别。
- **双拼的纠错行一律不算整句读法**：最简单，不需要任何新状态。但桌面开着纠错时（Windows 默认开），没有触点的纠错整句也会从表尾挪到整句块之后，none 模式的候选表变了，过不了 G0。
- **只要请求带触点，纠错行就留在座位上**：先这样实现，在调参子集上 clean 集有 29 / 455 条（6.4%）金标准被往后挤了一位。打对的输入里一半以上也有一条纠错整句（键心落点的罚分高，但 64 条 n-best 里总有一条带纠错边的路径），它挤在整句块之后，排在它后面的金标准都后退一位。按分数差留座位之后同一子集降到 1–2 条。
- **只看分数差**：在调参子集 σ=0.25、b=0 上已经够用（被挤 1 / 455），但留出集和 σ=0.35 上压线或超线（B=6、s=16、分数差 8 时 3640 条 clean 被挤 1.13%）。σ 大时 clean 的落点贴着键边，改到邻键的价接近 B，模型又给某条纠错整句的分接近字面整句，它就坐上座位。加上逐键的价的闸能单独压住这些：B=8、s=12、分数差 4 时，`show_penalty` 14 把被挤的比例从 0.44% 降到 0.19%，noisy top9 只从 0.125 降到 0.117。
- **runtime 按行带一列「留在座位上」**：语义最直接，但候选的每个平行数组都要在重排、在线插入、插件变换里同步一遍。一张双拼表里最多一行纠错整句，表级的一个布尔值就够，行上的标记留在 `WordItem` 里，由会话汇总。
- **计价参数经 `host::Options`（`EngineOptions`）下发**：方案原本这样写。`EngineOptions` 刻意没有 `Default`，每个构造点都列全字段，加一个只给评测用的字段要改十几处，宿主的 JSON 也得决定要不要认它。改成 `host::Session::set_touch_tuning` 这个只在 Rust 侧可调的 setter，经请求带给词典，效果相同。
- **`KeyTouch` 只记 (dx, dy)**：方案的写法。分离键盘上同一个字母有两个实例，只记偏移还原不出落点在哪一边，所以多记一个实例序号。
- **平面表的邻键在任何布局下都保留**：方案说「永不丢弃」，又要求「分离布局下 t、y 不产生替换」。两者在分离键盘上冲突；按几何距离判断后，普通布局下平面表的每个邻键仍都保留，只有分在两半的那几对被去掉。阈值起初定为 2 个键，量了七份参考布局才发现微软双拼那几份第二行多一个 `;`、键距变宽，J 和 M 相距 2.14–2.25 个键，会被误删，于是放到 3。
- **PR1b 保留 PR1a 的默认值**（`show_margin` 4）：在导出几何上它仍过全部门槛，留出集被挤 0.12%，余量最大。但同样的留出集上 `show_margin` 6 只多挤 5 条（0.16%），第一页召回高 1.6 个点，最难的一组仍在 0.4% 以下，余量够用就不该把召回留在桌上。
- **PR1b 取 B=6 那一排**（B=6、s=12、分数差 4 或 6，加 `show_penalty` 14）：留出集召回最高（0.156、0.174），总数也在门槛内（0.21%、0.27%），但 σ=0.35 的单组到了 5 和 7 / 1064，0.5% 的门槛已经贴线或越线，换一个方案或几何就可能超。
- **iPad 分离布局保留构造的那份**：构造的那份右半边重复 G 和 V，能在评测里走到重复键的实例选择；但 iOS 的分离键盘实际不重复字母键，留着一份宿主从不会发的几何当「iPad 参考」会误导，重复键的逻辑由 `touch::tests` 的分离布局用例覆盖。

- **触点经 `Action::Character` 传递**（评审提出）：语义上最干净，但 `Action::Character` 有约 140 个构造点（runtime、host-api、各宿主桥、测试、评测），每一处都要补一个字段或改成新变体，而且桌面宿主永远不带触点。方案选了旁路（`set_key_touch` 加原路派发再清掉），这一期不改；旁路的风险是待用触点被别的入口带到下一个字母，靠所有非字符入口一开头清掉来封住，并有测试逐个入口核对。
- **从全拼的 `plan_keys_with` 抽出与 `plan_priced_keys` 共用的弱位置和跨度生成**（评审提出）：两者有一段重复。但方案要求全拼的规划代码一行不改，全拼纠错整句的结果要逐字节不变，抽公共函数等于改全拼的规划；这一期保留重复，等全拼也接触点时再合。
- **重新设计逐行的 `candidate_touch_supported` 列**（评审提出：把它并进更通用的逐行标志，或不再在每个平行数组里同步）：现在这一列只在有落点支持的行时才非空，桌面永远为空。PR3 让落点支持的纠错整句有条件地坐首选时，本来就要一个逐行标志，届时一起重新设计，这一期不动。

## Consequences

- **收益**（PR1a，构造的参考布局；PR1b 在导出几何上的数字见上面「定参后的整句族与词级族」）：整句族（1519 条能编码的用例，五个方案）按默认参数重跑，与平面表（PR0 那份基线，`--no-touch-points` 逐字节复现）比：
  - noisy σ=0.25、b=0：第一页召回从 0.1%–0.7% 升到 14.0%–16.0%，found 从 9.5%–11.5% 升到 21.5%–24.7%，MRR 从 0.002–0.007 升到 0.036–0.041；小鹤单错的 1177 条第一页召回从 0.3% 到 17.8%，3–5 音节 1.3% 到 17.9%，6 个以上 0.1% 到 14.0%。σ=0.15 时小鹤 0.3% 到 20.6%，σ=0.35 时 0.1% 到 4.7%，b=0.1 时 0.2% 到 15.1%。
  - realistic σ=0.25：第一页召回升 4–6 个点（小鹤 0.550 到 0.591），top1 不变。
  - noisy 的 top1、top3 不变：纠错整句坐在整句块之后，这一期不抢首选。
  - 按方案 × σ × 偏置 × 音节档 × 错键数档的每一个分桶，top1、top3、top9、found、mrr 都不低于平面表。
  - clean 集 60760 条（五个方案 × 8 组 σ 和偏置）中，原本排第一的金标准没有降位，被往后挤一位的金标准有 33 条（0.05%），单组最多 4 / 1519（小鹤 σ=0.35）。旧门禁没有保存实际首选文本，不能据此断言全部首选不变；修正门禁后尚未重跑这份完整评测。
  - 词级族（2 音节）与平面表逐字节相同：纠错整句至少 3 个音节，这一期 2 音节没有纠错。
- **代价**：每次按键多一次触点换算和请求里一份触点的复制，带触点的输入缓存键多一段签名。
  - 门槛「每键 p95 增量 ≤ 0.5 ms」的量法：`rerank_latency --touch-arms`，两臂都挂模型，一臂送落点、一臂不送，打同样的键；集合是 sentences-v1 加 particles，σ=0.25，用两份 Android 参考几何。取两臂 p95 之差，每个方案用同一条命令重复 5 轮，取中位数。每轮紧接着再跑一次加 `--no-touch-points` 的 A/A（两臂都不送），作为噪声底。
  - 逐键配对增量的 p95 不作门槛。A/A 下它的中位数也有 0.19–0.31 ms，单轮最高 1.06 ms，量到的是两次相同按键各自的抖动。早先单轮记下的「微软配对增量 p95 0.65 ms」就是这类数字，所以不再引用。
  - **结果：门槛通过。** 10-28 在 Studio 上测，负载 10–18，小鹤、微软、自然码交替跑：
    - 两臂 p95 之差的中位数：小鹤 +0.04 ms（各轮 −0.34 至 +0.16），微软 +0.15 ms（−0.08 至 +0.23），自然码 +0.03 ms（−0.31 至 +0.10）。
    - A/A 的同一个量：−0.05、+0.07、+0.09 ms，各轮范围 −0.17 至 +0.32。触点的增量落在 A/A 的噪声范围之内。
    - 配对增量 p95 的中位数：0.27、0.29、0.36 ms；A/A 为 0.19、0.31、0.22 ms。
    - 带触点那一臂的 p95：中位数 5.76、5.63、5.90 ms，最高 6.23 ms，在 16 ms 预算之内。
  - 同一天负载 15–30 时另跑过一批，同样 5 轮，后两轮遇上机器尖峰，平面表那一臂自己的 p95 都到了 14–22 ms：
    - 两臂 p95 之差的中位数：+0.17、−0.25、+0.05 ms，仍在门槛之内。
    - 配对增量 p95 的中位数：0.69、0.81、0.37 ms，单轮最高 13.64 ms。这个量随机器负载漂移，单轮数字不能当门槛用。
  - Studio 同时挂着 CI runner。宿主接入以后要在真机上再量一次。
- **已知上限**：
  - 参数在模拟器导出的 Android 几何上调出（PR1b），落点仍是合成的，模型偏乐观（各向同性高斯、统一偏置、不模拟拇指的系统性偏移和同一音节两键都错），真机上的数字会更低。
  - 纠错整句仍不坐首选，noisy 集的 top1、top3 几乎不动，收益全在第一页的后几格。
  - 每个跨度只换一个音节，不出第二条纠错句；规划预算仍是 96 个键。
  - 光标前缀查询、撤回选词后重打、无障碍点击和程序化点击没有触点，按平面表算。
  - 1–2 个音节的输入还没有纠错（纠错整句至少 3 个音节），是后面一期的事。
- **隐私**：坐标只在会话内存里用，量化后的触点只进内存里的缓存键，不落盘、不进统计、不上传。
- **重访信号**：有了真机导出的几何或真实盲打的落点分布之后要重扫这里的全部参数；首选规则（`confident_correction`）在这里的 `touch_supported` 之外另设了更严的闸，见[双拼触点纠错坐首选的规则](2026-10-29-shuangpin-touch-first-seat.md)。用户接受过的码对给这里的罚分打折、被拒绝的输入不再纠，见[双拼纠错按两键码学习](2026-10-28-shuangpin-typo-learning.md)。

## Verification

- **单元和集成测试**（都在 Mac Studio 上经 `rbuild` 跑）：
  - `cargo test -p msime-engine --lib`：2715 项通过。新增 `touch::tests`（量化与钳位、非有限坐标、`;` 与重复键、整份校验、JSON 清除与拒绝、罚分随落点和 σ、偏置变化、分离布局的实例与邻接）；`shuangpin::scheme` 的触点对齐（逐键、退格、被拒的键、光标处插入删除、吃掉前缀、叠词吃掉和补回前缀、`hh`、`'` 和句中辅助码不带触点）；`shuangpin::typo_edges` 的计价变体（平面表变体全在、只按落点排价、平面表外的键过 P_CUT 才加、分离布局去掉 t→y）；`quanpin::typo_edges` 的全局按价规划和预算；`shuangpin::tests` 的同字母不同触点不串缓存、换几何代次不读旧答案、留座位的两道闸、不抢首选；`session::tests` 的待用触点生命周期和快照里只标上落点支持的那一行；`ordering::tests` 的 `runner_up_order_with`（含同一张表两条纠错行只留一条的座位）。
  - `cargo test -p msime-input-runtime --lib`：405 项通过，含 `key_geometry_survives_an_engine_replacement`（重建引擎后几何仍有效、代次不符和坐标非有限时没有触点、清除后不带触点）。`--example convert_eval --example rerank_latency`：21 / 16 项通过，含落点落在它报告的键上、只抽键与抽落点按到的键逐个相同。
  - `cargo test -p msime-host-api --no-fail-fast`：库测试 1008 项通过、2 项失败（`listing_reports_every_pack_and_checks_the_state_root`、`a_command_gets_the_local_time_to_the_minute`），这两项在本分支的父提交上同样失败，与这次改动无关；其余测试目标全部通过。新增的 `touch_characters_without_usable_touches_match_plain_characters` 逐字节比对代次不符、NaN、无穷大三种情况下的回应，并断言 ABI 仍为 3；`key_geometry_is_validated_at_the_host_boundary` 覆盖未知键、少键、零宽、归一后溢出、键心过远、非 JSON、超过 16 KiB、空指针和清除。
  - `cargo test -p msime-engine --test golden`：31 项通过。`cargo clippy -p msime-engine -p msime-input-runtime -p msime-host-api --all-targets -- -D warnings` 无警告。`cargo check --manifest-path fuzz/Cargo.toml --target-dir target/fuzz --bins` 通过，`host_session` 目标新增注册几何（合法的、带 `;` 和重复键的、任意字节）和带任意坐标（含 NaN、无穷大）的触点按键；没有用 nightly 实际跑模糊测试。
- **G0**：本分支和父提交（PR0 头）的 `convert_eval` 各跑 `verify-local.sh` 的 11 组，以及双拼 none 模式（五个方案，整句族和词级族，带 `--dump`），报告和 dump 逐字节相同；改成按方案各用一份可写目录、加 `--jobs` 之后，用最终的二进制重跑整句族 none 的报告和 dump、sentences、nine-key 三组，仍逐字节相同。父提交与 merge-base 的这 11 组在 PR0 已核对过逐字节相同。
- **评测可复现**：最终二进制加 `--no-touch-points --jobs 5` 重跑整句族全部组合，与 PR0 提交的平面表基线逐字节相同；词级族送落点的报告与 PR0 的词级基线逐字节相同（`--jobs 5`）。两份基线由 README 里的命令生成（`--report` 写出，与 `--update-baseline` 写的字节相同），旧 `--assert-top1-unchanged` 通过（只检查原本排第一的金标准未降位）。
- **评审后的修正**：评审之后改了三处。一是触点重新对齐时先认后缀；二是快照里的落点支持改成逐行的一列；三是拒绝归一后会溢出的几何。这三处在没有触点、只有一条纠错行、几何正常时都不改变结果。改完以后用最终二进制按 README 的参数重跑了整句族的一个子集：小鹤和微软 × none、clean、noisy、realistic，σ=0.25，b=0。8 组报告与提交的基线里对应的 8 组逐项相同，旧 `--assert-top1-unchanged` 也通过（同样不能证明实际首选文本不变）。完整的 125 组没有重跑。
- **原实现阶段没有做的**：`scripts/verify-local.sh` 整套没跑；没有在任何真机或模拟器上验证，宿主还没有接这两个函数（PR1b）；没有用真机几何调参。

### 本轮评审修正的验证

- 几何变化回归在原 setter 上失败，修正后方案层 13 项通过：代次变化、同代次布局变化、清除都丢弃旧触点和待用触点；相同几何重复下发保留触点；新按键仍与保留的输入串逐字节对齐。
- 引擎库测试 2717 项通过、57 项忽略，golden 31 项通过；运行时库 405 项、`convert_eval` 23 项、`rerank_latency` 16 项通过。两个新首选门禁回归在旧统计规则上失败，修正后覆盖金标准名次不变或未命中时首选互换、空表转换和仅后排变化。
- `cargo clippy -p msime-engine -p msime-input-runtime --all-targets -- -D warnings`、格式和笔记校验通过。
- 将本轮资源目录的全部文件逐项按本分支 `desktop-dictionary.lock.json` 校验大小与 SHA-256 后，使用 `sentences-v1 --limit 12`、小鹤和微软、none/clean、σ=0.25、b=0、种子 20261027 实跑；两方案各 12 条，修正后的 `--assert-top1-unchanged` 通过。这里只验证 24 条 clean 用例，没有重跑完整的 60760 条。

- 本轮完整 `bash scripts/verify-local.sh --quick` 退出 0：Windows 可移植用例、仓库契约与笔记、Rust workspace、Android 目标及 Java 宿主、WASM、Linux 设置应用和原生门禁（112 项、0 失败）、macOS 原生编译及 Apple 桥通过。缺 MinGW 的 Windows 原生/pipe-only 与缺暂存输入的鸿蒙 HAP 按规则跳过，不计设备验收；本轮没有补跑 iOS 原生套件或完整触点基线。

### PR #10083 第二轮评审修正

- 改了什么：触点缓存签名加上几何内容指纹（`KeyGeometry::fingerprint`，构造时对归一单位和键心的位模式求哈希，-0.0 归成 0.0，内容 `==` 相同则指纹相同）；`select_edge`、`finish`、`set_caret` 等其余非字符入口一开头清掉待用触点；`touch_supports_correction` 复用规划纠错边时算好的计价变体；在线行写入辅助码缓存改用 `signed_key`；英文词典、日文 provider、韩文汉字的原地复用路径清掉 `touch_supported`；解码表并进变体表的按方案缓存条目；`Runtime::key_geometry_id` 改成只在测试里编译。没有触点时以上每一处的结果和分配都与修改前相同。
- 新增和调整的回归，修正前都失败、修正后通过：`session::tests::a_same_id_geometry_with_moved_keys_does_not_reuse_cached_touch_rows`（同代次把 r 挪走、同样的量化触点，不再拿到旧布局的落点支持行；换回原布局答案照旧）、`session::tests::non_character_entries_discard_the_pending_touch`（17 个入口逐个核对，另有不调入口时触点照常交给下一个字母的对照）、`korean::hanja` 与 `japanese::provider` 的 `query_into_reuses_existing_candidate_rows`、`dictionary::english::keypad_append_tests::ordinary_decoder_reuses_and_resets_rows_like_frozen_entry`。另加 `touch::tests::the_fingerprint_follows_the_content_not_the_id`。
- 第一次全量跑出 `shuangpin::dictionary::online_cache_update_tests` 两项分配计数失败：改用 `signed_key` 时 `into_owned()` 在签名为空时多复制了一次键。改成签名为空时直接用原键后通过，分配次数与修改前相同。
- 结果（Mac Studio，`rbuild`）：`cargo test -p msime-engine --lib` 2720 项通过、57 项忽略；`--test golden` 31 项通过；`cargo test -p msime-input-runtime --lib` 通过；`--example convert_eval --example rerank_latency` 23 / 16 项通过；`cargo test -p msime-host-api --no-fail-fast` 库测试 1008 项通过、2 项失败，仍是上面记过的 `listing_reports_every_pack_and_checks_the_state_root` 和 `a_command_gets_the_local_time_to_the_minute`；`cargo clippy -p msime-engine -p msime-input-runtime -p msime-host-api --all-targets -- -D warnings` 无警告；`cargo check --manifest-path fuzz/Cargo.toml --target-dir target/fuzz --bins` 通过。
- 评测：资源目录用 `msime-client-core` 的 `install_resources` 按本分支 `desktop-dictionary.lock.json` 下载并逐项校验（generation `7356c4a4…`，stderr 打出 `no settled model at …`）。整句族四个集合，小鹤和微软 × none、clean、noisy、realistic，σ=0.25、b=0，种子 20261027，`--jobs 2 --assert-top1-unchanged`，退出 0；`--report` 写出的 8 组与提交的 `baseline-shuangpin-touch-sentences.json` 里对应的 8 组逐项相同，每组的三行在基线里逐字出现。完整的 125 组和词级族没有重跑；没有在真机或模拟器上验证。

### 并入 #10082 的评测工具评审后续

- 改了什么：#10082 合并之后对评测工具的评审后续（几何按引擎的 `host::shuangpin_uses_semicolon` 挑、同一种几何给两份报错、报告只列用上的几何；`--touch-sigma` / `--touch-bias-y` 共用校验，偏置可以为负、`-0` 记成 0；`case_rng` 的方案和模式标签写死；`rerank_latency` 编不出来的用例不计时并计数）挑进本分支，评测笔记照它改了。和这一期的触点路径合在一起时：注册给引擎的几何与合成落点用的是 `touch_noise::select_layouts` 挑出的同一份；写死的标签值与原来的 `profile as u64`、`mode as u64` 相同，随机数流不变；`--jobs` 下重排模型在开线程前只读一次，各方案的 Runtime 共用权重、各建自己的 `Reranker`，落点分布也在开线程前按（几何，σ，偏置）一次算好、各方案只读，送落点的 clean 查同一张表；`rerank_latency` 的 `encode_cases` 收下 `--no-touch-points`，送落点时连落点一起算好，noisy 下只能原样打的用例照旧不带落点。
- 验证（Mac Studio，`rbuild`）：`cargo test -p msime-input-runtime --example convert_eval --example rerank_latency` 28 / 25 项通过（`rerank_latency` 新增的跳过计数测试同时核对送落点时的落点和不送落点时按到的键逐个相同）；`cargo test -p msime-engine --lib host::` 124 项通过；`cargo clippy -p msime-input-runtime --all-targets -- -D warnings` 无警告。整句族四个集合，小鹤和微软 × none、clean、noisy、realistic，σ=0.25、b=0，种子 20261027，`--jobs 2`，stderr 打出 `no settled model at …`：加 `--assert-top1-unchanged` 退出 0，报告与提交的 `baseline-shuangpin-touch-sentences.json` 截出的这 8 组（`encoding` 只留这两个方案）逐字节相同；同样的命令改加 `--no-touch-points`，报告与 #10082 提交的平面表基线截出的 8 组逐字节相同。完整的 125 组和词级族没有重跑。

- 参数边界修正：`--touch-tuning` 在解析阶段拒绝 `min > max`，避免评测到触点罚分时触发 `f64::clamp` panic；相等边界仍合法。新增回归后随本轮修复重跑验证。

1–2 音节的完整词补查及其词级对照见[短词触点纠错](2026-10-31-shuangpin-touch-short-words.md)；沿用本篇的既有边界，不启用纠错首选或改变默认参数。

# Agent Note: 滑行输入的引擎解码与宿主契约

Status: implemented

## Problem

#5347 要在手机 26 键上支持谷歌拼音那样的滑行输入：手指不停地划过字母键，抬手出词；可以在键上稍作停留或折返来确认经过的键，但不能要求必须停留，落点有偏差要能自动纠正。Android、iOS、HarmonyOS 三个宿主之间不共享任何键盘代码，只共享 `crates/engine` 和 `crates/host-api` 的 C ABI；按 ARCHITECTURE.md，「这一笔拼的是什么」属于输入算法，必须在引擎里，宿主不能各写一份。

## Decision

- 解码分两层，都在引擎。`crates/engine/src/pinyin/glide.rs` 是纯几何解码器（不碰 SQLite）：坐标先按键宽、键高归一化，轨迹按 0.25 个键重采样（至多 128 点），在全拼音节前缀树约束下做令牌传递搜索。一个字母串的代价由四部分组成：对齐点到键中心的距离平方；对齐点之间的采样点偏离两键连线的距离平方，以及逆着两键方向往回走的距离；跳过的停留（手指在 0.35 键内停 120ms 以上）；每个字母的固定价，重复字母另加一点。代价随扩展单调不减，凑满 `limit` 个完整串后，比第 `limit` 名还贵的前缀直接剪掉。`rank_by_dictionary` 再用词典重排：取各切分里是词的最高对数权重，多音节词每多一个音节加 1.5，不是词的串按最罕见音节计分再罚 6，乘 0.7 后从几何代价里减掉。
- 会话层 `Session::glide`（`session/glide.rs`）只在全拼、无本地模式、非专用英文、九键数字没在组字时处理：取排名第一的串，用一次 `replace_editing_text` 写到光标处；组字里已有字母时两侧补 `'`，先 `xi` 再滑 `an` 得到 `xi'an` 而不是 `xian`。词典权重经 `ProviderRegistry::quanpin_best_weights` → `QuanpinDictionary::best_weights` → `PinyinDatabase::query_exact_keys_per_key` 一次批量查询取得。
- 宿主契约是 `msime_client_glide(handle, request, length)`：JSON 请求带 a..z 的键中心、一个字母键的尺寸和轨迹点 `[x, y]` 或 `[x, y, ms]`（2..1024 个，<=64KB，拒绝未知键），坐标系由宿主自选。返回值与 `msime_client_character` 相同；`handled=false` 时宿主丢掉这一笔，不得把经过的键当按键输入。经 input-runtime 的 `Action::Glide` 走与按键相同的 `dispatch`。
- 一次触摸是点按还是滑行只由宿主判断，三端规则相同：按下落在字母键上，手指到了另一个字母键、且横向离开按下点至少 0.4 个键宽才开始滑行（只看横向，在同一个键上纵向滑动的「滑动输入符号」照常工作）；滑行开始前第二根手指落下就放弃；开始后取消原来那个键的按下，画轨迹，抬手时一次性交给引擎。开关「滑行输入」默认关，只存在本机，不随账号同步。
- Android：`ImeGlideTyping` 实现 `KeyboardKeyArea.GlideTracker`，按键区在分发前先把事件交给它，接管后给子视图补一个 `ACTION_CANCEL`；轨迹 `GlideTrailView` 画在按键气泡的覆盖层上；判定与请求 JSON 在不依赖视图的 `GlideTypingPolicy`。开关是 `platform.android.glide_typing`，在「只在本机」一节。iOS 的宿主决定见 [2026-10-07-ios-glide-typing-host.md](2026-10-07-ios-glide-typing-host.md)。HarmonyOS 的触摸监听挂在三排字母行上，开关与按键音、振动一起存在 `key-feedback.json`（`glideTyping`），在共享设置页「屏幕键盘 › 手势」里显示，只有宿主返回这个字段时才出现。

## 几何代价各项的来历

只看偏离连线时，沿一条线来回走不花钱：在键盘一行上来回两次的 `laishuo` 被读成 `lao`，`keneng`、`shihou`、`geren` 都被读成单程的 `keng`、`shou`、`gen`。加上回退代价后，这一类全部消失。

字母固定价必须小：标准键盘上 `h` 几乎正好在 `z` 到 `o` 的连线上，`y` 在 `e` 到 `i` 上，`d` 在 `i` 到 `a` 上，几何上分不出 `zhong`/`zong`、`keyi`/`kei`、`zhidao`/`zhiao`。这类歧义要交给词典，所以几何层要多交几名（16 个）。出货词典里常用词权重压在五十万上下，常用字在几百万，不加多音节加分的话，企业（`qi'ye`）会输给切（`qie`）。

## Alternatives considered

- **SHARK² 式整词模板匹配**：对候选词生成理想折线，与轨迹按形状和位置比较，是英文滑行输入的经典做法，几何上更整体。但它要先有一张有限的候选词表去逐个比较，而拼音输入要求任意音节组合都能拼出来，不能只认词表里的词；把词表换成音节组合又会组合爆炸。逐字母对齐加音节前缀树约束，能拼出任何合法全拼，词典只用来排序。
- **每个字母只在轨迹拐点处对齐（先检测拐点再匹配）**：实现简单，但直线上经过的键（`zhong` 的 `h`）和停留确认的键都不在拐点上，平滑和噪声又会让拐点忽多忽少。
- **宿主把轨迹逐键翻译成字母后调 `msime_client_character`**：不用新增 C ABI，但纠错就得在三个宿主里各写一遍，违背「输入算法归引擎」。
- **几何解码之后把排名靠后的串也作为候选并进候选栏**：能在几何和词典都选错时让用户改选，但要给候选加上「来自哪种读法」的状态，还要改选中后的组字推进。现在先只写入第一名，用户可以退格重滑，或在途经的键上停一下来确认。
- **开关随账号同步**：Android 的其他手势开关都在 `settings_sync.rs` 的本机设置清单里随账号同步，但同步字段表在服务端也要登记，这次没有改服务端，所以三端都先只存本机。

## Consequences

- **收益**：三端共用同一个解码器和同一份请求格式，调参只改引擎。用本机出货词典里 540 个高频字词（1–3 音节）合成轨迹，在 Mac release 构建下评估：理想轨迹首选命中 98.7%；每个键位加 0.15 键的高斯偏移并平滑时为 94.1%，0.25 键时为 69%。一次滑行端到端（解码、词典重排、写入组字）p95 约 3–4ms，最慢约 10ms。
- **代价与已知上限**：共线歧义里两种读法都是常用词时（`jiu'shi`/`ji'shi`），几何和词典都分不出来，只能靠停留确认。噪声大到 0.35 键时首选只剩约四成。命中率和耗时都是合成轨迹在桌面 CPU 上的数，没有真人在真机上的数据，阈值（0.4 键宽）和代价常数都可能还要调。三个宿主都没有在真机或模拟器上用真手指滑过。

## Verification

- 引擎：`cargo test -p msime-engine --lib glide`（`pinyin/glide/tests.rs` 的几何与词典排序用例，`session/tests.rs` 的会话用例）。
- C ABI：`cargo test -p msime-host-api --lib glide`。
- Android：`ANDROID_SDK_ROOT=… bash platforms/android/check-host.sh`，跑 `tests/keyboard/GlideTypingPolicySmoke.java`，并用 NDK 对 `client_jni.cpp` 做语法检查。
- HarmonyOS：`bash platforms/harmony/tests/run.sh`。
- iOS 见上面链接的那篇。

# Agent Note: iOS 与鸿蒙候选显示英文音标

Status: implemented

## Problem

共享层早有 `msime_client_pronunciation_request` 和 ECDICT 生成的 `pronunciations/en-phonetic.db`（`resources/pronunciations.lock.json`，MIT），共享偏好 `candidate_pronunciation` 默认关闭。macOS、Windows、Android 已经把音标画在候选下面（Android 见 [Android 候选显示英文音标](2026-10-26-android-candidate-pronunciation.md)），iOS 和鸿蒙既没有开关、没有查询，也拿不到这张表：iOS 键盘不读这个偏好，鸿蒙的 HAP 不带表，设置页对鸿蒙也不显示「显示读音」。

## Decision

三端用同一套取词规则，与 Android 的 `CandidatePronunciationPolicy` 相同：纯英文候选（字母加空格、`-`、`'`）读它本身；中文候选只在释义目标语言是英语、释义画出来时读那条释义的第一行；首个词组、去掉 `to`/冠词、任一词不认识就不标，都交给共享层。音标只取决于查询文字和本机的表，与候选代次无关，所以两端都按文字缓存回答（查不到的记作空串），请求的代次固定为 0，开关或表变了才整份丢掉；缓存超过一次查询上限（256 段）的八倍时清空。音标在释义下面单独一行，和 macOS、Android 一样；这一行按设置预留，音标没到或查不到时留空，打字时候选栏高度不变。开关保持出厂关闭。

**iOS。** 原生设置「输入 › 翻译」在「离线英文释义」下面加「显示英文音标」，写共享文档的 `candidate_pronunciation`（`InputHabitSettings.pronunciationEnabled`），并镜像到 App Group 的 `candidate.pronunciation`。表走现有的 `IOSResourcePacks`：开关打开而资源包 `pronunciations` 未装时，设置页多出「离线英文音标词典」下载行，只有用户点「下载」才联网，按锁文件取回、校验后原子发布到 App Group 状态目录的 `resource-packs/pronunciations/`。键盘只看文件在不在（`CandidatePronunciationPreference.tableInstalled`），在 `applyCandidateGlossLayout` 时重查；开关打开、表在、方案走释义（中文或韩语）时顶栏多留一行（`currentGlossLines`）。候选条每次重画时收集还没缓存的文字，在释义工作队列上经 `MetasequoiaInputSessionBridge.candidatePronunciations` 查询，`supplementary_resources` 指向 `resource-packs`，回答回到主线程后重画候选条和展开网格。展开网格的格子按内容定高，音标到了才多一行。`IOSResourcePacks.bundled` 只认日文词典和语言词库两种随包资源，其余一律按未随包处理，否则带着语言词库的版本会把音标包当成已安装。

**鸿蒙。** 鸿蒙没有资源包下载，表随 HAP 分发（约 5.7 MB）：`stage-resources.sh` 第五个参数（默认 `target/pronunciations`）按锁核对长度和 SHA-256 后连同 NOTICE 放进 `resfile/pronunciations`，发版工作流先跑 `scripts/fetch_gloss_tables.py` 并设 `MSIME_REQUIRE_PRONUNCIATIONS=1`；键盘扩展像离线释义一样把它暂存到引擎资源旁。NAPI 新增 `candidatePronunciations`（`PAIR_ENTRY` 包 `msime_client_pronunciation_request`）。`KeyboardSession` 在发布候选前同步查还没缓存的文字（和离线释义一样在界面线程上查本机表），`CandidateEntry.pronunciation` 带给视图；`glossRows()` 变成 `translationRows()` 加 `pronunciationRows()`。触屏胶囊 34vp 只放得下词和一行注释，释义和音标都打开时 `KeyboardMetrics.touchExtraDetailVp` 让胶囊和工具条各高 11.5vp（`touchStripHeightVp`，按「候选栏大小」缩放），面板高度跟着变；只开其中一种时高度不变。2in1 横排把音标画在释义行下面，`CandidateWidthPolicy.chipContentVp` 让格子放得下音标；竖排接在词和释义旁边，`CandidateWidthPolicy.widthVp` 把音标的宽度加到这一行上（不是和释义取较宽的那个），否则长音标被窗口裁掉。只为音标预留了一行、没有释义行（`translationRows()` 为 0）时，译文和竖排一样画在词旁边，横排候选格（`candidatePrimaryLine`）和触屏胶囊（`chipAnnotationBeside`）是同一条规则，否则打开音标后译文就不见了。设置页：`apps/harmony` 给 `candidatePronunciation`，手机「输入 › 翻译」和 2in1 表达页一直显示「显示英文音标」，说明只写英文音标（`CandidatePronunciationSection` 的 `japaneseRomaji={false}`），因为鸿蒙不生成日文罗马音。键盘给英文候选标音标不看释义开关，所以这一行不以释义来源为前提，和 iOS 一样：关掉释义以后英文候选仍有音标，用户仍能在这里把它关掉。直接输入英文时的补全（`englishSuggestions`）鸿蒙这次不标音标，Android 会标；工具条照样按设置预留音标那一行，补全通道的高度不变。

**三端都不查带控制字符的文字。** 共享层的 `msime_client_pronunciation_request` 只要有一段文字带控制字符（Rust 的 `char::is_control`）就拒绝整批，在线、自定义或 AI 译文里的一个制表符会让整页都没有音标，而且失败不缓存，每次刷新都重查一遍。鸿蒙的 `CandidatePronunciationPolicy`、iOS 的 `CandidatePronunciationModel` 在取词和组请求时、Android 的 `CandidatePronunciationPolicy.lookupText` 在取词时把这样的文字当作没有可读的英文。

**Android** 版式已经是「释义下面单独一行、按设置预留」，这次不改。

## Alternatives considered

**iOS 也把表随 App 带上**，打开开关即可用，不要下载行。iOS 的键盘扩展有内存和包体积的压力，词库、日文词典已经是按需下载；共享资源包安装器、锁和校验已经在 iOS 上跑通，Android 也是同一条路，随包只会多出一份 5.7 MB 的数据给大多数不开这个开关的用户。

**鸿蒙做资源包下载**，HAP 不变大。鸿蒙现在没有任何资源包下载入口，为一张 5.7 MB 的表新做下载、进度、取消和镜像设置，工作量和风险都远大于随包；维护者已接受随 HAP 带这张表。

**按候选代次缓存，像释义那样新一代就丢弃。** 这是 Android 的做法，能保证画出来的音标一定对应当前这一页。但音标是文字的纯函数，按代次丢弃只会让每打一个键都重查同样的词；按文字缓存在代次变了时依然正确，旧回答只会填进还用得上它的文字。

**鸿蒙手机把音标接在释义同一行**，不用改工具条高度。胶囊的注释列宽只有可见宽度的三分之一，释义本来就常被截尾，接在后面的音标几乎总被截掉；macOS 也是因为同行太挤才改成单独一行（[macOS 候选读音单独一行](../bug-fix/2026-10-16-macos-candidate-pronunciation-own-line.md)）。

## Consequences

- **收益**：六个平台里五个能显示英文音标（Linux 另有 PR）；iOS 和鸿蒙的英文候选、中文候选的英语释义下面多一行音标，读屏在释义后念「音标 …」。
- **代价**：鸿蒙 HAP 多约 5.7 MB；鸿蒙手机在释义和音标都打开时工具条高 11.5vp，键盘整体高这么多。iOS 打开开关后顶栏最多高一行（释义开着时多 14pt，乘以候选栏大小；释义关着时候选行从 38pt 变成带注释的 42pt），第一次用要下载约 5.7 MB。鸿蒙在界面线程上同步查表，每次按键只查还没缓存的文字，中文候选在释义到达前不查。
- 展开候选网格：iOS 的格子在音标到了之后多一行；鸿蒙的展开网格本来就不画释义，这次也不画音标。
- 出厂默认仍是关闭；改成默认打开属于产品决策，不在这次范围内。
- 资源包的下载顺序和其他资源包相同：用户镜像、本项目镜像 `https://dl.msime.app/gh/`、GitHub 原地址，PRIVACY.md 按这个顺序写。
- 需要真机核对：iOS 下载行、键盘顶栏高度与音标位置；鸿蒙手机工具条长高后按键不被裁、2in1 横竖两种候选窗；Android 不变。

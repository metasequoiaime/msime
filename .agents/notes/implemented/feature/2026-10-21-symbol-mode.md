# Agent Note: 符号模式（S 模式，Shift+S）

Status: implemented

## Problem

Windows 正式版（v0.11.1）有一个「符号模式」：没有组字时按 Shift+S 进入，再输入分组码或符号名，候选里给出这一组符号。出厂配置 `installer/default_config/config.default.toml` 的 `utility.symbol_mode = true` 点名了四个分组码（`xl` 希腊字母、`sx` 数学、`szq` 带圈数字、`jt` 箭头）和两个符号名（`alpha`、`infinity`），`README.md` 的按键表把它列成「符号（S）」。

本仓库在 v0.11.1 对照里整块缺失：`LocalInputMode` 只有 Unicode、DateTime、QuickPhrase、Emoji、Kaomoji、SuperJianpin、TemporaryEnglish、TemporaryJapanese、Expression、Command、Mention、Url、Chaizi 十三个变体，没有 Symbol。已有的 `local::symbol_candidates` 只解决「打拼音出符号」（`zhengfu` 得 ±），查不到分组码：`xl`、`sx`、`szq`、`jt` 在它那张 271 行的表里零命中。于是按 Shift+S 打 `xl` 只会得到一个大写 S 开头的拼音组字。

缺口是三件事：新增模式变体、给这张表加分组码或另建一张分组表、加偏好与设置页一行。

## Decision

- **入口**：第 14 个本地模式 `LocalInputMode::Symbol`，名字 `symbol`，入口键 `S`。判定在 `local_mode_for_entry`：受总开关 `shift_letter_entry` 约束，方案用 `opens_local_modes`（全拼、双拼），和 E、M 一致；五笔下不进，因为分组码与符号名都是字母，不经方案的拼写。进入后只收字母：撇号、数字、`;` 都交还宿主，`spelling_symbols` 为空。和 E、M 一样不显示注释、不补辅助码。
- **行的来源复用 `CandidateSource::Emoji`（6）**。符号行选中时整段上屏、结束组字，不学习、不调频、不进个人上下文，不能置顶、固定或删除，不请求释义——这些规则今天由「来源是 Emoji」统一给出，与混排符号行同一条路。新增 `CandidateSource::Symbol` 要改线上值、`from_u8`、`CATALOG_SOURCES`、`normalize_online_slots`、各宿主的结束组字判断，漏一处就是只在某个平台出现的组字残留；[打拼音出符号那篇](2026-10-13-symbol-candidates.md) 已经权衡过一次，结论相同。
- **两条查法共用一次查询**（`local::symbol_groups::query_symbol_mode`）：先查整串分组码；没有精确组时才查前缀组，最后查符号名。整串优先是为了让 `xl` 只出希腊字母小写，不被 `xld`、`xlc` 这类同前缀的组掺进来。同一个符号只出一行，合计最多 `SYMBOL_MODE_LIMIT`（200，与 Windows `query_symbols` 的默认上限一致，一个组最多 214 个符号）。符号名走已有的 `local::symbol_candidates::query_mixed_symbols`，所以 `alpha` 得 α、`infinity` 得 ∞、`gamma` 得 γ Γ，和「打拼音出符号」用同一张自写表。
- **分组码表另建一张**：`crates/engine/src/local/symbol_groups.tsv`（`编码<TAB>中文组名<TAB>符号...`，87 组、3 929 个符号、约 43 KB），`symbol_groups.rs` 用 `include_str!` 编进引擎，第一次查询时解析一次。没往 `symbol_candidates.tsv` 里塞分组码：那张表的每一行是「名称 + 拼音」，写法规则（中文名称只含汉字、音节数等于字数、单音节只收派和度、拉丁名称至少四个字母）由它的单测钉死，分组码不是名称、没有读音，塞进去要么放宽这些规则、要么在表里造出假拼音。
- **默认开**，与 Windows 出厂 `symbol_mode = true` 对齐。Engine 的 `LocalModeOptions::symbol`、`EngineOptions::local_symbol` 默认 `true`；共享偏好是 `local_modes.symbol: bool`，`#[serde(default = "enabled_by_default", skip_serializing_if = "is_true")]`，缺键读作开、只有用户关掉时文档里才出现 `"symbol": false`。这和混输符号的 `mixed_input.symbols` 是同一套写法（[那篇笔记](2026-10-13-symbol-candidates.md) 记过这个先例）。共享设置页的 `localModeControlState` 按 `symbol ?? true` 读，所以桌面和触屏宿主上勾选状态都与实际一致。
- **设置页一行**：`packages/ui/src/settings/local-mode-options.ts` 的 `localModeRows` 在颜文字之后、超级简拼之前加「符号快捷输入(S 模式)」，说明写分组码与符号名两种查法，iOS 用触屏入口文案；它跟着「Shift+字母进入快捷模式」总开关一起禁用。macOS、Linux、HarmonyOS 的共享设置页可以修改该开关。Windows、iOS、Android 的原生设置页及 macOS 的原生回退设置未添加此项，见「没做的」。
- **脚本插件 ABI**：`LOCAL_MODE` 表加 `("symbol", 14)`，`LocalModeClass::Symbol = 14`；供作者复制的 C/Rust SDK 分别提供 `MSIME_LOCAL_MODE_SYMBOL` 和 `local_mode::SYMBOL`，值同为 14。`pack-tool` 的现有 ABI 测试严格比较两份 SDK 的完整常量表，防止新增模式只同步运行时而漏掉绑定。符号行不在可读文本白名单里（`local_mode_allows_text` 只放 0–5），和拆字 13 一样。

### 分组码来源与许可判断

- **只抄事实性映射**。上游是 rime-wanxiang 的 `wanxiang_symbols.yaml`（`amzxyz/rime-wanxiang`，发布 `v18.0.16`，提交 `7ab73ee0c7f80190004233e70f954833df64c80e`，CC BY 4.0）。本仓库取的是「分组码 → 码位集合」：编码、每组的码位和组内顺序。中文组名（`xl` → 希腊字母小写、`jt` → 箭头）是本项目自定的界面文字，组的先后也是本项目排的，不取上游的注释文字。
- **不整段复制**。只收符号面板本来就有的 87 个组；表情分类、元素周期表、XCompose 合成三段完全不读，所以没有任何 emoji、元素信息文本或合成序列进二进制；数字编码（`/0`–`/10`、`/hx…`）也不收，S 模式只收字母。生成器把这几点写成不变量，`scripts/test-symbol-groups.py` 再把 87 组、3 929 个符号、五个样例组的组名与头几个符号钉死，上游改动会在这里红。
- **署名义务随数据走**。CC BY 4.0 第 3(a) 条要求署名、给出许可证地址、说明改动。`resources/licenses/rime-wanxiang-CC-BY-4.0.txt` 写明上游、固定提交、版权行、取用范围、五条改动和「不背书」声明，后面接许可证全文；`resources/symbol-groups.lock.json` 按字节固定 `wanxiang_symbols.yaml` 与上游 `LICENSE`；`scripts/build_symbol_groups.py` 按锁下载并生成表，`--check` 逐字节比较。这份文件接进了十二个声明渠道（macOS 的 `CMakeLists.txt` 与 `Resources/Licenses`、Linux 的安装列表与 `THIRD_PARTY_NOTICES.txt`、Windows 的 `Collect-Notices.ps1` 与 `Notices.md`、Android 的 `build-native.sh`、HarmonyOS 的 `stage-resources.sh`、iOS 的 `project.yml` 与关于页、`docs/third-party.md`），清单由 `test-symbol-groups.py` 的 `NOTICE_CHANNELS` 核对。
- **与 GPL-3.0 的关系**：CC BY 4.0 是允许再分发的宽松许可，没有 ShareAlike，改编数据可以随 GPL-3.0 的引擎一起分发，署名义务落在上面那份文件和声明渠道上。这条判断和拆字表（CC BY 3.0）的处理一致，同样未经法务审阅。上游文件头写明它的符号分组抄自 Rime 的 `symbols.yaml`，那一层授权链条也没有独立核实；两条都记在 `docs/third-party.md` 对应行的末尾。

## Alternatives considered

- **只做符号名查法**（降级方案）：最强理由是这个切片零数据风险，复用现有 271 行的表。不用：任务点名的四个分组码正是 Windows 出厂配置里写出来的那四个，缺了它们「符号模式」只剩 `alpha`、`gamma` 十几个拉丁名，和已有的「打拼音出符号」几乎重叠；分组码数据量是 43 KB、87 组，没有触发降级条件。
- **把分组码塞进 `symbol_candidates.tsv`**：最强理由是一张表、一个解析器、一条查询路径。不用：那张表的行语义是「名称 + 拼音」，单测钉死了音节数与字数相等、单音节只收派和度；分组码没有读音，为了塞进去要么放宽规则（等于放弃表检查），要么编一个假拼音列，读代码的人会以为 `xl` 能按拼音打出来。
- **改用 `CandidateSource::Symbol`**：见「Decision」第二条，代价是把一个冻结的线上值链条改一遍，收益只是日志里不叫 Emoji。
- **前缀匹配时把同前缀的所有组混在一起**（`x` 出所有 `x` 开头的组）：最强理由是一次能看全。不用：`xl` 会连带 `xld`、`xlc`、`xlx` 等六个数学/希腊变体组，一页里三套字形，用户要的那一组被挤到后面；整串优先、前缀兜底两级，`x` 仍然能一次看全，`xl` 只出希腊小写。
- **默认关**：最强理由是不改现有用户按 Shift+S 打大写 S 的习惯（`S` 开头的英文、S 形词）。不用：Windows 正式版出厂就是开，这一片的目标是补齐对等行为；关掉时用户没有任何入口知道有这个功能。退路是改一个 `Default` 布尔值，偏好键已经就位。

## Consequences

- **收益**：`xl`、`sx`、`szq`、`jt` 等 87 个分组码和 `alpha`、`infinity` 等符号名在六个平台同一套规则，都走 host-api → input-runtime → engine；数据编进引擎，不读磁盘、不依赖词库代次。
- **代价**：
  - 默认开，升级后句首或中英混打时按 Shift+S 会进入符号模式，`S` 之后打出的字母变成符号检索，不再是大写 S 开头的英文词。这是与 Windows 正式版一致的行为，用户可在设置里关掉，或用总开关一次性关掉所有 Shift+字母模式。
  - 表是生成物（43 KB），改表要发引擎版本；上游改动要重新生成并复核样例数。
  - 一个组最多 214 行（康熙部首），候选列表会长，靠翻页。
- **没做的**：
  - **Windows、iOS、Android 原生设置页及 macOS 原生回退设置**没加这一行。macOS、Linux、HarmonyOS 共享设置页已经能改 `local_modes.symbol`；原生设置入口需要在后续切片补齐。Linux 没有独立的原生设置页。
  - **移动宿主的菜单**没加符号入口：iOS、Android、HarmonyOS 的触屏模式表仍是原来的项。屏幕键盘进符号模式的路径与实体键盘相同（发带 Shift 的 `S`），但菜单里没有磁贴，这一片没做。
  - **Windows 的 `utility.symbol_mode` 配置键**没有映射：`scripts/test-reference-config-coverage.py` 里已经加上 `utility.symbol_mode → local_modes.symbol` 的映射（181 条），所以这个键不再被报成无人应答；Windows 原生设置页那一行仍缺。
  - 未做物理设备与蓝牙键盘验收；Android 模拟器证据见下方验证，其他平台的系统入口仍未验收。
- 已检索的相关笔记：[打拼音出符号](2026-10-13-symbol-candidates.md) 是 `symbol_candidates.tsv` 与「来源复用 Emoji」的来源，本篇复用了它的查询函数、写法规则和偏好写法；[内置拆字反查](2026-10-13-builtin-chaizi-mode.md) 是「编进引擎的本地表 + CC BY 署名 + 声明渠道」这套做法的先例，本篇的生成器、锁文件与契约检查照它写；[Shift+字母进入快捷模式的总开关](2026-10-11-shift-letter-entry-switch.md) 是入口判定的位置，符号模式一并受它约束。

## Verification

审查修正后，精确分组 `xl` 的结果与希腊小写组完整相等，非精确 `x` 保留多组前缀查询。引擎符号定向测试、engine/runtime/host-api 严格 Clippy、完整 quick 门禁通过（构建机 run `20261021-200130`），Linux 原生 108 项通过。共享设置页 400 项、类型检查、生产构建、笔记校验与 Fcitx 标签契约通过；Windows notices 的合成目录补齐新增许可。使用真实固定资源暂存后重新生成 iOS 工程，许可已加入资源构建阶段。

Android 原生 APK 与合成编辑器已构建，API 28 的四个共享客户端 crate 设备单测通过；API 28 装包主流程及 API 35 整体验证仍在运行，完成后补充最终结果。生成表检查因上游缓存缺失跳过下载比较，不把它记作重新下载生成已验证。

设备回归首次在注音九键数字网格遇到跨帧取坐标：逐键等待得到相邻键纵坐标相差 1 像素，同包完整数字专项复跑通过。几何验收改为一次收集整组九键坐标、整组连续两拍稳定后仍严格比较同行同列，不增加容差；新测试包的设备复核单独记录。

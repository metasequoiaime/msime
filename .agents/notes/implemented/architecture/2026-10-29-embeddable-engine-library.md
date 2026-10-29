# Agent Note: msime-engine 作为可嵌入的输入引擎库：MPL-2.0、C ABI 与数据发布

Status: implemented

## Problem

`crates/engine`（`msime-engine`）原本只给本仓库的宿主用：唯一的 C ABI 是 host-api 的 `msime_client.h`，它把打字和账号、云、语音、皮肤混在一个 1368 行的头文件里，每次按键都走 JSON，错误只有字符串；引擎本身的公开面分成「镜像 C++ API 的核心层」和「产品门面 `host::*`」两层，后者在构造时写侧车文件、`!Send`、命令编号和核心层不同。整个 crate 继承工作区的 `GPL-3.0-only`，App Store 应用和非 GPL 的输入法都不能嵌入它；librime 是 BSD-3。

维护者 2026-10-29 决定把引擎做成给其他输入法用的库。要回答的是：库的定位、许可证和仓库形态；改许可证的法律前提是否成立；编进引擎的 GPL 数据怎么处理；对外 ABI 的形状和 ABI 稳定之前引擎要先修什么；数据格式怎么交给第三方；怎样发布。

最硬的约束在版权上：仓库没有 CLA 或 DCO，所有贡献都是以 GPL-3.0-only 进来的；Rust 引擎是 GPL-3.0 的 C++ `metasequoiaime/msime-engine`（固定在 `a9b9f09`）逐行引用的移植，原作者 `fanlusky` 持有该提交上约 32% 的存活 C++ 行，其中包括大部分被移植的逻辑。

## Decision

`msime-engine` 是一个开箱即用的拼音与多语言输入引擎库，代码按 MPL-2.0 发布到 crates.io；`msime-engine-capi` 在它的核心层外面包一层带版本的 C ABI，以 crates.io 上的 crate 和 GitHub Releases 上的预编译库两种形式分发；数据不随引擎改许可证，按各自的许可证另行分发。msime 自己的产品 crate 也只经文档化的引擎 API 使用引擎。各部分由下面列出的 PR 落地，细节在各自的笔记里，本篇只记整体的决定和它们之间的关系。

### 定位、许可证与仓库

- **定位：开箱即用的高质量拼音与多语言引擎**，卖点是已有的能力：词格加神经重排、触点与拼写纠错、九键、学习。它不是 Rime 式的方案驱动框架，没有方案 DSL。C ABI 覆盖词格整句、拼写纠错、调频与学习，加 10 个方案；九键、触点纠错、神经重排与搭配模型只在 Rust crate 里可用（经 `SessionOptions` 的 `sentence_association`、`collocation_model` 等字段），C ABI 留到以后的 minor 版本。对第三方的自定义承诺只有 `ShuangpinCustomTable` 和个人词典：引擎没有通用码表方案（`SchemeType` 是封闭的 10 变体枚举，通用码表解析器只在 dict-builder 构建期），脚本运行时 `msime-script-runtime` 是 GPL、`publish = false`、只在桌面编译，不随引擎库发布。
- **许可证**：`msime-engine` 和 `msime-path-trust` 是 MPL-2.0，不附 Exhibit B，按 MPL 3.3 与 GPL 兼容（#10193）；`msime-engine-capi` 自己的文件只有维护者的代码，从建 crate 起就是 MPL-2.0（#10201）；仓库其余部分仍是 GPL-3.0-only。做法沿用 `crates/script-sdk` 等 MIT 例外的先例（[脚本插件工具链](../../implemented/feature/2026-10-26-script-plugin-toolchain.md)、[脚本插件合集](../../implemented/architecture/2026-10-12-script-plugin-collection.md)）：每个 crate 有自己的 LICENSE（开头是 Exhibit A 声明），`license` 字段不从工作区继承并加注释，README 与 `docs/third-party.md` 写明例外范围。引擎的 `license` 是 `MPL-2.0 AND BSD-3-Clause AND BSD-2-Clause AND CC-BY-3.0 AND Unicode-3.0 AND Apache-2.0 AND Apache-2.0 WITH LLVM-exception AND MIT AND WTFPL`：代码是 MPL-2.0，其余各项是编进引擎、保留各自许可证的数据和改编代码，逐个文件列在 `crates/engine/README.md` 的「许可证」一节，全文副本在 `crates/engine/licenses/`。只给测试读的夹具（`tests/golden/real`、`src/handwriting/testdata`）仍是 GPL-3.0-only，用 `include` 白名单排除出包。
- **入站贡献**：`.github/CONTRIBUTING.md` 的「许可证」一节规定，向 `crates/engine`、`crates/path-trust` 提交的贡献按「GPL-3.0-only OR MPL-2.0」授权；PR 模板有一条改到这两个目录时必勾的确认项；GPL、LGPL 许可的代码和数据不能编进引擎。`scripts/test-engine-license.py` 固定这些写法、许可证表达式与 crate 里实际编进的数据一一对应、非测试代码没有 crate 外的 `include_str!`、各声明渠道带着 MPL-2.0 与各数据的全文。
- **仓库：留在 monorepo。** 三个 crate 从同一个仓库发布；引擎改动和宿主改动仍在同一个 PR 里原子落地。

### 改许可证的前提：版权人同意

改许可证需要每一位版权人的书面同意。#10167 合入时，按 `git blame -C -C -M`、上游 `a9b9f09` 的 blame、msime-windows 的提交历史和 Rust 注释里的 C++ 行号引用做了技术比对（不是法律意见），把版权人分成三类：必须同意的 `fanlusky`（C++ 原作者，被移植文件里占绝大多数）、`jsfaint`（上游 C++ 1,839 行，octagram 与自动纠错生成器的移植来源）、`apple-ouyang`、`EwigMidori`（`lattice/decode.rs`、`lattice/merge.rs` 的骨架）；同意或重写其贡献的 `ManwoodIsolation`、`cyj-hashtag`、`SlimeNull`、`alex3236`、`yyy735934-prog`、`ouchihao`、`PathGao`；无需同意的维护者本人（`houko`）、只改 Windows 宿主的贡献者、机器人与 AI 署名行，以及只借了思路的 libchewing、libpinyin。逐人逐文件的比对表留在 #10167 合入时的本篇里（`15fc3c45f5`）。

当时的退路是：`fanlusky`（或 `jsfaint`、`apple-ouyang`、`EwigMidori`）不同意时改许可证作罢，库以 GPL-3.0-only 发布。净室重写不现实：按 C++ 行号或缩写引用 C++ 的 Rust 文件有 80 个、约 9.5 万行，覆盖每个中文方案的核心，维护者和现有贡献者都读过 C++，没有资格做净室作者。

**同意结果（2026-10-29）。** 维护者确认上述「必须同意」和「同意或重写」两类版权人全部同意把各自被移植或包含进 msime-engine、msime-path-trust 的贡献以 MPL-2.0（不附 Exhibit B）再许可。同意记录由维护者留存，本仓库不保存联系方式或同意书原件。退路因此不再需要。

### 编进引擎的数据与改编代码（P1，#10186、#10188）

- rink-core 不开 `bundle-files`，GPL-3.0-or-later 的 GNU Units `definitions.units` 不再编进引擎；`local/units.rs` 用 `Context::new()` 加本项目按法定值自写的 19 行 `BASE_DEFINITIONS`，再加载市制定义。
- LaTeX 命令名表（MIT）作为第三份副本同步进 `crates/engine/src/local/latex_symbols.tsv`，非测试代码不再从 crate 外 `include_str!`；符号名称表在 msime-dictionary 改为 MPL-2.0 后重新同步。
- 符号模式的分组表 `symbol_groups.tsv` 只用 Unicode 字符数据库的事实重建（#10188，[符号模式分组表改由 Unicode 字符数据库重建](../../implemented/feature/2026-10-29-symbol-groups-from-unicode.md)），不再内嵌由 rime-prelude `symbols.yaml`（LGPL-3.0）派生的分组与顺序，按 Unicode-3.0 声明。
- 源码出处写全：`lattice/octagram.rs` 写明 librime-octagram（BSD-3）、msime-windows 的 `jsfaint` 代码（已随引擎以 MPL-2.0 再许可）与 darts-clone（BSD-2-Clause）；`lattice/cxx_sort.rs` 写明逐步复现自 libc++（Apache-2.0 WITH LLVM-exception），许可证全文接进各声明渠道；`zhuyin/` 的注释不再把本项目的平方和计分称为 libchewing 的算法。
- `nine_key/syllable_frequency.tsv`（424 个音节各自在 `msime-pinyin.db` 全部词条里的权重之和）按聚合统计事实保留，`docs/third-party.md` 与引擎 README 写明来源。

### C ABI（msime-engine-capi，P4）

新 crate `crates/engine-capi` 只包核心层 `msime_engine::Session`，不包 `host::Session`，也不扩展 host-api（#10201）。约定在 [引擎的 C ABI](../../implemented/architecture/2026-10-29-engine-capi.md)：`struct_size` 版本协商且同一主版本内只追加，`int32_t` 返回码加线程局部的错误说明，热路径上借用的字符串、不用 JSON，按下标取候选，选择类函数带候选修订号，会话可跨线程使用但不能并发（重叠调用返回 `BUSY`），panic 只作废自己的会话。构建用 cargo-c，头文件由 cbindgen 生成并提交，CI 检查漂移，Linux SONAME 的主版本等于 ABI 主版本。

ABI 1.0 有 33 个函数；ABI 1.1（#10292，[引擎 C ABI 1.1 补上的缺口](../../implemented/architecture/2026-10-30-engine-capi-abi-1-1-additions.md)）只追加，补上按方案准备数据、调频、emoji 与颜文字混排、本地模式开关、`candidate_key`、词条检查和日志回调，共 38 个函数。不进 ABI 的：在线、云与 AI 候选，插件指令，翻译与释义，同步与备份，排序内部（`ordering::*`、重排模型），九键、触点、手写等实验性能力。第三方拿到的是引擎原始顺序，`sentence_alternatives` 固定为假。

### 引擎侧加固（P3）

- 会话收尾在核心 `Session` 的 `Drop` 里：冲刷本日志队列、交还日志连接；日志连接可以跨线程关闭；核心会话自己维护候选修订号；冲刷线程有 `shutdown`（#10189，[核心会话收尾、候选修订号与日志连接的线程归属](../../implemented/architecture/2026-10-29-core-session-lifecycle-and-journal-threads.md)）。#10189 引入的并发关闭竞争（后到的关闭方在先到的一方还没关完连接时就返回）由 #10236 修正：关闭返回前等被别处取走的连接也关完。
- 引擎库只有一套默认值（`SessionOptions::new`，辅助码默认关、表名 `wubi86`，因为其余六张辅助码表没有再分发授权），`ErrorCode` 与诊断码，可注入的日志出口 `set_log_sink`，跨进程维护锁归引擎（#10195，[引擎库的一套默认值、错误码、日志出口与维护锁归属](../../implemented/architecture/2026-10-29-engine-library-defaults-errors-log-and-lock.md)）。
- 回车上屏时学英文词的策略是核心选项，代次准备按方案集合只要求用到的词库，非拼音方案不再硬性要求 `msime-english.db`（#10269，[回车学词策略进核心，代次准备按方案集合要求词库](../../implemented/architecture/2026-10-29-engine-core-raw-commit-policy-and-scheme-data-sets.md)）。

### 数据格式与数据发布（P5）

`docs/data-formats.md` 是运行时每个文件（词库、模型、学习日志、代次目录）的格式、版本检查、产出方与数据许可证的规格；主库、五笔库与 others 库带 `metadata.format_version`，读端不认识就拒绝（#10187，[主库、五笔库与 others 库的文件内版本](../../implemented/architecture/2026-10-29-main-dictionary-in-file-version.md)）。

数据的许可证跟着数据走：默认中文库是 rime-ice 的 GPL-3.0，n-gram 计数来自 zhwiki（CC BY-SA 4.0），其余逐文件见 `docs/data-formats.md`。没有再分发授权的辅助码表不进任何数据发布。最小全拼数据包随 capi 的预编译归档发布，每个文件带它的许可证文本（#10240，见下）。

### 发布（P6）

- 打包：三个 crate 各有自己的版本号，`include` 白名单，crates.io 元数据，`#[doc(hidden)]` 隐藏产品模块 `host`、`backend`、`ordering`、`web`（#10198、#10232）；ewts 升到含上游下溢修复的 0.1.4，第三方的 debug 构建不会在藏文 `bw`/`yw` 上 panic（#10198）。PR 上 CI 用 `cargo package` 校验三个 crate 的打包结果能编译（#10251），以 `-D warnings` 生成 rustdoc（#10263，[发布 crate 的 rustdoc 在 CI 上零警告](../../implemented/process/2026-10-29-engine-rustdoc-in-ci.md)）。
- 发布工作流：`.github/workflows/release-engine-crates.yml` 统一处理三个 crate，经 Trusted Publishing 上传，crates.io 上还没有的版本才发，已有的版本与本提交的包逐文件比较、不一致就失败（#10256）。
- 已发布：`msime-path-trust` 0.1.0、`msime-engine` 0.1.0、`msime-engine-capi` 0.1.0 于 2026-10-29 从 `f00db90064` 用令牌手动首发（Trusted Publishing 只能给已存在的 crate 配置）；engine 与 capi 的 0.1.1 由工作流发布；`msime-engine` 0.2.0 与 `msime-engine-capi` 0.1.2 于 2026-10-30 由工作流发布（#10298）。引擎跳到 0.2.0，是因为 #10269 给 `SessionOptions` 加了公开字段，用结构体字面量构造它的下游会编译失败；capi 的 ABI 1.0 到 1.1 只追加，所以是 0.1.2。
- 预编译 C 库：`release-engine-capi.yml` 构建 Linux x86_64、macOS universal、Windows x86_64 MSVC 三个归档和最小全拼数据包，并附快速上手示例（#10240，[msime-engine-capi 的预编译归档、最小数据包与快速上手](../../implemented/process/2026-10-29-engine-capi-prebuilt-release-and-data-pack.md)）。GitHub Release `msime-engine-capi-v0.1.1` 的数据包在 #10266 恢复 rime-wanxiang 署名后重新上传；`msime-engine-capi-v0.1.2` 带 dict-v2.0.41 的数据包。

### msime 宿主自用（P7，#10209）

input-runtime、host-api、engine-wasm、backend-engine 只经文档化的引擎 API 使用引擎，`host` 门面只剩产品胶水；dict-builder 是构建工具，可以继续用内部接口（[msime 自己的产品 crate 只经文档化的引擎 API 使用引擎](../../implemented/architecture/2026-10-29-engine-host-dogfood.md)）。

engine-wasm 保留自己的分页。#10167 的 P7 一行写了「engine-wasm 去掉自己的分页」，但同一份方案的 P3 定的是「分页留给宿主，`candidate_get` 按下标随取」：引擎不知道页大小、高亮和页面怎么画，网页引擎本身就是一个宿主；它也不能改用 input-runtime 的分页，那要把整个 `Runtime`（在线候选、本地模式、供应方进程）拉进 wasm。P7 那一行因此被 P3 的结论取代。

### 声明与许可证合规的后续

改许可证和打包时把各渠道的声明重新核了一遍，补上了几处缺口：词库里 MIT 与 Apache-2.0 数据的许可证全文随六个平台分发（#10264，[词库里的 MIT 与 Apache-2.0 数据随各平台通知渠道附上全文](../../implemented/process/2026-10-29-dictionary-data-permissive-licence-texts.md)）；词库里改编自 rime-wanxiang 的权重恢复 CC BY 4.0 署名（#10266，[词库里改编自 rime-wanxiang 的权重重新署名](../../implemented/process/2026-10-29-rime-wanxiang-weights-attribution.md)）；笔画与注音词库的许可证表达式写全来源，移动端带上词库来源说明（#10270，[语言词库的许可证表达式写全来源，移动端带上词库来源说明](../../implemented/process/2026-10-29-language-dictionary-licence-expressions-and-mobile-notice.md)）；iOS 与 HarmonyOS 随包带上水杉输入法的 GPL-3.0 全文（#10277，[iOS 与 HarmonyOS 随包带上水杉输入法自己的 GPL-3.0 全文](../../implemented/process/2026-10-29-mobile-gpl-licence-text.md)）；msime-dictionary 发布 dict-v2.0.41，词库锁随之更新，笔画与注音词库带上改正后的许可证（#10291）。

### 评测

engine-wasm 的 `parity.rs` 在 runtime 一侧对齐首屏门槛并关掉行内日期时间（#10268）；`Runtime` 在建立和换引擎时下发每页候选数，`convert_eval` 默认按出厂配置（#10283，[评测按宿主的每页候选数和本地模式配置](../../implemented/testing/2026-10-29-eval-page-size-and-local-modes.md)）。两者合入后，在 dict-v2.0.41 上按新的评测设置重跑了 13 个评测集、打字流和双拼触点词级评测，结果与仓库里的基线逐字节相同；要跑几个小时的双拼触点整句集没有重跑。

### 文档与入口

仓库首页 README 有引擎库的用法与入口（#10262）；README 与 crate 主页指向官网的引擎库页面 https://msime.app/engine/（#10290，页面本身是 metasequoiaime/msime-web#723）。

### 已拍板的问题（2026-10-29）

1. `syllable_frequency.tsv` 算不算 rime-ice 的派生物：按聚合统计事实保留并写明来源。用 Unihan `kHanyuPinlu`（Unicode-3.0）原始计数加缺失音节下限重建的原型在九键评测集上退步，没有采用。
2. 默认数据（中文库 rime-ice GPL-3.0、n-gram CC BY-SA 4.0、全拼双拼五笔要求的 `msime-english.db` 含 rime-ice-en GPL-3.0）不能让 App Store 采用方直接带上：引擎是 MPL，数据按各自许可证，由采用方选数据。宽松许可的最小数据集另立项目，它的前提（非拼音方案不再硬性要求 `msime-english.db`）已由 #10269 满足。
3. `fanlusky` 不同意时库还做不做：全部版权人已同意，无需决定。
4. C 库的构建工具：cargo-c。
5. `symbol_groups.tsv` 经 rime-prelude `symbols.yaml` 的授权链：按 LGPL-3.0 对待，只用 Unicode 字符数据库的事实重建分组（#10188）。

### 与 librime 对照

照搬：`struct_size` 版本协商且只追加（Rime 的 `data_size`，改用定宽类型）；每种返回分配配一个释放函数；不透明的会话句柄（用类型化指针，不用 `uintptr_t` 加全局表）；按下标取候选；带 `void* user_data` 的回调；`version()` 返回静态字符串，头文件按字段标注引入版本；SONAME 的主版本长期不动；pkg-config 让前端按版本做编译期开关。

不照搬：部署与维护流水线（`deploy_*`、`start_maintenance`，为编译 YAML 方案而存在，数据构建留在离线的 dict-builder）；通用 `config_*` 树和方案列表 API；进程级 `setup`/`initialize`/`finalize` 和全局通知回调；以 X11 keysym 加修饰掩码作按键契约（引擎有语义按键 `SchemeKey` 和 `Command`，键位映射是前端的事，与 libhangul、libchewing 一致）；静态构造器注册插件模块；`#define Bool int` 和没有大小头的冻结结构体；字符串选项名。

### 相关笔记（审计）

上文各节链到的实现笔记各自记录一个阶段的细节，本篇是它们的总纲，彼此部分重叠、互链。另有：[九键音节频率表](../../implemented/bug-fix/2026-10-27-nine-key-syllable-frequency-table.md) 记录该表的生成与被否的数据包方案，本篇只处理它的许可证；[宿主注入会话存储](../../proposed/architecture/2026-10-16-host-injected-session-storage.md) 讲 host-api 的账号 ABI，与引擎 ABI 无关；[符号数据归词库仓](../../implemented/process/2026-10-25-symbol-data-in-dictionary-repo.md) 的副本规则由 P1 扩展到引擎里的 LaTeX 表；[符号模式](../../implemented/feature/2026-10-21-symbol-mode.md) 记录了 `symbols.yaml` 授权链的由来；[内置拆字模式](../../implemented/feature/2026-10-13-builtin-chaizi-mode.md) 是 chaizi CC BY 问题的出处；[日文模型进程级缓存](../../implemented/architecture/2026-10-10-japanese-model-process-cache.md) 与进程级状态有关；[有符号单位换算](../../implemented/bug-fix/2026-10-16-signed-unit-conversion.md) 和 [单位目标分配预算](../../implemented/testing/2026-10-07-unit-target-allocation-budget.md) 拥有 P1 去掉 `bundle-files` 时用作判据的 `units.rs` 测试；[对 ewts 关掉溢出检查](../../implemented/bug-fix/2026-10-11-ewts-overflow-checks.md) 记录了 P6 升级 ewts 之前的绕法，以及升级后删掉 `[profile]` 覆盖。没有被本篇吸收或取代的笔记。

## Alternatives considered

- **Rime 式框架，带方案 DSL。** 最强的理由是覆盖面：第三方能自己定义方案，不必等我们加。不用：`SchemeType` 是封闭的 10 变体枚举，序号进了在线身份和宿主 ABI；`ProviderRegistry` 按方案硬编码字段（全拼、双拼、五笔、粤语、注音、笔画各有自己的字段）。改成 DSL 驱动等于做一个新产品，去和 Rime 已有的方案生态正面竞争，而我们的优势（词格、神经重排、纠错、九键）恰好是 DSL 表达不了的。
- **把引擎搬到独立仓库。** 最强的理由是外部贡献者和发布节奏与 GUI 宿主分开，许可证边界一目了然。不用：ARCHITECTURE.md「上游 Engine」一节记录了引擎是有意搬进本仓库、去掉锁文件和 overlay 这一层的；引擎改动和宿主改动在同一个 PR 里原子落地，拆仓库会把这层重新带回来。MIT 例外的先例表明，同仓库内按 crate 分许可证是可行的。
- **先把 msime-engine 拆成多个内部 crate。** 最强的理由是对外 API 面更小、各部分可以独立发布。不用：模块之间有环，`ime` 是所有方案的枢纽，方案又反过来依赖它，`session` 和 `user_dictionary` 互相引用；拆 crate 要先解环，工作量大，而第三方要的是一个稳定的 C ABI 和一个 crate，不是内部结构。产品模块改用 `#[doc(hidden)]` 隐藏，而不是放到 feature 后面，是因为 host-api、input-runtime、backend-engine、engine-wasm 都直接用它们，放到 feature 后面要改一串依赖声明。
- **库保持 GPL-3.0-only。** 最强的理由是不需要任何人签字，当时就能发布。不用：它挡住 App Store 应用和所有非 GPL 的采用方，而这正是做库的目的；librime 是 BSD-3。它曾是版权人不同意时的退路，全部同意之后不再需要。
- **扩展现有 host-api 的 C ABI（`msime_client.h`），不另起 capi。** 最强的理由是它已有、六个平台在用，句柄注册表、`catch_unwind`、重入保护都做好了。不用：它是 GPL 的客户端门面，混着账号、云、语音、皮肤和遥测；每次按键走 JSON；错误只有字符串；「ABI 3、只追加」没有功能探测；严格线程绑定。它的好做法（句柄、`catch_unwind`、重入检查、单一释放函数、带 generation 的选择）搬进了 capi。host-api 的 ABI 3 编号（如 `MSIME_OPEN_CANDIDATE_LIST = 16`）不变，平台宿主无需迁移。

## Consequences

- **收益**：App Store 应用和非 GPL 的输入法可以嵌入引擎：Rust 下游从 crates.io 依赖 `msime-engine`，C 下游用预编译库或 `cargo cinstall` 拿到带版本的头文件、`.pc` 和共享库或静态库，再配一个带许可证文本的最小全拼数据包就能打字。C ABI 由经 C 函数回放 golden 场景的测试守着（#10257，[引擎 C ABI 回放 golden 场景，逐步对照核心 API](../../implemented/testing/2026-10-29-engine-capi-golden-replay.md)）：300 个场景里回放 240 个，每一步与核心 API 的回放逐字段对照，跳过的 60 个按原因列出，跳过表就是 ABI 的缺口清单。msime 自己的产品 crate 只经文档化的 API 使用引擎，公开面的回归会先在自家宿主上暴露。
- **代价**：已经发出的 MPL 版本收不回，以后任何 GPL 或 LGPL 的代码和数据都不能编进引擎，CONTRIBUTING 与 `test-engine-license.py` 把这条变成了规则。引擎的公开 Rust API 成了 crate 的 semver 面：给 `SessionOptions` 加公开字段就是不兼容的改动（0.2.0 就是这样来的），产品需要的新选项都要按这个节奏发版。C ABI 的命令与枚举编号已冻结，只能追加。crates.io 上的包跑不了测试（测试读仓库外的文件），`rusqlite` 开 `bundled` 使下游必须对齐 rusqlite 大版本，一个进程只能链接一个 Rust staticlib。三平台的 capi 作业和发布工作流多占 CI，其中有 macOS runner。
- **验证**：P1 到 P7 各自的验证写在对应的 PR 与笔记里。贯穿的检查是 `scripts/test-engine-license.py`（许可证表达式、Exhibit A、`include` 白名单、各渠道的许可证全文、入站双授权条款）、PR 上的 `cargo package` 与 rustdoc `-D warnings`、`ci-platforms.yml` 的 `engine-capi` 作业（三平台上 `cargo test -p msime-engine-capi`，含 golden 回放，以及 `tests/c-example.sh` 的头文件漂移、SONAME 和静态、动态链接的 C 示例），和发布工作流里与 crates.io 已发布版本的逐文件比较。
- **没做的与未决的**：
  - 候选重排不公开：第三方拿到的是引擎原始顺序，msime 的产品经 `#[doc(hidden)]` 的 `ordering` 模块排序。要把重排给第三方，得先定它的公开 API 和模型分发。
  - HarmonyOS 在应用里没有随包许可证文件的列表：「关于 › 法律信息 › 开源许可」打开 GitHub 上的 `LICENSE`，`resfile/licenses/` 里的文件只在 HAP 里（见 [iOS 与 HarmonyOS 随包带上水杉输入法自己的 GPL-3.0 全文](../../implemented/process/2026-10-29-mobile-gpl-licence-text.md)）。
  - `msime-others.db` 的 emoji 数据来源没有记录（`docs/data-formats.md` 的数据许可证一节），这份文件再分发前要另行确认。
  - `typing_sim --profile convert-eval` 仍是 #10283 之前的 `convert_eval` 设置（每页 9、行内日期时间开着但钉在虚拟时钟上），要与现在的 `convert_eval` 对上需要另改并重跑它的参考臂。
  - 数据许可的几个问题从未经律师审阅：CC BY 与 CC BY-SA 数据和 GPL-3.0 词库合在一起分发的关系，以及 App Store 这类带 DRM 的渠道与 CC 许可证技术措施条款（CC BY 4.0 第 2(a)(5)(C) 条等）的关系。
  - 版权比对里标为未核实的几项没有补完：sunpinyin 源码没有比对；msime-windows `engine/` 里 `fanlusky`、`jsfaint`、`ouchihao` 之外的贡献者（HowGeGe、chr331、沐雪清泽等）是否进了 Rust 没有逐行查；改许可证之前对最终的树重跑 `git blame -w -C -C -M` 并统计非 AI 的 `Co-authored-by` 尾行这一步，#10193 也没有做。若查出新的版权人，要补同意或重写其贡献。

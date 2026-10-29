# Agent Note: 符号模式分组表改由 Unicode 字符数据库重建

Status: implemented

## Problem

符号模式（Shift+S）的分组表 `crates/engine/src/local/symbol_groups.tsv` 原先由 `scripts/build_symbol_groups.py` 从 rime-wanxiang 的 `wanxiang_symbols.yaml`（CC BY 4.0）生成，取的是「分组码 → 码位集合」以及每组的码位顺序，见[符号模式](2026-10-21-symbol-mode.md)的「分组码来源与许可判断」。上游文件头写明它的分组最初抄自 Rime 的 `symbols.yaml`，而 rime-prelude 仓库是 LGPL-3.0，这条授权链从没审过。

引擎准备改以 MPL-2.0 作为可嵌入的库发布（设计笔记在 PR #10167，待拍板第 5 条）。维护者已定：这条链按 LGPL-3.0 对待。LGPL-3.0 的数据静态编进 MPL 库，对 App Store 采用方不可接受（LGPL-3.0 §4 要求能替换库），所以引擎不能再内嵌由 rime-prelude 派生的分组与顺序。

## Decision

- **组员只按 Unicode 字符数据库的事实挑选。** 输入是 UCD 17.0.0 的 `UnicodeData.txt`（字符名、通用类别、分解映射、大写映射）、`Blocks.txt`（区块）和 `EquivalentUnifiedIdeograph.txt`（部首到统一汉字的等价映射），许可是 Unicode License V3。`resources/symbol-groups.lock.json` 按 unicode.org 的地址固定三份文件的长度与 sha256，并记下 `resources/licenses/Unicode-License-V3.txt` 的摘要。生成器不调用 Python 自带的 `unicodedata`，结果只取决于锁定的输入。
- **分组码、中文组名和组的先后不变**，仍是 87 组。变的是每组收哪些字、按什么顺序。中文组名和组的先后一直是本项目写的；分组码不是：87 个编码里有 54 个（`sx`、`jt`、`szq`、`xl`、`pp`、`kx`……）与 rime-prelude `symbols.yaml` 的编码逐字相同，其余 33 个（`dy`、`xy`、`yb`、`sxc`–`sxd`、`xlc`–`xlwz`、`fj`、`jz`、`bdzy`……）是 rime-wanxiang 加的，「有哪些组」这套概念也来自这两份表。这里有意保留它们，好让符号模式的用法不变（Windows 正式版出厂配置就点名了 `xl`、`sx`、`szq`、`jt`）。它们是拼音首字母缩写，单个编码大概不构成受保护的表达，但整套编码是否也落在 LGPL-3.0 链里没有经过法务审阅，列在下面的「待拍板」里。
- **取字规则写在生成器的 `GROUPS` 里，由本项目编写。** 规则的写法有四种：整个区块（几何图形、方块、麻将、六十四卦），字符名的正则（带圈数字、罗马数字、电报月份），类别加区块（夹注符号取标点区块里的 Ps/Pe/Pi/Pf，货币取基本多文种平面里的全部 Sc），以及分解映射（上标、下标取 `<super>`、`<sub>`，康熙部首取兼容分解出的统一汉字，单位取 `<square>` 分解全是拉丁字母的方块字）。需要点名个别字时写 Unicode 字符名，例如单位里的 DEGREE CELSIUS、声调里的 MODIFIER LETTER MACRON。拼音一组按《汉语拼音方案》的字母与四声生成，没有预组合字时给两个码位的序列（m̄、ê̄）。
- **组内按码位排序。** 例外有四处，前三处的排序键由数据导出：上标、下标按分解出的基本字符排；俄文字母里带变音的字母（ё）跟在基本字母后面；数学字母数字各字体按区里的排法（大写 A–Z、小写 a–z、数字），区里的空位由字母类符号区里 `<font>` 分解与字体名相符的字补上（ℎ ℬ ℂ）。第四处是非夹注符号一组：GB/T 15834—2011 规定中文的破折号、省略号各占两个字，Unicode 没有对应的单个字符，所以按标准写成两个码位的 —— 与 ……，排在组首，和拼音一组按《汉语拼音方案》生成是同一种做法。旧表把大小写交错排（A a B b），这个顺序不再沿用。
- **几处按标准点名的字。** 电脑按键一组除了杂项技术区按名字挑的按键符号，还收箭头区里印在键帽上的 ISO/IEC 9995-7 符号（⇧ ⇪ ⇥ ⇤ ↩ ↵ ⇞ ⇟ ⇱ ⇲，按 Unicode 字符名点名）、⌧ 清除键和 ⏩–⏯ ⏸ ⏹ ⏺ 媒体键。单位一组的 OHM SIGN、ANGSTROM SIGN 有单码位的规范分解，NFC 会改写它们，所以收的是 Ω（U+03A9）、Å（U+00C5）；KELVIN SIGN 的目标字是键盘上的 K，不收。部首补充区的等价映射按段展开（`2EBE..2EC0 ; 8279`），艹、辶、小、爫 这几个成段等价的部首出的是统一汉字。
- **每组不超过 200 个符号**（`SYMBOL_MODE_LIMIT`，整串分组码一次最多出这么多行）。康熙部首是唯一的例外，它是 214 个部首的完整集合，旧表同样如此。数学符号一组为此去掉了带 WITH 的修饰形、SMALL 小号形、有兼容分解的连写形和有规范分解的否定形，否定形只留 ≠ ≮ ≯，得 197 个。常用符号一组收杂项符号区里没被其他组收走的字，得 195 个。生成器和 `symbol_groups.rs` 的单测都检查这个上限。
- **符号模式不再署名 rime-wanxiang。** 分组表不再取自它，`resources/licenses/rime-wanxiang-CC-BY-4.0.txt` 里关于这张表的说明随之作废。Unicode License V3 早已因拆字表的 Unihan 数据接进了这些渠道，这次只把它的说明扩到符号模式。当时以为这份文件只为分组表存在，删掉了它和十二个渠道里的引用；其实 `msime-pinyin.db` 与 `msime-stroke.db` 一直带着改编自 rime-wanxiang 的权重，文件已按权重的用途重写并接回全部渠道，见[词库里改编自 rime-wanxiang 的权重重新署名](../../implemented/process/2026-10-29-rime-wanxiang-weights-attribution.md)。
- **CI 逐字节核对。** `ci-platforms.yml` 的 Rust engine 作业按锁的摘要缓存 `target/symbol-groups-cache`，再以 `MSIME_FETCH=1` 跑 `scripts/test-symbol-groups.py`，做法与拆字表相同：设了 `MSIME_FETCH=1` 时下载失败会让检查失败，不会当作跳过。锁文件改动也会触发引擎作业。此前文档写着 CI 会核对这张表，但工作流里其实没有这一步。

### 数据放在哪

[符号数据统一由词库仓保管](../process/2026-10-25-symbol-data-in-dictionary-repo.md)管的是第三方符号表与本项目自写的名称表，它们要有一处保管、一处署名。UCD 是 Unicode 联盟按版本发布的标准数据，仓库里的 Unihan（拆字表、字释义）一直按锁直接从 unicode.org 取。所以这张表也直接从 unicode.org 取，不进词库仓。取字规则是代码，写在生成器里。这是对词库仓那条规则的一个例外，维护者还没有确认。

## Alternatives considered

- **保留 rime-wanxiang 的表，按 LGPL-3.0 处理。** 这条路最强的理由是零行为变化。没有采用，因为 LGPL-3.0 §4 要求库可以替换，而静态链接、上架 App Store 的采用方做不到。设计笔记里已经把这条判为不可接受。
- **把分组表改成运行时数据，不编进引擎。** 这条路最强的理由是引擎库可以干净地以 MPL-2.0 发布，数据保留原样。没有采用，因为 LGPL 链没有消失，只是转给了分发数据的那一方，六个平台的宿主还要各自加载和分发这份数据。待拍板第 5 条已定为按 Unicode 事实重建。
- **在 UCD 之外再引 NamesList.txt 的小标题做分组**（例如「ISO/IEC 9995-7 键盘符号」「星号与雪花」这类小节）。这条路最强的理由是组员会更贴近 Unicode 码表的分节。没有采用，因为三个输入文件已经够用。电脑按键一组直接写了 U+2380–U+2395 这一段，并在注释里注明它是 ISO/IEC 9995-7 键盘符号所在的码位段。
- **按 GB 2312 第 1 区挑常用数学符号**，旧表的 45 个数学符号大致就是这一区。这条路最强的理由是选出的符号更「常用」。没有采用，因为 GB 2312 不在 UCD 里，要多引一份标准的码表。改为数学运算符区加四条按 UCD 字段写的排除规则。

## Consequences

- **收益**：
  - 引擎内嵌数据里不再有 rime 系的分组或顺序，以 MPL-2.0 发布时少了一份要单独声明的数据许可。
  - 规则按 UCD 版本可重放：升级 Unicode 版本时只改锁，新增的字按规则自动进组，样例数的变化由 `test-symbol-groups.py` 报出来。
- **行为变化**（87 组共 4 585 个符号，旧表是 3 929 个；逐组比，两表共有 3 693 个；42 组成员与旧表相同，其中 24 组顺序也相同，逐组对照见 PR #10188 的说明）：
  - 变化最大的几组：`sx` 从 45 个增到 197 个，开头是 ¬ ± × ÷ ∀ ∁。`yb` 从 48 个英语音标（含 i: tʃ 这类双字母组合）变为 IPA 扩展区的 96 个字母加 æ ð ŋ θ ː，两表共有的只有 11 个。`dn` 从 45 个变为 96 个（杂项技术区的按键符号与媒体键、ISO/IEC 9995-7 段、箭头区的键帽箭头和控制图形区），共有 26 个。`pp` 改为部首补充区按等价映射换成的统一汉字（109 个），共有 35 个。`hw` 从 14 个辅音扩到兼容字母区的 93 个。`bd` 从 61 个变为 82 个，多了拉丁-1 的 ¡ § « ¶ · » ¿ 和全角标点。
  - 首个候选变了的组有 30 个，用户最容易察觉的是：`szq` 从 ⓪ 变为 ①，`jt` 从 → 变为 ←，`sx` 从 ＋ 变为 ¬，`dw` 从 Å 变为 °，`dn` 从 ❖ 变为 ↩，`bd` 从 、 变为 ¡，`fj` 从 ⸺ 变为 ——，`zmq` 从 ⓐ 变为 Ⓐ（大写在前）。
  - 变小的组：`xb` 从 84 个降到 38 个，旧表混入的小型大写字母（ᴀ ʙ）不是下标，不再收。`bdz` 从 41 个降到 33 个。
  - 还有一些组多出了 Unicode 新版加的字，例如 `bh` 多了 ㇤ ㇥，`jg` 多了 ⿼–⿿ 和 ㇯。
- **从符号模式的任何一组都打不出来的旧符号**（按组列出旧表里有、新表 87 组里都没有的符号；名称查法 `symbol_candidates.tsv` 与 `latex_symbols.tsv` 只能补回其中一部分，例如 `sum`、`leq` 一类）：
  - `sx`：全角 ＋ ＝（全角 － ＜ ＞ 还在 `bd`、`dy`、`xy` 里）。
  - `sb`：˜ ᴯ ᵎ ᴻ；`sd`：ˆ ˜。它们没有 `<super>` 分解，也不是声调符号。
  - `xb`：33 个小型大写字母（ᴀ ᴁ ᴃ ᴄ ᴅ ᴇ ᴊ ᴋ ᴍ ᴏ ᴘ ᴛ ᴜ ᴠ ᴡ ᴢ 等）。
  - `yb`：37 个英语音标写法（i: ɑ: ɔ: u: ɜ: eɪ aɪ ɔɪ əʊ aʊ ɪə eə ʊə tʃ dʒ tr dr ts dz，以及 p b t d k g f v s z h m n l r j w e 这些 ASCII 字母）。
  - `fj`：⋯⋯ ｜；`jz`：⟨ ⟩；`bd`：˗ ؟。
  - `dn`：❖ ⌅ ⌀ ⌖ ⌗ ⌜ ⌝ ⌞ ⌟ ⍑，以及 ⌚ ⌛ ⏰ ⏱ ⏲ ⏳ 这几个手表、沙漏、闹钟。它们不是按键，没有放进电脑按键；常用符号一组已有 195 个，再收会超过 200。
  - `yy`：𝅗𝅥 𝅘𝅥𝅯 𝄀 𝄂。
  - `pp`：34 个不在部首补充区的统一汉字部件（丷 龹 龸 亼 亽 仒 冃 冄 冈 丆 壬 訁 釒 龺 兂 巜 龰 㞢 㣺 龵 牜 卝 龷 丗 龶 芈 丵 菐 龻 吅 㗊 㠭 㸚 叕），它们本身是普通汉字，用拼音或拆字也能打出来；另外旧表里的部首码位 ⺌ ⺶ 换成了等价的 小 羊。
- **代价**：
  - 依赖旧顺序的肌肉记忆会失效，比如 `jt` 的第一个候选、数学字母的 A a 交错排法。新顺序由码位决定，以后不会因为某个符号表的维护而漂移。
  - `kx` 仍有 214 个，最后 14 个部首用整串分组码打不出来，与旧表相同。这次没有调整 `SYMBOL_MODE_LIMIT`，因为它与 Windows 正式版的上限一致。
  - 网页引擎的声明（`web-engine-NOTICE.md`）原本既没有 rime-wanxiang 也没有 Unicode License V3。这次没有补，留给统一重写声明文档的那一步。
- **待拍板**：
  - 分组码与「有哪些组」从 rime-prelude、rime-wanxiang 的旧表保留下来。若维护者认为整套编码也属于 LGPL-3.0 链，就要另起一套编码，Windows 出厂配置里的 `xl`、`sx`、`szq`、`jt` 随之要改。
  - 上面列出的那些打不出来的旧符号，逐条看要不要按某个标准或 UCD 规则补回；规则在生成器的 `GROUPS` 里，补起来很容易。
  - 「数据放在哪」一节把 UCD 当作例外，直接从 unicode.org 取，不进词库仓。
- **后续**：msime-dictionary 的 `README.md` 第 133 行仍说 msime 的 `scripts/build_symbol_groups.py` 从词库仓读 `wanxiang_symbols.yaml`，本篇合入后要在词库仓改掉（或并入统一重写声明文档的那一步）。
- 已检索的相关笔记：[符号模式](2026-10-21-symbol-mode.md)的「分组码来源与许可判断」一节被本篇取代，其余决定（入口、查询顺序、来源复用 Emoji、默认开）照旧，那篇已加指向本篇的说明；[符号数据统一由词库仓保管](../process/2026-10-25-symbol-data-in-dictionary-repo.md)里分组表锁改从词库仓下载的那一条不再成立，那篇同样加了说明，「改符号数据只改词库仓」也限定为第三方符号表与自写名称表；[补齐希腊字母与数学符号名称](2026-10-25-math-greek-symbol-names.md)只用到 LaTeX 命令名表，与本篇无关。

## Verification

- 生成器从锁定的三份文件生成表，`--check` 逐字节一致。旧表与新表逐组比较，列出上面那份「打不出来的旧符号」清单。`scripts/test-symbol-groups.py` 在有缓存和无缓存（打印 skip）两种情况下都通过。
- 在构建机上跑 `cargo test -p msime-engine --lib -- symbol`，70 项通过，其中有 `local::symbol_groups` 的 7 项（含 `pp` 出 艹 辶、不出 U+2EBE，`fj` 以 —— …… 开头）和会话层符号模式的 5 项。`cargo clippy -p msime-engine --lib --tests -- -D warnings` 通过。
- 受影响的声明渠道检查都通过：`test-chaizi-table.py`、`test-language-data-notices.py`、`test-macos-package-resources.py`，以及 iOS 的 `ProjectConfigurationTests`（26 项）。各平台安装包没有重新构建，macOS `bundle_contents.py` 和 Windows `collect_notices.ps1` 只改了清单，没有实际跑过。

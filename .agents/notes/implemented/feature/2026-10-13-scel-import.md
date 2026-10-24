# Agent Note: 导入搜狗细胞词库（.scel）

Status: implemented

## Problem

用户要求「导入搜狗、讯飞的第三方词库」（metasequoiaime/msime-windows#684），另有用户手里只有搜狗导出的词库、转不成水杉能读的格式（msime-windows#778）。各平台的词库导入只吃文本（标准 TSV、Windows TSV、Rime、纯汉字词表），搜狗最常见的可下载词库是二进制的细胞词库 `.scel`，用户只能先找第三方工具转换。Android 词库页早就列出了「搜狗细胞词库（.scel）」并按 base64 发 `bytes_base64`，但 client-core 一律回答 `unsupported_format`。Windows 原生设置窗口的「词库」页更进一步，连文本文件的导入都没有。

## Decision

只有一个解析器：client-core 的 `dictionary::scel::decode` 把 `.scel` 字节转成标准 TSV 文本（每个词一行 `词<TAB>音节'音节`，不带权重列），之后完全走现有的文本导入，上限、去重、默认权重 10000、分批和导入报告都不改。它有两条入口：

- 命名词库的 `import` 动作接受 `format:"scel"` 加 `bytes_base64`（不带 `text`），在 client-core 里先转文本再按标准格式解析，集合来源记 `import{format:"scel"}`。Android 和鸿蒙手机走这条。
- 纯函数 C ABI `msime_client_dictionary_scel_text`（Rust 入口 `scel_text_json`，Tauri 命令 `dictionary_scel_text`，鸿蒙 NAPI `dictionaryScelTextAsync`）回答 `{name, category, entries, text}`，宿主再把 `text` 以 `standard` 交给自己现有的导入。共享设置页（macOS、Linux、鸿蒙 2in1、Tauri 移动壳）、iOS 原生 App 和 Windows 原生设置走这条。

### 坏词条与行号

文件第 k 个词就是输出第 k 行，所以任何下游报告里的「第 N 行」就是「第 N 个词」。坏词条不跳过，而是写成在每个下游都必定失败的一行：读音读不出（音节序号不在本文件的拼音表里）写 `词<TAB>`，按 `empty_key` 失败；词读不出写 `<TAB>读音`，按 `empty_value` 失败；两者都读不出写 `<TAB>'`，同样按 `empty_value` 失败（只有分隔符的行会被当成空行静默跳过）。词读不出包括孤立代理、任何控制字符（TAB、CR、LF 会拆行或拆列，让后面的行号错位）、U+FEFF（开头时被当成 BOM 剥掉）、首尾空白和以 `#` 开头（会被当成注释静默跳过）。音节数是否等于字数、音节是否在 Engine 的音节表里、是不是全是汉字，解析器不管，交给下游，待遇与文本导入相同。

### 格式与健壮性

布局按本机 13 个搜狗官方分类词库核对过（33 万多个词）：12 字节魔数 `40 15 00 00 44 43 53 01 01 00 00 00`，`0x120` 组数，`0x124` 词数，`0x130`/`0x338` 是 UTF-16LE 的词库名和类别，`0x1540` 是拼音表，`0x2628` 起是词条区。两点决定了实现：

- 拼音表必须按文件读。同一批官方词库里有两种表，第 230–232 号音节的顺序不同，内置一张表会把一部分文件的读音全读错。
- 每个词的扩展字段前 4 字节在每个样本里恰好是 1..n 的一个排列，是序号不是词频，所以不当权重用，一律用默认权重。

文件是不可信输入：只用 `get` 和 checked 运算读（模块开着 `clippy::indexing_slicing`），文件上限 8 MiB（`MAX_SCEL_BYTES`，由命名词库 17 MiB 的请求上限反推：base64 后约 10.7 MiB 加宿主选项），声明的组数不能超过剩余字节能装下的组数，声明的词数只用来预留容量且不超过剩余字节估出的值。文件上限管不住输出：同一组的每个词都把整条读音再写一遍，64 个音节的读音配上只占 6 字节的单字词，输出约是文件的 75 倍，一个不到 8 MiB 的文件实测转出 137 万词、进程常驻内存到 637 MB。所以转出的文本另有上限 `MAX_SCEL_TEXT_BYTES`（16 MiB，与命名词库一次导入的文本上限相同），边读边计数，一过就整份按「太大」拒绝。13 个官方样本的转出文本是文件大小的 0.74 到 1.04 倍，8 MiB 的上限文件也离它有一倍余量。前 4 字节不对是「不是细胞词库」，后 8 字节不是已核实的版本是「版本不支持」（社区资料提到的第 5 字节为 `E` 的变体没有样本，不去猜）；拼音表任何一条不合规、任何一次读取越过文件尾、任何字段超出范围都是「损坏」，整份拒绝，不做部分导入：下载不完整的词库只导入前一半，用户会以为导全了。声明的组数读完后剩下的字节不读。错误只有枚举值，不带词条内容。

### 各平台的入口

- 共享设置页：拼音词库的文件格式多一项「搜狗细胞词库 .scel（仅导入）」，宿主提供 `scelText` 时才出现；按扩展名或文件头的前 4 字节认出细胞词库（版本不对的也交给宿主去说「版本不支持」）。选文件时按扩展名换的格式作为参数传给这次导入，不读闭包里的旧格式，这也让第一次选 `.yaml` 时按 Rime 读。不分批的宿主（鸿蒙 2in1 和 Tauri 移动壳，设了 `maxImportFileBytes`）把一次导入整份作为一个请求，共享解析器只收 64 KiB 文本、只看 1000 行，而 13 个官方样本里 11 个转出的文本超过 64 KiB，整份送过去会被整份拒绝；所以页面只送转出文本开头的整行（最多 1001 行、不超过 64 KiB，`leadingImportRows`），有行没送时在报告上补「文件过长」。桌面分批导入收整份文本。
- 鸿蒙手机：`dictionary/add-dictionary-sheet.tsx` 的 `AddDictionarySheet` 复用底部 `ActionSheet`，只负责按传入来源打开文件选择器并回传所选来源与 `File`；设置页继续筛选宿主能力、读取与解析文件和执行导入。面板在选择器打开后保留隐藏输入框与来源引用，关闭面板和更新回调不会丢失稍后返回的选择，且每次回传前清空输入值以便重选同一文件。「添加词库」在格式清单含 `scel` 且宿主能传字节时列出「导入搜狗细胞词库」；会话只检查操作名、原样透传 `bytes_base64`。
- Android：沿用早已接好的来源，二进制文件的上限改为 8 MiB。
- iOS 原生 App：「词库文件」的格式多一项，选它时类型锁定为拼音；只把转出文本的前 1001 行（且不超过解析器一次收的 64 KiB）交给解析器，解析器一次只看 1000 行，多一行它才报告「文件过长」；词长的词库不到 1001 行就到 64 KiB，按字节截掉时由 App 补上「文件过长」。
- Windows 原生设置：「词库」页「本地词库管理」组补上「导入词库文件」（文本格式和细胞词库），规则与文案在 `platforms/windows/settings/LexiconImportModel.h`。
- 不接：云词库面板、macOS 进程内的原生词库后备窗口（只在设置应用打不开时出现，一次请求只收 64 KiB）、MCP 的 `import_dictionary_words`。

### 落盘格式

命名词库的 `IMPORT_FORMATS` 多了 `scel`，`index.json` 里可能出现 `source: {"type":"import","format":"scel"}`。记 `scel` 而不伪装成 `standard`，是为了留下「这个集合来自第三方二进制文件」的来源，供排查和以后的重新导入用。

## Alternatives considered

- **每个导入入口各自识别 `bytes_base64`**：字节一路送到 Rust 再解析，前端不用碰 base64 以外的东西。但导入入口有四个（`settings_dictionary` 的分批、`dictionary_request_json`、iOS 的 `queued_import`、命名词库），各自有分批、报告和上限，「把解析器的跳过数和行号并进导入报告」要写四遍，Windows 的 C++ 还要第五遍。先转文本，下游一个字不改就得到正确的上限、去重、分批和行号。命名词库是例外，因为 Android 已经按字节接好，在 client-core 里接住最省事，它内部同样是先转文本。
- **照 imewlconverter 的读法**：它读同样的偏移，是社区里最完整的转换器。但它是 GPL-3.0，而且用 `try/catch` 吞掉坏词条、把越界的音节序号当成英文字母，坏行既不计数也不报行号，与导入「坏行要计数、报行号，不悄悄导入」的规则相反。格式只有三张表，样本已核对每个字段，解析器独立编写，只拿它对照布局。
- **引入现成的 crate**：crates.io 上只有 `cidian-rs 0.1.1`（MIT，2026-08-07 首发，只有这一个版本，总下载约 400 次）。依赖一个单人维护、只发过一版的库，风险大于自己写三张表的解析器。
- **截断的文件导入能读出的部分**：对用户更宽容。但结构坏了的文件后面读出的内容本来就不可信，而部分导入会让用户以为导全了；重新下载就能解决。
- **把扩展字段换算成权重**：能保留词库里的先后顺序。但它是序号不是词频，换算等于给排在前面的词凭空加权。
- **讯飞解析器**：issue 只提了名字，没有样本也没有格式；能查到的讯飞导出是每行一个词的文本，今天的「汉字自动注音」已经能读大部分。放宽 `hans`、去掉行尾「空格 + 类型」会改变它对所有人的语义（`你好 世界` 会从失败行变成悄悄只导入「你好」），等有真实样本再定。百度 `.bdict` 和 QQ `.qcel` 没有人要求。

## Consequences

- **收益**：一个纯函数加一个 C ABI 让六个平台都能导入细胞词库，Android 现成的入口开始工作；所有路径共用一套计数，行号永远是词序；Windows 原生设置同时补回了文本词库的导入（导出仍缺）。
- **代价与已知上限**：
  - 只核实了 `DCS` 魔数，样本都是搜狗官方分类词库；自制、旧版或第三方工具写出的文件可能被当成损坏或不支持而拒绝，等用户报告再放宽。
  - 拼音表里有 3 个音节（`eng`、`rua`、`tei`）不在 Engine 的音节表里，用到它们的词由下游按行拒绝；样本里 33 万词中只有 2 个。
  - 桌面导入经 Engine upsert，细胞词库里与用户已有词相同的词，权重会被改成 10000；这是所有桌面文件导入的现有语义。要让「别人的大词表」不覆盖用户自己的权重，需要给 Engine 加一种「已有则跳过」的导入模式。
  - iOS 个人词库队列一次只收 128 个词，一个最小的细胞词库也有上千个词，所以 iOS 上这个来源只够试用；命名词库每个集合 2 万条，更大的词库导入前 2 万条并提示「词库已满」。
  - 降级：旧版本的 `validate_index` 只认旧的格式清单，读到含 `scel` 来源的 `index.json` 会按损坏处理（不改文件，提示「词库文件已损坏」）。Android 和鸿蒙的正式包降级要先卸载，只有侧载降级会碰到。
  - 8 MiB 的文件在前端是约 10.7 MiB 的 base64 字符串；使用[共享字节编码器](../simplification/2026-10-15-byte-base64.md)按 3 字节的倍数分块编码，不一次展开成参数。

## Verification

- client-core：`cargo test -p msime-client-core --locked --lib dictionary::` 覆盖往返、拼音表顺序、坏词条变成失败行且行号不错位、在每个字节位置截断都返回错误、固定种子的随机改写不 panic、各字段越界、小文件转出超过文本上限时按「太大」拒绝、命名词库的 `bytes_base64` 导入与错误码。`scel_reads_local_samples` 是 `#[ignore]` 的对照用例，读环境变量 `MSIME_SCEL_SAMPLES` 指向的真实词库目录，样本不进仓库。
- host-api：`cargo test -p msime-host-api --locked --lib scel` 与 `cell_dictionary` 覆盖 C ABI 的往返和错误码、桌面分批路径的跨批行号、iOS 队列路径的 128 条截断，以及 8 MiB 文件经命名词库请求不超过 17 MiB。
- 界面：`apps/desktop/tests/dictionary/` 与 `apps/desktop/tests/settings/harmony-phone-dictionary.test.tsx`；iOS：`PersonalDictionaryScelImportTests`；Windows：`tests/ui/lexicon_import_model.cpp`；鸿蒙接线：`scripts/test-harmony-dictionary-collections.py`。
- 测试夹具全部由测试代码现场拼出，内容是自造的词，仓库里没有任何搜狗词库内容。

## Related

- [搜狗细胞词库的词不能发布到社区](2026-10-17-scel-community-publish-guard.md)：导入之后这些词只供本机使用，社区发布在来源可查的宿主上拦下它们。
- [导入报告保留行号最早的失败](../bug-fix/2026-10-12-dictionary-import-failure-order.md)：细胞词库的坏词条按词序进入同一份报告，最早的几处仍按行号排在前面。
- [导入按已检查行数计上限](../bug-fix/2026-10-09-import-examined-row-limit.md)：iOS 只送前 1001 行依赖解析器每次最多看 1000 行。
- [iOS 与鸿蒙的全平台设计](2026-10-07-ios-harmony-all-platform-design.md)：命名词库在那里作为单独的一块列出，细胞词库经它的 `import` 动作进来。

- [Android 导入后应用词条](../bug-fix/2026-10-17-android-import-apply-before-success.md)：解析和权重不变，补充集合入队后趁空闲应用以及繁忙提示。

- [Gboard v1 文本导入](2026-10-20-gboard-dictionary-import.md)：文本格式识别复用共享解析，不改变细胞词库的固定列序与默认权重。

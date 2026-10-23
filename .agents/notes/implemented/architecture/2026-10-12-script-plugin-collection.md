# Agent Note: 脚本插件合集放在本仓库的 plugins/script/，共用构建配置、数据生成器和按夹具核对的测试

Status: implemented

## Problem

脚本插件（`kind = "script"`）的运行时、授权和作者工具都已合入 develop，但能装的只有 docs/script-plugin-template 里的一个例子。这个例子为了好读，逐字 `chars()` 并先对整段做 UTF-8 校验，body 超过约 100 个汉字就会耗尽 `before_commit` 的 fuel：在输入法里这算一次失败，50 次投递里失败 3 次插件就被停用。用户要的是能直接导入、长期开着也不会被熔断的实用插件（中英文之间加空格、生僻字靠后、隐藏生僻字、繁体字形注释、码位注释），而且要能放心地并行写好几个。

要回答的是：插件放在哪里、每个插件怎么组织、多个插件共用的东西（构建设置、字表、测试）放在哪里、怎么证明每个插件的行为和 fuel 上限，以及 CI 怎么接。

## Decision

**位置：本仓库的 `plugins/script/`。** 每个插件一个目录 `plugins/script/<id>/`，是独立的 `no_std` crate（自带空的 `[workspace]`，不属于仓库的 Cargo 工作区，没有任何依赖），用 `#[path]` 引入 `docs/script-plugin-template/sdk/msime_script.rs`。目录名、包名和清单 `id` 相同，清单的 `module` 是 `<id>.wasm`。源码树里不放 `.wasm`，`plugins/script/.gitignore` 排除 `target/`、`Cargo.lock` 和 `*.wasm`。

**共用的构建设置在 `plugins/script/.cargo/config.toml`。** cargo 从当前目录向上找配置文件，所以在插件目录里运行 cargo 时，目标 `wasm32-unknown-unknown`、`-zstack-size=65536`、release 配置（`opt-level = "s"`、LTO、`panic = "abort"`、strip）和产物目录 `target/script-plugins/cargo` 对所有插件一起生效，插件的 `Cargo.toml` 不写 profile。产物目录用配置写死在仓库根的 `target/` 下，不在插件目录里留 `target/`，也不新起别的产物目录（AGENTS.md 的产物目录规则）。工具链跟随 `rust-toolchain.toml`，脚本从它读出版本号再 `cargo +<版本>`。

**构建和测试是两个脚本。**

- `scripts/build-script-plugins.sh [<id>...]`：在插件目录里 `cargo build --release --offline`，再由 `plugins/script/tools/assemble.py` 组装成 `target/script-plugins/<id>/` 和 `target/script-plugins/<id>-<version>.zip`。组装时检查三个名字一致、`pack/` 里没有 `.wasm`、模块不超过 256 KiB、memory 段的最小页数不超过 `memory_pages`、`pack/` 里约定名字的许可证（`LICENSE.txt`、`Unicode-License-V3.txt`、`OpenCC-LICENSE.txt`）和仓库原件逐字节相同。许可证义务按模块实际编进的数据推出：顺着 `#[path]` 找到 crate 的全部源文件，收集 `include_bytes!` 的文件，在 `EMBEDDED_DATA` 表里查出包里必须带的许可证原文、`license` 必须含的 SPDX 标识和 `NOTICE.txt` 必须原文包含的来源说明；编进了表里没有登记的数据就失败。这样删掉一份许可证、漏掉一个 SPDX 标识，或者重新生成常用字集后忘了更新包里照录的 NOTICE，构建都会失败，而 `msime-pack validate` 不要求许可证文件，查不出这些。压缩包条目排序、放在顶层、时间固定为 1980-01-01、不压缩，同样的源码得到逐字节相同的压缩包。
- `scripts/test-script-plugins.sh [--update] [<id>...]`：编出 `target/debug/msime-pack`，对每个插件 `validate` 文件夹和压缩包；用插件目录里的 `fixture.jsonl` 跑 `run --grant all`，退出码为 0，输出去掉 `fuel`、`init_fuel` 后和 `expected.jsonl` 逐行相同，每行 fuel 不超过钩子单次上限的一半（`candidates` 62 500，`before_commit` 50 000，其余 12 500）；再不授权跑一遍，每个事件都不能有动作。核对在 `plugins/script/tools/check_run.py`。最后跑 `scripts/test-script-plugin-data.py`。没有 wasm 目标时打印 skip 退出 0，`MSIME_REQUIRE_SCRIPT_PLUGINS=1` 时失败。

**行为靠插件自己的日志核对。** `msime-pack run` 的输出只有修改类 import 的名字和返回码，看不到改写成了什么、重排成什么顺序。所以每个插件每个事件写一行 `log::debug` 的决策摘要（改写后的文本、`order 0,2,1`、`hide 2,5`、`ann 0:發/髮`，或 `keep`、`skip: <原因>`），`expected.jsonl` 连同日志一起核对。`log` 在输入法里是空操作，但照样计 fuel，所以只写一行。日志里有上屏和候选的文本（改写结果、注释），这只在 `log` 是空操作时成立：docs/plugins.md 的隐私规则写明，输入法以后若要收集脚本日志，必须把它当作用户输入处理。只记行号和计数的日志不含文本，但 `expected.jsonl` 就核对不了改写成了什么、注释是不是「頭髮」而不是「頭發」，插件最核心的行为没人守着，所以不这样做。fuel 的具体数值随编译器和优化选项变化，不进 `expected.jsonl`，只按上限断言。

**共用数据：生成器加提交进仓库的紧凑产物。** 生僻字类插件共用 `plugins/script/data/common-chars.bin`（4 KiB）：《通用规范汉字表》8105 字（Unihan 17.0.0 的 `kTGH`）并上它们的繁体形（Unihan `kTraditionalVariant` 和 OpenCC `STCharacters.txt` 的值），共 11 049 字。生成器 `plugins/script/tools/gen_common_chars.py` 只用标准库，按 `resources/character-glosses.lock.json` 下载并校验 Unihan zip（缓存在 `target/script-plugins/cache/`），同时写出来源说明 `common-chars-NOTICE.txt`。读法在 `plugins/script/shared/common_chars.rs`（`is_rare`、`is_kana`、按字节扫描一行的 `classify`），文件头在编译时用常量断言检查。`scripts/test-script-plugin-data.py` 逐个运行 `plugins/script/tools/gen_*.py --check`，`run-checks.sh` 自动发现它；要下载数据的生成器在缓存里没有数据时打印 skip 并以 0 退出，所以 contracts 工作流不联网也能跑，但那里只核对得了 `s2t.bin`。`common-chars.bin` 由 Script plugins 作业核对：它设 `MSIME_FETCH=1`，按锁的摘要缓存 Unihan zip，并设 `MSIME_REQUIRE_SCRIPT_PLUGINS=1`，这时任何生成器打印 skip 都算失败。

**SDK 加了不校验 UTF-8 的读法。** `commit::bytes` 和 `candidates::bytes` 直接交回宿主复制的字节。宿主只复制 Rust 的 `&str`，内容一定是合法 UTF-8，而 `text()` 的校验在 wasmi 里每个汉字上百 fuel。没有新增常量，`pack-tool/tests/sdk.rs` 的一致性测试不受影响。

**CI：`ci-platforms.yml` 新增「Script plugins」作业。** `changes` 作业多一个 `plugins` 输出：`plugins/`、`docs/script-plugin-template/`、`crates/script-runtime/`、`crates/pack-tool/`、三个脚本，以及插件包逐字节复制的 `resources/licenses/Unicode-License-V3.txt` 和生成器读的 `resources/character-glosses.lock.json` 改动时为真，共享路径（`shared`）为真时也为真。作业按引擎作业的写法：总是启动，没改动时走 unchanged 步骤；改动时 `rustup toolchain install`、`rustup target add wasm32-unknown-unknown`，先运行 `cargo test --locked -p msime-script-runtime`，执行运行时自身的沙箱单测，再运行 `cargo test --locked -p msime-pack-tool --test sdk`，逐项核对 C/Rust 作者 SDK 的常量及 import 白名单与运行时一致，再带 `MSIME_REQUIRE_SCRIPT_PLUGINS=1` 运行 `scripts/test-script-plugins.sh`，用 sccache。仅构建插件不会读到 SDK 中未被插件引用的常量；已有 ABI 测试必须在这个同样覆盖 SDK 与运行时改动的作业里实际执行，避免符号模式新增编号而作者绑定遗漏。SDK 测试和插件夹具只把运行时当依赖构建，不会执行其 `cfg(test)` 内的测试，不能代替 fuel、内存、栈和快照隔离的单测。`verify-local.sh` 的完整 Rust 测试与 `--all-targets` Clippy 清单同样显式包含 `msime-script-runtime`；4 项人工性能测量保持忽略，普通沙箱测试照常执行。`verify-local.sh` 的 `scope_classify` 把 `plugins/*` 归进不触发平台阶段的那一组，否则它落进 `*) echo all`，只改插件的 push 会让 `--quick` 编译全部平台。

### fuel 的计法

实测 rustc 1.97.1 编出的模块在 wasmi 2.0 上，fuel 比文档写的「大约一条 wasm 指令一个单位」更苛刻，写插件要按它来：

- `local.get`、`local.set` 各算一条。
- 计数指令只出现在函数入口、`loop` 和 `if`/`else` 处，`block` 继承外层的计数位置。所以进入一个函数时，函数体里循环以外的全部指令一次扣完，不管走哪个分支、是否提前返回；每轮循环同样按循环体词法上的全部指令扣。rustc 生成的是 `block` 加 `br_if` 链，很少有 `if`。
- 对策：逐字节执行的循环单独放进一个很小的 `#[inline(never)]` 函数；只在少数情况下用到的代码各自拆成不内联的函数，常见路径就只付自己那一小段；每个插件有自限的工作量（读多少字节、看多少处、查多少次表），超出就放弃，并在夹具里放最坏情况的事件。

pangu-spacing 是第一个按这些规则写的插件。第一版（一个循环里按字节分类、内联判断）扫 512 字节 ASCII 就要约 33 000，「中a」交替的 512 字节超过 56 000；把扫描换成只找「字节种类变化」的小函数后，扫描降到每字节约 17，再把中日韩判断改成查 1024 项的块表、把插空格拆出去，每处变化约 300。最终 body 上限 1024 字节、变化上限 64 处，最坏的成功情况约 43 800（片假名或注音和字母交替：这两个块里各有不算中日韩文字的码位，每处切换都要完整解码；汉字和字母交替约 37 200）。

### 第 0 行和执行顺序

合集的约定：生僻字类插件不动窗口第 0 行，既不移走也不隐藏。窗口第 0 行通常就是引擎下标 0，按首位取词的路径（失焦上屏、原样上屏）取的是它，移走会让屏幕上的首选和实际上屏的不一致。内置的逐键重排可能把引擎下标 0 挪到后面的行，插件看不到引擎的次序，这时它和别的行一样处理：宿主保证它不被隐藏，但重排可以移动它。几个插件的重排是排在后面的优先，所以列出全部非生僻行的重排插件应排在执行顺序最前。插件合集的 README 写合成规则，各插件的 README 写自己的取舍。

### 四个候选窗口插件

rare-chars-last、hide-rare-chars、traditional-glyph 和 unicode-codepoint 都挂 `candidates`，每次最多看 20 行，按上面的 fuel 计法各自定了工作量上限，并在夹具里放了塞满工作量的最坏情况（峰值 38 672、38 561、35 441、34 558，都在 62 500 以内）。

- **生僻字判定不调用共用的 `classify`。** `shared/common_chars.rs` 的 `classify` 每个多字节字符都进一次通用解码，再走区段判断和二分查找，实测每个常用汉字约 200–290 fuel、每个 ASCII 字节约 35，20 行的窗口最坏 11 万到 12.5 万，超过 `candidates` 的单次上限。两个插件各自绕开它：hide-rare-chars 的 `src/scan.rs` 按 UTF-8 首字节跳过，只在汉字上调用 `is_han`、`is_common`、`is_kana`（每字约 250、每 ASCII 字节约 17）；rare-chars-last 的 `src/rare.rs` 在编译时把 `common-chars.bin` 展开成 U+3000..U+9FFF 每字 2 位（7 KiB）和辅助平面按 256 字分组（约 2.5 KiB）的查表（每字约 64、每 ASCII 字节约 15，每换一种字符另加约 200），格式变化由编译期断言拦住。`classify` 留作判定规则的参照实现，rare-chars-last 的查表在宿主上和它逐个比对过。两种扫描各留在自己的插件里，不合进 `shared/`：它们在模块大小和 fuel 之间取的点不同（hide-rare-chars 不展开数据，模块约 7 KiB，每个汉字约 250；rare-chars-last 多 9.5 KiB 的查表，每个汉字约 64），合成一份就要让其中一个插件放弃自己那一头。判定一致靠 `classify` 这份参照和两个插件的夹具守着。
- **工作量上限。** rare-chars-last 一次最多读 768 字节、扫 80 段、每行 256 字节；hide-rare-chars 最多读 640 字节、细看 100 个 U+3000 以上的字（辅助平面字按 3 个算）、每行 256 字节。超出的行不判定、按不生僻处理，日志写明。traditional-glyph 把每一步按实测成本记到 34 000 的预算上，用完后剩下的行不注释，日志末尾加 `(budget)`。每个字的单价按字节数分档，同一档里取最贵的路径（“”…— 这类通用标点要多查一张格式字符表，比全角标点贵约 130），超过 128 字节、读不进来的行也扣一行的开销；起初按常见路径定价，20 行通用标点就要约 48 000、混进超长行约 52 000，同一个按键里第三次投递会被本轮配额截断。unicode-codepoint 只注单个字符和 2–4 个码位的 emoji 序列，最长的注释正好 32 个 UTF-16 码元。
- **traditional-glyph 的数据只取 2–4 字词组里逐字转换会转错的那些。** 完整的 OpenCC 词组表约 1 MB，放不进 256 KiB 的模块。`tools/gen_s2t_table.py` 只读仓库里 vendored 的 OpenCC，产物 `traditional-glyph/data/s2t.bin` 124 385 字节（单字 4 012 条、词组 7 559 条），模块 127 KiB、3 页内存，是合集里最大的。和 OpenCC 完整算法相比，2–4 字词组 46 535 条里有 3 条不同（都含辅助平面字），5 字以上的 3 257 条里有 175 条不同，插件的 README 写明这些限制。

`resources/licenses/Unicode-License-V3.txt` 由 `.gitattributes` 固定为 LF，保证 Windows 检出后仍满足既有原始字节校验；相关门禁范围见[Windows 本地门禁保留可运行的检查](../testing/2026-10-14-windows-local-gate-portability.md)。

## Alternatives considered

- **只补本地完整验证，CI 继续依赖 SDK 测试和插件夹具。** 最强的理由是符合部分 Rust 测试留在本地的既有分工，也不增加 CI 耗时。不用：运行时单测直接守住插件的资源限制与隔离，现有 Script plugins 作业已覆盖运行时路径并准备了同一工具链；复用该作业执行单测能让这条边界在 PR 上实际受检，不需要增加作业、触发范围或时限。

- **放进 msime-plugins 仓库。** 最强的理由是那里本来就是社区扩展包合集，插件和输入法分开发版，用户也习惯去那里找包。不用：GitHub 上还没有这个仓库（只有本机的脚手架），它的 README 写明扩展包是纯数据、「不能带任何可执行内容，也不能申请权限」，它的 CI 只按 schema_version 1 校验；脚本包也不能发布到社区库。更要紧的是，这些插件依赖本仓库的 SDK、`msime-pack` 和运行时的 fuel 计法，三者一变插件就要跟着改、跟着重测，放在同一个仓库里，同一个 PR 的 CI 就能发现。
- **把插件加进根工作区，或者每个插件自带 `.cargo/config.toml` 和 profile。** 加进工作区最强的理由是一条 `cargo build` 就能编全部。不用：工作区的成员都按宿主平台编译，`cargo test --workspace`、clippy 和各平台的构建会把 `no_std` 的 cdylib 编成宿主目标，`#[panic_handler]` 和 `core::arch::wasm32` 在宿主上编不过；`Cargo.lock` 也会被牵动。每个插件自带配置（模板的做法）最强的理由是复制出去就能单独编。不用：五份一样的配置迟早漂移；插件在仓库里，cargo 本来就会向上找到共用的那份。
- **从仓库根用 `--manifest-path` 构建。** 最强的理由是脚本不用 `cd`。不用：cargo 按当前目录查找 `.cargo/config.toml`，从根调用时目标、栈大小和产物目录全部不生效，编出的是宿主目标，或者带 1 MiB 栈的 17 页模块。
- **测试直接比较完整输出，包括 fuel。** 最强的理由是 fuel 回退也能当场看见。不用：fuel 随 rustc、LLVM 和 wasmi 的版本变化，升级工具链会让每个插件的期望全部失效，变成噪音；回退真正要拦的是「超过上限的一半」，这一条单独断言。
- **改 `msime-pack run`，让它打印重排顺序、隐藏的行、注释和改写结果。** 最强的理由是插件不用为测试写日志，也不用为日志付 fuel。不用：这要改核心工具的输出格式，和已有的金样测试一起动，超出这次的范围；日志每个事件一行，代价是一百多 fuel。以后 `run` 打印了 `instance.actions()`，插件可以去掉日志。
- **常用字集用 GB2312。** 最强的理由是 Python 自带 codec，生成不需要下载，也不需要另带许可证。不用：它缺不少常用字和人名用字（啰、堃、喆、镕、珺、赟、垚），又收了 125 个通用规范以外的部首和旧字形，通用规范一、二级里有 190 字不在其中。
- **常用字集用词库字频。** 最强的理由是它反映真实的使用频率。不用：字频来自另一个仓库里 GPL-3.0 的 rime-ice 数据，没有固定摘要，不满足确定性，GPL 还会让插件包整体变成 GPL。
- **生僻字插件直接调用共用的 `classify`。** 最强的理由是判定只有一份代码，不会两处漂移。不用：按 wasmi 的计法它扫满 20 行要 11 万以上的 fuel，超过 `candidates` 的单次上限，一次失败就记进熔断计数。各自的扫描都和它逐字核对过判定。
- **traditional-glyph 带完整的 OpenCC 词组表，或者不带词组只做逐字转换。** 带完整表最强的理由是和 OpenCC、和输入法内置的简繁转换完全一致。不用：约 1 MB，超过 256 KiB 的模块上限。只做逐字转换最强的理由是模块小、fuel 少。不用：「头发」会注成「頭發」，多值字在词里大多取错，注释反而误导。
- **插件代码用 GPL-3.0-only，和仓库一致。** 最强的理由是整个仓库一个许可证，README 不用写例外，也没有哪一份代码的归属要单独说明。不用：SDK 经 `#[path]` 编进每个插件，模板合入时就声明了 MIT；合集若用 GPL，复制合集代码或照它写插件的作者就得跟着用 GPL，和模板给作者的承诺相反。合集插件也不随输入法分发，单独成包，MIT 和包里 Unicode-3.0、Apache-2.0 的数据合在一起只是三份宽松许可证。
- **生成数据不提交，构建时现场生成。** 最强的理由是仓库里没有二进制文件。不用：Unihan zip 有 8.5 MB，要联网下载；提交 4 KiB 的产物以后，CI 和贡献者都不需要网络，`--check` 只在有缓存时比对。

## Consequences

- 收益：插件和它依赖的 SDK、`msime-pack`、运行时在同一个 PR 里一起测；新插件只要建一个目录、写 `fixture.jsonl`，构建和测试脚本自动发现它，不用改共享文件（插件表和第三方说明除外）。每个插件的行为由日志逐行核对，最坏情况的 fuel 有断言卡住。
- 代价：仓库里多了一类非平台代码，CI 多一个作业（改动时编一次 `msime-pack`，约几分钟）；运行时普通单测额外占用执行时间，其中对抗性模块膨胀上界检查较慢，沿用现有 30 分钟时限。本地完整验证也增加该 crate 的测试与全目标 Clippy。本地 `verify-local.sh --quick` 不跑插件测试，只改插件时要手动跑 `scripts/test-script-plugins.sh`。
- 代价：生僻字判定在 `shared/common_chars.rs`、hide-rare-chars 的 `scan.rs` 和 rare-chars-last 的 `rare.rs` 里有三种扫描写法，改常用字集的格式或「生僻」的定义要三处一起改；rare-chars-last 的编译期断言能拦住格式变化，定义变化只能靠夹具。
- 代价：插件为测试写日志，每个事件多花一百多 fuel；`expected.jsonl` 不记录 fuel，fuel 的漂移只在越过上限一半时才被发现。
- 插件代码和它编进的 SDK 用 MIT，沿用模板（模板的 `Cargo.toml` 和清单在合入 develop 时就声明 MIT）；仓库其余部分仍是 GPL-3.0-only。README 的「许可证」一节和 docs/third-party.md 的「本项目」一节写明这个例外的范围（`docs/script-plugin-template/` 和 `plugins/script/`），全文在这两个目录各自的 `LICENSE`。包里带 Unihan 数据的写 Unicode-3.0，带 OpenCC 数据的写 Apache-2.0，第三方说明在 docs/third-party.md 的「脚本插件合集」一节。
- 模板 README 加了一句：模板的写法在长 body 上会耗尽 fuel，实际使用装 `plugins/script/pangu-spacing`。模板本身没有改，`msime-pack` 的测试仍然解析它的夹具。
- pangu-spacing 和内置的[中英文之间加空格](../feature/2026-10-12-chinese-english-spacing.md)开关做相近的事；内置开关在 `before_commit` 之前执行，两个都开不会多出空格。插件的 README 写了两者的区别。

## Verification

- 运行时门禁的负对照：临时将已有最小模块单测的预期改错，旧 SDK 命令仍然两项通过，显式选择 `msime-script-runtime` 则该单测失败、退出 101；精确恢复后，普通套件 66 项通过、4 项人工性能测量忽略，全目标严格 Clippy 通过。一次性提取本地测试与 Clippy 循环、令运行时命令失败的接线检查中，旧清单未察觉失败，新清单均以 1 退出。

- CI 新增的 `cargo test --locked -p msime-pack-tool --test sdk` 在本机实际执行两项测试；临时删掉 SDK 的 `MSIME_LOCAL_MODE_SYMBOL` 做缺项负对照时，常量对照失败并退出 101，恢复后通过。插件构建与夹具校验继续由原脚本承担。

- `scripts/test-script-plugins.sh`：pangu-spacing 的 22 个事件和 `expected.jsonl` 一致，最大 fuel 37 171；不授权时每个事件只有 `skip: denied` 日志，没有动作；文件夹和压缩包都通过 `msime-pack validate`。连续两次构建的压缩包 sha1 相同。把 `expected.jsonl` 改错一行时脚本以 1 退出并打印两边的差异。
- `python3 scripts/test-script-plugin-data.py`：有缓存时 `common-chars.bin` 与生成结果一致；挪走缓存时打印 skip、以 0 退出。
- `shared/common_chars.rs` 在宿主上编成临时程序，对 U+0000..U+3FFFF 逐个调用 `is_common`，得到的集合和生成器在 Python 里算出的 11 049 字完全相同。
- 五个插件合并后 `scripts/test-script-plugins.sh` 全部通过：hide-rare-chars 26 个事件（峰值 fuel 37 992）、pangu-spacing 22 个（37 171）、rare-chars-last 28 个（36 561）、traditional-glyph 34 个（36 130）、unicode-codepoint 17 个（34 558）；`gen_common_chars.py` 和 `gen_s2t_table.py` 的 `--check` 都一致。
- `actionlint` 对 `ci-platforms.yml` 无报错；`shellcheck` 对两个脚本无报错。
- 评审之后的修正：`scripts/test-script-plugins.sh` 全部通过，hide-rare-chars 27 个事件（峰值 38 561）、pangu-spacing 23 个（43 816）、rare-chars-last 29 个（38 672）、traditional-glyph 38 个（35 441）、unicode-codepoint 17 个（34 558）。四个候选窗口插件各用几千个随机窗口（多种来源；汉字、繁体字、通用标点、泰文、注音、拉丁和阿拉伯字母、emoji、辅助平面字混排；超长行）压测，没有失败，traditional-glyph 最多约 33 900，其余三个的最贵窗口已收进夹具。挪走 Unihan 缓存后 `MSIME_FETCH=1` 重新下载、`--check` 通过；设 `MSIME_REQUIRE_SCRIPT_PLUGINS=1` 而不下载时以 1 退出。删掉一个包的 `OpenCC-LICENSE.txt`、去掉 `Unicode-3.0`、改动照录的 NOTICE 或 `LICENSE.txt` 时组装都会失败。
- 合入后的第二轮评审（#7456 之后）：pangu-spacing 见到 ASCII 的 `/`、`\`、`?`、`=` 整段不改，夹具加了不带协议头的网址、Windows 路径和查询串三条，都是 `skip: address`；traditional-glyph 原样写出的四字节字符也计两个 UTF-16 码元，20 个 emoji 加「发」的行不再交给宿主后被拒。`scripts/test-script-plugins.sh` 全部通过：pangu-spacing 26 个事件、traditional-glyph 39 个，其余三个和峰值 fuel 不变。

相关：[沙箱运行时](2026-10-12-script-runtime-sandbox.md) 定义 fuel 配额；[脚本插件的作者工具](../feature/2026-10-12-script-plugin-author-tools.md) 定义 `msime-pack run`、SDK 和模板；[脚本插件修改上屏和候选](2026-10-12-script-plugins-modify-commit-and-candidates.md) 定义重排、隐藏、注释的合成规则。

后续：pangu-spacing、traditional-glyph 和 unicode-codepoint 与内置功能重复，已从合集删除，见[删除与内置重复的脚本插件](../simplification/2026-10-13-remove-duplicate-script-plugins.md)。

后续：「不随输入法分发」对官方包翻转了：`plugins/official-packs.json` 登记的 uuid 随 macOS、Windows、Linux 的桌面安装包分发，默认停用、要授权才运行，内容哈希锁在那个文件里，见 [官方插件包随三个桌面安装包分发](2026-10-13-official-plugin-packs-in-desktop-installers.md)。合集里其余插件照旧由用户构建后导入。

# msime-engine

The input method engine of 水杉输入法 (Metasequoia IME): quanpin, shuangpin, nine-key, wubi, zhuyin, Cantonese, Japanese, Korean and more, ranked over a word lattice with typo correction and learning from what the user picks. Licensed under MPL-2.0 so that other input methods can embed it; see [License](#许可证).

Overview, integration examples and FAQ: <https://msime.app/engine/> (in Chinese).

水杉输入法的输入引擎：组字状态、各输入方案、词库查询、排序与学习。它是 GPL-3.0 的 C++ [metasequoiaime/msime-engine](https://github.com/metasequoiaime/msime-engine) 的 Rust 移植，移植时对照的参考提交记在 `tools/engine-golden/README.md`。

## 快速上手

Rust 项目只要 `cargo add msime-engine`：SQLite 已经编进引擎，不用另装任何库，也不用克隆仓库。

引擎不带词库，运行时要一个数据目录。最省事的是 [msime-engine-capi 的发布](https://github.com/metasequoiaime/msime/releases?q=msime-engine-capi&expanded=true) 里的最小数据包 `msime-engine-data-quanpin-<词库版本>.tar.gz`（Windows 用同名的 `.zip`），解开的目录就是数据目录，里面是全拼、双拼要的 `msime-pinyin.db`、`msime-english.db`、`msime-bigram.bin`、`msime-trigram.bin` 和 `msime-scowl_Copyright.txt`；也可以从 [msime-dictionary 的 `dict-v*` 发布](https://github.com/metasequoiaime/msime-dictionary/releases) 逐个下载这五个文件放进一个目录。**这些数据不是 MPL-2.0**：主体是 GPL-3.0，n-gram 表是 CC-BY-SA-4.0，逐项见数据包里的 `README.md`。

```rust
use msime_engine::{prepare_runtime_paths, DictionaryAccess, Session, SessionOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 数据目录（解开的数据包）和放学习日志、工作词库的目录，都要是绝对路径。
    let resources = std::path::absolute("msime-engine-data-quanpin-2.0.39")?;
    let work = std::env::temp_dir().join("msime-quickstart");

    // 准备代次：复制工作词库、回放学习日志。数据换了就换一个 content id。
    let paths = prepare_runtime_paths(&resources, &work.join("user"), &work.join("cache"), "dict-v2_0_39")?;
    // 会话期间持共享租约，与改个人词典、重置学习这类维护互斥。
    let access = DictionaryAccess::try_session(&paths.user_data, &paths.dictionaries)?
        .ok_or("the dictionaries are being maintained")?;
    // 引擎库的默认值：全拼、小鹤双拼、学习开。
    let mut session = Session::new(SessionOptions::new(paths))?;

    for key in b"nihao" {
        session.character(*key, false);
    }
    let state = session.display_snapshot();
    println!("preedit: {}", state.preedit);
    for (index, candidate) in state.candidates.iter().take(9).enumerate() {
        println!("{}. {}", index + 1, candidate.word);
    }
    if let Some(text) = session.select(0).commit {
        println!("commit: {text}");
    }

    drop(session); // 结束会话：写下它的学习、交还日志连接
    drop(access);
    msime_engine::shutdown(); // 退出前写完排着的学习，停掉后台写线程
    Ok(())
}
```

在 dict-v2.0.39 的数据包上，它打出：

```text
preedit: ni'hao
1. 你好
2. 你好像
3. 你好好
4. 你好多
5. 拟好
6. 你哈哦
7. 你还
8. 泥蚶
9. 你
commit: 你好
```

完整的版本是随包发布的 [`examples/quickstart.rs`](https://github.com/metasequoiaime/msime/blob/develop/crates/engine/examples/quickstart.rs)，数据目录从命令行给出：在仓库里 `cargo run --release --example quickstart -- <数据目录>`。`prepare_runtime_paths` 按全部方案准备，要 `msime-pinyin.db` 和 `msime-english.db`。只用几个方案时用 `prepare_runtime_paths_for`，传入与 `SessionOptions::enabled_schemes` 相同的 `SchemeSet`，数据目录只要有这些方案读的文件：没有全拼、双拼、五笔时两个库都不要求，韩文、越南文、藏文什么也不用带，日文只要 `msime-japanese.dat`（没有时只出假名），粤语、注音、笔画只要各自的语言库；带了 `msime-english.db` 时专用英文照样有补全、回车学词。重置学习用同一个集合调 `reset_learned_data_for`。逐个方案的文件与许可证见 [docs/data-formats.md 的「每个方案的最小数据集」](https://github.com/metasequoiaime/msime/blob/develop/docs/data-formats.md#20-每个方案的最小数据集)；从 C 或别的语言用，见 [msime-engine-capi](https://github.com/metasequoiaime/msime/blob/develop/crates/engine-capi/README.md#快速上手)。

## 能做什么

- 方案：全拼、双拼（含自定义双拼表 `ShuangpinCustomTable`）、九键与 14 键、五笔 86/98、注音、粤拼、笔画、日文罗马字、韩文（两套式与罗马字布局）、越南文（Telex、VNI）、藏文（EWTS）。
- 整句：词格解码，可选 n-gram、octagram 搭配模型和神经重排模型给句子打分。
- 纠错：全拼的拼写纠错，触屏键盘的触点纠错。
- 学习：用户选过的词与个人词典存在本地 SQLite，按日志回放。回车（`Command::CommitRaw`）缺省只把专用英文里打的词学进英文词库；`SessionOptions::learn_typed_spellings_on_enter` 打开后，临时英文的字母和拼不成完整音节的拼音也学（msime 自己的策略），`Command::CommitRawWithoutLearning` 上屏同样的原文而不学。

引擎只带算法和几张小的内嵌表，不带词库。运行时要由调用方给出词库文件（`msime-pinyin.db`、`msime-english.db` 等，格式与来源见 [docs/third-party.md](https://github.com/metasequoiaime/msime/blob/develop/docs/third-party.md)），这些数据按各自的许可证分发，不随引擎改为 MPL-2.0：默认的中文词库含 rime-ice 的 GPL-3.0 数据，n-gram 表的计数来自中文维基百科（CC-BY-SA-4.0），全拼、双拼、五笔都要的 `msime-english.db` 含 rime-ice-en 的 GPL-3.0 数据（只用日文、韩文、越南文、藏文、粤语、注音、笔画时可以不带它）。用哪套数据由采用方自己决定。

给其他语言用的 C ABI 是单独的 [`msime-engine-capi`](https://github.com/metasequoiaime/msime/blob/develop/crates/engine-capi/README.md)，发布里有预编译好的库；可嵌入引擎库的设计见仓库里的 `.agents/notes/proposed/architecture/2026-10-29-embeddable-engine-library.md`。

## 从 crates.io 使用

- **API 不稳定**：0.x 期间 Rust API 不承诺稳定，小版本之间也可能改。文档里的公开面是核心层：`Session`、`SessionOptions` 和它们旁边的函数与类型。`host`、`backend`、`ordering`、`web` 是水杉输入法自己的宿主用的产品门面，只为同一仓库里的 crate 保持 `pub`，不进文档，也不要依赖它们。
- **SQLite 是内置的**：`rusqlite` 开着 `bundled`，引擎自带一份编进去的 SQLite，不用系统的 libsqlite3。`libsqlite3-sys` 声明了 `links = "sqlite3"`，一个依赖图里只能有一个版本的 `libsqlite3-sys`：你的 crate 如果直接或间接依赖不兼容的另一版 `rusqlite`（引擎用 0.40，对应 `libsqlite3-sys` 0.38），cargo 会拒绝同时解析两者，要把 `rusqlite` 对齐到 0.40。
- **dev 构建里神经整句很慢**：打开 `SentenceAssociationOptions::neural_keyboard` 后，`chinese-ime-lm` 的句子模型每个按键都在调用线程上做整数点积给整句打分，这个 crate 不开优化时足以让候选窗口卡上好几秒。仓库自己的工作区对它单独开了 `opt-level = 3`，但 `[profile]` 不随 crate 发布，下游要在自己的工作区根 `Cargo.toml` 里加上：

  ```toml
  [profile.dev.package.chinese-ime-lm]
  opt-level = 3
  ```

  release 构建不受影响。
- **包里的源码只能编译，不能跑测试**：测试、金样和 `examples/` 读仓库里锁定的词库与仓库外的文件，`include` 白名单没有把它们放进包；只有不读仓库文件的 `examples/quickstart.rs` 随包发布。要跑测试请克隆 [仓库](https://github.com/metasequoiaime/msime)。
- **许可证扫描**：越南文方案依赖的 `vi` 0.8.0 在元数据里没有 SPDX `license` 字段，只有 `license-file`（内容是 MIT）。`cargo-deny` 之类按 SPDX 判断的工具会把它当成未知许可证，需要在配置里按 MIT 加一条澄清。

## 许可证

代码按 [Mozilla Public License 2.0](LICENSE) 授权，不附 Exhibit B（「Incompatible With Secondary Licenses」），所以按 MPL-2.0 第 3.3 条仍能与 GPL 代码组合；Exhibit A 声明在 `LICENSE` 开头和 `src/lib.rs` 第一行。仓库的其余部分仍是 GPL-3.0-only，见根目录的 [README](https://github.com/metasequoiaime/msime/blob/develop/README.md#许可证)。同一仓库里的 `msime-path-trust`（本 crate 的依赖）也是 MPL-2.0。

引擎移植自 GPL-3.0 的 C++ msime-engine 与 msime-windows 的引擎代码。被移植或收进本 crate 与 `msime-path-trust` 的贡献，其版权人都同意以 MPL-2.0（不附 Exhibit B）再许可，同意记录由维护者留存。

下面这些文件保留各自的许可证，不按 MPL-2.0。`Cargo.toml` 的 `license` 字段是引擎代码和编进引擎的数据的 SPDX 表达式，包括了它们。只给测试读的夹具不在 MPL-2.0 与这个表达式之内，仍按仓库的 GPL-3.0-only：`tests/golden/real/` 是对着含 rime-ice 数据（GPL-3.0）的词库录下的候选列表，`src/handwriting/testdata/` 是手写参考实现的输出；`Cargo.toml` 的 `include` 白名单把它们排除在发布到 crates.io 的包之外。许可证全文在本 crate 的 `licenses/` 目录，随包发布，与仓库 `resources/licenses/` 里的同名文件逐字节相同；分发编进了引擎的二进制时要随附它们和 `LICENSE`。msime 自己各平台的声明渠道见 [docs/third-party.md](https://github.com/metasequoiaime/msime/blob/develop/docs/third-party.md#通知文件在哪)。

| 文件 | 来源 | 许可证 | 全文 |
| --- | --- | --- | --- |
| `src/korean/hanja.tsv` | [libhangul](https://github.com/libhangul/libhangul) 提交 `717409ce61524bb3d8426060a384822f21354c62` 的 `data/hanja/hanja.txt`，Copyright (c) 2005,2006 Choe Hwanjin | BSD-3-Clause | [`licenses/libhangul-hanja-BSD-3-Clause.txt`](licenses/libhangul-hanja-BSD-3-Clause.txt) |
| `src/local/places.tsv` | [modood/Administrative-divisions-of-China](https://github.com/modood/Administrative-divisions-of-China) 提交 `c49d495b40ac73eb1a66f6eeae5f8fd10696f035` | WTFPL | [`licenses/Administrative-divisions-of-China-WTFPL.txt`](licenses/Administrative-divisions-of-China-WTFPL.txt) |
| `src/local/chaizi.tsv`、`src/local/chaizi-chars.tsv` | [kfcd/chaizi](https://github.com/kfcd/chaizi) 提交 `e177ab54ce4edf255315a60593ffecbd8167e96b`（© 2015 開放詞典）改编；读音、名次与繁体字形取自 Unihan 17.0.0 和 [OpenCC](https://github.com/BYVoid/OpenCC) 提交 `26753884f1984add422f3b0249ccee8613deaff6` 的简繁表 | CC-BY-3.0 AND Unicode-3.0 AND Apache-2.0 | [`licenses/kfcd-chaizi-CC-BY-3.0.txt`](licenses/kfcd-chaizi-CC-BY-3.0.txt)（含署名与改动说明）、[`licenses/Unicode-License-V3.txt`](licenses/Unicode-License-V3.txt)、[`licenses/OpenCC-Apache-2.0.txt`](licenses/OpenCC-Apache-2.0.txt) |
| `src/local/symbol_groups.tsv` | 组员按 Unicode 字符数据库 17.0.0 的 `UnicodeData.txt`、`Blocks.txt`、`EquivalentUnifiedIdeograph.txt` 挑选，规则写在 `scripts/build_symbol_groups.py` | Unicode-3.0 | [`licenses/Unicode-License-V3.txt`](licenses/Unicode-License-V3.txt) |
| `src/local/latex_symbols.tsv` | 本项目自写，唯一的编辑入口是 msime-dictionary 的 `sources/symbols/latex-symbols.tsv` | MIT | [`licenses/latex-symbols-MIT.txt`](licenses/latex-symbols-MIT.txt)（与 `plugins/phrase_table/LICENSE` 同一份正文） |
| `src/handwriting/model.rs`、`src/handwriting/features.rs` | 移植自 [Zinnia](https://github.com/taku910/zinnia) 提交 `581faa8f`，Copyright (c) 2005-2007 Taku Kudo | BSD-3-Clause（与本项目的 MPL-2.0 改动一起） | [`licenses/Zinnia-LICENSE.txt`](licenses/Zinnia-LICENSE.txt) |
| `src/lattice/octagram.rs` | 格式与查询算法来自 [lotem/librime-octagram](https://github.com/lotem/librime-octagram) 提交 `57d18b9f58e5284bd891d559f6bdd16cf60341e9`，Copyright (c) 2014, RIME Developers；双数组读取按 [darts-clone](https://github.com/s-yata/darts-clone) 0.32 的布局重新实现，Copyright (c) 2008-2014, Susumu Yata | BSD-3-Clause AND BSD-2-Clause（与本项目的 MPL-2.0 改动一起） | [`licenses/librime-octagram-BSD-3-Clause.txt`](licenses/librime-octagram-BSD-3-Clause.txt)（末尾附 darts-clone 的条款） |
| `src/lattice/cxx_sort.rs` | 逐步复现 LLVM libc++ 的 `__algorithm/partial_sort.h`、`sort.h`、`make_heap.h`、`sift_down.h`、`pop_heap.h`、`push_heap.h`、`sort_heap.h` | Apache-2.0 WITH LLVM-exception（与本项目的 MPL-2.0 改动一起） | [`licenses/libcxx-Apache-2.0-WITH-LLVM-exception.txt`](licenses/libcxx-Apache-2.0-WITH-LLVM-exception.txt) |

另外四处不另带许可证：

- `src/local/symbol_candidates.tsv` 是本项目自写的符号名称表，和代码一样是 MPL-2.0。
- `src/lattice/offensive_words.txt` 是本项目自写的粗俗词表和豁免写法（词网格不拿含这些词的词拼整句），和代码一样是 MPL-2.0；收词标准写在文件头。
- `src/korean/romaja.rs` 的罗马字映射参考 [rimeinn/rime-mungyeong](https://github.com/rimeinn/rime-mungyeong) 提交 `34c1b1437824e1e619e7429fb5f461dc7e754939` 的 `mungyeong_romaja.dict.yaml`，该文件头声明 public domain。
- `src/nine_key/syllable_frequency.tsv` 是 424 个音节各自的总权重，由 `msime-dict-build syllable-frequency` 从 msime-dictionary `dict-v2.0.41` 的 `msime-pinyin.db`（rime-ice 的 GPL-3.0 词库加补充）汇总而来，不含任何词、读音或单条权重，按聚合统计事实保留，只作九键切分的排序先验。用 Unihan `kHanyuPinlu`（Unicode-3.0）重建的版本在九键评测不退步时会替换它。

## 贡献

向本 crate（`crates/engine`）和 `crates/path-trust` 提交的贡献，按「GPL-3.0-only OR MPL-2.0」授权：提交 PR 即表示你同意你的改动既可按仓库的 GPL-3.0-only、也可按这两个 crate 的 MPL-2.0 使用。不能接受这一点的话请不要改这两个目录。引入第三方代码或数据时，在上表加一行，把许可证全文放进仓库的 `resources/licenses/` 并复制一份到本 crate 的 `licenses/`（`scripts/test-engine-license.py` 核对两份相同），再接到各平台的声明渠道。流程见 [CONTRIBUTING](https://github.com/metasequoiaime/msime/blob/develop/.github/CONTRIBUTING.md)。

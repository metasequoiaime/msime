# 第三方组件清单

这份清单回答「这个项目用了谁的代码和数据、各自什么许可」。它是仓库级的索引；各平台打包时实际生成的通知文件另有位置，见末尾的[通知文件在哪](#通知文件在哪)。

清单不替代许可证审计。它记录已知的来源与条款，并明确标出尚未记录的部分——后者同样重要，因为一份看起来完整、实则有空白的清单比承认空白更危险。

## 离线世界时钟

`client-core::world_clock` 使用 `chrono` 0.4.45、Jiff 0.2.35（`jiff-tzdb` 0.1.8，内嵌 IANA 2026c）和 `sunrise` 3.0.0；代码使用其 MIT 许可，时区数据库为公共领域。版本及 crate 归档校验由 `Cargo.lock` 固定，未修改上游源码。城市坐标原样来自 IANA `tzdata2026a/zone.tab`（公共领域），归档来源、SHA-256 与原始许可见 [目录说明](../crates/client-core/src/world_clock/README.md)。北京是本项目新增的目录条目。

[world-clock-NOTICE.txt](../resources/licenses/world-clock-NOTICE.txt) 包含这些 crate 原始许可全文和来源，随 Android 原生通知、macOS 许可目录、iOS 资源、HarmonyOS 许可目录及 Windows 通知分发；Linux 的 Cargo 通知收集器自动收录相应 crate。运行时完全离线，不请求定位权限。日出日落是指定参考城市的近似值。

## 本项目

源码为 **GPL-3.0-only**，全文在根目录 [LICENSE](../LICENSE)。Rust workspace 的 `license` 字段、Linux 打包元数据和共享客户端都声明同一许可证。例外是 `crates/script-sdk/`（脚本插件的 SDK，发布到 crates.io 的 `msime-script-sdk`）、`docs/script-plugin-template/`（脚本插件的模板）、`plugins/script/`（[脚本插件合集](#脚本插件合集pluginsscript)）的代码和 `plugins/phrase_table/`（官方短语表）的数据，用 MIT，全文在这四个目录各自的 `LICENSE`。SDK 是 Rust workspace 的成员，但它的 `license` 字段单独写 MIT、不从 workspace 继承；其余三个不属于 Rust workspace。其中 `plugins/official-packs.json` 登记的官方包（脚本插件 uuid、短语表 latex-symbols）随桌面安装包分发，每个包带着自己的 `LICENSE.txt`；其余只在源码里，由用户构建后导入。另一个例外是输入引擎 `crates/engine/`（`msime-engine`）和它依赖的 `crates/path-trust/`（`msime-path-trust`）：两者用 MPL-2.0，不附 Exhibit B，全文和 Exhibit A 声明在各自目录的 `LICENSE`，`license` 字段同样不从 workspace 继承，见[下一节](#输入引擎cratesengine)。整个输入法产品仍按 GPL-3.0-only 发布：各平台安装包、关于页、发行版元数据和网页引擎照旧写 GPL-3.0-only。GPL-3.0 第 4、6 条要求随目标代码附上许可证全文，`msime-pinyin.db` 里 rime-ice 的数据也是 GPL-3.0，所以根目录的 `LICENSE` 随每个平台的安装包分发：macOS 包的 `Resources/Licenses/GPL-3.0.txt`，Windows 安装目录的 `LICENSE.txt`，Linux 安装的 `copyright`，Android APK 的 `assets/client-LICENSE.txt`（`build-apk.sh`、`build-client-apk.sh`，关于页以应用名列出），iOS App 资源里原样的 `LICENSE`（`project.yml`，键盘扩展只随 App 安装，关于页「开源许可」排在第一项），HarmonyOS HAP 的 `resfile/licenses/GPL-3.0.txt`（`stage-resources.sh`）。三个移动端同时带着引擎的 `MPL-2.0.txt`，`scripts/test-language-data-notices.py` 的 `MOBILE_LICENCE_CHANNELS` 要求这三端两份全文都在。`crates/engine-capi/`（引擎的 C 接口 `msime-engine-capi`）同样用 MPL-2.0，不附 Exhibit B，全文在该目录的 `LICENSE`，`license` 字段单独写、不从 workspace 继承；从它构建出的二进制按 MPL-2.0 分发，链接进去的第三方 crate 和内嵌数据表保留各自的许可证，二进制归档的第三方声明怎么生成见它的 README。

## 输入引擎（`crates/engine`）

`crates/engine`（`msime-engine`）是本项目自己的代码，按 MPL-2.0（不附 Exhibit B）授权，它依赖的 `crates/path-trust`（`msime-path-trust`）也一样，好让其他输入法能嵌入引擎；编进输入法产品时与其余代码一起按 GPL-3.0-only 分发，MPL-2.0 第 3.3 条允许这种组合。引擎是 [`metasequoiaime/msime-engine`](https://github.com/metasequoiaime/msime-engine)（GPL-3.0）C++ 实现的 Rust 移植，另有部分代码译自 msime-windows 的引擎；被移植或收进这两个 crate 的贡献，其版权人都同意以 MPL-2.0 再许可，同意记录由维护者留存，比对依据见 `.agents/notes/implemented/architecture/2026-10-29-embeddable-engine-library.md`。crate 里保留各自许可证的内嵌数据和改编代码逐个文件列在 [`crates/engine/README.md`](../crates/engine/README.md#许可证)，`Cargo.toml` 的 `license` 是覆盖它们的 SPDX 表达式（`scripts/test-engine-license.py` 核对两边一致）。发布到 crates.io 的包由 `include` 白名单限定，只带源码、编进引擎的数据表、README、`LICENSE` 和 `crates/engine/licenses/`，后者是这些文件所需许可证全文在 `resources/licenses/` 里的逐字节副本（同一个脚本核对），录自 GPL 词库的金样和手写参考输出不进包；两个 crate 由手动触发的 `.github/workflows/release-engine-crates.yml` 发布。引擎运行时读的词库和模型不随引擎改许可证，按下文各节各自的条款。向这两个目录提交的贡献按「GPL-3.0-only OR MPL-2.0」授权（`.github/CONTRIBUTING.md`）。移植时对照的参考提交记在 `tools/engine-golden/README.md`。构建时不再取回任何上游源码归档，原先随 Engine 归档进来的组件去向如下：

| 组件 | 许可证 | 现状 |
| --- | --- | --- |
| `metasequoiaime/Google-PinyinIME-Rev` | Apache-2.0 | 已移除。Google 整句解码器和它的 `dict_pinyin.dat` 随 C++ Engine 一并退役，整句候选只来自词格 |
| `nemtrif/utfcpp` | BSL-1.0 | 引擎不再使用。Windows TSF 仍通过 vcpkg 引入它，见[各平台引入的第三方 SDK](#各平台引入的第三方-sdk) |
| `mackron/miniaudio` | 公有领域 / MIT-0 双许可 | 不再随引擎来。Windows 提示音（`platforms/windows/src/voice/CuePlayer.cpp`）用的单头文件固定在 `platforms/windows/third_party/miniaudio/`，许可证全文在同目录 `LICENSE`；麦克风采集改由下表的 `cpal` 负责。HarmonyOS 2in1 的按键音也用这同一份头文件（v0.11.25）：`platforms/harmony/native/key_sound_render.cpp` 只编译它的 WAV 解码、采样率转换和 WAV 编码（不编设备后端和其他编解码器），在长度上限内解码音效包的 WAV 样本并按旋律音高写出，再交给 SoundPool 播放 |
| `ggml-org/whisper.cpp` | MIT | 已移除。Whisper 本地文件识别不再提供，本地语音识别只走下文的 sherpa-onnx |
| Zinnia（Taku Kudo） | BSD-3-Clause | 不再编译 C++ 版。`crates/engine/src/handwriting/` 是其识别器的 Rust 移植，桌面包照旧携带 `Zinnia-LICENSE.txt` |
| 手写模型 `handwriting-zh_CN.model` | LGPL-2.1 | 由 `resources/handwriting-model.lock.json` 固定长度与 SHA-256，许可证全文 `HandwritingModel-LICENSE.txt` 与模型一同固定、一同分发 |
| Boost、fmt、spdlog | 各自上游许可证 | 只为编译 C++ Engine 而引入，已从各平台的依赖清单去掉；Windows TSF 自己用的 fmt 保留 |

引擎移植引入的 Rust crate（版本以 `Cargo.lock` 为准）：

| crate | 版本 | 许可证 | 用途 |
| --- | --- | --- | --- |
| `rusqlite`（`bundled`） | 0.40 | MIT；打包进来的 SQLite 本身属公有领域 | 词库、学习日志与个人词库。`bundled` 把 SQLite 编进库里，不再链接系统或 vcpkg 的 SQLite |
| `lru` | 0.18 | MIT | 查询结果缓存（`crates/engine/src/cache.rs`） |
| `lunar-lite` | 0.1 | MIT | 日期时间快捷模式里的农历（`crates/engine/src/local/date_time.rs`） |
| `wana_kana`（只是开发依赖，固定 `=5.0.0`） | 5.0 | MIT | 只在测试里用：`crates/engine/src/japanese/romaji.rs` 的测试拿它对照平假名转片假名的旧行为，不链接进任何产物；罗马字转假名是引擎自己的代码 |
| `cpal` | 0.18 | Apache-2.0 | `host-api` 的麦克风采集（iOS 与 HarmonyOS 不链接）。Linux 上经 ALSA 的 `libasound` 访问声卡，构建需要 `libasound2-dev` 与 `pkg-config` |
| `rubato` | 5 | MIT OR Apache-2.0 | 把采集到的音频重采样到识别所需的采样率 |
| `exmex` | 0.21 | MIT OR Apache-2.0 | V模式的算式求值（`crates/engine/src/local/expression.rs`），只注册四则运算、`%` 取余和 `^` 乘方。它带进 `regex` 与 `smallvec`，两者原本就在锁文件里 |
| `chinese-number`（关闭默认特性，只开 `std`、`number-to-chinese`） | 0.8 | MIT | V模式把数字写成中文小写、大写与金额（同一文件）。传递依赖 `chinese-variant`（MIT）、`enum-ordinalize`（MIT）、`num-bigint`（MIT OR Apache-2.0） |
| `rink-core`（关闭默认特性） | 0.9 | MPL-2.0 | V模式的单位换算（`crates/engine/src/local/units.rs`），例如 `3jin'g` 把 3 斤换成克。不开 `bundle-files`：它会把 rink 的单位库 `definitions.units` 编进库里，那份库分叉自 GNU Units 的数据库，是 GPL-3.0-or-later（Free Software Foundation）。`units.rs` 改为在空的 `Context::new()` 里加载本项目按法定值自写的 `BASE_DEFINITIONS`（只有用到的 SI 与国际单位）和市制定义，单位上下文在第一次用到时才加载；汇率要联网取数据，从不加载，所以不换算货币。带进 `num-rational`（MIT OR Apache-2.0）与 `strsim`（MIT）。源码在 crates.io 的对应版本，上游仓库是 [codeberg.org/tiffany/rink](https://codeberg.org/tiffany/rink)。MPL-2.0 的履行方式与下文音效包一节的 `symphonia` 相同 |
| `vi`（固定 `=0.8.0`） | 0.8.0 | MIT（crate 以 `license-file` 声明，Copyright 2020 Hung Nguyen） | 越南语方案的 Telex 与 VNI 变换（`crates/engine/src/vietnamese/`）。固定到补丁版本，上游的小版本发布不会悄悄改变输入行为。链接进二进制的传递依赖新增 `nom` 8（MIT）与 `phf`、`phf_shared` 0.11（MIT，与 workspace 已有的 0.13 并存）；`log`、`smallvec`、`memchr` 与 `siphasher` 1 原本就在锁文件里。`phf` 的 `macros` 特性在编译期另带 `phf_macros`、`phf_generator` 0.11（MIT）和 `rand` 0.8（MIT OR Apache-2.0），它们不进二进制。macOS 包内的 `vi-MIT.txt` 是它的许可证全文，Windows 安装包的 `THIRD_PARTY_NOTICES.txt` 也收入同一份（`Collect-Notices.ps1`）。上游仓库是 [ZeroX-DG/vi-rs](https://github.com/ZeroX-DG/vi-rs) |
| `ewts` | 0.1.4 | MIT OR Apache-2.0（按 MIT 使用，作者 Maxim Zommer） | 藏文方案把 EWTS（扩展威利转写）转成藏文 Unicode（`crates/engine/src/tibetan/`）。没有任何依赖。crate 和上游仓库都没有附许可证文件，`resources/licenses/ewts-MIT.txt` 是标准 MIT 文本，版权行写的是 crate 元数据里的作者；macOS 包内的 `ewts-MIT.txt`、Windows 安装包的 `THIRD_PARTY_NOTICES.txt`（`Collect-Notices.ps1`），以及 Android 的 `assets/native-notices/ewts.txt`（`build-native.sh`）、iOS App 资源（`project.yml`）和 HarmonyOS 的 `resfile/licenses`（`stage-resources.sh`）都收入这一份；这三个移动端同样带上 `vi-MIT.txt`。它的转换表按 README 所说取自 [rogerespel/ewts-js](https://github.com/rogerespel/ewts-js)（Copyright (C) 2010-2025 Roger Espel Llima，Apache-2.0）：`ewts-MIT.txt` 末尾附着这条版权行和 Apache-2.0 全文，所以上面每个渠道都随之带上；macOS 和 Linux 的 `THIRD_PARTY_NOTICES.txt` 另外注明出处。上游仓库是 [emgyrz/ewts-rs](https://github.com/emgyrz/ewts-rs) |
| `rustix`（只开 `fs`，只在 Unix 上） | 1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | 词库访问租约（`crates/engine/src/access.rs`）经目录句柄打开根目录和锁文件、不跟随符号链接，Android 上用它的 `flock` 取锁。原本就经 `msime-client-base` 在锁文件里，同一版本 |

## Android 组合表情

`shared/emoji-combinations/emoji-combinations.txt` 取自 [gurecn/yuyansdk 的固定提交](https://github.com/gurecn/yuyansdk/blob/c99f9b7c451b7b77d379dab7d55fdbfb93b04b12/src/main/java/com/yuyan/imemodule/data/emojicon/EmojiconData.kt)中 `icon_emojibar_hot` 的 165 项，仅转换为逐行 UTF-8 文本，顺序和 Unicode 序列不变。它与 #8193 的用户附件完全一致，源文件 SHA-256 为 `903c1da263b241a7e47ca32b11ab80d7c8e70e583370640894186d2f8a183604`。采用 BSD-3-Clause，版权为 Copyright (c) 2026, 王莹，源文件保留 Hieu Rocker 作者标记；完整许可、署名和修改说明在同目录 `emoji-combinations-NOTICE.txt`。Android 原生 APK 和 Tauri 合包均带此通知，原生关于页的「开源许可」可查看。未引入 Kotlin 代码、图片、字体或微信文案；其他平台不打包这份数据。

## 随包资源（`resources/desktop-dictionary.lock.json`）

锁文件固定十二个产物的长度和 SHA-256，每个都带可匿名下载的 URL：十一个来自 `metasequoiaime/msime-dictionary` 的 `dict-v2.0.41` 发布（由其 `release-built-dictionaries.yml` 调用 msime 构建器生成），`sentence-model.safetensors` 来自 `metasequoiaime/chinese-ime-lm` 的 `model-v1`——两者是不同的仓库和不同的发布，以锁文件里各自的 `url` 为准。早先只供 Google 整句解码器使用的 `dict_pinyin.dat` 已随解码器一起去掉。**锁文件本身不记录许可证字段**，来源信息分散在别处：

| 产物 | 大小 | 已知来源 |
| --- | --- | --- |
| `msime-pinyin.db` | 73.9 MB | 拼音工作词库与快捷短语 |
| `msime-wubi.db` | 95.0 MB | 86 与 98 五笔码表，从拼音构建产物拆出；两版都在构建时按单字全码补入对方原表缺的多字词、rime-ice 主表与补充表里权重不低于 100 的多字词以及人工词条与区划词（`msime-dict-build` 的五笔补全，见 `crates/dict-builder/README.md`），自 `dict-v2.0.29` 起由约 14.5 MB 涨到这个大小。准备代次时并回工作主词库 |
| `msime-english.db` | 4.5 MB | Engine 发布的英文词库，英文词表含 SCOWL 的词，来源与条款见 `resources/licenses/msime-engine-dictionary-NOTICE.md` |
| `msime-scowl_Copyright.txt` | 4.0 KB | SCOWL 的版权与许可声明。SCOWL 的条款要求随附文档保留它，**分发 `msime-english.db` 时必须一同携带** |
| `msime-bigram.bin` | 12.0 MB | Engine 发布的二元语言模型表，整句词格仲裁按它加权 |
| `msime-trigram.bin` | 12.0 MB | Engine 发布的三元语言模型表，同上 |
| `msime-others.db` | 1.3 MB | Engine 发布的表情等数据 |
| `msime-japanese.dat` | 66.5 MB | Mozc 的开源版日文词库，构成见[下一节](#日文词库的分发义务) |
| `msime-mozc_dictionary_oss_README.txt` | 5.8 KB | 上述词库的许可证全文。**分发时必须一同携带**，理由见下节 |
| `msime-mozc_LICENSE.txt` | 5.7 KB | Mozc 仓库根目录的 `LICENSE`（Google 三条款 BSD 及词典目录的附加条款）。**分发时必须一同携带**，理由见下节 |
| `msime-dictionary-manifest.json` | 4.8 KB | 资源清单 |
| `sentence-model.safetensors` | 4.5 MB | 整句重排模型，权重为 Apache-2.0；训练语料与分发要求见[下下节](#整句重排模型的署名要求) |

`Artifact` 结构体带 `#[serde(deny_unknown_fields)]`，所以在锁文件里直接加 `license` 字段会让解析失败；要记录许可证需要同时修改 `crates/client-core/src/resources.rs`。在那之前，新增或更换随包资源时请把来源与授权写进本文件。

### 词库里要随附全文的 MIT 与 Apache-2.0 数据

MIT 要求每份副本都附上版权与许可声明，Apache-2.0 第 4(a) 条要求把许可证副本交给接收者；只在 `msime-engine-dictionary-NOTICE.md` 里写出来源名字不够。下表是随包词库里属于这两种许可、并且确实进入了产物的数据，「进入产物的依据」一列是 `crates/dict-builder` 里读取它的位置：

| 数据 | 上游 | 许可 | 进入的产物与依据 | 全文 |
| --- | --- | --- | --- | --- |
| 单字读音校正 | [mozillazg/pinyin-data](https://github.com/mozillazg/pinyin-data)，Copyright (c) 2016 mozillazg | MIT | `msime-pinyin.db`：msime-dictionary 的 `sources/pinyin/single-chars.txt` 读音以它校正，`quanpin` 阶段经 `self.sources.pinned("sources/pinyin/single-chars.txt")` 读入（`main.rs`）。`msime-others.db` 表情、颜文字与符号关键词的逐字读音来自 `pinyin` crate 0.11（`pinyin.rs`），它的拼音表就是同一份 pinyin-data，crate 本身也是 mozillazg 的 MIT，版权行相同 | `resources/licenses/pinyin-data-MIT.txt` |
| 中英释义 | [skywind3000/ECDICT](https://github.com/skywind3000/ECDICT) 提交 `bc015ed2e24a7abef49fc6dbbb7fe32c1dadaf8b` 的 `ecdict.csv`，Copyright (c) 2025 Linwei | MIT | `msime-english.db` 的双向释义表：`english.rs` 的 ECDICT 一节，经 `self.sources.pinned("ecdict/ecdict.csv")` 读入，文件由 `resources/dictionary-sources.lock.json` 固定 | `resources/licenses/ECDICT-MIT.txt`（与该提交的 `LICENSE` 逐字节相同） |
| 颜文字 | [aoguai/rime_kaomoji_dict](https://github.com/aoguai/rime_kaomoji_dict)，Copyright (c) 2023 aoguai | MIT | `msime-others.db` 的 `kaomoji`、`kaomoji_catalog` 表：`Stage::Kaomoji` 读 `resources/dictionary-sources/kaomoji/kaomoji.txt`（文件头写明整理自该仓库），`others.rs` 的 `build_kaomoji` 写入 | `resources/licenses/rime_kaomoji_dict-MIT.txt`（上游提交 `2f10ddca83f4cd87f41672442d27943f733b488f` 的 `LICENSE`；该文件自 2023 年起未变） |
| 关键词整词读音 | [mozillazg/python-pinyin](https://github.com/mozillazg/python-pinyin) 0.55 的 `lazy_pinyin` 输出，Copyright (c) 2016 mozillazg, 闲耘 | MIT | `msime-others.db` 表情、颜文字与符号关键词的拼音列：`resources/dictionary-sources/pinyin-overrides.txt`（60 行）经 `Pinyin::with_overrides` 覆盖多音字的逐字读音（`main.rs`、`pinyin.rs`）。这些是常用词的标准读音，按保守做法仍随附全文 | `resources/licenses/python-pinyin-MIT.txt`（标签 `v0.55.0` 的 `LICENSE.txt`） |
| 86 五笔码表 | [KyleBing/rime-wubi86-jidian](https://github.com/KyleBing/rime-wubi86-jidian) | Apache-2.0 | `msime-wubi.db` 的 `wubi86` 表与 86 词组补充表的编码：msime-dictionary 的 `sources/wubi/wubi86-jidian.txt`（`wubi86_supplement.rs` 的 `JIDIAN`），网页引擎的 `msime-wubi86.db` 由它裁出。上游 2026-10-29 在提交 `33a1f9430fa9bcb30b2f32b698e4e497f8864091` 改成 MIT，这张表是在那之前按 Apache-2.0 取用的，所以随附当时的条款；上游仓库没有 `NOTICE` 文件 | `resources/licenses/rime-wubi86-jidian-Apache-2.0.txt`（提交 `063377a0e3ae7d4084a45f03e4269f62f874f7dc` 的 `LICENSE`） |

这五份文本走与上文粤语、注音、笔画数据相同的全部通知渠道：Windows 的 `Collect-Notices.ps1`，macOS 输入法包的 `Resources/Licenses`（`CMakeLists.txt` 与 `THIRD_PARTY_NOTICES.txt`），Linux 安装的声明（`CMakeLists.txt` 与 `data/THIRD_PARTY_NOTICES.txt`），Android 的 `assets/native-notices`（`build-native.sh`），iOS App 的资源（`project.yml`，关于页「开源许可」逐个列出），HarmonyOS 的 `resfile/licenses`（`stage-resources.sh`）；引擎 C 库的数据包另由 `crates/engine-capi/packaging/data-pack.py` 带上 `pinyin-data-MIT.txt` 与 `ECDICT-MIT.txt`。网页引擎只分发拼音库和五笔 86 库，`web-engine-NOTICE.md` 收 `pinyin-data-MIT.txt` 与 `rime-wubi86-jidian-Apache-2.0.txt`。`scripts/test-language-data-notices.py` 检查文本与这些渠道都还在。

不在表里的几项：`source/SampleIMESimplifiedQuanPin.txt`（MIT）没有任何构建读取，不进 `msime-pinyin.db`，只被 `crates/input-runtime/examples/build_eval_set.rs` 固化成评测集；`msime-others.db` 表情英文关键词来自 Unicode Emoji 与 CLDR，其 Unicode License V3 全文已随引擎的符号表数据分发；McBopomofo 的 MIT 声明附在 `libchewing-data-LGPL-2.1.txt` 里；98 五笔表的 Unlicense 和行政区划的 WTFPL 不要求随附全文。

### 词库里改编自 rime-wanxiang 的权重（CC BY 4.0）

msime-dictionary 在 [#84](https://github.com/metasequoiaime/msime-dictionary/pull/84)（提交 `060c102515f176ca3dfaf2a2cba1f320901ef414`，自 `dict-v2.0.30` 起发布）把 `sources/pinyin/single-chars.txt`、`sources/pinyin/rime-ice.txt`、`sources/pinyin/rime-ice-supplement.txt` 里与万象共有的单字和词语的权重，改成 [amzxyz/rime-wanxiang](https://github.com/amzxyz/rime-wanxiang) 发布 `v18.0.16`（提交 `7ab73ee0c7f80190004233e70f954833df64c80e`）`dicts/` 下十五份词库的权重乘以每份文件一个系数（9.42、9.2、9.28）后取整；词条、读音和行序不变，没有从万象加入任何字、词或读音。上游以 CC BY 4.0 发布，第 3(a) 条要求分发时署名、给出许可证、写明改动。当前词库锁所指的 `dict-v2.0.39`（源数据提交 `75978c6a6470171cc805c74e87ef465d894a7be4`）里，这些权重进入的产物是：

| 产物 | 依据 | 分发渠道 |
| --- | --- | --- |
| `msime-pinyin.db` | `Stage::Quanpin` 读入上面三份文件的权重列（`main.rs`）。已逐条核对：发布的库里「我们」的权重 13696325 = round(万象 1488731 × 9.2)，「象」51951 = round(5515 × 9.42) | 六个平台、网页引擎（`msime-dict-build web` 保留全部单字与权重最高的多字词，权重原样）、引擎 C 库的全拼双拼数据包 |
| `msime-stroke.db` | `stroke.rs` 把 `single-chars.txt` 里一个字各读音的权重相加作为它的权重，例如「万」2702880 = 2701552 + 1328 | 语言词库资源包（`resources/language-dictionaries.lock.json`）：随安装包带上（Windows 放在 `server\language-dictionaries`）或由设置按需下载 |

署名、改动说明和 CC BY 4.0 全文在 `resources/licenses/rime-wanxiang-CC-BY-4.0.txt`，走与上一节五份文本相同的全部通知渠道（Windows `Collect-Notices.ps1` 与 `Notices.md`，macOS `Resources/Licenses`，Linux 安装的声明，Android `assets/native-notices`，iOS App 资源与关于页，HarmonyOS `resfile/licenses`），网页引擎的 `web-engine-NOTICE.md` 和 C 库数据包的 `licenses/` 也带它。`scripts/test-language-data-notices.py` 检查文件与这些渠道都还在，并要求三份总览和本文件写出上游提交。#10188 删掉同名文件时以为它只服务符号模式分组表；分组表确实已不再用 rime-wanxiang，但上面两个产物一直在分发它的权重，所以文件按权重的用途重写后恢复。

不算在内的几项，理由是没有分发 rime-wanxiang 的数据：

- `msime-wubi.db` 的 86、98 词组补充表在构建时重新派生，用 rime-ice 主表与补充表的权重判断一个词是否达到 100 的门槛、同一编码下补充词的先后，写进库里的权重是按名次从 9 递减的数，不含任何万象权重。
- `msime-bigram.bin`、`msime-trigram.bin` 只用 `msime-pinyin.db` 的词表切分语料，不读权重；万象没有加入任何词。
- 编进引擎的 `crates/engine/src/nine_key/syllable_frequency.tsv` 是 424 个音节在 `msime-pinyin.db` 全部词条里的权重之和，求和的权重里有改编自万象的部分；它按聚合统计事实处理，见下文「编译进共享库的数据」。
- msime-dictionary 的 `sources/pinyin/wanxiang/`（万象 `dicts/` 原样十五份文件，提交 `aa9947f3f513ff88a3f7ce4e342fcb519d517025`）到 `dict-v2.0.41` 仍只入库，构建器不读。
- 万象 LTS 语法模型 `wanxiang-lts-zh-hans.gram`（CC BY 4.0）不随任何安装包分发，设置应用按需从上游 `amzxyz/rime-wanxiang-data` 自己的发布下载（`resources/grammar-models.lock.json`），选择界面标明 CC-BY-4.0。

`resources/licenses/msime-engine-dictionary-NOTICE.md` 从「来源与授权说明」起到「msime 构建时另加的输入」之前，逐字取自 msime-dictionary 提交 `8e8b7817c91600913625edb7995a7397fbb6899a`（`dict-v2.0.41` 的源数据提交，见发布的 `msime-dictionary-manifest.json` 的 `custom_dictionary_commit`）的 `NOTICE.md`，其中有上面这次权重改编的完整记录；末尾一节记的是由 msime 一侧进入同一批词库、那份文件没有写的来源（颜文字、ECDICT、python-pinyin 读音、SCOWL 声明原文和中文维基百科的署名要求）。此前的副本停在 2026 年 9 月前后的版本，没有万象权重的记录。这份副本随每个分发桌面词库的渠道走：Windows `Collect-Notices.ps1`，macOS 的 `Resources/Licenses`，Linux 安装的声明，C 库数据包的 `licenses/`，Android 的 `assets/native-notices`（`build-native.sh`；APK 带拼音、英文、其他词库与两个 n-gram 表，五笔码表和日文词典随非瘦包带上或作为资源包下载），iOS App 的资源与关于页（键盘扩展的 `EngineResources` 带除日文词典外的全部桌面词库，日文词典按需下载），HarmonyOS 的 `resfile/licenses`（`resfile/engine` 带全部桌面词库）。移动端此前只带各份许可证全文、不带这份说明，n-gram 表要求的中文维基百科署名在那里没有出处；`test-language-data-notices.py` 现在要求九个渠道都带它。词库锁换到新的 `dict-v*` 时，按那次发布的源数据提交重新同步，`test-language-data-notices.py` 核对副本开头写的发布与词库锁一致。

### 独立神经模型清单（`resources/neural-model.lock.json`）

`resources/neural-model.lock.json` 把同一 `model-v1` 发布中的两个 safetensors 权重放在一起：`sentence-model.safetensors` 是键盘按键路径使用的小模型，`sentence-model-desktop.safetensors` 是桌面输入停顿后使用的大模型。两者都由 `scripts/fetch_neural_model.py` 按 HTTPS、字节数和 SHA-256 下载到 `target/neural-model`；文件不进版本库，锁和本节署名信息随仓库分发。发布的桌面安装包不再带大模型，由设置应用按需下载到 `resource-packs/settled-model/`；随包时（本地构建和发行版打包）放在词库目录的同级 `settled-model/`，因为词库目录必须与 `desktop-dictionary.lock.json` 完全相等。小模型仍由词库安装器从后者取回。

### 日文词库的分发义务

`msime-japanese.dat` 是 Mozc 的开源版词典，不是 Google 日本語入力所用的那一份。按随附 `msime-mozc_dictionary_oss_README.txt` 的说明，它由四部分构成：

- **IPAdic**（`mecab-ipadic-2.7.0-20070801`），奈良先端科学技術大学院大学 2000–2003 年版权。允许使用、复制和分发，但要求任何副本——无论原样还是修改过——都必须同时包含其版权声明和紧随其后的两段免责声明。
- **ICOT Free Software**，词条中很大一部分源于此。其条款要求 `NO WARRANTY` 一节**始终**出现在随程序分发的材料中，或附加于其上。
- **冲绳辞書**（[o-dic](http://sourceforge.jp/projects/o-dic/)），明示为 Public Domain，使用、修改、分发均无限制。
- Google 手工增补的形容词／动词、片假名词和复合词，适用 Mozc 自身的条款。该 README 未复述这部分；Mozc 仓库根目录 `LICENSE`（按锁定提交 `9fbd649` 读取）开头是署名 Google Inc. 的三条款 BSD 文本，适用于整个仓库，其后以 `Files: src/data/dictionary*` 为题对词典目录另附与 README 相同的 IPAdic、ICOT 和冲绳辞書条款。GitHub 对 `google/mozc` 的许可证识别结果是 `NOASSERTION`，本文件以 `LICENSE` 原文为准，不采用这个自动识别结果。

构建时还按 Mozc OSS 构建组装系统词典的方式，用同一提交的 `src/data/dictionary_oss/dictionary_filter.tsv` 删去基础词库中的个别词条，并用 `src/data/dictionary_oss/aux_dictionary.tsv` 与 `src/data/dictionary_manual/places.tsv`、`words.tsv` 增补词条（按锁定的 Mozc 提交 `9fbd649` 计，增 226 条、删 15 条）。这几份是 Google 维护的词表，位于 `src/data/dictionary*` 之下，因此和基础词库一样同时适用根目录 `LICENSE` 的 Google 三条款 BSD 文本与附加的 IPAdic、ICOT、冲绳辞書条款。

**实际后果：分发这份词库时必须一并携带 `msime-mozc_dictionary_oss_README.txt`**，IPAdic 和 ICOT 两条都把"许可证文本随附"写成了硬性条件。锁文件把这个 5.8 KB 的文本和词库本身一起固定并校验，正是为此——它是许可证义务，不是文档习惯，重新打包资源时不要因为"只是个 README"而丢掉它。**但只带这份 README 还不够**：Google 的三条款 BSD 要求以二进制形式再分发时附上 Google 的版权声明、条件列表和免责声明，而 README 里没有这段文字。为此 `msime-dictionary` 按锁定提交原样收录了该文件（`sources/japanese/LICENSE`），日文模型阶段把它复制成 `msime-mozc_LICENSE.txt` 与模型一起输出，`product.rs` 检查它存在、写进清单和 `msime-SHA256SUMS.txt`，缺了就不出产品。`dict-v2.0.7` 起它作为 `dict-v` 发布的附件、由本仓库的 `desktop-dictionary.lock.json` 登记，凡是装 `msime-japanese.dat` 的渠道都按锁文件同时装上它；macOS 的按需日文资源包（`resources.rs` 的 `ON_DEMAND_JAPANESE_ARTIFACTS`）也把它和 README 一起下载。

开源版不含日本邮政编码词典；README 给出了自行生成的步骤，本仓库没有执行。

### 整句重排模型的署名要求

`sentence-model.safetensors` 来自 [`metasequoiaime/chinese-ime-lm`](https://github.com/metasequoiaime/chinese-ime-lm) 的 `model-v1` 发布。它的许可信息不在任何外部文档里，而是嵌在权重文件自身的 safetensors `__metadata__` 头中——上游这样做正是为了让署名无法与权重分离。以下内容读自本仓库锁定的那一份（SHA-256 与 `desktop-dictionary.lock.json` 逐位一致）：

| 字段 | 值 |
| --- | --- |
| `license` | `Apache-2.0` |
| `attribution` | Trained on the Chinese portion of C4 (ODC-BY) and LCCC (MIT) |
| `precision` | `int8` |
| `version` | `1` |

两份训练语料的许可都要求署名随衍生成果传播：[C4 中文部分](https://huggingface.co/datasets/allenai/c4)为 ODC-BY，[LCCC](https://github.com/thu-coai/CDial-GPT)为 MIT。**再分发权重时必须保留 `__metadata__` 中的 `attribution` 字段**；任何重新导出、量化或转换权重的流程，如果丢掉 safetensors 的元数据头，就切断了这条署名链。

可以随时自行核对：

```sh
python3 -c '
import json,struct,sys
f=open(sys.argv[1],"rb"); n=struct.unpack("<Q",f.read(8))[0]
print(json.loads(f.read(n))["__metadata__"]["attribution"])
' <资源目录>/sentence-model.safetensors
```

上游仓库还说明，`corpus/fetch.py` 支持的中文维基百科、MDN、Kubernetes 文档等来源带有 share-alike 义务，**本仓库分发的这份权重不使用它们**。

## 自带辅助码表（`resources/helpcodes/`）

这是全仓唯一一项**带着明确再分发限制**的随包数据，不在上面两个锁文件的覆盖范围里，所以单列一节。完整说明在 [`resources/helpcodes/NOTICE.md`](../resources/helpcodes/NOTICE.md)，这里只做索引。

| 项 | 值 |
| --- | --- |
| 文件 | `resources/helpcodes/jiajia_helpcode.txt`（方案标识 `jiajia`，加加辅助码） |
| 来源仓库 | [metasequoiaime/MSIME-Windows](https://github.com/metasequoiaime/MSIME-Windows) 的 `engine/helpcode/helpcodes/jiajia_helpcode.txt` |
| 来源提交 | `566ff8b8320e7f56256544b2b1f0da8c8e7f037e` |
| 内容摘要 | `sha256:6538d744547b590630e93160198ac16bf76112a51ec78b4dbae505b914761879`，7968 行 |
| 登记方式 | `crates/engine/src/assets.rs` 的 `HELPCODES` 把它登记为第六套方案 `jiajia`，资源目录里的路径是 `helpcodes/jiajia_helpcode.txt` |
| 许可状态 | **本仓库的 GPL-3.0 不覆盖这张表的内容** |

按 NOTICE.md 的记录，本表的一部分条目直接来自拼音加加 5.x 安装包内的数据表 `fzm.bin`，而拼音加加是商业软件；来源仓库的 `engine/helpcode/NOTICE.md` 写明六张辅助码表没有任何一项拿到明确的再分发授权，并指出 `jiajia` 一行与其余五张性质不同（其余各表只是复现已发表的输入方案）。**在权利澄清之前不要假定这张表可以自由再分发**，打包发布前需确认它在目标渠道是否可接受。退出方式：从 `HELPCODES` 去掉 `jiajia` 那一行，并让该渠道的资源打包不再带上这张表；设置页随之少一个选项，其余各套不受影响。

构成这张表所用的部件拆分与笔顺数据另有来源（rime-radical-pinyin，GPL-3.0，上游含 chaizi/CC-BY-3.0、CHISE/GPL-2+、yi-bai/ids/MIT；笔顺来自 cnchar，MIT），逐条同样见 NOTICE.md。另外五套辅助码表原先随 Engine 归档而来，现在同样放在 `resources/helpcodes/`，来源说明是从 Engine 原样带过来的 [`resources/helpcodes/ENGINE-NOTICE.md`](../resources/helpcodes/ENGINE-NOTICE.md)：它们同样没有拿到明确的再分发授权。

第七套五笔 86（`wubi86`）不同：它由本仓库从 msime-dictionary 的 86 五笔码表生成，上游许可明确（极点码表 Apache-2.0，另有 16 个字的码来自并入的 rime-wubi 行，LGPL-3.0，按 GPL-3.0 分发），可以随包分发。来源、生成命令、摘要和 Apache-2.0 全文在 [`resources/helpcodes/NOTICE-wubi86.md`](../resources/helpcodes/NOTICE-wubi86.md)。

## 自带音效包与插件包（`resources/sound-packs/`、`client-services::plugins`）

内置的九套音效包和两套背景音乐包是本项目自己的作品，随各平台安装包分发，放在资源目录的旁边（资源目录必须与锁文件完全一致）。它们的标识登记在 `client-services::plugins` 的 `BUILTIN_SOUND_PACKS` 与 `BUILTIN_MUSIC_PACKS`，这些标识保留给内置包，用户导入的包不能占用。

| 项 | 值 |
| --- | --- |
| 文件 | `default/`：普通键、空格、回车、退格、上屏与里程碑共 6 段 WAV；`msime-typewriter/`、`msime-bubble/`、`msime-8bit/`、`msime-woodblock/`：普通键、空格、回车、退格与上屏各 1 段 WAV；`twinkle/`、`msime-pentatonic/`、`msime-canon/`、`msime-ode-to-joy/`：1 段音色 `tone.wav`，按键时按清单里的半音序列变调演奏；`msime-music-lofi/`、`msime-music-ambient/`：各 1 段 36 秒的无缝循环曲目（`lofi.wav`、`ambient.wav`） |
| 来源 | `scripts/generate_sound_packs.py` 用正弦、衰减包络和定种子的噪声逐样本合成，不录音、不下载任何素材 |
| 许可证 | CC0-1.0，写在各包的 `plugin.toml` 里 |
| 核对方式 | `scripts/test-sound-packs.py` 把样本重新合成一遍，与提交的文件逐样本比对（容差为 16 位量化的 1 级），不一致就失败，所以手工替换的样本进不来 |
| 旋律 | `twinkle` 的音符序列是《小星星》（法国民谣 "Ah! vous dirai-je, maman"，18 世纪）；`msime-canon` 取自帕赫贝尔《D 大调卡农》（约 1680 年）的上声部；`msime-ode-to-joy` 是贝多芬第九交响曲（1824 年）的《欢乐颂》主题；`msime-pentatonic` 只是 C 大调五声音阶的上下行。前三者都属公有领域，曲目说明写在各自 `plugin.toml` 的注释里。两段背景音乐也是生成脚本按程序写出的和声与节奏，不取材于任何既有作品 |

第三方插件包（音效、背景音乐、/指令表）由用户自行导入，本仓库不分发。每个包必须在清单里声明 `license`，设置页原样显示；包里只能有清单、清单点名的 WAV/Ogg 和文本说明，不能带任何可执行内容，`permissions` 必须为空。

导入插件包用到的 Rust crate：

| crate | 版本 | 许可证 | 用途 |
| --- | --- | --- | --- |
| `zip`（关闭默认特性，只开 `deflate-flate2`） | 8.6 | MIT | 导入用户选中的 `.zip` 插件包（`crates/client-services/src/plugins/import.rs`）。带进 `typed-path`（MIT OR Apache-2.0）；其余依赖 `crc32fast`、`indexmap`、`memchr`、`flate2` 原本就在锁文件里 |

播放音效包与背景音乐用到的 Rust crate，只链进 macOS、Windows、Linux 的 `host-api`（`crates/host-api/src/key_sound/`）；iOS、Android、HarmonyOS 不链接，HarmonyOS 2in1 用系统 SoundPool 播 `msime_client_key_sound_pack` 解析出的文件，WAV 样本先经上文的 miniaudio 在长度上限内解码、按旋律音高重写，背景音乐则由系统 AVPlayer 流式播放 `msime_client_music_pack` 解析出的曲目：

| crate | 版本 | 许可证 | 用途 |
| --- | --- | --- | --- |
| `kira`（关闭默认特性，只开 `cpal`、`wav`、`pcm`、`ogg`、`vorbis`） | 0.12 | MIT OR Apache-2.0 | 混音、复音、按半音变调播放旋律、流式播放背景音乐。输出走上表已有的 `cpal` 0.18，不另带一份。带进 `glam`（MIT OR Apache-2.0）、`mint`（MIT）、`rtrb`（MIT OR Apache-2.0）、`atomic-arena`（MIT OR Apache-2.0）、`triple_buffer`（MPL-2.0）；`send_wrapper`（MIT OR Apache-2.0）只在 wasm 目标下用到 |
| `symphonia`（关闭默认特性，只开 `wav`、`pcm`、`ogg`、`vorbis`） | 0.6 | MPL-2.0 | WAV 与 Ogg Vorbis 解码，是 `kira` 自己用的解码器；`host-api` 直接调用它，在包的时长上限内逐包解码（`decode.rs`），不整段解码后再检查。带进 `symphonia-core`、`symphonia-common`、`symphonia-metadata`、`symphonia-format-riff`、`symphonia-format-ogg`、`symphonia-codec-pcm`、`symphonia-codec-vorbis`（均为 MPL-2.0）、`extended`（MIT）、`regex-lite`（MIT OR Apache-2.0） |

设置窗口「插件」页导入插件时弹出的系统选择框，只链进 macOS、Windows、Linux 的 Tauri 外壳（`apps/desktop/src-tauri/src/platform/desktop/desktop_plugins.rs`）。选择框只由 Rust 端调用，没有任何 capability 把对话框命令开放给网页：

| crate | 版本 | 许可证 | 用途 |
| --- | --- | --- | --- |
| `tauri-plugin-dialog` | 2.7 | Apache-2.0 OR MIT | 用各平台自己的打开对话框选择插件包文件夹或 `.zip` 文件（macOS 的 NSOpenPanel、Windows 的通用对话框、Linux 的 GTK 文件选择器），在主线程弹出并以设置窗口为父窗口。停在 2.7，因为 2.8 要求 tauri 2.12。带进 `rfd`（MIT，实际的原生对话框实现）与 `tauri-plugin-fs`（Apache-2.0 OR MIT，本插件的依赖，同样没有开放给网页）；它们用到的 `dunce`、`glob`、`schemars`、`serde_repr` 等原本就在锁文件里 |

MPL-2.0 是文件级 copyleft，在没有附 Exhibit B 时与本仓库的 GPL-3.0 兼容（第 3.3 条）：这些 crate 未作修改，以二进制随包分发时附上许可证全文并指明源码位置（crates.io 上的对应版本）即可。本项目自己的 `msime-engine`、`msime-path-trust` 也是 MPL-2.0，源码就是本仓库。Linux 与 Windows 由 `platforms/linux/collect-notices.py` 从 Cargo 依赖图收集的通知覆盖，它把许可证不是 GPL-3.0-only 的 workspace 成员也当作 crate 收集，所以这两个 crate 的 `LICENSE` 同样进 `rust-crates-NOTICES.txt`；macOS 的输入法包不走这个脚本，由 `resources/licenses/MPL-2.0.txt`（取自 `symphonia-core` 0.6.1 的 LICENSE）复制成 `Contents/Resources/Licenses/MPL-2.0.txt`，并在 `platforms/macos/resources/Licenses/THIRD_PARTY_NOTICES.txt` 里列出这些 crate 与源码位置，`platforms/macos/tests/settings/bundle_contents.py` 检查它在包里。iOS、Android 和 HarmonyOS 的引擎同样链接 rink-core、也就是 MPL-2.0 的引擎本身，三者不走收集脚本，同一份 `MPL-2.0.txt` 由 iOS 的 `project.yml`（App 资源）、Android 的 `build-native.sh`（`assets/native-notices`）和 HarmonyOS 的 `stage-resources.sh`（`resfile/licenses`）打包；网页引擎的 NOTICE 收的是 `crates/engine/LICENSE`。`scripts/test-engine-license.py` 检查这些渠道都还在。

## 背单词词书（`resources/wordbook.lock.json`）

背单词模式的八本内置词书来自 ECDICT，不在 `desktop-dictionary.lock.json` 的覆盖范围里，所以单列一节。

| 项 | 值 |
| --- | --- |
| 来源仓库 | [skywind3000/ECDICT](https://github.com/skywind3000/ECDICT) |
| 来源提交 | `82c9872576b23118d7c42e920c11beb77f510ae2` |
| 文件 | `ecdict.csv` |
| 内容摘要 | `sha256:1a6947e04785db63613a92e14903cdae7954f7e84860b10e68e5c7cbb3f9c3cf`，65,933,428 字节 |
| 许可 | MIT，`Copyright (c) 2025 Linwei` |
| 取用方式 | `scripts/fetch_wordbooks.py` 按锁校验后展开到被忽略的 `target/wordbooks/` |
| 分发限制 | MIT，保留版权声明与许可全文即可；与本仓库的 GPL-3.0 分发兼容 |

**源文件不进版本库，也不整份随包**。脚本只取 `word`、`phonetic` 和 `translation` 的首行释义——卡片要显示的就这三样——按 `tag` 列（`zk`/`gk`/`cet4`/`cet6`/`ky`/`ielts`/`toefl`/`gre`）分成八本，写成 `client-services::vocabulary::wordbook::Wordbook` 直接能反序列化的 JSON。八本合计约 3 MB，宿主用 serde_json 读，不需要 CSV 解析器或 SQLite 驱动。

考纲归属是 ECDICT 提供的：仓库自带的 `msime-english.db` 有中英释义和语料频次，够做「最常用的一千词」，但不足以断言某个词在四级大纲里。那是已发布的考纲，凭频次给它安一个名字就是编的。

词书**放在 `EngineResources/` 的兄弟目录 `wordbooks/`**，不放进去：`ResourceStore::verify` 要求固定资源目录与词库锁逐字节一致，多一个文件就会破坏那道「证明随包词库完整」的检查。`settled-model` 出于同样理由也是兄弟目录。

退出方式：不运行 `scripts/fetch_wordbooks.py` 即可。设置页随之只提供用户自行导入的词表，不影响其余功能。

## 非英语离线释义（`resources/offline-glosses.lock.json`）

候选词释义的目标语言是法语、日语、西班牙语、俄语、德语或韩语时，离线来源是从英文维基词典译文表生成的六个 SQLite 文件。`msime-english.db` 只有中英释义，这几种语言原来只能走在线翻译。

| 项 | 值 |
| --- | --- |
| 来源 | [英文维基词典](https://en.wiktionary.org/)（Wiktionary contributors），经 [Wiktextract](https://github.com/tatuylonen/wiktextract) 抽取，由 [kaikki.org](https://kaikki.org/dictionary/English/) 发布 |
| 许可 | CC BY-SA 4.0（维基词典按 CC BY-SA 4.0 与 GFDL 双许可，这里取前者）。CC BY-SA 4.0 可单向兼容到 GPL-3.0 |
| 生成器 | `scripts/build_offline_glosses.py`，只用 Python 标准库；离线测试 `scripts/test-offline-glosses.py` 用真实结构的夹具 `scripts/offline-glosses-fixture.jsonl` |
| 产物 | `offline-glosses/zh-<lang>.db`，每种语言一个文件，外加 `offline-glosses-NOTICE.txt` |
| 取词范围 | 同一张译文表（同一个英文义项）里的 Mandarin 行与目标语言行配对；读 `senses[].translations` 和顶层 `translations`；键为简体，用 `msime-pinyin.db` 的词表校验；每个中文词最多两个义项、每个义项最多两个词 |
| 锁 | `resources/offline-glosses.lock.json`：`input` 是 kaikki dump，`filtered_input` 是生成时实际读的过滤后 jsonl，`artifacts` 是六个数据库和 NOTICE |
| 发布位置 | [`metasequoiaime/chinese-ime-lm` 的 `offline-glosses-2026.09.02`](https://github.com/metasequoiaime/chinese-ime-lm/releases/tag/offline-glosses-2026.09.02)，与整句重排模型同一个仓库，不在应用内更新检查读取的 `metasequoiaime/msime/releases` |
| 获取 | `scripts/fetch_offline_glosses.py` 按锁下载并校验 sha256 与大小，默认写到 `target/offline-glosses` |

Android、iOS、macOS、Windows 与 Linux 的发布工作流在打包前运行 `fetch_offline_glosses.py`，各自的打包脚本发现 `target/offline-glosses` 里有数据库和 NOTICE 时才带上它们。HarmonyOS 的发布工作流还不能在 CI 上出包（缺 DevEco 的 NDK，也不暂存资源），本地按 `platforms/harmony/README.md` 构建时 `stage-resources.sh` 会带上它们。本地构建同样先运行这个脚本，不运行则照常构建、只有英语走离线释义。

kaikki 每周覆盖同一个 URL，所以能复现构建的是 `filtered_input`，用它加上锁住的 `msime-pinyin.db` 与 `msime-english.db` 运行 `build_offline_glosses.py build`（`--dump-date`、`--source-revision` 取锁里的值）即可得到逐字节相同的数据库，前提是 SQLite 版本与锁里的 `sqlite_version` 一致。上游已把 postprocessed 的 English jsonl 标为 deprecated，将来可能只剩约 2.9 GB 的 raw dump；生成器两种格式都接受。

**放在 resources 的兄弟目录 `offline-glosses/`**，不放进去，理由与 `settled-model`、`wordbooks` 相同：资源目录必须和词库锁逐字节一致，而且各平台只想带自己需要的语言。

**分发义务**：这些文件是维基词典文本的改编作品，发布时必须随附 `offline-glosses-NOTICE.txt`，并保持 CC BY-SA 4.0。App Store 这类带 DRM 的渠道与 CC BY-SA 4.0 第 2(a)(5) 条「不得附加有效技术措施」之间的关系，和已随包分发的 bigram/trigram 是同一个问题，需要维护者判断。

退出方式：不安装 `offline-glosses/` 即可。释义退回只用 `msime-english.db` 和在线翻译（macOS 26 及以上在没有选择翻译服务时还有系统自带的离线翻译，见 [PRIVACY.md](../PRIVACY.md#候选翻译默认不联网)），其余功能不受影响。

## 粤语与注音的数据（rime-cantonese、libchewing-data）

粤语（粤拼）与注音（大千）两个方案的音节和词条来自两份固定提交的上游数据，由 `msime-dict-build languages` 转换成 `msime-cantonese.db` 与 `msime-zhuyin.db`。两份数据库放在资源目录的兄弟目录 `language-dictionaries/`，不进 `desktop-dictionary.lock.json`，上面「九个产物」不变；理由与 `offline-glosses/` 相同：资源目录必须和词库锁逐字节一致，而且只有 macOS 与 Windows 提供这两个方案（Windows 安装包放在 `server\language-dictionaries`，与 `server\resources` 同级）。数据库不在时，这两个方案显示为不可用。

| 组件 | 许可证 | 位置与说明 |
| --- | --- | --- |
| [rime/rime-cantonese](https://github.com/rime/rime-cantonese) 的 `jyut6ping3.chars.dict.yaml`、`jyut6ping3.words.dict.yaml`、`essay-cantonese.txt`，提交 `259f0e48bba840c3a2e0d117539e96937f3d89bc`；以及 [fcbond/hkcancor](https://github.com/fcbond/hkcancor)（香港粤语语料库 HKCanCor）的 `data/utf8/` 转写，提交 `39aeadf920e0b5ca93d0ad7792c59e740e7bdd65`，经 msime-dictionary 的 `sources/cantonese/hkcancor-word-counts.txt` 使用 | 两者都是 CC BY 4.0（rime-cantonese 作者为粤语计算语言学基础建设组 CanCLID，拼写采用香港语言学学会 LSHK 的粤拼方案；HKCanCor 由陆镜光 Luke Kang Kwong 建立）。全文、署名与改动说明在 `resources/licenses/rime-cantonese-CC-BY-4.0.txt` | 转换成 `msime-cantonese.db`：去掉声调数字、音节以空格连接，字频与词频取自 `essay-cantonese.txt`；essay 没收的词按 HKCanCor 词频乘以两者共有词的频率比中位数取权重，语料文本本身不分发。按 ODbL 发布的 `jyut6ping3.maps.dict.yaml`、来源不明且没有读音的 `jyut6ping3.phrase.dict.yaml` 和 `jyut6ping3.lettered.dict.yaml` 都不读取、不分发。CC BY 4.0 要求署名并说明改动，许可证文件已写明两者 |
| [chewing/libchewing-data](https://github.com/chewing/libchewing-data) 的 `dict/chewing/tsi.csv`、`dict/chewing/word.csv`，提交 `c44e81aef24b06f1509f19e1be54c99812d0c43f`；以及 [openvanilla/McBopomofo](https://github.com/openvanilla/McBopomofo) 的 `Source/Data/BPMFMappings.txt` 与 `Source/Data/phrase.occ`，提交 `be6564acad6c4d3265c34a2e1a872d80f9db6068` | libchewing-data 为 LGPL-2.1-or-later（两个文件的文件头 `dc:license` 声明，Copyright (c) 2025 libchewing Core Team）；McBopomofo 多字词表说明它由 libtabe 的 `tsi.src`（BSD-3-Clause，`tsi-src/COPYING` 列出 TaBE Project、Pai-Hsiang Hsiao、中研院资讯所 CSCL 与 Chih-Hao Tsai 的版权）简化并修改而来，McBopomofo 的修改随项目以 MIT 发布；`phrase.occ` 是 McBopomofo 自己的语料计数，同样是 MIT（Copyright (c) 2011-2026 Mengjuei Hsieh et al.）。数据库 `license` 元数据因此记为 `LGPL-2.1-or-later AND BSD-3-Clause AND MIT`（`dict-v2.0.40` 及以前的发布只记了 `LGPL-2.1-or-later`）。三者的署名、源码地址和条款原文（含 libtabe 的 `tsi-src/COPYING`）在 `resources/licenses/libchewing-data-LGPL-2.1.txt` | 转换成 `msime-zhuyin.db`：`tsi.csv` 按词频排序，`word.csv` 补全单字，McBopomofo 补充表补入其余词语组合并以 0 作为无来源频率；`tsi.csv` 在任何读音下都没计数的两字及以上词语，权重取 `phrase.occ` 计数乘以两者共有词的频率比中位数，并压在同一读音下有计数的最高词之下。仓库里其他注音文件不使用；注音从不读 `msime-pinyin.db`，也不经过简繁转换。若转换本身算作修改，对应的源码是本仓库以 GPL-3.0 发布的 `crates/dict-builder`。 |

这些文件由 [msime-dictionary](https://github.com/metasequoiaime/msime-dictionary) 原样收在 `sources/cantonese/`、`sources/zhuyin/` 下（与上游固定提交逐字节一致）。`resources/dictionary-sources.lock.json` 用 `rime-cantonese`、`libchewing-data` 引用记下上游提交；文件字节由 msime-dictionary 的 `upstream.lock.json` 固定，构建器读取 checkout 时逐个校验，并要求记录的提交等于这两个引用。源文件只保存在仓库 Git 中，不再发布 `sources-v*` Release；构建后的数据库和许可证文件统一随 `dict-v*` Release 发布。许可证文本经各平台的通知渠道分发，与下一节的 libhangul 汉字表相同：Windows 的 `Collect-Notices.ps1`，macOS 输入法包的 `Resources/Licenses`（`CMakeLists.txt` 与 `THIRD_PARTY_NOTICES.txt`），Linux 安装的声明（`CMakeLists.txt` 与 `data/THIRD_PARTY_NOTICES.txt`），Android 的 `assets/native-notices`（`build-native.sh`），iOS App 的资源（`project.yml`），HarmonyOS 的 `resfile/licenses`（`stage-resources.sh`）。除 macOS 外的平台目前不提供这两个方案，也不带数据库，但这些方案的代码随每一份引擎分发，统一一份渠道清单检查起来最简单。`scripts/test-language-data-notices.py` 检查许可证文本和这几处渠道都还在。

## 笔画的数据（rime-stroke）

笔画方案（h 横、s 竖、p 撇、n 点、z 折）的笔顺码来自一份固定提交的上游数据，由 `msime-dict-build languages` 转换成 `msime-stroke.db`，与 `msime-cantonese.db`、`msime-zhuyin.db` 放在同一个 `language-dictionaries/` 目录、同一个语言词库资源包里。数据库不在时，笔画方案显示为不可用。

| 组件 | 许可证 | 位置与说明 |
| --- | --- | --- |
| [rime/rime-stroke](https://github.com/rime/rime-stroke) 的 `stroke.dict.yaml`，提交 `1e8fff9b9494ddec23b0cbc526bcfd8171a6fd48` | LGPL-3.0（仓库的 `LICENSE`；`AUTHORS` 记载四季的風、雪齋、Kunki Chou 整理的主码表依 CNS11643 资料的授权声明以 LGPL 再发布，扩展至 Ext J 的数据来自宋天，同为 LGPL）。主码表源自 CNS11643 全字库，该资料按「政府資料開放授權條款－第1版」要求署名：數位發展部，CNS11643中文標準交換碼全字庫網站，https://www.cns11643.gov.tw。附码表源自北大中文論壇（孙海峰、徐孟罗、唐捺之、谢振斌整理）。全文、署名与固定提交的源码地址在 `resources/licenses/rime-stroke-LGPL-3.0.txt` | 转换成 `msime-stroke.db`：每行 `字<TAB>笔顺码` 原样成为一条，键就是笔顺字母串；一个字的多个笔顺（大陆规范与台湾 CNS11643 笔顺并列）各成一条。上游没有权重，排序用 `sources/pinyin/single-chars.txt`（rime-ice，GPL-3.0-only）里每个字各读音权重之和，不在表里的字权重为 0。`dict-v2.0.30` 起这份字频里与万象共有的读音改编自 rime-wanxiang（CC BY 4.0，见上文「词库里改编自 rime-wanxiang 的权重」），所以数据库 `license` 元数据记为 `LGPL-3.0-only AND GPL-3.0-only AND CC-BY-4.0`，随库分发的 `rime-stroke-LGPL-3.0.txt` 也写着万象的作者、固定提交、许可证地址和改动，完整署名与 CC BY 4.0 全文在随各渠道分发的 `rime-wanxiang-CC-BY-4.0.txt`。`dict-v2.0.40` 及以前发布的笔画库还只记着 `LGPL-3.0-only AND GPL-3.0-only`，随它发布的许可证文本也还没有万象一段；`dict-v2.0.41` 起用改正后的构建器发布，库里的表达式和随库的许可证文本都是新的。只有单字，不含词组。上游数据原样未改时，随附许可证全文和固定提交的源码地址即满足 LGPL 的源码提供义务；若转换本身算作修改，对应的源码是本仓库以 GPL-3.0 发布的 `crates/dict-builder`。这一判断未经法务审阅 |

msime-dictionary 原样收录了 `stroke.dict.yaml`（`sources/stroke/stroke.dict.yaml`），`msime-dict-build languages` 必须传 `--dictionary`，同样从 msime-dictionary checkout 读取它，并按 `upstream.lock.json` 校验。`crates/dict-builder/src/stroke.rs` 里的 `SOURCE_SIZE`、`SOURCE_SHA256` 只守着 `--cache` 兜底分支，这个分支在命令行上已经走不到；`COMMIT` 和锁文件里的 `rime-stroke` 引用必须仍是上面的提交。许可证文本走与上一节相同的全部通知渠道，`scripts/test-language-data-notices.py` 一并检查。

## 英文读音表（`resources/pronunciations.lock.json`）

打开「显示读音」后，候选的英文释义行后面显示音标。`english.db` 的构建丢弃了 ECDICT 的 `phonetic` 列，所以音标单独生成一个文件，与背单词词书取自同一份锁定的 ECDICT。

| 项 | 值 |
| --- | --- |
| 来源仓库 | [skywind3000/ECDICT](https://github.com/skywind3000/ECDICT) |
| 来源提交 | `82c9872576b23118d7c42e920c11beb77f510ae2`（与 `wordbook.lock.json` 相同） |
| 文件 | `ecdict.csv`（`sha256:1a6947e0…c3cf`）与 `LICENSE`（`sha256:f8552dd2…ed0f`，1,063 字节），都在锁里 |
| 许可 | MIT，`Copyright (c) 2025 Linwei` |
| 生成器 | `scripts/build_pronunciations.py`，只用 Python 标准库，下载与校验复用 `fetch_wordbooks.fetch` |
| 产物 | `pronunciations/en-phonetic.db`（`en_phonetics(word, phonetic)`，`meta.kind = en_phonetic`，`user_version = 1`，约 18.8 万词、5.7 MB），外加 `pronunciations-NOTICE.txt`（含 ECDICT 的 LICENSE 全文） |
| 取词范围 | 纯 ASCII 字母词（可含 `-`、`'`），键为小写；同键时取小写词头那一行；`phonetic` 只取第一种读法，西里尔 `ә`、ASCII `'`/`:`/`,` 等写法规范成 IPA，规范后仍含 IPA 以外字符（中文注释、私用区字符、不配对括号）的整条丢弃；ECDICT 约七成条目是旧式英式注音（`dei`、`həˈləu`、`bəːd`、`buk`），生成时按规则改成现行学习词典的写法（`deɪ`、`həˈləʊ`、`bɜːd`、`bʊk`），词尾与元音前的 `i` 保留 happY 惯例，已是现行写法的部分不变 |

同样**放在 resources 的兄弟目录 `pronunciations/`**，理由与 `offline-glosses/` 相同。生成物发布在 [`metasequoiaime/chinese-ime-lm` 的 `pronunciations-ecdict-82c9872`](https://github.com/metasequoiaime/chinese-ime-lm/releases/tag/pronunciations-ecdict-82c9872)，锁的 `built` 记它们的地址、长度和 SHA-256；`scripts/fetch_gloss_tables.py` 下载到 `target/pronunciations`，macOS 与 Windows 的发布工作流在打包前调用它（本节与下面两节的三张表共用这个脚本）。

各平台怎么拿到这两个文件：macOS 与 Windows 随安装包带着；鸿蒙没有资源包下载，`platforms/harmony/stage-resources.sh` 按锁核对长度和 SHA-256 后放进 HAP 的 `resfile/pronunciations/`（约 5.7 MB），键盘把它暂存到引擎资源旁；Linux、Android 与 iOS 不随包，打开「显示读音」后在设置那一行点「下载」，才由共享资源包安装器按锁取回，装到状态目录的 `resource-packs/pronunciations/`，宿主把这个目录作为 `supplementary_resources` 交给查询。NOTICE 都和数据库放在一起。

退出方式：不安装 `pronunciations/` 即可。`msime_client_pronunciation_request` 返回空结果，释义照旧显示、只是没有音标。日语罗马音在 macOS 上由系统的 `CFStringTokenizer` 生成，在 Windows 上由微软日语输入法的 IFELanguage 生成，不需要数据文件；其他平台的日语罗马音来自下一节的读音表。

## 法日西俄德韩的读音表（`resources/target-pronunciations.lock.json`）

打开「显示读音」后，法、日、西、俄、德、韩语的释义行下面显示读音：日语是赫本式罗马字，其余是维基词典的国际音标。来源与上文「非英语离线释义」相同，都是英文维基词典经 Wiktextract 抽取、由 kaikki.org 发布的数据，同一个许可；离线释义取英语词条的译文表，读音取目标语言词条自己的 `sounds`（日语取 `forms` 里的罗马字），kaikki.org 把它们按语言分开发布。

| 项 | 值 |
| --- | --- |
| 来源 | [英文维基词典](https://en.wiktionary.org/)（Wiktionary contributors），经 [Wiktextract](https://github.com/tatuylonen/wiktextract) 提交 `f9fe161b9dd1e898352c51bef7295e774bba2ef1` 抽取（维基词典 2026-10-01 的转储，2026-10-25 抽取），由 [kaikki.org](https://kaikki.org/dictionary/) 发布的 `kaikki.org-dictionary-{French,Japanese,Spanish,Russian,German,Korean}.jsonl.gz` |
| 许可 | CC BY-SA 4.0（维基词典按 CC BY-SA 4.0 与 GFDL 双许可，这里取前者），与离线释义相同 |
| 生成器 | `scripts/build_target_pronunciations.py`，只用 Python 标准库；离线测试 `scripts/test-target-pronunciations.py` |
| 产物 | `target-pronunciations/<lang>-phonetic.db` 六份（`<lang>_phonetics(word, phonetic)`，`meta.kind = <lang>_phonetic`，`user_version = 1`，每种 1.4 万到 2.1 万词，合计约 3.7 MB），外加 `target-pronunciations-NOTICE.txt` |
| 取词范围 | 只收离线释义（`offline-glosses.lock.json` 的六份 `zh-<lang>.db`）用到的词条及其小写；去掉屈折形式、罗马化页面、字母和词缀；IPA 优先取斜线的音位标音、不带地区标签的或该语言标准音的那一条（法语反过来，标 `France`、`Europe` 的标准音排在不带标签的南部变体前面），古旧、方言、非标准的不用，一格里有两种读法或带注释的整格、只在联诵时出现的和只标了一部分的不用；日语取罗马字，同一词性的同形词条先取带假名并标了声调的，并把汉字词条的假名读法也作为键，片假名释义折成平假名去对 |
| 锁 | `resources/target-pronunciations.lock.json`：`inputs` 是六个 kaikki dump（地址、ETag、长度、SHA-256），`filtered_input` 是生成时实际读的过滤后 jsonl，`vocabulary` 指向离线释义的锁，`built` 是六个数据库和 NOTICE |
| 发布位置 | 数据库和 NOTICE 在 [`metasequoiaime/chinese-ime-lm` 的 `target-pronunciations-2026.10.25-r2`](https://github.com/metasequoiaime/chinese-ime-lm/releases/tag/target-pronunciations-2026.10.25-r2)，过滤后的输入在 [`target-pronunciations-2026.10.25`](https://github.com/metasequoiaime/chinese-ime-lm/releases/tag/target-pronunciations-2026.10.25)（那里的第一版数据库已被取代） |
| 获取 | `scripts/fetch_gloss_tables.py` 按锁下载并校验，写到 `target/target-pronunciations` |

离线释义锁住的 2026-09-02 那一周的 dump 已被 kaikki.org 覆盖，过滤后的输入里也只有英语词条，所以读音用的是 2026-10-25 那一周；读音按目标语言的词查，与释义来自哪一周无关，下次刷新离线释义时两者从同一周的 dump 一起重建。能复现构建的同样是 `filtered_input`：用它加上离线释义锁住的六个数据库运行 `build_target_pronunciations.py build --glosses <目录>`（`--dump-date`、`--source-revision` 取锁里的值），在锁里的 `sqlite_version` 下逐字节相同。原始 dump 合计约 420 MB，在 Mac Studio 上用 `filter` 过滤；`filter` 只看 `lang_code`，将来按语言的 dump 停发时读约 3 GB 的 raw dump 也一样。

**放在 resources 的兄弟目录 `target-pronunciations/`**，理由与 `pronunciations/` 相同；目录名就是资源包 id，不和英文音标表放在一起：资源包按锁里的文件清单严格判定是否已装，两张表合成一个包时，已经下载过英文音标表的用户会全部变成「需更新」。

各平台怎么拿到：macOS 与 Windows 随安装包带着（`platforms/macos/stage-resources.sh`、`package-release.sh`，`platforms/windows/installer/Prepare-PackageFiles.ps1`），不分版本；鸿蒙由 `platforms/harmony/stage-resources.sh` 按锁核对后放进 HAP 的 `resfile/target-pronunciations/`，`MSIME_REQUIRE_PRONUNCIATIONS=1` 时也必须在；Linux、Android 与 iOS 不随包，打开「显示读音」且翻译目标语言是这六种之一时，设置里多一行「离线外语读音词典」，点「下载」才由共享资源包安装器按锁取回，装到状态目录的 `resource-packs/target-pronunciations/`。

**分发义务**：这些文件是维基词典文本的改编作品，NOTICE 必须和数据库一起分发，并保持 CC BY-SA 4.0；与离线释义相同，App Store 这类渠道和 CC BY-SA 4.0 第 2(a)(5) 条之间的关系需要维护者判断。`scripts/test-language-data-notices.py` 检查 NOTICE 在锁里、生成器写的 NOTICE 带着署名、许可证和改动说明，以及三个随包脚本暂存数据库时都带上了它。

退出方式：不安装 `target-pronunciations/` 即可。这些语言的释义照旧显示、只是没有读音；macOS 和 Windows 的日语罗马音不受影响。

## 单字英文释义（`resources/character-glosses.lock.json`）

`english.db` 的中译英是把 ECDICT 反查得到的，只收 2 到 6 字的词（单字出现在太多英文词的译文里，反查没有意义），所以爱、天、我这样的单字候选原来没有英文释义。单字释义改用逐字编写的来源。

| 项 | 值 |
| --- | --- |
| 来源 | Unicode 汉字数据库 [Unihan](https://www.unicode.org/charts/unihan.html) 的 `kDefinition` 字段，Unicode 17.0.0 |
| 文件 | `https://www.unicode.org/Public/17.0.0/ucd/Unihan.zip`，`sha256:f7a48b2b…4b5e`，8,518,517 字节 |
| 许可 | Unicode License v3，宽松许可，与 GPL-3.0 兼容；全文收在 `resources/licenses/Unicode-License-V3.txt`（取自 `https://www.unicode.org/license.txt`，该地址不分版本，所以原文进了仓库，锁里记摘要） |
| 生成器 | `scripts/build_character_glosses.py`，只用 Python 标准库；离线测试 `scripts/test-character-glosses.py` 用 Unihan 17.0.0 的真实释义 |
| 产物 | `character-glosses/zh-en.db`（与离线释义同一结构：`zh_glosses(chinese, gloss, source)`，`meta.target_language = en`，`user_version = 1`，约 2.3 万字、1.2 MB），外加含许可全文的 `character-glosses-NOTICE.txt` |
| 取词范围 | 去掉辞书附注（部首编号、姓氏、干支、星宿、`numerary adjunct` 之类、括号注记），每字最多两个义项、每义项最多三个词；Unihan 标注「simp. for」的义项排到前面；简化字本身没有释义时取其繁体的释义；少数简化字 Unihan 只给了其本字的冷僻义（几、里、干等），在脚本的 `CORRECTIONS` 表里逐字改写 |

候选恰好是一个字时 host-api 先查这张表，再查下一节的词表（`english.db` 本就没有单字）；用户自己已学的释义不被替换，表缺失或损坏时保持原样。生成物发布在 [`metasequoiaime/chinese-ime-lm` 的 `character-glosses-unihan-17.0.0`](https://github.com/metasequoiaime/chinese-ime-lm/releases/tag/character-glosses-unihan-17.0.0)，由 `scripts/fetch_gloss_tables.py` 按锁的 `built` 下载。退出方式：不安装 `character-glosses/` 即可。

## 词语英文释义（`resources/word-glosses.lock.json`）

`english.db` 的中译英只收能对上常用英文词的中文词，约 1.86 万条；日语离线释义约 2.79 万条，其中 1.82 万个词有日语而没有英语（芋头、贺卡、店员、支票簿）。这张表从中文一侧编写的汉英词典补上英语。

| 项 | 值 |
| --- | --- |
| 来源 | [CC-CEDICT](https://cc-cedict.org/)，MDBG 发布的社区汉英词典；CEDICT - Copyright (C) 1997, 1998 Paul Andrew Denisowski |
| 文件 | `cedict_1_0_ts_utf-8_mdbg.zip`，2026-10-11 导出，`sha256:25bd79c7…e324`，3,980,433 字节；锁里的地址指向 `metasequoiaime/chinese-ime-lm` 上的镜像 |
| 许可 | CC BY-SA 4.0，与非英语离线释义相同，可单向兼容到 GPL-3.0 |
| 生成器 | `scripts/build_word_glosses.py`，只用 Python 标准库；离线测试 `scripts/test-word-glosses.py` 用真实词条 |
| 产物 | `word-glosses/zh-en.db`（离线释义同一结构，`meta.target_language = en`，约 9.8 万条、4.8 MB），外加 `word-glosses-NOTICE.txt` |
| 取词范围 | 去掉量词 `CL:`、`variant of`、`see`、姓氏、`abbr. for`、台湾读音等非释义项与交叉引用的拼音，每条最多两个义项、每义项最多三个短语；一个简体词有多个读音条目（便宜 biànyí/piányi）时各取第一个义项；专名（拼音首字母大写）排在最后 |

MDBG 每天用新导出覆盖同一个地址（`https://www.mdbg.net/chinese/export/cedict/cedict_1_0_ts_utf-8_mdbg.zip`），所以锁住的 2026-10-11 导出镜像在 [`metasequoiaime/chinese-ime-lm` 的 `word-glosses-2026.10.11`](https://github.com/metasequoiaime/chinese-ime-lm/releases/tag/word-glosses-2026.10.11)，锁的 `artifact.url` 指向它，从锁重建得到与发布物逐字节相同的产物。生成物也发布在同一个 release，由 `scripts/fetch_gloss_tables.py` 按锁的 `built` 下载。换新导出要先镜像、再同时更新锁。

这个输入法也是学英语的工具，所以释义以准确为先：词表有的词，显示词表的释义而不是 `english.db` 的（`english.db` 有些释义过于宽泛，漂亮 → chic、电源 → ps）；`english.db` 只回答词表没有的词和英文候选，用户自己已学的释义始终保留。CC-CEDICT 同一个字的多个读音按拼音排序而不是按常用程度（要 yāo 在 yào 前），所以 host-api 对单字先查 Unihan 表，词表只补 Unihan 没有的字；对词语直接查词表。**分发义务**：发布时必须随附 `word-glosses-NOTICE.txt` 并保持 CC BY-SA 4.0。退出方式：不安装 `word-glosses/` 即可。

## 编译进共享库的数据

| 组件 | 许可证 | 位置与说明 |
| --- | --- | --- |
| [rimeinn/rime-mungyeong 的罗马字码表](https://github.com/rimeinn/rime-mungyeong/blob/34c1b1437824e1e619e7429fb5f461dc7e754939/mungyeong_romaja.dict.yaml)，提交 `34c1b1437824e1e619e7429fb5f461dc7e754939` | 该码表文件头声明 public domain；此声明不代表整个仓库的许可证 | `crates/engine/src/korean/romaja.rs` 的拉丁拼写到韩文字母映射，复用本仓韩文组合规则。没有复制上游 Lua 处理器或引入 Rime 依赖；退格按原始拉丁按键删除，行为差异见[布局说明](korean-romaja.md) |
| [modood/Administrative-divisions-of-China](https://github.com/modood/Administrative-divisions-of-China) 的省、地、县三级行政区划，提交 `c49d495b40ac73eb1a66f6eeae5f8fd10696f035` | WTFPL（全文在 `resources/licenses/Administrative-divisions-of-China-WTFPL.txt`）；上游整理自国家统计局公布的统计用区划代码与城乡划分代码 | `crates/engine/src/local/places.tsv`，`@` 模式在用户自己的列表之后补充的内置地名（3302 个，约 126 KB，`include_str!` 编进引擎）。`msime-dict-build places` 读 `dist/provinces.csv`、`dist/cities.csv`、`dist/areas.csv` 三个文件生成这张表，三者的 URL、长度与 SHA-256 固定在 `resources/dictionary-sources.lock.json` 的 `places/` 条目；拼音由生成器按字注音，再用 `crates/dict-builder/src/places.rs` 的 `READINGS` 纠正地名专用读音。只取地名和上级关系，不带区划代码。设置里「@ 地名」默认关闭 |
| [libhangul](https://github.com/libhangul/libhangul) 的 `data/hanja/hanja.txt`，提交 `717409ce61524bb3d8426060a384822f21354c62` | BSD-3-Clause（文件头单独声明，Copyright (c) 2005,2006 Choe Hwanjin；全文在 `resources/licenses/libhangul-hanja-BSD-3-Clause.txt`）。libhangul 仓库整体是 LGPL-2.1，但本仓只取这一份单独声明 BSD-3 的数据文件。上游提交历史说明 2006–2007 年部分读音和词表整理自国立国语院的材料，这些材料能否以 BSD 再授权无法独立核实，属于上游作者的声明 | `crates/engine/src/korean/hanja.tsv`，韩语模式把正在组字的音节转换成汉字时的候选表（555 个音节、28416 行，约 430 KB，`include_str!` 编进引擎）。`msime-dict-build hanja --dictionary <msime-dictionary checkout>` 读 msime-dictionary 原样收录的这一个文件（`sources/korean/hanja.txt`，大小与 SHA-256 由该仓 `upstream.lock.json` 固定）生成这张表，libhangul 提交记在 `resources/dictionary-sources.lock.json` 的 `libhangul` 引用里。只取单音节条目：键是一个预组合音节，值是基本多文种平面内的一个统一汉字，即 CJK 统一汉字、扩展 A 区汉字，或兼容汉字区里 NFC 不改写的 12 个统一汉字（U+FA0E 﨎、U+FA11 﨑 等）；真正的兼容汉字（NFC 会改写，固定版本的单音节条目里没有）、辅助平面汉字和两字值都丢弃；每个音节内保持上游顺序，保留上游的训音（훈음）注释，约 27% 的行有训音，扩展 A 区一行都没有。词条级数据（约 5.7 MB）不使用。BSD-3 第 2 条要求二进制分发时附带版权声明和条款，表编进引擎后随六个平台的引擎一起分发，所以六个平台的通知渠道都收录这份文本：Windows 的 `Collect-Notices.ps1`，macOS 输入法包的 `Resources/Licenses`（`CMakeLists.txt` 与 `THIRD_PARTY_NOTICES.txt`），Linux 安装的声明（`CMakeLists.txt` 与 `data/THIRD_PARTY_NOTICES.txt`），Android 的 `assets/native-notices`（`build-native.sh`），iOS App 的资源（`project.yml`），HarmonyOS 的 `resfile/licenses`（`stage-resources.sh`）。`scripts/test-korean-hanja-table.py` 检查这几处都还在 |
| Unicode 字符数据库（UCD）17.0.0 的 `UnicodeData.txt`、`Blocks.txt`、`EquivalentUnifiedIdeograph.txt` | Unicode-3.0（Unicode, Inc.；全文在 `resources/licenses/Unicode-License-V3.txt`，与下一行拆字表的 Unihan 用同一份） | `crates/engine/src/local/symbol_groups.tsv`，符号模式（S 模式，Shift+S）的分组码表（87 组、4 585 个符号，`include_str!` 编进引擎）。中文组名和每组的取字规则由本项目编写，写在 `scripts/build_symbol_groups.py` 里；组员只按 UCD 的区块、通用类别、字符名和分解映射挑选（非夹注符号一组另按 GB/T 15834 加上 —— 与 ……），组内按码位排序，不沿用任何输入法符号表的取字或顺序。分组码（拼音首字母缩写）和有哪些组是有意从旧表保留的，好让符号模式的用法不变：87 个编码里 54 个与 rime-prelude `symbols.yaml` 的编码相同，其余 33 个是 rime-wanxiang 加的；这些短编码是否需要另行授权未经法务审阅，保留与否待维护者确认。生成器按 `resources/symbol-groups.lock.json` 从 unicode.org 下载并校验三份文件，`scripts/test-symbol-groups.py` 离线检查表的不变量与声明渠道，CI 的 Rust engine 作业设 `MSIME_FETCH=1` 下载锁定输入后逐字节核对。此前这张表取自 rime-wanxiang 的 `wanxiang_symbols.yaml`（CC BY 4.0），其分组上游抄自 rime-prelude 的 `symbols.yaml`（LGPL-3.0），为了引擎能以 MPL-2.0 发布而改为由 Unicode 事实重建，见 `.agents/notes/implemented/feature/2026-10-29-symbol-groups-from-unicode.md` |
| [kfcd/chaizi](https://github.com/kfcd/chaizi)（開放詞典《漢語拆字字典》）的 `chaizi-jt.txt`、`chaizi-ft.txt`，提交 `e177ab54ce4edf255315a60593ffecbd8167e96b` | CC BY 3.0 Unported（© 2015 開放詞典，http://www.kaifangcidian.com）；署名、改动说明和许可证全文在 `resources/licenses/kfcd-chaizi-CC-BY-3.0.txt`。表里的读音、名次和繁体字形取自 Unihan 17.0.0，Unicode-3.0（全文在 `resources/licenses/Unicode-License-V3.txt`） | `crates/engine/src/local/chaizi.tsv` 与 `chaizi-chars.tsv`，拆字反查（C 模式）的部件拼音索引（约 1.8 万个键、20 387 字，两表合计约 0.6 MB，`include_str!` 编进引擎，网页引擎里是空表）。`scripts/build_chaizi_table.py` 按 `resources/chaizi.lock.json` 校验后读这两个文件：简繁两表取并集，去掉「□」占位行、私用区和兼容表意字的条目（兼容字归一后已在表里的并入）及含私用区部件的拆法，按部件的去调拼音重新索引（读音取 Unihan 17.0.0 的 `kMandarin` 与 `kTGHZ2013`，加上生成器里 `ALIASES` 的部首俗称，如 氵 san、辶 zou），给每个字附 `kMandarin` 首个读音、按《通用规范汉字表》排的名次和 OpenCC s2t 的繁体字形；没有读音的部件所在的拆法不进索引。`scripts/test-chaizi-table.py` 离线检查表的不变量与署名，CI 的 Rust engine 作业设 `MSIME_FETCH=1` 下载锁定输入后逐字节核对。CC BY 3.0 第 4(b) 条要求分发改编作品时署名并写明改动，许可证文件就是这份说明；Unicode License V3 要求随数据或相关文档附上它的版权与许可声明。随引擎分发的每个平台都要在声明渠道里带上这两份文件；`test-chaizi-table.py` 的 `NOTICE_CHANNELS` 列出已经接上的渠道。CC BY 3.0 与产品的 GPL-3.0、引擎的 MPL-2.0 的关系、App Store 等带 DRM 的渠道与第 4(a) 条技术措施条款的关系，以及上游没有说明这份数据自身的来源，三点都未经法务审阅，需要维护者判断，见 `.agents/notes/implemented/feature/2026-10-13-builtin-chaizi-mode.md`。设置里的「拆字」开关只停用功能，数据仍随二进制；真正退出要删掉两张表，并让 `local::chaizi` 的查询返回空 |
| LaTeX 命令名到 Unicode 码位的对照（本项目自写） | MIT（全文在 `resources/licenses/latex-symbols-MIT.txt`，正文与 `plugins/phrase_table/LICENSE` 相同） | `crates/engine/src/local/latex_symbols.tsv`，符号模式按它查 LaTeX 命令名（`include_str!` 编进引擎）。与下文官方短语表的 `plugins/phrase_table/latex-symbols/symbols.tsv` 逐字节相同，两份都由 `scripts/sync_symbol_tables.py` 按 `resources/symbol-tables.lock.json` 从 msime-dictionary 的 `sources/symbols/latex-symbols.tsv` 同步；引擎副本让 crate 不必从目录外 `include_str!`。是本项目自己的数据；不带官方短语表包的构建照样编进了这张表，所以 MIT 全文随引擎走各平台的声明渠道（`latex-symbols-MIT.txt`；Windows 与网页引擎的 NOTICE 收的是 `plugins/phrase_table/LICENSE`，正文相同），随引擎 crate 分发时也要带上 |
| 符号名称到符号（本项目自写） | MPL-2.0，与引擎代码相同 | `crates/engine/src/local/symbol_candidates.tsv`，打拼音出符号的名称表（`include_str!` 编进引擎）。名称与码位是事实，没有整段复制现成的表；唯一的编辑入口是 msime-dictionary 的 `sources/symbols/symbol-names.tsv`，按 `resources/symbol-tables.lock.json` 同步 |
| 不拿来拼整句的粗俗词表（本项目自写） | MPL-2.0，与引擎代码相同 | `crates/engine/src/lattice/offensive_words.txt`（`include_str!` 编进引擎），词网格、纠错整句和联想不用含表里的词的词（豁免写法放行的除外），收词标准写在文件头。词和豁免写法都取自 dict-v2.0.39 的 `msime-pinyin.db` 里本来就有的词，按本项目自定的标准挑出，没有复制现成的敏感词表；长期应由 msime-dictionary 维护 |
| 九键切分的音节频度（424 个音节各自的总权重） | 聚合统计事实，不另附许可证；来源词库是 GPL-3.0 | `crates/engine/src/nine_key/syllable_frequency.tsv`（`include_str!` 编进引擎），由 `msime-dict-build syllable-frequency` 从 msime-dictionary `dict-v2.0.41` 的 `msime-pinyin.db` 汇总，第一行记着那份词库的 SHA-256。词库的词条来自 rime-ice（GPL-3.0）及补充表，其中与万象共有的词条权重改编自 rime-wanxiang（CC BY 4.0）；这张表只有每个音节在全部词条里的权重之和，不含任何词、读音或单条权重，只作九键切分的排序先验。维护者 2026-10-29 裁定按聚合事实保留并写明来源；当天试过用 Unihan 17.0.0 `kHanyuPinlu`（Unicode-3.0）原始计数加缺失音节下限重建这张表：它缺 en、nuo、seng、nang 等 32 个音节，1980 年代的小语料把 na、ma 排得几乎一样，中性九键集 top-1 净少 11 到 20 例，评测退步，所以没有替换，现行的表保留 |
| [OpenCC](https://github.com/BYVoid/OpenCC) 词典，提交 `26753884f1984add422f3b0249ccee8613deaff6` | Apache-2.0 | `msime-dictionary/sources/opencc/`，构建输入由 `resources/opencc.lock.json` 固定；许可证全文保留在 `resources/licenses/OpenCC-Apache-2.0.txt`。`STPhrases.txt`、`STCharacters.txt`、`CJK_Compatibility_Ideographs.txt`，以及臺灣变体用的 `TWPhrases.txt`、`TWVariants.txt`、`TWVariantsPhrases.txt` 原样取自该提交的 `data/dictionary/`；`STPhrases_GeneratedFromRegionalPhrases.txt` 是该提交的 OpenCC 构建产物（`data/scripts/generate_st_phrases_from_regional_phrases.py` 用 `t2s.json` 生成），本仓不重新生成。提交号与来源 MSIME-Windows 的 `vendor/opencc` 子模块一致。只使用数据，不链接 OpenCC 的 C++ 库；`chinese_conversion.rs` 按 `s2t.json`、`s2tw.json`、`s2twp.json` 的规则实现转换。Windows 通知由 `Collect-Notices.ps1` 一并收集 |

## 脚本插件合集（`plugins/script/`）

官方包（`plugins/official-packs.json` 列出的，现在是 uuid）随桌面安装包分发，其余插件由用户从源码构建后自己导入；插件代码用 MIT。uuid 不带第三方数据。带第三方数据的插件把数据变换成紧凑的二进制表，用 `include_bytes!` 编进 wasm 模块，生成器和产物都提交进仓库，`scripts/test-script-plugin-data.py` 检查两者一致（ci-platforms.yml 的 Script plugins 作业设 `MSIME_FETCH=1`，下载锁定的 Unihan zip 后也核对常用字集；不联网的 contracts 工作流只核对简繁对照表）。插件包里随附许可证原文和一份 `NOTICE.txt`，写明来源、版本和生成器。

| 组件 | 许可证 | 位置与说明 |
| --- | --- | --- |
| Unicode Han Database（Unihan）17.0.0 的 `kTGH`（《通用规范汉字表》）与 `kTraditionalVariant` | Unicode-3.0（全文在 `resources/licenses/Unicode-License-V3.txt`） | `plugins/script/data/common-chars.bin`，生僻字类插件共用的常用字集（8105 字并上它们的繁体形，共 11 049 字，约 4 KiB）。`plugins/script/tools/gen_common_chars.py` 读 `resources/character-glosses.lock.json` 锁定的 `Unihan-17.0.0.zip`（校验 sha256）生成，来源说明在同目录的 `common-chars-NOTICE.txt`。插件包随附 `Unicode-License-V3.txt` |
| [OpenCC](https://github.com/BYVoid/OpenCC) 词典，提交 `26753884f1984add422f3b0249ccee8613deaff6` | Apache-2.0（全文在 `resources/licenses/OpenCC-Apache-2.0.txt`） | 读的是上面「编译进共享库的数据」里由锁固定的同一份文件。常用字集取 `STCharacters.txt` 里《通用规范汉字表》各字的繁体单字值（只由 OpenCC 补进集合的有 151 字）。插件包随附 `OpenCC-LICENSE.txt`，组装时（`plugins/script/tools/assemble.py`）逐字节核对它和仓库里的原件相同 |

## 官方短语表（`plugins/phrase_table/`）

`plugins/phrase_table/latex-symbols` 随桌面安装包分发（`plugins/official-packs.json`），数据用 MIT，全文在 `plugins/phrase_table/LICENSE`，包里随附同样的 `LICENSE.txt`。

| 组件 | 许可证 | 位置与说明 |
| --- | --- | --- |
| LaTeX 命令名到 Unicode 码位的对照（369 条） | MIT（本项目） | `plugins/phrase_table/latex-symbols/symbols.tsv` 由本项目编写，命令名取自 LaTeX 内核、amssymb、amsmath、latexsym、stmaryrd、mathtools、gensymb 和 textcomp 的公开文档，名字与码位的对应是事实，不照录任何受版权保护的表：不用 unicode-math 的 `unicode-math-table.tex`（LPPL）、fcitx5 的 `latex.mb`（LGPL）或 Julia 的 `latex_symbols.jl`（许可未核实）。`plugins/phrase_table/tools/gen_latex_symbols.py` 生成包里的 `plugin.toml` |

## 本地语音识别

语音服务选 `local` 且 `asr_model_path` 指向已安装的模型目录时，识别在设备上完成。这条路径分两部分：一份随包分发的推理运行时，和由用户在设置页按需下载的模型。**模型不随包分发**，仓库只携带下载地址、长度和 SHA-256。

### 运行时（`resources/voice-runtime.lock.json`）

锁文件为每个平台固定一个 sherpa-onnx v1.13.8 的上游预编译产物（来源提交 `11afbd009a7f8c08f4bcf2fc1b265d0df4670fbf`）的 URL、长度和 SHA-256，`scripts/fetch_voice_runtime.py --platform <平台>` 校验后展开到被忽略的 `target/voice-runtime/<平台>/`，各平台的构建与打包脚本从那里取文件。

macOS 发布包可以不内置运行库，改为资源包 `voice-runtime-macos` 按需下载：上游的 universal dylib 剥掉局部符号后用本项目的 Developer ID 重签（输入法开着 hardened runtime，只加载同一 Team ID 签名的库），作为本仓库 `voice-runtime-macos-v<版本>` Release 的资产发布，`resources/voice-runtime-macos.lock.json` 固定归档和其中 dylib 的 SHA-256。重签不改动库的代码，Apache-2.0 与 ONNX Runtime 的许可证照旧随输入法 bundle 分发。

| 组件 | 许可证 | 说明 |
| --- | --- | --- |
| [k2-fsa/sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) | Apache-2.0 | 语音识别运行时。宿主不在构建期链接它，而是在第一次识别时用 `dlopen`/`LoadLibrary` 加载其 C API 库（`shared/voice/LocalAsr.h`）。构建期只需要它的 C 头文件，原样放在 `shared/voice/third_party/sherpa-onnx/c-api.h`，许可证全文在同目录 `LICENSE`。Android 使用上游的 `.aar`，HarmonyOS 使用上游的 `.har`，iOS 使用 `SherpaOnnxC.xcframework` |
| [microsoft/onnxruntime](https://github.com/microsoft/onnxruntime) | MIT | sherpa-onnx 的推理后端，随上述产物一起来，版本由 sherpa-onnx 的发布决定。macOS 与 iOS 的产物把它静态链接进 sherpa-onnx 库；Linux 另带 `libonnxruntime.so`，Windows 另带 `onnxruntime.dll` 与 `onnxruntime_providers_shared.dll`。分发时需要携带它的许可证与第三方通知 |

### 模型（`resources/local-asr-models.json`）

模型目录由设置页的模型管理下载：`client-providers::voice::local_models` 只接受 HTTPS，逐字节校验长度与 SHA-256，只保留目录清单 `files` 里列出的文件（`test_wavs`、`test_onnx.py`、`bpe.model` 不解包），最后写入 `msime-model.json`。四份归档都是 sherpa-onnx 项目转换成 ONNX 后在 `asr-models` 发布里公开提供的版本，下表的许可证取自目录里各条目的 `license` 字段：

| 目录 id | 模型 | 许可证 | 来源 | 说明 |
| --- | --- | --- | --- | --- |
| `x-asr-zh-en-streaming`（默认） | X-ASR 中英流式 zipformer transducer，int8 | Apache-2.0 | [Gilgamesh-J/X-ASR](https://github.com/Gilgamesh-J/X-ASR) | 原生支持热词。热词需要的 `bpe.vocab` 上游归档里没有，仓库自带一份 `resources/voice-models/x-asr-zh-en-bpe.vocab`（69,594 字节，SHA-256 记在目录的 `extra` 里），编译进 `client-providers`，安装时写入模型目录；它是该模型的派生数据，适用同一许可证 |
| `x-asr-zh-en-offline` | X-ASR 中英离线（非流式）zipformer transducer，int8 | Apache-2.0 | [Gilgamesh-J/X-ASR](https://github.com/Gilgamesh-J/X-ASR) | 与流式版同一模型的整句导出，`tokens.txt` 与流式版逐字节相同，因此共用同一份 `bpe.vocab` 做原生热词；另带 `silero_vad.onnx` 分段 |
| `sense-voice-small` | SenseVoice-Small，int8 | **FunASR Model License v1.1**（`LicenseRef-FunASR-Model-License-1.1`），不是 OSI 开源许可证 | [FunAudioLLM/SenseVoice](https://github.com/FunAudioLLM/SenseVoice)，条款全文见 [FunASR MODEL_LICENSE](https://github.com/modelscope/FunASR/blob/main/MODEL_LICENSE) | 阿里巴巴通义实验室。条款要求保留模型名称和署名，目录条目的 `notice` 原样记录了这段署名，设置页在模型旁显示它。**不随 MSIME 分发**，只在用户点下载时从上游取得 |
| `fun-asr-nano`（仅桌面） | Fun-ASR-Nano-2512，int8 | Apache-2.0 | [FunAudioLLM/Fun-ASR](https://github.com/FunAudioLLM/Fun-ASR) | 阿里巴巴通义实验室。解码器是 Qwen3-0.6B，同为 Apache-2.0 |
| `silero_vad.onnx`（离线 X-ASR、SenseVoice 与 Fun-ASR-Nano 的附加文件） | Silero VAD | MIT | [snakers4/silero-vad](https://github.com/snakers4/silero-vad) | 整句模型用它切分语音段。从 sherpa-onnx 的 `asr-models` 发布下载，SHA-256 固定在目录的 `extra` 里 |

用户可以在设置里配置下载镜像（`asr_model_mirror`）；镜像只改变从哪里取文件，校验用的 SHA-256 不变，所以镜像无法替换内容。

### 编译进共享层的 Rust 依赖

| crate | 版本 | 许可证 | 用途 |
| --- | --- | --- | --- |
| `pinyin` | 0.11 | MIT | 把用户词库的热词转成无声调拼音，给不支持原生热词的模型做近音纠正（`client-providers::voice::hotwords`） |
| `tar` | 0.4 | MIT OR Apache-2.0 | 解包模型归档 |
| `bzip2` | 0.6 | MIT OR Apache-2.0 | 解压 `.tar.bz2`。0.6 默认使用纯 Rust 的 `libbz2-rs-sys` 后端，该 crate 的许可证是 `bzip2-1.0.6`（bzip2 原作的 BSD 式许可，要求保留版权声明） |

## 云剪贴板参考中继（`crates/clipboard-relay`）

用户自己部署的服务端程序 `msime-clipboard-relay`，不进任何平台的安装包。它用的数据库、序列化和摘要 crate 与 workspace 其余部分共用同一版本，只为它新引入了 HTTP 服务器：

| crate | 版本 | 许可证 | 用途 |
| --- | --- | --- | --- |
| `axum`（关闭默认特性，只开 `tokio`、`http1`）及其 `axum-core` | 0.8 / 0.5 | MIT | 中继的 HTTP/1 服务器；接口逻辑在不碰网络的 `store` 里 |
| `matchit`（经 `axum`） | 0.8 | MIT AND BSD-3-Clause | axum 的路由表；中继只用一个兜底处理函数，不注册路由 |
| `httpdate`（经 `axum`） | 1.0 | MIT OR Apache-2.0 | 响应的 `Date` 头 |

## Android 可选离线 OCR 模块（`resources/ocr-models-android.lock.json`）

键盘内拍照识字用的可选模块 `msime-ocr-v1.apk`（`platforms/android/gradle-app/ocr-module`）不进入基础 APK，由用户从本仓库 `ocr-module-android-v1` Release 手动下载后导入；宿主只接受 `resources/ocr-module-android.lock.json` 固定的整个 APK 摘要，锁文件的 `artifact` 为 `null` 时任何模块都导入不了。模块由 `publish-android-ocr-module.yml` 手动构建发布；现在锁定的是从提交 `8e4c92657de3b2163297c122668059fbb2c746a9` 构建的那一份（81,146,926 字节，SHA-256 `1e08af23…b8eef8cf`），标签下的资产发布后不再替换。

`resources/ocr-models-android.lock.json` 只供构建使用：固定 ONNX Runtime 的 Maven 坐标与来源提交、两个模型仓库的 Hugging Face 提交和逐文件 SHA-256、从识别模型 `inference.yml` 抽出的 CTC 字典的条目数与 SHA-256、检测后处理参数，以及三份许可证全文的上游地址和 SHA-256。`scripts/fetch_ocr_models.py` 在模块构建时下载校验，`scripts/test-fetch-ocr-models.py` 离线核对许可证副本、Java 常量和 Gradle 依赖与锁一致。

| 组件 | 许可证 | 说明 |
| --- | --- | --- |
| [microsoft/onnxruntime](https://github.com/microsoft/onnxruntime) 1.31.0（`com.microsoft.onnxruntime:onnxruntime-android`，来源提交 `402979ac1bef89dcd55add41eaa7277c407599eb`） | MIT | 推理引擎，只进模块。模块带 `arm64-v8a` 与 `x86_64` 两套原生库，STORED 并按 16 KB 对齐，宿主从只读 APK 直接映射。AAR 自带向 Microsoft 上报的 1DS 遥测：模块清单删除它的 `TelemetryInitializer` 和网络权限，创建运行环境时 `setTelemetry(false)`。许可证全文 `resources/licenses/onnxruntime-MIT.txt`，第三方通知 `resources/licenses/onnxruntime-ThirdPartyNotices.txt` |
| [PaddlePaddle/PP-OCRv6_tiny_det_onnx](https://huggingface.co/PaddlePaddle/PP-OCRv6_tiny_det_onnx)（提交 `2ba1506c0380b8f0b03dd142459aac66d4421f6c`） | Apache-2.0 | 文字检测模型，原样打包为模块的 `assets/ocr/det.onnx` |
| [PaddlePaddle/PP-OCRv6_tiny_rec_onnx](https://huggingface.co/PaddlePaddle/PP-OCRv6_tiny_rec_onnx)（提交 `2612ab37152ae0a677521bae4e1e3d4fb4cf7c30`） | Apache-2.0 | 文字识别模型 `assets/ocr/rec.onnx`；CTC 字典 `assets/ocr/dict.txt` 逐条取自同一提交的 `inference.yml`。许可证全文取自 [PaddleOCR](https://github.com/PaddlePaddle/PaddleOCR) 提交 `dab3fe35379033fdcb2d0e9572fac0b36c9a9ebf` 的 `LICENSE`，在 `resources/licenses/PaddleOCR-Apache-2.0.txt`（PaddleOCR 没有 NOTICE 文件） |

## 各平台引入的第三方 SDK

| 平台 | 组件 | 许可 |
| --- | --- | --- |
| Android | `com.google.mlkit:digital-ink-recognition:19.0.0`，连带 Google datatransport（`transport-runtime` 2.2.6、`transport-backend-cct` 2.3.3）。宿主清单删掉 datatransport 的 `TransportBackendDiscovery`、`JobInfoSchedulerService` 和 `AlarmManagerSchedulerBroadcastReceiver`，ML Kit 的 `FIREBASE_ML_SDK` 诊断不再存储和上传到 `firebaselogging.googleapis.com`，见 [PRIVACY.md](../PRIVACY.md) | **Google 的 ML Kit 服务条款，不是开源许可证** |
| Android OCR 可选模块（发布在 `ocr-module-android-v1`，不进入基础包） | `com.microsoft.onnxruntime:onnxruntime-android:1.31.0`，加 PaddlePaddle PP-OCRv6 tiny 检测与识别模型，见[Android 可选离线 OCR 模块](#android-可选离线-ocr-模块resourcesocr-models-androidlockjson) | ONNX Runtime 为 MIT，模型为 Apache-2.0 |
| Android | AndroidX、`com.google.android.material` | Apache-2.0 |
| Android | `io.noties.markwon:core:4.6.2`（“设置”页公告正文的 Markdown）及其依赖 `com.atlassian.commonmark:commonmark:0.13.0` | Markwon 为 Apache-2.0，commonmark-java 为 BSD-2-Clause |
| Android | vcpkg 提供的 nlohmann/json（原生库，清单在 `platforms/android/vcpkg.json`） | MIT |
| iOS | `MLKitDigitalInkRecognition` 8.0.0（CocoaPods，链接进键盘扩展 target；经 `MLKitCommon` 14.0.0 带进 `GoogleDataTransport` 10.1.0 与 nanopb。上传所需的 Clearcut 传输没有链接，2026-10-25 在 iPhone XR 上抓包没有发出任何统计；键盘在调用 ML Kit 前另把 `com.google.firebase.ml.<模块>.__MLKIT_DEFAULT.isStatsCollectionEnabled` 写成 `false`，ML Kit 不再记录使用统计，见 [PRIVACY.md](../PRIVACY.md)） | **Google 的 ML Kit 服务条款，不是开源许可证** |
| iOS | `GoogleSignIn` 10.0.0（CocoaPods，只链接进主 App target，用于通过 Google 登录）及其依赖 AppAuth、GTMAppAuth、GTMSessionFetcher、AppCheckCore、GoogleUtilities、Promises、RecaptchaInterop | Apache-2.0，全文见 `platforms/ios/SharedResources/GoogleSignIn-Dependencies.txt` |
| macOS | Sparkle 2.9.6 | 以上游发布附带的许可证为准；框架不随仓库分发，由构建者按 `platforms/macos/README.md` 记录的 SHA-256 自行取得 |
| macOS、Windows | [tplgy/cppcodec](https://github.com/tplgy/cppcodec) v0.2（标签提交 `302dc28f8fd5c8bf2ea8d7212aed3be884d5d166`，归档 SHA-256 `0edaea2a9d9709d456aa99a1c3e17812ed130f9ef2b5c2d152c230a5cbc5c482`），只有头文件 | MIT。`shared/voice` 的阿里云百炼请求把录音编成 Base64 数据 URL 时用它。只原样取 `base64_rfc4648.hpp` 需要的 8 个头文件，放在 `shared/voice/third_party/cppcodec/`，许可证全文在同目录 `LICENSE`；macOS 输入法包的 `Resources/Licenses`（`CMakeLists.txt` 与 `THIRD_PARTY_NOTICES.txt`）和 Windows 的 `Collect-Notices.ps1` 收录这份文本。Linux 不编译 `msime-voice-providers`，不带它 |
| Windows | vcpkg 提供的 libcurl、fmt、nlohmann/json、utfcpp（清单与 baseline 在 `platforms/windows/vcpkg.json`） | 各自上游许可证；通知由 `platforms/windows/Collect-Notices.ps1` 收集 |
| Linux | IBus / Fcitx5、GTK 栈、libcurl、ICU、xkbcommon、nlohmann/json、X11 与 Wayland 客户端库 | 各自上游许可证，按发行版依赖引入 |
| HarmonyOS | 华为标志 `packages/ui/src/assets/huawei-id/logo-red.svg`、`logo-white.svg`，原样取自 HarmonyOS SDK 6.1.1 的 HMS 资源 `hms/previewer/resources/resources/resources/base/media/huawei_id_logo_red.svg`、`huawei_id_logo_white.svg`（系统 `LoginWithHuaweiIDButton` 画的就是这两张）。只用在共享账号页的「华为账号登录」按钮上，按华为账号登录按钮规范放在文字左边 | **华为的商标，不是开源许可证**，按华为账号登录按钮规范使用 |
| 桌面 | Tauri、React、Vite 等 | 见 `pnpm-lock.yaml` 与各自上游 |

两个移动平台的 ML Kit 是识别手写笔迹用的。桌面与 Linux 不使用它，改用 `crates/engine` 里移植的离线 Zinnia 识别器和上面固定的模型；Android 的原生构建明确把 Zinnia 及其模型路径排除在外（`platforms/android/verify-native.sh`）。

仓库不捆绑任何字体文件；界面使用系统字体，`Noto Sans SC` 与 `Microsoft YaHei` 只是回退字体名。

## 语言生态依赖

逐个列出会立刻过时，以锁文件为准：

- Rust：`Cargo.lock`，当前 578 个 package 条目（含本 workspace 自身的成员）。`cargo audit` 是 `scripts/verify-local.sh` 完整版的一个阶段，也是 `cargo-audit.yml` workflow 的一步，漏洞视为失败；被接受的 `unmaintained` / `unsound` 公告逐条记在 [`.cargo/audit.toml`](../.cargo/audit.toml) 里，每条都写明引入链和接受理由。
- Node：`pnpm-lock.yaml`。
- iOS：`platforms/ios/Podfile.lock`。

## 通知文件在哪

| 位置 | 覆盖范围 |
| --- | --- |
| `platforms/macos/resources/Licenses/THIRD_PARTY_NOTICES.txt` | macOS 客户端内嵌组件的完整通知 |
| `platforms/linux/data/THIRD_PARTY_NOTICES.txt` | Linux 安装到 `${CMAKE_INSTALL_DATADIR}/doc/msime-client/` 的组件总览，许可证全文由 `platforms/linux/CMakeLists.txt` 的 `MSIME_NOTICE_SOURCES` 一并安装 |
| `platforms/ios/SharedResources/MLKit-NOTICES.txt`、`MLKit-Dependencies.txt`、`GoogleSignIn-Dependencies.txt` | iOS 的 ML Kit 与 Google 登录依赖通知；编进引擎的数据与代码的声明（韩语汉字表的 `libhangul-hanja-BSD-3-Clause.txt`、引擎与 rink-core 的 `MPL-2.0.txt`、`cxx_sort` 的 `libcxx-Apache-2.0-WITH-LLVM-exception.txt`、符号模式 LaTeX 表的 `latex-symbols-MIT.txt` 等）由 `platforms/ios/project.yml` 作为 App 资源打包，关于页「开源许可」逐个列出；本应用的 GPL-3.0 全文是原样打包的根目录 `LICENSE`，列在第一项 |
| Android OCR 模块 `msime-ocr-v1.apk` 的 `assets/MSIME-OCR-NOTICE.txt` 与 `assets/licenses/` | 模块的来源、模型摘要、隐私说明，以及 ONNX Runtime 的 MIT 与第三方通知、PaddleOCR 的 Apache-2.0 全文（与 `resources/licenses/` 逐字节相同） |
| Android APK 的 `assets/native-notices/` | `platforms/android/build-native.sh` 收集的原生依赖声明，含编进引擎的韩语汉字表的 libhangul BSD-3-Clause 声明、引擎与 rink-core 的 MPL-2.0 全文和 `cxx_sort` 的 libc++ 许可证；两条打包路径都放进 APK，同时把根目录 `LICENSE` 复制成 `assets/client-LICENSE.txt`（本应用的 GPL-3.0 全文），关于页「开源许可」列出这些文件 |
| HarmonyOS HAP 的 `resfile/licenses/` | `platforms/harmony/stage-resources.sh` 暂存的编进引擎的数据与代码的声明，含韩语汉字表的 libhangul BSD-3-Clause 声明、引擎与 rink-core 的 MPL-2.0 全文和 `cxx_sort` 的 libc++ 许可证，以及根目录 `LICENSE` 复制成的 `GPL-3.0.txt`（本应用的 GPL-3.0 全文）。设置页不逐个列出这个目录，「开源许可」打开 GitHub 上的 `LICENSE` |
| `apps/desktop/src-tauri/gen/android/gradle/LICENSE-2.0.txt`、同目录 `NOTICE.md` | Android Gradle 模板的 Apache-2.0 文本与来源说明 |
| `platforms/windows/Notices.md` | Windows 通知生成器的用法与限制；产物由 `Collect-Notices.ps1` 生成 |
| `msime-engine-capi-vX.Y.Z` release 的库归档与数据包 | 引擎 C 库的预编译归档带 MPL-2.0 的 `LICENSE`、`licenses/`（引擎的 `LICENSE`、`crates/engine/licenses/` 里编进引擎的数据与改编代码的许可证全文、`ewts-MIT.txt`）、按 `msime-engine-capi` 依赖图生成的 `THIRD_PARTY_NOTICES.txt` 和写明对应源码与 rink-core 源码位置的 `SOURCE.txt`；全拼、双拼数据包 `msime-engine-data-quanpin-<词库版本>` 带说明各文件数据许可证的 `README.md`、`msime-scowl_Copyright.txt` 和 `licenses/`（GPL-3.0 全文、行政区划名称的 WTFPL、`pinyin-data-MIT.txt`、`ECDICT-MIT.txt`、`rime-wanxiang-CC-BY-4.0.txt`、`msime-engine-dictionary-NOTICE.md`）。两者由 `crates/engine-capi/packaging/` 的 `package.sh`、`data-pack.py` 组装，`.github/workflows/release-engine-capi.yml` 发布 |
| 网页引擎 `web-engine-vX.Y.Z` release 的 `NOTICE.md` | 网页内置输入法用的 wasm 引擎、裁剪后的拼音库与五笔 86 库和整句模型的来源与许可证（release 由 `.github/workflows/release-web-engine.yml` 构建）。来源是 `resources/licenses/web-engine-NOTICE.md`：词库、模型、编进 wasm 的数据等内容手写；仓库许可证全文与链接进 wasm 的 crate 两节由 `scripts/web-engine-notice.sh` 按 `cargo tree -p msime-engine-wasm --target wasm32-unknown-unknown -e normal,no-proc-macro` 生成，`--check` 与 `crates/engine-wasm/tests/notice.rs` 在依赖变化而 NOTICE 没有重新生成时失败。`scripts/build-web-engine.sh` 把它复制成 `NOTICE.md`，并把源码链接里的占位符换成构建所用的提交 |

这些生成器和收集器都在各自文档里写明「不是完整性或再分发授权的评估」。发布二进制前的逐平台要求见[开源发布清单](open-source-release.md)。

## 新增依赖时

引入新的上游代码、字体、图标、模型或服务 SDK 时，同时提交来源提交、许可证文本、通知位置和分发限制，并在本文件登记——不要只在 README 留一个链接。资源锁文件只校验内容，不授予任何分发权利。

## Windows 皮肤图片动画

`platforms/windows/msimeui` 的 `AnimatedImage` 与部分图片测试来自 msime-windows 固定提交 `dd018f79858314ed542a47c528608752b13bd261`，按 GPL-3.0 分发；来源路径、本地调整和完整许可见 [UPSTREAM.md](../platforms/windows/msimeui/UPSTREAM.md) 与 [LICENSE](../platforms/windows/msimeui/LICENSE)。仍使用 Windows 系统 WIC 解码，不打包新的第三方编解码库。

## Linux 原生画布翻页

`platforms/linux/src/candidates/NativeCanvas.h` 的翻页尺寸、布局规则与圆角三角路径参考 msime-windows 固定提交 `dd018f79858314ed542a47c528608752b13bd261` 的 `server/src/window/candidate_skin_scene.cpp` 和 `ui/src/PagerArrows.cpp`，以 Cairo 路径和 Pango 字号重写；按 GPL-3.0 分发，完整许可见仓库根目录的 [LICENSE](../LICENSE)。本段与来源提交信息随源代码保留，二进制分发沿用本项目 GPL 许可和对应源代码提供要求。

同一文件的横排卷轴、等宽跨格、2–8 附加行和滚动条语义参考固定提交 `c0a9d371f00b7b309c07e78bc0fd27d3a0d4f5b1` 的 `server/src/window/candidate_presenter.cpp`、`candidate_skin_scene.cpp` 与 `ui/src/CandidateList.cpp`，按 Linux 的 Pango 测量和 X11 事件重新实现，沿用上述 GPL-3.0 许可和源码提供要求。

## Windows 皮肤出厂布局

macOS 的 `platforms/macos/src/candidate/CandidateShadow.h` 对照 msime-windows 固定提交 `a60e2948dbba29f32bec58ee4bf44d68ce953a0b` 的 `server/src/window/candidate_skin_scene.cpp` 与 `candidate_presenter.cpp`，使用 AppKit 重写双层阴影、档位透明度和透明边带。按 GPL-3.0 与本仓一同分发，完整许可见仓库根目录 [LICENSE](../LICENSE)，二进制分发沿用对应源代码提供要求。

`crates/client-services/src/skin/catalog/factory-layouts/` 的八份 TOML 原样取自 [msime-windows 固定提交 `a60e2948dbba29f32bec58ee4bf44d68ce953a0b` 的 `skins/default/`](https://github.com/metasequoiaime/msime-windows/tree/a60e2948dbba29f32bec58ee4bf44d68ce953a0b/skins/default)，上游各 `<base>/skin.toml` 在本仓以 `<base>.toml` 保存。共享层只读取候选窗几何与开关，不执行样式或脚本。按上游 GPL-3.0 与本仓一同分发，许可全文为仓库根目录的 [LICENSE](../LICENSE)；二进制仍通过各平台既有 GPL 通知和对应源码提供这些内容，无额外分发限制。

### 装饰图候选词叠加的清单契约

`crates/client-services/src/skin/catalog/decoration_candidate.rs` 的默认值、边界和启停语义参考 [msime-windows 固定提交 `0978b5c28b2a1806e7ef3acdbc3a1c87e4784823` 的 `ReadDecorationCandidate`](https://github.com/metasequoiaime/msime-windows/blob/0978b5c28b2a1806e7ef3acdbc3a1c87e4784823/server/src/skin/candidate_skin_catalog.cpp)，以 Rust 重写；`packages/web-engine/src/skin.js`、`packages/ui/src/skin/decoration-candidate.ts` 的装饰词颜色解析，以及网页候选栏、共享预览、`shared/contracts/candidate_decoration_overlay.h`、macOS `CandidateMotion.mm`、Windows `CandidateDecorationDrawing.h`、HarmonyOS `DecorationCandidatePolicy.ts`、iOS `CandidateCanvasAssets.swift`／`CandidateCanvasView.swift`、Android `CandidateCanvasAssets.java`／`CandidateCanvasRenderer.java` 与 Linux `NativeCanvas.h` 的字号缩放和对齐参考同提交的 `candidate_skin_palette.cpp`、`candidate_skin_scene.cpp`，分别以 JavaScript、React、C++、AppKit、DirectWrite、ArkTS、UIKit、Android Canvas 和 Cairo/Pango 重写。随本仓按 GPL-3.0 分发，许可全文见根目录 [LICENSE](../LICENSE)；二进制分发沿用既有 GPL 通知和对应源码提供要求，无新增图片或编解码依赖。

macOS 的 `platforms/macos/src/candidate/CandidatePager.h` 对照 msime-windows 固定提交 `a60e2948dbba29f32bec58ee4bf44d68ce953a0b` 的 `server/src/window/candidate_skin_scene.cpp` 与 `ui/src/PagerArrows.cpp`，以 AppKit 重写列表翻页尺寸、布局与圆角三角路径。按 GPL-3.0 与本仓一同分发，许可全文见仓库根目录 [LICENSE](../LICENSE)；二进制分发沿用对应源码提供要求。

macOS 的 `platforms/macos/src/candidate/CandidateExpandToggle.h` 对照 msime-windows 固定提交 `b0bcb6ff5bef797d72e6b38930be8a6b9dccfb2f` 的 `server/src/window/candidate_skin_scene.cpp`，以 AppKit 重写卷轴展开／收起按钮的字号尺寸、两段折线与留白。按 GPL-3.0 与本仓一同分发，许可全文见仓库根目录 [LICENSE](../LICENSE)；二进制分发沿用对应源码提供要求。

## Octagram 语法模型查询

`crates/engine/src/lattice/octagram.rs` 的查询与编码逻辑移植自 [lotem/librime-octagram](https://github.com/lotem/librime-octagram/tree/57d18b9f58e5284bd891d559f6bdd16cf60341e9) 的 `src/gram_db.cc`、`src/gram_encoding.cc`、`src/octagram.cc`，固定提交 `57d18b9f58e5284bd891d559f6bdd16cf60341e9`。Copyright (c) 2014, RIME Developers，BSD-3-Clause；完整声明保存在 `resources/licenses/librime-octagram-BSD-3-Clause.txt`，经六个平台现有声明渠道随引擎分发。同一文件的双数组 Trie 读取（`traverse` 与 `DoubleArrayUnit` 位段）按 [darts-clone](https://github.com/s-yata/darts-clone) 0.32 的数据布局重新实现，Copyright (c) 2008-2014, Susumu Yata，BSD-2-Clause；它的条款原文（取自提交 `fc45469d6a4469b2c9f337b2e153983888c7826a` 的 `COPYING.md`）附在同一份声明文件末尾，随它走同样的渠道。代码本身逐函数翻译自 msime-windows 的 `engine/ngram/octagram/octagram_gram.cpp`（GPL-3.0），已随引擎以 MPL-2.0 再许可。移植改用 Rust，并增加文件边界校验、共享映射缓存与词格打分接线。模型数据另行按需下载，其来源与许可由各资源包单独记录。

## libc++ 排序算法

`crates/engine/src/lattice/cxx_sort.rs` 逐步复现了 LLVM libc++（参考解码器构建时用的 AppleClang 21 所带版本）的 `__algorithm/partial_sort.h`、`sort.h`、`make_heap.h`、`sift_down.h`、`pop_heap.h`、`push_heap.h`、`sort_heap.h`：等分假设在词格 beam 里的去留和先后只由不稳定排序怎样移动相等元素决定，Rust 的 `sort_unstable` 与 `select_nth_unstable` 的移动方式不同，所以照搬。许可证是 Apache-2.0 WITH LLVM-exception；LLVM 例外只免除编译成目标码时第 4(a)(b)(d) 条的义务，源码级翻译仍按 Apache-2.0 第 4 条写明出处与改动并随附许可证。文件头写着出处和本项目的改动（迭代器改为 `T: Copy` 切片上的下标、比较器改为 `FnMut` 闭包、只保留用到的代码路径，`push_partial` 与 `finish_partial` 是本项目自己的代码）；许可证全文是 `resources/licenses/libcxx-Apache-2.0-WITH-LLVM-exception.txt`，即 [libc++ 的 `LICENSE.TXT`](https://github.com/llvm/llvm-project/blob/4a1b95bda0c444798a5240fe924dd127b776d12d/libcxx/LICENSE.TXT) 原文加一段来源说明。它和 librime-octagram 的声明走同样的渠道：Windows 的 `Collect-Notices.ps1`，macOS 输入法包的 `Resources/Licenses`（`CMakeLists.txt` 与 `THIRD_PARTY_NOTICES.txt`），Linux 安装的声明（`CMakeLists.txt` 与 `data/THIRD_PARTY_NOTICES.txt`），Android 的 `assets/native-notices`（`build-native.sh`），iOS App 的资源（`project.yml`），HarmonyOS 的 `resfile/licenses`（`stage-resources.sh`），以及网页引擎的 NOTICE（`scripts/web-engine-notice.sh`）；`scripts/test-engine-license.py` 检查这几处都还在。

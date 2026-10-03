# 第三方组件清单

这份清单回答「这个项目用了谁的代码和数据、各自什么许可」。它是仓库级的索引；各平台打包时实际生成的通知文件另有位置，见末尾的[通知文件在哪](#通知文件在哪)。

清单不替代许可证审计。它记录已知的来源与条款，并明确标出尚未记录的部分——后者同样重要，因为一份看起来完整、实则有空白的清单比承认空白更危险。

## 本项目

源码为 **GPL-3.0-only**，全文在根目录 [LICENSE](../LICENSE)。Rust workspace 的 `license` 字段、Linux 打包元数据和共享客户端都声明同一许可证。

## 输入引擎（`crates/engine`）

`crates/engine`（`msime-engine`）是本项目自己的代码，按 GPL-3.0 分发。它是 [`metasequoiaime/msime-engine`](https://github.com/metasequoiaime/msime-engine)（GPL-3.0）C++ 实现的 Rust 移植；移植时对照的参考提交记在 `tools/engine-golden/README.md`。构建时不再取回任何上游源码归档，原先随 Engine 归档进来的组件去向如下：

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
| `lru` | 0.16 | MIT | 查询结果缓存（`crates/engine/src/cache.rs`） |
| `lunar-lite` | 0.1 | MIT | 日期时间快捷模式里的农历（`crates/engine/src/local/date_time.rs`） |
| `wana_kana` | 4 | MIT | 日语罗马字转假名（`crates/engine/src/japanese/romaji.rs`） |
| `cpal` | 0.18 | Apache-2.0 | `host-api` 的麦克风采集（iOS 与 HarmonyOS 不链接）。Linux 上经 ALSA 的 `libasound` 访问声卡，构建需要 `libasound2-dev` 与 `pkg-config` |
| `rubato` | 5 | MIT OR Apache-2.0 | 把采集到的音频重采样到识别所需的采样率 |
| `exmex` | 0.21 | MIT OR Apache-2.0 | V模式的算式求值（`crates/engine/src/local/expression.rs`），只注册四则运算、`%` 取余和 `^` 乘方。它带进 `regex` 与 `smallvec`，两者原本就在锁文件里 |
| `chinese-number`（关闭默认特性，只开 `std`、`number-to-chinese`） | 0.8 | MIT | V模式把数字写成中文小写、大写与金额（同一文件）。传递依赖 `chinese-variant`（MIT）、`enum-ordinalize`（MIT）、`num-bigint`（MIT OR Apache-2.0） |
| `rink-core`（关闭默认特性，只开 `bundle-files`） | 0.9 | MPL-2.0；它内嵌的单位库 `definitions.units` 分叉自 GNU Units 的数据库，为 GPL-3.0-or-later（Free Software Foundation），与本仓库的 GPL-3.0 兼容 | V模式的单位换算（`crates/engine/src/local/units.rs`），例如 `3jin'g` 把 3 斤换成克。`bundle-files` 把单位库编进库里，单位上下文在第一次用到时才加载；汇率要联网取数据，从不加载，所以不换算货币。带进 `num-rational`（MIT OR Apache-2.0）与 `strsim`（MIT）。源码在 crates.io 的对应版本，上游仓库是 [codeberg.org/tiffany/rink](https://codeberg.org/tiffany/rink)。MPL-2.0 的履行方式与下文音效包一节的 `symphonia` 相同 |
| `vi`（固定 `=0.8.0`） | 0.8.0 | MIT（crate 以 `license-file` 声明，Copyright 2020 Hung Nguyen） | 越南语方案的 Telex 与 VNI 变换（`crates/engine/src/vietnamese/`）。固定到补丁版本，上游的小版本发布不会悄悄改变输入行为。链接进二进制的传递依赖新增 `nom` 8（MIT）与 `phf`、`phf_shared` 0.11（MIT，与 workspace 已有的 0.13 并存）；`log`、`smallvec`、`memchr` 与 `siphasher` 1 原本就在锁文件里。`phf` 的 `macros` 特性在编译期另带 `phf_macros`、`phf_generator` 0.11（MIT）和 `rand` 0.8（MIT OR Apache-2.0），它们不进二进制。macOS 包内的 `vi-MIT.txt` 是它的许可证全文，Windows 安装包的 `THIRD_PARTY_NOTICES.txt` 也收入同一份（`Collect-Notices.ps1`）。上游仓库是 [ZeroX-DG/vi-rs](https://github.com/ZeroX-DG/vi-rs) |
| `ewts` | 0.1.3 | MIT OR Apache-2.0（按 MIT 使用，作者 Maxim Zommer） | 藏文方案把 EWTS（扩展威利转写）转成藏文 Unicode（`crates/engine/src/tibetan/`）。没有任何依赖。crate 和上游仓库都没有附许可证文件，`resources/licenses/ewts-MIT.txt` 是标准 MIT 文本，版权行写的是 crate 元数据里的作者；macOS 包内的 `ewts-MIT.txt`、Windows 安装包的 `THIRD_PARTY_NOTICES.txt`（`Collect-Notices.ps1`），以及 Android 的 `assets/native-notices/ewts.txt`（`build-native.sh`）、iOS App 资源（`project.yml`）和 HarmonyOS 的 `resfile/licenses`（`stage-resources.sh`）都收入这一份；这三个移动端同样带上 `vi-MIT.txt`。它的转换表按 README 所说取自 [rogerespel/ewts-js](https://github.com/rogerespel/ewts-js)（Copyright (C) 2010-2025 Roger Espel Llima，Apache-2.0）：`ewts-MIT.txt` 末尾附着这条版权行和 Apache-2.0 全文，所以上面每个渠道都随之带上；macOS 和 Linux 的 `THIRD_PARTY_NOTICES.txt` 另外注明出处。上游仓库是 [emgyrz/ewts-rs](https://github.com/emgyrz/ewts-rs) |

## 随包资源（`resources/desktop-dictionary.lock.json`）

锁文件固定九个产物的长度和 SHA-256，每个都带可匿名下载的 URL：八个来自本仓库 `metasequoiaime/msime` 的 `dict-v2.0.2` 发布（由 `.github/workflows/release-dictionary.yml` 构建），`sentence-model.safetensors` 来自 `metasequoiaime/chinese-ime-lm` 的 `model-v1`——两者是不同的仓库和不同的发布，以锁文件里各自的 `url` 为准。原先的第十个产物 `dict_pinyin.dat` 只供 Google 整句解码器使用，随解码器一起去掉。**锁文件本身不记录许可证字段**，来源信息分散在别处：

| 产物 | 大小 | 已知来源 |
| --- | --- | --- |
| `msime.db` | 81.9 MB | 工作词库，含 86 与 98 五笔码表 |
| `english.db` | 1.5 MB | Engine 发布的英文词库 |
| `bigram.bin` | 12.0 MB | Engine 发布的二元语言模型表，整句词格仲裁按它加权 |
| `trigram.bin` | 12.0 MB | Engine 发布的三元语言模型表，同上 |
| `others.db` | 1.3 MB | Engine 发布的表情等数据 |
| `dict_japanese.dat` | 66.5 MB | Mozc 的开源版日文词库，构成见[下一节](#日文词库的分发义务) |
| `mozc_dictionary_oss_README.txt` | 5.8 KB | 上述词库的许可证全文。**分发时必须一同携带**，理由见下节 |
| `dictionary-manifest.json` | 2.5 KB | 资源清单 |
| `sentence-model.safetensors` | 4.5 MB | 整句重排模型，权重为 Apache-2.0；训练语料与分发要求见[下下节](#整句重排模型的署名要求) |

`Artifact` 结构体带 `#[serde(deny_unknown_fields)]`，所以在锁文件里直接加 `license` 字段会让解析失败；要记录许可证需要同时修改 `crates/client-core/src/resources.rs`。在那之前，新增或更换随包资源时请把来源与授权写进本文件。

### 独立神经模型清单（`resources/neural-model.lock.json`）

`resources/neural-model.lock.json` 把同一 `model-v1` 发布中的两个 safetensors 权重放在一起：`sentence-model.safetensors` 是键盘按键路径使用的小模型，`sentence-model-desktop.safetensors` 是桌面输入停顿后使用的大模型。两者都由 `scripts/fetch_neural_model.py` 按 HTTPS、字节数和 SHA-256 下载到 `target/neural-model`；文件不进版本库，锁和本节署名信息随仓库分发。桌面安装器把大模型放在词库目录的同级 `settled-model/`，因为词库目录必须与 `desktop-dictionary.lock.json` 完全相等；小模型仍由词库安装器从后者取回。

### 日文词库的分发义务

`dict_japanese.dat` 是 Mozc 的开源版词典，不是 Google 日本語入力所用的那一份。按随附 `mozc_dictionary_oss_README.txt` 的说明，它由四部分构成：

- **IPAdic**（`mecab-ipadic-2.7.0-20070801`），奈良先端科学技術大学院大学 2000–2003 年版权。允许使用、复制和分发，但要求任何副本——无论原样还是修改过——都必须同时包含其版权声明和紧随其后的两段免责声明。
- **ICOT Free Software**，词条中很大一部分源于此。其条款要求 `NO WARRANTY` 一节**始终**出现在随程序分发的材料中，或附加于其上。
- **冲绳辞書**（[o-dic](http://sourceforge.jp/projects/o-dic/)），明示为 Public Domain，使用、修改、分发均无限制。
- Google 手工增补的形容词／动词、片假名词和复合词，适用 Mozc 自身的条款；该 README 未复述这部分，GitHub 对 `google/mozc` 的许可证识别结果是 `NOASSERTION`，因此本文件不替它断定 SPDX 标识。

**实际后果：分发这份词库时必须一并携带 `mozc_dictionary_oss_README.txt`**，IPAdic 和 ICOT 两条都把"许可证文本随附"写成了硬性条件。锁文件把这个 5.8 KB 的文本和词库本身一起固定并校验，正是为此——它是许可证义务，不是文档习惯，重新打包资源时不要因为"只是个 README"而丢掉它。

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

按 NOTICE.md 的记录，本表的一部分条目直接来自拼音加加 5.x 安装包内的数据表 `fzm.bin`，而拼音加加是商业软件；来源仓库的 `engine/helpcode/NOTICE.md` 写明六张辅助码表没有任何一项拿到明确的再分发授权，并指出 `jiajia` 一行与其余五张性质不同（其余各表只是复现已发表的输入方案）。**在权利澄清之前不要假定这张表可以自由再分发**，打包发布前需确认它在目标渠道是否可接受。退出方式：从 `HELPCODES` 去掉 `jiajia` 那一行，并让该渠道的资源打包不再带上这张表；设置页随之少一个选项，另外五套不受影响。

构成这张表所用的部件拆分与笔顺数据另有来源（rime-radical-pinyin，GPL-3.0，上游含 chaizi/CC-BY-3.0、CHISE/GPL-2+、yi-bai/ids/MIT；笔顺来自 cnchar，MIT），逐条同样见 NOTICE.md。另外五套辅助码表原先随 Engine 归档而来，现在同样放在 `resources/helpcodes/`，来源说明是从 Engine 原样带过来的 [`resources/helpcodes/ENGINE-NOTICE.md`](../resources/helpcodes/ENGINE-NOTICE.md)：它们同样没有拿到明确的再分发授权。

## 自带音效包与插件包（`resources/sound-packs/`、`client-core::plugins`）

内置的九套音效包和两套背景音乐包是本项目自己的作品，随各平台安装包分发，放在资源目录的旁边（资源目录必须与锁文件完全一致）。它们的标识登记在 `client-core::plugins` 的 `BUILTIN_SOUND_PACKS` 与 `BUILTIN_MUSIC_PACKS`，这些标识保留给内置包，用户导入的包不能占用。

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
| `zip`（关闭默认特性，只开 `deflate-flate2`） | 8.6 | MIT | 导入用户选中的 `.zip` 插件包（`crates/client-core/src/plugins/import.rs`）。带进 `typed-path`（MIT OR Apache-2.0）；其余依赖 `crc32fast`、`indexmap`、`memchr`、`flate2` 原本就在锁文件里 |

播放音效包与背景音乐用到的 Rust crate，只链进 macOS、Windows、Linux 的 `host-api`（`crates/host-api/src/key_sound/`）；iOS、Android、HarmonyOS 不链接，HarmonyOS 2in1 用系统 SoundPool 播 `msime_client_key_sound_pack` 解析出的文件，WAV 样本先经上文的 miniaudio 在长度上限内解码、按旋律音高重写，背景音乐则由系统 AVPlayer 流式播放 `msime_client_music_pack` 解析出的曲目：

| crate | 版本 | 许可证 | 用途 |
| --- | --- | --- | --- |
| `kira`（关闭默认特性，只开 `cpal`、`wav`、`pcm`、`ogg`、`vorbis`） | 0.12 | MIT OR Apache-2.0 | 混音、复音、按半音变调播放旋律、流式播放背景音乐。输出走上表已有的 `cpal` 0.18，不另带一份。带进 `glam`（MIT OR Apache-2.0）、`mint`（MIT）、`rtrb`（MIT OR Apache-2.0）、`atomic-arena`（MIT OR Apache-2.0）、`triple_buffer`（MPL-2.0）；`send_wrapper`（MIT OR Apache-2.0）只在 wasm 目标下用到 |
| `symphonia`（关闭默认特性，只开 `wav`、`pcm`、`ogg`、`vorbis`） | 0.6 | MPL-2.0 | WAV 与 Ogg Vorbis 解码，是 `kira` 自己用的解码器；`host-api` 直接调用它，在包的时长上限内逐包解码（`decode.rs`），不整段解码后再检查。带进 `symphonia-core`、`symphonia-common`、`symphonia-metadata`、`symphonia-format-riff`、`symphonia-format-ogg`、`symphonia-codec-pcm`、`symphonia-codec-vorbis`（均为 MPL-2.0）、`extended`（MIT）、`regex-lite`（MIT OR Apache-2.0） |

设置窗口「插件」页导入插件时弹出的系统选择框，只链进 macOS、Windows、Linux 的 Tauri 外壳（`apps/desktop/src-tauri/src/platform/desktop/desktop_plugins.rs`）。选择框只由 Rust 端调用，没有任何 capability 把对话框命令开放给网页：

| crate | 版本 | 许可证 | 用途 |
| --- | --- | --- | --- |
| `tauri-plugin-dialog` | 2.7 | Apache-2.0 OR MIT | 用各平台自己的打开对话框选择插件包文件夹或 `.zip` 文件（macOS 的 NSOpenPanel、Windows 的通用对话框、Linux 的 GTK 文件选择器），在主线程弹出并以设置窗口为父窗口。停在 2.7，因为 2.8 要求 tauri 2.12。带进 `rfd`（MIT，实际的原生对话框实现）与 `tauri-plugin-fs`（Apache-2.0 OR MIT，本插件的依赖，同样没有开放给网页）；它们用到的 `dunce`、`glob`、`schemars`、`serde_repr` 等原本就在锁文件里 |

MPL-2.0 是文件级 copyleft，与本仓库的 GPL-3.0 兼容：这些 crate 未作修改，以二进制随包分发时附上许可证全文并指明源码位置（crates.io 上的对应版本）即可。Linux 与 Windows 由 `platforms/linux/collect-notices.py` 从 Cargo 依赖图收集的通知覆盖；macOS 的输入法包不走这个脚本，由 `resources/licenses/MPL-2.0.txt`（取自 `symphonia-core` 0.6.1 的 LICENSE）复制成 `Contents/Resources/Licenses/MPL-2.0.txt`，并在 `platforms/macos/resources/Licenses/THIRD_PARTY_NOTICES.txt` 里列出这些 crate 与源码位置，`platforms/macos/tests/settings/bundle_contents.py` 检查它在包里。

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

**源文件不进版本库，也不整份随包**。脚本只取 `word`、`phonetic` 和 `translation` 的首行释义——卡片要显示的就这三样——按 `tag` 列（`zk`/`gk`/`cet4`/`cet6`/`ky`/`ielts`/`toefl`/`gre`）分成八本，写成 `client-core::vocabulary::wordbook::Wordbook` 直接能反序列化的 JSON。八本合计约 3 MB，宿主用 serde_json 读，不需要 CSV 解析器或 SQLite 驱动。

考纲归属是 ECDICT 提供的：仓库自带的 `english.db` 有中英释义和语料频次，够做「最常用的一千词」，但不足以断言某个词在四级大纲里。那是已发布的考纲，凭频次给它安一个名字就是编的。

词书**放在 `EngineResources/` 的兄弟目录 `wordbooks/`**，不放进去：`ResourceStore::verify` 要求固定资源目录与词库锁逐字节一致，多一个文件就会破坏那道「证明随包词库完整」的检查。`settled-model` 出于同样理由也是兄弟目录。

退出方式：不运行 `scripts/fetch_wordbooks.py` 即可。设置页随之只提供用户自行导入的词表，不影响其余功能。

## 非英语离线释义（`resources/offline-glosses.lock.json`）

候选词释义的目标语言是法语、日语、西班牙语、俄语、德语或韩语时，离线来源是从英文维基词典译文表生成的六个 SQLite 文件。`english.db` 只有中英释义，这几种语言原来只能走在线翻译。

| 项 | 值 |
| --- | --- |
| 来源 | [英文维基词典](https://en.wiktionary.org/)（Wiktionary contributors），经 [Wiktextract](https://github.com/tatuylonen/wiktextract) 抽取，由 [kaikki.org](https://kaikki.org/dictionary/English/) 发布 |
| 许可 | CC BY-SA 4.0（维基词典按 CC BY-SA 4.0 与 GFDL 双许可，这里取前者）。CC BY-SA 4.0 可单向兼容到 GPL-3.0 |
| 生成器 | `scripts/build_offline_glosses.py`，只用 Python 标准库；离线测试 `scripts/test-offline-glosses.py` 用真实结构的夹具 `scripts/offline-glosses-fixture.jsonl` |
| 产物 | `offline-glosses/zh-<lang>.db`，每种语言一个文件，外加 `offline-glosses-NOTICE.txt` |
| 取词范围 | 同一张译文表（同一个英文义项）里的 Mandarin 行与目标语言行配对；读 `senses[].translations` 和顶层 `translations`；键为简体，用 `msime.db` 的词表校验；每个中文词最多两个义项、每个义项最多两个词 |
| 锁 | `resources/offline-glosses.lock.json`：`input` 是 kaikki dump，`filtered_input` 是生成时实际读的过滤后 jsonl，`artifacts` 是六个数据库和 NOTICE |
| 发布位置 | [`metasequoiaime/chinese-ime-lm` 的 `offline-glosses-2026.09.02`](https://github.com/metasequoiaime/chinese-ime-lm/releases/tag/offline-glosses-2026.09.02)，与整句重排模型同一个仓库，不在应用内更新检查读取的 `metasequoiaime/msime/releases` |
| 获取 | `scripts/fetch_offline_glosses.py` 按锁下载并校验 sha256 与大小，默认写到 `target/offline-glosses` |

Android、iOS、macOS、Windows 与 Linux 的发布工作流在打包前运行 `fetch_offline_glosses.py`，各自的打包脚本发现 `target/offline-glosses` 里有数据库和 NOTICE 时才带上它们。HarmonyOS 的发布工作流还不能在 CI 上出包（缺 DevEco 的 NDK，也不暂存资源），本地按 `platforms/harmony/README.md` 构建时 `stage-resources.sh` 会带上它们。本地构建同样先运行这个脚本，不运行则照常构建、只有英语走离线释义。

kaikki 每周覆盖同一个 URL，所以能复现构建的是 `filtered_input`，用它加上锁住的 `msime.db` 与 `english.db` 运行 `build_offline_glosses.py build`（`--dump-date`、`--source-revision` 取锁里的值）即可得到逐字节相同的数据库，前提是 SQLite 版本与锁里的 `sqlite_version` 一致。上游已把 postprocessed 的 English jsonl 标为 deprecated，将来可能只剩约 2.9 GB 的 raw dump；生成器两种格式都接受。

**放在 resources 的兄弟目录 `offline-glosses/`**，不放进去，理由与 `settled-model`、`wordbooks` 相同：资源目录必须和词库锁逐字节一致，而且各平台只想带自己需要的语言。

**分发义务**：这些文件是维基词典文本的改编作品，发布时必须随附 `offline-glosses-NOTICE.txt`，并保持 CC BY-SA 4.0。App Store 这类带 DRM 的渠道与 CC BY-SA 4.0 第 2(a)(5) 条「不得附加有效技术措施」之间的关系，和已随包分发的 bigram/trigram 是同一个问题，需要维护者判断。

退出方式：不安装 `offline-glosses/` 即可。释义退回只用 `english.db` 和在线翻译（macOS 26 及以上在没有选择翻译服务时还有系统自带的离线翻译，见 [PRIVACY.md](../PRIVACY.md#候选翻译macos-与-linux-新装默认用水杉账号)），其余功能不受影响。

## 粤语与注音的数据（rime-cantonese、libchewing-data）

粤语（粤拼）与注音（大千）两个方案的音节和词条来自两份固定提交的上游数据，由 `msime-dict-build languages` 转换成 `cantonese.db` 与 `zhuyin.db`。两份数据库放在资源目录的兄弟目录 `language-dictionaries/`，不进 `desktop-dictionary.lock.json`，上面「九个产物」不变；理由与 `offline-glosses/` 相同：资源目录必须和词库锁逐字节一致，而且只有 macOS 与 Windows 提供这两个方案（Windows 安装包放在 `server\language-dictionaries`，与 `server\resources` 同级）。数据库不在时，这两个方案显示为不可用。

| 组件 | 许可证 | 位置与说明 |
| --- | --- | --- |
| [rime/rime-cantonese](https://github.com/rime/rime-cantonese) 的 `jyut6ping3.chars.dict.yaml`、`jyut6ping3.words.dict.yaml`、`essay-cantonese.txt`，提交 `259f0e48bba840c3a2e0d117539e96937f3d89bc` | CC BY 4.0（作者为粤语计算语言学基础建设组 CanCLID；拼写采用香港语言学学会 LSHK 的粤拼方案）。全文、署名与改动说明在 `resources/licenses/rime-cantonese-CC-BY-4.0.txt` | 转换成 `cantonese.db`：去掉声调数字、音节以空格连接，字频与词频取自 `essay-cantonese.txt`。按 ODbL 发布的 `jyut6ping3.maps.dict.yaml`、来源不明且没有读音的 `jyut6ping3.phrase.dict.yaml` 和 `jyut6ping3.lettered.dict.yaml` 都不读取、不分发。CC BY 4.0 要求署名并说明改动，许可证文件已写明两者 |
| [chewing/libchewing-data](https://github.com/chewing/libchewing-data) 的 `dict/chewing/tsi.csv`、`dict/chewing/word.csv`，提交 `c44e81aef24b06f1509f19e1be54c99812d0c43f` | LGPL-2.1-or-later（两个文件的文件头 `dc:license` 声明，Copyright (c) 2025 libchewing Core Team）。全文、版权行与固定提交的源码地址在 `resources/licenses/libchewing-data-LGPL-2.1.txt` | 转换成 `zhuyin.db`：词条按 `tsi.csv` 的词频排序，`word.csv` 补全单字。仓库里其他文件不使用；注音从不读 `msime.db`，也不经过简繁转换。上游数据原样未改时，随附许可证全文、版权行和固定提交的源码地址即满足 LGPL 的源码提供义务；若转换本身算作修改，对应的源码是本仓库以 GPL-3.0 发布的 `crates/dict-builder`。这一判断未经法务审阅 |

这些文件由 [msime-dictionary](https://github.com/metasequoiaime/msime-dictionary) 原样收在 `yue/`、`tw/` 下（与上游固定提交逐字节一致），`resources/dictionary-sources.lock.json` 按 `yue/`、`tw/` 路径固定它们在 msime-dictionary `sources-v*` release 里的附件（URL、长度与 SHA-256），上游提交记在同一文件的 `rime-cantonese`、`libchewing-data` 引用里。许可证文本经各平台的通知渠道分发，与下一节的 libhangul 汉字表相同：Windows 的 `Collect-Notices.ps1`，macOS 输入法包的 `Resources/Licenses`（`CMakeLists.txt` 与 `THIRD_PARTY_NOTICES.txt`），Linux 安装的声明（`CMakeLists.txt` 与 `data/THIRD_PARTY_NOTICES.txt`），Android 的 `assets/native-notices`（`build-native.sh`），iOS App 的资源（`project.yml`），HarmonyOS 的 `resfile/licenses`（`stage-resources.sh`）。除 macOS 外的平台目前不提供这两个方案，也不带数据库，但这些方案的代码随每一份引擎分发，统一一份渠道清单检查起来最简单。`scripts/test-language-data-notices.py` 检查许可证文本和这几处渠道都还在。

## 笔画的数据（rime-stroke）

笔画方案（h 横、s 竖、p 撇、n 点、z 折）的笔顺码来自一份固定提交的上游数据，由 `msime-dict-build languages` 转换成 `stroke.db`，与 `cantonese.db`、`zhuyin.db` 放在同一个 `language-dictionaries/` 目录、同一个语言词库资源包里。数据库不在时，笔画方案显示为不可用。

| 组件 | 许可证 | 位置与说明 |
| --- | --- | --- |
| [rime/rime-stroke](https://github.com/rime/rime-stroke) 的 `stroke.dict.yaml`，提交 `1e8fff9b9494ddec23b0cbc526bcfd8171a6fd48` | LGPL-3.0（仓库的 `LICENSE`；`AUTHORS` 记载四季的風、雪齋、Kunki Chou 整理的主码表依 CNS11643 资料的授权声明以 LGPL 再发布，扩展至 Ext J 的数据来自宋天，同为 LGPL）。主码表源自 CNS11643 全字库，该资料按「政府資料開放授權條款－第1版」要求署名：數位發展部，CNS11643中文標準交換碼全字庫網站，https://www.cns11643.gov.tw。附码表源自北大中文論壇（孙海峰、徐孟罗、唐捺之、谢振斌整理）。全文、署名与固定提交的源码地址在 `resources/licenses/rime-stroke-LGPL-3.0.txt` | 转换成 `stroke.db`：每行 `字<TAB>笔顺码` 原样成为一条，键就是笔顺字母串；一个字的多个笔顺（大陆规范与台湾 CNS11643 笔顺并列）各成一条。上游没有权重，排序用 `cn/SingleCharsAllV1.txt`（rime-ice，GPL-3.0-only）里每个字各读音权重之和，不在表里的字权重为 0；数据库 `license` 元数据因此记为 `LGPL-3.0-only AND GPL-3.0-only`。只有单字，不含词组。上游数据原样未改时，随附许可证全文和固定提交的源码地址即满足 LGPL 的源码提供义务；若转换本身算作修改，对应的源码是本仓库以 GPL-3.0 发布的 `crates/dict-builder`。这一判断未经法务审阅 |

`stroke.dict.yaml` 还没有收进 msime-dictionary 的 `sources-v*` release，`resources/dictionary-sources.lock.json` 暂不固定它；在那之前 `msime-dict-build languages` 只从 `--cache` 目录的 `stroke/stroke.dict.yaml` 读取手动放入的上游文件，并按 `crates/dict-builder/src/stroke.rs` 记下的固定提交、大小与 SHA-256 校验，不联网下载。固定之后改走锁文件，`rime-stroke` 引用必须仍是上面的提交。许可证文本走与上一节相同的全部通知渠道，`scripts/test-language-data-notices.py` 一并检查。

## 编译进共享库的数据

| 组件 | 许可证 | 位置与说明 |
| --- | --- | --- |
| [modood/Administrative-divisions-of-China](https://github.com/modood/Administrative-divisions-of-China) 的省、地、县三级行政区划，提交 `c49d495b40ac73eb1a66f6eeae5f8fd10696f035` | WTFPL（全文在 `resources/licenses/Administrative-divisions-of-China-WTFPL.txt`）；上游整理自国家统计局公布的统计用区划代码与城乡划分代码 | `crates/engine/src/local/places.tsv`，`@` 模式在用户自己的列表之后补充的内置地名（3302 个，约 126 KB，`include_str!` 编进引擎）。`msime-dict-build places` 读 `dist/provinces.csv`、`dist/cities.csv`、`dist/areas.csv` 三个文件生成这张表，三者的 URL、长度与 SHA-256 固定在 `resources/dictionary-sources.lock.json` 的 `places/` 条目；拼音由生成器按字注音，再用 `crates/dict-builder/src/places.rs` 的 `READINGS` 纠正地名专用读音。只取地名和上级关系，不带区划代码。设置里「@ 地名」默认关闭 |
| [libhangul](https://github.com/libhangul/libhangul) 的 `data/hanja/hanja.txt`，提交 `717409ce61524bb3d8426060a384822f21354c62` | BSD-3-Clause（文件头单独声明，Copyright (c) 2005,2006 Choe Hwanjin；全文在 `resources/licenses/libhangul-hanja-BSD-3-Clause.txt`）。libhangul 仓库整体是 LGPL-2.1，但本仓只取这一份单独声明 BSD-3 的数据文件。上游提交历史说明 2006–2007 年部分读音和词表整理自国立国语院的材料，这些材料能否以 BSD 再授权无法独立核实，属于上游作者的声明 | `crates/engine/src/korean/hanja.tsv`，韩语模式把正在组字的音节转换成汉字时的候选表（555 个音节、28416 行，约 430 KB，`include_str!` 编进引擎）。`msime-dict-build hanja` 读这一个文件生成这张表，其 URL、长度与 SHA-256 固定在 `resources/dictionary-sources.lock.json` 的 `hanja/` 条目。只取单音节条目：键是一个预组合音节，值是基本多文种平面内的一个统一汉字，即 CJK 统一汉字、扩展 A 区汉字，或兼容汉字区里 NFC 不改写的 12 个统一汉字（U+FA0E 﨎、U+FA11 﨑 等）；真正的兼容汉字（NFC 会改写，固定版本的单音节条目里没有）、辅助平面汉字和两字值都丢弃；每个音节内保持上游顺序，保留上游的训音（훈음）注释，约 27% 的行有训音，扩展 A 区一行都没有。词条级数据（约 5.7 MB）不使用。BSD-3 第 2 条要求二进制分发时附带版权声明和条款，表编进引擎后随六个平台的引擎一起分发，所以六个平台的通知渠道都收录这份文本：Windows 的 `Collect-Notices.ps1`，macOS 输入法包的 `Resources/Licenses`（`CMakeLists.txt` 与 `THIRD_PARTY_NOTICES.txt`），Linux 安装的声明（`CMakeLists.txt` 与 `data/THIRD_PARTY_NOTICES.txt`），Android 的 `assets/native-notices`（`build-native.sh`），iOS App 的资源（`project.yml`），HarmonyOS 的 `resfile/licenses`（`stage-resources.sh`）。`scripts/test-korean-hanja-table.py` 检查这几处都还在 |
| [OpenCC](https://github.com/BYVoid/OpenCC) 词典，提交 `26753884f1984add422f3b0249ccee8613deaff6` | Apache-2.0 | `crates/client-core/data/opencc/`，许可证全文在同目录 `LICENSE`。`STPhrases.txt`、`STCharacters.txt`、`CJK_Compatibility_Ideographs.txt` 原样取自该提交的 `data/dictionary/`；`STPhrases_GeneratedFromRegionalPhrases.txt` 是该提交的 OpenCC 构建产物（`data/scripts/generate_st_phrases_from_regional_phrases.py` 用 `t2s.json` 生成），本仓不重新生成。提交号与来源 MSIME-Windows 的 `vendor/opencc` 子模块一致。只使用数据，不链接 OpenCC 的 C++ 库；`chinese_conversion.rs` 按 `s2t.json` 的规则实现转换。Windows 通知由 `Collect-Notices.ps1` 一并收集 |

## 本地语音识别

语音服务选 `local` 且 `asr_model_path` 指向已安装的模型目录时，识别在设备上完成。这条路径分两部分：一份随包分发的推理运行时，和由用户在设置页按需下载的模型。**模型不随包分发**，仓库只携带下载地址、长度和 SHA-256。

### 运行时（`resources/voice-runtime.lock.json`）

锁文件为每个平台固定一个 sherpa-onnx v1.13.8 的上游预编译产物（来源提交 `11afbd009a7f8c08f4bcf2fc1b265d0df4670fbf`）的 URL、长度和 SHA-256，`scripts/fetch_voice_runtime.py --platform <平台>` 校验后展开到被忽略的 `target/voice-runtime/<平台>/`，各平台的构建与打包脚本从那里取文件。

| 组件 | 许可证 | 说明 |
| --- | --- | --- |
| [k2-fsa/sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) | Apache-2.0 | 语音识别运行时。宿主不在构建期链接它，而是在第一次识别时用 `dlopen`/`LoadLibrary` 加载其 C API 库（`shared/voice/LocalAsr.h`）。构建期只需要它的 C 头文件，原样放在 `shared/voice/third_party/sherpa-onnx/c-api.h`，许可证全文在同目录 `LICENSE`。Android 使用上游的 `.aar`，HarmonyOS 使用上游的 `.har`，iOS 使用 `SherpaOnnxC.xcframework` |
| [microsoft/onnxruntime](https://github.com/microsoft/onnxruntime) | MIT | sherpa-onnx 的推理后端，随上述产物一起来，版本由 sherpa-onnx 的发布决定。macOS 与 iOS 的产物把它静态链接进 sherpa-onnx 库；Linux 另带 `libonnxruntime.so`，Windows 另带 `onnxruntime.dll` 与 `onnxruntime_providers_shared.dll`。分发时需要携带它的许可证与第三方通知 |

### 模型（`resources/local-asr-models.json`）

模型目录由设置页的模型管理下载：`client-core::voice::local_models` 只接受 HTTPS，逐字节校验长度与 SHA-256，只保留目录清单 `files` 里列出的文件（`test_wavs`、`test_onnx.py`、`bpe.model` 不解包），最后写入 `msime-model.json`。三份归档都是 sherpa-onnx 项目转换成 ONNX 后在 `asr-models` 发布里公开提供的版本，下表的许可证取自目录里各条目的 `license` 字段：

| 目录 id | 模型 | 许可证 | 来源 | 说明 |
| --- | --- | --- | --- | --- |
| `x-asr-zh-en-streaming`（默认） | X-ASR 中英流式 zipformer transducer，int8 | Apache-2.0 | [Gilgamesh-J/X-ASR](https://github.com/Gilgamesh-J/X-ASR) | 原生支持热词。热词需要的 `bpe.vocab` 上游归档里没有，仓库自带一份 `resources/voice-models/x-asr-zh-en-bpe.vocab`（69,594 字节，SHA-256 记在目录的 `extra` 里），编译进 `client-core`，安装时写入模型目录；它是该模型的派生数据，适用同一许可证 |
| `sense-voice-small` | SenseVoice-Small，int8 | **FunASR Model License v1.1**（`LicenseRef-FunASR-Model-License-1.1`），不是 OSI 开源许可证 | [FunAudioLLM/SenseVoice](https://github.com/FunAudioLLM/SenseVoice)，条款全文见 [FunASR MODEL_LICENSE](https://github.com/modelscope/FunASR/blob/main/MODEL_LICENSE) | 阿里巴巴通义实验室。条款要求保留模型名称和署名，目录条目的 `notice` 原样记录了这段署名，设置页在模型旁显示它。**不随 MSIME 分发**，只在用户点下载时从上游取得 |
| `fun-asr-nano`（仅桌面） | Fun-ASR-Nano-2512，int8 | Apache-2.0 | [FunAudioLLM/Fun-ASR](https://github.com/FunAudioLLM/Fun-ASR) | 阿里巴巴通义实验室。解码器是 Qwen3-0.6B，同为 Apache-2.0 |
| `silero_vad.onnx`（后两个模型的附加文件） | Silero VAD | MIT | [snakers4/silero-vad](https://github.com/snakers4/silero-vad) | 整句模型用它切分语音段。从 sherpa-onnx 的 `asr-models` 发布下载，SHA-256 固定在目录的 `extra` 里 |

用户可以在设置里配置下载镜像（`asr_model_mirror`）；镜像只改变从哪里取文件，校验用的 SHA-256 不变，所以镜像无法替换内容。

### 编译进共享层的 Rust 依赖

| crate | 版本 | 许可证 | 用途 |
| --- | --- | --- | --- |
| `pinyin` | 0.11 | MIT | 把用户词库的热词转成无声调拼音，给不支持原生热词的模型做近音纠正（`client-core::voice::hotwords`） |
| `tar` | 0.4 | MIT OR Apache-2.0 | 解包模型归档 |
| `bzip2` | 0.6 | MIT OR Apache-2.0 | 解压 `.tar.bz2`。0.6 默认使用纯 Rust 的 `libbz2-rs-sys` 后端，该 crate 的许可证是 `bzip2-1.0.6`（bzip2 原作的 BSD 式许可，要求保留版权声明） |

## 各平台引入的第三方 SDK

| 平台 | 组件 | 许可 |
| --- | --- | --- |
| Android | `com.google.mlkit:digital-ink-recognition:19.0.0` | **Google 的 ML Kit 服务条款，不是开源许可证** |
| Android | AndroidX、`com.google.android.material` | Apache-2.0 |
| Android | `io.noties.markwon:core:4.6.2`（“设置”页公告正文的 Markdown）及其依赖 `com.atlassian.commonmark:commonmark:0.13.0` | Markwon 为 Apache-2.0，commonmark-java 为 BSD-2-Clause |
| Android | vcpkg 提供的 nlohmann/json（原生库，清单在 `platforms/android/vcpkg.json`） | MIT |
| iOS | `MLKitDigitalInkRecognition` 8.0.0（CocoaPods，链接进键盘扩展 target） | **Google 的 ML Kit 服务条款，不是开源许可证** |
| macOS | Sparkle 2.9.6 | 以上游发布附带的许可证为准；框架不随仓库分发，由构建者按 `platforms/macos/README.md` 记录的 SHA-256 自行取得 |
| Windows | vcpkg 提供的 libcurl、fmt、nlohmann/json、utfcpp（清单与 baseline 在 `platforms/windows/vcpkg.json`） | 各自上游许可证；通知由 `platforms/windows/Collect-Notices.ps1` 收集 |
| Linux | IBus / Fcitx5、GTK 栈、libcurl、ICU、xkbcommon、nlohmann/json、X11 与 Wayland 客户端库 | 各自上游许可证，按发行版依赖引入 |
| 桌面 | Tauri、React、Vite 等 | 见 `pnpm-lock.yaml` 与各自上游 |

两个移动平台的 ML Kit 是识别手写笔迹用的。桌面与 Linux 不使用它，改用 `crates/engine` 里移植的离线 Zinnia 识别器和上面固定的模型；Android 的原生构建明确把 Zinnia 及其模型路径排除在外（`platforms/android/verify-native.sh`）。

仓库不捆绑任何字体文件；界面使用系统字体，`Noto Sans SC` 与 `Microsoft YaHei` 只是回退字体名。

## 语言生态依赖

逐个列出会立刻过时，以锁文件为准：

- Rust：`Cargo.lock`，当前 578 个 package 条目（含本 workspace 自身的成员）。`cargo audit` 是 `scripts/verify-local.sh` 完整版的一个阶段，漏洞视为失败；被接受的 `unmaintained` / `unsound` 公告逐条记在 [`.cargo/audit.toml`](../.cargo/audit.toml) 里，每条都写明引入链和接受理由。
- Node：`pnpm-lock.yaml`。
- iOS：`platforms/ios/Podfile.lock`。

## 通知文件在哪

| 位置 | 覆盖范围 |
| --- | --- |
| `platforms/macos/resources/Licenses/THIRD_PARTY_NOTICES.txt` | macOS 客户端内嵌组件的完整通知 |
| `platforms/linux/data/THIRD_PARTY_NOTICES.txt` | Linux 安装到 `${CMAKE_INSTALL_DATADIR}/doc/msime-client/` 的组件总览，许可证全文由 `platforms/linux/CMakeLists.txt` 的 `MSIME_NOTICE_SOURCES` 一并安装 |
| `platforms/ios/SharedResources/MLKit-NOTICES.txt`、`MLKit-Dependencies.txt` | iOS 的 ML Kit 依赖通知；编进引擎的韩语汉字表的 `libhangul-hanja-BSD-3-Clause.txt` 由 `platforms/ios/project.yml` 作为 App 资源打包 |
| Android APK 的 `assets/native-notices/` | `platforms/android/build-native.sh` 收集的原生依赖声明，含编进引擎的韩语汉字表的 libhangul BSD-3-Clause 声明；两条打包路径都放进 APK |
| HarmonyOS HAP 的 `resfile/licenses/` | `platforms/harmony/stage-resources.sh` 暂存的编进引擎的韩语汉字表的 libhangul BSD-3-Clause 声明 |
| `apps/desktop/src-tauri/gen/android/gradle/LICENSE-2.0.txt`、同目录 `NOTICE.md` | Android Gradle 模板的 Apache-2.0 文本与来源说明 |
| `platforms/windows/Notices.md` | Windows 通知生成器的用法与限制；产物由 `Collect-Notices.ps1` 生成 |

这些生成器和收集器都在各自文档里写明「不是完整性或再分发授权的评估」。发布二进制前的逐平台要求见[开源发布清单](open-source-release.md)。

## 新增依赖时

引入新的上游代码、字体、图标、模型或服务 SDK 时，同时提交来源提交、许可证文本、通知位置和分发限制，并在本文件登记——不要只在 README 留一个链接。资源锁文件只校验内容，不授予任何分发权利。

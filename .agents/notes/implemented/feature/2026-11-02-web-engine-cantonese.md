# Agent Note: 网页引擎的粤拼和粤语双拼

Status: implemented

## Problem

metasequoiaime/msime-web#816 希望在线演示（wasm.msime.app，msime.app 嵌入它）能试粤语双拼：打拉丁字母出中文。网页引擎 `@msime/web-engine` 原来只有全拼、四种双拼、五笔 86、日语和韩文，wasm 层和 SDK 都没有粤语。引擎本身早就有粤拼和[粤语双拼](2026-10-26-cantonese-shuangpin.md)，桌面和触屏宿主靠 `force_cantonese_shuangpin` 区分两者（[桌面的粤语双拼选项](2026-11-02-desktop-cantonese-shuangpin-option.md)）；缺的是 wasm 层的方案名、粤语词库怎么进浏览器，以及演示页的入口。

## Decision

- **两个方案名，同一个引擎方案**：`crates/engine-wasm/src/host.rs` 的 `Scheme` 新增 `Jyutping`（`"jyutping"`）和 `CantoneseShuangpin`（`"cantonese_shuangpin"`），都映射到 `SchemeType::Cantonese`；`session_options` 给它们填 `cantonese_dictionary = <词库目录>/msime-cantonese.db`，`force_cantonese_shuangpin` 只对 `CantoneseShuangpin` 为真，与 host-api 对独立粤语双拼方案的设法相同。粤拼仍保留引擎的自动识别（前两键不是完整音节又能读成双拼码时按双拼读）。wasm 层不写任何粤语规则：候选、部分上屏（选只覆盖前几个音节的候选后其余接着组字）、键位都在引擎里。
- **路由只多一处**：粤语组字中，`WebHost::punct` 把引擎快照 `spelling_symbols` 里的符号（撇号是音节分界，按双拼读时 `;` 是韵母 ing/ik）先当字符交给引擎，引擎不收才按标点处理。粤语不加载句子模型（`uses_model` 改为只认拼音方案），`ordering::reorders_candidates` 对它本来也不重排；辅助码同样只对拼音方案起作用。
- **词库路径**：wasm 上是 `/res/msime-cantonese.db`（`host::CANTONESE_DICTIONARY_PATH`），Worker 用 `import_database` 导入，和拼音库是两个文件，所以两种粤语方案之间 `setScheme` 就地切换、不重新下载，和拼音方案之间互换仍要新建引擎。SDK 和演示页都按「同一份词库为一组」判断（`family`）。
- **词库只进 npm 包，不进 dist**：`scripts/build-web-engine.sh` 新增必填的 `--cantonese` 和 `--cantonese-license`（`--no-data` 时不带），把 `resources/language-dictionaries.lock.json` 钉住的 `msime-cantonese.db` 原样 `gzip -9n` 成 `assets/msime-cantonese.db.gz`，随词库发布的 `msime-rime_cantonese_LICENSE.txt` 原名放在旁边，体积门槛 5 MB（dict-v2.0.48 是 3,546,099 B，原始 7,237,632 B）。名字、大小和许可证文件名写进生成的 `assets.js` 的 `cantonese`（不带数据的构建为 `null`），`web-engine-manifest.json` 和 release 文件不变。`release-web-engine.yml` 按语言词库锁下载并校验这两个文件。CLI `copy` 默认连同许可证复制，`--no-cantonese` 两个都不复制，`info` 列出它们。
- **许可**：`web-engine-NOTICE.md` 新增手写的「粤语词库」一节；`scripts/web-engine-notice.sh` 的 `repo` 段收进 `resources/licenses/rime-cantonese-CC-BY-4.0.txt`。
- **演示页**：输入方案多「粤拼」「粤语双拼」两个按钮，说明和「看它打字」样例同其他方案。粤语双拼的说明从 `demo.js` 的三张键位表（韵母、换键的声母、独立鼻音）生成，`crates/engine-wasm/tests/cantonese.rs` 用 `cantonese::shuangpin::encode` 逐项核对，并穷举四个字母以内的韵母确认没有漏列。演示页跟着 develop 部署、用的却是 `demo/package.json` 钉住的 SDK 版本，所以 SDK 的 `SCHEMES` 里没有的方案按钮隐藏，升级 SDK 后才出现。

## Alternatives considered

- **粤语词库进 dist 和 `web-engine-manifest.json`，和日语模型一样**：SDK、CLI 和 release 从同一份清单读文件，`assets.js` 不用另拼一项。不这样做的理由和[辅助码表](2026-10-08-web-engine-shuangpin-helpcode.md)相同：清单是 TapTapGo 原样抄进锁文件、按 sha256 钉住的 release 契约，`release-web-engine.yml` 还按角色集合精确核对它；多一个 3.5 MB 的角色会让每个按清单下载的接入方多下一份用不到的词库，release 一经发布又不能改。只进 npm 包不碰这份契约，以后真要进 release 再加角色即可。
- **用 `msime-dict-build web` 裁一份更小的粤语库**：拼音库和日语模型都是裁过的。粤语库原始只有 7.2 MB、gzip 后 3.5 MB，比裁过的拼音库还小，原样分发还能和各平台安装包是同一个文件、许可证说明也原样适用；裁剪要新写一段构建器逻辑并为它另写改动说明，收益不够。
- **一个 `cantonese` 方案名，另给一个「强制双拼」选项**：贴近桌面偏好 `cantonese.input_method` 的形状。但 SDK 的方案是一个字符串，演示页和接入方都按方案名选；两个方案名与触屏宿主的独立「粤语双拼」方案一致，切换也只是 `setScheme`，不用为一个只对粤语有意义的选项扩 `createMsimeEngine` 的参数。
- **演示页的键位说明手写一段文字**：最省事，但键位表一改说明就悄悄过期；放成表并由 Rust 测试对照引擎，改键位时测试会失败。

## Consequences

- **收益**：网页引擎和演示页有了粤拼和粤语双拼，规则、候选和部分上屏与桌面、触屏宿主同出引擎；`tests/cantonese.rs` 用同一份夹具词库逐键对照桌面的 `Runtime`，首页和首选上屏一致。选粤语的页面只下载 wasm 和粤语词库（约 7.5 MB）。
- **代价**：npm 包多约 3.5 MB（加许可证约 22 KB）。`assets.js` 和 CLI 现在有三类来源：清单里的 release 文件、辅助码表和粤语词库。GitHub 上的 `web-engine-v*` release 不含粤语词库，只从 release 取文件的接入方要粤语得自己托管它。冒烟测试多了一个必填参数 `--cantonese`，CI 和 release workflow 都用 `make_fixture --cantonese` 写出夹具。
- **上线时机**：演示页按钮要等发布含粤语的 SDK 版本、再把 `demo/package.json` 的版本号改过去才出现；在那之前合并本改动不会让线上演示页出现不能用的按钮。

## Verification

- `cargo test -p msime-engine-wasm --tests`（Studio 上经 `rbuild`），含新增的 `tests/cantonese.rs`：粤拼组字与空格上屏、数字选字、部分上屏后剩余继续组字、撇号分界、`sxcr` 在两个方案下相同、`;` 是 ing、标点结束组字、缺词库时创建失败、与桌面 `Runtime` 逐键对照、演示页键位表与 `encode` 一致、SDK 的 `SCHEMES` 都是 WebHost 方案。`cargo clippy -p msime-engine-wasm --all-targets -- -D warnings` 零警告。
- `node --test 'packages/web-engine/test/*.test.mjs'`；`packages/web-engine/test/smoke.mjs` 的粤语段用真实 wasm 和夹具词库走 SDK 与 Worker：粤拼、就地切到粤语双拼、带身份的点选部分上屏、`b;`、拒绝切到全拼、辅助码被忽略、缺词库报错、CLI `--no-cantonese`。带全部数据（含 `--cantonese`）和 `--no-data` 各构建一次 `scripts/build-web-engine.sh`，两个包上这条冒烟都通过。
- 演示页用带数据构建的包在本机无头 Chromium 里跑过：粤拼 `neihou`、`hoenggong`、`mgoi`、`gwongdungwaa`、`nei'hou` 和粤语双拼 `nzhb`、`hxgs`、`omgo`、`isdpwa` 的首选分别是 你好、香港、唔該、廣東話、你好，`b;` 首选 並（兵、冰在同页），`sxcr` 后按 2 上屏「想」、剩下的 `cr` 接着组字；两个方案的「看它打字」样例上屏 你好、香港、唔該、廣東話；从粤拼切到粤语双拼不重新加载，切回全拼会重建；部署代码写出 `--no-pinyin --no-wubi --no-japanese`。没有在真实桌面浏览器里手动打过字。

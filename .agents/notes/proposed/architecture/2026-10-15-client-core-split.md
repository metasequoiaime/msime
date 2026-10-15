# Agent Note: 按职责把 client-core 拆成四个 crate

Status: proposed

## Problem

`crates/client-core` 在 `docs/ARCHITECTURE.md` 里的定位是「本地配置和固定资源的分代安装」，实际有约 9.8 万行、36 个顶层模块：偏好、版本、主题之外，还有皮肤、账号、插件、词库、语音、AI、翻译、云端、社区、遥测、检查更新，而且没有任何 feature 开关。于是 reqwest、tokio、tokio-tungstenite、image、zip、tar、bzip2、加密库和 wasmi（经 `msime-script-runtime`）是每个下游的无条件依赖：

- `crates/backend-engine` 只用 `chinese_conversion::simplified_to_traditional`，却要编译整棵依赖树。
- `crates/input-runtime` 只用文本谓词、几个偏好常量、`cloud::candidates` 和 `punctuation`，同样拖着网络栈和图片解码。
- 改其中任何一个模块，host-api、input-runtime、桌面外壳都要跟着重编这一个大 crate。

光把模块挪进不同 crate 做不到：模块之间有三个强连通分量。最大的一个（下文 SCC-A）把 preferences、edition 和 account、plugins、skin、clipboard_relay 等十个模块缠在一起，原因只是 preferences 和 edition 引用了上层模块里的几个值类型和校验函数。跟踪 issue 是 #8068。

## Proposal

行为不变，拆成四个 crate，依赖方向为 base → core → services、core → providers；services 和 providers 互不引用。

| crate（路径） | 模块 |
|---|---|
| `msime-client-base`（`crates/client-base`，新建） | storage、file_lock、bounded_io、text（含 `safe_id`）、calendar、chinese_conversion、`data/opencc/` |
| `msime-client-core`（原位置，包名不变） | preferences、edition、host_surface、settings_document、punctuation、helpcode、clipboard、translation、endpoint、resources、cloud、panels、candidate_document、theme；examples 下的 install_resources、verify_resources；`tests/candidate_*.rs`；保留 `pub use uuid` |
| `msime-client-services`（`crates/client-services`，新建） | account、community、telemetry、notices、update_check、diagnostics、clipboard_relay、skin、plugins、dictionary、vocabulary、common_phrases、typing_statistics；`tests/fixtures/plugin-packs` |
| `msime-client-providers`（`crates/client-providers`，新建） | voice、credential、ai、resource_packs；`examples/install_resource_pack.rs`；`tests/voice_wire_interop.rs` |

三个新 crate 都写 `version.workspace = true`：`voice/local_models.rs` 的 user agent 用 `env!("CARGO_PKG_VERSION")`，版本号不跟 workspace 走，发出去的 UA 就会变。依赖规格从现有 `crates/client-core/Cargo.toml` 逐字照抄，每个 crate 只带自己用到的部分；桌面专用的 `msime-script-runtime` target 块连同 cfg 表达式原样搬到 services。

### 依赖环怎么断

- storage 与 file_lock 互相引用，两个都进 base，环留在 crate 内部。
- voice、credential、resource_packs 互相引用，三个都进 providers，环同样留在内部。
- SCC-A 靠把 core 用到的上层定义下沉到 core 断开，原位置用 `pub use` 导回，对外路径不变：
  - `AccountPreferenceValue` 定义在 `preferences`，`account` 导回；
  - `EffectStyle`、`DEFAULT_SOUND_PACK`、`DEFAULT_MELODY_PACK` 定义在 `preferences`，`plugins` 导回；
  - 中继地址和令牌校验（`valid_relay_url`、`valid_relay_token`、`parse_relay_url`）定义在 `preferences`，`clipboard_relay` 导回前两个；
  - 全局主题 id、五套内置调色板、`rgb`、`luminance`、`readable_text`、`is_selectable_id` 定义在 core 的 `theme` 模块，`skin::theme`、`skin::catalog` 导回；`rgb`、`luminance`、`readable_text` 是 `pub`，因为留在 services 的 `skin::theme`、`skin::ai` 还要用；
  - `safe_id` 定义在 `text`，`skin::catalog` 以 `pub(crate) use` 导回；
  - preferences 测试里用到 voice、ai 的两条测试放在 `voice::provider`、`ai` 自己的测试模块；
  - core 解析地址用直接依赖 `url = "2.5"` 的 `url::Url`，不经 `reqwest::Url`。根锁文件和 `fuzz/Cargo.lock` 里都只有一份 url 2.5.8，两者是同一个类型。

core 那组模块（含 base）里 must 没有任何对 services、providers 模块的代码引用，包括测试和文档链接；`cloud/clipboard_e2e.rs` 里反引号括起来的 `crate::clipboard_relay` 是文字，不是链接，不算。

### 阶段

每个阶段一个 PR，按顺序合并，PR 正文写 `Refs #8068`，最后一个写 `Fixes #8068`。

- 阶段 0：在 client-core 内部切断 SCC-A，即上一节列出的下沉。
- 阶段 1：抽出 base。base 里的 `pub(crate)` 改成 `pub`（storage 和 bounded_io 是 `#[doc(hidden)] pub mod`）；core 的 lib.rs 用 `use`/`pub use` 把 base 接回来，crate 内部的 `crate::storage::...` 路径不用改。backend-engine 改依赖 base。Android 设备上的 client-core 单测改用 cargo runner 逐个测试二进制运行（见 [Android 设备门禁](../../implemented/testing/2026-10-08-android-device-gates.md)，那篇的事实同批更新）。
- 阶段 2：抽出 providers，core 去掉 tokio、tokio-tungstenite、futures-util、tar、bzip2、pinyin。
- 阶段 3：抽出 services，core 去掉 reqwest、toml、zip、image、zune-jpeg、pulldown-cmark、unicode-general-category、flate2 和 script-runtime。`.gitattributes` 里 plugin-packs fixture 的 `-text -whitespace` 规则 must 跟着改路径：那些 fixture 里的 CRLF、BOM 本身就是用例，规则失效时 Windows 检出会悄悄改写字节。msime-cloud 的 `scripts/sync_client_dialect.py` 写死了 `crates/client-core/src/skin/catalog/client_dialect.json` 的 raw 地址，要有一个同日合并的配套 PR。
- 阶段 4：文档里的路径和 `cargo test -p msime-client-core` 一类命令改到新 crate，本笔记转 `implemented/`。

每个新 crate 都要同时进根 `Cargo.toml` 的 `members` 和 `default-members`：path 依赖会自动成为 member，但不会进显式的 `default-members`，裸 `cargo test` 就会悄悄不再跑它的测试。

### 每个阶段都守住的检查

- `cargo test -p <这些 crate> -- --list` 数出的测试总数等于拆分前 client-core 的 1441 条（lib、集成测试、examples 合计）。
- `cargo tree -p msime-host-api -e normal,features --prefix none | sort -u` 前后的差异里只多出 `msime-client-*` 节点。
- 根 `Cargo.lock` 和 `fuzz/Cargo.lock` 除了新增本地包、url 成为 client-core 的直接依赖之外，没有版本变化。
- 每个模块只编进一个 crate，never 用 `#[path]` 把同一个文件编进两个 crate。

### 当场审计

按 client-core、crate 拆分、依赖环和被下沉的符号名检索了 proposed、implemented、rejected：

- [Android 设备门禁](../../implemented/testing/2026-10-08-android-device-gates.md)：部分重叠，阶段 1 改设备上跑 client-core 单测的方式，那篇的命令随阶段 1 就地更新。
- [自建云剪贴板中继](../../implemented/feature/2026-10-12-cloud-clipboard-relay.md)：部分重叠，写着客户端在 `client-core/src/clipboard_relay.rs`，阶段 3 后路径变化，就地更新事实；决定本身不变。
- [桌面生命周期核心](../../implemented/architecture/2026-10-11-desktop-lifecycle-core.md)：部分重叠，脚本插件的 target 依赖块随 plugins 搬进 services，cfg 表达式不变。
- 其余 140 多篇提到 `msime-client-core` 或 `crates/client-core/src` 的笔记只是在验证命令或路径里提到它，与本决定无关，路径事实在阶段 4 统一核对。没有被本决定取代或吸收的笔记。

## Alternatives considered

- **cloud 也搬出 core。** 最强的理由是 chacha20poly1305、x25519-dalek、hkdf、getrandom 这些加密库全在 cloud，搬走后 core 更轻。不搬，因为 input-runtime 要用 `cloud::candidates`，cloud 自己只依赖 base 那一层（它是 sans-IO 的），放在 services 会让 input-runtime 为一个模块拖进整个 services；services 的 `plugins::script_commands` 本来也要 getrandom。
- **不加 url 依赖，core 继续用 `reqwest::Url`。** 好处是不多一个直接依赖。但 preferences、translation、endpoint 只解析地址不联网，留着 `reqwest::Url` core 就甩不掉 reqwest，拆分对 input-runtime、backend-engine 的意义就没了；锁文件里本来就有 url 2.5.8，加上它不多一个包。
- **阶段 0 就把下游改到新路径，不留 `pub use`。** 好处是不留两条路径，阶段 3 的 sed 少改几处。不这样做：host-api、desktop、host-windows 等处有几十个引用，其中 desktop 只能在 Linux 容器里检查，阶段 0 的目的只是断环，`pub use` 让它不碰任何下游；下游路径在搬模块的那个阶段一起改。
- **用 `#[path]` 让同一个源文件同时编进两个 crate。** 能省掉 `pub` 化和重新导出。但 `storage.rs`、`edition.rs` 里有 static 状态，编进两个 crate 就是两份，锁和缓存各管各的；类型也会变成两个名义上不同的类型。
- **每个阶段跑一遍 `verify-local.sh --quick`。** `.github/CONTRIBUTING.md` 这样要求，也最稳。但改名或改 Cargo.toml 时 `--quick` 会直接全量，每次 45 分钟到 2 小时；每个阶段只跑上面列的针对性命令加改动涉及的脚本，全量交给 CI，阶段 3 再补本机的 Android 模拟器和 Wine。

## Acceptance criteria

- 四个 crate 按上表存在，依赖方向如上，`cargo tree -p msime-client-core -e normal -i tokio` 和 `-i reqwest` 为空，`cargo tree -p msime-backend-engine -e normal -i reqwest` 为空，`cargo tree -p msime-input-runtime -e normal` 里没有 reqwest、tokio、image、zip、wasmi、tar。
- 上面「每个阶段都守住的检查」在每个阶段都成立。
- 各阶段的 `cargo test`、`cargo clippy --all-targets -- -D warnings` 通过；消费方（host-api、input-runtime、mcp-server、pack-tool、host-macos、host-windows、tauri-mobile-platform、backend-engine、engine-wasm、clipboard-relay）`cargo check --all-targets` 通过，desktop 在 Linux 容器里 `cargo check -p msime-desktop --all-targets` 通过；iOS 目标和 windows-gnu 目标的检查通过。
- `git check-attr -a crates/client-services/tests/fixtures/plugin-packs/valid/wordbook-basic/words.tsv` 输出 `-text -whitespace`。
- 仓库里（不算 `target/` 和 `.agents/notes/archived/`）不再有指向已搬走文件的路径。

## Risks

- **合并冲突**：别的 PR 改了要搬走的模块会和对应阶段冲突；往旧目录新增文件的 PR 因为没有 mod 声明会直接编译失败，不会悄悄出错。
- **Android 设备测试在 PR 上不跑**：阶段 1 和阶段 3 合并前要在本机 API 28、35 模拟器上跑过。
- **对外 API 变大**：base 约 67 项、core 约 7 项从 `pub(crate)` 变成 `pub`，用 `#[doc(hidden)]` 压住，没有承诺稳定性。
- **二进制不会变小**：host-api 仍然链接全部四个 crate，受益的是 backend-engine、input-runtime、host-macos 的编译图和增量编译。
- **文档链接**：CI 不跑 rustdoc，intra-doc 链接断了只在本地 `cargo doc` 时出现 warning。
- **本机跑不了的环境**：fuzz 不在 CI 里，`fuzz/Cargo.lock` 要手工保持同步；HarmonyOS 的 native 构建需要 SDK。
- **跨仓库**：msime-cloud 的同步脚本和几处注释写死了 client-core 的路径，配套 PR 没有同日合并时，同步脚本取到的地址是 404。

# Agent Note: macOS 发布打包提速：只编所需目标、并行公证、合并 Rust 编译、sccache

Status: implemented

## Problem

`release-macos.yml` 的「Build macOS package」一步在 0.53.0（run `38423362429`）用了 60 分钟，0.54.0 第 1 次尝试（run `39267138324`）在 1 小时 7 分时才打到第二个版本。按日志时间戳拆开 0.53.0：

| 阶段 | 用时 |
| --- | --- |
| 资源准备（install_resources、资源包检查） | 约 1.7 分钟 |
| host 库，aarch64 + x86_64 两次 cargo | 4m38 + 4m23 |
| CMake 构建（`cmake --build` 全部目标） | 11m41 |
| msime-mcp，两次 cargo | 3m45 + 3m26 |
| 设置应用 msime-desktop，两次 cargo | 6m29 + 6m34 |
| 六个版本逐个打包、公证、装订、检查 | 约 17 分钟（每个约 2m50，其中公证上传和排队约 1 分钟） |

浪费集中在三处：

- `package-release.sh` 用不带 `--target` 的 `cmake --build`，把 `platforms/macos/CMakeLists.txt` 里上百个测试可执行文件和 Swift 测试全编了一遍，包里一个都不用。
- host 库、msime-mcp、msime-desktop 各自一次 cargo 调用，三次的 feature 集合不同（见下文），engine、client-core、input-runtime、host-api 和一半第三方依赖每个 target 编三遍；`Compiling msime-host-api` 在每对日志里都出现。
- 每个版本 `notarytool submit --wait`，六个版本依次各等一遍 Apple 的处理。

维护者看到的「110 分钟」是 2026-10-05 的 run `37290546639` 两次尝试加起来的时长：第 1 次在 50 分钟时因五笔版签名取不到时间戳失败（之后加了 `codesign_timestamped` 重试），第 2 次重跑用了 47 分钟。单次尝试离 120 分钟的 `timeout-minutes` 并不近，但一次失败就意味着整轮重来；而单次执行时间已经从 47 分钟涨到 60 到 76 分钟（0.53.0、0.54.0 与同日 nightly），随版本和依赖增多还在涨，问题是真实的。

0.54.0 第 1 次尝试还暴露了另一种浪费：五笔版公证通过后装订三次都在 CloudKit 票据查询处断线，整轮构建作废，只能整个 job 重跑。装订重试窗口已由 #8856 放宽（见 [nightly 公证笔记](2026-10-15-macos-nightly-notarized.md)），这里处理的是装订失败与其他版本的关系。

## Decision

### CMake 只编包里用到的目标

`package-release.sh` 用 `cmake --build ... --target MSIMEClientInputMethod backend-library-load-test`。输入法 bundle 的全部内容都出自 `MSIMEClientInputMethod` 和它的依赖（`apple-client`、`msime-voice-providers`、`msime-voice-local`、`macos-swift-backend`、`macos-voice-runtime`）及其 POST_BUILD；`backend-library-load-test` 是紧接着的 `ctest -R '^(bundle-contents|backend-library-load)$'` 要跑的可执行文件，`bundle-contents` 是 Python 测试，不需要编译。

不加 `BUILD_TESTING` 一类开关：CMakeLists.txt 里的测试没有被这样包起来，给一百多个 `add_executable` 和 `ALL` 自定义目标统一套条件要动整个文件，而 `--target` 只改打包脚本一行，CI（`ci-macos.yml`）和本地开发照旧全编、跑完整 `ctest`。

### host 库与 msime-mcp 合并成一次 cargo 调用

`cargo_universal` 改为接受多个产物名：`cargo_universal 输出目录 产物... -- cargo 参数`。host 库和 msime-mcp 一起编：`-p msime-host-api -p msime-mcp-server`。

用 `cargo tree -e normal,build -f '{p} {f}' --target <triple>` 在两种 target 上分别比对了三次调用解析出的 feature：

- host-api 单独编与 host-api + msime-mcp 一起编：msime-mcp 一侧的 feature 集合与单独编它时**完全相同**（只差一行格式），所以 msime-mcp 产物不变。host 库这一侧多出的全是增量 API：tokio 的 `io-std`、`macros`，futures-util 的 `async-await`、`async-await-macro`、`channel`，futures-sink 的 `std`，tracing 的 `attributes`，indexmap 的 `serde`，serde 的 `alloc`，以及只在编译期用的 syn 的 `full`、`visit-mut`。这些只增加类型、宏和 trait 实现，不改已有代码路径；多出的代码在输入法链接时被 dead strip 掉。
- 设置应用（msime-desktop）**不并进来**：它会给 objc2 打开 `exception`（Objective-C 异常的处理方式随之改变）和 `disable-encoding-assertions`，给 serde_json 打开 `unbounded_depth`（取消解析深度上限）和 `raw_value`，给 block2、dispatch2、objc2-foundation 和 objc2-core-foundation 打开更多 feature。前两项会改变 host 库和 msime-mcp 的运行行为，不满足「产物功能不变」。

`MSIME_VERSION` 现在也在编 host 库时设置；只有 `crates/mcp-server/build.rs` 读它。

### 公证先全部提交，再统一取结果，按顺序装订

- 每个版本 DMG 签好名后 `notary_submit` 用 `notarytool submit --output-format json` 提交、只记下提交 id，接着打下一个版本。
- 全部版本打完后，`notary_result` 对每个 id 执行 `notarytool wait`，再以 `notarytool info` 的 `status` 为准；wait 因网络提前返回、拿不到最终状态时隔 15 秒再等，最多五轮。任何一个不是 `Accepted`，先把每个版本的结果和失败提交的 `notarytool log` 都打出来，再让打包失败。
- 全部通过后才逐个装订，一律经过 `platforms/macos/scripts/staple-ticket.sh`，不同时向 CloudKit 取几份票据。某个版本装订失败（`staple-ticket.sh` 已按断线重试过）时先装订其余版本，过 60 秒再给失败的补一轮；补一轮仍失败才失败。已经编好、公证过的其他版本不再因为一个版本的票据下载断线被提前丢掉。
- 之后 `finish_edition` 按版本顺序做 `spctl` 评估、挂载 DMG 跑 `check_app`、报告体积，与原来每个版本末尾做的检查相同。`check_app` 用来比对构建号和公钥的输入法，换成该版本 DMG 暂存目录里 ditto 出来的同一个 bundle，因为 `target/macos` 里那份已被后面的版本清掉。
- 设置应用的 Sparkle 归档（`*-settings.zip`）改在提交公证之后、取结果之前生成：它取的是已签名的应用本身，票据不装订进应用，内容与等结果之后再打相同；`SHA256SUMS` 里各归档的顺序不变。
- 没有公证凭据时（本地、fork、`ci-macos-package.yml` 的 ad-hoc 包）只打印未公证警告，其余检查照常；单个版本的本地运行走同一条路径。

### Rust 编译缓存：sccache + GitHub Actions 缓存

维护者批准为这个 job 加编译缓存。沿用仓库已经在用的 `mozilla-actions/sccache-action`（固定在 `fc920bf0…`，v0.0.11），后端用 GitHub Actions 缓存（`SCCACHE_GHA_ENABLED=true`），与 `release-windows.yml` 在托管 runner 上的做法相同；`CARGO_INCREMENTAL=0`。

- **缓存不能让发版失败**：安装步骤 `continue-on-error`；只有 `sccache --start-server` 成功才把 `RUSTC_WRAPPER=sccache` 写进 `GITHUB_ENV`，否则照常编译；`SCCACHE_IGNORE_SERVER_IO_ERROR=1` 让构建途中 sccache 服务出错时 rustc 直接在本机编；缓存读写失败 sccache 按未命中处理。统计步骤同样 `continue-on-error`。同一接法后来推广到 CI 与 Windows 发版里所有用 Depot Cache 的 Rust 任务，见 [CI 的 sccache 起不来时照常编译](2026-10-25-ci-optional-sccache.md)。
- **可见范围与投毒**：GitHub 缓存按 ref 隔离。main 上的发布只能读 main 和默认分支 develop 写入的条目，PR（包括 fork 的 PR）写在 `refs/pull/<n>/merge` 下，发布读不到；在其他分支手动触发的运行只写自己分支的范围。能写入发布可读条目的只有在 develop 或 main 上运行的 workflow，而那需要先合并代码，与被发布的源码处在同一条信任边界上。条目的键是 rustc 的全部输入（编译器版本、参数、源文件与依赖产物的哈希、dep-info 里登记的环境变量），命中时拿到的是同样输入编出的同一份 rlib。
- **服务不随空闲退出**：`SCCACHE_IDLE_TIMEOUT=0`。默认空闲 600 秒退出，而六个版本打包时十几分钟不编 Rust，最后的统计步骤会起一个计数为零的新服务；第一次验证运行就因此显示 0 次请求，实际已写入 1321 个条目。
- **缓存覆盖面**：rustc 对 rlib 的编译可缓存；`bin`、`cdylib`、过程宏这类 crate 类型不缓存（第二次验证运行统计里 155 次）。cc crate 看到 `RUSTC_WRAPPER=sccache` 也经它编 C/C++ 和汇编，同样命中缓存。收益主要来自第三方依赖；工作区 crate（engine、client-core 等）每次发版都有改动，照样要编，所以真实发版的命中率低于下文验证运行的 100%。

## Alternatives considered

- **在 CMakeLists.txt 加 `MSIME_BUILD_TESTS`（或 `BUILD_TESTING`）开关，打包时关掉** — 最强的理由是意图写在构建系统里，以后新加的测试默认受控，不用记得打包脚本的目标名。否定：测试目标散落在一千三百行的文件里（还有 `ClipboardTests.cmake`、`shared/voice`），统一包条件是一次大改动，漏包一个就悄悄变回全编；`--target` 只依赖两个稳定的目标名，打包脚本里的 `ctest` 本来就按名字点这两个测试。
- **三个 Rust 包都并进一次 cargo 调用** — 最强的理由是再省一对 host 库链条的重编（约 3 分钟）。否定：feature 统一后 objc2 的 `exception` 和 serde_json 的 `unbounded_depth` 会进入 host 库和 msime-mcp，改变运行行为。
- **公证全部并行，装订也同时进行** — 最强的理由是省掉装订的串行等待。否定：0.54.0 的失败就出在 CloudKit 票据下载断线上，同一时刻发六个查询只会让它更容易断；装订每份只要几秒，串行不花多少时间。
- **Swatinem/rust-cache 缓存 `target/`** — 最强的理由是连非 rlib 的产物（host-api、过程宏、build script 输出）一起恢复，命中时更快。否定：仓库已在用并固定了 sccache-action，不用新增第三方依赖；`target/` 下两种 target 加 universal 产物有几个 GB，而 sccache 的条目按编译单元存取，缓存服务出错时也更容易退回正常编译。sccache 在两个交叉 target 和 lipo 流程下工作正常：缓存的是 rustc 的单次编译，lipo 在 cargo 之后，第二次验证运行两个 target 都是 100% 命中。
- **把 job 搬到自托管或 Depot runner** — 有现成的 Depot Cache，也更快。否定：这个 job 持有 Developer ID 证书和公证凭据，按 workflow 里的约定留在 GitHub 托管 runner 上。

## Consequences

同一提交 `7c3c919f75` 上、同一天的三次运行（都打六个版本、都签名公证）。基线是旧脚本的 nightly（run `39332197145` 的 macOS 任务），之后是本分支第一次（run `39288415598`，sccache 冷缓存）和第二次（run `39337923532`，热缓存）验证运行：

| 阶段 | 旧脚本 nightly | 新脚本，冷缓存 | 新脚本，热缓存 |
| --- | --- | --- | --- |
| 资源准备 | 1m29 | 2m11 | 0m48 |
| host 库 + msime-mcp（各两个 target） | 10m43 + 9m16 | 15m27（合并） | 5m33（合并） |
| CMake 构建 | 12m13 | 3m51 | 3m04 |
| 设置应用 msime-desktop | 17m08 | 18m18 | 8m59 |
| 六个版本打包、公证、装订、检查 | 24m52 | 17m36 | 18m48 |
| 「Build macOS package」合计 | 76m25 | 58m06 | 37m45 |
| release job 合计 | 78m14 | 60m04 | 39m42 |

rebase 到 develop `eb5bd41b55` 后又在同一源码上比了一次：旧脚本是维护者发的 0.54.1-beta（run `39345442127`），新脚本是本分支第三次验证运行（run `39354321497`）。这次的缓存来自上面 `7c3c919f75` 的运行，中间改过的工作区 crate 不命中，最接近真实发版：

| 阶段 | 旧脚本 0.54.1-beta | 新脚本 |
| --- | --- | --- |
| 资源准备 | 1m23 | 1m19 |
| host 库 + msime-mcp | 11m00 + 12m45 | 8m36 |
| CMake 构建 | 16m30 | 3m18 |
| 设置应用 | 17m13 | 12m14 |
| 六个版本打包、公证、装订、检查 | 23m26 | 17m01 |
| 「Build macOS package」合计 | 83m04 | 43m00 |
| release job 合计 | 87m01 | 44m41 |

sccache 这次 1295 次命中、27 次未命中（Rust 25 次，C/C++ 2 次），命中率 97.95%；六个 DMG 的 `Contents/Resources` 与 beta 逐个相同，`Verify release assets` 通过。

- **收益**：CMake 一步省约 9 分钟，host 库与 msime-mcp 合并在冷缓存下也省约 4.5 分钟，并行公证让六个版本的打包少约 6 分钟（以前每个版本约 1 分钟的公证等待现在与下一个版本的打包重叠，最后一个版本提交后只等了不到一分钟）。sccache 热缓存再省约 20 分钟：第二次运行 1322 次命中、0 次未命中。真实发版改动过的工作区 crate 不命中，Rust 部分会落在冷、热两列之间。装订失败不再让同一次运行里已经公证过的其他版本作废：第二次运行里拼音版第一次装订就遇到 `NSURLErrorDomain Code=-1005`，15 秒后重试成功，其余版本照常。
- **产物**：两次验证运行和 nightly 都产出同样的六个 DMG、同样的 artifact 文件清单，`Verify release assets` 通过，各版本 `Contents/Resources` 大小一致。把 nightly 与第一次验证运行的 full 版 DMG 挂载比对：1580 个文件名、符号链接全部相同；除 Mach-O 和 `CodeResources` 外 1563 个文件内容逐字节相同；去掉代码签名后，msime-mcp、MSIMEBackend.dylib、msime-voice-local、sherpa-onnx 和 Sparkle 的各个可执行文件逐字节相同。msime-desktop 只差 LC_UUID 和调试映射里目标文件的修改时间，这是两次构建之间本来就有的差异。输入法可执行文件小 544 字节：链接进来的 host 库换了 feature 集合，依赖 crate 的哈希随之改变，mangled 符号名里的哈希段不同，`__text` 相差约 3 KB（arm64，约 0.016%）；导入的符号完全相同。
- **代价**：host 库的静态库比以前多带几项未使用的依赖 API（链接时剥掉）。DMG 体积报告从「每个版本打完立即输出」变成全部装订之后集中输出。sccache 一个 ref 范围约 1321 个条目、1.2 GB，占仓库 GitHub Actions 缓存额度（10 GB，验证后约 3.8 GB 已用），由 GitHub 按 7 天未访问和 LRU 淘汰；冷缓存时写入条目让 Rust 编译反而慢一点（设置应用 18m18 对 17m08）。
- **仍然最慢的一段**：每个版本创建 ULMO 格式的 DMG 约 1 分 50 秒，六个版本合计约 11 分钟，是打包阶段的大头；它与后一个版本的打包互不依赖，可以作为下一步放到后台并行。
- **需要重访的信号**：msime-mcp 的依赖新增了会改变行为的 feature（再跑一次上面的 `cargo tree` 比对）；新增了要进包的 CMake 目标而它不是 `MSIMEClientInputMethod` 的依赖；Apple 公证排队变长到连最后一个版本的等待也成为瓶颈；sccache 命中率持续很低或缓存额度吃紧。

## Verification

- `python3 scripts/test-macos-notarization-flow.py`：用合成的 `xcrun` 跑 `notary_submit` 与 `notary_result`，覆盖处理中提前返回、Invalid、Rejected、取不到状态，以及密码不进输出；还核对脚本不再 `--wait`、装订经过 `staple-ticket.sh`。
- `python3 scripts/test-macos-stapler-retry.py`：装订与 `--validate` 两种模式的断线重试、重试用完的退出码（装订 68，核对 75）。
- 在 Mac Studio 上 `cmake --build target/macos-isolated --target MSIMEClientInputMethod backend-library-load-test` 后跑 `bundle-contents` 与 `backend-library-load`，再构建其余全部目标，输入法 bundle 的 184 个条目（文件哈希和符号链接）前后完全相同。
- 三次 `gh workflow run release-macos.yml -f publish=false`（runs `39288415598`、`39337923532`、`39354321497`）成功；`publish` 为假时发布、上传 dSYM、appcast 和 Homebrew 几步都跳过，没有建出任何 tag 或 release。

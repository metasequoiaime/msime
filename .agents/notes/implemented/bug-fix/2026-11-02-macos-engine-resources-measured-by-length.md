# Agent Note: macOS 包里的 EngineResources 按文件长度量，不按 du 的已分配块数

Status: implemented

## Problem

macOS 0.57.0 正式版的发布工作流（run 41438608904，main 上的 `b21fa6e878`）在 `finish_edition` 里失败：五笔版的安装包展开后，`check_app` 报 `EngineResources is 213656 KiB, over the 198900 KiB locked core-dictionary budget`。工作流按顺序收尾各版本，五笔版失败后拼音、日文、越南文、藏文版都没被查到。

多出来的 18852 KiB 不是内容：

- 同一次运行里，`package_edition` 在打 pkg 之前对同一个应用跑过同一个 `check_app`，那时在预算内；pkg 的载荷就是紧接着 `ditto` 进去的这个应用。
- 展开后的 `check_app` 先跑 `verify_resources --edition wubi`，它要求目录里恰好是锁定的那些文件、长度和 SHA-256 都对，这一步通过了；体积检查在它之后才失败。
- 前一天的 `macos-v0.56.2-beta`（`9bb5af0706`，同一个 runner 镜像 `macos-15-arm64 20261028.0489.1`、同一套 pkg 流程）五笔包下载下来展开，`EngineResources` 正好 194804 KiB；两次之间没有任何提交动过 `stage-resources.sh`、`package-release.sh`、`tauri.macos.conf.json`、版本锁或 `resources/helpcodes`，Tauri 报的整个五笔应用只大了 1.6 MiB（输入法可执行文件）。
- 本机按发布规则暂存五笔版，`du -sk` 与按长度取整都是 194804 KiB，打成同样参数的 pkg 再 `pkgutil --expand-full`，五次都是 194804 KiB。

预算一侧是锁里的长度按 4 KiB 向上取整，被量的一侧却是 `du -sk` 报的已分配块数。块数取决于文件系统怎么、什么时候给刚写出的文件分配空间，那台 runner 上展开出来的文件多占了 18 MiB 的块，内容一个字节没变。六个版本的预算余量都正好是 4096 KiB，任何一个版本都可能这样被误拒。没能在本机复现 runner 上多分配的具体机制。

## Decision

- 预算和被量的一侧收进同一个脚本 `platforms/macos/scripts/engine_resources_budget.py`：预算照旧是本版本锁里实际随包的条目、`resources/helpcodes` 的辅助码表和许可声明逐个按 4 KiB 向上取整，再加 4096 KiB；被量的一侧是 `EngineResources` 下每个文件（不跟随符号链接）的 `st_size` 按 4 KiB 向上取整之和，同一个规则。`check_app` 调它，不再用 `du`，打包前和展开安装包后两次检查都一样。
- 超限时脚本把每个文件的长度、计入的 KiB 和已分配的 KiB 都打出来，下一次失败直接能看出是多了文件、文件变大，还是分配。
- `ci-macos-package.yml` 的 `layout` job 按发布规则把 `shared/contracts/editions.json` 里每个版本的 `EngineResources` 各暂存一次，跑 `verify_resources --edition` 和这个脚本。`package` job 仍只打 full；版本锁、辅助码表或暂存规则让哪个版本超限，在 develop 上就报出来，不用等发布当天按顺序撞上。`resources/editions/` 与 `resources/helpcodes/` 也算打包输入，改了它们就跑这项检查。
- `scripts/test-macos-package-resources.py` 核对 `check_app` 用这个脚本、`check_app` 里没有 `du -sk`，并直接调脚本的函数：预算随锁、稀疏文件按全长计（`du` 会量成 0）、多一个不在锁里的 4 MiB 文件就超限。

预算本身的口径与来由见[词库发布锁跟上设备实际在用的词库](2026-10-27-dictionary-lock-latest.md)。

## Alternatives considered

- **把余量从 4 MiB 调大到能吞下 18 MiB。** 改一个数就能让这次发布过去。但被比较的仍是块数，下一次多分配多少没有上限，余量越大，这项检查越拦不住它本来要拦的东西（多带一个不在锁里的文件）；而且原因还是没人知道。所以不采用。
- **在 `du` 之前 `sync`，或展开后等一会再量。** 如果多出来的是还没落盘的延迟分配，这样可能就稳定了。但这只是猜测，本机复现不出来就验证不了它有效；即使有效，检查结果仍然依赖文件系统的分配策略，换一台机器、换一个系统版本又可能变。按长度量与预算同一个口径，结果只取决于内容。
- **把 develop 上的 `package` job 改成六个版本都打。** 能在发布之前跑一遍和发布完全相同的流程。但每个版本要多一次 Tauri 打包、签名和 pkg，develop 上每次动打包输入都要多花约十五分钟 macOS runner；而且它挡不住这一次：这次失败的不是内容，是度量，同样的 `du` 在另一次运行里可能就过了（beta 那次就过了）。体积预算只取决于暂存的内容，在 Linux runner 上按版本暂存就能查全，所以选这个。

## Consequences

- **收益**：同一份内容的体积检查结果固定，不再随 runner 的文件系统分配而时过时不过；超限时日志里有逐文件的长度与分配，能直接定位。六个版本的核心词库预算在 develop 上都会被查到。
- **代价与已知上限**：检查不再反映安装后在磁盘上实际占的块数，那从来不是这项检查要守的东西（用户磁盘的分配本来就由安装器和文件系统决定）。`helpcodes/` 子目录里的文件只按长度计、不做哈希校验，和以前一样靠 4 MiB 余量兜住。如果哪天要守「安装后的磁盘占用」，应当另立一项检查，而不是改回 `du`。

## Verification

- 本机按 `MSIME_EDITION=<id> MSIME_MACOS_OMIT_ON_DEMAND=1 bash platforms/macos/stage-resources.sh` 暂存六个版本，`engine_resources_budget.py` 的结果：full 与 pinyin 106420 / 110516 KiB，wubi 194804 / 198900 KiB，japanese、vietnamese、tibetan 6408 / 10504 KiB；`du -sk` 与之相同。`macos-v0.56.2-beta` 的五笔包展开后同样是 194804 / 198900 KiB。
- `python3 scripts/test-macos-package-resources.py` 在改动后通过；把 `package-release.sh` 换回改动前的版本，它报 `check_app` 没有用这个脚本、仍在用 `du -sk`。

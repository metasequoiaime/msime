# Agent Note: 背单词内置词书进安装包

Status: implemented

## Problem

背单词的八本内置词书（中考、高考、CET-4、CET-6、考研、雅思、托福、GRE）从 2026-09-23 功能上线（#632）起就没有进过任何安装包。`scripts/fetch_wordbooks.py` 能从 `resources/wordbook.lock.json` 锁定的 ECDICT 提交生成它们，`crates/client-services/src/vocabulary/builtin.rs` 也会在资源根的 `wordbooks/` 里读它们，但没有任何发版工作流运行这个脚本，也没有任何打包脚本把 `wordbooks/` 放进包里。上线时的验证是手工把词书暂存到资源根之后做的，所以没有暴露。

结果是每个安装上的设置页「背单词」词书列表都是空的，下拉框因为 `books.length === 0` 被禁用，只显示「未选择」；用户要先自己导入词表才能用这个功能。缺词书从来不会让构建失败：`builtin.rs`、`host.rs` 和设置应用的注释都把「宿主没暂存词书」当成正常情况，只列导入的词表。

## Decision

内置词书和英文释义表一样，作为资源目录的同级目录 `wordbooks/` 随包分发，发版时必须在包里。

- `scripts/fetch_wordbooks.py` 在八本 `<id>.json` 旁边写出 `wordbooks-NOTICE.txt`：说明来源和提交，再逐字附上锁定提交的 ECDICT `LICENSE`（MIT 要求版权与许可声明随数据分发）。`resources/wordbook.lock.json` 因此多了 `license_file`，与 `resources/pronunciations.lock.json` 指向同一份 LICENSE。宿主只按 `<id>.json` 读书，NOTICE 放在同一目录里不会被当成一本书。
- 词书不分产品版本：背英文单词和有没有中文方案无关，日文、越南文、藏文版也带。
- 各平台的打包脚本在设了 `MSIME_REQUIRE_WORDBOOKS=1` 时，缺了锁文件列出的任何一本就失败；发版工作流和打包检查工作流都设它，本地打包不设时有就带上。这条约束由 `scripts/test-macos-package-resources.py` 对 macOS 两个工作流做静态检查，Windows 由 `platforms/windows/installer/tests/package-files.ps1` 用夹具检查。

### 各平台的资源根

`builtin::directory(resources)` 是 `resources.join("wordbooks")`，不向上找，所以每个宿主传进来的必须是放着 `EngineResources/`（或等价目录）的那一层，而不是资源目录本身。

| 平台 | 读的进程 | `wordbooks/` 的位置 | 接入 |
| --- | --- | --- | --- |
| macOS | 设置应用 `MSIME.app`，按 `resource_dir()` | `MSIME.app/Contents/Resources/wordbooks` | `stage-resources.sh` 暂存到 `target/macos/wordbooks`，`package-release.sh` 拷进包并在 `check_app` 里核对；`release-macos.yml`、`ci-macos-package.yml` 生成词书 |
| Windows | `msime-client-settings.exe` 和 `MSIME.exe`，都以所在的 Server 目录为资源根 | 安装目录 `server\wordbooks` | `Prepare-PackageFiles.ps1` 装进 `server_exe\wordbooks`；`release-windows.yml` 生成词书 |

Linux、Android、iOS 的宿主当前传进来的资源根不对（Linux 的 `resource_dir()` 指向不会被安装的 `lib/MSIME`，Android 的是 `asset://localhost/`，iOS 传的是 `EngineResources` 本身），鸿蒙的发版工作流不生成词书；这四个平台随后续改动接入，接入时更新这张表。

## Alternatives considered

- **发布预先生成好的词书，打包时按锁下载。** 这是 `pronunciations` 等释义表的做法（锁里的 `built` 指向 release 附件），最强的理由是打包不必再下载 66 MB 的 ECDICT CSV、产物有固定哈希。没有采用：词书的生成是确定性的，输入已经由锁按 SHA-256 固定，几秒就能生成完；预先发布要多维护一个 release 附件和一组哈希，换来的只是省一次下载。以后打包时间或下载稳定性成了问题，可以改成这种方式，宿主一侧不用改。
- **把八本 JSON 提交进仓库。** 打包完全不依赖网络。没有采用：3.3 MB 的生成数据会随每次重新生成整段变动，`fetch_wordbooks.py` 的设计本来就是源文件和产物都不进版本库，与 `settled-model`、释义表一致。
- **保持可选，不加 `MSIME_REQUIRE_WORDBOOKS`。** 和其他同级资源一样有就带、没有就跳过，改动最小。没有采用：词书正是在「可选、缺了也能打包」的状态下从上线起就漏掉了，没有任何检查报出来；发版必须带上，才能让以后再漏掉时打包失败。

## Consequences

- 每个桌面安装包增加约 3.3 MB（八本词书合计）。
- macOS 和 Windows 的发版、macOS 的 develop 打包检查每次多下载一次 ECDICT CSV（约 66 MB，按锁核对哈希）并生成词书。
- 本地用 `package-release.sh` 打包时不设 `MSIME_REQUIRE_WORDBOOKS`，没先跑 `python3 scripts/fetch_wordbooks.py` 的话包里照旧没有内置词书，背单词只列导入的词表。
- 验证：本机跑 `fetch_wordbooks.py` 产出八本词书和 NOTICE；`stage-resources.sh` 把它们暂存到 `target/macos/wordbooks`；`cargo test -p msime-client-services --lib vocabulary` 104 个用例通过；`scripts/test-macos-package-resources.py` 和 `scripts/run-checks.sh` 通过。macOS 的完整 universal 打包和 Windows 的安装包测试（`package-files.ps1`，需要 PowerShell）由 CI 运行，没有在本机跑过。

# Agent Note: Windows 安装包的 LICENSE.txt 被语言词库的授权文本顶替

Status: implemented

## Problem

`platforms/windows/installer/Prepare-PackageFiles.ps1` 在前面把 `$license` 设成仓库根的 `LICENSE`（GPL-3.0），最后把它复制成安装包的 `LICENSE.txt`。词库数据里有 GPL-3.0 的第三方内容，GPLv3 第 4、6 节要求许可证全文随程序交给接收者，这就是那份文件。

2026-10-02 的 `798d434ac`（粤拼、注音、越南文方案）在两者之间加了装语言词库的循环，循环变量也叫 `$license`：`$license = Join-Path $languagesSource $pair[1]`。赋值发生在检查词库文件是否存在之前，所以只要本版本用到语言词库（full 用到注音和笔画），`$license` 就会停在最后一对的授权文件上：

- **词库在场**（发布流程 `release-windows.yml` 会先跑 `scripts/fetch_language_dictionaries.py`）：`LICENSE.txt` 被静默换成那份词库的授权文本。full 现在是 `msime-rime_stroke_LICENSE.txt`，加笔画之前是注音的 libchewing 授权。GPL 全文没有随安装包分发。windows-v0.1.0、v0.2.0、v0.2.1 都包含这次提交。
- **词库不在**（干净的 worktree 里做本地安装）：打包时以 `Cannot find path ...\target\language-dictionaries\msime-rime_stroke_LICENSE.txt` 失败，而脚本的说明写的是「缺少词库时什么也不装」。

本机做的包里确认过：暂存出的 `installer/LICENSE.txt` 与 `msime-rime_stroke_LICENSE.txt` 逐字节相同。`tests/package-files.ps1` 没能发现，是因为它的夹具把每个文件都写成同样的 `fixture` 内容，GPL 和词库授权在夹具里分不出来。

## Decision

- 循环里的授权文件改用 `$languageLicense`，外层的 `$license` 只表示仓库的 GPL `LICENSE`。
- `tests/package-files.ps1` 的 `LICENSE` 夹具写成独有的内容，在装了注音和笔画词库的那次打包后断言 `LICENSE.txt` 就是它。对改动前的脚本，这条断言以 `LICENSE.txt is not the repository LICENSE` 失败。

## Alternatives considered

- **把复制 `LICENSE.txt` 挪到语言词库循环之前** — 一行移动就能修好这一次。否决：变量名复用的陷阱还在，之后任何人在两者之间再加一处同名赋值，都会悄无声息地重演；改名让两个含义分开，再加一条能区分两者的断言。

## Consequences

- **收益**：安装包重新带上 GPL 全文；没有语言词库时打包也能照说明继续。
- **代价与已知上限**：已经发布的 windows-v0.1.0 到 v0.2.1 安装包仍是旧的 `LICENSE.txt`，只能由下一次发布纠正。这件事本身不涉及代码，是否另外处理（例如在发布说明里注明）由维护者决定。

## Verification

- `pwsh platforms/windows/installer/tests/package-files.ps1` 改动后通过；只还原 `Prepare-PackageFiles.ps1` 的改名，新断言失败。
- 同一个测试在 develop 上原本就在更早一步失败（夹具里缺 `version.txt`，见 [本地安装的版本号](2026-10-16-windows-local-install-vs2022-and-version.md)），这次一并补上，才走得到新断言。

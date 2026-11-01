# Contributing to 水杉输入法

感谢参与水杉输入法共享客户端。提交改动前，请先阅读 [ARCHITECTURE.md](../docs/ARCHITECTURE.md) 了解分层方式、不能打破的四条边界、验证流程和各平台的验证覆盖面，[变更记录](../docs/CHANGELOG.md)里是历次改动。使用编码代理的话，[AGENTS.md](../AGENTS.md) 里是给它们的操作约定。

## 5 分钟快速开始

第一次贡献，只需要做下面四步。架构、许可证和完整验证流程放在后面的章节，需要时再读。

1. **找一个任务**：在 issue 列表里筛选 [`good first issue`](https://github.com/metasequoiaime/msime/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22) 标签，选一个你感兴趣的。不确定的可以先在 issue 里留言问维护者。
2. **开分支**：`git checkout -b fix/<简短描述> origin/develop`。不要直接在 `develop` 上改。
3. **做小改动**：只改和 issue 直接相关的文件。改动越小，审阅越快。
4. **提交前检查**：运行 `bash scripts/verify-local.sh --quick`，确认通过后用 Conventional Commits 格式提交，例如 `fix(data): 修正粤语双拼「為/爲」词条`。

提交 PR 时，在描述里写清楚：改了什么、为什么改、在哪个平台上验证过什么。

> 没有 Rust 或 Node 环境也能参与：词库数据、文档、issue 整理和复现报告同样重要。

## 开发流程

1. 从远端实际默认分支（当前为 `origin/develop`）创建独立分支或 worktree，放在仓库外的 `~/worktrees/<feature>/` 下；不要在共享主工作区切换分支。
2. 保持改动聚焦；目录或命名整理应同步更新构建文件、测试和文档引用。
3. Rust 改动运行相关测试、`cargo fmt` 和 `cargo clippy`；前端改动运行类型检查和构建；原生宿主改动在可用平台上运行对应验证。目录整理必须同时更新构建文件、测试路径和平台 README。
4. 提交前运行 `scripts/verify-local.sh --quick`，并检查 `git diff --check`。
5. 新增文本契约检查时，把它写成 `scripts/test-<name>.py` 即可：`scripts/run-checks.sh` 会自动发现并执行 `scripts/` 下每一个 `test-*.py`，阶段名取自文件名，不需要再去脚本里登记；`verify-local.sh` 的每次运行（包括 `--quick`）和 `Core CI` 的 contracts job 都跑它。因为没有登记这一步，检查必须能在任何贡献者的机器和 CI 的 ubuntu runner 上直接运行：缺少它需要的工具链或输入（pnpm、参考仓库、Android SDK、交叉编译器等）时，打印一行 `skipped: <原因>` 并以 0 退出，而不是失败。确实不能无参运行的检查（需要参数、需要加锁，或由别的门禁负责执行），在 `scripts/run-checks.sh` 的 `special_checks` 里写成 `名字=负责执行它的脚本`（没有任何门禁执行的，写成说明如何手动运行它的文档）；`contract check registry` 阶段会在该脚本没有实际调用它、该文档没有提到它、或脚本已不存在时失败。
6. 使用 Conventional Commits，例如 `fix(linux): ...`、`refactor(android): ...` 或 `docs: ...`。每个可验证切片独立提交；合并前运行 `bash scripts/verify-local.sh --quick`。

不要提交真实输入、凭据、个人资料、生成目录或本机配置。测试数据应使用合成值。

## 许可证

仓库整体是 GPL-3.0-only，贡献默认按它授权。例外目录各有自己的 `LICENSE`，见 [README](../README.md#许可证)：

- `crates/engine`（`msime-engine`）和 `crates/path-trust`（`msime-path-trust`）用 MPL-2.0。向这两个目录提交的贡献按「GPL-3.0-only OR MPL-2.0」授权：提交 PR 即表示你同意这些改动既可按 GPL-3.0-only、也可按 MPL-2.0 使用。PR 模板里有一条对应的确认项，改到这两个目录时必须勾上；不同意的话请不要改这两个目录。
- `crates/script-sdk`、`docs/script-plugin-template`、`plugins/script` 的代码和 `plugins/phrase_table` 的数据用 MIT，贡献按 MIT 授权。

往引擎里加第三方代码或数据之前，先确认它的许可证允许以 MPL-2.0 的 crate 分发（GPL、LGPL 许可的代码和数据不能编进引擎），在 [crates/engine/README.md](../crates/engine/README.md#许可证) 的表里加一行，许可证全文放进 `resources/licenses/` 并接到各平台的声明渠道（[第三方组件清单](../docs/third-party.md#通知文件在哪)）。

## Pull request

描述问题、行为变化和验证结果：在哪个平台上跑了哪些命令，得到什么。CI 会在 Pull Request 上运行，本地 quick 门禁仍是提交前的快速检查。

GitHub 的 issue 和 PR 模板会提醒贡献者使用合成输入、列出实际验证并补充第三方许可证信息。安全漏洞、凭据泄露、真实输入或进程边界问题不要通过公开 issue/PR 报告，请按 [安全策略](SECURITY.md) 私下提交。

# Agent Note: msime-engine 测试进 CI

Status: implemented

## Problem

`msime-engine` 的单元测试和 golden 基准此前没有任何 CI 作业运行。`ci-platforms.yml` 的 Linux 容器只跑 `msime-client-core`、`msime-input-runtime` 和 `msime-host-api` 的测试，`Linux settings app` 只跑 `msime-desktop`，`Web engine (wasm)` 只跑 `msime-engine-wasm`；提交前的 `scripts/verify-local.sh --quick`（pre-push 和 pre-merge-commit 钩子跑的就是它）只编译不跑 Rust 测试。唯一运行引擎测试的是全量 `verify-local.sh`，靠作者自觉。

2026-10-10 就出了一次：`a8b0b24f87`（#6744，18:52 合入）让光标处插入少了一次分配，却没更新 `session::tests::typing_at_a_caret_reuses_the_editing_text_length` 钉死的 207。作者跑过引擎测试，看到 expected 207、actual 206，把它当成既有失败排除了；二分证明父提交 `ca7367b208` 还是 207。此后 develop 上的引擎测试一直是红的，#6745 到 #6754 照常合入，它们的 CI 里没有一项检查运行这个测试，直到 #6755（20:17）顺手同步成 206。206 是 #6744 预期内的收益，不是回退，但这一点要靠事后二分才确定。

同一轮还查到一类只在容器里出现的失败：在 `Dockerfile.desktop-check` 镜像里以 root 跑 `cargo test -p msime-desktop`，`private_open_rejects_a_replaced_parent`、`private_remove_refuses_a_symlinked_parent` 和 `stale_snapshot_cleanup_never_deletes_through_a_symlinked_directory` 失败。它们用 `std::os::unix::fs::symlink` 建要被拒绝的链接，root 在 0700 临时目录里建的链接满足 `msime_path_trust::is_root_only_link`，被当成受信任的系统链接跟随过去。`Linux settings app` 以 runner 普通用户运行，所以 CI 一直是绿的。

## Decision

- `ci-platforms.yml` 新增 `Rust engine` 作业：`changes` 作业新增 `engine` 输出，取共享层（`crates/`、`packages/`、`apps/`、`scripts/`、`shared/`、工作区清单、工具链和本 workflow）是否改动；为真时在 `depot-ubuntu-24.04-8` 上以普通用户跑 `cargo test --locked -p msime-engine`，包括 1729 项单元测试和 31 项 golden 基准，用 sccache 缓存编译。整句模型不在仓库里，依赖它的测试照旧自行跳过并说明原因，与没有模型的本机一致。
- 它不是必需检查：加进 develop 和 main 的保护规则要改仓库设置，还得在 `ci-docs.yml` 补同名检查，这次不动。失败照样让 PR 变红。
- 测试里要被拒绝的符号链接一律用 `msime_path_trust::untrusted_symlink` 建，这条约定 #3773 已经立下，上面三个桌面端用例是漏网的，这次改过来；它们在容器里以 root 运行后全部通过。

## Alternatives considered

- **让 `--quick` 也跑引擎测试**：最强的理由是在推送前就拦住，不用等 CI，引擎测试本身只要十几秒。没有采用，因为 `--quick` 的约定是「只编译」，钩子耗时一长就会被绕开（ARCHITECTURE.md 写过这个教训），`--no-verify` 也随时可以跳过它。CI 是唯一每个 PR 都绕不过去的地方。
- **把 `msime-engine` 加进 `platforms/linux/tests/tools/in-container.sh` 的 `cargo test`**：最强的理由是不新增作业，复用已有的容器和缓存。没有采用：容器以 root 运行，引擎里有 4 项测试只在 root 下失败（三项同样是用普通 `symlink` 建链接，一项用 0444 权限模拟写入失败，root 不受权限位限制），要先改它们；而且把引擎测试挂在平台作业下，它的失败会显示成「Linux native host」坏了。
- **CI 跑整个 workspace 的 `cargo test`**：覆盖最全。没有采用：`msime-desktop` 和 `msime-host-macos` 要前端产物或 macOS 资源，在 ubuntu 上整体跑要额外装 Tauri 依赖并处理平台专属 crate，是另一项更大的改动；`msime-desktop` 已有自己的作业。
- **把分配预算从 `==` 放宽成 `<=`**：漂移就不会让测试失败。没有采用：精确预算的用处正是让每次分配变化都被看见，并在同一个 PR 里说明来由；放宽之后 207→206 这类变化根本不会留下记录。

## Consequences

- **收益**：引擎行为和分配预算的漂移在引入它的 PR 上就变红，不再靠事后有人跑全量本地验证才发现；golden 基准也第一次在 CI 上对照。
- **代价与已知上限**：每个碰到共享层的 PR 多一个 Depot runner 作业，测试本身二十秒左右，冷缓存时编译要几分钟。CI 只在 Linux x86_64 上跑，分配预算如果出现平台差异（目前在 macOS arm64 和容器里的 Linux aarch64 上一致），会以这个作业失败的形式暴露，届时要么修预算，要么为平台分开钉。不是必需检查，所以红了仍然合得进去；如果再出现带着它变红合并的情况，就该把它加进保护规则。引擎里那 4 项只在 root 下失败的测试还没改，目前没有任何流程以 root 跑引擎测试。

## Verification

- 本机 macOS arm64：`cargo test -p msime-engine` 1729 项通过、13 项忽略，golden 31 项通过，总耗时约 25 秒（增量编译）。
- `Dockerfile.desktop-check` 镜像（Linux aarch64，root）：`cargo test -p msime-engine` 除上面 4 项 root 专属失败外全部通过，所有分配预算测试与 macOS 一致；`cargo test -p msime-desktop` 改动前 3 项失败，改动后 331 项全部通过。
- 二分：`3c8570335a`（207）到 `a67c25279e`（206）之间，`git bisect run` 定位到 `a8b0b24f87`；`b0959fbdd2` 和 #6744 合入前的 `72b1365557` 都是 207。

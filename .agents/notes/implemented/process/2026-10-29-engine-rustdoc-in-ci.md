# Agent Note: 发布 crate 的 rustdoc 在 CI 上零警告

Status: implemented

## Problem

`msime-path-trust`、`msime-engine`、`msime-engine-capi` 的 0.1.0 已经发到 crates.io，docs.rs 按它们的文档注释渲染 API 文档。仓库里没有任何检查跑 rustdoc 的 lint：`cargo test` 跑文档测试但不报 intra-doc 链接，clippy 不碰 `rustdoc::` 这组 lint，CI 里也没有 `cargo doc`。于是 `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p msime-path-trust -p msime-engine -p msime-engine-capi` 在 develop 上积了 10 个警告，全在 `msime-engine`：

- 公开文档链到私有项 7 个（`rustdoc::private_intra_doc_links`）：`touch` 的三处 `COORDINATE_LIMIT`、`SchemeSet::ALL` 的 `Self::LAST`、`user_dictionary` 里 `LearningHabitsMerge`、`DictionaryStateMerge` 和根导出的 `dictionary_state_revision` 各一处，链向的是 `user_dictionary` 这个私有模块里的同名函数。
- 没转义的方括号 1 个（`rustdoc::broken_intra_doc_links`）：`lattice/octagram.rs` 头部布局里的 `char[32]` 被当成指向 `32` 的链接。
- 裸 URL 2 个（`rustdoc::bare_urls`）：`handwriting/features.rs` 与 `handwriting/model.rs` 的 zinnia 出处。

docs.rs 上三个 crate 的 0.1.0 都构建成功（这些是警告，不让构建失败），但链到私有项的地方在 docs.rs 上是点不开的链接，`char[32]` 渲染成原样的方括号。

## Decision

- 文档注释逐处修正，不动代码：私有模块里的函数有公开的同名包装（`msime_engine::host::merge_learning_habits`、`merge_dictionary_state`、`cached_dictionary_state_revision`，同一份缓存与合并规则），链接改指向这些公开路径；没有公开对应物的私有常量（`COORDINATE_LIMIT`、`SchemeSet::LAST`）改成普通代码片段；方括号转义；URL 加尖括号。私有函数 `within_limit` 的文档链到私有常量不报警告，保持链接。
- `ci-platforms.yml` 的 `Rust engine` 作业在引擎测试之后加一步，以 `RUSTDOCFLAGS=-D warnings` 跑同样的 `cargo doc --no-deps --locked -p msime-path-trust -p msime-engine -p msime-engine-capi`，条件与引擎测试相同（`changes` 的 `engine` 输出，即共享层有改动）。不加 `--document-private-items`，和 docs.rs 一致，链到私有项的那类警告只在这种模式下出现。
- 三个 crate 都不需要 `[package.metadata.docs.rs]`：docs.rs 用默认特性和默认目标（x86_64 Linux）构建，0.1.0 三个全部成功；`msime-path-trust` 的 `test-support` 只开给别的 crate 测试用的建链接辅助函数，`msime-engine-capi` 的 `capi` 特性不开任何代码，都不该出现在采用方看到的文档里，所以不用让 docs.rs 打开特性。

和[引擎测试进 CI](../testing/2026-10-10-engine-tests-in-ci.md) 部分重叠：本篇只在那个作业里加一步，不改它的触发条件和覆盖范围。[C 库预编译发布](2026-10-29-engine-capi-prebuilt-release-and-data-pack.md) 管三个 crate 的发布产物，本篇只管它们在 docs.rs 上的文档，互不吸收。

## Alternatives considered

- **在三个 crate 的 `lib.rs` 里写 `#![deny(rustdoc::all)]` 或 `#![deny(rustdoc::broken_intra_doc_links, …)]`**：最强的理由是规则跟着源码走，本机 `cargo doc` 也会直接报错，不靠 CI。没有采用：这些属性只在跑 rustdoc 时生效，本机和 CI 照样没人跑 `cargo doc`，还是要一个 CI 步骤才拦得住；而 `-D warnings` 在 CI 上一并覆盖以后 rustdoc 新增的警告类别，不用逐个列出。
- **给私有常量也写成链接、在 CI 上加 `--document-private-items`**：最强的理由是维护者本机读私有文档时能点过去。没有采用：docs.rs 不这样构建，采用方看到的正是会坏的那份；CI 要检查的是发布出去的文档。

## Consequences

- **收益**：再有人在公开文档里链到私有项、写裸方括号或裸 URL，引入它的 PR 就会变红，docs.rs 上的文档不再悄悄积坏链接。修过的几处在 docs.rs 上指向能打开的公开函数。
- **代价**：碰到共享层的 PR 在 `Rust engine` 作业里多编一次三个 crate 的文档，依赖只做元数据检查并走 sccache。CI 只在 Linux 上生成文档，`cfg(windows)`、`cfg(target_os = "macos")` 下的公开项的文档不在检查范围，和 docs.rs 默认目标一致。它不是必需检查，红了仍然合得进去。

## Verification

- 修改前：同一条命令不加 `-D warnings` 报 10 个警告，分布如上；加了就在 `msime-engine` 处失败。
- 修改后：Mac Studio 上（`rbuild`，macOS arm64，`rust-toolchain.toml` 钉住的工具链）`RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --locked -p msime-path-trust -p msime-engine -p msime-engine-capi` 通过，零警告。`actionlint` 对 `ci-platforms.yml` 无报告。
- docs.rs：`https://docs.rs/crate/<name>/0.1.0/status.json` 三个都是 `doc_status: true`，构建页显示 「All builds succeeded」（2026-10-29，nightly 1.101.0）。Linux 上的 `cargo doc` 由这个 PR 的 CI 首次实跑。

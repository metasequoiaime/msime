# Agent Note: NiuTrans 失败回复 FFI 先校验长度

Status: implemented

## Problem

`msime_client_niutrans_translation_reply_failed` 收到宿主传入的指针和长度后，直接用 `std::slice::from_raw_parts` 构造借用切片，再交给共享解析器。解析器内部虽然会拒绝超过 1 MiB 的切片，但切片构造本身已经要求整段地址可读；长度超过实际缓冲区时，安全边界在进入解析器前就被破坏。成功解析和自定义翻译的 FFI 入口都已经在构造切片前检查同一上限，失败判定入口需要保持一致。

## Decision

`msime_client_niutrans_translation_reply_failed` 通过一个可注入解析闭包的原始缓冲区辅助函数处理输入。辅助函数先拒绝空指针和超过 1 MiB 的长度，只有通过检查后才调用 `from_raw_parts`；FFI 入口把现有 `niutrans_translation::failed` 作为闭包传入。单元测试使用悬空的非空指针和超长长度，并让解析闭包在被调用时 panic，证明超长路径在借用前返回失败。

## Alternatives considered

- **只依赖 `niutrans_translation::failed` 内部的长度检查**：最强的理由是已有检查且改动最少。没有采用：调用方已经先构造了不满足 `from_raw_parts` 前置条件的切片，内部检查无法撤销这次未定义行为。
- **在 FFI 函数内直接复制一份 `if` 判断**：最强的理由是实现最短。没有采用：原始指针边界容易在相邻入口继续分叉；抽出小型辅助函数后，测试可以直接证明“先校验、后借用”的顺序，且解析逻辑保持不变。

## Verification

- 旧实现下，新增的辅助函数回归先因符号不存在而编译失败，确认测试不是空跑；实现后定向测试通过。
- `cargo test --locked -p msime-host-api --lib niutrans_reply_failure_checks_length_before_borrowing` 通过。
- 已运行 `cargo test --locked -p msime-host-api --lib niutrans`、相邻 `translation_` 回归、`cargo fmt --all -- --check`、`cargo clippy --locked -p msime-host-api --all-targets -- -D warnings` 与仓库 quick 门禁，全部通过。

## Consequences

超长或空指针的 NiuTrans 失败回复在进入解析器前稳定返回失败，不会因外部长度不可信而构造越界切片；合法回复的解析、重试和负缓存语义保持不变。新增一个很小的原始缓冲区辅助函数和一项只覆盖 FFI 顺序的单元测试。

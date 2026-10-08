# Agent Note: 本地语音安装非 Unix 分支补全文件类型

Status: implemented

## Problem

本地语音模型安装流程在 Unix 分支把目录句柄保存为 `Option<File>`，非 Unix 分支却写成无类型的 `None`。Windows 交叉编译时 Rust 无法推断 `Option` 的元素类型，导致 client-core 无法构建。

## Decision

为非 Unix 的 `model_directory` 和 `staging_directory` 显式声明 `Option<File>`，让两套条件编译分支保持相同的接口类型。

## Alternatives considered

- 删除变量并复制整段条件编译逻辑：会扩大平台分歧，增加后续维护成本。
- 使用占位结构体：没有必要，已有 `File` 类型已经是下游函数的统一句柄类型。

## Consequences

Unix 行为不变；Windows 分支可以完成类型检查，仍按原路径实现其平台专用安装逻辑。

## Verification

- CI Windows GNU 交叉构建曾在旧实现因 `Option<_>` 类型推断失败而失败。
- `cargo fmt --all -- --check` 通过。
- `cargo clippy -p msime-client-core --lib --locked -- -D warnings` 通过。

# Agent Note: 插件与本地语音模型共用暂存目录生命周期

Status: implemented

## Problem

插件导入与本地语音模型安装各自保存 staging 路径、父目录句柄和叶名称，并复制了创建、打开和析构清理的逻辑。两处安全边界相同，却可能在以后分别修改而产生差异。

## Decision

`storage::StagingDirectory` 统一负责 staging 目录的生命周期。Unix 上先打开父目录，再用这个句柄创建、打开、清理 staging；非 Unix 保留路径创建和清理。插件导入与语音模型安装都使用它；模型所需的 `model/` 子目录通过共用入口的子目录方法创建和打开。

只有 `create()` 成功后，实例才取得清理该 staging 名称的责任。若同名目录已经存在或创建因其他原因失败，析构保留原有目录及其内容。否则一次随机名称碰撞就可能删除另一项操作的暂存数据。

创建出的 staging 路径仍提供给现有后续流程。插件导入和模型安装原有的命名、验证、发布与错误映射不变。此决定沿用[语音模型 staging 句柄](../bug-fix/2026-10-09-local-model-staging-directory-handle.md)和[插件导入残留句柄](../bug-fix/2026-10-09-plugin-import-leftover-directory-handle.md)的安全边界。

## Alternatives considered

- 只抽取共同的父目录打开函数，可以少改调用点；但两个析构实现和目录创建策略仍需同步维护。
- 继续保留两份局部类型，便于各自独立演化；但两处已有相同的句柄与清理不变量，分开维护增加安全策略漂移的机会。

## Consequences

一个内部类型维护 staging 目录的创建和清理，减少重复实现。语音模型的特有业务仍留在模型模块；后续路径型发布逻辑保持原状，不能把本次抽取视为对它们的额外安全保证。

清理责任与成功创建绑定，失败的创建不再误删已有目录。实例需要保存这一状态；成功创建后的析构清理仍按原来的父目录句柄或路径执行。

## Verification

创建失败的回归先在旧析构逻辑下复现哨兵文件被删除，再验证修复后保留文件；已有成功创建后的清理测试同步改用 `create()`。本机完整 `client-core` 测试、`clippy --all-targets`、Android 35 专用 AVD 上的完整 `client-core` 测试和 macOS 原生宿主构建均通过。

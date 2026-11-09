# Agent Note: 修复 Nightly 发布链的代次资源目录和鸿蒙枚举名

Status: implemented

## Problem

2026-11-08 的 Nightly（run `42526762836`）在两个独立步骤失败。HarmonyOS 的 ArkTS 编译器只提供大写的 `HitTestMode.BLOCK_HIERARCHY`，键盘调整按钮修复使用了不存在的 `BlockHierarchy`。Windows 的桌面资源 job 把 `install_resources` 返回的资源代次目录误当成资源根目录比较；随后上传的 artifact 还保留了一层代次目录，下载到 release job 后不能直接作为 `MSIME_DESKTOP_RESOURCES` 使用。

## Decision

HarmonyOS 继续使用平台定义的 `HitTestMode.BLOCK_HIERARCHY`，只修正枚举成员名，不改变触摸行为。Windows 资源 job 把 `install_resources` 返回值当作已校验的代次目录验证，再把代次目录中的文件复制到独立的 artifact 根目录。release job 下载后得到的目录直接包含资源文件，与原来各 edition job 使用的 `MSIME_DESKTOP_RESOURCES` 语义一致。

## Alternatives considered

- **让 `install_resources` 改为返回资源根目录**：最少改工作流，但会破坏所有现有调用方依赖“返回代次目录”的契约，也会让构建阶段失去对具体代次的明确绑定，因此不采用。
- **让 release job 继续适配 artifact 内的代次子目录**：可以避免复制文件，但每个 edition 都要重新解析子目录，且容易把 artifact 布局误传给打包脚本；保持下载目录直接可用更清晰，因此不采用。

## Consequences

Nightly 的 HarmonyOS 编译恢复，Windows 资源只下载和校验一次的优化仍然保留，release 矩阵收到的 artifact 不再依赖随机代次目录名。资源 job 会在 runner 上额外复制一份已校验文件，代价是短暂增加磁盘占用；复制不改变资源字节，也不绕过 `ResourceStore` 的摘要校验。

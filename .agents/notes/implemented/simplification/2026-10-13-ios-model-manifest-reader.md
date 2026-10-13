# Agent Note: iOS 本地模型 manifest 共用有界读取

Status: implemented

## Problem

本地语音模型的 manifest 自己重复实现了 descriptor、普通文件检查和带上限读取，而词表等模型文本已经使用 `BoundedFileReader`。两套安全边界容易在修复时产生差异。

## Decision

`LocalSpeechModelManifest` 改用 `BoundedFileReader.read` 读取 manifest，继续限制在 `maximumManifestBytes` 内。reader 明确标记打开失败，让模型层保留“无法打开 manifest”与“manifest 损坏或读取失败”两种领域错误文案。模型载荷文件仍由各自调用方按需读取，没有改成整文件加载。

## Alternatives considered

- 保留 manifest 的专用 `open` 和 `FileHandle` 读取：会继续复制有界读取、特殊文件和符号链接防护，后续修复容易遗漏此处。
- 让 `BoundedFileReader` 负责 JSON 解码：会把本地模型的领域字段校验和错误文案耦合进通用文件工具。

## Consequences

iOS 本地模型描述文件、词表和其他受限文本文件共用拒绝符号链接、特殊文件及超限读取的底层实现。manifest 的 JSON 解码和模型类型错误仍由领域层处理，现有超大文件和符号链接测试继续覆盖这些边界。

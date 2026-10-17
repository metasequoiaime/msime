# Agent Note: 九键刷新复用已构造的排名上下文前缀

Status: implemented

## Problem

`NineKeySession::refresh` 已经构造 `locked_key` 用于路径查询和候选筛选，但固定位置应用随后调用 `ranking_context()`，再次把 `self.locked` 用撇号拼接成同一段前缀。这个短命字符串只为一次排名上下文使用，落在九键刷新热路径上。

## Decision

固定位置应用直接用已有 `locked_key` 格式化 `nine-key:<digits>:<locked_key>`，保留 `ranking_context()` 给调整词频和其它独立调用使用。排名上下文文本、个人词频分组、固定位置顺序和缓存键保持不变。

## Historical audit

已检索九键排名上下文、锁定拼写、候选固定位置和快照前缀笔记。[九键快照前缀长度复用](2026-10-17-nine-key-snapshot-prefix-offset.md)只处理快照内的替换偏移，[九键表情读法前缀复用](2026-10-17-nine-key-expressive-prefix-borrow.md)只处理表情读法字符串；本片复用刷新函数已经存在的排名前缀，不改变两者的所有权边界。既有 `ranking_context()` 测试继续覆盖独立构造结果。

## Alternatives considered

- **继续调用 `ranking_context()`**：实现最直接，但在刷新内重复拼接已存在的锁定前缀。
- **让 `ranking_context()` 接受可选前缀**：会把刷新专用的借用生命周期带进所有调用方，接口更复杂；直接在唯一刷新调用点格式化更清晰。
- **在会话中缓存完整上下文**：需要在数字和锁定拼写变化时维护失效关系；本片只复用当前刷新已经验证过的字符串。

## Verification

新增带锁定拼写的刷新分配回归。旧实现固定输入 `64426`、锁定 `ni` 时为 107 次分配，按 106 次上限失败；直接使用 `locked_key` 后通过。九键模块 142 项、完整引擎库 2263 项（40 项既有忽略）、golden 31 项、runtime 318 项、Clippy、格式、差异和笔记校验通过。`scripts/verify-local.sh --quick` 的 Rust workspace、Android target、WASM、Android Java、macOS、共享 Apple bridge 和 Windows 本机检查完成；quick 最终被当前 develop 基线的 Android JNI 清单失败阻断（`client_jni.cpp` 有而 `verify-native.sh` 清单没有 `localSpeechSpeakingRaw`、`voiceNormalizeNumbersRaw`），本项未修改 Android 文件。共享 Apple bridge 单独复现构建通过；Linux Docker、MinGW、HarmonyOS 真工程和 pipe-only 按环境跳过。

## Consequences

带锁定拼写的刷新少一次排名上下文前缀分配；其它 `ranking_context()` 调用仍按原接口独立拥有字符串。该收益局限于九键刷新中的固定位置路径，不推导整体九键延迟目标已经达标。

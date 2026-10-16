# Agent Note: 九键读音行复用输出缓冲

Status: implemented

## Problem

`NineKeySession::refresh` 已经清空并保留 `reading` 字符串，但随后通过 `reading_for` 构造新的 `String` 再赋回，热刷新仍会为当前首候选的读音行重新分配存储。简拼首字母路径还会临时构造 `Vec` 和连接字符串。

## Decision

增加写入已有字符串的私有 `write_reading`，由刷新路径先取出 `self.reading` 的容量，直接写入并放回；测试和其它只读调用继续通过小包装 `reading_for` 获得拥有型字符串。普通拼音、九键双拼、简拼首字母、十四键英文和空/模糊/非拼音来源的输出规则保持不变；读音行末尾的分隔符直接截断，不再创建中间字符串。

## Historical audit

已检索九键候选、读音显示、候选缓冲和英文读音笔记。九键英文数字借用只处理 `digits` 查询键的临时副本；九键规范读音和候选拼音缓冲仍负责候选字段的拥有性。本项只处理最终 `nine_key_reading` 输出，不能改变候选行的 `pinyin` 或 `canonical_pinyin`。

## Alternatives considered

- **继续返回新的 `String`**：实现最简单，但浪费会话已经保留的读音容量。
- **把读音改为借用 `&str`**：快照需要独立拥有文本，且简拼和未完成尾段需要拼接，无法覆盖所有分支。
- **删除测试包装并让所有调用传输出缓冲**：可以减少一层函数，但会扩大测试和内部调用的改动面；包装只用于需要拥有结果的测试与纯函数式断言。

## Verification

新增热刷新分配回归先在旧实现失败（601 次分配），生产改动后通过 `<601` 门禁；九键模块 133 项测试通过。冻结的 dominated projection 分配对照同步扣除本项独立节省的一次读音缓冲分配，仍保留重复前缀字段复制的精确预算。完整引擎 2223 项单测、golden 31 项、Clippy、格式、笔记和差异检查通过；`MSIME_BUILD_JOBS=4 CARGO_BUILD_JOBS=4 bash scripts/verify-local.sh --quick` 通过，Rust、WASM、Android Java、macOS CMake 与 shared Apple bridge 均完成，缺少 Docker、MinGW 和 HarmonyOS 前置的阶段按脚本跳过。

## Consequences

重复刷新复用 `reading` 的已有容量，简拼首字母路径也直接写入目标字符串；读音文本、候选顺序、快照所有权和公开接口不变。首次遇到更长读音仍可能扩容，读音缓冲收益不能单独推断九键 16 ms 端到端预算达标。

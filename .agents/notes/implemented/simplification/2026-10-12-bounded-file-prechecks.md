# Agent Note: 统一有界文件读取的大小预检

Status: implemented

## Problem

六个 `client-core` 存储读取路径在调用 `bounded_io::read_bounded_file` 前重复读取文件元数据并判断长度。公共读取函数本身已经执行同一检查，并限制实际读取字节数。两次预检会增加文件系统调用，也让大小限制规则分散在调用方。

## Decision

移除词汇进度、社区资源库、常用语、词典集合、键盘皮肤试用记录及自定义皮肤库的重复元数据预检。继续通过 `read_bounded_file` 的 `too_large` 回调返回各模块原有的领域错误；普通 I/O 错误仍由 `From<io::Error>` 转换。

## Alternatives considered

- 在各调用方保留预检：和公共函数重复，无法减少规则散落和额外文件系统调用。
- 移除公共函数的预检：会让未预检的其他调用方失去文件长度检查。

## Consequences

六处读取共享公共函数的一次元数据检查和实际字节上限；缺失文件、解析及领域验证流程不变。`cargo test -p msime-client-core --lib`（1348 通过、1 忽略）、`cargo clippy -p msime-client-core --all-targets -- -D warnings`、格式与 notes 校验通过。本地 quick 门禁跑过契约、工作区、Android 目标和 Java 宿主阶段；macOS 原生构建时因磁盘剩余空间不足而中止。

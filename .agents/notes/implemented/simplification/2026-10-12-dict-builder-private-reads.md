# Agent Note: 词库构建器共用只读普通文件入口

Status: implemented

## Problem

文本源、ngram 语料、下载输入和只读 SQLite 输入各自复制了一套文件打开选项与普通文件检查。若其中一处修改了符号链接或 FIFO 的处理，其他入口容易落后。

## Decision

`crates/dict-builder/src/private_file.rs` 负责打开只读文件：Unix 使用 `O_NOFOLLOW | O_CLOEXEC | O_NONBLOCK`，Windows 使用 `FILE_FLAG_OPEN_REPARSE_POINT`，然后在已打开的句柄上确认它是普通文件。调用方传入原有的非普通文件错误文案，保留现有诊断。

`scripts/test-dict-builder-private-reader.py` 同时检查下载输入的调用关系和共用入口的安全选项，避免只盯着旧函数体而误报。

SQLite 创建数据库的入口继续独立处理，因为它需要独占创建、读写权限，且不使用 `O_NONBLOCK`。只读 SQLite 连接仍先解析和检查路径，再使用共用入口确认普通文件，最后按原有方式交给 SQLite 打开。

## Alternatives considered

- 把 SQLite 创建入口也并入同一方法，可以减少更多重复；但创建与只读的打开标志、返回值和后续操作均不同，统一参数会掩盖安全策略差异。
- 保留每个入口各自的复制代码，局部诊断更直接；但后续安全选项调整必须同步四处，且现有测试只分别覆盖部分特殊文件类型。

## Consequences

安全选项和句柄类型检查只有一个维护点，原有路径策略与诊断保持不变。此方法只检查叶文件，父目录的信任边界仍由各调用方负责；SQLite 连接的第二次打开仍沿用现有逻辑。

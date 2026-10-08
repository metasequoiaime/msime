# Agent Note: 本地语音模型拒绝硬链接文件

Status: implemented

## Problem

本地语音识别器只检查 `msime-model.json` 和模型文件是不是普通路径文件。符号链接或特殊文件会被拒绝，但指向其它 inode 的硬链接仍会通过检查。攻击者或被替换的模型目录因此可以让识别器把不属于模型的私有文件当作清单、词表或 ONNX 模型读取。

## Decision

清单在打开后的句柄上要求普通文件且只有一个硬链接；Windows 句柄同时检查 `NumberOfLinks`。模型清单的路径检查和每个清单引用的模型文件也要求普通单硬链接文件。POSIX 词表读取再做一次同样的句柄检查，形成分层边界。

## Alternatives considered

- 只检查 `O_NOFOLLOW` 或 `symlink_status`：硬链接不是符号链接，仍会把外部 inode 当作模型文件。
- 只在识别运行库打开后检查路径：运行库已经自行按路径读取，无法保证检查对象和使用对象一致。
- 只依赖安装器拒绝硬链接：安装目录可被用户或其它进程替换，读取入口仍需独立执行边界检查。

## Consequences

正常安装的模型文件都是单硬链接普通文件，行为不变。符号链接祖先、符号链接文件、FIFO、设备文件和硬链接文件均不会被交给识别运行库。

## Verification

- `shared/voice/tests/local_asr.cpp` 覆盖清单硬链接被拒绝；旧实现上的断言按预期失败，修复后通过。
- `scripts/test-local-asr-file-readers.py` 检查 POSIX/Windows 读取器及模型文件边界。
- `shared-voice-local-asr` CTest 通过。

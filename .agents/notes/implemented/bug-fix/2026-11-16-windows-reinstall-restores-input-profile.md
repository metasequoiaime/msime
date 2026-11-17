# Agent Note: Windows 重装恢复当前用户输入法 profile

Status: implemented

## Problem

Windows 的 TSF 安装注册只调用 `ITfInputProcessorProfileMgr::RegisterProfile`。用户从系统键盘列表删除水杉后，这个调用只能恢复系统层 profile，不能把它重新加入当前用户的启用列表；覆盖安装或全新安装因此仍然无法切换到水杉输入法。

## Decision

安装完成后，Inno Setup 以启动安装器的原用户身份启动已经安装的原生设置宿主，并传入专用的 `--repair-input-method` 参数。两个 Windows 设置宿主在普通设置路由和管理员操作之前识别该参数，直接调用系统 `input.dll` 的 `InstallLayoutOrTip`，使用共享版本表生成的 `langid:CLSID+profile GUID`。这一步只恢复当前用户的键盘列表，不改变默认输入法。

失败只写安装日志，不回滚已经成功复制的程序；下一次启动设置应用仍可由维护入口显示缺失状态。安装器不会让管理员进程直接写用户 profile，避免把条目写到错误的用户配置。

## Alternatives considered

- **只再次调用 `RegisterProfile`**：它是必要的系统注册步骤，但不修改当前用户的启用列表，无法覆盖报告的删除后重装场景，拒绝。
- **安装器直接调用 `InstallLayoutOrTip`**：管理员安装器的用户上下文可能不是启动安装器的普通用户，可能把条目写入错误的 profile；拒绝。
- **强制设为默认输入法**：可以让输入法立即出现，但会改变用户当前选择，超出 issue 范围；拒绝。

## Consequences

- 删除水杉后重新安装时，安装成功路径会重新启用当前用户的水杉 profile；普通升级和首次安装也经过同一条幂等调用。
- 需要随安装包提供原生设置宿主；调用失败时不阻塞安装，日志保留原因，用户仍可从设置页的“去添加”入口手动恢复。
- 这只覆盖安装器启动的用户会话；其他用户会话仍需各自安装或在其设置中添加。真实 Windows 系统设置验收待可用的 Windows 编辑器环境，交叉编译和契约测试不能替代它。

## Verification

- `windows-installer-launch` 契约测试：编译并运行通过，确认安装器以原用户身份调用 `--repair-input-method`，且在迁移清理之后完成修复。
- `python3 scripts/test-windows-native-run.py`：完整检出下 198 项通过，2 项宿主差异排除，116 项需要 Windows 构建，20 项非测试。原稀疏检出缺头文件的失败不计为通过。
- MinGW `x86_64-w64-mingw32-g++ -std=c++17 -Wall -Wextra -Werror -fsyntax-only`：`InputMethodSetup.h` 通过。
- Windows CI 暴露 Win7 入口遗漏 `InputMethodSetup.h`，现补齐直接依赖；MinGW 对实际 `win7/main.cpp` 的 `-fsyntax-only` 检查通过，使用产品的 edition、Unicode 和 `NOMINMAX` 定义。已有 `GameCompatProfile.h` 的结构体部分初始化仍有告警；MSVC 完整构建由更新后的 PR 检查覆盖，不将交叉语法检查表述为链接或系统验收。

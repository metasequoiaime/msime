# Agent Note: macOS 默认英文时的起始标点跟随模式

Status: implemented

## Problem

默认输入模式为英文、标点锁定为跟随且保存的起始标点为中文时，新应用首次输入 `,.` 会输出中文标点。切到中文再切回英文后恢复正常。`setEnglishInputMode:` 会在模式发生变化时同步标点，但直接以英文启动没有经过这条转换；`runtimeChinesePunctuation` 缺少应用内覆盖时直接回落到 `chinesePunctuation`。

## Decision

只修改 `runtimeChinesePunctuation` 缺少运行时覆盖时的回退：英文模式返回英文标点，中文模式继续使用保存的 `chinesePunctuation`。固定中英文标点仍优先于运行时覆盖；用户在英文模式中手动打开中文标点后仍按应用保留。不改共享偏好、出厂默认值和中英文模式记忆范围。

## Alternatives considered

关闭保存的中文标点能绕过启动问题，且不需要改二进制，但会让中文模式也失去原来的中文标点，不能满足使用需求。

在每次焦点激活时强制同步标点能覆盖启动路径，但会清掉同一应用中用户手动选择的标点覆盖。让 getter 仅在无覆盖时跟随模式更小，也覆盖输入源重置后的读取。

## Testing

- 默认英文直接进入新应用时输出英文标点，无需来回切换模式。
- 中文模式仍使用保存的起始标点，固定中英文标点仍优先。
- 手动覆盖按应用保留；清除运行时状态后重新跟随当前模式。
- `ctest --test-dir target/macos-isolated -R '^shortcut-english-punctuation$' --output-on-failure`：修复前在默认英文标点断言失败，修复后通过。
- `cmake --build target/macos-isolated --target MSIMEClientInputMethod --parallel 6`：arm64 原生拼音 bundle 构建通过。
- 完整 `shortcut` 测试在修复前即因候选窗边框颜色断言失败；独立标点入口避免该基线问题掩盖本回归。
- 关联的标点空格、标点影子状态、成对标点、局部模式偏好、外观默认值及标点专项共六项 CTest 全部通过。
- 补齐 Android API 35 和 Build Tools 35.0.0 后，以 Python 3.12、Node 24、`TZ=UTC`、构建并发 6 重跑 `bash scripts/verify-local.sh --quick`，退出码 0，报告 `quick check passed`；Rust workspace、macOS 构建、Android host Java、契约检查和共享 Apple bridge 均通过。缺少 NDK、MinGW、Docker、HarmonyOS 等环境的阶段按脚本规则跳过。
- 默认本机时区下 Android 的 `TimestampDisplayPolicySmoke` 在极端时间戳断言失败；从补丁前提交 `5a622be` 单独取出同一测试和实现可复现，设置 `TZ=UTC` 后通过。本补丁不修改该测试、实现或系统时区。
- macOS 27.0.1 / Apple Silicon 上以本地 ad-hoc 签名安装原生拼音试用包，用户在 Calendar 中确认默认英文直接输出半角标点，随后确认正常使用、修复有效。该验证针对原生输入法，不是完整发布安装包。

## Consequences

以往依赖默认英文却自动使用中文标点的行为会变化；显式标点锁定和手动覆盖继续提供该能力。本次同时完成专项回归和用户实机验收；本地临时签名试用不代表 Developer ID 签名、公证或完整发布包验证。

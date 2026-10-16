# Agent Note: 云剪贴板中继偏好使用公共模型

Status: implemented

## Problem

`Preferences` 内联声明 `cloud_clipboard_relay` 的地址和令牌字段，中继逻辑文件再从总偏好反推 `CloudClipboardRelayPreferences`。设置区、写回逻辑和总偏好没有共同的纯类型来源，字段变化可能只更新其中一处。

## Decision

在无依赖的 `cloud-clipboard-relay-preferences.ts` 定义 `CloudClipboardRelayPreferences`，总偏好直接组合该模型，中继逻辑继续从旧路径转导出并使用它。只移动类型归属，保持偏好键名、字段形状、清除时删除键的行为和中继校验规则不变。

## Existing notes audit

检索活跃笔记中的 `CloudClipboardRelayPreferences`、`cloud_clipboard_relay` 和中继设置，已有中继协议、Android 接入及令牌输入脱敏笔记描述运行时行为与安全边界，不拥有本次共享 TypeScript 类型的归属。

## Alternatives considered

- 保留 `Preferences` 内联类型并让中继文件继续使用 `NonNullable`：改动最少，但总偏好与中继设置仍没有共同的字段来源。
- 让纯模型文件从 `Preferences` 派生类型：可以避免重复写字段，但会让总偏好反向依赖自身的投影，无法成为持久化契约的叶子模型。
- 只把类型转移到中继逻辑文件：设置区仍依赖带校验和偏好操作的模块，纯类型消费者无法脱离运行时代码使用，因此单独放置无依赖模型。

## Consequences

- `Preferences` 和云剪贴板中继逻辑共用 `CloudClipboardRelayPreferences`；旧的中继模块导出路径保持，设置组件和写回函数调用方式不变。
- 中继偏好仍是 `{ url, token }`，缺省时不写键，地址和令牌校验、令牌脱敏、路由选择与持久化行为不变。
- 类型兼容测试锁定公共模型与 `Preferences` 字段一致；云剪贴板设置测试、类型检查、构建、lint、笔记检查和 quick 门禁覆盖迁移。

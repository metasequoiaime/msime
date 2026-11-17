# Agent Note: Android 对话换号后重新读取默认模型

Status: implemented

## Problem

#11467 报告 Android 已登录账号进入「AI 对话」仍固定返回失败。对话页从 `chat_models` 读取服务端的 `defaultModel`，并把它缓存到 Fragment 的 `model` 字段。账号会话变化时页面会清空对话、取消请求并重新绑定账号，但原实现没有清空 `model`，所以页面可能把旧账号的模型 ID 带给新账号；服务端拒绝该模型时，用户只能看到对话失败。

## Decision

账号会话变化时把 `model` 置空。下一次发送会按现有流程重新读取当前账号的 `chat_models.defaultModel`，不改变模型由服务端决定的产品规则，也不改变正常同一账号内的缓存行为。

## Alternatives considered

- 每次发送都重新请求 `chat_models`：能避免缓存过期，但会给每条消息增加一次网络请求，超出本 issue 的账号切换缺陷范围。
- 在服务端接受旧模型或自动改用第一个模型：会掩盖客户端会话绑定错误，并改变服务端模型选择策略。

## Consequences

换号、退出后重新登录或页面检测到账号变化后，下一次对话会使用新账号的默认模型；同一账号连续发送仍复用已读取的模型。没有 Android 真机时，设备交互验收保留在 PR 验证段，不阻塞源码和回归契约验证。

## Verification

- `python3 scripts/test-android-keyboard-tryout.py`
- `git diff --check`
- `pnpm run verify-notes`
- `bash scripts/verify-local.sh --quick`：因本机可用磁盘不足 50 GiB，未启动新的完整构建；PR 记录该限制并交由 CI 编译验证。

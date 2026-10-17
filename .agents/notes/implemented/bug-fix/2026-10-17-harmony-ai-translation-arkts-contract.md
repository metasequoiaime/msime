# Agent Note: HarmonyOS AI 候选翻译 ArkTS 契约

Status: implemented

## Problem

候选翻译接入后，HarmonyOS 原生 NAPI 已注册 `aiTranslationHttpRequest` 和 `parseAiTranslationResponse`，但 `libmsimeclient/index.d.ts` 没有公开这两个函数。键盘会话还以内联对象字面量构造 AI 翻译请求；ArkTS 真编译要求这类跨边界请求使用显式接口，因此 HAP 编译在类型检查阶段失败。共享逻辑测试和普通 TypeScript 检查没有覆盖 DevEco 的这些限制。

## Decision

HarmonyOS 的 NAPI 类型声明与 `client_napi.cpp` 的注册表保持一一对应，补上 AI 翻译请求和响应解析函数的声明。键盘会话为 AI 翻译请求的输入和外层文档定义显式接口，并在调用 NAPI 前使用该接口构造请求。请求字段沿用共享层既有的 `config`、`text`、`source_language` 和 `target_language`，不新增运行时字段或另一个翻译协议。

## Alternatives considered

- **只在 `.d.ts` 里补函数声明**：能消除 NAPI 符号缺失，但 ArkTS 仍会拒绝请求处的未声明对象字面量，HAP 仍然无法编译，因此不采用。
- **把请求改成 `JSON.stringify` 的字符串拼接**：可能绕过部分静态检查，却会失去字段类型约束并让请求格式偏离其他翻译服务，不能解决跨文件契约漂移，因此不采用。
- **让 ArkTS 类型声明省略这两个函数，继续依赖运行时导出**：运行时虽然可能找到符号，但编译期无法保证宿主和键盘使用同一 ABI，错误会推迟到设备上，因此不采用。

## Consequences

HarmonyOS 真正的 ArkTS/HAP 编译现在能检查并接受 AI 候选翻译路径，NAPI 类型文件和原生注册表的公开面保持同步。新增接口只描述已有请求，不改变翻译服务优先级、凭据处理、网络策略或缓存行为。以后新增 NAPI 导出仍需同时更新注册表和 `.d.ts`，而共享逻辑套件单独通过不能替代 `hvigorw assembleHap` 验证。

## Verification

在本 PR 中运行 `bash platforms/harmony/tests/run.sh`（4387 个键盘断言、9 个共享沙箱检查、38 个共享状态镜像检查）和 `ohpm install && hvigorw assembleHap --no-daemon`；后者的 ArkTS 编译和未签名 HAP 打包成功。`bash scripts/verify-local.sh --quick`、`pnpm run verify-notes` 与 `git diff --check` 也通过。quick 门禁结束前清理了本地 HAP 产物，因此它按设计跳过了需要 staged 原生库的重复 ArkTS 阶段；实际 HAP 命令已在同一提交上单独运行。修改前的 CI 日志曾在同一调用点报告两个缺失导出和两个未声明对象字面量错误。

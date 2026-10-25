# Agent Note: HarmonyOS 签名使用未压缩 HAP 输入

Status: implemented

## Problem

资源压缩进入 HarmonyOS 发布流程后，工作流把 DEFLATE 过的未签名 HAP 直接传给华为 `hap-sign-tool.jar`。资源压缩步骤本身成功，但签名工具以 Java 退出码 1 失败，导致 `0.2.1` 发布无法生成正式包。

## Decision

工作流同时保留两条路径：`steps.hap.outputs.path` 是供未发布构建 artifact 使用的压缩未签名 HAP，`steps.hap.outputs.input` 指向 hvigor 生成的原始未压缩 HAP。正式签名步骤只读取 `input`。`sign-release.py` 继续在签名完成后压缩签名结果，并用 `verify-app` 复验压缩包，再生成用户 HAP 和应用市场 APP。

## Alternatives considered

- **取消资源压缩**：能够绕开签名工具限制，但会把已验证的约 40% 下载体积收益全部丢掉，因此不采用。
- **修改签名工具参数让它接受压缩输入**：工具由华为提供，无法在仓库内可靠修复，也不能以猜测参数替代签名验证，因此不采用。
- **签名后直接改 ZIP 而不复验**：可能产生签名失效的下载包；现有 `verify-app` 复验是发布安全边界，必须保留。

## Consequences

华为签名工具继续处理它能接受的原始 HAP，未发布 artifact 仍然是小体积压缩包，正式发布包仍由签名后压缩路径生成。发布流程多保留一个原始中间文件，上传的用户 HAP 和 APP 不变。若未来签名工具开始接受压缩输入，仍可保留此分离路径，因为它明确区分构建 artifact 与签名输入。

## Verification

- 失败 job 日志确认压缩步骤成功，签名工具在压缩输入上退出码 1。
- `scripts/test-harmony-release-signing.py` 增加工作流输入路径契约，防止再次把 `path` 传给签名。
- HarmonyOS 发布工作流应在 CI 中重新运行，确认签名、压缩后的 `verify-app` 和应用市场打包全部通过。

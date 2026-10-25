# Agent Note: HarmonyOS 资源压缩在签名前完成

Status: implemented

## Problem

Hvigor 把 `resources/` 下的词库、模型、设置页和音效按 ZIP 存储方式写入 HAP。`harmony-v0.2.0` 的包为 328,655,000 字节，约 313.4 MB；这些资源可以无损压缩。此前引入压缩时，签名脚本又对签名后的包执行 ZIP 重写，导致 `0.2.1` 发布失败。

运行 `40214456864` 报 `No Hap Signing Block before ZIP Central Directory`。本机用合成证书和华为 SDK 复现：未签名 HAP 先压缩，再 `sign-app`、`verify-app` 可以成功；已签名 HAP 经压缩器重写后，`verify-app` 报相同错误。HAP 签名块在 ZIP 中央目录前，不是 ZIP 条目；`zipfile` 重写会丢弃它。此前仅凭 Java 退出码推断“签名工具不接受 DEFLATE 资源”没有依据。

## Decision

发布工作流只取指定 product 的本次 Hvigor 产物，在 `Locate the packaged HAP` 中调用 `scripts/compress_harmony_hap.py`，把输出通过 `steps.hap.outputs.path` 传给签名脚本。压缩器仅对原本存储的 `resources/` 非目录条目使用 DEFLATE，保留 ZIP 注释、条目内容和元数据，以临时文件原子替换目标；小于 1024 字节的资源保持原样。

`libs/`、`ets/`、`module.json`、`pack.info`、`resources.index` 等其他条目保留原压缩方式。签名工具会处理原生库和代码的布局、代码完整性索引；签名完成后只允许读取验证或逐字节复制 HAP，不能再次重写 ZIP。压缩器识别中央目录前的 HAP V2/V3 签名块魔数，拒绝已签名输入，并在失败时保留已有目标文件。

`sign-release.py` 对压缩后的未签名 HAP 签名并验签，检查嵌入 Profile、资源内容和 `.pages.info`，然后用 `--replace-pack-info false` 生成应用市场 APP，核对内层 HAP 与已验签文件逐字节相同。正式发布只上传签名后的 HAP；只构建的 artifact 使用压缩后的未签名 HAP。保留[发布凭据校验](2026-10-25-harmony-release-signing.md)的全部限制，不改证书或 Profile 的授权契约。

工具失败时沿用[脱敏诊断](2026-10-26-harmony-signing-error-diagnostics.md)，增加“签名 HAP”“验证 HAP 签名”“打包应用市场 APP”阶段名。发布工作流安装 SDK 后、编译原生宿主前，用合成材料运行真实工具回归；没有 SDK 的普通契约检查明确跳过该用例，其余校验照常执行。

## Alternatives considered

- **签名后压缩并再验签**：可以保证下载包的压缩率，不受签名器重写 ZIP 的影响；但 ZIP 重写会丢掉签名块，即使手工保留旧块，其内容摘要也已失效，因此不采用。
- **保留原始 HAP 作为签名输入并取消压缩**：能恢复 `0.2.0` 已通过的路径；但真实工具已证明 DEFLATE 资源可签名且会保留，取消压缩会无谓丢失约 40% 的体积收益，因此不采用。
- **移除可选词库或模型**：可以直接缩小包体，但会移除五笔、日文、手写、语言词库或背单词能力，超出缩小包体的范围。
- **首次运行时下载或解压资源**：可以进一步缩小安装包，但引入联网、离线首次启动、校验和资源生命周期的新契约；本次只改变包的压缩方式和签名顺序。

## Consequences

公开 `0.2.0` 包的资源压缩实验从 328,655,000 字节降至 197,712,995 字节，减少约 40%；这是压缩效果证据，不代表那个经过改写的旧签名包仍有效。正确发布产物必须由压缩后的输入重新签名，并通过华为验签。资源内容和路径保持不变，运行时不增加资源下载或额外解压流程；发布增加几十秒压缩开销，设备端对 DEFLATE 资源的读取仍需真实设备验收。

合成 `0.2.1` HAP、三级 EC 证书链、PKCS#12 和签名 Profile 运行整个 `sign` 流程，通过签名、代码完整性验签、APP 打包及最终文件独立验签；资源保持 DEFLATE，原生库条目保持存储，APP 内层 HAP 字节相同。测试还覆盖压缩器拒绝真实签名包、V2/V3 布局标记和失败时不覆盖目标。Profile 的格式版本 `2.0.0` 与应用版本 `0.2.1` 独立，不额外限制它们相等。

本机不新建完整构建树：磁盘可用空间低于 50 GB。完整产品的正式证书签名、发布和真机验收仍待后续工作流与设备验证，合成测试不代表应用市场接受或真机安装通过。

## 检索审计

原 `2026-10-26-harmony-hap-resource-compression.md` 与 `2026-10-26-harmony-signing-input.md` 被本篇完整吸收并删除：保留资源范围、体积实验、未签名 artifact 和市场包要求，纠正签名后压缩与“不接受 DEFLATE”的错误结论；检索未发现入站链接。[脱敏诊断](2026-10-26-harmony-signing-error-diagnostics.md)与[正式发布授权](2026-10-25-harmony-release-signing.md)各有独立边界，保留。

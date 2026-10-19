# Agent Note: 社区画布资源按服务端能力上传

Status: implemented

## Problem

上游夜樱包包含四张 PNG/APNG/GIF 资源，共享社区代码仍限制三张 PNG/JPEG，并在本地拒绝 canvas 与 wechat_scroll。[云端 PR #956](https://github.com/metasequoiaime/msime-cloud/pull/956) 扩展图片协议，但不能假定用户配置的服务端已部署它。

## Decision

打包、校验与安装在共享 client-services 支持最多 37 张有界 PNG/JPEG/APNG/GIF/WebP。预览仍限 PNG/JPEG，所有图片完整解码，动画逐帧按画布累计。上传新增方言或扩展资源前从同源列表响应探测 X-MSIME-Candidate-Resources: canvas-v1；没有明确能力头时拒绝扩展上传，保留普通旧包上传。列表、详情、下载与同步声明 resources=canvas-v1，旧部署忽略该查询键时照常返回旧包。滚动升级的拒绝作为正常请求失败处理，不无限重试。

## Related notes audit

- [共享清单对齐](2026-10-19-windows-skin-schema-parity.md) 记录了本地拒绝新基底以保护旧服务器，本次以能力探测替代无条件拦截。
- 显式整库同步笔记约束触发时机，本次只修改资源通路，沿用手动触发。
- 目录句柄与原子安装笔记约束写入安全，本次沿用原有暂存与交换路径。

## Alternatives considered

直接删除上传拦截改动较少，但会把新包发给尚未升级的服务器。按域名或版本号判断可少一次探测，但用户可配置自托管服务且滚动升级混有多个版本，因此要求明确能力头。

## Testing

合成五资源动画包覆盖打包、原子安装、目录重扫与字节往返；旧部署和未知能力头只收到 GET 探测，不接收扩展上传；普通旧包保持一次 POST/PUT。新增替换接口探测顺序、损坏后续帧不覆盖已安装包、帧数、像素与预览格式边界测试。完整服务测试、clippy、quick 和 Android 设备结果随验收记录。

## Consequences

MCP 创建与社区安装共用这些资源规则；工具 schema、生成提示词和共享/Windows 发布说明同步更新。MCP 不持有账号会话，上传由宿主触发并协商。逐帧完整校验有 CPU 成本，受尺寸、帧数、内存与总像素约束。云端与客户端需配套部署；通过资源往返不能宣称 Linux 等宿主已完整绘制画布。

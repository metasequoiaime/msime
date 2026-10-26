# Agent Note: 鸿蒙发布校验市场签名并自动提交华为公开测试

Status: implemented

## Problem

鸿蒙发布工作流原来直接上传未签名 HAP。真机调试使用限定设备的 debug Profile；把这种包标成正式版不能提供应用市场可用的产物。已有 release Profile 虽然包名正确，却不带输入法共享沙箱授权，安装成功也不能证明设置与统计能在两个进程间交换。

## Decision

公开发布必须经过 `platforms/harmony/sign-release.py`。构建前验证华为应用市场类型的 release Profile、有效期、包名、发布证书与私钥的匹配、共享沙箱 ID 和剪贴板受限权限；缺任何一项都失败，不降级成未签名或 debug 签名。

密钥库、密码、证书链和 Profile 放在仓库的 `HARMONY_RELEASE_*` secrets，仅显式请求签名或发布的步骤解码到临时目录，退出即删除。签名工具失败时不转发原始输出，以免凭据进入日志。签名任务继续在 GitHub 托管 runner 上运行。只构建的 nightly 不读取这些 secrets，保留未签名产物。手动工作流可用 `sign=true, publish=false` 生成正式签名材料而不创建 GitHub release。额外指定 `source_release` 时，只下载匹配版本的已发布 HAP，用 `--signed-hap` 复验签名、Profile 和内容后直接复制，跳过产品编译与 HAP 重签；这样修复市场封装时仍与用户下载的发布 HAP 完全一致。

HAP 使用 release 模式完整构建，签名前拒绝调试标记，并核对包名、两个版本字段、原生库和必需资源；签名后用华为工具验证、比对嵌入 Profile，并逐项核对原包内容没有被改变。华为打包工具将通过验证的 HAP 与同次构建的 `pack.info` 组成市场提交 APP，然后对 APP 外层独立签名和验签，比对其嵌入 Profile，最后核对 APP 内的 HAP 与已签名包逐字节相同。APP 打包本身不会生成外层签名；只有内层 HAP 签名时，AGC 返回非法软件包 `991`。本机对原 `0.2.1` APP 复验报 `No Hap Signing Block before ZIP Central Directory`，而内层 HAP 验签成功，因此最终 HAP 与最终 APP 都必须独立验签。

GitHub 版本发布页只放已签名 HAP 和只列该 HAP 的 `SHA256SUMS`；应用市场提交 APP 放独立 workflow artifact。生成市场提交材料与完成应用市场审核是两个结果，不能把创建 GitHub release 报成市场上架成功。

APP 打包必须传 `--replace-pack-info false`：华为工具的默认行为会重打包内层 HAP，即使所有 ZIP 条目内容没有变化，也会丢掉 ZIP 外的签名块。以本机已签名包实测，默认命令生成的内层 HAP 不同；关闭替换后内层 HAP 与输入逐字节一致。末尾的完整字节比较专门守住这一点。

华为分发由 `platforms/harmony/appgallery-release.py` 通过官方 Connect API 完成，工作流显式选择 `none`、`upload` 或 `public-test`。默认 `none`，nightly 不读取正式密钥或市场凭据；选择市场上传会隐式要求正式签名。GitHub release 的 `publish` 开关保持独立。先把 APP 存入 artifact，再上传华为，平台拒绝资料不会丢掉构建产物。

市场鉴权优先使用开发者级服务账号 JSON，通过 OpenSSL 生成 PS256 JWT 直接调用 Connect API，密钥临时落盘后清理；兼容已有 API Client。市场凭据与签名证书分开管理；App ID 来自仓库变量，资料覆盖 JSON 来自 secret。构建前验证鉴权与本地配置。OBS 只使用上传接口返回的临时认证头，文件流式发送，不转发 AGC Bearer 或重定向；任何原始响应和上传地址都不进日志。只接受明确成功的响应；HarmonyOS 使用 `v3/package/compile/status`，不复用 Android 的 v2 状态接口。包解析失败、未知或有界轮询超时都停止提交。

公开测试类型固定 `testType=4`、`releaseType=6`，默认继承华为控制台已有资料，不生成假的资质或联系人。默认未来一天开始、持续 28 天，上限 30 天，分享链接开启、通知关闭、安装限额 10000；secret 可覆盖真实素材、资质和测试日期。`test_version_id` 支持更新已有公开测试草稿，`test_package_id` 支持跳过构建与上传后继续。包 ID 和草稿 ID 立即写入日志与步骤摘要；有副作用的请求不自动重试，网络结果不确定时先核对控制台再恢复。`submitted` 只表示审核提交成功，不代表通过审核或可安装。

## Alternatives considered

- 浏览器自动操作控制台：能直接沿用当前登录态，但页面异步变化和会话过期会使无人值守 CI 不稳定；采用官方 REST 合同与独立 API 凭据，首次真实资质仍由控制台管理。
- 复用真机已验证的 debug 签名包：能最快重装开发者自己的设备，但它限定设备、有调试授权，不能充当正式发布签名。
- 在已验证旧包里只换设置页后直接发布：节省本机空间且适合界面验收，但原生库不保证来自待发布提交，正式包必须由工作流从固定提交完整构建。
- 将签名路径和密码写入 `build-profile.json5`：Hvigor 可以直接签名，但会让本机路径与敏感配置混进源码；使用环境输入和临时目录保留无凭据的工程配置。

## Consequences

发布必须提前配置有效的四项 secrets，且 release Profile 带有 `group.1516753898738609107`；开发者需要在 AGC 更新 Profile，不能本地编辑授权字段。应用市场渠道的包由市场分发，GitHub 下载页不承诺绕过系统分发限制直接安装。

合成证书与合成 HAP 的回归覆盖 debug 或错误渠道、过期、缺共享组或 ACL、设备限制、证书不匹配、版本错误、缺库和缺资源，以及失败日志不泄密。合成材料还对最终 APP 独立验签并比对 Profile；完整签名、市场接受和真机输入法验收仍要使用真正的发布材料验证，单元测试不能替代。

离线合成测试覆盖服务账号 JWT 的真实 PS256 签名验签、API Client 兼容鉴权、应用权限预检、完整上传和公开测试提交、OBS 独立认证与真实 HTTP 流式上传、解析失败和未知状态、轮询超时、草稿与包恢复、提交失败不重试、日期边界、响应错误码与日志脱敏。真实 API 验收仍需要配置开发者级凭据；本次仓库没有这些 secrets，不能把合成测试说成已自动上架。此前经修复包的人工上传已获得华为合法性检测通过，但它不能替代自动化链路的真实验收。

检索审计：[共享沙箱](../architecture/2026-10-09-harmony-ime-shared-sandbox.md) 持有运行时数据交换与授权值，本篇不改变该决定；[六平台 nightly](2026-10-14-six-platform-nightly-builds.md) 的非 iOS nightly 不使用正式密钥规则保留。检索其余鸿蒙发布和证书记录无重复归属。

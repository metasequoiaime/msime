# Agent Note: 鸿蒙正式发布使用应用市场证书并校验共享沙箱授权

Status: implemented

## Problem

鸿蒙发布工作流原来直接上传未签名 HAP。真机调试使用限定设备的 debug Profile；把这种包标成正式版不能提供应用市场可用的产物。已有 release Profile 虽然包名正确，却不带输入法共享沙箱授权，安装成功也不能证明设置与统计能在两个进程间交换。

## Decision

公开发布必须经过 `platforms/harmony/sign-release.py`。构建前验证华为应用市场类型的 release Profile、有效期、包名、发布证书与私钥的匹配、共享沙箱 ID 和剪贴板受限权限；缺任何一项都失败，不降级成未签名或 debug 签名。

密钥库、密码、证书链和 Profile 放在仓库的 `HARMONY_RELEASE_*` secrets，仅发布步骤解码到临时目录，退出即删除。签名工具失败时不转发原始输出，以免凭据进入日志。签名任务继续在 GitHub 托管 runner 上运行。只构建的 nightly 不读取这些 secrets，保留未签名产物。

HAP 使用 release 模式完整构建，签名前拒绝调试标记，并核对包名、两个版本字段、原生库和必需资源；签名后用华为工具验证、比对嵌入 Profile，并逐项核对原包内容没有被改变。华为打包工具将通过验证的 HAP 与同次构建的 `pack.info` 组成市场提交 APP，随后核对 APP 内的 HAP 与已签名包逐字节相同。

GitHub 版本发布页只放已签名 HAP 和只列该 HAP 的 `SHA256SUMS`；应用市场提交 APP 放独立 workflow artifact。生成市场提交材料与完成应用市场审核是两个结果，不能把创建 GitHub release 报成市场上架成功。

APP 打包必须传 `--replace-pack-info false`：华为工具的默认行为会重打包内层 HAP，即使所有 ZIP 条目内容没有变化，也会丢掉 ZIP 外的签名块。以本机已签名包实测，默认命令生成的内层 HAP 不同；关闭替换后内层 HAP 与输入逐字节一致。末尾的完整字节比较专门守住这一点。

## Alternatives considered

- 复用真机已验证的 debug 签名包：能最快重装开发者自己的设备，但它限定设备、有调试授权，不能充当正式发布签名。
- 在已验证旧包里只换设置页后直接发布：节省本机空间且适合界面验收，但原生库不保证来自待发布提交，正式包必须由工作流从固定提交完整构建。
- 将签名路径和密码写入 `build-profile.json5`：Hvigor 可以直接签名，但会让本机路径与敏感配置混进源码；使用环境输入和临时目录保留无凭据的工程配置。

## Consequences

发布必须提前配置有效的四项 secrets，且 release Profile 带有 `group.1516753898738609107`；开发者需要在 AGC 更新 Profile，不能本地编辑授权字段。应用市场渠道的包由市场分发，GitHub 下载页不承诺绕过系统分发限制直接安装。

合成证书与合成 HAP 的回归覆盖 debug 或错误渠道、过期、缺共享组或 ACL、设备限制、证书不匹配、版本错误、缺库和缺资源，以及失败日志不泄密。完整签名、市场接受和真机输入法验收仍要使用真正的发布材料验证，单元测试不能替代。

检索审计：[共享沙箱](../architecture/2026-10-09-harmony-ime-shared-sandbox.md) 持有运行时数据交换与授权值，本篇不改变该决定；[六平台 nightly](2026-10-14-six-platform-nightly-builds.md) 的非 iOS nightly 不使用正式密钥规则保留。检索其余鸿蒙发布和证书记录无重复归属。

# Agent Note: 语法模型目录读不到时退回锁文件

Status: implemented

## Problem

#10761 报告 Windows 上语法模型下载失败。安装语法模型的第一步是 `fetch_grammar_catalog()`，它直连 `raw.githubusercontent.com/metasequoiaime/msime/develop/resources/grammar-models.catalog.json`，不走任何镜像。国内网络经常连不上这个主机，于是即使用户填了下载镜像、模型文件本身能经镜像下载，整次安装也在读目录这一步就以网络错误结束。

## Decision

用户发起安装时，目录读不到或被整份作废，改用编译进来的 `grammar-models.lock.json` 生成一份只有稳定通道的目录（`locked_grammar_catalog`），后面的镜像顺序、`.gram` 格式校验和摘要比对不变。锁文件随程序一起审过，比运行时从自动合并的 develop 读到的目录更可信，所以退回它不扩大信任范围，也不需要再按来源前缀去约束它（它本来就满足那条约束，测试里照样过 `validate_grammar_catalog`）。

主动检查更新（`check_grammar_updates`）不退回：它的结果是告诉用户「有新版本」，拿锁文件去比会把比已安装版本更旧的锁当成更新。

## Alternatives considered

- 目录也经镜像读取（用户镜像、`dl.msime.app`）：目录在可变的 develop 分支上，项目镜像只缓存内容不再变化的地址，需要改服务端放行和缓存规则，不在本仓库；用户镜像能看到并改写目录内容，而目录正是决定下载哪份字节的那一层。留给另开的国内镜像 issue 跟进。
- 只对标签不再变化的八股文退回、万象 LTS 不退回：万象 LTS 在上游原地替换，锁文件里的摘要可能落后，退回后会白下 418 MB 再报摘要不符。但锁文件刚更新过的大多数时候它是对的，而摘要不符时什么也不会装上，和现在直接失败的结果相同，所以三个模型一样处理，规则更简单。
- 在线目录失败时直接报错、提示用户导入文件：这是现状，#10706 已经补上了 Windows 的导入入口；本条是让配置了镜像的用户不必退到手动导入。

## Consequences

连不上 raw.githubusercontent.com 但能经镜像（或直连）下载 GitHub Release 的用户可以装上语法模型。测试版和每日构建通道在目录读不到时装的是锁文件里的稳定版。万象 LTS 的锁文件过期时，退回路径会下载完整文件后因摘要不符失败，不会安装未审过的字节。`PRIVACY.md` 写明了退回行为。没有新增偏好、ABI 或网络请求，宿主不用改。

## Verification

- 先写的 `grammar_install_falls_back_to_lock_when_catalog_unavailable` 在实现前编译失败；实现后三个通道下退回的目录都通过 `validate_grammar_catalog`，三个可下载模型的地址、长度、摘要和上游提交与锁文件一致，目录读得到时照用在线目录。
- `cargo test -p msime-client-providers --lib resource_packs::tests::` 全部通过。

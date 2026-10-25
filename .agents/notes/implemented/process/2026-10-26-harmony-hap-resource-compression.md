# Agent Note: HarmonyOS 发布 HAP 压缩资源条目

Status: implemented

## Problem

HarmonyOS 的 HAP 打包器把 `resources/` 下的词库、模型、设置页和音效全部以 ZIP 存储方式写入。`harmony-v0.2.0` 的 HAP 为 328,655,000 字节（约 313.4 MB），其中拼音词库、日文词典和手写模型占了主要体积；构建产物没有利用这些数据可以无损压缩的事实。

## Decision

发布工作流在定位 hvigor 产物时调用 `scripts/compress_harmony_hap.py`，把原本存储的 `resources/` 非目录条目用 DEFLATE 复制到新的 HAP。脚本保留 ZIP 注释、条目内容和元数据，并用临时文件完成替换，避免失败时留下半个产物。

`libs/`、`ets/`、`module.json`、`pack.info`、`resources.index` 以及其他非 `resources/` 条目保持原压缩方式。这样原生库继续按鸿蒙加载器所需的未压缩布局存在，代码完整性索引也不被脚本改写。

签名发布路径在 `hap-sign-tool` 签名后再次压缩，并用 `verify-app` 验证压缩后的签名包；未发布的工作流产物则使用压缩后的未签名 HAP。应用市场包从这份已验证的压缩 HAP 生成。

## Alternatives considered

- **只移除可选词库或模型**：可以直接缩小下载包，但会让五笔、日文、手写、语言词库或背单词能力随平台版本消失，超出“缩小包体”范围，因此不采用。
- **只压缩未签名中间产物**：实现简单，但签名工具可能重新生成 ZIP 并恢复存储方式，正式下载包的体积无法保证，因此签名后重新压缩并再次验签。
- **让应用首次运行时自己下载或解压资源**：能够继续减少安装包，但会引入联网、离线首次启动、校验和资源生命周期的新契约；当前只改变 ZIP 传输层，不改变运行时资源路径。

## Consequences

`harmony-v0.2.0` 的实测 HAP 从 328,655,000 字节降到 197,712,995 字节，减少 130,942,005 字节（约 40%）。安装后资源内容和路径不变，运行时无需额外解压逻辑；代价是发布阶段需要重新读取并压缩资源，构建时间增加约几十秒，并依赖鸿蒙包加载器对 DEFLATE 资源的既有支持。

压缩脚本的契约测试固定了资源条目压缩、原生库和索引保持存储、ZIP 内容完整及注释保留。签名脚本还会对压缩后的 HAP 执行 `verify-app`，任何签名工具不接受压缩包的情况都会阻止发布。

## Verification

- `python3 scripts/test-harmony-hap-compression.py` 通过。
- `python3 scripts/test-harmony-release-signing.py`：14 项通过。
- `python3 -m py_compile scripts/compress_harmony_hap.py scripts/test-harmony-hap-compression.py platforms/harmony/sign-release.py` 通过。
- 使用公开的 `harmony-v0.2.0` HAP 实测压缩输出，`zipfile.testzip()` 通过；`resources/` 条目出现 DEFLATE，`libs/` 条目保持存储方式。
- 未在本机重新构建或设备安装 HAP：当前磁盘可用空间低于仓库规定的 50 GB 构建门槛，正式构建与真实设备验收留给 HarmonyOS 发布工作流和设备门禁。

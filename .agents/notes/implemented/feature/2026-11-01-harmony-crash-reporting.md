# Agent Note: HarmonyOS 崩溃采集与系统补报

Status: implemented

## Problem

HarmonyOS 宿主和输入法扩展已经能记录部分运行状态，但原生异常、系统保存的应用退出记录和两进程之间的诊断身份没有统一进入共享崩溃上报链路。发布构建也没有把 native host 的版本身份和调试符号作为可追溯产物保存，导致线上签名无法稳定对应源码。

## Decision

HarmonyOS 的 ArkTS 宿主、键盘扩展和 NAPI native host 共用共享崩溃采集核心提供的诊断身份与持续授权代次。宿主在启动、恢复和扩展生命周期边界补报系统可取得的异常退出记录，并通过共享状态镜像保持设置应用和键盘扩展看到同一条授权状态；native host 构建将 `MSIME_HARMONY_VERSION` 注入构建身份，生成独立的调试符号 zip。

发布 workflow 同时上传符号 artifact；明确执行发布时，再把符号归档上传到平台专用的调试符号 release，符号不进入用户安装包。符号发布脚本沿用 Android 切片的参数和校验约定，保证各平台的归档方式一致。

## Verification

本地通过 shell、Python 和 Node 语法检查，工作流只保留 HarmonyOS 崩溃相关改动，并覆盖 native capture、ArkTS telemetry adapter、系统退出记录和符号归档脚本的契约测试。真实 DevEco 工具链和设备验收交给 PR 的 HarmonyOS CI；本机没有可用的 DevEco/鸿蒙设备环境。

## Alternatives considered

- 只在 ArkTS 层捕获异常：改动最小，但捕不到 native host 的访问违规和进程级崩溃，也无法生成可对应源码的 native 符号，因此不采用。
- 把符号混进 HAP 或普通发布附件：取用方便，但会增大用户包并让调试文件跟随公开下载渠道，不符合崩溃诊断的保留和权限边界，因此不采用。
- 继续让设置应用和键盘扩展各自维护授权状态：实现简单，但跨进程代次会漂移，系统补报可能重复提交或漏报，因此不采用。

## Consequences

HarmonyOS 具备与其他平台一致的崩溃现场、系统退出补报和符号追溯路径；发布 workflow 增加一个符号 artifact，明确发布时还会更新平台专用调试符号归档。由于本机缺少 DevEco/设备，ArkTS 编译、HAP 打包和真实输入法扩展验收必须等待 CI 或维护者设备复验。

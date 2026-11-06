# Agent Note: 鸿蒙日文与五笔资源改为按需下载

状态：已实现

## Problem

鸿蒙 HAP 把日文词典和五笔码表随包放进 `resfile/engine`。两份资源合计约 80 MiB，安装后还会分别复制到设置应用和键盘扩展的可写目录；大多数只使用拼音的用户因此承担了不必要的安装包和应用数据体积。

## Decision

鸿蒙沿用共享层已有的 `japanese` 与 `wubi` 资源包。`on_demand_artifacts("ohos")` 允许这两组文件整体缺席，`stage-resources.sh` 在默认和发布模式下用 `verify_resources --omit-on-demand --platform ohos` 只把核心资源放入 HAP。设置应用通过现有的资源包锁文件、下载、校验和原子发布流程安装资源包；共享 UI 显示日文和五笔的下载行，安装进度经 NAPI 和 ArkWeb 桥接回页面。键盘进程只读取 `state/resource-packs` 中已经发布的完整目录，安装完成后下一次聚焦由 `host-api` 刷新词典路径。

## Alternatives considered

- 继续把两份资源随 HAP 分发：改动最小，但无法降低安装包和首次复制的数据体积。
- 让设置应用和键盘扩展直接共用一份只读词库：鸿蒙扩展沙箱与 SQLite 可写路径的限制仍然存在，已有共享沙箱笔记记录了这条边界，继续推进会把一次体积修复扩大成跨进程存储重构。

## Consequences

拼音用户的 HAP 和首次数据准备少了日文、五笔资源；选择日文或五笔时，设置页会提示并下载对应资源包。下载需要隐私政策授权，键盘进程不自行联网。开发者可设置 `MSIME_HARMONY_OMIT_ON_DEMAND=0` 打出包含两组资源的开发 HAP。

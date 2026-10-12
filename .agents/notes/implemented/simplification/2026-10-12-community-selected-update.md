# Agent Note: 社区图库当前作品更新共用流程

Status: implemented

## Problem

社区候选皮肤的可见性和分类修改，以及社区键盘皮肤的分类修改，都重复执行请求、检查图库 client 代次、替换打开的详情及列表项、发布操作提示。三段流程的差异只有请求和成功文案。

## Decision

在 `useCommunityGallery` 增加 `updateSelected`，由调用方传入具体更新请求和成功提示。hook 统一使用当前作品 id、已有的 `runAction` 生命周期与 `replaceSelected`，并在过期 client 的请求完成后忽略结果。三个页面操作调用该入口，保留原有可见性和分类文案。

## Alternatives considered

- 分别抽取分类和可见性更新：仍会有两套相同的代次检查与详情、列表同步代码。
- 把具体服务端方法塞入图库 hook：会让通用图库依赖皮肤特有的字段和命令。

## Consequences

社区作品更新的并发状态和 UI 同步只有一处实现；每页的请求及文案仍在页面定义。图库 hook 和两个皮肤页面交互测试共 75 项通过，类型检查、前端构建、格式、lint、notes 和本地 quick 门禁通过。quick 中 HarmonyOS 原生编译因缺少已暂存的库和资源按脚本规则跳过。

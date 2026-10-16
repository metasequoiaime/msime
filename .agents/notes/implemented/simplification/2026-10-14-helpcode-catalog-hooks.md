# Agent Note: 辅助码目录读取 hook 从设置入口抽出

Status: implemented

## Problem

自定义辅助码表与已安装辅助码插件的读取 hook 仍定义在 `index.tsx`，与设置外壳模型放在一起。目录加载、宿主归属和输入页进入时刷新是可独立复用的逻辑，放在入口里让调用方与生命周期测试依赖整个设置外壳。

## Decision

两项 hook 移到 `settings/use-helpcode-catalog.ts` 并从公共入口导出，设置模型调用同一实现。原有异步代次和依赖保持一致：自定义表的 reader 更换时等待新回答，缺少 reader 或当前读取失败时清空；插件只在输入页打开时读取，离开时清空，再进入时刷新，且只投影 `kind="helpcode"` 的 id 和名称。旧宿主与旧页面请求的成功或失败均不能改写当前列表。

目录元数据类型从 `settings/helpcode-preferences.ts` 引入，与页面和候选预览共用[辅助码公共模型](2026-10-16-helpcode-preferences.md)，不依赖页面模块。

## Alternatives considered

- 保留入口内的私有 hook：调用位置与逻辑相邻，已有设置整页测试。代价是它们不能单独复用，生命周期检查仍需定位入口源码。
- 抽成一个通用异步列表 hook：两个加载流程都能用同一 Promise 链，代码更少。代价是需要参数化 reader 更换时保留列表、离页清空和目录过滤这些不同策略；当前沿用两项明确的业务 hook，复用已有 `useAsyncGeneration`。

## Verification

抽取前后的公共代次契约与设置整页辅助码用例均通过。新增 hook 测试检查两类目录的缺能力、当前失败清空非空列表、旧成功与失败不改写新列表，以及插件过滤和离页重进刷新。测试直接调用公共导出的 hook，目录 fixture 使用合成值并按宿主接口填写字段；自定义表的 reader 更换时保留列表，失败断言等待实际拒绝完成。

## Consequences

hook 逻辑从设置入口移出，可由其他共享设置页面复用，入口文件只保留模型编排。代价是公共导出的 hook 需要继续遵守 `CustomHelpcodeSchema` 和 `PluginClient` 类型契约；实现保持原 Promise 链和 effect 依赖，对加载归属及保留旧列表的边界做行为检查。

## Existing notes audit

按 hook 名称、辅助码目录、辅助码加载和自定义辅助码表检索活跃笔记，未发现共享 React hook 的同机制归属。Windows 辅助码包记录关注原生页面与 host-api 请求，手机导入记录关注码表安装、选择及原生页面；本项不改变这些契约。它们属于不同消费者与职责，没有吸收或替换旧决定。

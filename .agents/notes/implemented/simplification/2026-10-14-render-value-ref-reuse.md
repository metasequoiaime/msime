# Agent Note: 渲染值引用统一复用 useLatestRef

Status: implemented

## Problem

设置、资源管理、皮肤同步和键盘面板多处重复 `useRef(value)` 后在 render 内赋值 `ref.current = value`。公共 `useLatestRef` 已表达同一机制，现有消费者仍各自维护它。

## Decision

12 个文件中的 17 处只随 render 更新的引用使用已有 `useLatestRef`，保持 ref 对象身份、赋值所在阶段和 hook 调用顺序。覆盖回调、当前宿主、列表、模型路径和手写资源状态。

保存快照和草稿、皮肤扫描结果、轮播当前位置、下载登记等引用会在事件或异步完成时立即写入，继续由所属模块管理。本项不改变公共 hook 的更新阶段，也不把更新移进 effect。

## Alternatives considered

- 保留局部 `useRef` 加赋值：实现直观、没有额外 hook 调用；但这套相同机制已经由公共 hook 提供，逐处维护使后续审计难以区分渲染镜像和事件状态。
- 在 effect 中更新引用：更符合 React 对 render 纯度的指导；但会改变现有回调读到最新值的阶段，涉及事件时序设计，不能作为这次等价复用的附带变化。

## Verification

逐项核对迁移引用的所有赋值，确认它们只有 render 赋值。既有行为测试覆盖稳定引用、最新回调、宿主切换、资源下载、模型选择、扫描重入、保存生命周期和手写模型就绪；抽取前的 14 个相关文件、207 项测试通过；抽取后连同定时回调消费者的 17 个文件、253 项测试通过，设置整页的 17 项导航与保存集成测试通过。类型检查、生产构建、lint、格式、quick、笔记门禁和独立审查通过。独立审查另运行 4 个文件、17 项测试通过。

## Consequences

公共 hook 统一维护渲染镜像，消费者只声明所跟随的值。异步保存快照、事件队列和游标等状态仍由所属模块即时更新。这次只改变复用关系，不消除既有 render 写 ref 的限制；未来若迁移更新阶段，需要单独设计并验证回调时序。

## Existing notes audit

按 `useLatestRef`、`use-latest-ref`、渲染引用、`useTimeout` 和 `useFlushOnWindowLeave` 检索活跃笔记，没有同机制归属。原有窗口离开监听和设置订阅记录分别持有事件注册与释放决定；本项只合并其最新回调引用的维护机制。

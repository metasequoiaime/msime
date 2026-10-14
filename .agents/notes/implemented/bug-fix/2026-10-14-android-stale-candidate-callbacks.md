# Agent Note: Android 过期候选回调保留输入会话

Status: implemented

## Problem

候选管理菜单、展开候选按钮和九键面板保留绘制时的候选代次。菜单打开期间新按键或在线候选更新推进代次，回调仍可到达。运行时正确拒绝旧身份，但 Android 把拒绝送入 `fail`，停止原本可继续输入的会话。#7837 的现场环境未知，附件也没有记录这条操作链，不能把本地找到的缺陷当作现场根因。

## Decision

宿主在 JNI 调用前复核当前 `session/generation`，明确过期的 UI 回调直接返回，不改变编辑器、视图或会话。管理菜单和展开候选复用 `sameCandidateVersion`，九键面板保留会话和代次；运行时的拒绝契约不放宽，其他响应解析、JNI 与编辑器失败仍执行现有分类和停止逻辑。

## 旧笔记审计

- [跨平台候选渲染边界](../../implemented/architecture/2026-10-13-cross-platform-candidate-rendering-boundaries.md)：部分重叠，继续遵守宿主复核不能替代运行时围栏。
- [候选删除子菜单](../../implemented/bug-fix/2026-10-14-android-candidate-removal-submenu.md)：部分重叠，补齐旧身份拒绝后的宿主处理；保留子菜单确认决定。
- [非法固定位置不推进代次](../../implemented/bug-fix/2026-10-09-invalid-fixed-position-generation.md)：相关不变量，不修改运行时。
- 其余候选主题、插件与其他平台笔记：机制相关、改动无关，不吸收。

## Alternatives considered

- 运行时接受旧代次：可以避免拒绝，但旧序号可能已指向另一个候选，会误上屏或误删词，不能采用。
- 所有 `ok:false` 都当作未处理：改动集中，但会吞掉真正的运行时故障，丢失已有失败分类，不能采用。
- 匹配错误文字识别过期：可覆盖宿主尚未观察到的运行时代次，但会让 UI 依赖非结构化错误文案；当前事件在主线程串行派发，优先在已知身份边界复核。

## Verification

API 28 上使用已安装完整原生宿主的类加载器创建服务、执行 `onCreate` 与 `onCreateInputView`，再创建关闭学习与云候选的合成原生会话。旧实现在 `stale menu session retained` 断言失败，服务的会话句柄被清零；没有用未初始化对象的空指针充当复现。

修复后四种旧菜单操作均不触发失败，不替换当前视图；保留的旧展开按钮、旧代次和其他会话的拼写回调不写合成编辑器，也不停止会话。新按键继续推进代次，新展开按钮通过产品 `apply` 正常向合成 `InputConnection` 上屏。普通及 `--core` 冒烟都注册这项回归。

完整原生宿主增量构建、签名和 16 KB 对齐通过；复用该分支已校验的 JNI 与资源。API 28 的 `smoke.sh --core --feedback --diagnostics` 通过系统输入上屏退格、失败分类、过期回调、真实反馈页 50 次上屏和五轮删除/键盘重开、原生日志包上传编码及剪贴板视图/Activity 重建与保存失败重试。测试不提交反馈、不访问云端。本轮不占用其他会话的 API 35 设备。

`bash scripts/verify-local.sh --quick`、`pnpm run verify-notes` 与 `git diff --check` 通过。

## Consequences

收益是过期的界面回调不再把仍可用的会话降级为直接输入。代价是菜单打开期间候选更新后，旧菜单操作被忽略，用户需要针对当前候选重新打开菜单。测试使用安装产品完整服务和视图，但窗口不附着到系统编辑器；真实反馈页连续输入由独立设备套件覆盖。

宿主复核依赖当前已应用的视图，不能掩盖运行时内部未同步的代次变化。#7837 现场仍须新的固定类别诊断才能确认，本地缺陷修复不关闭该 issue。

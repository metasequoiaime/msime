# Agent Note: 隔离首次设置流程的浏览器历史

Status: implemented

## Problem

`onboarding-page.test.tsx` 的 HarmonyOS 手机流程会用 `history.pushState` 模拟每一步，并在完成或跳过时回退自己的记录。测试文件的 `afterEach` 只卸载 React 树和恢复计时器，没有重置 JSDOM 的历史栈。前面的用例留下的当前历史记录会改变后续用例的 `history.back()` 落点，导致完整文件运行时「右滑回到第一步」间歇或稳定失败；单独运行该用例则通过，掩盖了测试间状态泄漏。

## Decision

在该测试文件的统一 `afterEach` 中调用 `window.history.replaceState({}, "")`，把每个用例结束时的当前历史状态恢复为空状态。这样保留流程用例对真实 `pushState`、`back` 和清理深度的覆盖，同时让下一用例从相同的历史入口开始。

## Alternatives considered

让每个涉及历史的用例自行清理，能缩小修改位置，但漏掉新加的历史用例就会再次泄漏；统一 teardown 能覆盖整个文件的所有测试路径。

在产品组件中检测并纠正未知的历史状态，会把测试隔离问题变成运行时行为，且无法消除不同测试之间共享 JSDOM 窗口的问题，因此不采用。

## Consequences

首次设置测试仍会验证流程自己的历史记录数量和 `msimeOnboarding` 状态；每个用例结束后，后续测试不再继承前一个用例的入口状态。修复只作用于测试 teardown，不改变产品导航行为。

## Verification

旧 teardown 下，完整 `apps/desktop/tests/account/onboarding-page.test.tsx` 运行结果为 28 项中 1 项失败，失败点是 HarmonyOS 手机流程第二次右滑后找不到「把水杉加进键盘」；单独筛选该用例会通过。加入历史重置后，同一文件 28 项全部通过，单独运行仍通过。

`pnpm exec vp test apps/desktop/tests/account`（34 个文件、181 项）通过；`pnpm exec vp fmt --check apps/desktop/tests/account/onboarding-page.test.tsx`、`pnpm run verify-notes`、`git diff --check` 和 `bash scripts/verify-local.sh --quick` 也通过。quick 门禁对共享 Rust 工作区、Android target、Linux desktop shell（本机无 Docker 时跳过）和 Apple bridge 完成了适用检查。

## 历史检索与参考

按 `onboarding`、`history`、`pushState`、`popstate` 和 `pushedEntries` 检索活跃笔记，没有记录该测试文件的历史栈隔离约束；仓库其他移动端测试已在各自 `afterEach` 中重置 `window.history`，本篇补齐首次设置流程测试的同一约束。

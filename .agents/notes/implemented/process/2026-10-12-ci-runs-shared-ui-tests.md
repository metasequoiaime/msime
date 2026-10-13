# Agent Note: CI 跑共享设置界面的 lint、类型检查和 vitest

Status: implemented

## Problem

共享设置界面（`packages/ui`，鸿蒙和各平台的设置窗口都画它）有两千九百多个 vitest 测试，但没有任何自动门禁跑它们：

- GitHub 上的 workflow 里没有 vitest。`ci-platforms.yml` 的「HarmonyOS host」job 装了 pnpm 依赖，只跑鸿蒙键盘逻辑测试和设置页打包检查。
- 推送前钩子跑 `scripts/verify-local.sh --quick`。`--quick` 在编译阶段结束后就退出（`if [ "$quick" -eq 1 ]; then … exit`），跑不到后面的 typescript 阶段（tsc 加 vitest）。

结果是 #7210（鸿蒙手机不再列语音快捷键）合进 develop 时，`mobile-app-chrome.test.tsx` 里断言「手机列出语音快捷键」的测试变红了，CI 全绿，直到下一个改语音页的 PR 本地跑全量时才发现。

## Decision

- `ci-platforms.yml` 新增 `ui` job（显示名「Shared settings UI」）。`changes` 多一个输出 `ui`，以下任一改动时为真：`packages/`、`apps/`、`package.json`、`pnpm-lock.yaml`、`pnpm-workspace.yaml`、`.nvmrc`、这个 workflow 本身。没法判断改动范围时（非 push / PR 事件、取不到 base）和其他输出一样为真。
- 为真时依次：
  - 按 `.nvmrc` 装 Node，`pnpm install --frozen-lockfile --ignore-scripts`；
  - `pnpm run lint`；
  - `apps/desktop` 和 `apps/harmony` 的 `tsc --noEmit`；
  - 用项目自己的 `vitest run`（不用 npx，理由见 `verify-local.sh` 的 typescript 阶段）。
- 为假时和其他平台 job 一样照样跑完，只打印跳过：必需检查报 skipped 会挡住合并。
- 同一个 PR 带上 #7214 里对 `mobile-app-chrome.test.tsx` 的同一处修复，这个 job 一开始就是绿的；两处改动一字不差，谁先合并都不冲突。
- 云词库子页导航测试在请求发出后继续等待目标按钮解除 `disabled`，然后再点击。「完整目录」用例的首次列表请求延迟返回，覆盖请求已发出、界面仍在加载的窗口；「应用到本机」遵守同一等待条件。

## Alternatives considered

- **让推送前钩子的 `--quick` 也跑 vitest。** 每次推送多一两分钟；而且钩子可以被 `--no-verify` 绕过，在别的机器上也不一定装了依赖。CI 是唯一每个 PR 都一定经过的地方。
- **把 vitest 塞进「HarmonyOS host」job。** 它只在鸿蒙或共享代码改动时跑，而且一个 job 两种失败原因不好看；单独一个 job，名字就说明挂的是什么。
- **给云词库测试加固定等待或重跑 CI。** 能让当前失败暂时消失，修改量也很小；但机器负载和请求耗时都不固定，仍会点击禁用按钮。测试按按钮的真实可用状态等待，延迟只用于模拟响应，不充当同步条件。

## Consequences

- **收益**：改共享界面的 PR 在 CI 上就能看到测试失败。
- **代价**：动界面的 PR 多一个几分钟的 job。
- **还没生效的部分**：仓库规则集「main and develop protection」的必需检查里还没有这个 job，失败时只显示红叉、不挡合并。要加进去得改仓库设置，等仓库负责人确认。

## Verification

- `actionlint` 通过。
- 本地照 CI 的步骤：删掉全部 `node_modules`，`pnpm install --frozen-lockfile --ignore-scripts`，然后 lint、两个 tsc、全量 vitest，2905 个通过。
- 不带 `mobile-app-chrome` 修复时，develop 上全量 vitest 1 个失败，就是 #7210 留下的那个。
- CI 运行 `38339433158` 中 3016 项通过、1 项失败：云词库测试只等 `list` 调用发出，就点击仍禁用的「完整目录」。本地给首次响应增加 100 ms 延迟，在旧等待条件下复现同一断言失败；该测试随后改为等待按钮可用。[鸿蒙设置行笔记](../bug-fix/2026-10-10-harmony-phone-settings-input-rows.md)也记录过同一个偶发失败，属于独立的界面布局决定，保留原记录。

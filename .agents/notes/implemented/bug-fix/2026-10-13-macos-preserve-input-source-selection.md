# Agent Note: macOS 重装和启动保留用户删除的输入源

Status: implemented

## Problem

用户在系统设置移除水杉的模式后，重装又出现中、双、五、日、韩、英。`MSIMERegisterAndEnableInputSources` 在每次登记后启用全部非按需模式，`Info.plist.in` 同时声明六个模式默认启用；启动的 `MSIMEEnableNewInputModes` 还会启用记录里没有的模式。

## Decision

登记前查本 bundle 已安装的输入源，已有输入源时仅登记并验证列表可见，不改变任何启用状态，包括全部模式都被用户关闭的情况。真正首次登记只启用主模式 `.Hans` 和存在时的顶层输入源，按标识符选择而非取列表第一项。plist 仅保留主模式默认启用。

启动只把发现的模式写进 offered 记录，不启用或关闭模式。已有模式保持原样，升级新增模式也由用户选择加入。控制器切换明确按需方案的既有逻辑保留；模式未加入时，同一个主模式仍可使用不同输入方案。

## 历史审计

[首次选中按需模式](../../implemented/bug-fix/2026-10-08-macos-first-pick-opt-in-mode.md) 的「已启用模式不关闭」决定保留；本篇扩大「不自动加入」的范围。[首次安装需要登录](../../implemented/bug-fix/2026-10-09-macos-first-install-login-required.md) 的注册表缺失判定保留，登记后仍查不到源时仍返回 fnfErr。

## Alternatives considered

- 只跳过启用当前已启用的模式：减少重复调用，但用户移除的模式恰好是关闭的，仍会被重新启用，不能解决问题。
- 只修登记函数：安装更新不再补加；但 plist 的默认值和启动补加路径仍能加入新模式，因此三条路径一起收窄。

## Verification

`input-source-registration` 覆盖首次安装仅主模式、模式顺序变化、只保留中或五、全部关闭，以及启动发现未记录模式不启用。原生构建、`skin-preview`、`info-plist-names`、`bundle-contents`、`backend-library-load` 与六版本生成检查通过。本机首次换成新默认值时，原先随包默认启用的五个扩展模式变为关闭；再次安装相同包，启用模式集合不变，仍只保留主模式。

## Consequences

首次升级会收窄旧包的默认模式集合，本机实测从六个默认模式变为主模式一个。其他模式继续可在系统设置手动添加，同一个主模式仍能运行用户选择的输入方案；更新登记与启动不再请求启用这些扩展模式。已登记源的启用状态由系统保存，代码不调用关闭 API。

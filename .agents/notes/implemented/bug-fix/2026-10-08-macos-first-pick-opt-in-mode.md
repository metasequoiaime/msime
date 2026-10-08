# Agent Note: macOS 首次启动不再关掉已启用的按需模式

Status: implemented

## Problem

macOS 15 用户反馈：装好 0.52.0 后，在右上角输入法菜单里选水杉没有反应。

在 GitHub 的 macOS 15.7.9 runner 上按用户路径复现：设置应用「立即安装」，在系统设置「添加」对话框的「简体中文」下添加水杉，再从菜单栏选它。系统设置里排在「简体中文」第一位的是「水杉 · 笔」（Stroke），所以用户第一次加入的往往就是它，这也解释了反馈截图里为什么有「BH」。从菜单第一次选它时，输入法进程被拉起，当前输入源却仍是 U.S.；之后再选、杀掉进程后冷启动再选都能切过去。

只有第一次启动有问题。原因是 `MSIMEEnableNewInputModes`：还没有 `MSIMEOfferedInputModes` 记录时，它把已经启用的按需模式（粤、注、越、藏、笔）当成「系统无视 `tsInputModeDefaultStateKey` 自行打开的」，关掉一次。进程启用不了输入法本身（总输入源 `TISEnableInputSource` 返回 0 但状态不变），用户第一次加入水杉只能经系统设置，所以第一次启动时看到的已启用按需模式，多半就是用户刚加的、此刻正要切过去的那个。关掉它，系统就丢掉这次切换，退回原来的输入源。

对照实验：预先写好完整的记录，第一次选择就能切过去；记录里只缺「笔」（第一次启动只会关掉它），第一次选择仍然失败。

## Decision

`MSIMEEnableNewInputModes` 对按需模式只记录、不启用，已经启用的也不关掉。函数不再接受 `disabler` 参数，输入法进程也不再调用 `TISDisableInputSource`。

## Alternatives considered

- 保留关掉一次的行为，只跳过当前选中的那个模式：它最强的理由是仍能挡住系统自行打开的粤拼入口。但第一次启动时选择还在进行中，选中的输入源未必已经是这个模式，判断不可靠；况且原注释记录的 macOS 27.0.1 现象是用户每次移除「粤」它都会回来，关一次本来就挡不住。
- 第一次启动时推迟到切换完成后再关：同样分不清是用户加的还是系统开的，推迟只是把关掉用户选择的时间往后挪。
- 改连接名 `InputMethodConnectionName`：排查时 imklaunchagent 的 `Refusing connection name ... unrecognized` 日志看起来像原因。反汇编 macOS 15 的 `+[IMKServer connectionNameFor:]` 后确认，不在苹果白名单里的名字都会退回 `%@.Connection`；但 macOS 27 上同样如此、同样有这条日志，输入法照常工作，而且冷启动再选能成功，所以它不是这个问题的原因，不改。

## Consequences

用户在系统设置里加入的按需模式，第一次启动后保持启用，第一次从菜单选择就能切换。代价是：如果某个系统版本真的自行打开了按需模式，输入菜单里会多出这一项，需要用户自己在系统设置里移除；之后它被记录下来，不会再被启用。

## Verification

- `tests/input/InputSourceRegistrationTests.mm` 改为断言已启用的按需模式只被记录、不被启用；直接用 CMake 目标 `input-source-registration-test` 的编译参数构建并运行，通过。
- macOS 15.7.9 runner 上的对照实验见 Problem；完整流程是临时分支 `diag/macos15-ime-smoke` 上的诊断 workflow，分支已删除。
- 反馈里的「在输入框切换导致 app 崩溃」在 runner 上没有复现，这次改动不涉及它。

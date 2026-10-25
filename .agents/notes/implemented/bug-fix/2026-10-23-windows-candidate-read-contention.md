# Agent Note: Windows 候选读取争用不触发退场

Status: implemented

## Problem

#9430 报告 Windows 输入时候选窗持续闪烁。`SessionController::candidate_view` 的非阻塞读取在焦点闸门、信箱或连接注册表争用时返回空；`CandidateWindow::reposition` 和 `paint` 将其交给 `hide_for`，即使同一组字仍在输入，也会开始退场或立即隐藏。下一轮成功读取后又显示，形成一次隐藏和重现。

## Decision

定位和绘制共用一次读取规则：真实快照优先，包括明确不可见的快照；只有本轮读空、未在退场、已显示的快照可见，且现有无锁焦点租约检查确认其仍是当前租约时，才沿用已显示的一帧。未知租约、焦点改变、停止或没有旧帧时立即收起。已开始的正常退场在读空时继续，但同样要求租约确认。

这是对[候选窗默认几何与动效](../../implemented/feature/2026-10-20-windows-candidate-mac-default-geometry.md)中“读空开始退场”的定向替代；几何、进出场时长、上屏和取消的退场行为保留。旧帧仅供显示，候选点击仍按服务端租约与候选代次重新校验。

## Alternatives considered

阻塞读取能始终区分最新状态，省去旧帧选择；但会让窗口线程等待持有闸门的管道写入或引擎工作，违背非阻塞界面边界，因此保留现有非阻塞读取。

继续让读空开始退场，能在不确认租约时尽早收起旧内容；但现有 `still_current` 已能无锁确认焦点，争用本身并不表示组字结束，继续退场正是闪烁来源。

## Verification

`scripts/test-windows-candidate-contention.py` 提取生产方法和定位、绘制入口的读取与隐藏分支，以真实 `FocusGate` 确定性占住闸门：未修改的源码失败于“锁争用隐藏候选窗”，修复后同一租约连续三轮读空均不隐藏、不开始退场。回归还覆盖新快照替换旧帧、明确不可见不恢复旧帧、正常结束和退场、退场途中明确隐藏优先、焦点切换尚未确认时立即收起、服务停止和缺失租约检查。绘图与窗口隐藏调用由模型记录，不能据此声称跑过真实 Win32 窗口。

macOS 主机和远端均通过最终争用回归；`test-windows-preedit-caret.py`、164 项可移植 Windows 原生测试、`pnpm run verify-notes` 与 `git diff --check` 通过。远端 `scripts/verify-local.sh --quick` 通过，包含最终争用回归、全量脚本契约、Rust workspace 与共享 Apple 桥接层检查；该环境没有 MinGW 和 Node，Windows 产品编译留给 PR CI，笔记校验在本机补齐。Windows 真机持续输入的观感尚未验收。

## 现场报告 #9677

#9677 是 Windows 0.3.0 用户在 VS Code 里用全拼连续输入时录到的同一现象。录屏（670×446、标称 30 fps 的可变帧率，116 帧）逐帧看：约二十个按键里有 9 个按键之后候选卡片整块消失一帧（不超过约 33 毫秒），下一帧带着新一页回来；消失的那一帧里没有淡出、没有留下阴影或空白底板、卡片也没有移位，说明是窗口被隐藏，不是画了一帧空白或尺寸变化。9 个消失帧里有 6 个，宿主文档里的组字串还是上一个键的样子，也就是隐藏发生在 Server 处理下一个键的期间——输入线程在 `FocusedSession` 的 `gate_.with_active` 里跑引擎、占着焦点闸门的时候，正是本篇说的争用窗口。默认皮肤没有退场动效，0.3.0 的 `hide()` 因此直接 `SW_HIDE`，下一轮轮询（0.3.0 主循环最长 50 毫秒）读到快照再显示。

0.3.0 不含本修复：`windows-v0.3.0`（725d5b3d41，2026-10-23 15:44 +0900）早于 0b85106df2（同日 22:45），之后没有发过 Windows 版本。把 `scripts/test-windows-candidate-contention.py` 指向 `windows-v0.3.0` 的 `CandidateWindow.cpp` 运行，失败在「#9430: a busy focus gate hides the live candidate window」；指向 develop 通过。

develop 上后来的两处改动不改变这个结论。按客户端桌面路由（#9509）把读取挪到协调线程后，读空时协调线程沿用上一轮的归属，只要租约仍是当前焦点、桌面仍有效就继续路由给原桌面，桌面线程的 `read_presentation` 照本篇规则保帧；Windows CI 上的 `windows-desktop-candidates` 覆盖了「读锁争用保留当前帧、失焦时即使一直争用也收起」。渲染屏障的叫醒（#9667）由 `SessionPump` 在按键入队、进入焦点闸门之前触发，只让候选窗早一点读到已经发布的快照；即使叫醒后的读取撞上争用，也只会保帧，不会隐藏。

仍缺 Windows 真机上连续输入的观感验收，需要报告者在包含本修复的下一个 Windows 版本上复验。

## Consequences

允许同一租约在争用期间显示上一帧，短暂落后于正在计算的候选；不把它当成新一代结果或新的点击授权。模拟并发和可移植测试不替代 Windows 真机持续输入的观感验收。

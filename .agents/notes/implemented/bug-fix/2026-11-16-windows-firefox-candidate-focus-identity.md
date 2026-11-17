# Agent Note: Windows Firefox 候选焦点按 COM 身份判断

Status: implemented

## Problem

Issue #11483 报告 Windows Firefox 的 Nodeseek 评论编辑器在连续输入时候选栏经常消失，退格后才恢复。报告视频显示候选列表已经出现，继续输入后候选窗消失，而编辑器仍在输入；Edge 和 Windows 默认输入法没有同样现象。Windows TSF 候选呈现器在文档焦点变化和线程焦点恢复时，直接比较 `ITfDocumentMgr *` 接口指针。Firefox 这类宿主可能为同一文档管理器返回不同的 COM 接口包装，导致输入法把仍属当前文档的候选误判成失焦并调用 `OnKillThreadFocus`。

## Decision

候选文档与当前焦点文档的归属统一按 COM identity 判断，不按 `ITfDocumentMgr *` 的接口地址判断。`ThreadMgrEventSink` 的文档焦点切换和 `ThreadFocusSink` 的线程焦点恢复都调用已有 `_IsSameComObject`，再由 `candidate_belongs_to_focus` 约束两边对象都存在且身份相同才恢复候选显示。真实的不同文档、空焦点和缺少候选文档仍然隐藏或不恢复候选。

回归测试覆盖同一 COM 身份、不同身份以及两边对象缺失的四种组合，并作为 TSF host-independent policy test 接入 CMake。该修复只改变焦点对象身份判定，不改变候选代次、延迟隐藏或上屏规则。

## Alternatives considered

- **在 Firefox 进程名上加特殊放宽**：可以快速绕过报告者环境，但会把宿主特例扩散到候选生命周期，无法覆盖同样返回包装接口的其他编辑器，也无法区分真实失焦，因此不采用。
- **继续使用接口指针直接比较**：实现最小，但违反 COM 对象身份规则，正是同一文档被误判为失焦的根因，因此不采用。
- **在线程焦点丢失时一律延迟隐藏**：能掩盖短暂焦点变化，却可能让真实切换到其他应用时旧候选继续留在屏幕上；先修正文档归属判断，保留现有真实失焦语义。

## Consequences

- Firefox 等宿主为同一文档管理器返回不同接口包装时，候选栏不会因地址不同而被隐藏，连续输入和退格恢复路径保持同一候选会话。
- 不同文档仍会按原规则调用 `OnKillThreadFocus`，因此旧候选不会跨编辑器或跨应用泄露。
- 本机是 macOS，无法启动 Windows 原生宿主或在 Firefox/Edge 真实编辑器中验收；PR 验证段保留报告者环境复验缺口。Windows 编译和 TSF 测试交给 PR CI，不把未运行的设备验证写成已通过。

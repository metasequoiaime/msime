# Agent Note: 跟踪桌面云剪贴板全局快捷键

Status: proposed

## Problem

`#11677` 是机器人从 QQ 群转述整理的功能请求，希望在 macOS、Linux 和 Windows 上通过可配置的全局快捷键一键调出云剪贴板。issue 正文只有这一句诉求，没有写输入法版本、期望的默认组合键或使用场景细节；截至本笔记更新，issue 下没有补充评论，也没有找到重复 issue，引用它的只有本跟踪 PR #11706。

三端目前都已有不经快捷键的云剪贴板入口：macOS 输入法菜单的 `showCloudClipboard:`（`platforms/macos/src/input/InputController.mm`，安全输入激活时直接拒绝），Linux 的桌面面板动作 `cloud-clipboard`（`platforms/linux/fcitx5/FcitxEngine.cpp`、`platforms/linux/src/core/ClientEngine.cpp`），Windows 托盘菜单的「云剪贴板…」（`platforms/windows/src/candidate/TrayMenuLayout.h`）。共享路由 `SurfaceRoute::CloudClipboard`（`crates/client-core/src/host_surface.rs`）已经存在。缺的是可配置的全局快捷键：共享偏好里还没有对应字段，也没有定义快捷键冲突、输入法焦点、密码框保护和面板打开后目标编辑器的行为。

## Proposal

本笔记作为 `#11677` 的跟踪记录合入 develop，实现另开 PR。先在共享偏好中加入可配置的快捷键字段，复用现有 `cloud-clipboard` 路由，定义冲突检测、启停生命周期、未登录行为、密码框拒绝（沿用 macOS 菜单入口的安全输入判断）和面板打开时的焦点归属，再按 macOS、Linux、Windows 宿主分别实现和验证。每个平台保持独立代码切片和真实桌面验收。

## Alternatives considered

- **只注册一个固定快捷键**：最强理由是实现简单，但会与系统和用户应用冲突，也无法满足可配置需求，因此不采用。
- **只依赖现有菜单和托盘入口**：最强理由是三端已经有入口，不需要新增全局按键处理，但仍需多次点击，没有实现 issue 要求的快速全局唤起能力，因此不采用。

## Acceptance criteria

- 共享偏好、冲突校验和路由契约完成并有测试。
- macOS、Linux、Windows 分别完成宿主接线、冲突和安全输入保护测试。
- 三端真实桌面验收和界面截图齐全。

## Risks

本笔记只做设计跟踪，不宣称三端已有快捷键实现。全局按键处理若没有焦点和密码框边界，可能干扰用户正在使用的应用，或在密码框中暴露云端剪贴历史。反馈没有给出期望的默认组合键，默认值若选错会与常用应用冲突，因此默认应为关闭、由用户自行配置。

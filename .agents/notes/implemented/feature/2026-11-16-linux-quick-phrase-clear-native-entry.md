# Agent Note: Linux 原生快捷短语清空入口

Status: implemented

## Problem

共享 `clear_quick_phrases` 事务和 Windows、macOS 适配器已经存在，但 Linux 的「词库」入口仍然把用户带到共享设置页面。这样 Linux 没有原生确认、宿主 API 调用和结果反馈，#10990 的原生迁移切片没有闭合。

## Decision

Linux 在 GTK 4.8 原生设置窗口中提供独立的「清空快捷短语」入口。IBus 和 Fcitx5 的桌面菜单都通过继承的运行配置路径启动同一个 `msime-linux-<edition>-quick-phrase-clear` 程序；窗口只负责确认、调用 `msime_client_settings_dictionary` 的 `clear_quick_phrases` 操作和显示实际清除条数，事务、维护租约、CAS 无关的本机词库语义继续由共享 Engine/host-api 负责。没有 GTK 4.8 时不编译该窗口，也不在菜单中暴露不可用入口。

入口作为独立菜单动作接入，保留现有 Linux 词库页面的其他管理能力，不把整个共享词库页面临时重写成 GTK 窗口。运行配置路径只从宿主继承的 `MSIME_CLIENT_HOST_OPTIONS` / `MSIME_FCITX5_OPTIONS` 读取，不能从窗口参数接受任意目录；命令行参数仅用于宿主传入的绝对路径。

## Alternatives considered

- 把现有 Linux「词库」路由整体改成新的 GTK 词库管理窗口：能一次覆盖全部词库功能，但范围远超 #10990，也会重复实现已有共享页面，延误本次清空能力。
- 在共享 React 页面继续保留 Linux 专属按钮：实现最省，但违反原生 UI 迁移边界，Linux 仍没有原生确认和宿主接线。
- 让 IBus/Fcitx5 各自实现一套清空逻辑：可以避免新增可执行文件，但会重复维护事务调用、错误处理和确认语义，容易与 Windows/macOS 分叉。

## Consequences

- Linux 的本机清空入口与 Windows/macOS 使用同一 host-api 操作，失败时不会由界面层逐条回滚，成功提示使用 Engine 返回的实际条数。
- 新增一个很小的 GTK 原生程序和两个菜单动作；安装包在缺少 GTK 4.8 开发环境时不带该入口，已有输入宿主仍可运行。
- Linux 的完整词库浏览、导入和编辑页面仍由既有入口提供；后续若迁移这些页面，应复用本窗口的配置路径和事务调用边界。

## Verification

- `git diff --check`：通过。
- 远端 quick 门禁 `rbuild 20261116-073959`：通过（退出码 0）。
- Linux native host：128/128 测试通过。
- Windows native run：225 项通过，另有 2 项主机差异已排除。
- Linux CMake 目标、IBus/Fcitx5 菜单接线和宿主 API 调用由上述门禁验证；真实 Linux 桌面窗口验收留作报告者环境复验，不作为代码推进前置。

# Agent Note: Windows 沉浸式进程读取用户数据目录

Status: implemented

## Problem

Windows 11 26H2 的开始菜单和搜索宿主属于 AppContainer。安装器以高完整性创建 `%LOCALAPPDATA%\metasequoiaime-full` 后，只给 `Users` 修改权限并降到中完整性；目录没有 `ALL APPLICATION PACKAGES` 与 `ALL RESTRICTED APPLICATION PACKAGES` 的读取权限，宿主因此读不到 `runtime-options.json`。报告者手动补上这两条目录 ACL 后，设置搜索恢复正常。

## Decision

`EnsureImeUserDataDir` 在保留 `Users` 修改权限和中完整性标签的同时，为 `S-1-15-2-1`（`ALL APPLICATION PACKAGES`）和 `S-1-15-2-2`（`ALL RESTRICTED APPLICATION PACKAGES`）递归授予 `(OI)(CI)RX`。安装器生命周期契约测试检查命令仍然存在，Windows 安装冒烟测试从实际 ACL 中分别核对两个 SID 的读取和执行权限。

开始菜单特有的组合与提交异常仍未定位；本次只修复已由 ACL 对照实验确认的读取缺口，不把未知根因写成代码改动。

## Alternatives considered

- **只授予 `ALL APPLICATION PACKAGES`**：普通商店应用能读取，但受限应用包仍可能被拒绝；报告者的复现明确涉及受限宿主，因此同时授予两个系统 SID。
- **把数据目录搬到系统目录或放宽为所有用户读取**：会改变用户数据布局并扩大可读范围，超出本 issue 已确认的最小权限修复，因此不采用。

## Consequences

开始菜单、搜索等 AppContainer 可以读取运行时选项，同时仍只有桌面用户组拥有写入权限。安装器会把权限递归应用到已有文件和新建子目录；Windows 11 26H2 真机上开始菜单特有的组合行为仍需报告者或 Windows runner 继续验收。

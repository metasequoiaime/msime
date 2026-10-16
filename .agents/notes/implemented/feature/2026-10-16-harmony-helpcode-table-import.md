# Agent Note: HarmonyOS 从输入设置导入辅助码表

Status: implemented

## Problem

#7176 的 Android、iOS 已能导入单个辅助码表，HarmonyOS 手机却隐藏插件页，输入设置也没有导入入口。旧移动端导入笔记暂缓鸿蒙是因为两个沙箱不通；主线已经实现插件目录与偏好的共享镜像，这个前提已改变。

## Decision

共享输入设置仅在宿主提供 `importHelpcodeTable` 时显示「导入辅助码表」。HarmonyOS 通过异步桥打开系统文档选择器，读取至多 1 MiB 的普通文件并严格 UTF-8 解码，再交给已有 Rust `import_helpcode_table`，不在 ArkTS 重写码表格式、命名、替换或安装规则。取消返回 null；失败保留原因、不改选择。

成功后刷新辅助码目录，用函数式偏好更新选到发起操作的全拼或双拼组，保留等待选择器期间其他设置的修改。共享沙箱仍沿用已有发布、焦点导入与插件刷新机制，不另建共享路径。

## Related decisions

- [移动端码表导入](../../implemented/feature/2026-10-12-mobile-helpcode-table-import.md)：部分取代其中的鸿蒙暂缓决定；Android、iOS 与 Rust 导入契约不变。
- [共享沙箱镜像](../../implemented/architecture/2026-10-09-harmony-ime-shared-sandbox.md)：沿用插件与偏好同步，签名授权限制不变。
- [辅助码公共偏好](../../implemented/simplification/2026-10-16-helpcode-preferences.md)与[目录 hook](../../implemented/simplification/2026-10-14-helpcode-catalog-hooks.md)：部分重叠，保留归属与异步代次守卫，补导入后的主动刷新。

## Alternatives considered

- 只开放手机插件页能复用压缩包导入，但用户只有 `.txt` 文件，仍需手工编写清单，因此沿用其他移动端已有的单表导入。
- Web 文件输入可以少写宿主桥，但不能沿用原生选择器的权限与严格有界读取；页面不接收路径或整表内容，交给宿主处理。

## Verification

共享设置 6 文件 449 项回归通过，覆盖取消、失败、重复点击、等待期间修改偏好、宿主切换、卸载与目录刷新竞态；新增手机和桌面整页导入保存后，导入文件的 9 项再次通过（其中 2 项为新增整页用例）。桌面与 HarmonyOS 类型检查、lint、全仓格式检查、前端生产构建、HarmonyOS 逻辑套件通过。arm64 原生库与实际 ArkTS HAP 编译成功，本地 quick 完整通过（含 Windows 交叉编译；Linux 因无 Docker 跳过）。`hdc list targets` 没有连接设备，未执行系统选择器或键盘跨沙箱验收。

## Consequences

手机不需要先制作插件清单或压缩包即可导入码表；桌面形态的 HarmonyOS 同样得到入口，其他宿主没有能力时不显示空按钮。导入成功选到发起组，取消与失败保留原偏好。目录主动刷新沿用原有代次守卫，旧响应不能覆盖新目录。

包含插件目录的镜像仍依赖签名 Profile 的 data-group-id；缺授权时导入只发生在设置沙箱。文件上限、UTF-8 解码与 Rust 请求上限均沿用现有移动端契约。模拟器打包和组件回归不能替代带授权的设备验收。

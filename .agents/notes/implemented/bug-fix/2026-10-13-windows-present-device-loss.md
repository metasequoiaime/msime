# Agent Note: Windows 自绘窗口在 Present 报设备丢失时重建，不再让 Server 退出

Status: implemented

## Problem

Server 的自绘窗口（候选窗、候选菜单、托盘卡片、悬浮状态栏、输入模式提示、双拼键位图）都经 msimeui 的 `DeviceResources` 画：Direct2D 画进 DirectComposition 交换链，`EndDraw` 之后 `Present`。六处的写法相同：`EndDraw` 报 `D2DERR_RECREATE_TARGET` 时丢掉目标、下一帧重建；`Present` 失败则一律抛出「presentation failed」。

显卡驱动更新、TDR 重置显卡、远程会话换了适配器之后，交换链的 `Present` 报的是 `DXGI_ERROR_DEVICE_REMOVED` 或 `DXGI_ERROR_DEVICE_RESET`，不是 `D2DERR_RECREATE_TARGET`。候选窗把这当成画不出来：异常进窗口过程的兜底，`fail()` 置上永久的 `failed_`，Server 主循环看到 `candidates.failed()` 退出，要等看门狗至少 2 秒后重启，期间没有输入法。其他几个窗口则被当作组件故障永久停用（工具栏、提示、键位图）。

## Decision

- `msimeui::DeviceResources::IsDeviceLost(HRESULT)` 是唯一的判定：`D2DERR_RECREATE_TARGET`、`DXGI_ERROR_DEVICE_REMOVED`、`DXGI_ERROR_DEVICE_RESET` 算设备丢了，其余失败不算（包括分层窗口回退路径里 `UpdateLayeredWindow` 失败返回的 `E_FAIL`）。宏先转成 `HRESULT` 再比较，原因同各处原来的注释：这几个宏在某些 SDK 和 MinGW 里是无符号的。
- 六个窗口统一成同一个顺序：`EndDraw` 成功才 `Present`，把两者合成一个结果；`IsDeviceLost` 为真时走各自原有的重建分支（`DiscardTarget`，候选窗另外清掉 `shown_` 并 `InvalidateRect`，其余照旧）；否则 `EndDraw` 失败报「drawing failed」，`Present` 失败报「presentation failed」，和以前一样是致命的。
- `DiscardTarget` 已经会清掉 D3D 设备、交换链和 DirectComposition 对象，下一次 `EnsureForComposition` 看到没有设备就整套重建，所以不需要别的改动。

## Alternatives considered

- **只改候选窗** — 只有它的失败会让 Server 退出，改一处就够解决「输入法掉线」。没有采用：设备丢失是整机的事件，同一时刻每个窗口的下一次 `Present` 都会失败，只改候选窗的话工具栏、提示、键位图会在同一次驱动更新里被永久停用；判定放在 `DeviceResources` 里，六个窗口共用一份。
- **把 `Present` 的设备丢失在 `DeviceResources::Present` 内部吞掉并自行 `DiscardTarget`** — 调用方一行不用改。没有采用：候选窗的重建还要清 `shown_`、`InvalidateRect`，否则缓存的帧挡住重画（`D2DERR_RECREATE_TARGET` 分支的注释写的就是这个坑）；只在库里丢掉目标而调用方以为这一帧画上了，`painted_` 会记下一帧没有上屏的内容。

## Consequences

- **收益**：显卡驱动更新、TDR 或远程会话切换适配器之后，候选窗和其他自绘窗口在下一次绘制时换新设备重画，Server 不退出、不等看门狗。
- **代价与已知上限**：设备真的一直起不来时（每次重建后的 `Present` 都报设备丢失），窗口会反复重建而不是报故障；`EnsureForComposition` 创建设备失败时仍会走原来的失败路径。没有在真实的 TDR 上验证：`dxcap -forcetdr` 或 `TdrDelay` 一类的复现需要 Windows 真机。

## Verification

- `platforms/windows/msimeui/tests/src/test_device_resources.cpp` 的 `device_loss_covers_present_and_end_draw` 钉住哪些 HRESULT 算设备丢失。这个用例随 `msimeui-tests` 交叉编译通过；本机 Wine（arm64 主机经 Rosetta 模拟 x86_64）上这套测试在它之前就会因已记录的「rosetta error: invalid gdt selector index 5」中止（`scripts/known-failures.txt` 的 `wine msimeui-tests`），所以没有在本机运行。

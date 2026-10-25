# Agent Note: Android 9、10 设备套件开始前等系统放弃被强停的输入法连接

Status: implemented

## Problem

API 28 的 CI 设备验收偶发停在 `BatchCompositionDeviceSmoke` 的「deferred composition length 1」，报错里 `IME showed []`：键盘根本没出现。Android 11 以下 `smoke.sh` 不能用 `--no-restart` 附着到已运行的产品进程，以产品为目标的套件（这里是它前面的 `FourteenKeyDeviceSmoke`）结束时，系统按 `finished inst` 连同 `:ime` 一起强停产品。

Android 9 的 `InputMethodManagerService` 收到这次断开时把它当作服务崩溃：`onServiceDisconnected` 清掉输入法接口，把 `mLastBindTime` 设为现在，等系统重启服务；但强停已经移除了服务记录，系统不会再拉起它，而这种强停不发 `PACKAGE_RESTARTED`，管理服务也不会改选别的输入法。此后 `TIME_TO_RECONNECT`（3 秒）以内，编辑器取得焦点只得到「等待输入法绑定」，弹键盘的请求也被丢掉；下一个套件的 `restartIme`（`ime disable/enable/set`）选回的还是同一个 id，不会重绑。CI 日志里断开在 04:42:32.956，下一个套件 04:42:34.9 打开编辑器，`:ime` 直到套件结束、焦点再次变化的 04:42:50.5 才启动。

本机 API 28 模拟器上能稳定复现：键盘开着时跑一个很快结束的产品目标 instrumentation，`dumpsys input_method` 随即显示 `mHaveConnect=true`、`mCurMethod=null`；结束后立刻打开编辑器并点输入框，8 秒后 `:ime` 仍不存在；等 3.5 秒再点，系统断开重连，键盘正常出现。

## Decision

`DeviceSmoke.onStart` 在跑用例之前调用 `outlastDeadImeConnection()`：仅 Android 11 以下，读 `dumpsys input_method`，看到「有连接、没有输入法接口」（`mHaveConnect=true` 且 `mCurMethod=null`）就每 250 毫秒再看一次，直到连接恢复或满 3.5 秒。断开一定发生在第一次看到这个状态之前，所以等满后第一次焦点请求必然超过 `TIME_TO_RECONNECT`，系统走强制重连。连接正常或根本没有连接时立即返回，不增加等待。

## Alternatives considered

- 键盘等不到就重新点输入框或重跑套件：能掩盖问题，但不知道等多久才有效，也会把真正的「键盘不出来」缺陷一起掩盖。
- 在 `smoke.sh` 每个产品目标套件结束后固定睡 3 秒：只覆盖 `smoke.sh` 的顺序，别的脚本（如直接调用 `am instrument` 的专项脚本）照样会撞上；而且不管有没有死连接都要等。
- 在 `restartIme` 里先选别的输入法再切回，迫使系统解绑：解绑只在有客户端开始输入时才发生，`restartIme` 时通常没有聚焦的编辑器，切换不会清掉死连接。

## Consequences

所有继承 `DeviceSmoke` 的套件在 Android 9、10 上多一次 `dumpsys input_method`（只读输入法绑定状态，不读编辑器内容）；只有前一个产品目标套件刚强停了输入法时才最多等 3.5 秒。Android 11 起 `smoke.sh` 附着到已运行的产品进程，不强停 `:ime`，不受影响。不继承 `DeviceSmoke` 的套件（如 `ClipboardExportDeviceSmoke`）自己的第一次聚焦远在 3 秒之后，暂不需要。

验证：上述本机复现里，立刻聚焦两次都失败，等 3.5 秒两次都恢复；`dumpsys` 判据与 Android 9 源码 `InputMethodManagerService.dump` 的输出一致。

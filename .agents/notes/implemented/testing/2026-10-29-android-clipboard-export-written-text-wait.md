# Agent Note: 剪贴板导出设备验收等正文写完、等键盘真正捕获

Status: implemented

## Problem

#10215：develop 提交 `ee650ba113` 的 Native Platform CI 里，只有 Android device (API 35) 失败，同一次运行的 API 28 通过。失败在 `ClipboardExportDeviceSmoke` 的 verify 段：从键盘剪贴板面板点「导出全部剪贴板历史」，导出文件已经出现，但正文没过 `checkText` 的「complete Unicode snapshot and order」。前后几次跑到这一步的运行都通过，中间的提交没有改剪贴板代码。

读代码后能确认两处缺陷，第三处是诊断缺口：

- **文件出现就读，正文可能还没写完。** SAF 目录的导出走 `ExportDirectory.write`：先 `DocumentsContract.createDocument` 让提供方建一个空文档（`DiagnosticDocumentsProvider.createDocument` 用的是 `createNewFile`），再读一遍确认是空的，最后用 `"wt"` 打开写入。验收在 `files()` 的数量加一时立刻读全部文件，这一刻读到的可能是空文件，也可能只写了一部分（合成长文本有 5000 多个字），于是「文件确实写出来了，内容不对」。键盘导出在 `:ime` 进程里写，设置页直连导出在应用进程的工作线程里写，两处都有这个竞态；API 35 模拟器在套件后段更慢，更容易撞上。
- **「等 LONG 出现在历史里」这一步本来就成立。** LONG 在 select 段的 `seed()` 里写进历史，verify 段设好系统剪贴板后等它出现，证明不了输入法捕获过这次复制。
- `checkText` 把七个条件写成一个 `&&`，失败时只有一句固定消息，看不出缺的是条目数、置顶顺序还是正文。

## Decision

只改 `platforms/android/tests/device/settings/ClipboardExportDeviceSmoke.java`，不动产品代码。

- **SAF 目录的导出等正文写完再检查。** `awaitExportedFiles(count, message)` 先等 `.txt` 文件数到 `count`，再在 15 秒内反复读，直到每一份都通过正文检查才返回；超时后按最后一次读到的内容抛出具体缺了哪一项。select 段 Android 9 的首次授权导出、verify 段的直连导出和键盘导出都改用它。下载目录（Android 10 起的默认去处）不需要：产品写完正文才把 `IS_PENDING` 归零，`downloads()` 只认已发布的条目。
- **键盘捕获改等 LONG 的时间戳变化。** 设系统剪贴板之前先读出历史里 LONG 的 `timestamp`；共享存储收下已有文字时保留置顶、换上新时间戳写盘（`client-core` 的 `push_validated`）。等待放在打开键盘剪贴板面板之后、点导出之前：verify 段每次都先 `am force-stop app.msime.android`，冷启动的 `:ime` 在 `onCreate` 才注册剪贴板监听，收不到它起来之前设好的那一条，要到打开面板时补读（`ImePanels.showClipboardHistory` 调 `captureClipboard(PANEL_OPENED)`）才捕获；输入法进程早已在运行时由监听在设剪贴板时捕获。两条路径到面板打开时都已经发生，此时等时间戳变化，既证明键盘真的捕获并写了盘，也保证导出读到的是写盘后的历史。超时消息带上原时间戳和当前时间戳（-1 表示这一条不在或读不出）。
- **正文检查逐项报。** `textProblems` 对七项分别检查，失败消息写出缺失项的名字、文件字节数、全部条目头行（含 `==========` 的行和「共 N 条」那一行，每行截到 80 字），长正文缺失时再写出它是整段不在、换行被改成 `\n`、首尾空白被去掉，还是只有尾行在。消息里不带 5000 字的正文。

## Alternatives considered

- **按分诊方案，在打开剪贴板面板之前等 LONG 的时间戳变化。** 这样改最直接，位置和原来的等待一样。不采用：冷启动的 `:ime` 在面板打开前不会捕获这一条，等待会在这种情况下必然超时，把间歇失败变成稳定失败。
- **verify 段设剪贴板前先删掉 LONG，再等它重新出现。** 判据同样可靠。不采用：删掉会丢掉置顶，捕获回来的是未置顶的新记录，`checkText` 期望的「1 · 已置顶」对不上。
- **数量到了之后固定睡一会儿再读，或者等文件长度连续两次不变。** 改动更小。不采用：固定延时在慢模拟器上同样会撞上，长度不变也可能是写入方被调度出去；以「正文通过检查」为完成条件才是确定的，失败时也只多花 15 秒。

## Consequences

- 收益：去掉了读到未写完文件的竞态；键盘这一步真正验收了「输入法捕获剪贴板并写盘」；以后再失败时，断言能直接说明是条目数、顺序还是正文出了问题。
- 代价：导出正文确实错了时，失败要等满 15 秒的正文等待才报出来。
- 未在设备上跑过：本机只用 `check-host.sh` 编译了设备套件。需要 PR 上的 `android-device-pr`（API 35）和合并后 develop 上 API 28、35 的整套运行确认。若键盘捕获这一步在设备上超时，说明面板补读没有捕获（例如输入法读不到系统剪贴板），按消息里的时间戳再查。
- 相关：同一套件键盘入口「real keyboard panel clipboard entry」的间歇失败见 [Nightly 设备失败](../bug-fix/2026-10-28-android-nightly-device-failures.md)，与本篇是不同阶段。

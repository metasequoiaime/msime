# Controlled TSF edit control

This directory contains a small native Win32 editor host for repeatable Windows TSF checks. It builds a `msime-tsf-edit-control` library and a `msime-tsf-edit-control-demo` window that exercise Direct2D/DirectWrite rendering, pre-edit display attributes, candidate positioning, selection, caret movement, soft wrapping, mouse hit testing, and the production TSF service boundary from a controlled editor surface.

It is an experiment and a verification tool, not a product component: it ships in neither the installer nor any package, it does not register the IME, it does not start the production Server, and it does not replace checks in third-party editors. It exists because third-party editors differ in how they drive TSF, so a surface whose behaviour is fully known makes a regression attributable to the TIP rather than to the editor.

It is built with the rest of the platform under `if(WIN32)` and registers no CTest entry — it is driven by hand. Cross-building it confirms that it compiles and links; the interaction it is for happens on Windows.

The source was imported from `MSIME-Windows` commit `345cb87a3822f6ad7013bb29506fe3d856c1931a`.

## 韩语系统输入复现

使用已配置的原生构建目录编译 `msime-tsf-edit-control-demo`。完整输入验收需要先按正常安装流程注册本次构建的 TSF DLL，并运行同一构建的 Server；这个程序不负责安装或注册。

```powershell
cmake --build target/windows-full/x64 --config RelWithDebInfo --target msime-tsf-edit-control-demo
target/windows-full/x64/bin/msime-tsf-edit-control-demo.exe --activate-profile
```

`--activate-profile` 只为当前进程选择已注册的本版本输入源，并核对实际激活的 CLSID 和 profile GUID。窗口切到前台后，可以按 F12 再次选择，避免 Windows 恢复该应用此前的输入源。普通启动不主动切换输入源。默认英文模式下先按 Shift 进入韩语组字。

从 WinUI 设置选择韩语，保留默认两套式，输入 `gksrnr` 应显示 `한국`。点击保存罗马字布局后重新聚焦编辑框，输入 `hang` 应显示 `항`，继续 `ug` 应变为 `한국`；`bang` 后退格应从 `방` 回到 `반`。Escape 取消当前组字。Windows 的汉字键是右 Ctrl：输入 `han` 后按右 Ctrl，再按数字 1，应提交 `韓`。另检查 `xa`、Shift+G 后接 `a`、半角标点及 `q/v/z` 保留。

焦点验收要检查未完成文字留在原框、新框独立组字，以及候选窗跟随当前框。这个实验控件与系统记事本等实际应用须分别记录结果；预编辑或选择汉字通过，不能代替候选定位及跨应用验收。保留当前输入源、布局、构建提交、实际文字和界面截图；失败时不要将整套流程记作通过。

## 系统记事本复核

打开独立的测试文本文件，确认焦点在该文档，再用系统输入源选择器切换到本次注册的输入法。不要通过粘贴或直接写入 Unicode 代替按键输入。先检查默认两套式 `gk → 하`，再从 WinUI 选择罗马字，回到同一文档输入 `hangug → 한국`；记录首次激活和切换布局后的结果。

韩语组字中的 Return 提交文字后继续交给应用，因此记事本同时插入换行。保存文件后核对 UTF-8 文字：`한국` 为 `U+D55C U+AD6D`，`하` 为 `U+D558`。若字体显示方框，以实际文件内容判断字符，同时将显示缺口单独记录；不能据此宣称界面验收通过。

继续检查 `bang` 退格得到 `반`、Escape 取消当前组字、`han` 后右 Ctrl 打开候选并选择 `韓`，以及切换到另一个测试文档后文字归属和候选位置。首次激活的异常需要单独复现，后一次正常不能覆盖它。结束时恢复原输入源，注销临时测试注册，不关闭其他文档或更改正式安装。

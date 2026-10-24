# 韩语系统输入验收

`korean-romaja-controller` 检查生产控制器、真实 Rust 会话和 AppKit 文档；它隔离框架基类的最后一次失焦 IPC，不代表系统输入源验收。

`korean-system-input-acceptance` 是另一个独立可执行文件。它不链接宿主库、不导入输入控制器，只向自己的应用进程投递系统键盘事件，由已安装输入法通过 InputMethodKit 写入两个原生 `NSTextView`。检查预编辑、后续组字、Return、退格、取消、布局外字母、紧音、标点、汉字候选及焦点归属，逐项输出实际文字、marked 状态和输入源 ID。

## 前提

在能够运行图形应用的 Mac 登录会话中，按 `platforms/macos/scripts/install.sh` 的要求安装、签名本次构建，并在系统输入源中添加韩语模式。新安装后是否需要重新登录，以 `scripts/check_input_source.swift` 的检查为准。

先在水杉设置中选择韩语和待验收的布局。验收程序只读取所选布局对应的结果，不替用户改写偏好。运行期间不要操作键鼠。系统事件投递权限须由测试者自行配置；程序仅调用 `CGPreflightPostEventAccess` 查询，不请求或修改权限。

## 执行

在仓库规定且已配置的 macOS 构建目录中编译两个入口：

```sh
cmake --build target/macos-isolated --target korean-romaja-controller-test korean-system-input-acceptance
ctest --test-dir target/macos-isolated -R '^korean-romaja-controller$' --output-on-failure
target/macos-isolated/korean-system-input-acceptance --probe
```

`--probe` 只报告前提，始终包含 `system_input_executed: false`。CI 的 `korean-system-input-prerequisites` 运行这一探测，不将它算作系统输入通过。

独立 CI Mac 另外执行 `run-korean-system-ci.sh`：安装当前构建、注册系统输入源，分别验证缺省两套式与罗马字。`--appkit-events` 将按键放入验收应用自身的 AppKit 事件队列，经当前输入源连接独立 IMK 进程；不直接调用生产控制器或 Rust 库，也不向其他应用注入事件。报告记录事件路径、两个进程的 PID、实际文本和原生编辑框截图。注册或输入失败使 CI 失败，不将前提不足记为通过。

已安装输入源步骤在 macOS 15 Intel CI 中执行；涉及此入口的 PR 必须启动该构建，不能因普通 PR 的 Intel 默认省略而跳过。macOS 26 arm64 继续执行原生 CTest 和前提探测，其新模式启用限制不记为系统验收通过。

CI 优先选择实际启用的 `.Korean`。仅在它未启用时显式传入 `--primary-source`，选择已默认启用的 `.Hans` 主输入源，偏好仍为 `scheme: korean`，尝试生产控制器的主模式回退路径；仍须通过独立 IMK 进程完成全部韩语输入断言。TIS 启用调用可能返回成功但模式仍 disabled；`enabled-sources.json` 单独记录实际状态，不能将主输入源验收当作韩语菜单入口已启用。手动入口省略此参数，继续要求已添加的 `.Korean` 输入源。

将下面的输入法路径替换为本次构建实际安装的绝对路径，报告路径也必须为绝对路径。先保留缺省两套式并执行：

```sh
target/macos-isolated/korean-system-input-acceptance dubeolsik \
  "$HOME/Library/Input Methods/水杉输入法.app" \
  "$PWD/korean-system-dubeolsik.json"
```

再从设置界面保存罗马字布局，执行：

```sh
target/macos-isolated/korean-system-input-acceptance romaja \
  "$HOME/Library/Input Methods/水杉输入法.app" \
  "$PWD/korean-system-romaja.json"
```

程序核对 LaunchServices 登记路径与指定输入法路径一致，在自己的前台窗口选择该输入法的韩语模式，退出时恢复此前的输入源。它不会注册、卸载或替换输入法。

退出码 `0` 且报告的 `system_input_executed`、`passed` 均为 `true` 才算该布局通过；`77` 表示前提不满足，`1` 表示输入断言失败，`2` 表示参数或报告写入失败。验收材料应同时附上被安装构建的提交 SHA、两个 JSON 报告，以及测试者对候选界面和设置保存的实际观察；缺少其中一项需明确记录。

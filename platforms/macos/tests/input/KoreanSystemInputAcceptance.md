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
target/macos-isolated/korean-system-input-acceptance.app/Contents/MacOS/korean-system-input-acceptance --probe
```

`--probe` 只报告前提，始终包含 `system_input_executed: false`。CI 的 `korean-system-input-prerequisites` 运行这一探测，不将它算作系统输入通过。

独立 CI Mac 另外执行 `run-korean-system-ci.sh`：安装当前构建、注册系统输入源，分别验证缺省两套式与罗马字。默认通过 `CGEventPostToPid` 将系统事件投递给验收应用自身，由正常 IMK 路径处理；只检查已有投递权限，不请求或修改 TCC。`--appkit-events` 仅用于对照诊断，手工 AppKit 队列不能替代真实系统输入路径。程序不直接调用生产控制器或 Rust 库，也不向其他应用注入事件。报告记录事件路径、两个进程的 PID、实际文本和原生编辑框截图。注册或输入失败使 CI 失败，不将前提不足记为通过。

汉字候选打开时还读取该输入法进程的可见窗口几何，检查窗口靠近原生编辑框光标（48 点邻域）且位于编辑器前方，再按数字选字。报告保存光标与窗口矩形；不依赖屏幕录制权限，不读取其他应用的窗口标题或图像。

验收先创建前台窗口、聚焦编辑框并激活其 `NSTextInputContext`，再选择输入源；TIS 选择失败时设置 AppKit 的 `selectedKeyboardInputSource` 属性。报告同时保存 TIS 状态、实际全局输入源与文本上下文输入源，实际输入源及全部文字断言仍须通过，不能将发出选择请求当作成功。

验收客户端是带独立 bundle identifier 的 AppKit `.app`。CI 在启用源前初始化 AppKit，先尝试父输入法，再重新枚举并启用韩语模式；每个源最多等待五秒，保存逐项返回码、源类型及实际启用状态。Mavericks 起模式按独立输入源管理，父记录 disabled 不能阻止尝试韩语；实际 `.Korean` 必须 enabled，不能用默认启用的 Hans 代替。

报告另保存原输入源的 TIS 对照返回码，以及输入上下文实际提供的本 bundle 输入源。选源最多等待五秒；若活动窗口的两条选源路径仍停留在 ABC/US，结果为前提失败，必须在能实际选择本次已安装输入源的 Mac 登录会话中验收，不能按编译或控制器测试结果放行。

已安装输入源步骤在 macOS 15 Intel CI 中执行；涉及此入口的 PR 必须启动该构建，不能因普通 PR 的 Intel 默认省略而跳过。arm64 在 macOS 26 宿主继续执行原生 CTest 和前提探测，不记为系统输入验收通过；其检查名称因分支保护沿用 `macOS 15 arm64`。

`enabled-sources.json` 的 `passed` 只在 `.Korean` 实际启用时为真。API 添加未生效时，CI 的 `SourceSetup` XCTest UI 夹具打开系统设置，通过韩语分类添加 `Metasequoia · KO`，再用同一客户端复核真实状态。夹具只使用镜像已有的 UI 测试自动化会话，不改 TCC、信任设置或系统输入源数据库。固定的 Xcode 派生目录在现有 CI 构建目录下的 `input-source-setup`，结果与界面树保存在 `input-source-setup.xcresult` 和 `.log`。系统设置仍不列出该模式或最终无法选择时继续失败，需要可实际选源的登录会话；不将注册调用或 UI 点击返回成功当成系统验收通过。

将下面的输入法路径替换为本次构建实际安装的绝对路径，报告路径也必须为绝对路径。先保留缺省两套式并执行：

```sh
target/macos-isolated/korean-system-input-acceptance.app/Contents/MacOS/korean-system-input-acceptance dubeolsik \
  "$HOME/Library/Input Methods/水杉输入法.app" \
  "$PWD/korean-system-dubeolsik.json"
```

再从设置界面保存罗马字布局，执行：

```sh
target/macos-isolated/korean-system-input-acceptance.app/Contents/MacOS/korean-system-input-acceptance romaja \
  "$HOME/Library/Input Methods/水杉输入法.app" \
  "$PWD/korean-system-romaja.json"
```

程序核对 LaunchServices 登记路径与指定输入法路径一致，在自己的前台窗口选择该输入法的韩语模式，退出时恢复此前的输入源。它不会注册、卸载或替换输入法。

退出码 `0` 且报告的 `system_input_executed`、`passed` 均为 `true` 才算该布局通过；`77` 表示前提不满足，`1` 表示输入断言失败，`2` 表示参数或报告写入失败。验收材料应同时附上被安装构建的提交 SHA、两个 JSON 报告，以及测试者对候选界面和设置保存的实际观察；缺少其中一项需明确记录。

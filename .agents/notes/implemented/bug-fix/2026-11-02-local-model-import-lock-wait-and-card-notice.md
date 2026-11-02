# Agent Note: 本地模型安装等锁可取消，Windows 模型管理器的结果写在卡片里

Status: implemented

## Problem

#10633：Windows 0.4.1 的原生设置窗口里，「语音输入 › 服务配置」的模型管理器对「中英流式」点「从文件导入」，选中长度和 SHA-256 都与目录一致的上游归档后，页面看上去没有任何反应：没有可见的进度，没有错误，`%APPDATA%\app.msime.windows\voice-models\` 下只有 `.models.lock`，没有暂存目录也没有装好的模型。报告者问失败的错误码有没有真的传到设置页，以及卡住时能不能取消。

沿链路查下来，导入的请求和错误码本身是通的：`install_local_model` 经 `msime_client_voice_local_model_install` 带 `files` 调 `local_models::import`，失败时 `models::error_code` 归并、`local_model_import_error_message` 取文案，取消以外的每一种失败都有一句说明。看上去「没有反应」的是两处：

- 共享安装器在暂存目录建出来之前先拿模型根目录的跨进程锁 `.models.lock`（`acquire_model_lock`），用的是阻塞的 `file_lock::exclusive`。锁被别的操作占着（另一个进程的安装，或同一进程里另一个模型还没下完）时，导入停在 C ABI 先报的那次「导入中 0%」，磁盘上正好只有 `.models.lock`。等锁期间不看取消标记，「取消导入」和关窗时的 `msime_client_voice_local_model_cancel(nullptr, 0)` 都叫不停它，只能等前一个操作结束。
- 原生页把模型管理器的说明统一画在四张模型卡片之后。导入很快失败时，按钮立即变回「从文件导入」，失败说明落在窗口下沿以外，用户看到的就是「点了没反应」。打开文件对话框本身失败（`CoCreateInstance`、`Show`、`GetResults` 出错）时则直接返回，和用户关掉对话框一样什么都不显示。

本机只能交叉编译 Windows，没有在报告者的机器上确认是哪一种：如果是等锁，修复后可以取消；如果是很快失败，修复后失败原因写在卡片上，报告者能告诉我们具体是哪一句。

## Decision

- `acquire_model_lock(root, cancel)` 改成每 `MODEL_LOCK_POLL`（50 ms）`try_exclusive` 一次，每次之前 `check_cancel`，取消后报 `LocalModelError::Cancelled`。锁放开后照常拿到锁继续，互斥的语义不变：同一个根目录的写操作仍然一个接一个，只是等待可以被取消。`install_model`、`install_files_with`、`install_archive_members_with` 传各自的取消标记；`adopt_files` 没有取消入口，传一个不会被置位的标记，行为与原来相同。删除（`remove_installed`）原来就用 `try_exclusive_with_grace`，被占时报 `InstallRunning`，不变。
- 不新增进度阶段：等锁时进度仍是 C ABI 先报的 `import` / `download` 零字节。新增一个 `wait` 阶段能让界面写「等待其他安装完成」，但阶段名是 Windows、Tauri 共享页、iOS、Android、HarmonyOS 五个宿主共用的契约，iOS 把不认识的阶段写成「即将完成…」，要一起改。这次只保证能取消。
- Windows 原生设置窗口（`platforms/windows/settings/main.cpp`）：
  - 说明经 `set_local_models_notice(message, id, severity)` 设置。属于某个模型的下载、导入、删除的结果带上模型 id，画成卡片里按钮下方的 `InfoBar`（成功 `Success`、取消 `Informational`、失败 `Error`），紧挨着用户刚点的按钮；读列表失败这类不属于某张卡片的说明仍在所有卡片之后。
  - `pick_model_files(HRESULT &failure)` 把对话框打不开、读不到所选文件的 HRESULT 交给调用方，用户取消（`HRESULT_FROM_WIN32(ERROR_CANCELLED)`）仍不算失败；选了文件却一个本机路径都读不到时返回空列表。两种情况都由 `VoiceSettings.h` 的 `local_model_picker_error_message` 给出说明，写在这个模型的卡片里。
  - `install_local_model` 的 `id` 改为按值传：打开对话框期间页面可能重画，按钮和它持有的 id 随之换掉。
- 共享 React 页（`packages/ui/src/voice/local-models.tsx`）的说明同样在所有卡片之后，Windows 原生页在这一点上不再与它一致。共享页只在 macOS 和 Linux 作为设置界面出现，这次不动。

## Alternatives considered

- **等锁时立即报 `busy`（`local_model_install_running`）** — 最直接回应「卡住的安装要有明确提示」：用户立刻看到「该模型正在安装」，不会有一个停在 0% 的进度条。但占锁的往往是另一个模型的下载或另一个进程，这句话说的是「这个模型」，是错的；而且同一进程里先后点两个模型的下载原本会排队完成，改成失败会让第二个白点。等待加可取消保留了排队，又不再卡死。
- **新增 `wait` 进度阶段** — 见上，界面能如实写出在等什么。代价是五个宿主的阶段契约一起改，超出这个 Windows issue 的范围；可取消是先要有的那一半。
- **给等锁加一个超时** — 不需要取消标记，几分钟后自动报错。但一个上百 MB 的模型在慢网络下要下很久，任何固定超时都会误伤正常排队的安装；取消由用户决定更合适。
- **把说明移到模型管理器顶部，或用整页的提示条（`notice_`）** — 改动更小。顶部同样可能在视口外（服务配置子视图里模型管理器上面还有好几组），整页提示条又会和偏好保存失败这类全局说明混在一起；写在被操作的那张卡片里最不会被错过。

## Consequences

- **收益**：等别的安装放开模型根目录时，「取消导入」「取消下载」和关窗都能在 50 ms 内停下，磁盘上什么都不留。Windows 上任何一次下载、导入、删除的结果都出现在被操作的那张卡片里；文件对话框失败也有说明并带 HRESULT，便于对照报告。
- **代价与已知上限**：等锁仍然没有专门的文字，界面停在「导入中 0%」直到前一个操作结束或用户取消；要显示「等待其他安装完成」需要按上面的 `wait` 阶段方案一起改五个宿主。轮询比阻塞多了每 50 ms 一次的 `try_exclusive`，只发生在锁被占的时候。Win7 构建的 `settings/win7` 模型表单没有改。

## Verification

- `cargo test -p msime-client-providers --lib voice::local_models`：新增 `cancelling_an_import_that_waits_for_the_model_lock_returns_promptly`（另一把锁占着根目录时导入合成的小 tar.bz2，取消后应在时限内以 `Cancelled` 返回且根目录为空）和 `an_import_waiting_for_the_model_lock_installs_once_it_is_released`（锁放开后导入照常完成）。前者在改动前的阻塞实现上先失败（10 秒内没有返回，`Timeout`），改动后两条都通过，本地模型组 86 条、`msime-client-providers` 全部 303 条单元测试通过。
- `platforms/windows/tests/ui/voice_settings.cpp` 新增 `local_model_picker_error_message` 的检查，在 macOS 上用 clang 单独编译运行通过，并随 `bash platforms/windows/build-cross.sh x64` 用 MinGW 交叉链接。
- `main.cpp` 是 WinUI 3 工程，MinGW 交叉构建不编它，由 CI 的 Windows runner 编译；没有在 Windows 上运行设置窗口，也没有在报告者环境复现，证据等级为交叉编译与单元测试。

## Related notes

- [本地语音模型从本地文件安装](../feature/2026-10-09-local-voice-model-import.md)：部分重叠，导入复用 `install_model` 与按 id 的登记不变，此处补上等模型根目录锁时的取消。
- [Windows 原生设置窗口的语音页](../feature/2026-10-11-windows-settings-voice-page.md)：部分重叠，模型管理器的说明位置从「所有卡片之后」改为写在被操作的卡片里。

# Agent Note: macOS 提示音测试卡在 Depot 虚拟机的麦克风授权框上

Status: implemented

## Problem

2026-10-26 12:52Z 之后，`ci-macos.yml` 在 Depot arm64 runner（`depot-macos-26`）上的每一次真实构建都挂在 `voice-cue-player`：普通 job 20 秒超时，Sanitizers 300 秒超时；GitHub 托管的 `macOS 15 x86_64` 照常通过。在 develop 上手动触发的 run 40349583473 同样复现，所以与具体 PR 无关；`VoiceCuePlayer.mm` 和它的测试很久没改过。最后一次通过的 arm64 构建（run 40324418790，12:55Z，2.58 秒通过）和失败的构建报的系统都是 macOS 26.3.1（25D2128），型号都是 `VirtualMac2,1`，挂起前的输出也完全相同，日志里看不出镜像差异。

## Decision

`VoiceCuePlayerTest` 在开始播放之前先判断这次播放会不会停在授权框上：默认输出设备带输入流，而且当前进程的麦克风授权是「未决定」。两项都只用只读查询，不会弹框：CoreAudio 的 `kAudioHardwarePropertyDefaultOutputDevice` 和输入域的 `kAudioDevicePropertyStreams`，以及 TCC 预检 `AVCaptureDevice authorizationStatusForMediaType:`。条件成立时，测试跑完 bundle 加载与回退那部分断言后以 77 退出，CMake 给这个测试设 `SKIP_RETURN_CODE 77`，CTest 记为跳过，并打印跳过原因。条件不成立的机器（开发机、授权已决定的 runner）照常跑播放和完成回调断言。产品代码 `VoiceCuePlayer.mm` 不改。

## 证据

诊断 job 在同一个 `depot-macos-26` 镜像上单独编译测试和几个探针，在每个进程挂住 12 到 20 秒后抓 `sample`，同时抓 `coreaudiod` 和两个 `tccd` 的栈，并导出 TCC 日志。

- 挂住的不只是我们的测试：只调用 `[[NSSound soundNamed:@"Glass"] play]` 的探针、播放仓库里 `start.mp3` 的探针，以及系统自带的 `afplay`，都挂住。CoreAudio 的只读属性查询瞬间返回：唯一的设备是 `Apple Virtual Sound Device`（`AVIODevice`），同时是默认输入和默认输出。
- 被测进程的主线程停在 `-[NSSound play]` → `AVAudioPlayerCpp::prepareToPlayQueue()` → `AudioQueuePrime` → `AQMEIO_HAL::SelectDevice` → `AudioDeviceCreateIOProcID` → `HALC_ProxyIOContext::_TellServerAboutStreamUsage` → `HALC_ProxyObject::SetPropertyData` → `mach_msg`，同步等 `coreaudiod` 回复。
- `coreaudiod` 处理这条消息的线程停在 `HALS_IOContext::SetPropertyData` → `CheckTCCAndThrow` → `macOS_TCC_And_Input_Status_Handler::CheckTCC` → `TCCAccessRequest` → `tccd_send_message` → `xpc_connection_send_message_with_reply_sync`。
- TCC 日志记下这次请求的去向：`service=kTCCServiceMicrophone, preflight=no`，`AUTHREQ_SUBJECT: subject=/Users/runner/runner-20261026-231540/bin/Runner.Listener`，随后 `AUTHREQ_PROMPTING` 和 `display_prompt: called ... for service kTCCServiceMicrophone`。用户会话里的 `tccd` 停在 `CFUserNotificationReceiveResponse`，也就是在等一个没人会去点的授权框。
- 同一台机器上，TCC 预检报的麦克风状态是 0（未决定），播放前后都一样，预检本身 1 秒内返回。用户和系统 TCC.db 里的麦克风授权只有 `/usr/local/opt/runner/provisioner/provisioner`、`/opt/hca/hosted-compute-agent` 等几个固定路径，没有 runner 那条按时间戳命名的 `Runner.Listener` 路径。

结论：在这台虚拟机上，往一个收放一体的设备播放时，`coreaudiod` 会按调用方的责任进程检查麦克风授权；责任进程现在是每个 job 新解出来的 `Runner.Listener`，TCC 里没有它的记录，于是 `tccd` 弹授权框并一直等。NSSound、CoreAudio 和我们的代码都没有超时可以兜住这一步。12:52Z 前后 Depot 那边具体改了什么（多半是 runner 的启动方式或安装路径，让责任进程不再落到已授权的 provisioner 上）从旧日志里看不出来：作业日志不打印 runner 路径，所以这一点是推断，没有核实。

## 为什么产品不受影响

`InputController.mm` 的 `toggleVoiceInput:` 在任何采集路径（系统听写、本地模型、HTTP、豆包、外部 provider socket）开始之前，都要求 `microphoneAuthorizationStatus == AVAuthorizationStatusAuthorized`，否则先走授权流程、不启动采集。开始提示音由 `voiceCaptureDidStart` 播放，它只在采集入口成功之后调用；结束提示音由 `voiceCaptureDidEnd` 播放，只在开始之后才会发生。所以输入法播提示音时，这个进程的麦克风授权一定已经是允许，`coreaudiod` 的检查直接通过，不会弹框。授权被拒绝时 TCC 也是直接返回拒绝，不会等。所以这个挂起是 CI 镜像的问题，不是用户 Mac 上提示音会卡住输入法的路径。

## Alternatives considered

- **把产品里的 `play` 挪到后台线程，或者加超时。** 理由最强的一点是：主线程上任何同步等 `coreaudiod` 的调用，理论上都可能让输入法卡顿。但这次的证据说明，阻塞只发生在麦克风授权未决定的时候，而产品里提示音一定在授权之后才播；NSSound 的委托回调和 `isPlaying` 状态也绑在调用线程的 run loop 上，挪线程要连带改完成回调的契约。在没有用户侧证据的情况下为 CI 现象改产品的线程模型，代价大于收益，所以不改。
- **在 CI 里预先写入 TCC 授权。** 系统 TCC.db 受 SIP 保护，改不了；用户 TCC.db 要的是这个 job 的 `Runner.Listener` 路径，每个 job 都不一样，而且写 TCC 属于改 runner 环境，不该由仓库的 workflow 去做。
- **整条测试在 CI 上直接跳过，或者只按 `GITHUB_ACTIONS` 判断。** 实现最简单，但 x86_64 runner 和开发机上这条测试是能真正跑播放断言的，按环境变量一刀切会白白丢掉这部分覆盖。现在的条件只在播放必然停在授权框上时才跳过。
- **只看默认输出设备有没有输入流，不查授权。** 收放一体的设备在真机上也存在（不少 USB 耳机就是），只要终端已经有麦克风授权，播放就不会卡；不查授权会多跳过这些本来能跑的机器。

## Consequences

Depot arm64 上 `voice-cue-player` 记为跳过，加载、回退和默认初始化的断言照常执行，CTest 输出里能看到跳过原因；播放和完成回调的时序断言在这台 runner 上不再覆盖，靠 x86_64 runner 和开发机覆盖。等 Depot 修好 runner 的 TCC 归属，测试会自动恢复执行播放部分，不用改代码。代价是测试多链接了 AVFoundation 和 CoreAudio。别的测试如果在这台 runner 上播放声音或打开音频 IO，也会撞上同样的授权框；遇到时用同样的预检判断，不要靠拉长超时。

## Verification

修复前，同一镜像上的诊断 job 抓到了上面三段栈和 TCC 日志。修复后，本机（Apple Silicon，麦克风已授权，默认输出是独立的扬声器设备）用和 CMake 相同的编译选项编出测试，2.7 秒内跑完全部播放断言，退出码 0。Depot arm64 上的结果以本 PR 的 `macOS 15 arm64` 检查为准：`voice-cue-player` 应显示为跳过，而不是超时。

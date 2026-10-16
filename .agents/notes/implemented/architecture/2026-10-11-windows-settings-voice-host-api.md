# Agent Note: 原生设置窗口语音页用到的 C 接口

Status: implemented

## Problem

原生设置窗口接下来要把语音页做成原生页。共享语音页在 Windows 上要的宿主能力有三样：「录音设备」分组里的「刷新设备」（Tauri 的 `list_voice_capture_devices`），识别服务和语音润色的「测试配置」（Tauri 的 `test_api_credential`，`voice.asr` 是豆包时走 WebSocket，其余整句上传服务走 HTTP，`voice.polish` 发一次最短的对话），和本地模型的列出、安装、取消、删除。窗口只能经 `msime_client.h` 的 C 接口调 Rust。

查下来，后两样已经在：在线翻译那一步把「测试配置」收进了 `msime_client_providers::credential::test_service`，Tauri（Windows、macOS）和 `msime_client_test_api_credential` 调的是同一个函数，里面就是 `credential::asr::test`、`credential::doubao::test` 和 `credential::probe::test_chat`；本地模型有 `msime_client_voice_local_models`、`_install`、`_cancel`、`_remove`，和 Tauri 一样调 `msime_client_providers::voice::local_models`。缺的只有列录音设备：`msime_host_api::voice_capture_devices` 是 Rust 函数，没有 C 导出，而且 `{backend,id,label}` 的形状是 Tauri 命令里自己拼的。

## Decision

- 形状收进 `msime_host_api::voice_capture_device_values(backend, devices)`，Tauri 的 Windows 和 macOS 分支都改调它，各自传 `"windows"`、`"macos"`。
- 新导出 `msime_client_voice_capture_devices(void)`：Windows 上返回 `[{backend:"windows",id,label}]`，`id` 是 `wasapi:` 加端点 id 每个 UTF-16 单元四位小写十六进制（`voice_capture::wasapi_device_id`，也就是 TSF 宿主打开的那种），至多 128 个，没有设备时是空数组。其他目标返回错误 `voice capture devices unavailable`：macOS 的 Tauri 用 `msime_host_macos` 列的 CoreAudio UID，Linux 用自己的 PulseAudio/PipeWire/ALSA 列表，host-api 在那里列出的 `coreaudio:`/`alsa:` 编码不是这两处页面保存的值，假装能用反而会存进错的设备。
- 枚举经 COM（cpal 在当前线程按 STA 初始化，线程已是 MTA 时沿用），头文件写明不要在界面线程上调用；原生窗口的 `run_host` 在线程池线程上跑，符合要求。
- 本地模型的根目录：Tauri 是 `app_data_dir()/voice-models`，Windows 上的标识按版本不同（full 是 `tauri.windows.conf.json` 的 `app.msime.windows`，其他版本由 `platform/windows.rs` 换成版本表的 `tauri_identifier`），即 `%APPDATA%\<identifier>\voice-models`。原生页从 `msime_edition.h` 的 `MSIME_EDITION_TAURI_IDENTIFIER` 取标识，把这个绝对路径作为 `root` 传给现有四个导出，管的就是同一批模型。
- 测试：`voice_capture_device_values` 的纯形状；导出在 Windows 上返回的每一项都是 `windows` 后端、`wasapi:` 加四位一组的十六进制、标签非空（机器上有几个设备就查几个，不打开设备）；「测试配置」导出补上语音页的几种不联网就回答的配置（豆包非 wss、上传服务缺 Key、http 转写地址、本地识别、http 润色地址）。

## Alternatives considered

- **给导出加一个 `backend` 参数，各平台都列** — 一个接口通吃；但 host-api 在 macOS 和 Linux 上列出的编码和那两处设置页保存的值不同，接上就会存进宿主打不开的设备，而那两个平台的设置不走这个导出。
- **原生窗口自己用 WASAPI 列设备、自己编码 id** — 不用动 Rust；但 id 编码和 64 单元的上限就有了两份，TSF 宿主认的是 host-api 那一份，哪天改了一边，原生窗口存的设备就打不开。
- **另加一个只做语音的「测试配置」导出** — 名字更直白；但 `msime_client_test_api_credential` 已经按 `service` 分派到同样的三个检查，再加一个只是第二个入口。

## Consequences

- 原生语音页能像共享页一样刷新并选择麦克风，存下的值与 Tauri 设置存的完全一样。
- Tauri 的录音设备命令行为不变，只是形状改由 host-api 产生。
- 本地模型导出的失败是 `local_model_*` 原样的错误串（如 `local_model_size_mismatch: ...`），Tauri 会把 size/checksum 归成 `local_model_checksum_mismatch`、unsafe_archive/missing_file 归成 `local_model_invalid_archive` 再给页面；原生页要按 `: ` 前的代码做同样的归并后再取共享页的文案（`packages/ui/src/voice/local-model-helpers.ts`），iOS 的 `LocalSpeechModelStore.swift` 也是这么做的。

## Verification

- `cargo test --locked -p msime-host-api --lib -- voice_capture credential_test_export` 通过（8 项，不联网）。
- `cargo build --release --locked -p msime-host-api`、`cargo check --locked -p msime-desktop` 通过，警告都是改动前就有的。
- 设置工程按 `/WX` 编译通过（`settings-test-build.ps1`），没有启动窗口。没有在真机上看录音设备列表在原生页里的样子：这一步还没有界面。

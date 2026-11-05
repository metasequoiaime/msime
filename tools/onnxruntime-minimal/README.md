# 精简构建的 ONNX Runtime

按键重排的 ONNX 后端（`crates/onnx-rerank`）链接的 ONNX Runtime 不是官方的预编译库，而是按 `required_operators_and_types.config` 精简构建的静态库：只编进重排模型用到的那 27 个算子和它们的类型，只读 ORT 格式的模型，不带 RTTI 和异常。链进程序后约 1.5 MB，官方安卓库是 34 MB。为什么用 ONNX Runtime、为什么精简构建，见 `.agents/notes/implemented/architecture/2026-11-01-onnx-rerank-backend.md`。

- 版本：ONNX Runtime 1.24.4（`v1.24.4`，提交 `2d924974ef147392ced8409d36bd6d2e7fcc8a74`），MIT 许可。与生成 ORT 格式模型的 Python 包同一个版本，`ort` crate 用 `api-24` 对齐它。
- 算子清单：chinese-ime-lm 的 `neural/choice/export_step.py` 导出模型时写出各自的清单；这里的是 `rerank-v1` 两个模型（8x448 与 6x256）一起生成的并集。现在随包的 `rerank-v2` 结构没变，两个模型导出的清单与 v1 逐字相同（chinese-ime-lm 的 `rerank-v2` release 附件），所以这份清单和用它编的静态库不用跟着换。两个模型的来源：GitHub Release [`metasequoiaime/chinese-ime-lm` 的 `rerank-v1`](https://github.com/metasequoiaime/chinese-ime-lm/releases/tag/rerank-v1)（文件名带尺寸前缀，如 `rerank-8x448.ort`），同一批文件也在 Hugging Face：[`msime/pinyin-ime-keystroke-reranker-25M`](https://huggingface.co/msime/pinyin-ime-keystroke-reranker-25M)（8x448）和 [`msime/pinyin-ime-keystroke-reranker-8M`](https://huggingface.co/msime/pinyin-ime-keystroke-reranker-8M)（6x256），那里是原名（`lm_step.ort`、`model.json`、`vocab.json`）并附 `LICENSE` 与 `NOTICE`；每个模型导出时写出的清单也在那里。6x256 的 GELU 是 tanh 近似，比 8x448 多一个 `Tanh`。换模型时若清单变了，按新的并集重编所有平台。
- 产物：每个平台一个静态库，gzip 压缩后作为 GitHub Release `onnxruntime-minimal-<版本>-r<序号>` 的附件发布，同时附上算子清单、ONNX Runtime 的 `LICENSE` 与 `ThirdPartyNotices.txt` 和 `SHA256SUMS`。`resources/onnxruntime-minimal.lock.json` 按 Rust 目标三元组锁住每个附件的地址、SHA-256 和大小，以及解压后库的文件名、SHA-256 和大小；`scripts/fetch_onnxruntime.py` 下载核对后给出 `ORT_LIB_LOCATION` 要指的目录（见下面「用它构建 msime」）。锁里还记着算子清单的 SHA-256，`scripts/test-fetch-onnxruntime.py` 核对它：改了清单而没有重编、重新发布，检查就失败。

## 发布了哪些

`onnxruntime-minimal-1.24.4-r3`（r2 的全部附件原样带过来，只多了鸿蒙 arm64；r2 补上了 Linux x86_64，r1、r2 仍在，指向它们的旧锁照样能下载）：

| 目标三元组 | 附件 | `build.sh` 平台 / 构建方式 |
| --- | --- | --- |
| `aarch64-apple-darwin` | `libonnxruntime-aarch64-apple-darwin.a.gz` | `macos-arm64`，最低 macOS 13.0 |
| `x86_64-apple-darwin` | `libonnxruntime-x86_64-apple-darwin.a.gz` | `macos-x86_64`，最低 macOS 13.0 |
| `aarch64-apple-ios` | `libonnxruntime-aarch64-apple-ios.a.gz` | `ios-arm64`，最低 iOS 15.0 |
| `aarch64-apple-ios-sim` | `libonnxruntime-aarch64-apple-ios-sim.a.gz` | `ios-sim-arm64` |
| `aarch64-linux-android` | `libonnxruntime-aarch64-linux-android.a.gz` | `android-arm64`，API 28 |
| `x86_64-linux-android` | `libonnxruntime-x86_64-linux-android.a.gz` | `android-x86_64`，API 28 |
| `aarch64-unknown-linux-ohos` | `libonnxruntime-aarch64-unknown-linux-ohos.a.gz` | `ohos-arm64`，DevEco Studio 自带的 OpenHarmony NDK（clang 15），r3 新增 |
| `aarch64-unknown-linux-gnu` | `libonnxruntime-aarch64-unknown-linux-gnu.a.gz` | `linux-aarch64`，glibc 2.36 |
| `x86_64-unknown-linux-gnu` | `libonnxruntime-x86_64-unknown-linux-gnu.a.gz` | `linux-x86_64`，glibc 2.36，`onnx-rerank.yml` 在 CI 里编出并上传（r2 新增） |
| `x86_64-pc-windows-msvc` | `onnxruntime-x86_64-pc-windows-msvc.lib.gz` | `onnxruntime-minimal.yml` 的 x64，动态 CRT（/MD） |
| `aarch64-pc-windows-msvc` | `onnxruntime-aarch64-pc-windows-msvc.lib.gz` | `onnxruntime-minimal.yml` 的 arm64-crt-static，静态 CRT（/MT） |

没有发布的：

- 32 位（`armv7-linux-androideabi` 等）：32 位设备不跑 ONNX 重排，照旧用随包模型。`build.sh` 留着 `android-armv7`，但不发布。
- `x86_64-apple-ios`（Intel Mac 上的模拟器）：没有产物。
- 鸿蒙 x86_64（模拟器）：编不出来。MLAS 的 `x86_64/cvtfp16Avx.S` 用了 AVX-NE-CONVERT 指令，OpenHarmony NDK 的 clang 15 汇编器不认（clang 16 起才认），ONNX Runtime 没有不编它的开关。模拟器上照旧用随包模型。

换算子清单、换 ONNX Runtime 版本或改构建方式时，重编全部平台，发一个新的 `-r<序号>`（不覆盖旧的附件，旧的锁仍能下载），再更新锁里的 `release_tag` 和每个目标的条目。只补目标时（r2 补 Linux x86_64，r3 补鸿蒙），新的 `-r<序号>` 带上前一个的全部附件，原样不动，再加新的那个、重新生成 `SHA256SUMS`。发布用 `gh release create <标签> --latest=false`，不让它顶掉 msime 的正式版本成为 Latest。

Linux x86_64 的库从 CI 取，不在 Mac 上编：Apple Silicon 上 Docker 的 `linux/amd64` 是模拟的，在 Mac Studio 上这样编过一次，没有编成；x86_64 的 runner 上是原生构建。`onnx-rerank.yml` 在 x86_64 的 runner 上、用同一个 bookworm 镜像跑 `build.sh linux-x86_64`，把 `libonnxruntime.a.gz` 作为附件 `libonnxruntime-x86_64-unknown-linux-gnu` 上传（留 14 天），同一次运行里还拿它跑 golden 测试和 host-api 的挂载测试；发布时用 `gh run download <运行> -n libonnxruntime-x86_64-unknown-linux-gnu` 取下来，改名为 `libonnxruntime-x86_64-unknown-linux-gnu.a.gz`。r2 的那一份来自运行 41406232655。

## 构建

```sh
git clone --depth 1 --branch v1.24.4 https://github.com/microsoft/onnxruntime.git <源码>
tools/onnxruntime-minimal/build.sh <源码> <工作目录> <平台>
```

平台有 `android-arm64`、`android-armv7`、`android-x86_64`、`ohos-arm64`、`ios-arm64`、`ios-sim-arm64`、`macos-arm64`、`macos-x86_64`、`linux-aarch64`、`linux-x86_64`。苹果、安卓和鸿蒙平台要在 macOS 上编（Xcode、Android NDK 28、DevEco Studio 自带的 OpenHarmony NDK，单独的命令行 SDK 用 `MSIME_OHOS_NDK` 指到它的 `native` 目录）；Linux 平台在 Docker 里用 msime 发 Linux 版的同一个镜像编，静态库链接的 glibc 版本才和 msime 的二进制一致。CMake 4 去掉了对 3.5 以下旧版本的兼容，ONNX Runtime 拉进来的一些依赖还写着旧版本，所以带 `CMAKE_POLICY_VERSION_MINIMUM=3.5`。`JOBS` 控制并发，默认 4：在 Mac Studio 上编时要给它上面的 CI runner 留核。

macOS 两个平台直接给 CMake `CMAKE_OSX_DEPLOYMENT_TARGET=13.0`：ONNX Runtime 的 `build.py` 只在 `--ios` 或 `--build_apple_framework` 时才把 `--apple_deploy_target` 交给 CMake，普通 macOS 构建会忽略它，目标文件的最低版本变成编译机的系统版本。发布前用 `otool -l libonnxruntime.a | grep -A3 LC_BUILD_VERSION` 核对 `minos`。

鸿蒙不在 ONNX Runtime 官方支持的平台里，`build.py` 也没有鸿蒙选项，`build.sh` 按普通的交叉编译把 NDK 的 `ohos.toolchain.cmake` 交给它，再补四处构建参数，都不改 ONNX Runtime 的源码：关掉 `-Wunused-command-line-argument`（工具链给每条命令都加 `--gcc-toolchain`，ONNX Runtime 按 `-Werror` 编）；用 cc1 的 `-target-feature +bf16`（MLAS 在 aarch64 Linux 上无条件用 `bfloat16_t`，NDK 的 clang 15 只在开了 bf16 时才定义它；指令集基线仍是 armv8-a）；强制包含 `ohos-compat.h`（鸿蒙的 musl 没有 `pthread_setaffinity_np`）；`onnxruntime_ENABLE_CPUINFO=OFF`（cpuinfo 的 CMake 不认识 OHOS 这个系统名，编出的库缺 ARM Linux 探测代码，链接失败；关掉后 ONNX Runtime 按 `getauxval` 的 hwcap 判断指令集扩展）。理由都写在 `build.sh` 和那个头文件里。同一份 ONNX Runtime 源码在鸿蒙上编得出来，有旁证：HAP 里的 sherpa-onnx HAR 带着它自己编的 `libonnxruntime.so`。

Windows 要在 Windows 上用 MSVC 编，配方在 `.github/workflows/onnxruntime-minimal.yml`：改了这个目录的 Pull Request 上自动编 x64 和 arm64 两份，作为构建附件上传。在 CI 之外自己编时用同样的参数，有两点要注意：

- 关掉调试信息（`CMAKE_MSVC_DEBUG_INFORMATION_FORMAT=` 留空）。`build.py` 在 MinSizeRel 下也默认 `/Zi`，静态库不带 PDB，`.debug$S` 只是让库变大（x64 从 374 MB 到 111 MB）并让链接它的每个程序报 LNK4099。
- arm64 用静态 CRT（`--arm64 --enable_msvc_static_runtime`，/MT）。`platforms/windows/Build-RustOutputs.ps1` 给 arm64 的 host-api 开着 `+crt-static`，用动态 CRT（/MD）编的库在那里链接失败（LNK1120）。x64 的 host-api 用动态 CRT，库也用 /MD。

## 用它构建 msime

`ORT_LIB_LOCATION` 指向放着 `libonnxruntime.a`（MSVC 目标是 `onnxruntime.lib`）的目录。锁里有的目标用下载脚本取，它打印的就是这个目录（默认 `target/onnxruntime/<三元组>`）：

```sh
export ORT_LIB_LOCATION=$(python3 scripts/fetch_onnxruntime.py --target aarch64-apple-darwin)
cargo build -p msime-host-api --features onnx-rerank
```

`--list` 列出锁里的目标。锁里没有的目标，指向上面 `build.sh` 输出的那个平台的目录（`<工作目录>/out/<平台>`）。

各平台的接入：iOS 是 `platforms/ios/build-native.sh`，Linux 是 `platforms/linux/package-container.sh`，Android 与鸿蒙是各自的 `build-native.sh`（鸿蒙只编 arm64，并用 `ORT_CXX_STDLIB=c++_shared` 让 `ort-sys` 链 NDK 的 libc++，它默认链的 libstdc++ 鸿蒙没有）；Windows 是 `platforms/windows/Build-RustOutputs.ps1`，只给 x64 宿主取 `x86_64-pc-windows-msvc` 的库、开 `onnx-rerank`（逐键重排只在 Server 的会话里跑，Server 是 x64 程序）。`aarch64-pc-windows-msvc` 的库发布了，但现在没有构建用它：arm64 宿主只给 TIP 的原生那一半加载，TIP 的会话不挂重排。

CI 的 `.github/workflows/onnx-rerank.yml` 在 Linux x86_64 上用 `build.sh` 编出库（按算子清单和构建脚本缓存，并作为附件上传，见上），跑 `msime-onnx-rerank` 的 golden 测试和 host-api 的挂载测试。Linux 的发布包不用它，由 `platforms/linux/package-container.sh` 按锁下载。没有库时 `cargo clippy` / `cargo check` 也能开着这些特性做类型检查，`ort-sys` 把找不到库的报错推迟到链接。

`ort-sys` 的构建脚本会把这个库复制进自己的输出目录，之后换了库文件也不会重新复制；换库之后先 `cargo clean -p ort-sys`。

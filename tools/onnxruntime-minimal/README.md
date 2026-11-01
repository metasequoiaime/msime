# 精简构建的 ONNX Runtime

按键重排的 ONNX 后端（`crates/onnx-rerank`）链接的 ONNX Runtime 不是官方的预编译库，而是按 `required_operators_and_types.config` 精简构建的静态库：只编进重排模型用到的那 27 个算子和它们的类型，只读 ORT 格式的模型，不带 RTTI 和异常。链进程序后约 1.5 MB，官方安卓库是 34 MB。为什么用 ONNX Runtime、为什么精简构建，见 `.agents/notes/implemented/architecture/2026-11-01-onnx-rerank-backend.md`。

- 版本：ONNX Runtime 1.24.4（`v1.24.4`，提交 `2d924974ef147392ced8409d36bd6d2e7fcc8a74`），MIT 许可。与生成 ORT 格式模型的 Python 包同一个版本，`ort` crate 用 `api-24` 对齐它。
- 算子清单：chinese-ime-lm 的 `neural/choice/export_step.py` 导出模型时写出各自的清单；这里的是 `rerank-v1` 两个模型（8x448 与 6x256）一起生成的并集。6x256 的 GELU 是 tanh 近似，比 8x448 多一个 `Tanh`。换模型时若清单变了，按新的并集重编所有平台。
- 产物：每个平台一个 `libonnxruntime.a`（gzip 压缩）。计划发布后由 `resources/onnxruntime-minimal.lock.json` 按 Rust 目标三元组锁住地址、SHA-256 和大小，由 `scripts/fetch_onnxruntime.py` 下载核对后给出 `ORT_LIB_LOCATION` 要指的目录；这两个文件还没有，现在只能按下面的步骤自己编。

## 构建

```sh
git clone --depth 1 --branch v1.24.4 https://github.com/microsoft/onnxruntime.git <源码>
tools/onnxruntime-minimal/build.sh <源码> <工作目录> <平台>
```

平台有 `android-arm64`、`android-armv7`、`android-x86_64`、`ios-arm64`、`ios-sim-arm64`、`macos-arm64`、`macos-x86_64`、`linux-aarch64`、`linux-x86_64`。苹果和安卓平台要在 macOS 上编（Xcode、Android NDK 28）；Linux 平台在 Docker 里用 msime 发 Linux 版的同一个镜像编，静态库链接的 glibc 版本才和 msime 的二进制一致。CMake 4 去掉了对 3.5 以下旧版本的兼容，ONNX Runtime 拉进来的一些依赖还写着旧版本，所以带 `CMAKE_POLICY_VERSION_MINIMUM=3.5`。`JOBS` 控制并发，默认 4：在 Mac Studio 上编时要给它上面的 CI runner 留核。

Windows 要在 Windows 上用 MSVC 编，鸿蒙不在 ONNX Runtime 官方支持的平台里，两者都还没有产物。

## 用它构建 msime

`ORT_LIB_LOCATION` 指向上面 `build.sh` 输出的那个平台的目录（里面有 `libonnxruntime.a`）：

```sh
export ORT_LIB_LOCATION=<工作目录>/out/macos-arm64
cargo build -p msime-host-api --features onnx-rerank
```

CI 的 `.github/workflows/onnx-rerank.yml` 就是这样在 Linux x86_64 上编出库、跑 `msime-onnx-rerank` 的 golden 测试和 host-api 的挂载测试。没有库时 `cargo clippy` / `cargo check` 也能开着这些特性做类型检查，`ort-sys` 把找不到库的报错推迟到链接。

`ort-sys` 的构建脚本会把这个库复制进自己的输出目录，之后换了库文件也不会重新复制；换库之后先 `cargo clean -p ort-sys`。

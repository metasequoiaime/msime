# Windows GNU build checks

```sh
bash platforms/windows/build-cross.sh x64
CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
  cargo check -p msime-desktop --target x86_64-pc-windows-gnu --locked
```

The script uses the repository's pinned vcpkg manifest, builds the Rust host DLL, and compiles/links the native Server, TSF DLL, supported UI demos and tests. It sets `MSIMEUI_BUILD_HANDWRITING_DEMO=OFF` because that demo needs the Windows SDK's C++/WinRT headers, which MinGW does not carry; normal CMake builds default that option to ON. 脚本在本地测试目录暂存实际生产编译器的三个 MinGW 运行时 DLL，并验证 PE 架构，不运行 Windows 程序、注册 TSF 或验收音频硬件；这些文件不是发行包。`../../run-tests-wine.sh` 优先使用已暂存的运行时执行 Wine 套件，`../../stage-runtime.sh` 无模式参数时补齐完整导入图与可复制到 Windows 的验证目录。

The TSF dynamic thread-local strings have a single source definition while retaining per-thread storage. MinGW gets the two property GUID definitions absent from its UUID archive; values match Microsoft's windows-sys 0.61.2 bindings. MSVC continues to use the SDK UUID library. Unicode startup flags are applied to the UI demos, and Server Unicode macros apply to both compilers.

For x86, Homebrew's i686 MinGW uses SJLJ exceptions while Rust's `i686-pc-windows-gnu` needs DWARF unwinding, so `build-cross.sh x86` refuses that combination up front. Use `bash platforms/windows/build-cross-container.sh x86`, which runs the same script against Debian's DWARF-built i686 MinGW inside a container.

The candidate initialization regression (CTest `windows-candidate-initialization`) also runs on a host compiler, which is how it gets sanitizer coverage. It needs the header-only JSON dependency, the `common` directory and the `src` subdirectories that `platforms/windows/CMakeLists.txt` puts on the include path, and the Cargo-built host library that `ChineseTextConversion.cpp` calls into:

```sh
cargo build -p msime-host-api --locked
c++ -std=c++17 -Wall -Wextra -Werror -fsanitize=address,undefined \
  -Iplatforms/windows/common \
  -Iplatforms/windows/src/candidate -Iplatforms/windows/src/input \
  -Iplatforms/windows/src/ipc -Iplatforms/windows/src/system \
  -Icrates/host-api/include -Ishared/contracts \
  -I/opt/homebrew/include \
  platforms/windows/tests/ui/candidate_initialization.cpp \
  platforms/windows/src/input/ChineseTextConversion.cpp \
  target/debug/libmsime_host_api.a -o target/candidate-initialization
./target/candidate-initialization
```

All fixtures here use synthetic data, so they cover the initialization order rather than real Engine output; the Engine-backed paths are covered by the session suites that link the real host library.

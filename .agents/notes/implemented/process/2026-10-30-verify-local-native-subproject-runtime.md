# Agent Note: 本地门禁把宿主运行时也放进 Windows 子项目的输出目录

Status: implemented

## Problem

`scripts/verify-local.sh` 在 Windows 原生构建树（`MSIME_NATIVE_BUILD`，默认 `target/win-full`）上先由 `build_windows_host_library` 重新编 `msime_host_api.dll`，复制到 `$MSIME_NATIVE_BUILD/Debug`，再 `cmake --build` 并跑 `ctest -C Debug`。`tsf` 和 `shared-voice` 是 `add_subdirectory` 的子项目，它们的测试在各自的 `tsf/Debug`、`shared-voice/Debug` 里运行，拿不到这个运行时：`msime-tsf-class-factory` 自己 `LoadLibrary` TSF DLL，加载失败后静默返回 1；`shared-doubao-auth` 这类进程缺 DLL 时一直挂到 ctest 超时。两者都不在 `known-failures.txt` 里，每次都算成新失败。维护者在 Windows 本机实测：补齐前 287 个原生测试失败 3 个（`windows-session` 是已知失败），补齐后只剩它。

修复（`3ce6a537b7`）提交在 #10206 的分支上，却是在 #10206 合并之后才推上去的。GitHub 只合并合并那一刻的分支头，之后推到已合并分支的提交不会进 develop，也不会有提示，这一修复就此丢失。

## Decision

`build_windows_host_library` 复制完 `$MSIME_NATIVE_BUILD/Debug` 之后，对构建树第一层每个有 `CTestTestfile.cmake` 的子目录先建 `Debug` 再复制同一个运行时。没有登记测试的子目录不动。

按 `CTestTestfile.cmake` 找子项目，而不是原提交的 `"$MSIME_NATIVE_BUILD"/*/Debug`：这个函数在 `cmake --build` 之前运行，刚 configure 完的构建树里子项目还没有 `Debug` 目录，而 `CTestTestfile.cmake` 是 configure 时就生成的。原写法在没有匹配时通配符原样留下，循环最后一次 `[ -d ]` 的非零状态成了函数的返回值，门禁会报成「cargo build -p msime-host-api (configured native host library)」失败；`scripts/test-verify-local-native-host-library.py` 的夹具里没有子项目，原写法让已有 9 项中的 5 项失败。

## Alternatives considered

- **把复制挪到 `cmake --build` 之后**：那时 `*/Debug` 都在了，原循环可以直接用。但运行时由 `build_windows_host_library` 产出和放置（`Debug` 那一份已经在这里，`test-verify-local-native-host-library.py` 也只测这个函数），拆一半到调用方后，放置规则分在两处、夹具测不到子项目那一半；`CTestTestfile.cmake` 在 configure 时就有，留在原处就能覆盖第一次运行。
- **在 CMake 里给子项目的测试加 `POST_BUILD` 复制**：发版构建（`Build-Client.ps1`）有自己的运行时放置，不需要它，而且要改两个子项目的 CMake 和宿主库路径的传递，只为本地门禁不划算。

## Consequences

收益：Windows 本机跑 `verify-local.sh` 时，`tsf` 和 `shared-voice` 的测试能加载宿主运行时，不再每次报两项新失败或挂到超时；第一次 configure 后的运行也成立。

代价：构建树里每个登记了测试的第一层子目录都会多一个 `Debug/msime_host_api.dll`，包括本来不需要它的子项目。更深层的子目录（如 `tests/native-pipe`）不覆盖，维护者实测时没有需要它的测试。

CI 不走这条路径：没有工作流运行 `verify-local.sh`，Windows 上的 ctest（`ci-platforms.yml` 的 Win7 设置、面板、兼容 API、状态目录）各用自己的构建树，并在 `ctest` 前显式 `Copy-Item target/debug/msime_host_api.dll` 到输出目录。所以这次丢失没有让 CI 变红，也不需要改工作流。

## Verification

`scripts/test-verify-local-native-host-library.py` 新增 `test_subprojects_with_tests_get_the_runtime_before_their_first_build`：夹具构建树里 `tsf`、`shared-voice` 有 `CTestTestfile.cmake` 但还没有 `Debug`，另有一个没有测试的子目录；断言函数成功、两个子项目拿到本次产出的运行时、没有测试的子目录不建 `Debug`。它在 develop 现有脚本上失败（子项目没有运行时），在本篇的写法上 10 项全过；把原提交的循环套到 develop 上，原有 5 项失败。`bash -n` 与 `scripts/run-checks.sh` 通过。

没验证：没在 Windows 本机跑完整原生 ctest，本篇依据的 286 通过是维护者在原提交里记的结果。

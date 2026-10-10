# Agent Note: Windows 构建方暂存匹配的测试运行时

Status: implemented

## Problem

Wine 原先在 x86 验证前另构建 amd64 编译镜像并复制运行时，x64 则要求本机 MinGW。Windows 产物可以由原生 ARM64 容器生成，但验证仍准备另一套宿主架构的工具链，或因本机缺少编译器而跳过。消费方重新准备编译器既有重复成本，也不能保证与生成产物的编译器相同。

## Decision

`build-cross.sh` 完成链接后调用现有 `stage-runtime.sh <arch> --runtime-only <output>`，从实际生产编译器暂存 libstdc++、libwinpthread 和按目标选择的 libgcc 展开器，使用同一工具链的 objdump 检查每项 PE 架构，再以 `copy_if_different` 写入现有产物目录。x86 为 DWARF，x64 为 SEH；不同 edition 继续使用各自已有输出目录。

`stage-runtime.sh` 复用同一依赖定位、架构检查和复制函数；默认模式仍检查完整导入图与测试程序，生成 Windows 验证目录。运行时定位保留原有目录优先级，目录找不到时才向同一编译器询问 `-print-file-name`，不查其他工具链。该参数的 [GCC 官方说明](https://gcc.gnu.org/onlinedocs/gcc/Developer-Options.html#index-print-file-name) 约束按编译器的链接搜索路径定位库。

Wine 优先读取构建目录内完整的三个匹配运行时，不再为这些产物构建另一份编译镜像，不要求生产编译器仍留在主机上。旧目录缺少完整运行时则保留原有回退。Wine 执行镜像仍为 amd64，测试循环、超时、资源参数和 Rust 测试准备方式保持原有行为。

## Alternatives considered

- Wine 按 daemon 架构再构建编译镜像：可以消除 amd64 编译工具仿真，但仍需镜像准备，并把运行时版本选择留给消费方。
- Wine 只从本机工具链收集：实现简单，但容器产物的生产工具链未必存在于本机，x86 的 SJLJ/DWARF 也不能互换。
- 直接调用完整 `stage-runtime.sh`：可同时检查全部导入，但绑定 full edition 的固定测试目录，并为单纯准备 Wine 运行时增加无关目录整理；运行时模式复用已有检查函数，完整模式仍可显式调用。

## Consequences

新构建目录多出三个本地测试 DLL，构建失败或运行时缺失会明确失败；不同编译器的 PE 架构仍由生产方检查。Wine 减少重复的编译环境准备，容器构建的 x64 也能走已有运行时路径。旧构建目录仍可能需要原有工具链准备；重新构建后使用新路径。目录用于本地验证，不是发行包，不能把暂存 DLL 当作带完整分发义务说明的安装包。

检索活跃笔记后，[原生交叉编译宿主](2026-10-09-windows-native-cross-host.md) 与本篇部分重叠，保留原生工具选择决定并互链；固定工具链、Cargo 缓存和头文件别名涉及其他准备成本，不被本篇替代。未发现运行时所有权已有独立归属。

## Verification

原实现上最初 5 项回归共 6 个断言失败：缺少仅运行时模式、Wine x86 仍准备编译镜像、x64 仍查询不可用的本机工具链。补充完整模式系统依赖回归后，旧分类在 `CRYPT32.dll` 处失败；扩展明确的系统 DLL 清单后最终 6 项通过，未知依赖仍拒绝。覆盖双架构暂存、缺失依赖与错误 PE 架构、完整模式构建要求、系统与未知 DLL 分类、Wine 只准备执行镜像并使用已暂存文件。Shell 语法、diff 和笔记校验通过。

ARM64 容器内真实 x86/x64 构建均退出 0，完成 Rust 宿主 DLL、TSF、Server 与全部原生测试链接，并分别暂存三个 PE32、PE32+ 运行时；六个 DLL 与生产编译器返回的文件逐字节匹配。`stage-runtime.sh x86` 和 `x64` 完整导入图检查也退出 0。

专用 amd64 Wine 10 镜像构建退出 0。x86/x64 线程与 C++ 异常展开合成程序均使用暂存 DLL 输出 `synthetic runtime ok`。实际 Wine 脚本的 x86 C++/工具套件为 120 PASS、9 FAIL，x64 为 121 PASS、8 FAIL，两个脚本均退出 0；脚本只输出测试结果，退出码不证明全绿。失败尚未全部建立修复前基线，不能统称为已知失败。已定向观察到共享宿主操作错误、ClipboardHistory reparse 清理断言、Wine 符号链接删除 Access denied，以及 stroke 测试缺少命令行参数；完整失败处理是后续独立工作，不向 known-failures 增项。本切片的动态运行时验证通过，不声称整个 Wine 套件通过。宿主 `rustup target list --installed` 没有 Windows GNU 标准库，Wine Rust 测试未构建，不能归为 linker 阶段失败或计入通过。

本地最新 quick 退出 0：102 项 Windows 主机测试、6 项新增回归、常驻契约、Rust workspace 与共享 Apple bridge 编译通过。Windows 原生/pipe-only 阶段缺少本机工具链跳过，x86 语法检查缺少本机 i686 编译器，其他平台按 scope 跳过，桌面包因缺少 macOS 资源跳过。未运行系统输入法验收，不生成发行安装包。

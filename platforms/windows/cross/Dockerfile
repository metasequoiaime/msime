# Cross-build the Windows targets for an architecture this host has no working
# toolchain for.
#
# x86 is the reason this exists. The 32-bit TSF DLL is loaded into every 32-bit
# host process, and it had never been linked: `build-cross.sh x86` refuses when
# the i686 MinGW uses SJLJ exceptions, because Rust's i686-pc-windows-gnu needs
# DWARF unwinding, and the i686 toolchain Homebrew ships is SJLJ. That is a
# property of one machine, not of the architecture - Debian's i686 MinGW is
# built `--disable-sjlj-exceptions --with-dwarf2`, so the same build works here.
#
# 编译器和 vcpkg 工具使用 Docker daemon 的原生 Linux 架构，输出仍为 x86/x64 PE。
FROM debian:trixie-slim
RUN apt-get update -qq \
 && apt-get install -y -qq --no-install-recommends \
    g++-mingw-w64-i686 g++-mingw-w64-x86-64 \
    build-essential cmake ninja-build pkg-config \
    curl ca-certificates git zip unzip tar python3 \
 && rm -rf /var/lib/apt/lists/*
# Debian ships win32 and posix thread variants side by side and leaves the
# unsuffixed name pointing at win32, whose C++ standard library has no
# std::thread. The project uses it, so select posix explicitly.
RUN update-alternatives --set i686-w64-mingw32-gcc /usr/bin/i686-w64-mingw32-gcc-posix \
 && update-alternatives --set i686-w64-mingw32-g++ /usr/bin/i686-w64-mingw32-g++-posix \
 && update-alternatives --set x86_64-w64-mingw32-gcc /usr/bin/x86_64-w64-mingw32-gcc-posix \
 && update-alternatives --set x86_64-w64-mingw32-g++ /usr/bin/x86_64-w64-mingw32-g++-posix
# Linux 区分大小写，单次扫描为 MinGW 头文件补齐首字母大写的相对链接。
# 使用已经安装的 Python，避免在 amd64 仿真中为每个文件启动多个外部命令。
COPY add-header-aliases.py /tmp/msime-add-header-aliases.py
RUN python3 /tmp/msime-add-header-aliases.py \
      /usr/share/mingw-w64/include /usr/i686-w64-mingw32/include /usr/x86_64-w64-mingw32/include \
    && rm /tmp/msime-add-header-aliases.py

# 与仓库声明使用同一固定工具链，避免临时容器另装编译器、组件和目标标准库。
# 先确认下载成功，避免 curl 失败被管线末端的空 sh 吞掉并缓存空安装层。
RUN curl -sSf https://sh.rustup.rs -o /tmp/msime-rustup-init.sh \
    && sh /tmp/msime-rustup-init.sh -y --profile minimal --default-toolchain 1.97.1 --component rustfmt --component clippy \
    && rm /tmp/msime-rustup-init.sh
ENV PATH=/root/.cargo/bin:$PATH
RUN rustup target add i686-pc-windows-gnu x86_64-pc-windows-gnu
ENV VCPKG_DISABLE_METRICS=1

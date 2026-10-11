# Anthropic OSS Scanner 的构建镜像：anthropics/oss-scanner 中的 projects/msime 登记项指向这里。构建 Rust workspace（引擎、输入运行时、C 宿主 ABI、工具）和 Linux 原生宿主（IBus 引擎与 Fcitx5 插件，C++），做法与 platforms/linux/build-container.sh 相同。Windows、macOS 和移动端宿主需要各自的 SDK，这里不构建，但它们共用的 Rust crate 都在其中。
FROM rust:1.97.1-bookworm@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates git cmake ninja-build pkg-config python3 clang \
      libasound2-dev libssl-dev \
      libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev \
      libibus-1.0-dev libxkbcommon-dev nlohmann-json3-dev \
      libwayland-dev wayland-protocols libx11-dev libxfixes-dev libxrandr-dev \
      libfcitx5core-dev libfcitx5utils-dev libfcitx5config-dev libcairo2-dev \
      dbus dbus-bin \
    && rm -rf /var/lib/apt/lists/*

# 扫描器约定：检出的源码放在镜像内的 /src
COPY . /src
WORKDIR /src

ENV CARGO_TARGET_DIR=/src/target \
    RUSTFLAGS="-C force-frame-pointers=yes" \
    CFLAGS="-g -fno-omit-frame-pointer" \
    CXXFLAGS="-g -fno-omit-frame-pointer"

# 安装 rust-toolchain.toml 固定的工具链并预取全部 crate：扫描阶段没有网络。
RUN rustup show active-toolchain && cargo fetch --locked

# Rust workspace（默认成员：全部库 crate 和命令行工具），同时编译测试。
RUN cargo build --locked \
    && cargo build --locked -p msime-dict-builder -p msime-pack-tool -p msime-backend-engine \
    && cargo test --locked --no-run

# Linux 原生宿主（IBus + Fcitx5），链接 debug 版宿主库。
RUN cargo build --locked -p msime-host-api \
    && cmake -S platforms/linux -B /src/build/linux -G Ninja \
         -DCMAKE_BUILD_TYPE=Debug \
         -DMSIME_ENABLE_FCITX5=ON \
         -DMSIME_HOST_LIBRARY=/src/target/debug/libmsime_host_api.so \
    && cmake --build /src/build/linux -j"$(nproc)"

# 跑一遍开销小的测试，确认镜像可用；测试失败不中断构建。
RUN (cargo test --locked --no-fail-fast -- --test-threads="$(nproc)" || echo "WARNING: cargo test reported failures") \
    && (ctest --test-dir /src/build/linux --output-on-failure -j"$(nproc)" --timeout 300 || echo "WARNING: ctest reported failures")

FROM rust:1.97.1-bookworm@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97
RUN apt-get update && apt-get install -y --no-install-recommends cmake ninja-build pkg-config libasound2-dev libibus-1.0-dev libxkbcommon-dev nlohmann-json3-dev dbus ibus ibus-gtk3 gir1.2-gtk-3.0 xvfb xauth xdotool x11-xkb-utils python3-pyqt5 python3-pyqt6 qtwayland5 qt6-wayland sway libwayland-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /source

# Paths are relative to the build context, which is platforms/linux/tests. Both
# files used to sit beside this Dockerfile; the move that split tests into
# per-responsibility directories left them in tools/ and runtime/.
COPY tools/virtual-keyboard-unstable-v1.xml runtime/wayland_keyboard.c /tmp/msime-wayland-driver/
RUN cd /tmp/msime-wayland-driver && wayland-scanner client-header virtual-keyboard-unstable-v1.xml virtual-keyboard-client.h && wayland-scanner private-code virtual-keyboard-unstable-v1.xml virtual-keyboard-protocol.c && cc -Wall -Wextra -Werror wayland_keyboard.c virtual-keyboard-protocol.c $(pkg-config --cflags --libs wayland-client) -o /usr/local/bin/msime-test-wayland-keyboard && rm -rf /tmp/msime-wayland-driver

# 仓库工具链要求这两个组件，预装后临时容器不再补装；保留系统包和 Wayland 驱动缓存。
RUN rustup component add rustfmt clippy

#!/usr/bin/env bash
set -euo pipefail
if [[ ${MSIME_ISOLATED_LINUX_TEST:-} != 1 ]]; then
  echo "skipped: Wayland canvas 仅在专用隔离环境中运行"
  exit 77
fi
# 容器内构建以 root 运行，合成器必须使用普通用户；截图先留在该用户的临时目录。
if [[ $(id -u) == 0 ]]; then
  user_output=$(mktemp -d /tmp/msime-canvas-user.XXXXXX)
  chown nobody "$user_output"
  trap 'rm -r -- "$user_output"' EXIT
  result=0
  MSIME_TEST_CANVAS_ARTIFACTS="$user_output/artifacts" \
    runuser -u nobody -- bash "$0" "$@" || result=$?
  if [[ -n ${MSIME_TEST_CANVAS_ARTIFACTS:-} && -d "$user_output/artifacts" ]]; then
    mkdir -p "$MSIME_TEST_CANVAS_ARTIFACTS"
    cp -R "$user_output/artifacts/." "$MSIME_TEST_CANVAS_ARTIFACTS/"
  fi
  exit "$result"
fi
runtime=$(mktemp -d /tmp/msime-canvas-wayland.XXXXXX)
chmod 700 "$runtime"
export XDG_RUNTIME_DIR="$runtime"
export MSIME_TEST_CANVAS_ROOT="$runtime/profile"
export XDG_CACHE_HOME="$runtime/cache" XDG_CONFIG_HOME="$MSIME_TEST_CANVAS_ROOT/config"
export XDG_DATA_HOME="$MSIME_TEST_CANVAS_ROOT/data"
mkdir -p "$XDG_CACHE_HOME" "$XDG_CONFIG_HOME/fcitx5/conf" "$XDG_DATA_HOME"
unset DISPLAY
export WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1
compositor_pid=
cleanup() {
  if [[ -n "$compositor_pid" ]]; then
    kill "$compositor_pid" 2>/dev/null || true
    wait "$compositor_pid" 2>/dev/null || true
  fi
  rm -r -- "$runtime"
}
trap cleanup EXIT
sway -c "$PWD/platforms/linux/tests/tools/sway-test.conf" > "$runtime/sway.log" 2>&1 &
compositor_pid=$!
for attempt in $(seq 1 100); do
  for socket in "$runtime"/wayland-*; do
    if [[ -S "$socket" ]]; then export WAYLAND_DISPLAY="$socket"; break 2; fi
  done
  kill -0 "$compositor_pid"
  sleep 0.05
done
[[ -n ${WAYLAND_DISPLAY:-} && -S "$WAYLAND_DISPLAY" ]] || { cat "$runtime/sway.log"; exit 1; }
dbus-run-session -- env GDK_BACKEND=wayland GTK_IM_MODULE=fcitx NO_AT_BRIDGE=1 \
  python3 platforms/linux/fcitx5/tests/canvas_editor.py "$@" --wayland

#!/usr/bin/env bash
set -euo pipefail
[[ ${MSIME_ISOLATED_LINUX_TEST:-} == 1 ]] || { echo "Run only inside the dedicated Linux test container" >&2; exit 1; }
binary=${1:?host executable required}
options=${2:?runtime options required}
test_root=$(mktemp -d /tmp/msime-ibus-daemon.XXXXXX)
daemon_pid=
host_pid=
cleanup() {
  if [[ -n "$host_pid" ]]; then kill "$host_pid" 2>/dev/null || true; wait "$host_pid" 2>/dev/null || true; fi
  if [[ -n "$daemon_pid" ]]; then kill "$daemon_pid" 2>/dev/null || true; wait "$daemon_pid" 2>/dev/null || true; fi
  rm -rf "$test_root/config"
  rmdir "$test_root" 2>/dev/null || true
}
trap cleanup EXIT
# D-Bus activation otherwise inherits the environment from before this fixture
# created its private IBus socket (notably in the non-root Wayland session).
update_activation_environment() {
  local activation_environment=(IBUS_ADDRESS) name
  for name in XDG_RUNTIME_DIR XDG_CONFIG_HOME XDG_CACHE_HOME DISPLAY WAYLAND_DISPLAY; do
    if printenv "$name" >/dev/null; then activation_environment+=("$name"); fi
  done
  dbus-update-activation-environment "${activation_environment[@]}"
}
daemon_options=(--single --panel disable --config disable --emoji-extension disable)
ibus_version=$(ibus version 2>/dev/null | sed -n 's/^IBus //p' || true)
if [[ -n "$ibus_version" && $(printf '%s\n' "$ibus_version" 1.5.20 | sort -V | head -1) != 1.5.20 ]]; then
  # IBus 1.5.20 起 --address 才接受 unix:path=。1.5.19（legacy 包的 Debian 10 基线）只接受 unix:tmpdir=，套接字由 GLib 自选，地址只写进 IBus 的地址文件，所以让守护进程把地址文件写在这个测试自己的目录里，再用 ibus address 读回。
  # ibus address 先看环境里的 IBUS_ADDRESS，有就直接返回它而不读地址文件；从桌面会话继承来的值会让测试连到用户自己的守护进程上。
  unset IBUS_ADDRESS
  XDG_CONFIG_HOME="$test_root/config" ibus-daemon "${daemon_options[@]}" --address "unix:tmpdir=$test_root" &
  daemon_pid=$!
  address=
  for attempt in $(seq 1 100); do
    address=$(XDG_CONFIG_HOME="$test_root/config" ibus address 2>/dev/null || true)
    [[ -n "$address" && "$address" != "(null)" ]] && break
    kill -0 "$daemon_pid"
    sleep 0.05
  done
  export IBUS_ADDRESS="$address"
  update_activation_environment
else
  export IBUS_ADDRESS="unix:path=$test_root/bus"
  update_activation_environment
  ibus-daemon "${daemon_options[@]}" --address "$IBUS_ADDRESS" &
  daemon_pid=$!
  for attempt in $(seq 1 100); do
    [[ -S "$test_root/bus" ]] && break
    kill -0 "$daemon_pid"
    sleep 0.05
  done
fi
# Start the host the way ibus-daemon does, through the launcher installed beside it and that launcher's crash supervisor. daemon_smoke.py crashes the supervised host and stops the supervisor, so it needs the supervisor's PID and a way to start another.
launcher="$(dirname -- "$binary")/msime-linux-ibus-launcher"
if [[ -x "$launcher" ]]; then
  MSIME_IBUS_OPTIONS="$options" "$launcher" &
  host_pid=$!
  export MSIME_SMOKE_SUPERVISOR_PID=$host_pid MSIME_SMOKE_LAUNCHER=$launcher MSIME_SMOKE_OPTIONS=$options
else
  "$binary" "$options" &
  host_pid=$!
fi
/usr/bin/python3 "${3:-platforms/linux/tests/runtime/daemon_smoke.py}" "${@:4}"

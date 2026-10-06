#!/usr/bin/env bash
# Cross compilation: links codec and pipe tests, but does not run Windows code.
set -euo pipefail
repo_root=$(cd "$(dirname "$0")/../../../.." && pwd)
cd "$repo_root"
json_include=${1:?usage: check-cross.sh <nlohmann-json-include-root>}
[[ -f "$json_include/nlohmann/json.hpp" ]] || { echo "nlohmann JSON headers required" >&2; exit 2; }
winrt_include=${MSIME_WINRT_INCLUDE:-}
# shared/contracts/msime_edition.h 要求选定一个版本；与 CMake 的缺省一样编 full。
windows_includes=(-DMSIME_EDITION_FULL -Iplatforms/windows -Iplatforms/windows/common -Iplatforms/windows/src)
for area in server ipc candidate voice clipboard input system; do
  windows_includes+=(-Iplatforms/windows/src/$area)
done
for arch in x86_64 i686; do
  output="$repo_root/target/windows-cross/$arch"
  mkdir -p "$output"
  compiler="$arch-w64-mingw32-g++"
  for source in src/input/InputQueue.cpp src/ipc/SessionPump.cpp src/ipc/SessionWorkers.cpp src/ipc/SessionController.cpp src/system/PreferenceMonitor.cpp src/system/WindowsServer.cpp tests/runtime/server_smoke.cpp tests/input/input_queue.cpp tests/runtime/session_pump.cpp tests/runtime/session_workers.cpp tests/core/preference_monitor.cpp; do
    "$compiler" -std=c++17 -Wall -Wextra -Werror "${windows_includes[@]}" -Iplatforms/windows/msimeui/include -Icrates/host-api/include \
      -Ishared/contracts -I"$json_include" -c "platforms/windows/$source" -o "$output/$(basename "$source").o"
  done
  "$compiler" -std=c++17 -Wall -Wextra -Werror "${windows_includes[@]}" \
    platforms/windows/tests/runtime/registration_inbox.cpp -o "$output/registration-inbox.exe"
  "$compiler" -std=c++17 -Wall -Wextra -Werror "${windows_includes[@]}" -Ishared/contracts \
    platforms/windows/tests/runtime/focus_router.cpp -o "$output/focus-router.exe"
  "$compiler" -std=c++17 -Wall -Wextra -Werror "${windows_includes[@]}" -Ishared/contracts \
    platforms/windows/tests/core/main_frame.cpp -o "$output/main-frame.exe"
  "$compiler" -std=c++17 -Wall -Wextra -Werror "${windows_includes[@]}" -Iplatforms/windows/msimeui/include \
    -c platforms/windows/msimeui/demos/msimeui-keyboard-demo/KeyboardPanel.cpp -o "$output/keyboard-panel.o"
  if [[ -n "$winrt_include" && -f "$winrt_include/winrt/Windows.Foundation.Collections.h" ]]; then
    "$compiler" -std=c++20 -Wall -Wextra -Werror "${windows_includes[@]}" -Iplatforms/windows/msimeui/include -I"$winrt_include" \
      -c platforms/windows/msimeui/demos/msimeui-handwriting-demo/HandwritingPanel.cpp -o "$output/handwriting-panel.o"
  else
    echo "Windows C++/WinRT headers unavailable; handwriting panel cross-check skipped"
  fi
  "$compiler" -std=c++17 -Wall -Wextra -Werror "${windows_includes[@]}" -Iplatforms/windows/msimeui/include -I"$json_include" \
    -c platforms/windows/msimeui/demos/msimeui-emoji-panel/EmojiPanel.cpp -o "$output/emoji-panel.o"
  "$compiler" -std=c++17 -Wall -Wextra -Werror "${windows_includes[@]}" -Icrates/host-api/include \
    -Ishared/contracts -I"$json_include" -c platforms/windows/src/input/FocusedSession.cpp -o "$output/FocusedSession.o"
  "$compiler" -std=c++17 -Wall -Wextra -Werror "${windows_includes[@]}" \
    platforms/windows/tests/runtime/focus_gate.cpp -o "$output/focus-gate.exe"
  "$compiler" -std=c++17 -Wall -Wextra -Werror "${windows_includes[@]}" -Ishared/contracts \
    platforms/windows/src/ipc/PipeIo.cpp platforms/windows/src/ipc/PipePeer.cpp platforms/windows/src/ipc/PipeHandshake.cpp \
    platforms/windows/src/ipc/PipeListener.cpp platforms/windows/src/ipc/PipeRegistry.cpp platforms/windows/src/ipc/PipeIntake.cpp platforms/windows/src/ipc/PipeService.cpp platforms/windows/src/ipc/PipeMainTransport.cpp platforms/windows/src/ipc/ReplyCodec.cpp platforms/windows/tests/runtime/pipe_io.cpp -ladvapi32 -o "$output/pipe-io.exe"
  "$compiler" -std=c++17 -Wall -Wextra -Werror "${windows_includes[@]}" -Ishared/contracts \
    platforms/windows/src/ipc/ReplyCodec.cpp platforms/windows/tests/core/reply_codec.cpp -o "$output/reply-codec.exe"
  "$compiler" -std=c++17 -Wall -Wextra -Werror "${windows_includes[@]}" -Icrates/host-api/include \
    -Ishared/contracts -I"$json_include" -c platforms/windows/src/ipc/ReplyComposer.cpp -o "$output/ReplyComposer.o"
  "$compiler" -std=c++17 -Wall -Wextra -Werror "${windows_includes[@]}" -Icrates/host-api/include \
    -Ishared/contracts -I"$json_include" -c platforms/windows/tests/input/reply_composer.cpp -o "$output/reply_composer.o"
  "$compiler" -std=c++17 -Wall -Wextra -Werror "${windows_includes[@]}" -Icrates/host-api/include \
    -Ishared/contracts -I"$json_include" -c platforms/windows/src/ipc/ServerSession.cpp -o "$output/ServerSession.o"
  "$compiler" -std=c++17 -Wall -Wextra -Werror "${windows_includes[@]}" -Icrates/host-api/include \
    -Ishared/contracts -I"$json_include" -c platforms/windows/tests/runtime/session_smoke.cpp -o "$output/session_smoke.o"
  "$compiler" -std=c++17 -Wall -Wextra -Werror -DMSIME_EDITION_FULL -c shared/contracts/tests/windows_ipc_contract.cpp -o "$output/windows_ipc_contract.o"
done
echo "Windows x86/x64 adapter and contract compiled, codec and pipe tests linked; no Windows runtime verified"

# Windows capture selection

The Tauri device command enumerates Engine capture endpoint identities on a blocking worker. It returns the `{backend, id, label}` shape with `backend: "windows"`; the ID is an endpoint identity, not an enumeration index. Labels and endpoint IDs can identify user hardware and must not be logged.

The shared preference store persists `voice_input.capture_backend` and `capture_device`. The Server's preference observer copies these into `VoiceInputConfig`; both native-hotkey and controller-review recording pass the captured ID to Engine when starting. Updates do not switch an active recording's device. Empty, `auto`, and `windows` backends are supported here, compared without regard to case. A foreign backend (`pulse`, `macos` and the like, from a configuration copied off another machine) is read as no selection together with its device, so recording uses the system default instead of failing; the stored values are left as they are. Empty IDs select the system default. Nonempty stale or invalid IDs fail rather than recording another device.

Numeric indices saved by older builds were never honored by the native capture path. They are deliberately not reinterpreted: refresh and choose an endpoint, or clear the selection to explicitly use the system default.

Checks that need no microphone, run from the repository root:

```sh
c++ -std=c++17 -Wall -Wextra -Werror -fsanitize=address,undefined \
  -I/opt/homebrew/include platforms/windows/tests/voice/voice_capture_selection.cpp \
  -o target/voice-capture-selection
./target/voice-capture-selection
pnpm --filter @msime/desktop exec vitest run tests/input/windows-capture-devices.test.tsx
pnpm --filter @msime/desktop build
```

The C++ test uses synthetic configuration; the UI test uses synthetic same-label devices and reorders their list. Engine's own capture-device tests verify the same ID resolver the Server uses, including ambiguous and missing IDs and invalid syntax rejected before device initialization. What these cover is the selection and resolution logic; whether a given endpoint records depends on the machine's devices and the microphone permission granted there.

# TSF host configuration

The Windows DLL requires `MSIME_HOST_LIBRARY` at CMake configure time. Supply an absolute path to the Cargo-built host static library or DLL import library for the same architecture. Missing paths and directories are rejected before native dependency discovery. The linker remains responsible for format, architecture and symbols. Portable component tests and the standalone export fixture do not require it.

The DLL version resource uses the standard `VS_VERSION_INFO` identifier, the workspace package version with a zero Windows revision, and the actual `MetasequoiaImeTsf.dll` output name. The portable CMake tests verify this source contract without loading or registering the DLL.

The TIP reads `%LOCALAPPDATA%\MSIME-Client\runtime-options.json`, not legacy product state or guessed paths relative to the embedding application. The installer prepares that state for the production TIP; the command below prepares an equivalent state directory by hand, which is what a development build of the DLL wants.

With all sessions using that state stopped, prepare it through the shared host tool (PowerShell, from the repository root):

```powershell
cargo run -p msime-host-api --example prepare_host -- '<verified-resource-directory>' "$env:LOCALAPPDATA\MSIME-Client"
```

The tool verifies locked resources, prepares Engine working data, loads shared preferences and atomically publishes the complete document. The TIP reads up to the C ABI limit of 16384 bytes and passes it unchanged to the host. Missing, empty, unreadable or oversized files do not create a session; malformed schema and invalid preferences are rejected by the shared host. Running it by hand neither registers the TIP nor updates resources later — installation and registration belong to `installer/msime_setup.iss`.

Native focus is routed through activation, document/top-context changes, deferred loss and deactivation; repeated same-context notifications are deduplicated because runtime focus resets composition. Preference changes reach the input path through the Server's `PreferenceMonitor` rather than through the DLL, so the TIP does not poll the preference file itself. Like the switch-language hotkeys, the default mode and the running scheme, which the key path reads through `FanyUtils`, `game_compatibility` is read by the DLL directly: `ActivateEx` reads it once per activation through `FanyUtils::ReadConfiguredGameCompatibility`, because the decision to draw the candidate window for a game that declares UI-less (`Global/CandidateOverlayHostPolicy.h`) has to be made before UI-less is reported to the Server and before `BeginUIElement`, and the process lists do not fit in a configuration frame of at most 199 UTF-16 code units (`FanyImePipeLimits::CandidateTextMaxLength`). A read error means no forced overlay for that activation, and a change reaches an already open game only when the input method is activated there again. The rules and trade-offs are in [the game candidate overlay note](../../../.agents/notes/implemented/feature/2026-10-08-windows-game-candidate-overlay.md).

The 31 CTest entries registered here (`msime-tsf-version-resource`, `msime-tsf-client-key-router`, `msime-tsf-host-focus`, `msime-tsf-prepared-options`, `msime-tsf-candidate-ownership`, `msime-tsf-engine-response`, `msime-tsf-candidate-overlay-policy` and the rest) cover the DLL-side policy and encoding without loading the TIP into a TSF host. `tests/registration_profiles` and `tests/registration_categories` are separate sub-projects covering profile and category registration; `tests/exports` covers the COM export surface below.

## COM export contract

The production DLL links `IME/MetasequoiaIME.def` to expose the four undecorated COM entry points on x86 and x64. A standalone link-only fixture tests this same definition without requiring the complete SDK-dependent TIP:

```sh
cmake -S platforms/windows/tsf/tests/exports -B target/tsf-exports-x64 -DCMAKE_SYSTEM_NAME=Windows -DCMAKE_CXX_COMPILER=x86_64-w64-mingw32-g++
cmake --build target/tsf-exports-x64
ctest --test-dir target/tsf-exports-x64 --output-on-failure
```

Use a separate directory and `i686-w64-mingw32-g++` for x86. Tests inspect PE exports and reject a DLL missing the COM symbols; they never load or register the fixture. An MSVC build uses `dumpbin`. These checks cover the export surface alone; the complete TIP is built by `../build-cross.sh` or `../Build-Client.ps1` and registered by the installer.

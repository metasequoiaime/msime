# Windows UI package layout

The Windows settings product is the native WinUI 3 `msime-client-settings.exe`. The shared desktop UI is still built from `apps/desktop` for the panel shell and is staged as `MSIME.exe`. Candidate and toolbar presentation use the Windows native implementation in `platforms/windows/src/candidate`, not either shell executable.

Both full and light packages install the executable UI. Preparation does not run the installer or delete any already-installed user directory.

There is no post-build HTML version-string replacement. Tauri preview version metadata is supplied at build time through `Build-Client.ps1 -TargetVersion`; the WinUI 3 settings binary gets the native project version. Installer `TargetVersion` does not patch an embedded frontend after compilation. Installer upgrade cleanup is defined in `msime_setup.iss` and is unaffected by this layout.

Synthetic package tests check the Tauri executable and native artifacts. `tests/tauri-layout.ps1` checks the Inno file rules. These check the packaged file layout, not UI rendering or installation on a machine.

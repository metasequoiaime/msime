# Client local installation workflow

The full `test.ps1` and existing-installation-only `test-light.ps1` wrappers use `Invoke-LocalInstall.ps1` and the actual Client `Build-Client.ps1`. They do not reference legacy windows/scripts, server/scripts or ui-html build projects.

```powershell
.\platforms\windows\installer\test.ps1 -X64Dependencies C:\deps\x64 -X86Dependencies C:\deps\x86 -NoticesDirectory target/windows-notices
```

This is an explicit local build/sign/install command, not a unit test: the signing scripts create or reuse a local test certificate and may modify the local trust store, and the final stage launches the installer. For building alone, use `platforms/windows/Build-Client.ps1`. For a signed public release, use `Package-SimplySign.ps1` instead. None of these scripts trigger CI.

The shared sequence is Client build, package preparation, binary signing, Inno compilation, installer signing, then installer launch. Exceptions and nonzero script/native exit status stop the sequence. The caller's working directory is restored. Full/light selection is passed consistently through preparation, compilation, signing and launch. Both architectures are selected explicitly from `target/windows-full`, with the Tauri shell beside the x64 Server.

Supply prepared native dependency prefixes and a notice directory containing `THIRD_PARTY_NOTICES.txt`. Supplying notices is not a completeness or permission check; review the limitations in `../Notices.md`. Full packages additionally need the locked `DesktopResourcesDirectory` (default `target/desktop-resources`), the pinned Engine assets and the Windows toolchains documented in `../Build-Client.md`.

`TargetVersion` is the single version for one installation build: `Build-Client.ps1` passes it to the native build as `MSIME_WINDOWS_VERSION` and to Tauri as the desktop `version`, and `Prepare-PackageFiles.ps1` rewrites `#define MyAppVersion` in `msime_setup.iss` from it. The value checked into `msime_setup.iss` is therefore whatever the last staging run wrote, not an independent source of truth. Leaving `TargetVersion` unset in the local install scripts (`test.ps1`, `test-light.ps1`, `Invoke-LocalInstall.ps1`, `Prepare-PackageFiles.ps1`) takes it from `platforms/windows/version.txt`, the same file the native build and the WinUI settings project fall back to, so a local install carries the current Windows version rather than a placeholder.

`tests/installer_entry.ps1` substitutes every side-effect stage and verifies ordering, arguments, light-mode propagation, missing notices, and failure at all six stages. It drives substituted stages, so it covers the orchestration rather than signing, compilation or installation themselves. `tests/build-failures.ps1` forwards to this regression instead of referring to removed component scripts. Registration behaviour, UIAccess and dependency closure on a given machine are properties of that machine's installed package, checked by running the installer there rather than by this suite.

#!/usr/bin/env bash
# Prints the path of the keystore every development Android build signs with, creating it the first time. It is the Android SDK's own debug keystore (alias androiddebugkey, passwords android), so all worktrees, and the Tauri and Android Studio debug builds, sign with one key and a package built in one worktree installs over one built in another. A key kept per worktree under target/ made the first build in every new worktree unable to update the installed package without uninstalling it, data and all.
set -euo pipefail
keystore="$HOME/.android/debug.keystore"
if [[ ! -f "$keystore" ]]; then
  mkdir -p "$(dirname "$keystore")"
  keytool -genkeypair -keystore "$keystore" -storepass android -keypass android \
    -alias androiddebugkey -dname "CN=Android Debug,O=Android,C=US" -keyalg RSA -keysize 2048 -validity 10000 >&2
fi
printf '%s\n' "$keystore"

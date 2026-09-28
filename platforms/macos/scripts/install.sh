#!/usr/bin/env bash
# Install the built input method into ~/Library/Input Methods and register it as an input source.
#
# The bundle CMake produces is ad-hoc signed and carries no entitlements. macOS will not register an input
# source from it: --register-input-source returns 1 and the source never reaches TISCreateInputSourceList,
# so the input method cannot be selected at all. It has to be signed first, and voice input additionally
# needs com.apple.security.device.audio-input, which only arrives with an entitlements file.
#
# The first --register-input-source after replacing a bundle can return 0 on a stale LaunchServices entry
# from the previous one. That is why this script checks the registry afterwards rather than trusting the
# exit code, and why an input method the registry cannot offer is a failed install rather than a warning.
# Not every disabled source is that, though - see the three outcomes at the bottom.
#
# Usage: platforms/macos/scripts/install.sh [path/to/bundle.app]
#   MSIME_SIGNING_IDENTITY       signing identity, a name or a SHA-1; defaults to the first Developer ID Application certificate in the keychain, by SHA-1
#   MSIME_INPUT_METHODS_DIR      destination; defaults to ~/Library/Input Methods
#   MSIME_VOICE_ENTITLEMENTS     entitlements to sign with; defaults to resources/VoiceInput.entitlements
set -euo pipefail

name="水杉输入法.app"
executable="水杉输入法"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
source_bundle="${1:-$root/target/macos/$name}"
destination_root="${MSIME_INPUT_METHODS_DIR:-$HOME/Library/Input Methods}"
entitlements="${MSIME_VOICE_ENTITLEMENTS:-$root/platforms/macos/resources/VoiceInput.entitlements}"

if [ ! -d "$source_bundle" ]; then
  echo "no bundle at $source_bundle; build MSIMEClientInputMethod first (see platforms/macos/README.md)" >&2
  exit 1
fi
/usr/bin/plutil -lint "$entitlements" >/dev/null

# Restricted entitlements are the ones a provisioning profile has to vouch for, and the whole
# com.apple.developer.* family is restricted. A Developer ID signature cannot vouch for them: AMFI rejects
# the binary at exec, so the input method never launches and therefore never registers as an input source.
# The signature itself verifies fine, which is what makes this worth refusing up front rather than
# discovering it as an input method that silently will not start. com.apple.developer.applesignin is the
# one that actually tempts us — native Apple sign-in is simply not available to a Developer ID input
# method. See resources/VoiceInput.entitlements.
restricted="$(/usr/bin/plutil -convert json -o - "$entitlements" |
  /usr/bin/python3 -c 'import json,sys; print(" ".join(k for k in json.load(sys.stdin) if k.startswith("com.apple.developer.")))')"
if [ -n "$restricted" ]; then
  echo "refusing to sign: $entitlements declares restricted entitlements: $restricted" >&2
  echo "a Developer ID signature cannot carry them and the input method would not launch" >&2
  exit 1
fi

[ -d "$destination_root" ] || mkdir -p "$destination_root"
destination="$destination_root/$name"

# Never replace a bundle out from under a running process: macOS lets the files go while the process keeps
# running against them, which leaves an input method serving a version that no longer exists on disk.
if pgrep -f "$destination/Contents/MacOS/$executable" >/dev/null 2>&1; then
  echo "stopping the running input method"
  pkill -f "$destination/Contents/MacOS/$executable" || true
  for _ in $(seq 1 50); do
    pgrep -f "$destination/Contents/MacOS/$executable" >/dev/null 2>&1 || break
    sleep 0.1
  done
  if pgrep -f "$destination/Contents/MacOS/$executable" >/dev/null 2>&1; then
    echo "it did not stop; nothing was changed" >&2
    exit 1
  fi
fi

staging="$(mktemp -d "$destination_root/.msime-staging.XXXXXX")"
backup=""
restore() {
  status=$?
  if [ "$status" -ne 0 ]; then
    if [ -n "$backup" ] && [ -d "$backup/$name" ]; then
      rm -rf "$destination"
      mv "$backup/$name" "$destination" && echo "restored the previous installation" >&2
    fi
  fi
  [ -n "$backup" ] && rm -rf "$backup"
  rm -rf "$staging"
  exit "$status"
}
trap restore EXIT

ditto "$source_bundle" "$staging/$name"

# Sign the staged copy, not the destination: a half-signed bundle must never be the installed one.
identity="${MSIME_SIGNING_IDENTITY:-}"
label="$identity"
if [ -z "$identity" ]; then
  # Resolve to the certificate's SHA-1, not its name. Two valid Developer ID Application certificates for
  # the same team is an ordinary state - a renewed one next to the one it replaces - and codesign refuses a
  # name that matches both rather than picking one:
  #
  #   Developer ID Application: Name (TEAMID): ambiguous (matches "Developer ID Application: Name (TEAMID)"
  #   and "Developer ID Application: Name (TEAMID)" in /Users/…/login.keychain-db)
  #
  # which failed the install after the running input method had already been stopped. A hash is never
  # ambiguous. Any of them works for a local install, so take the first and say that there were others.
  candidates="$(security find-identity -v -p codesigning 2>/dev/null |
    sed -n 's/^ *[0-9]*) *\([0-9A-F]\{40\}\) *"\(Developer ID Application: [^"]*\)".*/\1 \2/p')"
  identity="$(printf '%s\n' "$candidates" | head -1 | cut -d' ' -f1)"
  label="$(printf '%s\n' "$candidates" | head -1 | cut -d' ' -f2-)"
  found="$(printf '%s\n' "$candidates" | grep -c . || true)"
  if [ "${found:-0}" -gt 1 ]; then
    echo "$found Developer ID Application identities in the keychain; signing with $label ($identity)" >&2
    echo "set MSIME_SIGNING_IDENTITY to a SHA-1 from 'security find-identity -v -p codesigning' for another" >&2
  fi
fi
if [ -z "$identity" ]; then
  echo "no Developer ID Application identity found; set MSIME_SIGNING_IDENTITY" >&2
  echo "an ad-hoc signed bundle will not register as an input source" >&2
  exit 1
fi
# --deep, because the nested code arrives signed by whoever published it. Sparkle ships signed by the
# Sparkle project, and under the hardened runtime a process cannot load a library whose Team ID differs
# from its own:
#
#   code signature in '.../Sparkle.framework/Versions/B/Sparkle' not valid for use in process:
#   mapping process and mapped file (non-platform) have different Team IDs
#
# which kills the input method on launch, so it never registers. Apple discourages --deep for distribution
# signing; this is a local install of a bundle whose nested code is a pinned framework and one dylib we
# built ourselves, and re-signing all of it with one identity is exactly what is needed.
#
# --timestamp needs the network. Losing it is not a reason to refuse to install; the signature is still
# valid locally, which is all a local install needs.
codesign --force --deep --options runtime --timestamp --entitlements "$entitlements" --sign "$identity" \
  "$staging/$name" 2>/dev/null ||
  codesign --force --deep --options runtime --entitlements "$entitlements" --sign "$identity" "$staging/$name"
codesign --verify --strict "$staging/$name"
if [ "$label" = "$identity" ]; then echo "signed with $identity"; else echo "signed with $label ($identity)"; fi

if [ -d "$destination" ]; then
  backup="$(mktemp -d "$destination_root/.msime-backup.XXXXXX")"
  mv "$destination" "$backup/$name"
fi
mv "$staging/$name" "$destination"

# The text input system relaunches a stopped input method as soon as a client asks for it, and signing takes long enough for that to happen: the process it starts runs the previous bundle, now under the backup directory, and keeps serving the old code with nothing on screen to say so. Anything running at this point started before the new bundle arrived, so stop it once more and let the next launch come from the new one.
pkill -f "$destination/Contents/MacOS/$executable" || true

echo "installed $destination"

# Installing and registering are different operations with different failure meanings, so the rollback
# ends here. A bundle that is in place and signed is a good install; whether this login session can see it
# in the input source list is a separate question, and undoing a correct install over it would be worse
# than reporting it.
trap - EXIT
[ -n "$backup" ] && rm -rf "$backup"
rm -rf "$staging"

identifier="$(/usr/libexec/PlistBuddy -c "Print :CFBundleIdentifier" "$destination/Contents/Info.plist")"

# Every build of this bundle gets registered with LaunchServices wherever it lands, so each worktree's
# target/ directory leaves behind another record claiming the shipping identifier. Five of them had piled
# up on the machine this was written on, pointing at build outputs and at temporary directories that no
# longer existed. Leave only the installed one: LaunchServices has to resolve the identifier to a single
# bundle, and it is the installed path that should win.
lsregister="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"
if [ -x "$lsregister" ]; then
  "$lsregister" -dump 2>/dev/null |
    awk -v id="$identifier" '
      /^path:/ { path = $0; sub(/^path: */, "", path); sub(/ \(0x[0-9a-f]*\)$/, "", path) }
      $0 ~ "^identifier: +" id "$" { if (path != "") print path }' |
    while IFS= read -r stale; do
      [ "$stale" = "$destination" ] && continue
      echo "dropping a competing LaunchServices record: $stale"
      "$lsregister" -u "$stale" >/dev/null 2>&1 || true
    done
fi

# Refresh the record for the bundle that was just written, before asking it to register itself.
# Replacing a bundle in place leaves LaunchServices holding the previous copy's record for that path -
# observed as `lsappinfo` reporting a null bundle identifier for the running process, the system refusing
# to launch the input method on demand, and the first --register-input-source afterwards returning 0
# without the source appearing. One -f on the installed path fixes all three.
if [ -x "$lsregister" ]; then
  "$lsregister" -f "$destination" >/dev/null 2>&1 || true
fi

if ! "$destination/Contents/MacOS/$executable" --register-input-source; then
  echo "--register-input-source exited non-zero; what the registry says below is what decides" >&2
fi

# System Settings' add-input-source dialog does not read the input-source registry but a cache that the sandboxed Keyboard settings extension keeps in its own container, and nothing regenerates that copy when a bundle changes its modes - restarting System Settings does not. A mode added to the bundle stays missing from the dialog until the files are gone; the extension rebuilds them the next time it opens. It holds them while it runs, so the files are left alone then.
keyboard_settings_cache="$(getconf DARWIN_USER_CACHE_DIR)com.apple.Keyboard-Settings.extension"
if pgrep -xq "System Settings"; then
  echo "System Settings is open: quit it and run this again, or its add-input-source dialog keeps showing the modes it cached before this install" >&2
else
  rm -f "$keyboard_settings_cache"/com.apple.IntlDataCache.le*
fi

# The Ctrl+Space switcher beside the caret (CursorUIViewService) and the held-shortcut switcher (TextInputSwitcher) read the input-source list and its icons once and keep them for the life of the process, so after an install they go on showing the modes and icons of the build before it - a mode added since is missing and a changed icon is the old one. Both ignore SIGTERM, and launchd starts each again the next time it is needed.
pkill -KILL -x CursorUIViewService || true
pkill -KILL -x TextInputSwitcher || true

# Three outcomes, and the note at the bottom is only right for one of them. Printing it for all three is
# what this used to do: an input method that registers, enables its Chinese mode and types perfectly well
# would be reported as absent from the session, with instructions to log out that cannot change anything,
# because one of the bundle's other modes was left disabled. check_input_source.swift tells them apart and
# its header carries what each one means.
checked=0
"$root/platforms/macos/scripts/check_input_source.swift" "$identifier" "$destination" || checked=$?
if [ "$checked" -eq 0 ]; then
  echo "select 水杉输入法 from the input menu to start typing"
  exit 0
fi
if [ "$checked" -eq 2 ]; then
  echo "installed, and selectable from the input menu; add the source listed above as not enabled in"
  echo "System Settings > Keyboard > Text Input > Input Sources - a keyboard input mode cannot be enabled"
  echo "from a process on this macOS version, so no amount of re-registering or re-login will add it."
  exit 0
fi
# The registry is scoped to the login session, which is measured rather than assumed: copy the input method
# that does work, give the copy a fresh bundle identifier, re-sign it with the same Developer ID and
# register it, and it fails identically. The registration itself is recorded - TISRegisterInputSource
# answers noErr - so this is a report, not a failed install. check_input_source.swift carries the rest.
cat >&2 <<'NOTE'

the input method is installed and signed, but this login session's input source list does not show it yet.
log out and back in, then choose 水杉输入法 in System Settings > Keyboard > Text Input > Input Sources.
if it is still missing after that, see the ruled-out causes in platforms/macos/README.md.
NOTE

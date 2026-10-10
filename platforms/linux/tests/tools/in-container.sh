#!/usr/bin/env bash
set -euo pipefail
[[ ${MSIME_ISOLATED_LINUX_TEST:-} == 1 && -d /resources && -d /build ]] || exit 2
python3 platforms/linux/tests/candidate/panel_keymap.py
python3 platforms/linux/tests/provider/provider_config_discovery.py
python3 platforms/linux/tests/candidate/provider_candidate_validation.py
python3 platforms/linux/tests/provider/cloud_timeout_parity.py
python3 platforms/linux/tests/candidate/ai_candidate_cache.py
python3 platforms/linux/tests/dictionary/translation_cache_parity.py
python3 platforms/linux/tests/voice/provider_voice_text_validation.py
python3 platforms/linux/tests/voice/provider_request_format.py
python3 platforms/linux/tests/voice/doubao_auth.py
python3 platforms/linux/tests/dictionary/niutrans_credential_normalization.py
python3 platforms/linux/tests/dictionary/tencent_credential_normalization.py
python3 platforms/linux/tests/dictionary/custom_translation_config.py
python3 platforms/linux/tests/dictionary/translation_provider_selection.py
python3 platforms/linux/tests/voice/doubao_auth_mode.py
python3 platforms/linux/tests/voice/provider_polish_prompt.py
python3 platforms/linux/tests/provider/credential_test_contract.py
python3 platforms/linux/tests/provider/ai_service_contract.py
python3 platforms/linux/tests/clipboard/clipboard_capture_destination.py
python3 platforms/linux/tests/clipboard/clipboard_watch_lifecycle.py
python3 platforms/linux/tests/clipboard/clipboard_wayland_watch.py
cargo build -p msime-host-api --locked
python3 - <<'PY'
import ctypes
import re
from pathlib import Path

header = Path("crates/host-api/include/msime_client.h").read_text()
library = ctypes.CDLL("/build/cargo/debug/libmsime_host_api.so")
symbols = set(re.findall(r"\b(msime_client_\w+)\s*\(", header))
inline_symbols = set(re.findall(
    r"\bstatic\s+inline\b[^{;]*\b(msime_client_\w+)\s*\(", header
))
symbols -= inline_symbols
assert symbols, "Host API header contains no exported declarations"
for name in sorted(symbols):
    getattr(library, name)
print("Host API header exports verified")
PY
cargo test -p msime-client-core -p msime-input-runtime -p msime-host-api --locked
if [[ ${MSIME_TEST_FCITX5:-0} == 1 ]]; then
  bash platforms/linux/tests/tools/fcitx5-container.sh
  exit 0
fi
cmake -S platforms/linux -B /build/ibus -G Ninja -DMSIME_HOST_LIBRARY=/build/cargo/debug/libmsime_host_api.so -DMSIME_LINUX_VOICE=ON
cmake --build /build/ibus
ctest --test-dir /build/ibus --output-on-failure --no-tests=error
rm -rf /build/stage
DESTDIR=/build/stage cmake --install /build/ibus
if [[ -x /build/stage/usr/local/bin/msime-linux-clipboard-watch-x11 ]]; then
  xvfb-run -a python3 platforms/linux/tests/clipboard/clipboard_x11_events.py /build/stage/usr/local/bin/msime-linux-clipboard-watch-x11
  xvfb-run -a python3 platforms/linux/tests/clipboard/clipboard_x11_read.py /build/stage/usr/local/bin/msime-linux-clipboard-watch-x11 /build/ibus/msime-test-x11-string-owner
fi
test -x /build/stage/usr/local/bin/msime-linux-ibus
test -x /build/stage/usr/local/bin/msime-linux-dictionary
test -x /build/stage/usr/local/bin/msime-linux-cloud-dictionary
test -x /build/stage/usr/local/bin/msime-linux-cloud-clipboard
test -x /build/stage/usr/local/bin/msime-linux-voice
test -f /build/stage/usr/local/share/ibus/component/msime-linux.xml
test -f /build/stage/usr/local/etc/xdg/autostart/msime-linux-clipboard.desktop
grep -q '/usr/local/etc/msime-client/runtime-options.json' \
  /build/stage/usr/local/share/ibus/component/msime-linux.xml
python3 platforms/linux/tests/dictionary/dictionary_smoke.py /build/ibus/msime-linux-dictionary /build/cargo/debug/libmsime_host_api.so /resources
clipboard_fixture=$(mktemp -d /tmp/msime-clipboard.XXXXXX)
trap 'rm -rf "$clipboard_fixture"' EXIT
/build/stage/usr/local/bin/msime-linux-clipboard "$clipboard_fixture/history.json" add $'first\nentry'
/build/stage/usr/local/bin/msime-linux-clipboard "$clipboard_fixture/history.json" add "second"
[[ $(stat -c '%a' "$clipboard_fixture/history.json") == 600 ]]
[[ $(stat -c '%a' "$clipboard_fixture/history.json.lock") == 600 ]]
! compgen -G "$clipboard_fixture/history.json.tmp.*" >/dev/null
[[ $(/build/stage/usr/local/bin/msime-linux-clipboard "$clipboard_fixture/history.json" get 1) == $'first\nentry' ]]
if /build/stage/usr/local/bin/msime-linux-clipboard "$clipboard_fixture/history.json" get 2 >/dev/null; then
  echo "clipboard get accepted an out-of-range index" >&2
  exit 1
fi
echo "Linux clipboard stream acceptance passed"

/build/stage/usr/local/bin/msime-linux-clipboard "$clipboard_fixture/history.json" remove $'first\nentry\r'
if /build/stage/usr/local/bin/msime-linux-clipboard "$clipboard_fixture/history.json" get 1 >/dev/null; then
  echo "Linux clipboard remove did not normalize the requested value" >&2
  exit 1
fi
[[ $(/build/stage/usr/local/bin/msime-linux-clipboard "$clipboard_fixture/history.json" get 0) == "second" ]]
echo "Linux clipboard normalized remove passed"

python3 - "$clipboard_fixture/history.json" <<'PYTHON'
import sys
from pathlib import Path

Path(sys.argv[1]).write_text('["' + 'x' * (1024 * 1024) + '"]')
PYTHON
if [[ $(/build/stage/usr/local/bin/msime-linux-clipboard "$clipboard_fixture/history.json" list) != '[]' ]]; then
  echo "Linux clipboard history accepted an oversized store" >&2
  exit 1
fi
echo "Linux clipboard history size limit passed"

# The size-limit check left an empty history, so put back the entry remove-index must keep.
/build/stage/usr/local/bin/msime-linux-clipboard "$clipboard_fixture/history.json" add second
unicode_text='水杉输入法 😀'
/build/stage/usr/local/bin/msime-linux-clipboard "$clipboard_fixture/history.json" add "$unicode_text"
[[ $(/build/stage/usr/local/bin/msime-linux-clipboard "$clipboard_fixture/history.json" get 0) == "$unicode_text" ]]
echo "Linux clipboard UTF-8 acceptance passed"
/build/stage/usr/local/bin/msime-linux-clipboard "$clipboard_fixture/history.json" remove-index 0
[[ $(/build/stage/usr/local/bin/msime-linux-clipboard "$clipboard_fixture/history.json" get 0) == "second" ]]
echo "Linux clipboard remove-index acceptance passed"
/build/stage/usr/local/bin/msime-linux-clipboard "$clipboard_fixture/history.json" clear
[[ ! -e "$clipboard_fixture/history.json" ]]
echo "Linux clipboard clear acceptance passed"
/build/ibus/ibus-engine-smoke /resources
installed_host=/build/stage/usr/local/bin/msime-linux-ibus
python3 - "$installed_host" <<'PYTHON'
import re
import subprocess
import sys
from pathlib import Path

output = subprocess.check_output(["ldd", sys.argv[1]], text=True)
match = re.search(r"libmsime_host_api\.so => (\S+)", output)
expected = Path("/build/stage/usr/local/lib/msime-client/libmsime_host_api.so")
assert match and Path(match[1]).resolve() == expected.resolve(), \
    "Installed host did not resolve the staged Host API library"
print("Installed host resolves staged Host API library")
PYTHON
# The installed prepare records a declined cloud-candidate choice in both the shared store and the published options.
declined=$(mktemp -d /tmp/msime-prepare-declined.XXXXXX)
/build/stage/usr/local/bin/msime-linux-prepare --no-cloud-candidates /resources "$declined/state" >/dev/null
python3 - "$declined/state" <<'PYTHON'
import json
import sys
from pathlib import Path

state = Path(sys.argv[1])
assert json.loads((state / "preferences.json").read_text())["preferences"]["cloud_candidates"] is False
assert json.loads((state / "runtime-options.json").read_text())["preferences"]["cloud_candidates"] is False
print("Declined cloud candidates are recorded at preparation")
PYTHON
# Cloud candidates start off; only an accepted choice at preparation turns them on.
/build/stage/usr/local/bin/msime-linux-prepare --cloud-candidates /resources "$declined/accepted" >/dev/null
python3 -c 'import json,sys; assert json.load(open(sys.argv[1]))["preferences"]["cloud_candidates"] is True' "$declined/accepted/runtime-options.json"
# A user that has never logged in to a desktop may not have ~/.config yet: prepare creates the missing parents, private to the user, instead of failing.
/build/stage/usr/local/bin/msime-linux-prepare /resources "$declined/home/.config/default" >/dev/null
python3 -c 'import json,sys; assert json.load(open(sys.argv[1]))["preferences"]["cloud_candidates"] is False' "$declined/home/.config/default/runtime-options.json"
test "$(stat -c %a "$declined/home/.config")" = 700
rm -rf "$declined"
fixture=$(mktemp -d /tmp/msime-ibus-bootstrap.XXXXXX)
options=$(cargo run --quiet -p msime-host-api --example prepare_host --locked -- /resources "$fixture")
global_mode_options="$fixture/global-runtime-options.json"
python3 - "$options" "$global_mode_options" <<'PYTHON'
import json
import os
import sys
from pathlib import Path

value = json.loads(Path(sys.argv[1]).read_text())
# Every runtime smoke below starts from the prepared default and expects it to compose Chinese, as Windows does out of the box.
assert value["preferences"]["default_ime_mode"] == "chinese", value["preferences"]["default_ime_mode"]
value["preferences"]["ime_mode_scope"] = "global"
value.pop("preferences_directory", None)
with os.fdopen(os.open(sys.argv[2], os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "w") as output:
    json.dump(value, output)
PYTHON
dbus-run-session -- bash platforms/linux/tests/runtime/daemon_smoke.sh "$installed_host" "$global_mode_options"

GTK_IM_MODULE=ibus XMODIFIERS=@im=ibus NO_AT_BRIDGE=1 xvfb-run -a dbus-run-session -- bash platforms/linux/tests/runtime/daemon_smoke.sh "$installed_host" "$options" platforms/linux/tests/runtime/gtk_smoke.py

QT_IM_MODULE=ibus XMODIFIERS=@im=ibus xvfb-run -a dbus-run-session -- bash platforms/linux/tests/runtime/daemon_smoke.sh "$installed_host" "$options" platforms/linux/tests/runtime/qt_smoke.py

QT_IM_MODULE=ibus XMODIFIERS=@im=ibus xvfb-run -a dbus-run-session -- bash platforms/linux/tests/runtime/daemon_smoke.sh "$installed_host" "$options" platforms/linux/tests/runtime/qt_smoke.py --qt6

XCOMPOSEFILE="$PWD/platforms/linux/tests/input/compose.fixture" GTK_IM_MODULE=ibus XMODIFIERS=@im=ibus NO_AT_BRIDGE=1 xvfb-run -a dbus-run-session -- bash platforms/linux/tests/runtime/daemon_smoke.sh "$installed_host" "$options" platforms/linux/tests/runtime/gtk_smoke.py --custom-compose

runuser -u nobody -- dbus-run-session -- bash platforms/linux/tests/runtime/wayland_smoke.sh "$installed_host" /resources /build/cargo/debug/examples/prepare_host

runuser -u nobody -- dbus-run-session -- bash platforms/linux/tests/runtime/wayland_smoke.sh "$installed_host" /resources /build/cargo/debug/examples/prepare_host platforms/linux/tests/runtime/qt_smoke.py --wayland
runuser -u nobody -- dbus-run-session -- bash platforms/linux/tests/runtime/wayland_smoke.sh "$installed_host" /resources /build/cargo/debug/examples/prepare_host platforms/linux/tests/runtime/qt_smoke.py --wayland --qt6


runuser -u nobody -- dbus-run-session -- bash platforms/linux/tests/runtime/wayland_smoke.sh "$installed_host" /resources /build/cargo/debug/examples/prepare_host platforms/linux/tests/runtime/portal_smoke.py

# The staged uninstall cannot reach any user's systemd manager or session, so it names the units to disable and the input method lists to clean up instead of touching them, and still removes the program files.
uninstall_log=$(DESTDIR=/build/stage cmake -P /build/ibus/uninstall.cmake)
grep -F "systemctl --user disable --now msime-linux-online.socket msime-linux-online.service msime-linux-voice.socket msime-linux-voice.service msime-linux-clipboard.service" <<<"$uninstall_log" >/dev/null
grep -F "MSIME from the current group in fcitx5-configtool" <<<"$uninstall_log" >/dev/null
test ! -e /build/stage/usr/local/bin/msime-linux-ibus
test ! -e /build/stage/usr/local/etc/xdg/autostart/msime-linux-clipboard.desktop
echo "Staged uninstall names the user units and input method lists, and removes the programs"

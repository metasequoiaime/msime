#!/usr/bin/env bash
# The repository's text contracts: every scripts/test-*.py except the special checks named below, plus the registry phase that keeps that list honest.
#
# Two callers. verify-local.sh sources this after its inline special phases, so the output there is unchanged and failures land in its own `failed`. The contracts workflow (.github/workflows/contracts.yml) runs it on its own on an ubuntu runner, which is why every check found here has to skip cleanly and exit 0 when the tool or input it needs is absent.
#
# Run on its own it needs nothing beyond the checkout and exits non-zero when any check fails.

if [ "${BASH_SOURCE[0]}" = "$0" ]; then
  set -uo pipefail
  cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)" || exit 2
  failed=0
  note() { printf '\n=== %s ===\n' "$1"; }
  fail() { echo "FAIL: $1"; failed=1; }
  run_checks_standalone=1
else
  run_checks_standalone=0
fi

# Every scripts/test-*.py is a contract check, and the loop at the end of this section runs each one without it being named anywhere: a check is wired the moment its file exists. The hand-kept list this replaced had already forgotten one (test-clipboard-capture-bounds.py sat in scripts/ unrun). A check that cannot run bare - it needs arguments, a lock, or is run by another gate - is named in special_checks instead, paired with the script that runs it (or, for the Playwright pair no gate runs, the document that says how to), and the registry phase fails when that script does not invoke it or the document does not mention it. Because nothing opts a check in, a new one has to skip cleanly and exit 0 when its toolchain or input is missing; CONTRIBUTING.md says so for contributors.
#
# Why the discovered checks exist, keyed by the part of the file name after test-:
#
# - conflict-markers: the pre-commit hook only ever sees the commit that introduces a marker, so a marker already in HEAD is invisible to it forever. Six of them lived in docs/windows-parity.md until this scan existed.
# - tracked-symlinks: same shape as the marker scan: a symlink pointing at one machine's absolute path breaks every other checkout, and the checkout it was made on is the one place it keeps working.
# - file-locking-goes-through-client-core: same shape again, one target further out: `std::fs::File::lock` compiles for Android and then fails at runtime, so only a keyboard running on a handset ever finds out.
# - windows-config-keys: the reference's factory configuration is the most complete list of what that product can be told to do. A key it has and this repository does not is a feature nobody migrated, and nothing else would notice. Skips without a reference checkout beside the main worktree.
# - reference-ui-actions: the configuration is what the reference can be told to do; this is what its interface can ask the host to do. Between them they cover the capability surface from both sides.
# - reference-feature-log: keys and actions both miss a feature that changes behaviour without adding either. The reference's changelog does not, because it is generated from its own commits.
# - reference-source-inventory: windows-config-keys, reference-ui-actions and reference-feature-log look at the reference from the outside - its configuration, its interface, its changelog. This one walks its source tree, which is the only place a file nobody migrated can still be hiding.
# - preferences-field-parity: a settings-page key the Rust document has no field for does not get dropped: deny_unknown_fields fails the whole save. Cheap enough to run in --quick, and it is the pre-merge gate that would have caught it.
# - shell-route-parity: the tray menu hands the shared Tauri shell a route string that C++ builds and Rust parses. Both sides pass their own tests on their own vocabulary, and a name renamed on one side alone breaks nothing visible: an unparseable route is not an error, it opens the ordinary settings window, so the row keeps working and opens the wrong thing.
# - settings-action-guard: one settings page, six hosts, and the things a host can do arrive as optional callbacks. A button that calls one without checking it is present renders live where the feature does not exist and does nothing when pressed.
# - no-host-dialogs: settings-action-guard checks that a button is gated on the host being able to do the thing. It cannot see a button gated on a dialog the host never shows: `window.confirm` returns false with nothing on screen under wry's WKWebView, so on macOS and iOS those buttons did nothing at all.
# - support-channels: one group number and one link, written out on four surfaces because nothing shares them. A typo in any one of them sends that platform's users to a group that does not exist, and every copy is correct on its own terms, so only comparing them catches it.
# - android-voice-project-config: the Android voice plugin is Kotlin, and nothing on this machine or on a runner compiles it: the APK build does, and that runs in neither place. These assertions are what stands in for a compiler on its wiring. The file existed for that reason and was not being run at all, so when the voice commands moved out of lib.rs it went red and stayed red unnoticed.
# - android-preference-keys: a switch that writes a retired preference key compiles, renders and saves; only reading the shared crate tells you it does nothing. 自动纠错 was in that state on this host for as long as the key has been retired, showing checked for a correction that was off and unreachable.
# - ios-project-config: the iOS counterpart of android-voice-project-config went the same way, and further: it was never wired here at all. Seven assertions rotted silently while the code they pin moved on - the voice types were generalised from Ios* to Mobile* for Android, the voice commands left lib.rs for voice.rs, the onboarding skip button became a CSS-module class, and the keyboard bridging header and two frontend modules moved into their <feature> directories. Nothing built the XcodeGen project on this machine or on a runner either, so the same reasoning that wired the Android one applies here.
# - reference-config-coverage: the reference ships one file with a default for every setting it has. Comparing the two settings pages by eye has been done repeatedly and keeps producing the same false results in both directions, so the mapping is written down and checked instead - including, when a reference checkout is on the machine, that nothing new has appeared upstream without a home here.
# - settings-label-parity: the parity checks compare identifiers, and an identifier being right says nothing about the words printed next to it: both Linux menus spelled the 首右 helpcode schemes 搜狗, which is a different company's input method, and the macOS backend page named the paging choices in words where the settings window showed the keys. Every identifier around both was correct.
# - quick-phrase-limit: a quick phrase ends up in the candidate pipe's text field, whose size shared/contracts/ipc_protocol_limits.h declares. The limit on it was six bare literals across three crates, none attached to that header, so resizing the field would have changed it and nothing else.
# - handwriting-limits: the panel and the shared contract cap strokes, points and candidates separately. The panel may be stricter, never looser: past the contract the user draws and recognition silently returns nothing, because the request was refused before it reached a recogniser.
# - cloud-request-budget: how long a cloud candidate is worth waiting for belongs to the product, but each host reaches the network with its own library and can shorten it on its own. Two of them had, and a dropped cloud candidate looks exactly like a query that had no cloud answer.
# - phrase-preedit-hosts: a host either holds a half-composed phrase in the composition and draws it, or commits each piece as it is picked. Half of that is invisible in the worst way: a host that asks for the piece to be held and draws it nowhere shows nothing at all for text the user already chose.
# - preference-suite-cleanup: a macOS test that opens an NSUserDefaults suite writes a plist into the user's Preferences directory, and emptying the domain does not delete the file. Every run of a test that forgets leaves one behind, on every machine, forever.
# - candidate-sources: whether the candidate right-click actions are offered is decided on the engine's CandidateSource value (crates/engine/src/types.rs), which arrives as a number this side cannot name in C++. Renumbering a source there compiles cleanly and starts offering 删除 for cloud suggestions.
# - clipboard-capture-bounds: whether a clipboard entry is storable is decided by crates/client-core/src/clipboard.rs alone. Android once kept a second copy of that rule that counted UTF-16 units instead of graphemes, so this fails when a host starts deciding it again.
# - offline-glosses: the offline glosses for the non-English targets are built from Wiktionary rows whose shape is easy to misread: the Mandarin rows are "Chinese Mandarin", the plain "Chinese" ones are topolects, and senses[] repeats the top-level tables. The fixture holds real rows, so a rule that drifts from them fails here instead of in a release.
# - korean-hanja-table: the Hanja table is generated from a pinned libhangul file and committed so the engine can embed it, and nothing rebuilds it: a hand edit, or a generator change committed without rerunning it, would ship as it is. This checks the table's invariants offline, and regenerates it for comparison when the pinned source is already cached.
# - language-data-notices: the Cantonese, Zhuyin and Stroke data comes from rime-cantonese (CC BY 4.0), libchewing-data (LGPL-2.1-or-later) and rime-stroke (LGPL-3.0), whose texts have to travel through every platform's notice channel. A channel that stops naming one still builds and ships, so only reading the channels catches it, and the licence files name the commit they cover, so a re-pin in the sources lock without them fails here too.
# - scheme-traits-parity: the engine's SchemeType predicates decide how each input scheme behaves, and three places cannot call them: the macOS controller and the Windows Server and TIP copy the ones a view does not publish into their InputSchemeTraits.h, and the settings page keeps its own lists of which schemes are Chinese. A scheme added or moved on one side alone compiles everywhere and shows up only as a key that behaves like the wrong language.
# - settings-palette-parity: the palette is most of what makes one window look like another, and this one is built with Tailwind rather than by importing the source's sheet, so the two copies of the same 64 names can drift a hex at a time without anyone noticing.
# - windows-path-encoding: path::string() converts through the ANSI code page on Windows, so a profile with Chinese characters in it mangles or throws. Nothing about that shows up on a host whose system encoding is UTF-8, which is every host that runs this script - hence a static check rather than a test.
# - installer-prerequisites: the prerequisite check lives in Inno Setup's Pascal Script, which nothing off Windows can compile. This pins the parts a later edit could quietly drop.
# - harmony-bridge-parity: ArkTS decides what the injected bridge exposes twice - the method on the class and its name in registerJavaScriptProxy - and only the second is what the page sees. A name added in one place and not the other type-checks, compiles and builds, then fails on a device as "not a function". It has happened once.
# - harmony-arkts-subset: tsc accepts the whole TypeScript language; the ArkTS compiler that actually builds the HAP does not. Six merged PRs left develop unable to produce a HAP - thirteen errors in three .ets files - and every gate here was green. This checks the two rules that can be checked without the SDK.
# - harmony-unwired-policies: a ported policy with unit tests and no call site is shipped by nobody, and a test suite cannot see that: it imports the module itself. This has got through twice - #3419 wrote four accessibility policies the view never attached, and #3435 was a merge that put that state back, dropping the file and its tests together so the assertion count merely got smaller.
# - linux-rendered-view-guard: rendered_view is null until the first render and after every session rebuild, and nlohmann's value() throws on null. A throw inside the Linux key handler is caught, so the symptom is a silently dropped key and one warning line - the first letter after a Chinese/English toggle. Reproducing it needs a live IBus session with a rebuilt Engine session, which no phase here has.
#
# Each special check as name=file-that-runs-it. The first three run inline in verify-local.sh, which holds the locks and toolchains they need; the Playwright pair needs a served fixture plus --url and --csp, which packages/ui/UPSTREAM.md describes; android-json-null-reads runs inside "android host java".
special_checks="windows-32bit-compile=scripts/verify-local.sh
windows-native-run=scripts/verify-local.sh
harmony-settings-bundle=scripts/verify-local.sh
android-json-null-reads=platforms/android/check-host.sh
appearance-preview=packages/ui/UPSTREAM.md
skin-palette-csp=packages/ui/UPSTREAM.md"

# A special check is only exempt from the loop because the file it names runs it. Without this, adding a name to special_checks would be a quiet way to switch a check off, and a stale name would hide a renamed script.
note "contract check registry"
special_names=""
registry_ok=1
for entry in $special_checks; do
  name="${entry%%=*}"
  owner="${entry#*=}"
  special_names="$special_names $name"
  if [ ! -f "scripts/test-$name.py" ]; then
    fail "special check $name: scripts/test-$name.py does not exist"
    registry_ok=0
  elif [ "${owner%.sh}" != "$owner" ]; then
    # A shell owner has to invoke the check on a live line; a comment that merely mentions the path does not count.
    if ! grep -qE "^[^#]*python3 [^#]*scripts/test-$name\.py" "$owner" 2>/dev/null; then
      fail "special check $name: $owner does not run scripts/test-$name.py"
      registry_ok=0
    fi
  elif ! grep -qF "scripts/test-$name.py" "$owner" 2>/dev/null; then
    fail "special check $name: $owner does not document scripts/test-$name.py"
    registry_ok=0
  fi
done
[ "$registry_ok" -eq 1 ] && echo "contract check registry: every special check is run by the script it names, or documented where no gate runs it"

# Everything else, in file-name order. The checks are independent of each other and almost all of them are single-threaded Python reading the tree, so they run in a bounded pool: one after another they cost about half a minute of every push, which is most of the gate's time on a push that builds nothing. Each check's output goes to its own file and is printed afterwards in file-name order under its own header, so what is printed and what fails is the same as when they ran one by one. The pool is MSIME_CHECK_JOBS wide, else CARGO_BUILD_JOBS (rbuild sets it to the cores it may use on the Studio), else the core count capped at six. PYTHONUNBUFFERED keeps a check's stdout and stderr in the order it wrote them now that neither is a terminal.
check_jobs="${MSIME_CHECK_JOBS:-${CARGO_BUILD_JOBS:-}}"
if [ -z "$check_jobs" ]; then
  check_jobs="$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 1)"
  [ "$check_jobs" -gt 6 ] 2>/dev/null && check_jobs=6
fi
case "$check_jobs" in '' | *[!0-9]* | 0) check_jobs=1 ;; esac
checks_dir="$(mktemp -d)"
# Sourced, verify-local.sh's own EXIT trap removes checks_dir; on its own this script has no other trap to share.
[ "$run_checks_standalone" -eq 1 ] && trap 'rm -rf -- "$checks_dir"' EXIT
pending=""
for check in scripts/test-*.py; do
  name="${check#scripts/test-}"
  name="${name%.py}"
  case " $special_names " in *" $name "*) continue ;; esac
  pending="$pending$check"$'\n'
done
# xargs's own status is ignored: each check's status is read from the file it leaves, and a check that left none (killed, or never started) counts as failed below.
# shellcheck disable=SC2016  # expanded by the sh that xargs starts, not here
printf '%s' "$pending" | xargs -P "$check_jobs" -I {} sh -c \
  'out="$2/$(basename "$1" .py)"; PYTHONUNBUFFERED=1 python3 "$1" </dev/null >"$out.log" 2>&1; echo $? >"$out.status"' \
  run-check {} "$checks_dir"
for check in $pending; do
  name="${check#scripts/test-}"
  name="${name%.py}"
  note "${name//-/ }"
  cat "$checks_dir/test-$name.log" 2>/dev/null
  [ "$(cat "$checks_dir/test-$name.status" 2>/dev/null)" = 0 ] || fail "${name//-/ }"
done
rm -rf -- "$checks_dir"

if [ "$run_checks_standalone" -eq 1 ]; then
  if [ "$failed" -ne 0 ]; then
    printf '\nrun-checks: at least one contract check failed\n'
    exit 1
  fi
  printf '\nrun-checks: every contract check passed or skipped\n'
fi

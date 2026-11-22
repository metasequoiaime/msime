#!/usr/bin/env bash
set -euo pipefail

# The runner ci-ios.yml calls for the three iOS jobs. It reads its scope from the environment rather than from arguments because all three jobs invoke it the same way and differ only in what they set.
#
#   MSIME_TEST_SCOPE=pr           the unit suites, minus the cases that need ML Kit  (job: iOS Simulator)
#   MSIME_TEST_SCOPE=all          same, on the pull request that promotes develop to main
#   MSIME_TEST_SCOPE=handwriting  only the cases that need ML Kit                    (job: iOS Handwriting)
#   MSIME_TEST_SHARD=heavy        the interface suite                                (job: iOS Simulator (settings and skins))
#   MSIME_TEST_SHARD=rest|<empty> everything the heavy shard does not take
#
# Scope and shard are deliberately separate: the handwriting split is about which runner architecture can link the SDK, and the heavy/rest split is about wall-clock. They answer different questions and a single knob conflated them.
#
# This covers the unit schemes only. MSIMEClientUITests drives a real app launch and MSIMESharedTests/HandwritingTests links MLKitDigitalInkRecognition through CocoaPods, and this tree generates its project into build/ios while its Podfile integrates the checked-in project in platforms/ios -- two different containers. Wiring those two scopes needs that resolved first, so they report as not-yet-covered rather than silently passing.

project_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
cd "$project_root"

scope=${MSIME_TEST_SCOPE:-pr}
shard=${MSIME_TEST_SHARD:-}
derived_data=${MSIME_IOS_DERIVED_DATA:-build/ios-derived}
project=${MSIME_IOS_PROJECT:-build/ios/MSIMEClient.xcodeproj}

# The three cases that assert on real ink: the SDK ships no simulator slice for arm64, so only the Intel job can link them. This list is the single definition of that line -- the unit scopes skip exactly these, the handwriting scope runs exactly these, so the two cannot drift apart.
recognition_cases=(
  MSIMESharedTests/HandwritingTests/testRealChineseInkRecognition
  MSIMESharedTests/HandwritingTests/testCommonCharactersFromPenTrajectories
  MSIMESharedTests/HandwritingTests/testCandidateSelectionInsertsOnlyAfterConfirmation
)

if [[ ! -d "$project" ]]; then
  echo "Generated Xcode project not found: $project" >&2
  echo "ci-ios.yml generates it with: xcodegen generate --spec platforms/ios/project.yml --project build/ios --project-root platforms/ios" >&2
  exit 1
fi

# A concrete UDID beats a name: the runner image renames devices between Xcode releases, and a `name=` destination that no longer matches makes xcodebuild pick something arbitrary rather than fail.
if [[ -n "${IOS_SIMULATOR_UDID:-}" ]]; then
  device=$IOS_SIMULATOR_UDID
else
  device=$(xcrun simctl list devices available --json | python3 -c '
import json, sys
runtimes = json.load(sys.stdin)["devices"]
best = None
for runtime, devices in runtimes.items():
    if "iOS" not in runtime:
        continue
    for device in devices:
        if not device.get("isAvailable") or "iPhone" not in device["name"]:
            continue
        # Highest runtime, then highest model number, so the choice does not move when the image adds an older runtime.
        key = (runtime, device["name"])
        if best is None or key > best[0]:
            best = (key, device["udid"])
if best is None:
    raise SystemExit("no available iPhone simulator")
print(best[1])
')
fi
echo "Simulator: $device"
echo "Scope: $scope  Shard: ${shard:-<none>}  Project: $project"

case "$scope:$shard" in
  handwriting:*)
    echo "::notice::The handwriting scope needs the CocoaPods workspace that carries MLKitDigitalInkRecognition; this tree generates its project into build/ios while the Podfile integrates platforms/ios. Not covered yet."
    exit 0
    ;;
  *:heavy)
    echo "::notice::The heavy shard is the MSIMEClientUITests interface suite, which drives a real app launch. Not covered yet."
    exit 0
    ;;
esac

# Both unit schemes, in one build. MSIMEClientTests carries MSIMEKeyboardTests, MSIMEServiceTests and MSIMESharedTests; MSIMEDoubaoTransportTests is its own scheme because the transport builds standalone.
schemes=(MSIMEClientTests MSIMEDoubaoTransportTests)

status=0
for scheme in "${schemes[@]}"; do
  echo "──────── $scheme ────────"
  arguments=(
    -project "$project"
    -scheme "$scheme"
    -destination "platform=iOS Simulator,id=$device"
    -derivedDataPath "$derived_data"
    CODE_SIGNING_ALLOWED=NO
  )
  # Only the run phase writes a result bundle. Passing -resultBundlePath to both phases makes the
  # second invocation fail with "Existing file at -resultBundlePath", because the build phase has
  # already created it. Removing a bundle left by an earlier attempt keeps a re-run idempotent.
  result_bundle="$derived_data/Logs/Test/$scheme.xcresult"
  rm -rf "$result_bundle"
  mkdir -p "$(dirname "$result_bundle")"
  # Only MSIMEClientTests contains the handwriting cases; passing -skip-testing for a target a scheme does not build makes xcodebuild fail outright.
  if [[ "$scheme" == MSIMEClientTests ]]; then
    for case_name in "${recognition_cases[@]}"; do
      arguments+=(-skip-testing:"$case_name")
    done
  fi
  # Separate phases so a compile failure is reported as a compile failure rather than as a test run that produced nothing.
  xcodebuild "${arguments[@]}" build-for-testing
  xcodebuild "${arguments[@]}" -resultBundlePath "$result_bundle" test-without-building || status=$?
done

exit "$status"

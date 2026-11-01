#!/usr/bin/env bash
set -euo pipefail
repo_root=$(cd "$(dirname "$0")/../../../.." && pwd)
cd "$repo_root"
android_sdk=${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}
adb="$android_sdk/platform-tools/adb"
serial=${1:-emulator-5580}
settings=false
handwriting=false
statistics=false
core=false
feedback=false
diagnostics=false
text_edit_layout=false
oobe=false
crash_only=false
# 单次 instrumentation 默认不设上限：完整套件里 NumberPadDeviceSmoke 一项就要跑约 6.5 分钟，整套的总时长由 workflow 的 timeout-minutes 兜底。需要给单次运行加上限时设置 MSIME_DEVICE_INSTRUMENTATION_TIMEOUT_SECONDS（秒）。
instrumentation_timeout_seconds=${MSIME_DEVICE_INSTRUMENTATION_TIMEOUT_SECONDS:-}
[[ -z "$instrumentation_timeout_seconds" || "$instrumentation_timeout_seconds" =~ ^[1-9][0-9]*$ ]] || { echo "MSIME_DEVICE_INSTRUMENTATION_TIMEOUT_SECONDS must be a positive integer" >&2; exit 1; }
for option in "${@:2}"; do
  case "$option" in
    # --core 跑安装、词库准备、打字主路径及剪贴板保存生命周期；其余键盘套件在 API 28 上仍不支持。
    --core) core=true ;;
    --settings) settings=true ;;
    --handwriting) handwriting=true ;;
    --statistics) statistics=true ;;
    --feedback) feedback=true ;;
    --diagnostics) diagnostics=true ;;
    --text-edit-layout) text_edit_layout=true ;;
    --oobe) oobe=true ;;
    --crash-only) crash_only=true ;;
    *) echo "usage: smoke.sh [emulator-5580] [--core] [--settings] [--handwriting] [--statistics] [--feedback] [--diagnostics] [--text-edit-layout] [--oobe] [--crash-only]" >&2; exit 1 ;;
  esac
done
[[ "$serial" == emulator-* ]] || { echo "Only the dedicated emulator is supported" >&2; exit 1; }
avd_name=$("$adb" -s "$serial" emu avd name | tr -d '\r' | head -1)
[[ "$avd_name" == msime-client-test || "$avd_name" == msime-client-test-api* ]] || { echo "Refusing a non-test AVD" >&2; exit 1; }
api_level=$("$adb" -s "$serial" shell getprop ro.build.version.sdk | tr -d '\r')
[[ "$api_level" =~ ^[0-9]+$ ]] || { echo "Could not read Android API level" >&2; exit 1; }
# 输入法 id 必须写成 `ime list` 给出的短写：系统按字符串原样查找，长写 app.msime.android/app.msime.android.MSIMEInputService 在 `ime enable` 时报 Unknown input method。设备测试里的 `ime` 命令都用同一个短写。
ime_id=app.msime.android/.MSIMEInputService
# ---- 失败收集 ----
# 一项验收失败不结束整套：记下失败，收拾它可能留在进程外的状态，接着跑后面互不依赖的检查，最后在一处列出全部失败并以非零退出。以前第一处失败就退出，排在后面的独立问题要等前面的修好才看得见，2026-10-25 到 10-27 的夜间每晚停在不同的地方，就是每晚只看得到当时最靠前的那一个。失败不重试：重试会把真实的竞态一起盖住。
# 后面的检查已经没有意义时照旧当场停下，并在汇总里写明原因：装包、词库准备或选中输入法失败；系统输入主路径（DeviceSmoke）失败，后面每一项都经同一个键盘打字；模拟器不在了；instrumentation 进程崩溃，它的 finally 没有执行，设备和键盘处于未知状态；连续五项失败，多半是键盘整体不可用，再跑只会得到一串同源的失败，还可能拖过 job 的 90 分钟时限，连日志都收不回来。
failures=()
consecutive_failures=0
stop_reason=""
# 失败汇总写到 stderr；CI 上同时写进 $GITHUB_STEP_SUMMARY，在运行页面上就能看到全部失败。
report() {
  local entry heading
  heading="Android device acceptance on $serial ($avd_name): ${#failures[@]} failed"
  {
    echo "==== $heading ===="
    if [[ ${#failures[@]} -gt 0 ]]; then
      for entry in "${failures[@]}"; do echo "FAILED $entry"; done
    fi
    [[ -z "$stop_reason" ]] || echo "STOPPED $stop_reason; the checks after this point did not run"
    echo "===="
  } >&2
  [[ -n "${GITHUB_STEP_SUMMARY:-}" ]] || return 0
  # 失败行放进 Markdown 行内代码，原文里的反引号换成单引号，免得截断代码段。
  {
    echo "### $heading"
    echo
    if [[ ${#failures[@]} -gt 0 ]]; then
      for entry in "${failures[@]}"; do
        entry=${entry//\`/\'}
        echo "- \`$entry\`"
      done
    fi
    [[ -z "$stop_reason" ]] || echo "- Stopped: $stop_reason; the checks after this point did not run"
    echo
  } >> "$GITHUB_STEP_SUMMARY"
}
# finish [通过时打印的一行]：没有失败就照旧打印通过说明并以 0 退出，否则打印汇总并以 1 退出。
finish() {
  if [[ ${#failures[@]} -eq 0 && -z "$stop_reason" ]]; then
    [[ -z "${1:-}" ]] || echo "$1"
    exit 0
  fi
  report
  exit 1
}
stop() {
  stop_reason=$1
  echo "Stopping the device acceptance run: $1" >&2
  finish
}
device_alive() {
  [[ $("$adb" -s "$serial" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r') == 1 ]]
}
# 清除合成锁屏 PIN（DeviceCredentialFixture.PIN）：用到它的套件中途失败或崩溃时来不及清，留下的锁屏会挡住后面的套件。只在确有凭据时清除，理由见 clipboard-export.sh 的 clear_credential。
clear_credential() {
  if ! "$adb" -s "$serial" shell locksettings verify 2>/dev/null | grep -q "verified successfully"; then
    "$adb" -s "$serial" shell locksettings clear --old 2580 >/dev/null 2>&1 || true
  fi
}
# 确认系统的默认输入法是水杉，连续四次读到才算数；读到别的就再选一次，十五秒内稳不住返回 1。停用输入法后系统会异步回退到别的键盘，命令返回不代表选择已经生效，和 DeviceSmoke.restartIme 是同一个判据。
confirm_ime() {
  local attempt settled=0
  for attempt in $(seq 1 60); do
    if [[ $("$adb" -s "$serial" shell settings get secure default_input_method 2>/dev/null | tr -d '\r') == "$ime_id" ]]; then
      settled=$((settled + 1))
      (( settled < 4 )) || return 0
    else
      settled=0
      "$adb" -s "$serial" shell ime set "$ime_id" >/dev/null 2>&1 || true
    fi
    sleep 0.25
  done
  return 1
}
select_ime() {
  "$adb" -s "$serial" shell ime enable "$ime_id" >/dev/null 2>&1 \
    && "$adb" -s "$serial" shell ime set "$ime_id" >/dev/null 2>&1 \
    && confirm_ime
}
# 失败的套件在自己进程的 finally 里还原偏好、键盘层和测试夹具，这里只收它留在进程外的东西：前台的合成编辑器、系统文件选择器和设置页（Android 9、10 的锁屏凭据确认在设置里）、合成锁屏 PIN、被换掉的默认输入法，和 clipboard-export.sh 的 restore_ime 收的是同一类。不强停产品：那会连 :ime 一起停掉，见 run_instrumentation 上面的说明。
reset_after_failure() {
  "$adb" -s "$serial" shell am force-stop app.msime.android.test >/dev/null 2>&1 || true
  "$adb" -s "$serial" shell am force-stop com.android.documentsui >/dev/null 2>&1 || true
  "$adb" -s "$serial" shell am force-stop com.android.settings >/dev/null 2>&1 || true
  clear_credential
  select_ime
}
# 失败输出里最能说明问题的一行：套件自己报的 *_FAILED 行，其次是 am instrument 或 adb 的错误行，都没有就取最后一行非空输出。
failure_detail() {
  local line last=""
  while IFS= read -r line; do
    line=${line%$'\r'}
    case "$line" in
      *MSIME_*FAILED*|INSTRUMENTATION_FAILED*|INSTRUMENTATION_ABORTED*|*shortMsg=*|Error:*|error:*)
        printf '%s' "$line"
        return 0
        ;;
    esac
    [[ -z "${line//[[:space:]]/}" ]] || last=$line
  done <<< "$1"
  printf '%s' "${last:-no output}"
}
# record_failure <失败说明> <输出>：记下一项失败，再判断后面的检查还有没有意义。
record_failure() {
  local label=$1 output=$2
  echo "$label" >&2
  failures+=("$label: $(failure_detail "$output")")
  consecutive_failures=$((consecutive_failures + 1))
  device_alive || stop "the emulator is gone or no longer reports boot completed"
  case "$output" in
    *"shortMsg=Process crashed"*|*INSTRUMENTATION_ABORTED*)
      stop "the instrumentation process crashed, so its cleanup did not run and the device is in an unknown state" ;;
  esac
  (( consecutive_failures < 5 )) || stop "five checks in a row failed; the keyboard is probably unusable and later failures would share that cause"
  reset_after_failure || stop "$ime_id could not be selected again after the failure"
}
# verdict <失败说明> <通过标记> <输出> <退出码>：命令成功且输出里有通过标记才算通过；非零退出不能被已经打印的通过标记盖掉。
verdict() {
  local output=$3 status=$4
  if (( status == 0 )) && [[ "$output" == *"$2"* ]]; then
    consecutive_failures=0
  else
    if (( status != 0 )); then
      output="error: command exited with status $status"$'\n'"$output"
    fi
    record_failure "$1" "$output"
  fi
}
# check <失败说明> <通过标记> <命令…>：跑一项 instrumentation 验收，打印它的输出，再交给 verdict。adb 的错误输出一并收下，失败时能进汇总。
check() {
  local label=$1 marker=$2 output status=0
  shift 2
  output=$("$@" 2>&1) || status=$?
  printf '%s\n' "$output"
  verdict "$label" "$marker" "$output" "$status"
}
# check_script <失败说明> <脚本> <参数…>：跑一个自带前置检查和清理（trap）的辅助脚本，以退出码判定，输出照常实时打印。
check_script() {
  local label=$1 log=target/android/device-test/check-script.log status=0
  shift
  bash "$@" 2>&1 | tee "$log" || status=$?
  if (( status == 0 )); then
    consecutive_failures=0
  else
    record_failure "$label" "$(<"$log")"
  fi
}
# ---- 前置条件：这里失败，后面的检查都跑不起来 ----
device_alive || stop "Test AVD has not booted"
if [[ "$crash_only" == true ]]; then
  MSIME_ANDROID_CRASH_REPORTING=1 bash platforms/android/tests/device/build-editor.sh || stop "building the crash reporting test APK failed"
elif [[ "$oobe" == true ]]; then
  MSIME_ANDROID_OOBE_SMOKE=1 bash platforms/android/tests/device/build-editor.sh || stop "building the device test APKs failed"
else
  bash platforms/android/tests/device/build-editor.sh || stop "building the device test APKs failed"
fi
# 先推送完整 APK 再交给包管理器，避免 API 28 的流式安装桥卡住而阻塞整套验收。
"$adb" -s "$serial" install --no-streaming --no-incremental -r target/android/msime-android.apk || stop "installing msime-android.apk failed"
"$adb" -s "$serial" install --no-streaming --no-incremental -r target/android/editor-test.apk || stop "installing editor-test.apk failed"
"$adb" -s "$serial" install --no-streaming --no-incremental -r target/android/first-frame-editor.apk || stop "installing first-frame-editor.apk failed"
# 同包名搜索夹具只在专用兼容验收时安装，不进入默认设备测试。
if [[ "${MSIME_ANDROID_APP_COMPOSITION:-0}" == 1 ]]; then
  "$adb" -s "$serial" install --no-streaming --no-incremental -r target/android/composition-search-editor.apk || stop "installing composition-search-editor.apk failed"
fi
mkdir -p target/android/device-test
xml="$repo_root/target/android/device-test/window.xml"
dump() {
  "$adb" -s "$serial" shell uiautomator dump /data/local/tmp/msime-test-window.xml >/dev/null \
    && "$adb" -s "$serial" pull /data/local/tmp/msime-test-window.xml "$xml" >/dev/null 2>&1
}
# 点 $xml 这份 dump 里的目标，不另取 dump；目标不在里面时返回 1，由调用方决定是否重试。
tap() {
  bounds=$(xmllint --xpath "string(($1)[1]/@bounds)" "$xml")
  [[ "$bounds" =~ ^\[([0-9]+),([0-9]+)\]\[([0-9]+),([0-9]+)\]$ ]] || return 1
  "$adb" -s "$serial" shell input tap "$(( (BASH_REMATCH[1] + BASH_REMATCH[3]) / 2 ))" "$(( (BASH_REMATCH[2] + BASH_REMATCH[4]) / 2 ))"
}
# The host prepares the shipped dictionary itself on first run; there is no button to press for it any more. The 设置 tab shows the state only while preparing or after a failure, so the tab having rendered with no preparation notice is what readiness looks like.
# A fresh install plays the first-launch splash and then opens onboarding over the home screen; the loop skips onboarding when it finds it, and the splash dismisses itself.
# The 设置 tab is drawn under the splash, so 试用键盘 is in the dump while the splash still covers it, and onboarding is only started as the splash begins to fade. Readiness therefore also needs the splash gone, and has to hold on two polls a second apart so an onboarding window that is still opening gets its chance to appear and be skipped.
"$adb" -s "$serial" shell am start -W -n app.msime.android/app.msime.android.home.HomeActivity >/dev/null
ready=false
settled=0
for attempt in $(seq 1 60); do
  # 应用刚启动、窗口还在切换时，uiautomator 偶尔报 `ERROR: null root node returned by UiTestAutomationBridge.` 并以非零退出；set -e 下这一次失败会让整个冒烟在第一条用例之前退出（API 35 上出现过）。轮询里把它当作这一拍还没就绪。
  if ! dump; then settled=0; sleep 1; continue; fi
  if [[ $(xmllint --xpath 'boolean(//node[contains(@text,"词库准备失败")])' "$xml") == true ]]; then stop "Device bootstrap failed"; fi
  if [[ $(xmllint --xpath 'boolean(//node[contains(@resource-id,":id/onboarding_skip")])' "$xml") == true ]]; then
    settled=0
    # 用这一拍刚读到的 dump 去点：tap 原先会另取一份 dump，引导页在两次 dump 之间可能正在打开或关闭，另取的那份偶尔是空的（null root），找不到按钮就让整个冒烟在第一条用例之前退出（API 28 上出现过）。这一拍点不到就当作还没就绪，下一拍再看。
    tap '//node[contains(@resource-id,":id/onboarding_skip")]' || true
    sleep 1
    continue
  fi
  if [[ $(xmllint --xpath 'boolean(//node[contains(@text,"试用键盘")])' "$xml") == true \
     && $(xmllint --xpath 'boolean(//node[contains(@text,"正在准备词库")])' "$xml") == false \
     && $(xmllint --xpath 'boolean(//node[contains(@resource-id,":id/home_intro")])' "$xml") == false ]]; then
    settled=$((settled + 1))
    if (( settled >= 2 )); then ready=true; break; fi
  else
    settled=0
  fi
  sleep 1
done
[[ "$ready" == true ]] || stop "Device bootstrap timed out"
sdk=$("$adb" -s "$serial" shell getprop ro.build.version.sdk | tr -d '\r')
[[ "$sdk" =~ ^[0-9]+$ ]] || stop "Android API level was unavailable"
# API 31 起可附着到已运行的产品进程。普通 instrument 若以产品包为目标，会连同 :ime 服务一起强停；连续跑设备套件时，系统偶尔来不及重新绑定输入法。以测试 APK 为目标的用例继续正常启动 instrumentation。
run_instrumentation() {
  local component=$1 class=${1#*/} target
  # 引导专项替换这个清单入口，目标进程仍与原入口相同。
  if [[ "$oobe" == true && "$class" == app.msime.android.test.OnboardingDeviceSmoke ]]; then
    class=app.msime.android.test.CandidateCanvasDeviceSmoke
  fi
  target=$(xmllint --xpath "string(//instrumentation[@*[local-name()='name']='$class']/@*[local-name()='targetPackage'])" \
    platforms/android/tests/device/AndroidManifest.xml)
  [[ "$target" == app.msime.android || "$target" == app.msime.android.test ]] \
    || { echo "Unknown instrumentation target: $component" >&2; return 1; }
  local limit=()
  [[ -z "$instrumentation_timeout_seconds" ]] || limit=(timeout --foreground "${instrumentation_timeout_seconds}s")
  if (( sdk >= 31 )) && [[ "$target" == app.msime.android ]]; then
    "$adb" -s "$serial" shell am start -W -n app.msime.android/app.msime.android.home.HomeActivity >/dev/null
    ${limit[@]+"${limit[@]}"} "$adb" -s "$serial" shell am instrument --no-restart "${@:2}" -w "$component"
  else
    ${limit[@]+"${limit[@]}"} "$adb" -s "$serial" shell am instrument "${@:2}" -w "$component"
  fi
}
"$adb" -s "$serial" shell ime enable "$ime_id" || stop "enabling $ime_id failed"
"$adb" -s "$serial" shell ime set "$ime_id" || stop "selecting $ime_id failed"
confirm_ime || stop "$ime_id did not stay selected"
"$adb" -s "$serial" shell am force-stop app.msime.android.test
# 崩溃专项只准备真实产品进程和测试夹具；完整输入套件会覆盖同一批 instrumentation，既浪费设备时间，也会让 UI 套件的已知问题遮住崩溃链路。
if [[ "$crash_only" == true ]]; then
  python3 platforms/android/tests/device/telemetry-watch.py "$serial" || stop "Android telemetry watch acceptance failed"
  if (( api_level >= 30 )); then
    python3 platforms/android/tests/device/telemetry-consent.py "$serial" || stop "Android system-exit acceptance failed"
    python3 platforms/android/tests/device/telemetry-anr.py "$serial" || stop "Android ANR acceptance failed"
    finish "Dedicated Android AVD: crash, system-exit and ANR reporting acceptance passed"
  fi
  echo "API $api_level has no ApplicationExitInfo; system-exit and ANR replay are not applicable" >&2
  finish "Dedicated Android AVD: Java/JNI crash and consent acceptance passed"
fi
# ---- 验收：失败记下后继续，只有上面 record_failure 列出的情况才停 ----
check "System input acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.DeviceSmoke
[[ ${#failures[@]} -eq 0 ]] || stop "the system input path failed, and every later check types through the same keyboard"
if [[ "$oobe" == true ]]; then
  check "原生引导保存与系统状态验收失败" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.OnboardingDeviceSmoke
  finish "Dedicated Android AVD: OOBE resource setup, system state and save recovery acceptance passed"
fi
if [[ "$text_edit_layout" == true ]]; then
  check "Text edit layout acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.TextEditRecentClipboardDeviceSmoke -e layout-only true
  finish
fi
# 十八键复用已有入口，API 28 核心验收与 API 35 完整验收都覆盖组码、回落和实际几何。
check "十八键全拼验收失败" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.FourteenKeyDeviceSmoke -e eighteen_key true
if [[ "$core" != true ]]; then
  check "Japanese toggle input acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.ChineseLanguageReturnDeviceSmoke -e japanese-toggle true
fi
if [[ "${MSIME_ANDROID_APP_COMPOSITION:-0}" == 1 ]]; then
  check "App composition compatibility acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.CompositionRestartDeviceSmoke -e app-composition true
fi
# Compose 可在批量结束时才执行写入，核心验收覆盖延迟组字和中英往返。
check "Deferred batch composition acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.BatchCompositionDeviceSmoke
# 同一编辑器每次输入后重启连接时，手动数字层不能退回字母页；换输入框仍重置。
check "Number layer restart acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.NumberLayerRestartDeviceSmoke
# 同编辑器的组字重启必须保留候选，外部清空、改写和移动不能复活旧组字。
check "Composition restart acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.CompositionRestartDeviceSmoke
# 中文临时大写用完或取消后返回中文，快速双击仍锁定大写。
check "Chinese Shift return acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.ChineseShiftReturnDeviceSmoke
# 分词操作通过真实分享接收器、共享常用语存储和系统剪贴板验收。
check "Clipboard segment actions acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.ClipboardSegmentActionsDeviceSmoke
# 常用语结果显示在手机顶部，不占诊断行，也不移动剪贴板列表。
check "Clipboard phrase notice acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.ClipboardSegmentActionsDeviceSmoke -e phrase-notice true
# 稀疏横拖直接经过安装产品的触摸分发，最低 API 也检查接手位移与子控件取消。
check "Paged grid gesture acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.PagedTileGridGestureDeviceSmoke
# 菜单复制使用真实系统剪贴板与组字会话，最低 API 的核心验收也必须覆盖。
check "Device information copy acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.CopyDeviceInfoDeviceSmoke
# 云候选描述符经过安装产品的 HTTP 栈，不读取偏好或真实输入。
check "Custom cloud transport acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.CloudCandidateTransportDeviceSmoke
# 在安装产品中区分失败来源；只用合成响应和拒写连接，不读取实际输入。
check "Input failure category acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.InputFailureCategoryDeviceSmoke
# 批量结束才应用的编辑器也要保留组字；限长回退的两批选区回声都不能取消候选。
check "Composition batch acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.CompositionBatchDeviceSmoke
# 旧菜单、展开按钮和拼写回调只检查身份，不写编辑器、不停止当前会话。
check "Stale candidate callback acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.StaleCandidateDeviceSmoke
# 空闲联想只在点选时上屏，自己的选区回声保留候选，外部移动和隐私模式清除候选。
check "Next word prediction acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.StaleCandidateDeviceSmoke -e next-word true
# 横向候选条不展开也能越过分页和分批边界，左右返回后按全局身份选词。
check "Continuous candidate strip acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.CandidatePanelDeviceSmoke -e continuous-strip true
# 从安装产品绘制候选标签，旧系统缺字形时也必须在深浅主题中可见。
check "Candidate heavy equals glyph acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.HeavyEqualsCandidateDeviceSmoke
# 四个新手势及肤色在旧系统缺字时仍保留可见候选。
check "Candidate hand emoji acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.HandEmojiCandidateDeviceSmoke
# 首道双拼完整候选换代后，释义与面板同步视图，连续选词不得停止输入会话。
check "Candidate expansion acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.CandidateExpansionDeviceSmoke
# 候选包读应用私有目录，最低 API 的核心验收同样覆盖配色接入与资源失效。
check "Candidate skin package acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.CandidateSkinDeviceSmoke
# Canvas 像素和动画必须在 Android 的真实解码器与绘制器中验收。
check "Candidate canvas acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.CandidateCanvasDeviceSmoke
if [[ "$feedback" == true ]]; then
  # 使用真实反馈页连续输入合成文字，不点击提交；API 28 的 --core 也能单独带上此回归。
  check "Feedback input stability acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.FeedbackInputStabilityDeviceSmoke
fi
# 剪贴板全文导出涉及存储授权，API 28/35 核心套件都覆盖。
check_script "Clipboard export acceptance failed" platforms/android/tests/device/clipboard-export.sh "$serial"
if [[ "$diagnostics" == true ]]; then
  # 只生成合成日志包，覆盖原生导出到上传请求的契约，不发网络请求。
  check "Diagnostics bundle acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.DiagnosticsBundleDeviceSmoke
  check "Voice diagnostics privacy acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.VoiceDiagnosticsPrivacyDeviceSmoke
  check_script "Diagnostic directory acceptance failed" platforms/android/tests/device/diagnostics-export-directory.sh "$serial"
fi
# 三万条以上的码表也要经应用沙箱的 JNI 和真实 Engine 验证，API 28 的核心套件同样执行。
check "Helpcode import acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.HelpcodeImportDeviceSmoke
# 导入必须经过安装产品的 Java 入口和 JNI，第六项不能等到下一次收起键盘才出现。
check "Dictionary import acceptance failed" MSIME_DICTIONARY_IMPORT_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.DictionaryImportDeviceSmoke
# 分词拖选在最低与目标 API 的真实键盘上覆盖跨行、取消和边缘滚动。
result_status=0
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.ClipboardSegmentDragDeviceSmoke 2>&1) || result_status=$?
printf '%s\n' "$result"
# 清空历史前的系统认证用合成锁屏 PIN（DeviceCredentialFixture.PIN）；套件中途崩溃时它来不及清除，这里不论成败都兜底清一次，免得后面的套件遇到锁屏。
clear_credential
verdict "Clipboard segment drag acceptance failed" MSIME_DEVICE_SMOKE_PASSED "$result" "$result_status"

# 数字默认九宫格涉及常规键面和字面提交，API 28 的核心验收同样覆盖。
check "Number pad acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.NumberPadDeviceSmoke
# 随机数字专项复用数字页入口，覆盖真实原生开关和逐次重排，两版设备都执行。
check "Random digit pad acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.NumberPadDeviceSmoke -e random-digits true
check "Password number pad acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.NumberPadDeviceSmoke -e password-number-pad true
# 导航必须在原生宿主中验收：设置列表有「皮肤」一行，底部第二格是没有返回按钮、顶部为「皮肤 / 词库」分类的社区根页，AI 对话从设置压入二级页。
check "Native chat navigation acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.ChatNavigationDeviceSmoke
# 工具栏编辑入口在最低 API 上也支持，核心验收覆盖真实长按和原生设置跳转。
check "Toolbar editor entry acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.ToolbarEditorDeviceSmoke
# 使用同一系统输入法和合成服务检查长按锁定，不新增 instrumentation 入口。
check_script "长按语音锁定验收失败" platforms/android/tests/device/voice-lifecycle.sh "$serial" hold
# 本地语音句柄经过真实 JNI；不加载模型或录音，最低 API 的核心验收也执行。
check "Local speech handle acceptance failed" MSIME_LOCAL_SPEECH_HANDLES_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.LocalSpeechHandleDeviceSmoke
# 真实 Android JSON 的润色回复回归不录音、不联网，最低 API 的核心验收也执行。
check "Voice polish reply acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.VoicePolishReplyDeviceSmoke
# AI 地址、预设和模型选择经真实 JNI 与原生页面验证；模型目录只连接合成回环服务，最低 API 的核心验收也执行。
check "AI endpoint check acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.AiEndpointCheckDeviceSmoke
# 语音设置目录从 Rust 经真实 JNI 读取；同一套件还验证原生模型页的两项准备状态及运行库导入入口。
check "Shared voice catalog JNI acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.VoiceProviderCatalogDeviceSmoke
# 用合成内存文档核对共享换槽和清空字段策略，不保存或发送凭据。
check "Shared voice settings JNI acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.VoiceProviderSettingsDeviceSmoke
# 语音接口提示与运行时共用校验；只传合成地址，不保存或联网。
check "组合语音入口设置验收失败" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.SettingsDeviceSmoke -e voice-triggers true
check "Shared voice endpoint JNI acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.VoiceEndpointSettingsDeviceSmoke
check "Local voice pause settings acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.SettingsDeviceSmoke -e local-voice-pause true
# 账号与云端的 JNI 入口和注入的会话存储（#8201）经真实 JNI 与 Rust 核对；只用合成会话、不联网，最低 API 的核心验收也执行。
# 其中旧快照队列那一项（迁移计划 P13）要在主进程里真的激活一份整份快照，激活要独占词库维护权；输入法还绑着时 `:ime` 的会话占着词库，Rust 答 snapshot_unavailable。产品里激活只在键盘会话结束以后的 `:ime` 空闲时做，所以这里先停用本输入法让系统解绑它，跑完再启用、选回来，后面的套件照旧用它。
"$adb" -s "$serial" shell ime disable "$ime_id" >/dev/null || stop "disabling $ime_id before the session store check failed"
result_status=0
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.SessionStoreDeviceSmoke 2>&1) || result_status=$?
# 不论这一项成败都先选回输入法；选不回来时后面每一项都会失败在同一处。
select_ime || stop "$ime_id could not be selected again after the session store check"
printf '%s\n' "$result"
verdict "Session store JNI acceptance failed" MSIME_DEVICE_SMOKE_PASSED "$result" "$result_status"
# 原生皮肤删除覆盖真实确认框、锁与原子替换；最低 API 的核心验收也运行。
check "Native saved skin deletion acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.SettingsDeviceSmoke -e skin-delete true
# 背单词内置词书：Bootstrap 解到 files/bootstrap/wordbooks，设置页经真实 JNI 读到八本书并能作答。这是应用私有目录里的解包和文件锁，最低 API 的核心验收也运行；APK 要先用 scripts/fetch_wordbooks.py 生成的词书打包。
check "Bundled wordbook acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.SettingsDeviceSmoke -e vocabulary true
# 公告获取和关闭即使等待独立文件锁，实际设置内容也必须可读；复用设置 runner，覆盖最低 API。
check "Notice queue isolation acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.SettingsDeviceSmoke -e notice-queue true
# 「防止截屏」（#10098）读系统真正拿到的窗口属性：开着时设置应用的 Activity 和对话框窗口带 FLAG_SECURE，关上后重新打开的窗口不带；复用设置 runner，最低 API 也覆盖。
check "Screen capture protection window acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.SettingsDeviceSmoke -e screen-capture true
# 常用语备注经原生设置、真实键盘和本地备份验收；最低 API 的核心验收也执行。
check "Native common phrase note acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.SettingsDeviceSmoke -e phrase-notes true
# 常用语分类覆盖原生管理、纵向展开、正文插入和本地备份；最低 API 同样执行。
check "Native common phrase category acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.SettingsDeviceSmoke -e phrase-categories true
# 触控板通过独立编辑器验收真实 DPAD、选区、临时面板和密码回读，最低 API 同样覆盖。
check "Text trackpad acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.TextEditRecentClipboardDeviceSmoke -e trackpad true

# 真实恢复和撤销涉及应用私有目录，最低 API 的核心验收也执行合成备份回归。
check "Layout backup restore acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.PreferencesDeviceSmoke -e backup-layout true

if [[ "$core" == true ]]; then
  check_script "Clipboard save lifecycle acceptance failed" platforms/android/tests/device/clipboard-edit-lifecycle.sh "$serial" app.msime.android
  finish "Dedicated Android AVD: install, resource setup, core typing and clipboard save lifecycle acceptance passed"
fi
# MSIME_DEVICE_SMOKE_SKIP：空格分隔的套件名。这些套件已知失败、正在修（原因写在调用方，见 .github/workflows/android-device.yml），跳过时明确打印出来，不算通过。
skipped_suites=()
for entry in \
    "CandidatePanelDeviceSmoke|Candidate panel acceptance failed" \
    "MoreToolsDeviceSmoke|More tools acceptance failed" \
    "ToolbarEdgesDeviceSmoke|Toolbar fixed edges acceptance failed" \
    "ToolbarScrollDeviceSmoke|Toolbar swipe acceptance failed" \
    "EmojiPickerDeviceSmoke|Emoji picker acceptance failed" \
    "EmojiCombinationLayoutDeviceSmoke|Combination emoji layout acceptance failed" \
    "EmojiCombinationDeviceSmoke|Combination emoji input acceptance failed" \
    "PairedPunctuationDeviceSmoke|Paired punctuation acceptance failed" \
    "NineKeySidebarPairDeviceSmoke|Nine-key sidebar pair acceptance failed" \
    "LetterHintPunctuationDeviceSmoke|Letter hint punctuation acceptance failed" \
    "SuperscriptShortcutDeviceSmoke|Superscript shortcut acceptance failed" \
    "PreferencesDeviceSmoke|Preferences acceptance failed" \
    "ThemeModeDeviceSmoke|Theme mode acceptance failed" \
    "LanguageNumberKeySwapDeviceSmoke|Language/number key swap acceptance failed" \
    "ChineseLanguageReturnDeviceSmoke|Chinese language return acceptance failed" \
    "BottomPaddingSettingsDeviceSmoke|Bottom padding settings acceptance failed" \
    "BottomPaddingHeightDeviceSmoke|Bottom padding height acceptance failed" \
    "KeyboardHeightDeviceSmoke|Keyboard height acceptance failed" \
    "FuzzyPinyinDeviceSmoke|Fuzzy pinyin acceptance failed" \
    "CandidateGlossDeviceSmoke|Candidate gloss acceptance failed" \
    "InitialKeyboardRowDeviceSmoke|Initial keyboard row acceptance failed" \
    "NineKeyNumericLayoutDeviceSmoke|Nine-key numeric layout acceptance failed" \
    "NineKeyEnglishDeviceSmoke|Nine-key English acceptance failed" \
    "EnglishCorrectionDeviceSmoke|Mobile English correction acceptance failed" \
    "NineKeyPanelDeviceSmoke|Nine-key panel acceptance failed" \
    "QuanpinSeparatorDeviceSmoke|Quanpin separator acceptance failed" \
    "FourteenKeyDeviceSmoke|Fourteen-key acceptance failed" \
    "NineKeyShuangpinDeviceSmoke|Nine-key shuangpin acceptance failed" \
    "ChineseHelpcodeDeviceSmoke|Chinese helpcode acceptance failed" \
    "ChineseHelpcodeOffDeviceSmoke|Chinese helpcode-off Shift letter acceptance failed" \
    "MicrosoftShuangpinDeviceSmoke|Microsoft double-pinyin acceptance failed" \
    "SymbolPanelLayoutDeviceSmoke|Symbol panel first layout acceptance failed" \
    "LongSymbolCellDeviceSmoke|Long symbol cell acceptance failed" \
    "EmailSuffixDeviceSmoke|Email suffix acceptance failed" \
    "NumberRowDeviceSmoke|Number row acceptance failed" \
    "NumberLayerEntryDeviceSmoke|Number layer entry acceptance failed" \
    "QuickDeleteUndoDeviceSmoke|Quick delete undo acceptance failed" \
    "QuickDeletePreviewDeviceSmoke|Quick delete held preview acceptance failed" \
    "EmptyEditorBackspaceDeviceSmoke|Empty editor backspace acceptance failed" \
    "ClipboardImageDeviceSmoke|Clipboard image acceptance failed" \
    "ClipboardCapacityDeviceSmoke|Clipboard capacity acceptance failed" \
    "TextEditRecentClipboardDeviceSmoke|Text edit recent clipboard acceptance failed" \
    "CaretCandidatesDeviceSmoke|Caret candidates acceptance failed" \
    "HardwareTranslationDeviceSmoke|Hardware Ctrl+Enter translation acceptance failed" \
    "AiCandidateSeatDeviceSmoke|AI candidate seat acceptance failed" \
    "WubiLookupDeviceSmoke|Wubi lookup acceptance failed"
do
  suite=${entry%%|*}
  if [[ " ${MSIME_DEVICE_SMOKE_SKIP:-} " == *" $suite "* ]]; then
    echo "SKIPPED (known failure): $suite" >&2
    skipped_suites+=("$suite")
    continue
  fi
  if [[ "$suite" == WubiLookupDeviceSmoke ]]; then
    # 它直接改偏好文件，再让设置进程读新配置；必须重新启动产品主进程。
    check "${entry#*|}" MSIME_DEVICE_SMOKE_PASSED "$adb" -s "$serial" shell am instrument -w "app.msime.android.test/app.msime.android.test.$suite"
  elif [[ "$suite" == ClipboardCapacityDeviceSmoke ]]; then
    check "${entry#*|}" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.CandidateCanvasDeviceSmoke -e capacity true
  else
    check "${entry#*|}" MSIME_DEVICE_SMOKE_PASSED run_instrumentation "app.msime.android.test/app.msime.android.test.$suite"
  fi
done
# 18 键双拼复用字母组布局入口，避免超过 Android 35 的清单数量限制。
check "十八键双拼验收失败" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.FourteenKeyDeviceSmoke -e eighteen_shuangpin true
# 26 键双拼的按下点（#10018）复用微软双拼入口：注入真实触摸，核对边角与键距命中、纠错整句随落点保留和微软 ; 键；宿主注册给引擎的键位几何经 instrumentation 状态交回，存到 target/android/device-test/key-geometry/，用来替换评测的参考布局。
result_status=0
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.MicrosoftShuangpinDeviceSmoke -e shuangpin-touch true 2>&1) || result_status=$?
printf '%s\n' "$result"
verdict "双拼按下点验收失败" MSIME_DEVICE_SMOKE_PASSED "$result" "$result_status"
mkdir -p target/android/device-test/key-geometry
printf '%s\n' "$result" | tr -d '\r' \
  | sed -n 's/^INSTRUMENTATION_STATUS: msime_key_geometry_\([a-z0-9-]*\)=\(.*\)$/\1 \2/p' \
  | while read -r name json; do printf '%s\n' "$json" > "target/android/device-test/key-geometry/$name.json"; done
# 九键逐字选词复用原入口；真实触摸部分选词后面板必须留着并换上剩余组字的候选（#9193），选完最后一个字才收起，全程不发返回操作。
check "Nine-key partial selection acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.NineKeyPanelDeviceSmoke -e partial-selection true
# 横向候选条的双拼部分选词复用完整候选入口，选词后禁止再滑动来触发刷新。
check "Xiaohe strip partial selection acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.CandidatePanelDeviceSmoke -e strip-partial-selection true
# 中文字母长按与上滑选择大小写/符号，复用标点套件的 instrumentation 入口。
check "Letter hold case acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.LetterHintPunctuationDeviceSmoke -e letter-hold true
# 连续触摸拖选、中文标点与手势开关/方向；复用入口，不增加 instrumentation 数量。
check "Letter swipe option acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.LetterHintPunctuationDeviceSmoke -e letter-swipe true
# 英文三排长按覆盖大小写、数字与字母上下标、声调和 O 的既有切换入口。
check "English letter hold acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.LetterHintPunctuationDeviceSmoke -e english-hold true
# 英文三排角标在两种方向上均应上屏，包含 D 的非 ASCII 符号和 #9995 改成下划线、反引号的 X、C。
check "English symbol swipe acceptance failed" MSIME_DEVICE_SMOKE_PASSED run_instrumentation app.msime.android.test/app.msime.android.test.LetterHintPunctuationDeviceSmoke -e english-swipe true
if [[ "$settings" == true ]]; then
  for suite in OptionSheetDeviceSmoke SettingsDeviceSmoke SettingsLifecycleSmoke AccountStorageDeviceSmoke CloudClipboardKeystoreDeviceSmoke CloudClipboardDriverDeviceSmoke CloudClipboardRelayDeviceSmoke; do
    check "Shared settings acceptance failed: $suite" MSIME_DEVICE_SMOKE_PASSED "$adb" -s "$serial" shell am instrument -w "app.msime.android.test/app.msime.android.test.$suite"
  done
  # 「我的词条」网盘同步的 JNI 入口和本机目录，借自建中继那一项的 instrumentation 跑（测试包的 instrumentation 已满 100 个）。
  check "Dictionary sync acceptance failed" MSIME_DEVICE_SMOKE_PASSED "$adb" -s "$serial" shell am instrument -w -e dictionary-sync true app.msime.android.test/app.msime.android.test.CloudClipboardRelayDeviceSmoke
  check "Account session binding acceptance failed" MSIME_DEVICE_SMOKE_PASSED "$adb" -s "$serial" shell am instrument -w app.msime.android.test/app.msime.android.BackendAccountSessionDeviceSmoke
fi
if [[ "$statistics" == true ]]; then
  "$adb" -s "$serial" shell am force-stop app.msime.android
  check "Typing statistics acceptance failed" MSIME_DEVICE_SMOKE_PASSED "$adb" -s "$serial" shell am instrument -w app.msime.android.test/app.msime.android.test.TypingStatisticsDeviceSmoke
fi
if [[ "$handwriting" == true ]]; then
  check_script "Handwriting acceptance failed" platforms/android/tests/device/handwriting-smoke.sh "$serial"
fi
# 生命周期回归会重启宿主进程，放在键盘套件之后，避免打断系统已经绑定的输入法服务。
check_script "Clipboard save lifecycle acceptance failed" platforms/android/tests/device/clipboard-edit-lifecycle.sh "$serial" app.msime.android
[[ ${#skipped_suites[@]} -eq 0 ]] || echo "Skipped known failures: ${skipped_suites[*]}" >&2
finish "Dedicated Android AVD: install, resource setup, system input and live preferences acceptance passed"

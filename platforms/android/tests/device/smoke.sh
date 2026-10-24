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
    *) echo "usage: smoke.sh [emulator-5580] [--core] [--settings] [--handwriting] [--statistics] [--feedback] [--diagnostics] [--text-edit-layout]" >&2; exit 1 ;;
  esac
done
[[ "$serial" == emulator-* ]] || { echo "Only the dedicated emulator is supported" >&2; exit 1; }
avd_name=$("$adb" -s "$serial" emu avd name | tr -d '\r' | head -1)
[[ "$avd_name" == msime-client-test || "$avd_name" == msime-client-test-api* ]] || { echo "Refusing a non-test AVD" >&2; exit 1; }
[[ $("$adb" -s "$serial" shell getprop sys.boot_completed | tr -d '\r') == 1 ]] || { echo "Test AVD has not booted" >&2; exit 1; }
bash platforms/android/tests/device/build-editor.sh
# 先推送完整 APK 再交给包管理器，避免 API 28 的流式安装桥卡住而阻塞整套验收。
"$adb" -s "$serial" install --no-streaming --no-incremental -r target/android/msime-android.apk
"$adb" -s "$serial" install --no-streaming --no-incremental -r target/android/editor-test.apk
"$adb" -s "$serial" install --no-streaming --no-incremental -r target/android/first-frame-editor.apk
# 同包名搜索夹具只在专用兼容验收时安装，不进入默认设备测试。
if [[ "${MSIME_ANDROID_APP_COMPOSITION:-0}" == 1 ]]; then
  "$adb" -s "$serial" install --no-streaming --no-incremental -r target/android/composition-search-editor.apk
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
  if [[ $(xmllint --xpath 'boolean(//node[contains(@text,"词库准备失败")])' "$xml") == true ]]; then echo "Device bootstrap failed" >&2; exit 1; fi
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
[[ "$ready" == true ]] || { echo "Device bootstrap timed out" >&2; exit 1; }
sdk=$("$adb" -s "$serial" shell getprop ro.build.version.sdk | tr -d '\r')
[[ "$sdk" =~ ^[0-9]+$ ]] || { echo "Android API level was unavailable" >&2; exit 1; }
# API 31 起可附着到已运行的产品进程。普通 instrument 若以产品包为目标，会连同 :ime 服务一起强停；
# 连续跑设备套件时，系统偶尔来不及重新绑定输入法。以测试 APK 为目标的用例继续正常启动 instrumentation。
run_instrumentation() {
  local component=$1 class=${1#*/} target
  target=$(xmllint --xpath "string(//instrumentation[@*[local-name()='name']='$class']/@*[local-name()='targetPackage'])" \
    platforms/android/tests/device/AndroidManifest.xml)
  [[ "$target" == app.msime.android || "$target" == app.msime.android.test ]] \
    || { echo "Unknown instrumentation target: $component" >&2; return 1; }
  if (( sdk >= 31 )) && [[ "$target" == app.msime.android ]]; then
    "$adb" -s "$serial" shell am start -W -n app.msime.android/app.msime.android.home.HomeActivity >/dev/null
    "$adb" -s "$serial" shell am instrument --no-restart "${@:2}" -w "$component"
  else
    "$adb" -s "$serial" shell am instrument "${@:2}" -w "$component"
  fi
}
# 输入法 id 必须写成 `ime list` 给出的短写：系统按字符串原样查找，长写 app.msime.android/app.msime.android.MSIMEInputService 在 `ime enable` 时报 Unknown input method。设备测试里的 `ime` 命令都用同一个短写。
"$adb" -s "$serial" shell ime enable app.msime.android/.MSIMEInputService
"$adb" -s "$serial" shell ime set app.msime.android/.MSIMEInputService
"$adb" -s "$serial" shell am force-stop app.msime.android.test
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.DeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "System input acceptance failed" >&2; exit 1; }
if [[ "$text_edit_layout" == true ]]; then
  result=$(run_instrumentation app.msime.android.test/app.msime.android.test.TextEditRecentClipboardDeviceSmoke -e layout-only true)
  printf '%s\n' "$result"
  [[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Text edit layout acceptance failed" >&2; exit 1; }
  exit 0
fi
# 十八键复用已有入口，API 28 核心验收与 API 35 完整验收都覆盖组码、回落和实际几何。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.FourteenKeyDeviceSmoke -e eighteen_key true)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "十八键全拼验收失败" >&2; exit 1; }
if [[ "$core" != true ]]; then
  result=$(run_instrumentation app.msime.android.test/app.msime.android.test.ChineseLanguageReturnDeviceSmoke -e japanese-toggle true)
  printf '%s\n' "$result"
  [[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Japanese toggle input acceptance failed" >&2; exit 1; }
fi
if [[ "${MSIME_ANDROID_APP_COMPOSITION:-0}" == 1 ]]; then
  result=$(run_instrumentation app.msime.android.test/app.msime.android.test.CompositionRestartDeviceSmoke -e app-composition true)
  printf '%s\n' "$result"
  [[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "App composition compatibility acceptance failed" >&2; exit 1; }
fi
# Compose 可在批量结束时才执行写入，核心验收覆盖延迟组字和中英往返。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.BatchCompositionDeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Deferred batch composition acceptance failed" >&2; exit 1; }
# 同一编辑器每次输入后重启连接时，手动数字层不能退回字母页；换输入框仍重置。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.NumberLayerRestartDeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Number layer restart acceptance failed" >&2; exit 1; }
# 同编辑器的组字重启必须保留候选，外部清空、改写和移动不能复活旧组字。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.CompositionRestartDeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Composition restart acceptance failed" >&2; exit 1; }
# 中文临时大写用完或取消后返回中文，快速双击仍锁定大写。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.ChineseShiftReturnDeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Chinese Shift return acceptance failed" >&2; exit 1; }
# 分词操作通过真实分享接收器、共享常用语存储和系统剪贴板验收。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.ClipboardSegmentActionsDeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Clipboard segment actions acceptance failed" >&2; exit 1; }
# 常用语结果显示在手机顶部，不占诊断行，也不移动剪贴板列表。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.ClipboardSegmentActionsDeviceSmoke -e phrase-notice true)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Clipboard phrase notice acceptance failed" >&2; exit 1; }
# 稀疏横拖直接经过安装产品的触摸分发，最低 API 也检查接手位移与子控件取消。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.PagedTileGridGestureDeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Paged grid gesture acceptance failed" >&2; exit 1; }
# 菜单复制使用真实系统剪贴板与组字会话，最低 API 的核心验收也必须覆盖。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.CopyDeviceInfoDeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Device information copy acceptance failed" >&2; exit 1; }
# 云候选描述符经过安装产品的 HTTP 栈，不读取偏好或真实输入。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.CloudCandidateTransportDeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Custom cloud transport acceptance failed" >&2; exit 1; }
# 在安装产品中区分失败来源；只用合成响应和拒写连接，不读取实际输入。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.InputFailureCategoryDeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Input failure category acceptance failed" >&2; exit 1; }
# 批量结束才应用的编辑器也要保留组字；限长回退的两批选区回声都不能取消候选。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.CompositionBatchDeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Composition batch acceptance failed" >&2; exit 1; }
# 旧菜单、展开按钮和拼写回调只检查身份，不写编辑器、不停止当前会话。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.StaleCandidateDeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Stale candidate callback acceptance failed" >&2; exit 1; }
# 空闲联想只在点选时上屏，自己的选区回声保留候选，外部移动和隐私模式清除候选。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.StaleCandidateDeviceSmoke -e next-word true)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Next word prediction acceptance failed" >&2; exit 1; }
# 横向候选条不展开也能越过分页和分批边界，左右返回后按全局身份选词。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.CandidatePanelDeviceSmoke -e continuous-strip true)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Continuous candidate strip acceptance failed" >&2; exit 1; }
# 从安装产品绘制候选标签，旧系统缺字形时也必须在深浅主题中可见。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.HeavyEqualsCandidateDeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Candidate heavy equals glyph acceptance failed" >&2; exit 1; }
# 四个新手势及肤色在旧系统缺字时仍保留可见候选。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.HandEmojiCandidateDeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Candidate hand emoji acceptance failed" >&2; exit 1; }
# 首道双拼完整候选换代后，释义与面板同步视图，连续选词不得停止输入会话。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.CandidateExpansionDeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Candidate expansion acceptance failed" >&2; exit 1; }
# 候选包读应用私有目录，最低 API 的核心验收同样覆盖配色接入与资源失效。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.CandidateSkinDeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Candidate skin package acceptance failed" >&2; exit 1; }
# Canvas 像素和动画必须在 Android 的真实解码器与绘制器中验收。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.CandidateCanvasDeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Candidate canvas acceptance failed" >&2; exit 1; }
if [[ "$feedback" == true ]]; then
  # 使用真实反馈页连续输入合成文字，不点击提交；API 28 的 --core 也能单独带上此回归。
  result=$(run_instrumentation app.msime.android.test/app.msime.android.test.FeedbackInputStabilityDeviceSmoke)
  printf '%s\n' "$result"
  [[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Feedback input stability acceptance failed" >&2; exit 1; }
fi
# 剪贴板全文导出涉及存储授权，API 28/35 核心套件都覆盖。
bash platforms/android/tests/device/clipboard-export.sh "$serial"
if [[ "$diagnostics" == true ]]; then
  # 只生成合成日志包，覆盖原生导出到上传请求的契约，不发网络请求。
  result=$(run_instrumentation app.msime.android.test/app.msime.android.test.DiagnosticsBundleDeviceSmoke)
  printf '%s\n' "$result"
  [[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Diagnostics bundle acceptance failed" >&2; exit 1; }
  result=$(run_instrumentation app.msime.android.test/app.msime.android.test.VoiceDiagnosticsPrivacyDeviceSmoke)
  printf '%s\n' "$result"
  [[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Voice diagnostics privacy acceptance failed" >&2; exit 1; }
  bash platforms/android/tests/device/diagnostics-export-directory.sh "$serial"
fi
# 三万条以上的码表也要经应用沙箱的 JNI 和真实 Engine 验证，API 28 的核心套件同样执行。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.HelpcodeImportDeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Helpcode import acceptance failed" >&2; exit 1; }
# 导入必须经过安装产品的 Java 入口和 JNI，第六项不能等到下一次收起键盘才出现。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.DictionaryImportDeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DICTIONARY_IMPORT_PASSED* ]] || { echo "Dictionary import acceptance failed" >&2; exit 1; }
# 分词拖选在最低与目标 API 的真实键盘上覆盖跨行、取消和边缘滚动。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.ClipboardSegmentDragDeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Clipboard segment drag acceptance failed" >&2; exit 1; }

# 数字默认九宫格涉及常规键面和字面提交，API 28 的核心验收同样覆盖。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.NumberPadDeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Number pad acceptance failed" >&2; exit 1; }
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.NumberPadDeviceSmoke -e password-number-pad true)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Password number pad acceptance failed" >&2; exit 1; }
# 导航必须在原生宿主中验收：设置列表有「皮肤」一行，底部第二格是没有返回按钮、顶部为「皮肤 / 词库」分类的社区根页，AI 对话从设置压入二级页。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.ChatNavigationDeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Native chat navigation acceptance failed" >&2; exit 1; }
# 工具栏编辑入口在最低 API 上也支持，核心验收覆盖真实长按和原生设置跳转。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.ToolbarEditorDeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Toolbar editor entry acceptance failed" >&2; exit 1; }
# 本地语音句柄经过真实 JNI；不加载模型或录音，最低 API 的核心验收也执行。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.LocalSpeechHandleDeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_LOCAL_SPEECH_HANDLES_PASSED* ]] || { echo "Local speech handle acceptance failed" >&2; exit 1; }
# 真实 Android JSON 的润色回复回归不录音、不联网，最低 API 的核心验收也执行。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.VoicePolishReplyDeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Voice polish reply acceptance failed" >&2; exit 1; }
# AI 地址、预设和模型选择经真实 JNI 与原生页面验证；模型目录只连接合成回环服务，最低 API 的核心验收也执行。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.AiEndpointCheckDeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "AI endpoint check acceptance failed" >&2; exit 1; }
# 语音设置目录从 Rust 经真实 JNI 读取；同一套件还验证原生模型页的两项准备状态及运行库导入入口。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.VoiceProviderCatalogDeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Shared voice catalog JNI acceptance failed" >&2; exit 1; }
# 用合成内存文档核对共享换槽和清空字段策略，不保存或发送凭据。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.VoiceProviderSettingsDeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Shared voice settings JNI acceptance failed" >&2; exit 1; }
# 语音接口提示与运行时共用校验；只传合成地址，不保存或联网。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.VoiceEndpointSettingsDeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Shared voice endpoint JNI acceptance failed" >&2; exit 1; }
# 账号与云端的 JNI 入口和注入的会话存储（#8201）经真实 JNI 与 Rust 核对；只用合成会话、不联网，最低 API 的核心验收也执行。
# 其中旧快照队列那一项（迁移计划 P13）要在主进程里真的激活一份整份快照，激活要独占词库维护权；输入法还绑着时 `:ime` 的会话占着词库，Rust 答 snapshot_unavailable。产品里激活只在键盘会话结束以后的 `:ime` 空闲时做，所以这里先停用本输入法让系统解绑它，跑完再启用、选回来，后面的套件照旧用它。
"$adb" -s "$serial" shell ime disable app.msime.android/.MSIMEInputService >/dev/null
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.SessionStoreDeviceSmoke)
"$adb" -s "$serial" shell ime enable app.msime.android/.MSIMEInputService >/dev/null
"$adb" -s "$serial" shell ime set app.msime.android/.MSIMEInputService >/dev/null
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Session store JNI acceptance failed" >&2; exit 1; }
# 原生皮肤删除覆盖真实确认框、锁与原子替换；最低 API 的核心验收也运行。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.SettingsDeviceSmoke -e skin-delete true)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Native saved skin deletion acceptance failed" >&2; exit 1; }
# 背单词内置词书：Bootstrap 解到 files/bootstrap/wordbooks，设置页经真实 JNI 读到八本书并能作答。这是应用私有目录里的解包和文件锁，最低 API 的核心验收也运行；APK 要先用 scripts/fetch_wordbooks.py 生成的词书打包。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.SettingsDeviceSmoke -e vocabulary true)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Bundled wordbook acceptance failed" >&2; exit 1; }
# 公告获取和关闭即使等待独立文件锁，实际设置内容也必须可读；复用设置 runner，覆盖最低 API。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.SettingsDeviceSmoke -e notice-queue true)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Notice queue isolation acceptance failed" >&2; exit 1; }
# 常用语备注经原生设置、真实键盘和本地备份验收；最低 API 的核心验收也执行。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.SettingsDeviceSmoke -e phrase-notes true)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Native common phrase note acceptance failed" >&2; exit 1; }
# 常用语分类覆盖原生管理、纵向展开、正文插入和本地备份；最低 API 同样执行。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.SettingsDeviceSmoke -e phrase-categories true)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Native common phrase category acceptance failed" >&2; exit 1; }
# 触控板通过独立编辑器验收真实 DPAD、选区、临时面板和密码回读，最低 API 同样覆盖。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.TextEditRecentClipboardDeviceSmoke -e trackpad true)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Text trackpad acceptance failed" >&2; exit 1; }

if [[ "$core" == true ]]; then
  bash platforms/android/tests/device/clipboard-edit-lifecycle.sh "$serial" app.msime.android
  echo "Dedicated Android AVD: install, resource setup, core typing and clipboard save lifecycle acceptance passed"
  exit 0
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
    result=$("$adb" -s "$serial" shell am instrument -w "app.msime.android.test/app.msime.android.test.$suite")
  else
    result=$(run_instrumentation "app.msime.android.test/app.msime.android.test.$suite")
  fi
  printf '%s\n' "$result"
  [[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "${entry#*|}" >&2; exit 1; }
done
# 18 键双拼复用字母组布局入口，避免超过 Android 35 的清单数量限制。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.FourteenKeyDeviceSmoke -e eighteen_shuangpin true)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "十八键双拼验收失败" >&2; exit 1; }
# 九键逐字选词复用原入口；真实触摸后必须自动刷新剩余组字，不能依靠手动返回解除旧面板。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.NineKeyPanelDeviceSmoke -e partial-selection true)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Nine-key partial selection acceptance failed" >&2; exit 1; }
# 横向候选条的双拼部分选词复用完整候选入口，选词后禁止再滑动来触发刷新。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.CandidatePanelDeviceSmoke -e strip-partial-selection true)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Xiaohe strip partial selection acceptance failed" >&2; exit 1; }
# 中文字母长按与上滑选择大小写/符号，复用标点套件的 instrumentation 入口。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.LetterHintPunctuationDeviceSmoke -e letter-hold true)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Letter hold case acceptance failed" >&2; exit 1; }
# 连续触摸拖选、中文标点与手势开关/方向；复用入口，不增加 instrumentation 数量。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.LetterHintPunctuationDeviceSmoke -e letter-swipe true)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Letter swipe option acceptance failed" >&2; exit 1; }
# 英文三排长按覆盖大小写、数字与字母上下标、声调和 O 的既有切换入口。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.LetterHintPunctuationDeviceSmoke -e english-hold true)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "English letter hold acceptance failed" >&2; exit 1; }
# 英文三排角标在两种方向上均应上屏，包含 D、X、C 的非 ASCII 符号。
result=$(run_instrumentation app.msime.android.test/app.msime.android.test.LetterHintPunctuationDeviceSmoke -e english-swipe true)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "English symbol swipe acceptance failed" >&2; exit 1; }
if [[ "$settings" == true ]]; then
  for suite in OptionSheetDeviceSmoke SettingsDeviceSmoke SettingsLifecycleSmoke AccountStorageDeviceSmoke CloudClipboardKeystoreDeviceSmoke CloudClipboardDriverDeviceSmoke CloudClipboardRelayDeviceSmoke; do
    result=$("$adb" -s "$serial" shell am instrument -w "app.msime.android.test/app.msime.android.test.$suite")
    printf '%s\n' "$result"
    [[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Shared settings acceptance failed" >&2; exit 1; }
  done
  result=$("$adb" -s "$serial" shell am instrument -w app.msime.android.test/app.msime.android.BackendAccountSessionDeviceSmoke)
  printf '%s\n' "$result"
  [[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Account session binding acceptance failed" >&2; exit 1; }
fi
if [[ "$statistics" == true ]]; then
  "$adb" -s "$serial" shell am force-stop app.msime.android
  result=$("$adb" -s "$serial" shell am instrument -w app.msime.android.test/app.msime.android.test.TypingStatisticsDeviceSmoke)
  printf '%s\n' "$result"
  [[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Typing statistics acceptance failed" >&2; exit 1; }
fi
if [[ "$handwriting" == true ]]; then
  bash platforms/android/tests/device/handwriting-smoke.sh "$serial"
fi
# 生命周期回归会重启宿主进程，放在键盘套件之后，避免打断系统已经绑定的输入法服务。
bash platforms/android/tests/device/clipboard-edit-lifecycle.sh "$serial" app.msime.android
[[ ${#skipped_suites[@]} -eq 0 ]] || echo "Skipped known failures: ${skipped_suites[*]}" >&2
echo "Dedicated Android AVD: install, resource setup, system input and live preferences acceptance passed"

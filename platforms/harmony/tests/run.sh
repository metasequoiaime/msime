#!/usr/bin/env bash
# 在 node 下运行移植过来的键盘逻辑。HarmonyOS 自己的 hypium 测试需要插桩并依赖设备；这些类不依赖 ArkUI 或 NAPI，所以可以改在这里检查。
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
repo_root=$(cd "$here/../../.." && pwd)
tsc="$repo_root/apps/desktop/node_modules/.bin/tsc"
if [[ ! -x "$tsc" ]]; then
  echo "TypeScript compiler not found at $tsc; run pnpm install at the repository root" >&2
  exit 1
fi
# grep rather than rg: the HarmonyOS CI job installs no ripgrep, and a missing rg made this guard report a regression that was not there.
if ! grep -qF 'StagedResources.removeDirectory(this.cacheRoot)' \
    "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeySoundPlayer.ets"; then
  echo "key sound reloads must remove the rendered cache tree, not only the empty root" >&2
  exit 1
fi
# Resource staging must build off to the side and swap a complete tree into place; copying into the
# live directory lets the settings and keyboard processes observe a half-staged engine.
if ! grep -qF 'StagedResources.replaceDirectory(staging, destination)' \
    "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/StagedResources.ets"; then
  echo "Harmony resource staging must replace the live directory only after the staged tree is complete" >&2
  exit 1
fi
# Starter insertion is several separately locked native operations; the returned panel snapshot
# must be reloaded after that sequence so a concurrent settings edit cannot leave it stale.
if ! grep -qF "await KeyboardSession.commonPhrasesCall(directory, { operation: 'load' })" \
    "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardSession.ets"; then
  echo "Harmony common phrases must reload after starter seeding" >&2
  exit 1
fi
# 字母键的 ForEach 按 faceKey() 判定子树是否重建。「双拼键位提示」不在这个键里时，键盘开着切换开关，已经画好的字母键会一直留着旧提示。
face_key_body=$(sed -n '/^  private faceKey(): string {$/,/^  }$/p' \
  "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets")
# 先取进变量再判断：管道交给 `grep -q` 时它命中即退出，sed 还没写完的输出会收到 SIGPIPE，pipefail 下这一步偶发失败。
if ! grep -qF 'this.shuangpinKeyHints' <<< "$face_key_body"; then
  echo "KeyboardView.faceKey must include the shuangpin key hint switch so a live change redraws the letter keys" >&2
  exit 1
fi
# 统计页认证默认关闭，认证成功后五分钟冷却。冷却规则由 keyboard-logic.test.ts 的 `StatisticsAuthenticationCooldown` 覆盖；认证本身是 ArkTS，node 下编不了，这里钉住接线：默认值是关、冷却期内直接放行、认证成功开启冷却、离开时从现在算起，时钟用含深度睡眠的开机时间。
statistics_auth="$repo_root/platforms/harmony/entry/src/main/ets/keyboard/settings/StatisticsAuthentication.ets"
if [[ $(grep -cF "getSync('enabled', false)" "$statistics_auth") -ne 2 ]] \
  || grep -qF "getSync('enabled', true)" "$statistics_auth" \
  || ! grep -qF 'if (cooldown.active()) {' "$statistics_auth" \
  || ! grep -qF 'if (success) cooldown.granted();' "$statistics_auth" \
  || ! grep -qF 'cooldown.ended();' "$statistics_auth" \
  || ! grep -qF 'systemDateTime.TimeType.STARTUP' "$statistics_auth"; then
  echo "Statistics authentication must default to off and skip re-authentication within the cooldown" >&2
  exit 1
fi
# 功能面板「手写」磁贴再点一次要回到进入手写前的入口和中英状态（#9703）。入口怎么选由 keyboard-logic.test.ts 的 `KeyboardScheme.handwritingReturn` 覆盖；视图是 ArkTS，node 下编不了，这里钉住接线：离开手写经 handwritingReturn 取入口，进入前是英文时回到英文。
toggle_body=$(sed -n '/^  private toggleHandwritingScheme(): void {$/,/^  }$/p' \
  "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets")
if ! grep -qF 'KeyboardScheme.handwritingReturn(before' <<< "$toggle_body" \
  || ! grep -qF 'this.englishBeforeHandwriting' <<< "$toggle_body"; then
  echo "Leaving handwriting from the function panel must return to the scheme and English mode it was entered from" >&2
  exit 1
fi
# 候选胶囊的释义列按候选取舍（`CandidateChipWidth.reservesColumn`），最小宽度只有触控下限（`CandidateChipWidth.touchMinimum`），规则由 keyboard-logic.test.ts 覆盖；视图是 ArkTS，这里钉住接线：手机胶囊的最小宽度按候选算，不能退回整页一律留一栏，也不能再取皮肤桌面格宽 `itemMinWidth`。
min_width_body=$(sed -n '/^  private chipMinWidth(candidate: CandidateEntry): number {$/,/^  }$/p' \
  "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets")
if ! grep -qF 'CandidateChipWidth.reservesColumn(' <<< "$min_width_body" \
  || ! grep -qF 'CandidateChipWidth.touchMinimum(' <<< "$min_width_body" \
  || grep -qF 'itemMinWidth' <<< "$min_width_body"; then
  echo "KeyboardView.chipMinWidth must ask CandidateChipWidth.reservesColumn and touchMinimum, and must not use the skin's desktop itemMinWidth" >&2
  exit 1
fi
# AI 生成类功能的总闸（`AiGenerationGate`，取自 Rust 的 `HostCapabilities.ai_generation`）：规则由 ai-generation-gate.test.ts 覆盖；键盘、语音润色和设置桥是 ArkTS，node 下编不了，这里钉住接线：两个进程启动时都读宿主能力，功能面板按它去掉 AI 格子，AI 联想、高情商回复、润色和设置桥的 AI 请求在发出前都问它。
ets="$repo_root/platforms/harmony/entry/src/main/ets"
for wiring in \
  "keyboard/KeyboardSession.ets:AiGenerationGate.adopt(client.hostCapabilities('harmony'))" \
  "keyboard/KeyboardSession.ets:aiQuery.ai_eligible && AiGenerationGate.enabled()" \
  "keyboard/KeyboardSession.ets:request === null || !AiGenerationGate.enabled()" \
  "keyboard/KeyboardView.ets:FunctionPanelPolicy.items(AiGenerationGate.enabled())" \
  "keyboard/input/HarmonyVoicePolisher.ets:if (!AiGenerationGate.enabled()) return null;" \
  "pages/Settings.ets:AiGenerationGate.adopt(client.hostCapabilities(PLATFORM))" \
  "pages/Settings.ets:if (!AiGenerationGate.allowsSettingsRequest(kind, payload)) {"; do
  if ! grep -qF "${wiring#*:}" "$ets/${wiring%%:*}"; then
    echo "${wiring%%:*} must consult AiGenerationGate: missing \"${wiring#*:}\"" >&2
    exit 1
  fi
done
if grep -qF 'FunctionPanelPolicy.items()' "$ets/keyboard/KeyboardView.ets"; then
  echo "KeyboardView must list function tiles through FunctionPanelPolicy.items(AiGenerationGate.enabled())" >&2
  exit 1
fi
# 词库快照队列只在键盘显示时查、没有待处理请求就停，重建会话的重试按 `IdleDictionaryRetryPolicy` 退避（规则由 keyboard-logic.test.ts 覆盖）。会话是 ArkTS，这里钉住接线：失焦和系统收起键盘（keyboardHide）时取消检查、停下没有进展的个人词库重试，显示时排一次检查并接着排个人词库，键盘没显示时不排、查到没有请求时不再自己续排，两个重试都按退避取间隔而不是固定两秒。
session_ets="$ets/keyboard/KeyboardSession.ets"
detach_body=$(sed -n '/^  detach(): void {$/,/^  }$/p' "$session_ets")
attach_body=$(sed -n '/^  attach(inputClient: inputMethodEngine.InputClient): void {$/,/^  }$/p' "$session_ets")
poll_body=$(sed -n '/^  private scheduleDictionarySnapshotPoll(): void {$/,/^  }$/p' "$session_ets")
drain_body=$(sed -n '/^  private schedulePersonalDictionaryDrain(): void {$/,/^  }$/p' "$session_ets")
hidden_body=$(sed -n '/^  keyboardHidden(): void {$/,/^  }$/p' "$session_ets")
pause_body=$(sed -n '/^  private pauseIdleDictionaryWork(): void {$/,/^  }$/p' "$session_ets")
if ! grep -qF 'this.pauseIdleDictionaryWork();' <<< "$detach_body" \
  || ! grep -qF 'this.pauseIdleDictionaryWork();' <<< "$hidden_body" \
  || ! grep -qF 'this.cancelDictionarySnapshotPoll();' <<< "$pause_body" \
  || ! grep -qF 'this.personalDictionaryStalls > 0' <<< "$pause_body" \
  || ! grep -qF 'clearTimeout(this.personalDictionaryTimer);' <<< "$pause_body" \
  || ! grep -qF 'this.scheduleDictionarySnapshotPoll();' <<< "$attach_body" \
  || ! grep -qF 'this.schedulePersonalDictionaryDrain();' <<< "$attach_body" \
  || ! grep -qF 'this.inputClient === undefined) return;' <<< "$poll_body" \
  || [[ $(grep -cF 'this.scheduleDictionarySnapshotPoll();' <<< "$poll_body") -ne 1 ]] \
  || ! grep -qF 'IdleDictionaryRetryPolicy.delay(this.snapshotQueueStalls)' <<< "$poll_body" \
  || ! grep -qF 'IdleDictionaryRetryPolicy.delay(this.personalDictionaryStalls)' <<< "$drain_body"; then
  echo "KeyboardSession must poll the dictionary snapshot queue only while shown and until it is empty, and back off both idle rebuild retries" >&2
  exit 1
fi
# 打字统计的提交和语音时长不在 UI 线程上写：都排进同一个 SequenceRunner（`writeTypingStatistics`）在 TaskPool 线程上调用原生库，原生层攒着的计数在键盘收起（dismiss 与系统的 keyboardHide）、编辑框失去焦点（detach）和扩展销毁（shutdown）时 flush；detach 等 flush 写完再把统计文件发布给设置应用。会话是 ArkTS，这里钉住接线。
record_body=$(sed -n '/^  private recordTypingStatistics(text: string, source: string): void {$/,/^  }$/p' "$session_ets")
voice_body=$(sed -n '/^  insertVoiceResult(text: string): void {$/,/^  }$/p' "$session_ets")
dismiss_body=$(sed -n '/^  dismiss(): void {$/,/^  }$/p' "$session_ets")
shutdown_body=$(sed -n '/^  shutdown(): void {$/,/^  }$/p' "$session_ets")
ability_ets="$ets/inputmethodextability/KeyboardExtensionAbility.ets"
if ! grep -qF 'this.typingStatisticsWrites.execute(new taskpool.Task(writeTypingStatistics, request))' <<< "$record_body" \
  || grep -qF 'client.typingStatistics(' <<< "$record_body" \
  || ! grep -qF 'this.typingStatisticsWrites.execute(new taskpool.Task(writeTypingStatistics, request))' <<< "$voice_body" \
  || grep -qF 'client.typingStatistics(' <<< "$voice_body" \
  || ! grep -qF 'this.flushTypingStatistics().then((): void => {' <<< "$detach_body" \
  || ! grep -qF 'SharedStateSync.keyboardPublish(mirror);' <<< "$detach_body" \
  || ! grep -qF 'this.flushTypingStatistics();' <<< "$dismiss_body" \
  || ! grep -qF 'this.flushTypingStatistics();' <<< "$shutdown_body" \
  || ! grep -qF "engine.on('keyboardHide', this.keyboardHidden);" "$ability_ets" \
  || ! grep -qF "engine.off('keyboardHide', this.keyboardHidden);" "$ability_ets"; then
  echo "KeyboardSession must write typing statistics on its TaskPool SequenceRunner and flush them on hide, focus loss and teardown" >&2
  exit 1
fi
# 完整体验模式下键盘失焦、获得焦点各发布一次共享状态，文件身份没变时不读文件、不再算指纹（缓存规则由 shared-state-mirror-policy.test.ts 的 `FingerprintCache` 覆盖）；每次获得焦点比偏好修订号时，文档身份没变就不经 NAPI 读整份文档；构建会话时处理词库快照队列、显示后查一次队列状态，队列空着都不扫词库算本机版本。这些都是 ArkTS，这里钉住接线。
mirror_ets="$ets/keyboard/SharedStateMirror.ets"
fingerprint_body=$(sed -n '/^  private static fingerprintOf(entry: MirrorEntry, path: string): string {$/,/^  }$/p' "$mirror_ets")
reload_body=$(sed -n '/^  reloadChangedPreferences(): void {$/,/^  }$/p' "$session_ets")
snapshot_body=$(sed -n '/^  private processDictionarySnapshot(options: Object, stateRoot: string): void {$/,/^  }$/p' "$session_ets")
needs_work_body=$(sed -n '/^  private dictionarySnapshotNeedsWork(): boolean {$/,/^  }$/p' "$session_ets")
if ! grep -qF 'SharedStateMirror.localFingerprints.fingerprint(' <<< "$fingerprint_body" \
  || ! grep -qF 'stat.mtimeNs' <<< "$fingerprint_body" \
  || ! grep -qF 'this.storedRevisionIfMoved(this.stateDirectory)' <<< "$reload_body" \
  || grep -qF 'this.storedRevision(' <<< "$reload_body" \
  || ! grep -qF 'skip_when_idle: true' <<< "$snapshot_body" \
  || ! grep -qF 'skip_when_idle: true' <<< "$needs_work_body"; then
  echo "Shared state fingerprints, the focus-time preference revision check and the snapshot queue pass and poll must skip work when nothing changed" >&2
  exit 1
fi
# 候选皮肤位图在候选条放手后停放一份，同一套皮肤不再每个词重新解码；停放规则由 candidate-canvas-lease.test.ts 覆盖。解码器和扩展能力是 ArkTS，这里钉住接线：解码器经 CandidateCanvasLeaseCache 取用和放手，键盘收起、停止输入和销毁时 trim。
frame_ets="$ets/keyboard/candidate/CandidateCanvasFrame.ets"
hidden_body=$(sed -n '/^  private readonly keyboardHidden: () => void = (): void => {$/,/^  };$/p' "$ability_ets")
stopped_body=$(sed -n '/^  private readonly stopped: () => void = (): void => {$/,/^  };$/p' "$ability_ets")
teardown_body=$(sed -n '/^  private teardown(): void {$/,/^  }$/p' "$ability_ets")
if ! grep -qF 'new CandidateCanvasLeaseCache<CanvasDecodeLease>(' "$frame_ets" \
  || grep -qF 'Map<string, CanvasDecodeLease>' "$frame_ets" \
  || ! grep -qF 'CandidateCanvasDecoder.trim();' <<< "$hidden_body" \
  || ! grep -qF 'CandidateCanvasDecoder.trim();' <<< "$stopped_body" \
  || ! grep -qF 'CandidateCanvasDecoder.trim();' <<< "$teardown_body"; then
  echo "Candidate canvas decodes must park in CandidateCanvasLeaseCache and be trimmed on keyboard hide, input stop and teardown" >&2
  exit 1
fi
# 键盘收起时停掉键盘内的语音识别（取舍规则由 kb-view.test.ts 的 `VoiceRecognitionPolicy.cancelsOnHide` 覆盖）。面板收起时视图不会销毁，会话和视图是 ArkTS，这里钉住接线：会话在系统的 keyboardHide 和 inputStop（detach）时都通知视图，视图按 cancelsOnHide 取消录音、取消还在走的空格长按计时，销毁时清掉回调。
view_ets="$ets/keyboard/KeyboardView.ets"
session_hidden_body=$(sed -n '/^  keyboardHidden(): void {$/,/^  }$/p' "$session_ets")
detach_body=$(sed -n '/^  detach(): void {$/,/^  }$/p' "$session_ets")
panel_hidden_body=$(sed -n '/^    KeyboardSession.shared.onPanelHidden = (): void => {$/,/^    };$/p' "$view_ets")
if ! grep -qF 'this.onPanelHidden();' <<< "$session_hidden_body" \
  || ! grep -qF 'this.onPanelHidden();' <<< "$detach_body" \
  || ! grep -qF 'VoiceRecognitionPolicy.cancelsOnHide(' <<< "$panel_hidden_body" \
  || ! grep -qF 'this.cancelVoice();' <<< "$panel_hidden_body" \
  || ! grep -qF 'this.cancelSpaceLongPress();' <<< "$panel_hidden_body" \
  || ! grep -qF 'KeyboardSession.shared.onPanelHidden = undefined;' "$view_ets"; then
  echo "Hiding the keyboard must cancel in-keyboard voice recognition on touch devices" >&2
  exit 1
fi
# 书写中每个移动事件只补画最后一段，不清屏重画整个字（墨色是否可补画由 keyboard-logic.test.ts 的 `HandwritingStrokePolicy.opaqueInk` 覆盖）。视图是 ArkTS，这里钉住接线：移动事件走 drawHandwritingSegment，补画不清屏、半透明墨色退回整幅重画，抬笔仍整幅重画让定稿画面与逐点重画一致。
touch_body=$(sed -n '/^  private handwritingTouch(event: TouchEvent): void {$/,/^  }$/p' "$view_ets")
segment_body=$(sed -n '/^  private drawHandwritingSegment(): void {$/,/^  }$/p' "$view_ets")
move_branch=$(sed -n '/^    if (event.type === TouchType.Move) {$/,/^    }$/p' <<< "$touch_body")
up_branch=$(sed -n '/^    if (event.type === TouchType.Up) {$/,/^    }$/p' <<< "$touch_body")
if ! grep -qF 'this.drawHandwritingSegment();' <<< "$move_branch" \
  || grep -qF 'this.redrawHandwriting();' <<< "$move_branch" \
  || ! grep -qF 'HandwritingStrokePolicy.opaqueInk(' <<< "$segment_body" \
  || grep -qF 'clearRect' <<< "$segment_body" \
  || ! grep -qF 'this.redrawHandwriting();' <<< "$up_branch"; then
  echo "Handwriting moves must draw only the new segment, and lifting the pen must redraw the whole pad" >&2
  exit 1
fi
"$tsc" --project "$here/tsconfig.json"
# 上面已编译所有 tests/*.test.ts；逐个运行它们的输出。遍历源文件而不是输出目录，可以避免已删除测试遗留的旧 .js 被运行。
for source in "$here"/*.test.ts; do
  node "$repo_root/target/harmony-tests/tests/$(basename "${source%.ts}").js"
done
# The key-sound renderer is plain C++ over miniaudio with no NAPI in it, so the build machine's compiler checks what it writes and what it refuses.
out="$repo_root/target/harmony-tests"
miniaudio="$repo_root/platforms/windows/third_party/miniaudio"
"${CXX:-c++}" -std=c++17 -O1 -c "$here/../native/miniaudio.cpp" -I"$miniaudio" -o "$out/miniaudio.o"
"${CXX:-c++}" -std=c++17 -O1 -Wall -Wextra -Werror -I"$here/../native" -I"$miniaudio" \
  "$here/key-sound-render.test.cpp" "$here/../native/key_sound_render.cpp" "$out/miniaudio.o" \
  -lm -lpthread -o "$out/key-sound-render"
scratch="$out/key-sound-render.d"
rm -rf "$scratch"
mkdir -p "$scratch"
"$out/key-sound-render" "$scratch"
rm -rf "$scratch"

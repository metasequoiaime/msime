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
# 按键振动的 `startVibration` 在 NAPI 层同步发 IPC，放在 UI 线程上开了振动打字就跟不上手；这里钉住接线：振动经 TaskPool 以高优先级发出，上一下还没发完时不再排队。
if ! grep -qF 'new taskpool.Task(startKeyVibration,' "$ets/keyboard/KeyboardSession.ets" \
  || ! grep -qF 'taskpool.Priority.HIGH);' "$ets/keyboard/KeyboardSession.ets" \
  || ! grep -qF 'this.vibrationPending) {' "$ets/keyboard/KeyboardSession.ets" \
  || ! grep -qF '@Concurrent' "$ets/keyboard/KeyVibration.ets"; then
  echo "key vibration must go through a high-priority taskpool.Task(startKeyVibration, ...) and skip while one is pending" >&2
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

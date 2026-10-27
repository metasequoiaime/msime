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
# 底行样式和数字行同理：设置页改了之后键盘开着也要按新排法重建键，所以这四个值都要在 faceKey() 里。
for field in this.twentySixKeyStyle this.fourteenKeyStyle this.nineKeyStyle this.numberRow; do
  if ! grep -qF "$field" <<< "$face_key_body"; then
    echo "KeyboardView.faceKey must include $field so a live layout style change redraws the keys" >&2
    exit 1
  fi
done
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
# 数字行的数字在组字中先交给引擎，收没收只能看引擎的 handled：韩语、越南语、藏文的引擎会先上屏组字再把数字交回来，`press()` 按组字变了算作收下，数字就丢了。视图和会话都是 ArkTS，这里钉住接线：数字行走 `pressElseCommitThenType`，它按 apply() 的 handled 决定，不收时 `commitThenType`。
digit_body=$(sed -n '/^  private typeNumberRowDigit(digit: string): void {$/,/^  }$/p' \
  "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets")
press_type_body=$(sed -n '/^  pressElseCommitThenType(ascii: number): void {$/,/^  }$/p' \
  "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardSession.ets")
if ! grep -qF 'KeyboardSession.shared.pressElseCommitThenType(' <<< "$digit_body" \
  || grep -qF 'KeyboardSession.shared.press(' <<< "$digit_body" \
  || ! grep -qF 'if (this.apply(client.character(this.handle, ascii, false))) return;' <<< "$press_type_body" \
  || ! grep -qF 'this.commitThenType(ascii);' <<< "$press_type_body"; then
  echo "A number-row digit typed while composing must go through KeyboardSession.pressElseCommitThenType, which decides on the engine's handled flag and types the digit after the commit when the engine declines it" >&2
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

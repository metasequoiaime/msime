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
if ! sed -n '/^  private faceKey(): string {$/,/^  }$/p' \
    "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets" \
    | grep -qF 'this.shuangpinKeyHints'; then
  echo "KeyboardView.faceKey must include the shuangpin key hint switch so a live change redraws the letter keys" >&2
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

#!/usr/bin/env bash
# 在独立的 CI Mac 上安装当前产物，通过系统注册的 IMK 输入源写入原生编辑框。
set -euo pipefail
[[ "${GITHUB_ACTIONS:-}" == true && "$(uname -s)" == Darwin ]] || {
  echo '此入口仅用于独立的 macOS GitHub Actions runner。' >&2; exit 2;
}
root="$(cd "$(dirname "$0")/../../../.." && pwd)"
build="${1:?需要已构建目录}"
build="$(cd "$build" && pwd)"
reports="${2:?需要报告目录}"
mkdir -p "$reports"
reports="$(cd "$reports" && pwd)"
installed="$HOME/Library/Input Methods/MSIME-Korean-CI.app"
state="$HOME/Library/Application Support/app.msime.macos"
options="$state/runtime-options.json"
[[ ! -e "$installed" && ! -e "$options" ]] || {
  echo 'runner 中已有目标输入法或运行时配置，拒绝覆盖。' >&2; exit 2;
}
server_pid=''
cleanup() {
  if [[ -n "$server_pid" ]]; then kill "$server_pid" 2>/dev/null || true; wait "$server_pid" 2>/dev/null || true; fi
  rm -f "$options"
  # 目标为上面验证不存在、随后由本入口创建的固定 CI bundle。
  if [[ -d "$installed" ]]; then rm -rf "$installed"; fi
}
trap cleanup EXIT
mkdir -p "$(dirname "$installed")" "$state"
ditto "$build/水杉输入法.app" "$installed"
# 临时开发 bundle 使用 ad-hoc 签名；没有 Developer ID 团队身份可供 Hardened Runtime 校验动态库。
codesign --force --deep --entitlements "$root/platforms/macos/resources/VoiceInput.entitlements" --sign - "$installed"
codesign --verify --deep --strict "$installed"
codesign -dv --verbose=4 "$installed" > "$reports/signature.log" 2>&1
registrar='/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister'
"$registrar" -f "$installed"
"$registrar" -u "$build/水杉输入法.app" || true
executable="$installed/Contents/MacOS/$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$installed/Contents/Info.plist")"
if ! "$executable" --register-input-source > "$reports/registration.log" 2>&1; then
  cat "$reports/registration.log" >&2
  printf '%s\n' '{"passed":false,"system_input_executed":false,"blocked_by":"input source registration failed"}' > "$reports/registration.json"
  exit 1
fi
"$build/korean-system-input-acceptance" --enable-ci-source "$installed" > "$reports/enabled-sources.json"
if [[ "$(uname -m)" == x86_64 ]]; then export RB_USE_BINARY_ARCHIVE=0 RB_DISABLE_GPU=1; fi
for layout in dubeolsik romaja; do
  python3 - "$options" "$reports/profile-$layout" "$layout" <<'PY'
import json, pathlib, sys
options_path, root, layout = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2]), sys.argv[3]
preferences = {
    "scheme": "korean", "default_ime_mode": "chinese", "candidate_page_size": 5,
    "chinese_punctuation": True, "learning": False, "cloud_candidates": False,
    "candidate_translations": False, "floating_toolbar": {"enabled": False},
    "tsf_preedit_style": "empty",
}
# 第一轮确实省略布局字段，验证已有用户的缺省两套式。
if layout == "romaja":
    preferences["korean"] = {"layout": layout}
options = {"api_version": 1, "preferences": preferences}
for name in ("resources", "user_data", "cache", "dictionaries"):
    directory = root / name
    directory.mkdir(parents=True, exist_ok=True)
    options[name] = str(directory)
options_path.write_text(json.dumps(options, ensure_ascii=False), encoding="utf-8")
PY
  "$executable" > "$reports/server-$layout.log" 2>&1 &
  server_pid=$!
  sleep 2
  if ! kill -0 "$server_pid" 2>/dev/null; then
    cat "$reports/server-$layout.log" >&2
    echo '已安装输入法启动后立即退出，未执行系统输入。' >&2
    exit 1
  fi
  python3 - "$build/korean-system-input-acceptance" "$layout" "$installed" "$reports/$layout.json" "$reports/enabled-sources.json" <<'PY'
import json, pathlib, subprocess, sys
source = json.loads(pathlib.Path(sys.argv[-1]).read_text())
flags = ["--appkit-events"]
if not source["korean_mode_enabled"]:
    flags.append("--primary-source")
subprocess.run([*sys.argv[1:-1], *flags], check=True, timeout=90)
PY
  kill "$server_pid" 2>/dev/null || true
  wait "$server_pid" 2>/dev/null || true
  server_pid=''
done

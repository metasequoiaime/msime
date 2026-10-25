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
python3 - "$reports/login-session.json" <<'PY'
import json, os, pathlib, subprocess, sys
def output(*args):
    result = subprocess.run(args, text=True, capture_output=True)
    return result.stdout.strip() if result.returncode == 0 else None
parents, pid = [], os.getpid()
for _ in range(10):
    if pid <= 1:
        break
    parents.append({"pid": pid, "uid": output("ps", "-p", str(pid), "-o", "uid="),
                    "name": output("ps", "-p", str(pid), "-o", "comm=")})
    parent = output("ps", "-p", str(pid), "-o", "ppid=")
    if not parent:
        break
    pid = int(parent)
pathlib.Path(sys.argv[1]).write_text(json.dumps({"uid": os.getuid(), "manager": output("launchctl", "managername"),
    "console_uid": output("stat", "-f", "%u", "/dev/console"),
    "automatic_login_user": output("defaults", "read", "/Library/Preferences/com.apple.loginwindow", "autoLoginUser"),
    "parents": parents}, indent=2))
PY
client_bundle="$build/korean-system-input-acceptance.app"
client="$client_bundle/Contents/MacOS/korean-system-input-acceptance"
codesign --force --sign - "$client_bundle"
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
"$registrar" -u "$build/水杉输入法.app" || true
# 同一 identifier 的构建记录必须先移除，再登记最终安装路径。
"$registrar" -f "$installed"
executable="$installed/Contents/MacOS/$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$installed/Contents/Info.plist")"
if ! "$executable" --register-input-source > "$reports/registration.log" 2>&1; then
  cat "$reports/registration.log" >&2
  printf '%s\n' '{"passed":false,"system_input_executed":false,"blocked_by":"input source registration failed"}' > "$reports/registration.json"
  exit 1
fi
if ! "$client" --enable-ci-source "$installed" > "$reports/enabled-sources.json"; then
  cat "$reports/enabled-sources.json" >&2
  # 系统拒绝 API 添加时，走用户实际使用的系统设置；由镜像已有的 XCTest 自动化会话执行。
  # 不改输入源数据库，也不配置或绕过隐私权限；实际启用状态仍交给 TIS 复核。
  # 与生产安装器相同，设置扩展关闭时丢弃其旧模式清单缓存，打开后由系统重新生成。
  if ! pgrep -x 'System Settings' >/dev/null; then
    keyboard_cache="$(getconf DARWIN_USER_CACHE_DIR)com.apple.Keyboard-Settings.extension"
    rm -f "$keyboard_cache"/com.apple.IntlDataCache.le*
  fi
  if ! xcodebuild test -project "$root/platforms/macos/tests/input/SourceSetup/SourceSetup.xcodeproj" \
      -scheme SourceSetup -destination "platform=macOS,arch=$(uname -m)" \
      -derivedDataPath "$build/input-source-setup" -resultBundlePath "$reports/input-source-setup.xcresult" \
      -test-timeouts-enabled YES -default-test-execution-time-allowance 60 -maximum-test-execution-time-allowance 90 \
      -parallel-testing-enabled NO CODE_SIGN_IDENTITY=- > "$reports/input-source-setup.log" 2>&1; then
    cat "$reports/input-source-setup.log" >&2
    exit 1
  fi
  "$client" --enable-ci-source "$installed" > "$reports/enabled-after-settings.json" || {
    cat "$reports/enabled-after-settings.json" >&2; exit 1;
  }
fi
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
  python3 - "$client" "$layout" "$installed" "$reports/$layout.json" <<'PY'
import subprocess, sys
subprocess.run(sys.argv[1:], check=True, timeout=90)
PY
  kill "$server_pid" 2>/dev/null || true
  wait "$server_pid" 2>/dev/null || true
  server_pid=''
done

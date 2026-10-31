#!/usr/bin/env python3
"""Android 设备验收 smoke.sh 的失败收集：一项失败后接着跑互不依赖的检查，最后列出全部失败并以非零退出；只在后面的检查已经没有意义时当场停下。

用假的 adb、辅助脚本和 sleep 在临时目录里跑真实的 smoke.sh，不需要模拟器。缺 bash 或 xmllint（smoke.sh 用它读清单）时跳过。
"""

from pathlib import Path
import os
import re
import shutil
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parent.parent
DEVICE = ROOT / "platforms/android/tests/device"
SERIAL = "emulator-5580"
IME = "app.msime.android/.MSIMEInputService"

# 假 adb：按 instrumentation 的类名（带第一个 -e 参数时写成 类名:参数）决定通过、失败、崩溃或打印通过后非零退出，并把调用记进 calls。
FAKE_ADB = r"""#!/usr/bin/env bash
state=$FAKE_STATE
echo "adb $*" >> "$state/calls"
if [[ -e "$state/dead" ]]; then echo "error: device '$2' not found" >&2; exit 1; fi
shift 2
has() { [[ " $1 " == *" $2 "* ]]; }
case "$*" in
  "emu avd name") printf 'msime-client-test-api%s\r\nOK\r\n' "$FAKE_SDK"; exit 0 ;;
  "shell getprop sys.boot_completed") echo 1; exit 0 ;;
  "shell getprop ro.build.version.sdk") echo "$FAKE_SDK"; exit 0 ;;
  "shell settings get secure default_input_method")
    if [[ -e "$state/ime-stuck" ]]; then echo com.android.inputmethod.latin/.LatinIME; else echo "$FAKE_IME"; fi
    exit 0 ;;
  "shell locksettings verify") echo "Lock credential verified successfully"; exit 0 ;;
  install*) echo Success; exit 0 ;;
  pull*)
    printf '%s\n' '<hierarchy><node text="试用键盘" resource-id="" bounds="[0,0][10,10]"/></hierarchy>' > "$3"
    exit 0 ;;
esac
if [[ "$1 $2 $3" == "shell am instrument" ]]; then
  component=${!#}
  key=${component##*.}
  args=("$@")
  for ((i = 0; i < ${#args[@]}; i++)); do
    if [[ "${args[i]}" == -e ]]; then key="$key:${args[i + 1]}"; break; fi
  done
  echo "instrument $key" >> "$state/calls"
  if has "${FAKE_CRASH:-}" "$key"; then
    printf 'INSTRUMENTATION_RESULT: shortMsg=Process crashed.\nINSTRUMENTATION_CODE: 0\n'
  elif has "${FAKE_FAIL:-}" "$key"; then
    printf 'MSIME_DEVICE_SMOKE_FAILED: fake stage of %s (AssertionError) Expected synthetic UI state was not observed\nIME showed [a, b]\n\nINSTRUMENTATION_CODE: 0\n' "$key"
  else
    printf 'MSIME_DEVICE_SMOKE_PASSED: %s MSIME_DICTIONARY_IMPORT_PASSED MSIME_LOCAL_SPEECH_HANDLES_PASSED\nINSTRUMENTATION_CODE: -1\n' "$key"
  fi
  if has "${FAKE_DIE_AFTER:-}" "$key"; then touch "$state/dead"; fi
  if has "${FAKE_IME_STUCK_AFTER:-}" "$key"; then touch "$state/ime-stuck"; fi
  if has "${FAKE_NONZERO:-}" "$key"; then exit 7; fi
  exit 0
fi
exit 0
"""

# 假辅助脚本：名字在 FAKE_FAIL_SCRIPTS 里就像真脚本那样打印套件失败行和说明后退出 1。
FAKE_SCRIPT = r"""#!/usr/bin/env bash
name=$(basename "$0" .sh)
echo "script $name $*" >> "$FAKE_STATE/calls"
if [[ " ${FAKE_FAIL_SCRIPTS:-} " == *" $name "* ]]; then
  echo "MSIME_DEVICE_SMOKE_FAILED: $name phase (AssertionError) fake"
  echo "$name acceptance failed" >&2
  exit 1
fi
echo "$name passed"
"""

SCRIPTS = (
    "build-editor.sh",
    "clipboard-export.sh",
    "clipboard-edit-lifecycle.sh",
    "diagnostics-export-directory.sh",
    "handwriting-smoke.sh",
    "voice-lifecycle.sh",
)


def manifest() -> str:
    """真实的设备测试清单，补上 smoke.sh 引用了而清单里没有的入口。

    run_instrumentation 按清单判定目标包，引用了未声明的入口时它返回 1，那一项照常记为失败。这里只检查失败怎么收集，清单完不完整不归这个测试管，所以补齐，免得「全部通过」的场景被那一项带偏。
    """
    text = (DEVICE / "AndroidManifest.xml").read_text(encoding="utf-8")
    smoke = (DEVICE / "smoke.sh").read_text(encoding="utf-8")
    referenced = set(re.findall(r"app\.msime\.android\.test\.(\w+)", smoke))
    referenced |= set(re.findall(r'^ +"(\w+)\|', smoke, re.M))
    missing = sorted(name for name in referenced if f'android:name="app.msime.android.test.{name}"' not in text)
    extra = "".join(
        f'    <instrumentation android:name="app.msime.android.test.{name}" android:targetPackage="app.msime.android" />\n'
        for name in missing
    )
    return text.replace("</manifest>", extra + "</manifest>")


def write(path: Path, text: str, executable: bool = False) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")
    if executable:
        path.chmod(0o755)


class Run:
    def __init__(self, base: Path, name: str, flags: list[str], sdk: str = "35", **fake: str) -> None:
        root = base / name
        device = root / "platforms/android/tests/device"
        device.mkdir(parents=True)
        shutil.copy(DEVICE / "smoke.sh", device / "smoke.sh")
        write(device / "AndroidManifest.xml", manifest())
        for script in SCRIPTS:
            write(device / script, FAKE_SCRIPT, executable=True)
        write(root / "sdk/platform-tools/adb", FAKE_ADB, executable=True)
        write(root / "bin/sleep", "#!/bin/sh\nexit 0\n", executable=True)
        state = root / "state"
        state.mkdir()
        summary = root / "step-summary.md"
        env = {key: value for key, value in os.environ.items() if not key.startswith(("FAKE_", "MSIME_", "GITHUB_"))}
        env.update({
            "ANDROID_SDK_ROOT": str(root / "sdk"),
            "PATH": f"{root / 'bin'}{os.pathsep}{env.get('PATH', '')}",
            "FAKE_STATE": str(state),
            "FAKE_SDK": sdk,
            "FAKE_IME": IME,
            "GITHUB_STEP_SUMMARY": str(summary),
        })
        env.update({f"FAKE_{key.upper()}": value for key, value in fake.items()})
        completed = subprocess.run(
            ["bash", str(device / "smoke.sh"), SERIAL, *flags],
            env=env, stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=120,
        )
        self.name = name
        self.code = completed.returncode
        self.stdout = completed.stdout
        self.stderr = completed.stderr
        self.calls = (state / "calls").read_text(encoding="utf-8").splitlines()
        self.summary = summary.read_text(encoding="utf-8") if summary.exists() else ""

    def ran(self, key: str) -> bool:
        return f"instrument {key}" in self.calls

    def ran_script(self, name: str) -> bool:
        return any(line.startswith(f"script {name} ") for line in self.calls)

    def failed_lines(self) -> list[str]:
        return [line for line in self.stderr.splitlines() if line.startswith("FAILED ")]

    def stopped_line(self) -> str:
        lines = [line for line in self.stderr.splitlines() if line.startswith("STOPPED ")]
        return lines[0] if lines else ""

    def expect(self, condition: bool, message: str) -> None:
        if not condition:
            print(f"--- {self.name}: exit {self.code}\n--- stdout tail\n{self.stdout[-3000:]}\n--- stderr\n{self.stderr[-3000:]}", file=sys.stderr)
            raise AssertionError(f"{self.name}: {message}")


def main() -> None:
    if shutil.which("bash") is None or shutil.which("xmllint") is None:
        print("Android device smoke failure collection: skipped (needs bash and xmllint)")
        return
    with tempfile.TemporaryDirectory(prefix="msime-smoke-report-") as temp:
        base = Path(temp)

        run = Run(base, "full-pass", [])
        run.expect(run.code == 0, "全部通过时应以 0 退出")
        run.expect(run.stdout.rstrip().endswith("Dedicated Android AVD: install, resource setup, system input and live preferences acceptance passed"), "通过时最后一行仍是原来的通过说明")
        run.expect("====" not in run.stderr and run.summary == "", "全部通过时不打印失败汇总，也不写 step summary")
        run.expect(run.ran("LetterHintPunctuationDeviceSmoke:english-swipe") and run.ran_script("clipboard-edit-lifecycle"), "全部通过时应跑到最后一项")
        run.expect(f"script voice-lifecycle {SERIAL} hold" in run.calls, "完整验收应调用长按语音锁定专项")

        run = Run(base, "voice-hold-failure", [], fail_scripts="voice-lifecycle")
        run.expect(run.code == 1 and run.stopped_line() == "", "长按语音专项失败应汇总并继续独立检查")
        run.expect(run.failed_lines() == ["FAILED 长按语音锁定验收失败: MSIME_DEVICE_SMOKE_FAILED: voice-lifecycle phase (AssertionError) fake"], "长按语音专项失败应保留原始失败行")
        run.expect(run.ran_script("clipboard-edit-lifecycle"), "长按语音专项失败不能吞掉后续验收")

        run = Run(base, "gate-nonzero-with-marker", ["--core"], sdk="28", nonzero="DeviceSmoke")
        run.expect(run.code == 1 and "system input path failed" in run.stopped_line(), "主路径即使打印通过标记，命令非零退出也应停下")
        run.expect(not run.ran("FourteenKeyDeviceSmoke:eighteen_key"), "主路径命令失败后不应继续跑")
        run.expect(run.failed_lines() == ["FAILED System input acceptance failed: error: command exited with status 7"], "主路径失败汇总应保留命令退出码")

        nonzero = ["CandidateCanvasDeviceSmoke", "ClipboardSegmentDragDeviceSmoke", "SessionStoreDeviceSmoke", "WubiLookupDeviceSmoke"]
        run = Run(base, "nonzero-with-marker", [], nonzero=" ".join(nonzero))
        run.expect(run.code == 1 and run.stopped_line() == "", "普通检查的命令非零退出应记为失败并继续后续独立检查")
        run.expect("acceptance passed" not in run.stdout, "命令失败时不应打印整套通过说明")
        run.expect(run.ran_script("clipboard-edit-lifecycle"), "命令失败后应跑到最后一项")
        expected_nonzero = [
            f"FAILED {label}: error: command exited with status 7"
            for label in ["Candidate canvas acceptance failed", "Clipboard segment drag acceptance failed", "Session store JNI acceptance failed", "Wubi lookup acceptance failed"]
        ]
        run.expect(run.failed_lines() == expected_nonzero, "包装调用、直接 adb 和先清理再裁决的路径都必须保留退出码")
        for line in expected_nonzero:
            run.expect(f"- `{line[len('FAILED '):]}`" in run.summary, "命令失败应进入 step summary")

        failing = ["CandidateCanvasDeviceSmoke", "EmojiPickerDeviceSmoke", "LetterHintPunctuationDeviceSmoke:english-hold"]
        run = Run(base, "collect", [], fail=" ".join(failing), fail_scripts="clipboard-export")
        run.expect(run.code == 1, "有失败时应以 1 退出")
        run.expect("acceptance passed" not in run.stdout, "有失败时不应打印通过说明")
        run.expect(run.ran("LetterHintPunctuationDeviceSmoke:english-swipe") and run.ran_script("clipboard-edit-lifecycle"), "失败之后互不依赖的检查应继续跑完")
        failed = run.failed_lines()
        expected = [
            "FAILED Candidate canvas acceptance failed: MSIME_DEVICE_SMOKE_FAILED: fake stage of CandidateCanvasDeviceSmoke (AssertionError) Expected synthetic UI state was not observed",
            "FAILED Clipboard export acceptance failed: MSIME_DEVICE_SMOKE_FAILED: clipboard-export phase (AssertionError) fake",
            "FAILED Emoji picker acceptance failed: MSIME_DEVICE_SMOKE_FAILED: fake stage of EmojiPickerDeviceSmoke (AssertionError) Expected synthetic UI state was not observed",
            "FAILED English letter hold acceptance failed: MSIME_DEVICE_SMOKE_FAILED: fake stage of LetterHintPunctuationDeviceSmoke:english-hold (AssertionError) Expected synthetic UI state was not observed",
        ]
        run.expect(failed == expected, f"汇总应按发生顺序列出每项失败及其第一行信息，实际为 {failed}")
        run.expect(run.stopped_line() == "", "独立失败不应让整套停下")
        for line in expected:
            run.expect(f"- `{line[len('FAILED '):]}`" in run.summary, f"step summary 缺少 {line}")
        after = run.calls[run.calls.index("instrument CandidateCanvasDeviceSmoke") + 1:]
        run.expect(f"adb -s {SERIAL} shell am force-stop app.msime.android.test" in after and f"adb -s {SERIAL} shell ime set {IME}" in after, "失败后应收起合成编辑器并重新选中输入法")

        run = Run(base, "gate", [], fail="DeviceSmoke")
        run.expect(run.code == 1 and "system input path failed" in run.stopped_line(), "系统输入主路径失败应当场停下并说明原因")
        run.expect(not run.ran("FourteenKeyDeviceSmoke:eighteen_key"), "主路径失败后不应再跑后面的检查")

        run = Run(base, "crash", [], crash="NumberLayerRestartDeviceSmoke")
        run.expect(run.code == 1 and "crashed" in run.stopped_line(), "instrumentation 崩溃应停下并说明原因")
        run.expect(run.failed_lines() == ["FAILED Number layer restart acceptance failed: INSTRUMENTATION_RESULT: shortMsg=Process crashed."], "崩溃那一项应带着崩溃行进汇总")
        run.expect(not run.ran("CompositionRestartDeviceSmoke"), "崩溃后不应再跑后面的检查")
        run.expect("Stopped: the instrumentation process crashed" in run.summary, "停下的原因也要写进 step summary")

        run = Run(base, "emulator-gone", [], fail="BatchCompositionDeviceSmoke", die_after="BatchCompositionDeviceSmoke")
        run.expect(run.code == 1 and "emulator is gone" in run.stopped_line(), "模拟器不在了应停下并说明原因")
        run.expect(not run.ran("NumberLayerRestartDeviceSmoke"), "模拟器不在后不应再跑后面的检查")

        run = Run(base, "ime-lost", [], fail="BatchCompositionDeviceSmoke", ime_stuck_after="BatchCompositionDeviceSmoke")
        run.expect(run.code == 1 and "could not be selected again" in run.stopped_line(), "失败后选不回输入法应停下并说明原因")
        run.expect(not run.ran("NumberLayerRestartDeviceSmoke"), "选不回输入法后不应再跑后面的检查")

        streak = ["BatchCompositionDeviceSmoke", "NumberLayerRestartDeviceSmoke", "CompositionRestartDeviceSmoke", "ChineseShiftReturnDeviceSmoke", "ClipboardSegmentActionsDeviceSmoke"]
        run = Run(base, "streak", ["--core"], sdk="28", fail=" ".join(streak))
        run.expect(run.code == 1 and "five checks in a row" in run.stopped_line(), "连续五项失败应停下并说明原因")
        run.expect(len(run.failed_lines()) == 5 and not run.ran("ClipboardSegmentActionsDeviceSmoke:phrase-notice"), "第五项失败后不应再跑后面的检查")

        run = Run(base, "core-separated", ["--core"], sdk="28", fail="BatchCompositionDeviceSmoke CompositionRestartDeviceSmoke ClipboardSegmentActionsDeviceSmoke CandidateCanvasDeviceSmoke TextEditRecentClipboardDeviceSmoke:trackpad")
        run.expect(run.code == 1 and run.stopped_line() == "" and len(run.failed_lines()) == 5, "不连续的失败不触发连续失败上限")
        run.expect(run.ran_script("clipboard-edit-lifecycle"), "核心套件失败后仍应跑到剪贴板保存生命周期")

        run = Run(base, "core-pass", ["--core"], sdk="28")
        run.expect(run.code == 0 and run.stdout.rstrip().endswith("Dedicated Android AVD: install, resource setup, core typing and clipboard save lifecycle acceptance passed"), "核心套件全部通过时保持原来的通过说明")
        run.expect(not run.ran("EmojiPickerDeviceSmoke"), "核心套件不跑完整套件的检查")
        run.expect(f"script voice-lifecycle {SERIAL} hold" in run.calls, "最低 API 的核心验收也应调用长按语音锁定专项")

    print("Android device smoke failure collection: passed")


if __name__ == "__main__":
    main()
